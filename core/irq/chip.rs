//! Interrupt controller implementations
//!
//! This module provides implementations for various interrupt controllers
//! found on different hardware platforms.

use crate::{Result, Error};
use crate::core::irq::{InterruptController, IrqNumber, Priority};
use crate::core::mm::VirtAddr;
use crate::arch::common::MmioAccess;
use crate::core::sync::SpinLock;
use alloc::vec::Vec;

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
            Priority::Lowest => 0,
            Priority::Low => 1,
            Priority::Normal => 2,
            Priority::High => 3,
            Priority::Highest => 4,
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
        // Standard SiFive PLIC configuration: 256 IRQs, 16 contexts, max priority 7
        let plic = Box::new(Plic::new(0x0c000000, 256, 16, 7));
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

/// PLIC (Platform-Level Interrupt Controller) - RISC-V
pub struct Plic {
    /// Base address for PLIC registers
    base_addr: VirtAddr,
    /// Number of interrupt sources
    num_irqs: usize,
    /// Number of contexts (usually 2 * number of HARTs)
    num_contexts: usize,
    /// Maximum priority level
    max_priority: u8,
    /// Parent interrupt for virtualization
    parent_irq: Option<IrqNumber>,
    /// Global interrupt priorities
    priorities: SpinLock<heapless::Vec<u8, 1024>>,
    /// Pending interrupts bitmap
    pending: SpinLock<heapless::Vec<u32, 32>>, // Up to 1024 interrupts
    /// Enable bits per context
    enables: SpinLock<heapless::Vec<heapless::Vec<u32, 32>, 64>>, // Up to 64 contexts
    /// Priority thresholds per context
    thresholds: SpinLock<heapless::Vec<u8, 64>>,
    /// Claimed interrupts per context
    claimed: SpinLock<heapless::Vec<u32, 64>>,
    /// Completion registers per context
    completed: SpinLock<heapless::Vec<u32, 64>>,
}

/// PLIC context for interrupt delivery
#[derive(Debug, Clone)]
pub struct PlicContext {
    /// Context number
    pub context_id: u32,
    /// Associated VCPU ID (if virtualized)
    pub vcpu_id: Option<u32>,
    /// Mode (M-mode or S-mode)
    pub mode: PlicMode,
    /// Priority threshold
    pub threshold: u8,
    /// Currently claimed interrupt
    pub claimed_irq: Option<IrqNumber>,
}

/// PLIC operating modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlicMode {
    /// Machine mode
    M,
    /// Supervisor mode
    S,
}

/// PLIC register offsets
pub mod plic_regs {
    pub const PRIORITY_BASE: usize = 0x000000;
    pub const PENDING_BASE: usize = 0x001000;
    pub const ENABLE_BASE: usize = 0x002000;
    pub const CONTEXT_BASE: usize = 0x200000;
    pub const CONTEXT_STRIDE: usize = 0x1000;

    pub const THRESHOLD_OFFSET: usize = 0x00;
    pub const CLAIM_OFFSET: usize = 0x04;
    pub const COMPLETE_OFFSET: usize = 0x04;
}

impl Plic {
    /// Create a new PLIC instance
    pub fn new(base_addr: VirtAddr, num_irqs: usize, num_contexts: usize, max_priority: u8) -> Self {
        Self {
            base_addr,
            num_irqs,
            num_contexts,
            max_priority,
            parent_irq: None,
            priorities: SpinLock::new(heapless::Vec::new()),
            pending: SpinLock::new(heapless::Vec::new()),
            enables: SpinLock::new(heapless::Vec::new()),
            thresholds: SpinLock::new(heapless::Vec::new()),
            claimed: SpinLock::new(heapless::Vec::new()),
            completed: SpinLock::new(heapless::Vec::new()),
        }
    }

    /// Create a PLIC with parent interrupt for virtualization
    pub fn new_with_parent(base_addr: VirtAddr, num_irqs: usize, num_contexts: usize,
                          max_priority: u8, parent_irq: IrqNumber) -> Self {
        let mut plic = Self::new(base_addr, num_irqs, num_contexts, max_priority);
        plic.parent_irq = Some(parent_irq);
        plic
    }

    /// Read from PLIC register
    fn read_reg(&self, offset: usize) -> u32 {
        let mmio = MmioAccess;
        mmio.read_u32(self.base_addr + offset as u64)
    }

    /// Write to PLIC register
    fn write_reg(&self, offset: usize, value: u32) {
        let mmio = MmioAccess;
        mmio.write_u32(self.base_addr + offset as u64, value);
    }

