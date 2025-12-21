//! MSI (Message Signaled Interrupt) Support
//!
//! This module provides MSI support for PLIC and other interrupt controllers,
//! enabling direct interrupt delivery through memory writes.

use crate::{Result, Error};
use crate::core::irq::IrqNumber;
use crate::core::mm::{PhysAddr, VirtAddr, PAGE_SIZE};
use crate::core::sync::SpinLock;
use alloc::vec::Vec;
use alloc::boxed::Box;

/// Simple volatile memory access helper
fn read_volatile_u32(addr: VirtAddr) -> u32 {
    unsafe { core::ptr::read_volatile(addr as *const u32) }
}

/// Simple volatile memory write helper
fn write_volatile_u32(addr: VirtAddr, value: u32) {
    unsafe { core::ptr::write_volatile(addr as *mut u32, value) }
}

/// Simple volatile memory access helper for 64-bit
fn read_volatile_u64(addr: VirtAddr) -> u64 {
    unsafe { core::ptr::read_volatile(addr as *const u64) }
}

/// Simple volatile memory write helper for 64-bit
fn write_volatile_u64(addr: VirtAddr, value: u64) {
    unsafe { core::ptr::write_volatile(addr as *mut u64, value) }
}

/// MSI address format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsiAddress {
    /// Physical address
    pub addr: PhysAddr,
    /// Data value to write
    pub data: u32,
    /// Interrupt vector
    pub vector: u8,
    /// Whether this is a 64-bit address
    pub is_64bit: bool,
}

impl MsiAddress {
    /// Create a new MSI address
    pub fn new(addr: PhysAddr, data: u32, vector: u8) -> Self {
        Self {
            addr,
            data,
            vector,
            is_64bit: false,
        }
    }

    /// Create a 64-bit MSI address
    pub fn new_64bit(addr: PhysAddr, data: u32, vector: u8) -> Self {
        Self {
            addr,
            data,
            vector,
            is_64bit: true,
        }
    }

    /// Get the base address for MSI writes
    pub fn base_addr(&self) -> PhysAddr {
        self.addr & !(PAGE_SIZE - 1)
    }

    /// Get the offset within the page
    pub fn offset(&self) -> u32 {
        (self.addr & (PAGE_SIZE - 1)) as u32
    }

    /// Extract the interrupt ID from MSI data
    pub fn extract_irq_id(&self) -> IrqNumber {
        // Standard MSI format: bits [7:0] contain the vector
        let irq_id = self.data & 0xFF;
        irq_id as IrqNumber
    }

    /// Check if this MSI is valid
    pub fn is_valid(&self) -> bool {
        // Check if address is properly aligned
        self.addr & 0x3 == 0
    }
}

/// MSI controller interface
pub trait MsiController {
    /// Allocate an MSI vector
    fn allocate_vector(&mut self, irq: IrqNumber) -> Result<MsiAddress>;

    /// Free an MSI vector
    fn free_vector(&mut self, msi_addr: &MsiAddress) -> Result<()>;

    /// Enable MSI for an interrupt
    fn enable_msi(&mut self, irq: IrqNumber, msi_addr: &MsiAddress) -> Result<()>;

    /// Disable MSI for an interrupt
    fn disable_msi(&mut self, irq: IrqNumber) -> Result<()>;

    /// Write MSI data to trigger interrupt
    fn trigger_msi(&self, msi_addr: &MsiAddress) -> Result<()>;

    /// Get MSI configuration
    fn get_msi_config(&self) -> MsiConfig;
}

/// MSI configuration
#[derive(Debug, Clone, Copy)]
pub struct MsiConfig {
    /// MSI base address
    pub base_addr: PhysAddr,
    /// Size of MSI region
    pub size: u64,
    /// Number of vectors supported
    pub num_vectors: u32,
    /// Whether multi-message MSI is supported
    pub multi_message: bool,
    /// Address alignment requirement
    pub address_alignment: u32,
    /// Data alignment requirement
    pub data_alignment: u32,
}

impl Default for MsiConfig {
    fn default() -> Self {
        Self {
            base_addr: 0xfee0_0000,  // Standard MSI address range
            size: PAGE_SIZE as u64,
            num_vectors: 32,
            multi_message: false,
            address_alignment: 4,
            data_alignment: 4,
        }
    }
}

/// Simple MSI controller implementation
pub struct SimpleMsiController {
    /// MSI configuration
    config: MsiConfig,
    /// Allocated vectors
    allocated_vectors: SpinLock<Vec<bool>>,
    /// MSI mappings
    msi_mappings: SpinLock<Vec<(IrqNumber, MsiAddress)>>,
    /// Base MMIO region
    mmio_base: VirtAddr,
}

impl SimpleMsiController {
    /// Create a new MSI controller
    pub fn new(config: MsiConfig, mmio_base: VirtAddr) -> Self {
        Self {
            config,
            allocated_vectors: SpinLock::new(Vec::new()),
            msi_mappings: SpinLock::new(Vec::new()),
            mmio_base,
        }
    }

