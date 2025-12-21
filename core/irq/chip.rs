//! Interrupt controller implementations
//!
//! This module provides implementations for various interrupt controllers
//! found on different hardware platforms.

use crate::{Result, Error};
use crate::core::irq::{InterruptController, IrqNumber, IrqPriority};
use crate::core::mm::VirtAddr;
use crate::arch::common::MmioAccess;
use crate::utils::spinlock::SpinLock;

/// GIC (Generic Interrupt Controller) - ARM
pub struct Gic {
    /// Base address for GIC distributor
    distributor_base: VirtAddr,
    /// Base address for GIC CPU interfaces
    cpu_base: VirtAddr,
    /// Number of interrupt lines
    num_irqs: usize,
    /// Enable register state
    enabled: SpinLock<u32>,
    /// Priority register state
    priorities: SpinLock<Vec<u8>>,
    /// Target register state
    targets: SpinLock<Vec<u8>>,
}

impl Gic {
    /// Create a new GIC instance
    pub fn new(distributor_base: VirtAddr, cpu_base: VirtAddr, num_irqs: usize) -> Self {
        Self {
            distributor_base,
            cpu_base,
            num_irqs,
            enabled: SpinLock::new(0),
            priorities: SpinLock::new(vec![0; num_irqs]),
            targets: SpinLock::new(vec![0; num_irqs]),
        }
    }

    /// Get the distributor base address
    pub fn distributor_base(&self) -> VirtAddr {
        self.distributor_base
    }

    /// Get the CPU interface base address
    pub fn cpu_base(&self) -> VirtAddr {
        self.cpu_base
    }

    /// Read from GIC distributor register
    fn read_distributor_reg(&self, offset: u32) -> u32 {
        let mmio = MmioAccess;
        mmio.read_u32(self.distributor_base + offset as u64)
    }

    /// Write to GIC distributor register
    fn write_distributor_reg(&self, offset: u32, value: u32) {
        let mmio = MmioAccess;
        mmio.write_u32(self.distributor_base + offset as u64, value);
    }

    /// Read from GIC CPU interface register
    fn read_cpu_reg(&self, offset: u32) -> u32 {
        let mmio = MmioAccess;
        mmio.read_u32(self.cpu_base + offset as u64)
    }

    /// Write to GIC CPU interface register
    fn write_cpu_reg(&self, offset: u32, value: u32) {
        let mmio = MmioAccess;
        mmio.write_u32(self.cpu_base + offset as u64, value);
    }

    /// Get SPI interrupt base (start of SPI range)
    fn get_spi_base(&self) -> u32 {
        32 // Standard GIC starts SPI at 32
    }

    /// Get PPI interrupt base (start of PPI range)
    fn get_ppi_base(&self) -> u32 {
        self.get_spi_base() + self.num_irqs as u32
    }
}

impl InterruptController for Gic {
    fn init(&mut self) -> Result<()> {
        crate::info!("Initializing GIC");

        // Disable all interrupts first
        self.write_distributor_reg(0x000, 0);

        // Configure interrupt priorities
        for irq in 0..self.num_irqs {
            let reg_offset = 0x400 + (irq as u32) * 4;
            let priority = match irq {
                0..=31 => 0, // SGI - Software Generated Interrupts
                _ => 1, // Others - Low priority
            };
            self.write_distributor_reg(reg_offset, priority);
        {
            let mut priorities = self.priorities.lock();
            priorities[irq] = priority as u8;
        }
        }

        // Set CPU targets (all to CPU 0 for now)
        for irq in 0..self.num_irqs {
            let reg_offset = 0x800 + (irq as u32) * 4;
            self.write_distributor_reg(reg_offset, 1); // Target CPU 0
            {
                let mut targets = self.targets.lock();
                targets[irq] = 1;
            }
        }

        // Enable GIC distributor
        self.write_distributor_reg(0x000, 1);

        // Enable CPU interface
        self.write_cpu_reg(0x000, 0);

        // Set priority mask
        self.write_cpu_reg(0x004, 0xF0); // Allow all priorities

        // Enable group 0 interrupts
        self.write_cpu_reg(0x008, 0xF0);

        // Enable group 1 interrupts
        self.write_cpu_reg(0x00C, 0xF0);

        crate::info!("GIC initialized with {} IRQ lines", self.num_irqs);

        Ok(())
    }