    /// Get priority register offset for an interrupt
    fn get_priority_offset(&self, irq: IrqNumber) -> usize {
        plic_regs::PRIORITY_BASE + (irq as usize * 4)
    }

    /// Get enable register offset for a context and interrupt
    fn get_enable_offset(&self, context: u32, irq: IrqNumber) -> usize {
        let word_offset = irq / 32;
        plic_regs::ENABLE_BASE + (context as usize * plic_regs::CONTEXT_STRIDE) + (word_offset * 4)
    }

    /// Get enable bit mask for an interrupt
    fn get_enable_mask(&self, irq: IrqNumber) -> u32 {
        1 << (irq % 32)
    }

    /// Get pending register offset for an interrupt
    fn get_pending_offset(&self, irq: IrqNumber) -> usize {
        let word_offset = irq / 32;
        plic_regs::PENDING_BASE + (word_offset * 4)
    }

    /// Get pending bit mask for an interrupt
    fn get_pending_mask(&self, irq: IrqNumber) -> u32 {
        1 << (irq % 32)
    }

    /// Get context register base address
    fn get_context_base(&self, context: u32) -> usize {
        plic_regs::CONTEXT_BASE + (context as usize * plic_regs::CONTEXT_STRIDE)
    }

    /// Claim an interrupt for a context
    pub fn claim_interrupt(&self, context: u32) -> Option<IrqNumber> {
        if context as usize >= self.num_contexts {
            return None;
        }

        let mut thresholds = self.thresholds.lock();
        let threshold = thresholds[context as usize];
        drop(thresholds);

        // Find highest priority pending interrupt above threshold
        let best_irq = self.find_best_pending_interrupt(threshold);

        if let Some(irq) = best_irq {
            // Mark as claimed
            let mut claimed = self.claimed.lock();
            claimed[context as usize] = irq as u32;

            // Clear pending bit
            self.clear_pending(irq);

            Some(irq)
        } else {
            None
        }
    }

    /// Complete an interrupt for a context
    pub fn complete_interrupt(&self, context: u32, irq: IrqNumber) {
        if context as usize >= self.num_contexts {
            return;
        }

        let mut claimed = self.claimed.lock();
        if claimed[context as usize] == irq as u32 {
            claimed[context as usize] = 0;

            let mut completed = self.completed.lock();
            completed[context as usize] = irq as u32;
        }
    }

    /// Find the best pending interrupt for a given threshold
    fn find_best_pending_interrupt(&self, threshold: u8) -> Option<IrqNumber> {
        let priorities = self.priorities.lock();
        let pending = self.pending.lock();
        let enables = self.enables.lock();

        let mut best_irq: Option<IrqNumber> = None;
        let mut best_priority = 0;

        // Check all pending interrupts
        for word_idx in 0..pending.len() {
            let pending_word = pending[word_idx];
            if pending_word == 0 {
                continue;
            }

            for bit_idx in 0..32 {
                let irq_num = (word_idx * 32 + bit_idx) as IrqNumber;
                if irq_num >= self.num_irqs {
                    break;
                }

                if (pending_word & (1 << bit_idx)) == 0 {
                    continue;
                }

                // Check if this interrupt is enabled for any context
                let mut enabled_for_context = false;
                for ctx_idx in 0..self.num_contexts {
                    if ctx_idx < enables.len() {
                        let ctx_enables = &enables[ctx_idx];
                        let enable_word = irq_num / 32;
                        if enable_word < ctx_enables.len() {
                            if (ctx_enables[enable_word] & (1 << (irq_num % 32))) != 0 {
                                enabled_for_context = true;
                                break;
                            }
                        }
                    }
                }

                if !enabled_for_context {
                    continue;
                }

                // Check priority
                let priority = if (irq_num as usize) < priorities.len() {
                    priorities[irq_num as usize]
                } else {
                    0
                };

                if priority > threshold && priority > best_priority {
                    best_priority = priority;
                    best_irq = Some(irq_num);
                }
            }
        }

        best_irq
    }

    /// Set an interrupt as pending
    pub fn set_pending(&self, irq: IrqNumber) {
        if irq as usize >= self.num_irqs {
            return;
        }

        let offset = self.get_pending_offset(irq);
        let mask = self.get_pending_mask(irq);
        let mut pending = self.pending.lock();

        // Ensure the pending vector is large enough
        while pending.len() <= offset / 4 {
            pending.push(0).unwrap();
        }

        let word_idx = offset / 4 - plic_regs::PENDING_BASE / 4;
        pending[word_idx] |= mask;
    }