    /// Find a free vector
    fn find_free_vector(&self) -> Result<u32> {
        let mut allocated = self.allocated_vectors.lock();

        // Initialize vector if needed
        if allocated.is_empty() {
            allocated.resize(self.config.num_vectors as usize, false).unwrap();
        }

        // Find first free vector
        for (i, &used) in allocated.iter().enumerate() {
            if !used {
                allocated[i] = true;
                return Ok(i as u32);
            }
        }

        Err(Error::ResourceBusy)
    }

    /// Free a vector
    fn free_vector_internal(&self, vector: u32) -> Result<()> {
        let mut allocated = self.allocated_vectors.lock();

        if vector < allocated.len() as u32 {
            allocated[vector as usize] = false;
            Ok(())
        } else {
            Err(Error::InvalidArgument)
        }
    }
}

impl MsiController for SimpleMsiController {
    fn allocate_vector(&mut self, irq: IrqNumber) -> Result<MsiAddress> {
        let vector = self.find_free_vector()?;

        // Calculate MSI address
        let msi_addr = self.config.base_addr + (vector as u64 * 16);
        let msi_data = vector | (irq << 8); // Include IRQ in data bits [15:8]

        let msi = if self.config.is_64bit {
            MsiAddress::new_64bit(msi_addr, msi_data as u32, vector as u8)
        } else {
            MsiAddress::new(msi_addr, msi_data as u32, vector as u8)
        };

        // Store mapping
        let mut mappings = self.msi_mappings.lock();
        mappings.push((irq, msi));

        Ok(msi)
    }

    fn free_vector(&mut self, msi_addr: &MsiAddress) -> Result<()> {
        let vector = msi_addr.vector as u32;
        self.free_vector_internal(vector)?;

        // Remove mapping
        let mut mappings = self.msi_mappings.lock();
        mappings.retain(|(_, addr)| addr.vector != msi_addr.vector);

        Ok(())
    }

    fn enable_msi(&mut self, irq: IrqNumber, msi_addr: &MsiAddress) -> Result<()> {
        // In a real implementation, this would configure the interrupt controller
        // to accept MSI for the given IRQ
        crate::info!("Enabling MSI for IRQ {} at address {:#x}", irq, msi_addr.addr);
        Ok(())
    }

    fn disable_msi(&mut self, irq: IrqNumber) -> Result<()> {
        // Find and remove MSI mapping for this IRQ
        let mut mappings = self.msi_mappings.lock();
        if let Some((_, msi_addr)) = mappings.iter().find(|(mapped_irq, _)| *mapped_irq == irq) {
            self.free_vector(msi_addr)?;
            mappings.retain(|(mapped_irq, _)| *mapped_irq != irq);
        }
        Ok(())
    }

    fn trigger_msi(&self, msi_addr: &MsiAddress) -> Result<()> {
        if !msi_addr.is_valid() {
            return Err(Error::InvalidArgument);
        }

        // Write MSI data to trigger interrupt
        // Using direct volatile access

        if msi_addr.is_64bit {
            // For 64-bit MSI, write the address first
            write_volatile_u64(self.mmio_base + msi_addr.offset() as u64, msi_addr.addr);
        }

        // Write the MSI data to trigger the interrupt
        let data_offset = if msi_addr.is_64bit {
            msi_addr.offset() + 8
        } else {
            msi_addr.offset()
        };

        write_volatile_u32(self.mmio_base + data_offset as u64, msi_addr.data);

        Ok(())
    }

    fn get_msi_config(&self) -> MsiConfig {
        self.config
    }
}

/// MSI-X controller (extended MSI support)
pub struct MsiXController {
    /// MSI-X table base address
    table_base: VirtAddr,
    /// MSI-X pending table base address
    pending_base: VirtAddr,
    /// Number of MSI-X vectors
    num_vectors: u32,
    /// Vector table entries
    vectors: SpinLock<Vec<MsiXVector>>,
}

/// MSI-X vector configuration
#[derive(Debug, Clone, Copy)]
pub struct MsiXVector {
    /// Vector number
    pub vector_num: u32,
    /// MSI-X address
    pub address: PhysAddr,
    /// MSI-X data
    pub data: u32,
    /// Vector control
    pub control: u32,
    /// Whether this vector is masked
    pub masked: bool,
}

impl MsiXVector {
    /// Create a new MSI-X vector
    pub fn new(vector_num: u32, address: PhysAddr, data: u32) -> Self {
        Self {
            vector_num,
            address,
            data,
            control: 0,
            masked: false,
        }
    }

    /// Get the table entry for this vector
    pub fn get_table_entry(&self) -> u64 {
        let mut entry = self.address as u64;
        entry |= (self.data as u64) << 32;
        entry
    }

    /// Set the vector mask
    pub fn set_mask(&mut self, masked: bool) {
        self.masked = masked;
        self.control = if masked { 1 } else { 0 };
    }
}