    fn enable_irq(&mut self, irq: IrqNumber) -> Result<()> {
        if irq as usize >= self.num_irqs {
            return Err(Error::InvalidArgument);
        }

        let mut enabled = self.enabled.lock();
        let spi_base = self.get_spi_base();
        let spi_irq = irq as u32;

        if spi_irq >= spi_base {
            // SPI interrupt
            let enable_offset = 0x100 + ((spi_irq - 32) / 32);
            let enable_mask = 1 << ((spi_irq - 32) % 32);
            self.write_distributor_reg(enable_offset, enable_mask);
        } else {
            // SGI (Software Generated Interrupt)
            let enable_offset = 0x100;
            let enable_mask = 1 << irq;
            self.write_distributor_reg(enable_offset, enable_mask);
        }

        *enabled |= 1 << irq;
        Ok(())
    }

    fn disable_irq(&mut self, irq: IrqNumber) -> Result<()> {
        if irq as usize >= self.num_irqs {
            return Err(Error::InvalidArgument);
        }

        let mut enabled = self.enabled.lock();
        let spi_base = self.get_spi_base();
        let spi_irq = irq as u32;

        if spi_irq >= spi_base {
            // SPI interrupt
            let enable_offset = 0x100 + ((spi_irq - 32) / 32);
            let enable_mask = !(1 << ((spi_irq - 32) % 32));
            self.write_distributor_reg(enable_offset, enable_mask);
        } else {
            // SGI (Software Generated Interrupt)
            let enable_offset = 0x100;
            let enable_mask = !(1 << irq);
            self.write_distributor_reg(enable_offset, enable_mask);
        }

        *enabled &= !(1 << irq);
        Ok(())
    }

    fn ack_irq(&mut self, irq: IrqNumber) -> Result<()> {
        // Write to End Of Interrupt register
        self.write_cpu_reg(0x010, irq as u32);
        Ok(())
    }

    fn set_priority(&mut self, irq: IrqNumber, priority: IrqPriority) -> Result<()> {
        if irq as usize >= self.num_irqs {
            return Err(Error::InvalidArgument);
        }

        let reg_offset = 0x400 + (irq as u32) * 4;
        let priority_value = match priority {
            IrqPriority::Lowest => 0,
            IrqPriority::Low => 1,
            IrqPriority::Normal => 2,
            IrqPriority::High => 3,
            IrqPriority::Highest => 4,
        };

        self.write_distributor_reg(reg_offset, priority_value);
        {
            let mut priorities = self.priorities.lock();
            priorities[irq as usize] = priority_value as u8;
        }

        Ok(())
    }

    fn set_type(&mut self, irq: IrqNumber, edge_triggered: bool) -> Result<()> {
        if irq as usize >= self.num_irqs {
            return Err(Error::InvalidArgument);
        }

        // GIC uses interrupt configuration registers for type configuration
        // This is a simplified implementation
        // Real implementation would be more complex

        Ok(())
    }

    fn get_pending_irqs(&self) -> u64 {
        // Read the Interrupt Acknowledge Register
        let iar = self.read_cpu_reg(0x0C);
        let pending = iar & 0x3FF; // Lower 10 bits for interrupt ID

        if pending != 1023 {
            // Valid interrupt ID
            crate::debug!("GIC pending interrupt: {}", pending);
            return 1u64 << pending;
        }

        0
    }

    fn is_pending(&self, irq: IrqNumber) -> bool {
        // This would require checking the appropriate register
        // For now, just check if it's in the pending list
        self.get_pending_irqs() & (1u64 << irq) != 0
    }

    fn handle_interrupt(&mut self) -> Option<IrqNumber> {
        let iar = self.read_cpu_reg(0x0C);
        let interrupt_id = iar & 0x3FF;

        if interrupt_id != 1023 {
            // Valid interrupt
            return Some(interrupt_id as IrqNumber);
        }

        None
    }
}

/// APIC (Advanced Programmable Interrupt Controller) - x86
pub struct Apic {
    /// Base address
    base_addr: VirtAddr,
    /// Local APIC ID
    lapic_id: u8,
    /// Task priority register
    tpr: SpinLock<u32>,
    /// Local vector table
    lvt: SpinLock<[u32; 256]>,
    /// In-service register
    isr: SpinLock<u32>,
    /// End of interrupt register
    eoi: SpinLock<u32>,
}