    /// Clear an interrupt from pending
    fn clear_pending(&self, irq: IrqNumber) {
        if irq as usize >= self.num_irqs {
            return;
        }

        let offset = self.get_pending_offset(irq);
        let mask = self.get_pending_mask(irq);
        let mut pending = self.pending.lock();

        let word_idx = offset / 4 - plic_regs::PENDING_BASE / 4;
        if word_idx < pending.len() {
            pending[word_idx] &= !mask;
        }
    }

    /// Check if an interrupt is enabled for a context
    fn is_enabled_for_context(&self, irq: IrqNumber, context: u32) -> bool {
        if irq as usize >= self.num_irqs || context as usize >= self.num_contexts {
            return false;
        }

        let enables = self.enables.lock();
        if (context as usize) >= enables.len() {
            return false;
        }

        let ctx_enables = &enables[context as usize];
        let word_idx = irq / 32;
        if word_idx >= ctx_enables.len() {
            return false;
        }

        (ctx_enables[word_idx] & (1 << (irq % 32))) != 0
    }
}

impl InterruptController for Plic {
    fn init(&mut self) -> Result<()> {
        crate::info!("Initializing PLIC with {} IRQs, {} contexts, max priority {}",
                    self.num_irqs, self.num_contexts, self.max_priority);

        // Initialize data structures
        {
            let mut priorities = self.priorities.lock();
            priorities.resize(self.num_irqs, 0).map_err(|_| crate::Error::OutOfMemory)?;
        }

        {
            let mut pending = self.pending.lock();
            pending.resize((self.num_irqs + 31) / 32, 0).map_err(|_| crate::Error::OutOfMemory)?;
        }

        {
            let mut enables = self.enables.lock();
            enables.resize(self.num_contexts, heapless::Vec::new())
                .map_err(|_| crate::Error::OutOfMemory)?;
            for ctx_enables in enables.iter_mut() {
                ctx_enables.resize((self.num_irqs + 31) / 32, 0)
                    .map_err(|_| crate::Error::OutOfMemory)?;
            }
        }

        {
            let mut thresholds = self.thresholds.lock();
            thresholds.resize(self.num_contexts, 0).map_err(|_| crate::Error::OutOfMemory)?;
        }

        {
            let mut claimed = self.claimed.lock();
            claimed.resize(self.num_contexts, 0).map_err(|_| crate::Error::OutOfMemory)?;
        }

        {
            let mut completed = self.completed.lock();
            completed.resize(self.num_contexts, 0).map_err(|_| crate::Error::OutOfMemory)?;
        }

        // Disable all interrupts initially
        for context in 0..self.num_contexts {
            let context_base = self.get_context_base(context as u32);
            self.write_reg(context_base + plic_regs::THRESHOLD_OFFSET, self.max_priority as u32);
        }

        crate::info!("PLIC initialized successfully");
        Ok(())
    }

    fn enable_irq(&mut self, irq: IrqNumber) -> Result<()> {
        if irq as usize >= self.num_irqs {
            return Err(Error::InvalidArgument);
        }

        // Enable for context 0 by default
        self.enable_irq_for_context(irq, 0)
    }

    fn disable_irq(&mut self, irq: IrqNumber) -> Result<()> {
        if irq as usize >= self.num_irqs {
            return Err(Error::InvalidArgument);
        }

        // Disable for context 0 by default
        self.disable_irq_for_context(irq, 0)
    }

    fn ack_irq(&mut self, irq: IrqNumber) -> Result<()> {
        // Complete the interrupt for context 0
        self.complete_interrupt(0, irq);
        Ok(())
    }

    fn set_priority(&mut self, irq: IrqNumber, priority: Priority) -> Result<()> {
        if irq as usize >= self.num_irqs {
            return Err(Error::InvalidArgument);
        }

        let priority_value = match priority {
            Priority::Lowest => 0,
            Priority::Low => 1,
            Priority::Normal => 2,
            Priority::High => 3,
            Priority::Highest => self.max_priority.min(4),
        };

        let offset = self.get_priority_offset(irq);
        self.write_reg(offset, priority_value as u32);

        // Update our shadow copy
        {
            let mut priorities = self.priorities.lock();
            if irq as usize < priorities.len() {
                priorities[irq as usize] = priority_value as u8;
            }
        }

        Ok(())
    }

    fn set_type(&mut self, _irq: IrqNumber, _edge_triggered: bool) -> Result<()> {
        // PLIC doesn't support edge/level configuration per interrupt
        // This is typically handled at the device level
        Err(Error::NotImplemented)
    }