impl MsiXController {
    /// Create a new MSI-X controller
    pub fn new(table_base: VirtAddr, pending_base: VirtAddr, num_vectors: u32) -> Self {
        let vectors = SpinLock::new(Vec::new());

        Self {
            table_base,
            pending_base,
            num_vectors,
            vectors,
        }
    }

    /// Initialize MSI-X table
    pub fn init(&self) -> Result<()> {
        let mut vectors = self.vectors.lock();
        vectors.resize(self.num_vectors as usize, MsiXVector::new(0, 0, 0)).unwrap();

        // Initialize pending table
        // Using direct volatile access
        for i in 0..self.num_vectors {
            write_volatile_u32(self.pending_base + (i * 4) as u64, 0);
        }

        Ok(())
    }

    /// Configure an MSI-X vector
    pub fn configure_vector(&mut self, vector: u32, address: PhysAddr, data: u32) -> Result<()> {
        if vector >= self.num_vectors {
            return Err(Error::InvalidArgument);
        }

        let mut vectors = self.vectors.lock();
        vectors[vector as usize] = MsiXVector::new(vector, address, data);

        // Update the table entry
        let table_offset = vector * 16; // Each MSI-X entry is 16 bytes
        // Using direct volatile access

        // Write lower 32 bits of address
        write_volatile_u32(self.table_base + table_offset as u64, (address & 0xFFFFFFFF) as u32);

        // Write upper 32 bits of address
        write_volatile_u32(self.table_base + (table_offset + 4) as u64, ((address >> 32) & 0xFFFFFFFF) as u32);

        // Write data
        write_volatile_u32(self.table_base + (table_offset + 8) as u64, data);

        // Write vector control (unmasked)
        write_volatile_u32(self.table_base + (table_offset + 12) as u64, 0);

        Ok(())
    }

    /// Mask/unmask an MSI-X vector
    pub fn mask_vector(&mut self, vector: u32, masked: bool) -> Result<()> {
        if vector >= self.num_vectors {
            return Err(Error::InvalidArgument);
        }

        let mut vectors = self.vectors.lock();
        vectors[vector as usize].set_mask(masked);

        // Update table entry
        let table_offset = vector * 16;
        // Using direct volatile access
        let control = if masked { 1 } else { 0 };
        write_volatile_u32(self.table_base + (table_offset + 12) as u64, control);

        Ok(())
    }

    /// Trigger an MSI-X interrupt
    pub fn trigger_vector(&self, vector: u32) -> Result<()> {
        if vector >= self.num_vectors {
            return Err(Error::InvalidArgument);
        }

        let vectors = self.vectors.lock();
        let msi_vector = &vectors[vector as usize];

        if msi_vector.masked {
            return Err(Error::InvalidState); // Vector is masked
        }

        // Set pending bit
        // Using direct volatile access
        let pending_offset = vector * 4;
        let pending_bit = 1 << (vector % 32);
        write_volatile_u32(self.pending_base + pending_offset as u64, pending_bit);

        // In a real implementation, this would trigger the interrupt through the MSI-X mechanism
        crate::debug!("Triggering MSI-X vector {}", vector);

        Ok(())
    }

    /// Check if a vector is pending
    pub fn is_pending(&self, vector: u32) -> bool {
        if vector >= self.num_vectors {
            return false;
        }

        // Using direct volatile access
        let pending_value = read_volatile_u32(self.pending_base + (vector * 4) as u64);
        (pending_value & (1 << (vector % 32))) != 0
    }

    /// Clear pending bit for a vector
    pub fn clear_pending(&self, vector: u32) -> Result<()> {
        if vector >= self.num_vectors {
            return Err(Error::InvalidArgument);
        }

        // Using direct volatile access
        let pending_offset = vector * 4;
        write_volatile_u32(self.pending_base + pending_offset as u64, 0);

        Ok(())
    }
}

/// Create an MSI controller
pub fn create_msi_controller(base_addr: VirtAddr) -> Result<Box<dyn MsiController>> {
    let config = MsiConfig::default();
    let controller = SimpleMsiController::new(config, base_addr);
    Ok(Box::new(controller))
}

/// Create an MSI-X controller
pub fn create_msix_controller(table_base: VirtAddr, pending_base: VirtAddr, num_vectors: u32) -> Result<MsiXController> {
    let controller = MsiXController::new(table_base, pending_base, num_vectors);
    controller.init()?;
    Ok(controller)
}

/// Helper function to map MSI address to IRQ
pub fn msi_to_irq(msi_addr: &MsiAddress) -> IrqNumber {
    (msi_addr.data >> 8) & 0xFF
}

/// Helper function to create standard MSI address
pub fn create_standard_msi_address(irq: IrqNumber, vector: u8) -> MsiAddress {
    let addr = 0xfee0_0000 + (vector as u64 * 16);
    let data = (irq << 8) | vector as u32;
    MsiAddress::new(addr, data, vector)
}