impl Apic {
    /// Create a new APIC instance
    pub fn new(base_addr: VirtAddr) -> Self {
        Self {
            base_addr,
            lapic_id: 0,
            tpr: SpinLock::new(0),
            lvt: SpinLock::new([0; 256]),
            isr: SpinLock::new(0),
            eoi: SpinLock::new(0),
        }
    }

    /// Read from APIC register
    fn read_reg(&self, offset: u32) -> u32 {
        let mmio = MmioAccess;
        mmio.read_u32(self.base_addr + offset as u64)
    }

    /// Write to APIC register
    fn write_reg(&self, offset: u32, value: u32) {
        let mmio = MmioAccess;
        mmio.write_u32(self.base_addr + offset as u64, value);
    }
}

impl InterruptController for Apic {
    fn init(&mut self) -> Result<()> {
        crate::info!("Initializing APIC");

        // Read Local APIC ID
        let apic_id = self.read_reg(0x020);
        self.lapic_id = ((apic_id >> 24) & 0xFF) as u8;

        // Configure task priority register
        self.write_reg(0x80, 0x00); // Accept all interrupts

        // Set up Local Vector Table
        for vector in 0..=255 {
            let entry = match vector {
                0..=31 => 0x100 | vector as u32, // External interrupts
                32..=39 => 0x000 | ((vector - 32) << 4), // Timer interrupts
                64..=255 => 0x000 | vector as u32, // Other interrupts
                _ => 0,
            };
            {
                let mut lvt = self.lvt.lock();
                lvt[vector as usize] = entry;
            }
        }

        // Enable APIC
        let spurious_vector = 0xFF;
        self.write_reg(0xF0, spurious_vector);
        self.write_reg(0x80, 0x1FF); // Enable all

        // Clear EOI
        self.write_reg(0xB0, 0);

        crate::info!("APIC initialized, Local ID: {}", self.lapic_id);

        Ok(())
    }

    fn enable_irq(&mut self, irq: IrqNumber) -> Result<()> {
        if irq >= 256 {
            return Err(Error::InvalidArgument);
        }

        // Set bit in Interrupt Mask Register
        let imr = self.read_reg(0xF0);
        let new_imr = imr & !(1 << irq);
        self.write_reg(0xF0, new_imr);

        Ok(())
    }

    fn disable_irq(&mut self, irq: IrqNumber) -> Result<()> {
        if irq >= 256 {
            return Err(Error::InvalidArgument);
        }

        // Clear bit in Interrupt Mask Register
        let imr = self.read_reg(0xF0);
        let new_imr = imr | (1 << irq);
        self.write_reg(0xF0, new_imr);

        Ok(())
    }

    fn ack_irq(&mut self, irq: IrqNumber) -> Result<()> {
        // Write to End Of Interrupt register
        self.write_reg(0xB0, irq as u32);
        Ok(())
    }

    fn set_priority(&mut self, _irq: IrqNumber, _priority: IrqPriority) -> Result<()> {
        // APIC uses Task Priority Register for global priority
        // Individual interrupt priorities are not directly configurable
        Err(Error::NotImplemented)
    }

    fn set_type(&mut self, _irq: IrqNumber, _edge_triggered: bool) -> Result<()> {
        // APIC LVT entries contain trigger mode bits
        Err(Error::NotImplemented)
    }

    fn get_pending_irqs(&self) -> u64 {
        // Read In-Service Register
        let isr = self.read_reg(0x100);
        // Convert to bitmap format
        let mut pending = 0u64;

        for bit in 0..32 {
            if (isr & (1 << bit)) != 0 {
                pending |= 1u64 << bit;
            }
        }

        // Check Local Interrupt Status Register
        let lisr = self.read_reg(0x350);
        for bit in 0..7 {
            if (lisr & (1 << bit)) != 0 {
                pending |= 1u64 << (bit + 16);
            }
        }

        pending
    }

    fn is_pending(&self, irq: IrqNumber) -> bool {
        self.get_pending_irqs() & (1u64 << irq) != 0
    }

    fn handle_interrupt(&mut self) -> Option<IrqNumber> {
        // Read In-Service Register
        let isr = self.read_reg(0x100);
        if isr != 0 {
            // Get the interrupt number from ISR
            let irq = isr.trailing_zeros() as IrqNumber;
            Some(irq)
        } else {
            // Check Local Interrupt Status Register
            let lisr = self.read_reg(0x350);
            if lisr != 0 {
                let irq = lisr.trailing_zeros() as IrqNumber;
                Some(irq + 16) // LSR bits 16-22 map to IRQs 16-22
            } else {
                None
            }
        }
    }
}