    fn get_pending_irqs(&self) -> u64 {
        let pending = self.pending.lock();
        let mut pending_bitmap = 0u64;

        for (word_idx, &word) in pending.iter().enumerate() {
            for bit_idx in 0..32 {
                let irq_num = (word_idx * 32 + bit_idx) as IrqNumber;
                if irq_num >= self.num_irqs || irq_num >= 64 {
                    break;
                }

                if (word & (1 << bit_idx)) != 0 {
                    pending_bitmap |= 1u64 << irq_num;
                }
            }
        }

        pending_bitmap
    }

    fn is_pending(&self, irq: IrqNumber) -> bool {
        self.get_pending_irqs() & (1u64 << irq) != 0
    }

    fn handle_interrupt(&mut self) -> Option<IrqNumber> {
        // Claim interrupt for context 0
        self.claim_interrupt(0)
    }
}

impl Plic {
    /// Enable interrupt for a specific context
    pub fn enable_irq_for_context(&mut self, irq: IrqNumber, context: u32) -> Result<()> {
        if irq as usize >= self.num_irqs || context as usize >= self.num_contexts {
            return Err(Error::InvalidArgument);
        }

        let offset = self.get_enable_offset(context, irq);
        let mask = self.get_enable_mask(irq);

        // Read current enable register
        let current = self.read_reg(offset);
        self.write_reg(offset, current | mask);

        // Update shadow copy
        {
            let mut enables = self.enables.lock();
            if (context as usize) < enables.len() {
                let ctx_enables = &mut enables[context as usize];
                let word_idx = irq / 32;
                if (word_idx as usize) < ctx_enables.len() {
                    ctx_enables[word_idx as usize] |= mask;
                }
            }
        }

        Ok(())
    }

    /// Disable interrupt for a specific context
    pub fn disable_irq_for_context(&mut self, irq: IrqNumber, context: u32) -> Result<()> {
        if irq as usize >= self.num_irqs || context as usize >= self.num_contexts {
            return Err(Error::InvalidArgument);
        }

        let offset = self.get_enable_offset(context, irq);
        let mask = self.get_enable_mask(irq);

        // Read current enable register
        let current = self.read_reg(offset);
        self.write_reg(offset, current & !mask);

        // Update shadow copy
        {
            let mut enables = self.enables.lock();
            if (context as usize) < enables.len() {
                let ctx_enables = &mut enables[context as usize];
                let word_idx = irq / 32;
                if (word_idx as usize) < ctx_enables.len() {
                    ctx_enables[word_idx as usize] &= !mask;
                }
            }
        }

        Ok(())
    }

    /// Set priority threshold for a context
    pub fn set_context_threshold(&mut self, context: u32, threshold: u8) -> Result<()> {
        if context as usize >= self.num_contexts {
            return Err(Error::InvalidArgument);
        }

        let context_base = self.get_context_base(context);
        self.write_reg(context_base + plic_regs::THRESHOLD_OFFSET, threshold as u32);

        // Update shadow copy
        {
            let mut thresholds = self.thresholds.lock();
            thresholds[context as usize] = threshold;
        }

        Ok(())
    }

    /// Create PLIC contexts for virtualization
    pub fn create_contexts(&mut self, vcpu_count: usize) -> Result<Vec<PlicContext>> {
        let mut contexts = Vec::new();

        // Each VCPU gets 2 contexts: M-mode and S-mode
        for vcpu_id in 0..vcpu_count {
            // M-mode context
            contexts.push(PlicContext {
                context_id: (vcpu_id * 2) as u32,
                vcpu_id: Some(vcpu_id as u32),
                mode: PlicMode::M,
                threshold: 0,
                claimed_irq: None,
            });

            // S-mode context
            contexts.push(PlicContext {
                context_id: (vcpu_id * 2 + 1) as u32,
                vcpu_id: Some(vcpu_id as u32),
                mode: PlicMode::S,
                threshold: 0,
                claimed_irq: None,
            });
        }

        Ok(contexts)
    }

    /// Get the parent IRQ (for virtualization)
    pub fn parent_irq(&self) -> Option<IrqNumber> {
        self.parent_irq
    }

    /// Set interrupt affinity to a specific context
    pub fn set_irq_affinity(&mut self, irq: IrqNumber, context: u32) -> Result<()> {
        if irq as usize >= self.num_irqs || context as usize >= self.num_contexts {
            return Err(Error::InvalidArgument);
        }

        // Disable IRQ for all contexts first
        for ctx in 0..self.num_contexts {
            self.disable_irq_for_context(irq, ctx as u32)?;
        }

        // Enable IRQ only for the target context
        self.enable_irq_for_context(irq, context)?;

        Ok(())
    }

    /// Get the affinity context for an interrupt
    pub fn get_irq_affinity(&self, irq: IrqNumber) -> Option<u32> {
        if irq as usize >= self.num_irqs {
            return None;
        }

        let enables = self.enables.lock();
        for (ctx_idx, ctx_enables) in enables.iter().enumerate() {
            let word_idx = irq / 32;
            if word_idx < ctx_enables.len() {
                if (ctx_enables[word_idx] & (1 << (irq % 32))) != 0 {
                    return Some(ctx_idx as u32);
                }
            }
        }

        None
    }

    /// Set interrupt affinity with load balancing
    pub fn set_irq_affinity_balanced(&mut self, irq: IrqNumber, context_mask: u64) -> Result<()> {
        if irq as usize >= self.num_irqs {
            return Err(Error::InvalidArgument);
        }

        // Disable IRQ for all contexts first
        for ctx in 0..self.num_contexts {
            self.disable_irq_for_context(irq, ctx as u32)?;
        }

        // Enable IRQ for contexts in the mask
        for ctx in 0..self.num_contexts {
            if ((context_mask >> ctx) & 1) != 0 {
                self.enable_irq_for_context(irq, ctx as u32)?;
            }
        }

        Ok(())
    }

    /// Get interrupt statistics
    pub fn get_interrupt_stats(&self) -> PlicStats {
        let pending = self.pending.lock();
        let enables = self.enables.lock();

        let mut stats = PlicStats {
            total_interrupts: self.num_irqs as u64,
            pending_interrupts: 0,
            enabled_interrupts: 0,
            claimed_interrupts: 0,
            contexts: self.num_contexts as u64,
        };

        // Count pending interrupts
        for word in pending.iter() {
            stats.pending_interrupts += word.count_ones() as u64;
        }

        // Count enabled interrupts
        for ctx_enables in enables.iter() {
            for word in ctx_enables.iter() {
                stats.enabled_interrupts += word.count_ones() as u64;
            }
        }

        // Count claimed interrupts
        let claimed = self.claimed.lock();
        for &claimed_irq in claimed.iter() {
            if claimed_irq != 0 {
                stats.claimed_interrupts += 1;
            }
        }

        stats
    }

    /// Migrate interrupts from one context to another
    pub fn migrate_interrupts(&mut self, from_context: u32, to_context: u32) -> Result<u64> {
        if from_context as usize >= self.num_contexts || to_context as usize >= self.num_contexts {
            return Err(Error::InvalidArgument);
        }

        let mut migrated = 0;

        // Find all interrupts enabled for the source context
        let enables = self.enables.lock();
        if (from_context as usize) < enables.len() {
            let ctx_enables = enables[from_context as usize].clone();
            drop(enables);

            // Migrate each enabled interrupt
            for (word_idx, &enable_word) in ctx_enables.iter().enumerate() {
                for bit_idx in 0..32 {
                    let irq_num = (word_idx * 32 + bit_idx) as IrqNumber;
                    if irq_num >= self.num_irqs {
                        break;
                    }

                    if (enable_word & (1 << bit_idx)) != 0 {
                        // Disable for source context
                        self.disable_irq_for_context(irq_num, from_context)?;
                        // Enable for target context
                        self.enable_irq_for_context(irq_num, to_context)?;
                        migrated += 1;
                    }
                }
            }
        }

        Ok(migrated)
    }

    /// Balance interrupt load across contexts
    pub fn balance_interrupt_load(&mut self) -> Result<Vec<(u32, u64)>> {
        let enables = self.enables.lock();
        let mut context_loads: Vec<(u32, u64)> = Vec::new();

        // Count interrupts per context
        for (ctx_idx, ctx_enables) in enables.iter().enumerate() {
            let mut load = 0;
            for word in ctx_enables.iter() {
                load += word.count_ones() as u64;
            }
            context_loads.push((ctx_idx as u32, load));
        }

        // Sort by load (least loaded first)
        context_loads.sort_by_key(|(_, load)| *load);

        Ok(context_loads)
    }
}

/// PLIC statistics
#[derive(Debug, Clone, Copy)]
pub struct PlicStats {
    /// Total number of interrupt sources
    pub total_interrupts: u64,
    /// Number of pending interrupts
    pub pending_interrupts: u64,
    /// Number of enabled interrupts
    pub enabled_interrupts: u64,
    /// Number of claimed interrupts
    pub claimed_interrupts: u64,
    /// Number of contexts
    pub contexts: u64,
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