/// Generic interrupt controller that can be used for simple implementations
pub struct GenericController {
    /// Base address
    base_addr: VirtAddr,
    /// Number of IRQ lines
    num_irqs: usize,
    /// Enabled interrupts bitmap
    enabled: SpinLock<u64>,
    /// Pending interrupts bitmap
    pending: SpinLock<u64>,
}

impl GenericController {
    /// Create a new generic controller
    pub fn new(base_addr: VirtAddr, num_irqs: usize) -> Self {
        Self {
            base_addr,
            num_irqs,
            enabled: SpinLock::new(0),
            pending: SpinLock::new(0),
        }
    }

    /// Read a register
    fn read_reg(&self, offset: usize) -> u32 {
        let mmio = MmioAccess;
        mmio.read_u32(self.base_addr + offset as u64)
    }

    /// Write a register
    fn write_reg(&self, offset: usize, value: u32) {
        let mmio = MmioAccess;
        mmio.write_u32(self.base_addr + offset as u64, value);
    }
}

impl InterruptController for GenericController {
    fn init(&mut self) -> Result<()> {
        crate::info!("Initializing Generic interrupt controller");

        // Clear all registers
        for offset in (0..(self.num_irqs / 32)).step_by(4) {
            self.write_reg(offset as usize, 0);
        }

        Ok(())
    }

    fn enable_irq(&mut self, irq: IrqNumber) -> Result<()> {
        if irq as usize >= self.num_irqs {
            return Err(Error::InvalidArgument);
        }

        let mut enabled = self.enabled.lock();
        *enabled |= 1u64 << irq;
        Ok(())
    }

    fn disable_irq(&mut self, irq: IrqNumber) -> Result<()> {
        if irq as usize >= self.num_irqs {
            return Err(Error::InvalidArgument);
        }

        let mut enabled = self.enabled.lock();
        *enabled &= !(1u64 << irq);
        Ok(())
    }

    fn ack_irq(&mut self, _irq: IrqNumber) -> Result<()> {
        // Generic controller may not need acking
        Ok(())
    }

    fn set_priority(&mut self, _irq: IrqNumber, _priority: IrqPriority) -> Result<()> {
        Err(Error::NotImplemented)
    }

    fn set_type(&mut self, _irq: IrqNumber, _edge_triggered: bool) -> Result<()> {
        Err(Error::NotImplemented)
    }

    fn get_pending_irqs(&self) -> u64 {
        *self.pending.lock()
    }

    fn is_pending(&self, irq: IrqNumber) -> bool {
        self.get_pending_irqs() & (1u64 << irq) != 0
    }

    fn handle_interrupt(&mut self) -> Option<IrqNumber> {
        let pending = self.get_pending_irqs();
        if pending != 0 {
            // Get the highest priority pending interrupt
            let irq = pending.trailing_zeros() as IrqNumber;
            if irq < self.num_irqs {
                // Clear the pending bit
                let mut pending = self.pending.lock();
                *pending &= !(1u64 << irq);
                return Some(irq);
            }
        }
        None
    }
}

/// Create an appropriate interrupt controller for the current platform
pub fn create_interrupt_controller() -> Result<*mut dyn InterruptController> {
    #[cfg(target_arch = "aarch64")]
    {
        // Use GIC for ARM64
        let gic = Box::new(Gic::new(0x08010000, 0x08020000, 128));
        Ok(Box::into_raw(gic) as *mut dyn InterruptController)
    }

    #[cfg(target_arch = "riscv64")]
    {
        // Use PLIC for RISC-V
        let plic = Box::new(GenericController::new(0x0c000000, 256));
        Ok(Box::into_raw(plic) as *mut dyn InterruptController)
    }

    #[cfg(target_arch = "x86_64")]
    {
        // Use APIC for x86_64
        let apic = Box::new(Apic::new(0xFEC00000));
        Ok(Box::into_raw(apic) as *mut InterruptController)
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "riscv64", target_arch = "x86_64")))]
    {
        // Use generic controller as fallback
        let generic = Box::new(GenericController::new(0xF0000000, 64));
        Ok(Box::into_raw(generic) as *mut InterruptController)
    }
}

/// Set up the interrupt controller for the current platform
pub fn setup_interrupt_controller() -> Result<()> {
    let controller = create_interrupt_controller()?;

    {
        let mut manager = crate::core::irq::get();
        manager.set_controller(controller);
    }

    Ok(())
}