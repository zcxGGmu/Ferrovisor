//! RISC-V Interrupt Controller Support
//!
//! This module provides interrupt controller support including:
//! - PLIC (Platform-Level Interrupt Controller)
/// - ACLINT (Advanced Core Local Interruptor)
/// - APLIC (Advanced Platform-Level Interrupt Controller)
/// - IMSIC (Incoming Message Signaled Interrupt Controller)

use crate::arch::riscv64::*;
use bitflags::bitflags;

// Helper trait for downcasting
pub trait AsAny {
    fn as_any(&self) -> &dyn core::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn core::any::Any;
}

impl<T: core::any::Any> AsAny for T {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn core::any::Any {
        self
    }
}

/// Interrupt controller types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptControllerType {
    /// PLIC - Platform-Level Interrupt Controller
    PLIC,
    /// ACLINT - Advanced Core Local Interruptor
    ACLINT,
    /// APLIC - Advanced Platform-Level Interrupt Controller
    APLIC,
    /// IMSIC - Incoming Message Signaled Interrupt Controller
    IMSIC,
    /// Legacy local interrupt controller
    Legacy,
}

/// Interrupt controller trait
pub trait InterruptController: AsAny {
    /// Initialize the interrupt controller
    fn init(&mut self) -> Result<(), &'static str>;

    /// Enable an interrupt
    fn enable(&mut self, irq: u32, cpu: usize) -> Result<(), &'static str>;

    /// Disable an interrupt
    fn disable(&mut self, irq: u32, cpu: usize) -> Result<(), &'static str>;

    /// Set interrupt priority
    fn set_priority(&mut self, irq: u32, priority: u8) -> Result<(), &'static str>;

    /// Get interrupt priority
    fn get_priority(&self, irq: u32) -> Result<u8, &'static str>;

    /// Claim an interrupt
    fn claim(&mut self, cpu: usize) -> Result<u32, &'static str>;

    /// Complete an interrupt
    fn complete(&mut self, irq: u32, cpu: usize) -> Result<(), &'static str>;

    /// Set interrupt affinity
    fn set_affinity(&mut self, irq: u32, cpu: usize) -> Result<(), &'static str>;

    /// Get controller type
    fn controller_type(&self) -> InterruptControllerType;
}

/// PLIC registers
pub mod plic {
    /// PLIC priority register offset
    pub const PRIORITY_OFFSET: usize = 0x000000;
    /// PLIC pending register offset
    pub const PENDING_OFFSET: usize = 0x001000;
    /// PLIC enable register offset
    pub const ENABLE_OFFSET: usize = 0x002000;
    /// PLIC threshold register offset
    pub const THRESHOLD_OFFSET: usize = 0x200000;
    /// PLIC claim/complete register offset
    pub const CLAIM_COMPLETE_OFFSET: usize = 0x200004;
}

/// PLIC (Platform-Level Interrupt Controller)
pub struct PlicController {
    /// Base address
    base: usize,
    /// Number of interrupt sources
    num_sources: u32,
    /// Number of contexts (targets)
    num_contexts: u32,
    /// Maximum priority value
    max_priority: u32,
}

impl PlicController {
    /// Create a new PLIC controller
    pub fn new(base: usize, num_sources: u32, num_contexts: u32) -> Self {
        Self {
            base,
            num_sources,
            num_contexts,
            max_priority: 7, // Default PLIC has 8 priority levels (0-7)
        }
    }

    /// Read a PLIC register
    #[inline]
    fn read(&self, offset: usize) -> u32 {
        unsafe { (self.base + offset) as *const u32 }.read_volatile()
    }

    /// Write a PLIC register
    #[inline]
    fn write(&self, offset: usize, value: u32) {
        unsafe { (self.base + offset) as *mut u32 }.write_volatile(value) }
    }

    /// Get priority register offset
    fn priority_offset(&self, irq: u32) -> usize {
        assert!(irq > 0 && irq <= self.num_sources);
        plic::PRIORITY_OFFSET + (irq as usize) * 4
    }

    /// Get pending register offset
    fn pending_offset(&self, irq: u32) -> usize {
        assert!(irq > 0 && irq <= self.num_sources);
        plic::PENDING_OFFSET + ((irq as usize) / 32) * 4
    }

    /// Get pending bit position
    fn pending_bit(&self, irq: u32) -> usize {
        (irq as usize) % 32
    }

    /// Get enable register offset
    fn enable_offset(&self, context: usize, irq: u32) -> usize {
        assert!(context < self.num_contexts as usize);
        assert!(irq > 0 && irq <= self.num_sources);
        plic::ENABLE_OFFSET + (context * (self.num_sources as usize / 8)) + ((irq as usize - 1) / 32) * 4
    }

    /// Get enable bit position
    fn enable_bit(&self, irq: u32) -> usize {
        (irq as usize - 1) % 32
    }

    /// Get threshold register offset
    fn threshold_offset(&self, context: usize) -> usize {
        assert!(context < self.num_contexts as usize);
        plic::THRESHOLD_OFFSET + context * 0x1000
    }

    /// Get claim/complete register offset
    fn claim_complete_offset(&self, context: usize) -> usize {
        assert!(context < self.num_contexts as usize);
        plic::CLAIM_COMPLETE_OFFSET + context * 0x1000
    }

    /// Check if an interrupt is pending
    pub fn is_pending(&self, irq: u32) -> bool {
        let offset = self.pending_offset(irq);
        let bit = self.pending_bit(irq);
        let value = self.read(offset);
        (value & (1 << bit)) != 0
    }

    /// Check if an interrupt is enabled for a context
    pub fn is_enabled(&self, context: usize, irq: u32) -> bool {
        let offset = self.enable_offset(context, irq);
        let bit = self.enable_bit(irq);
        let value = self.read(offset);
        (value & (1 << bit)) != 0
    }

    /// Set interrupt threshold for a context
    pub fn set_threshold(&mut self, context: usize, threshold: u32) -> Result<(), &'static str> {
        if threshold > self.max_priority {
            return Err("Threshold too high");
        }

        let offset = self.threshold_offset(context);
        self.write(offset, threshold);
        Ok(())
    }

    /// Get interrupt threshold for a context
    pub fn get_threshold(&self, context: usize) -> u32 {
        let offset = self.threshold_offset(context);
        self.read(offset)
    }

    /// Enable all interrupts for a context
    pub fn enable_all(&mut self, context: usize) {
        for irq in 1..=self.num_sources {
            let _ = self.enable(irq, context);
        }
    }

    /// Disable all interrupts for a context
    pub fn disable_all(&mut self, context: usize) {
        for irq in 1..=self.num_sources {
            let _ = self.disable(irq, context);
        }
    }
}

impl InterruptController for PlicController {
    fn init(&mut self) -> Result<(), &'static str> {
        log::info!("Initializing PLIC controller at {:#x}", self.base);

        // Disable all interrupts for all contexts
        for context in 0..self.num_contexts {
            self.disable_all(context as usize);

            // Set threshold to 0 (allow all interrupts)
            self.set_threshold(context as usize, 0)?;
        }

        // Set all interrupt priorities to 1 (minimum non-zero)
        for irq in 1..=self.num_sources {
            self.set_priority(irq, 1)?;
        }

        log::info!("PLIC controller initialized with {} sources, {} contexts",
                 self.num_sources, self.num_contexts);
        Ok(())
    }

    fn enable(&mut self, irq: u32, cpu: usize) -> Result<(), &'static str> {
        if irq == 0 || irq > self.num_sources {
            return Err("Invalid IRQ number");
        }

        let offset = self.enable_offset(cpu, irq);
        let bit = self.enable_bit(irq);
        let mut value = self.read(offset);
        value |= 1 << bit;
        self.write(offset, value);

        log::debug!("Enabled IRQ {} for CPU {}", irq, cpu);
        Ok(())
    }

    fn disable(&mut self, irq: u32, cpu: usize) -> Result<(), &'static str> {
        if irq == 0 || irq > self.num_sources {
            return Err("Invalid IRQ number");
        }

        let offset = self.enable_offset(cpu, irq);
        let bit = self.enable_bit(irq);
        let mut value = self.read(offset);
        value &= !(1 << bit);
        self.write(offset, value);

        log::debug!("Disabled IRQ {} for CPU {}", irq, cpu);
        Ok(())
    }

    fn set_priority(&mut self, irq: u32, priority: u8) -> Result<(), &'static str> {
        if irq == 0 || irq > self.num_sources {
            return Err("Invalid IRQ number");
        }

        if priority > self.max_priority as u8 {
            return Err("Priority too high");
        }

        let offset = self.priority_offset(irq);
        self.write(offset, priority as u32);

        log::debug!("Set IRQ {} priority to {}", irq, priority);
        Ok(())
    }

    fn get_priority(&self, irq: u32) -> Result<u8, &'static str> {
        if irq == 0 || irq > self.num_sources {
            return Err("Invalid IRQ number");
        }

        let offset = self.priority_offset(irq);
        let value = self.read(offset);
        Ok(value as u8)
    }

    fn claim(&mut self, cpu: usize) -> Result<u32, &'static str> {
        let offset = self.claim_complete_offset(cpu);
        let irq = self.read(offset);

        if irq == 0 {
            Err("No interrupt to claim")
        } else {
            log::debug!("CPU {} claimed IRQ {}", cpu, irq);
            Ok(irq)
        }
    }

    fn complete(&mut self, irq: u32, cpu: usize) -> Result<(), &'static str> {
        if irq == 0 || irq > self.num_sources {
            return Err("Invalid IRQ number");
        }

        let offset = self.claim_complete_offset(cpu);
        self.write(offset, irq);

        log::debug!("CPU {} completed IRQ {}", cpu, irq);
        Ok(())
    }

    fn set_affinity(&mut self, irq: u32, cpu: usize) -> Result<(), &'static str> {
        // For PLIC, affinity is controlled by which contexts have the interrupt enabled
        // Disable for all contexts and enable only for the target CPU
        for context in 0..self.num_contexts {
            if context != cpu as u32 {
                self.disable(irq, context as usize)?;
            }
        }
        self.enable(irq, cpu)?;
        Ok(())
    }

    fn controller_type(&self) -> InterruptControllerType {
        InterruptControllerType::PLIC
    }
}

/// ACLINT registers
pub mod aclint {
    /// SSIP register offset
    pub const SSIP_OFFSET: usize = 0x0000;
    /// STIP register offset
    pub const STIP_OFFSET: usize = 0x0004;
    /// SEIP register offset
    pub const SEIP_OFFSET: usize = 0x0008;
    /// STIMECMP register offset
    pub const STIMECMP_OFFSET: usize = 0x4000;
    /// STIMECMPH register offset (for 64-bit)
    pub const STIMECMPH_OFFSET: usize = 0x4004;
}

/// ACLINT (Advanced Core Local Interruptor)
pub struct AclintController {
    /// Base address
    base: usize,
    /// Number of harts
    num_harts: usize,
}

impl AclintController {
    /// Create a new ACLINT controller
    pub fn new(base: usize, num_harts: usize) -> Self {
        Self { base, num_harts }
    }

    /// Read an ACLINT register
    #[inline]
    fn read(&self, offset: usize, hart: usize) -> u32 {
        let addr = self.base + offset + hart * 0x4000;
        unsafe { (addr as *const u32).read_volatile() }
    }

    /// Write an ACLINT register
    #[inline]
    fn write(&self, offset: usize, hart: usize, value: u32) {
        let addr = self.base + offset + hart * 0x4000;
        unsafe { (addr as *mut u32).write_volatile(value) }
    }

    /// Send IPI to a hart
    pub fn send_ipi(&mut self, hart: usize) -> Result<(), &'static str> {
        if hart >= self.num_harts {
            return Err("Invalid hart ID");
        }

        // Set SSIP bit for the target hart
        let value = self.read(aclint::SSIP_OFFSET, hart);
        self.write(aclint::SSIP_OFFSET, hart, value | 0x1);

        log::debug!("Sent IPI to hart {}", hart);
        Ok(())
    }

    /// Clear IPI for a hart
    pub fn clear_ipi(&mut self, hart: usize) -> Result<(), &'static str> {
        if hart >= self.num_harts {
            return Err("Invalid hart ID");
        }

        // Clear SSIP bit for the target hart
        self.write(aclint::SSIP_OFFSET, hart, 0);

        log::debug!("Cleared IPI for hart {}", hart);
        Ok(())
    }

    /// Set timer compare value for a hart
    pub fn set_timer(&mut self, hart: usize, time: u64) -> Result<(), &'static str> {
        if hart >= self.num_harts {
            return Err("Invalid hart ID");
        }

        // Write low 32 bits
        self.write(aclint::STIMECMP_OFFSET, hart, (time & 0xFFFFFFFF) as u32);

        // Write high 32 bits if supported
        self.write(aclint::STIMECMPH_OFFSET, hart, ((time >> 32) & 0xFFFFFFFF) as u32);

        log::debug!("Set timer for hart {} to {:#x}", hart, time);
        Ok(())
    }

    /// Get timer compare value for a hart
    pub fn get_timer(&self, hart: usize) -> Result<u64, &'static str> {
        if hart >= self.num_harts {
            return Err("Invalid hart ID");
        }

        let low = self.read(aclint::STIMECMP_OFFSET, hart) as u64;
        let high = self.read(aclint::STIMECMPH_OFFSET, hart) as u64;
        Ok((high << 32) | low)
    }
}

impl InterruptController for AclintController {
    fn init(&mut self) -> Result<(), &'static str> {
        log::info!("Initializing ACLINT controller at {:#x}", self.base);

        // Clear all IPIs
        for hart in 0..self.num_harts {
            self.clear_ipi(hart)?;
            // Set timer to a large value to prevent immediate interrupt
            self.set_timer(hart, u64::MAX)?;
        }

        log::info!("ACLINT controller initialized for {} harts", self.num_harts);
        Ok(())
    }

    fn enable(&mut self, irq: u32, cpu: usize) -> Result<(), &'static str> {
        // ACLINT doesn't have explicit enable/disable for local interrupts
        // They are controlled through the mie CSR
        log::warn!("ACLINT enable not implemented for IRQ {} on CPU {}", irq, cpu);
        Ok(())
    }

    fn disable(&mut self, irq: u32, cpu: usize) -> Result<(), &'static str> {
        // ACLINT doesn't have explicit enable/disable for local interrupts
        log::warn!("ACLINT disable not implemented for IRQ {} on CPU {}", irq, cpu);
        Ok(())
    }

    fn set_priority(&mut self, _irq: u32, _priority: u8) -> Result<(), &'static str> {
        // ACLINT doesn't support priority
        Err("ACLINT doesn't support priority")
    }

    fn get_priority(&self, _irq: u32) -> Result<u8, &'static str> {
        // ACLINT doesn't support priority
        Err("ACLINT doesn't support priority")
    }

    fn claim(&mut self, _cpu: usize) -> Result<u32, &'static str> {
        // ACLINT local interrupts are claimed via the mcause CSR
        Err("ACLINT claim not implemented")
    }

    fn complete(&mut self, _irq: u32, _cpu: usize) -> Result<(), &'static str> {
        // ACLINT local interrupts are completed by clearing the source
        // (e.g., clearing SSIP for IPI)
        Ok(())
    }

    fn set_affinity(&mut self, _irq: u32, _cpu: usize) -> Result<(), &'static str> {
        // ACLINT interrupts are always local to a hart
        Err("ACLINT doesn't support affinity")
    }

    fn controller_type(&self) -> InterruptControllerType {
        InterruptControllerType::ACLINT
    }
}

/// Interrupt manager
pub struct InterruptManager {
    /// Primary interrupt controller
    primary: Box<dyn InterruptController>,
    /// Secondary interrupt controllers
    secondary: Vec<Box<dyn InterruptController>>,
}

impl InterruptManager {
    /// Create a new interrupt manager
    pub fn new(primary: Box<dyn InterruptController>) -> Self {
        Self {
            primary,
            secondary: Vec::new(),
        }
    }

    /// Add a secondary interrupt controller
    pub fn add_secondary(&mut self, controller: Box<dyn InterruptController>) {
        self.secondary.push(controller);
    }

    /// Initialize all interrupt controllers
    pub fn init(&mut self) -> Result<(), &'static str> {
        log::info!("Initializing interrupt manager");

        // Initialize primary controller
        self.primary.init()?;

        // Initialize secondary controllers
        for controller in &mut self.secondary {
            controller.init()?;
        }

        log::info!("Interrupt manager initialized");
        Ok(())
    }

    /// Enable an interrupt
    pub fn enable(&mut self, irq: u32, cpu: usize) -> Result<(), &'static str> {
        self.primary.enable(irq, cpu)
    }

    /// Disable an interrupt
    pub fn disable(&mut self, irq: u32, cpu: usize) -> Result<(), &'static str> {
        self.primary.disable(irq, cpu)
    }

    /// Set interrupt priority
    pub fn set_priority(&mut self, irq: u32, priority: u8) -> Result<(), &'static str> {
        self.primary.set_priority(irq, priority)
    }

    /// Claim an interrupt
    pub fn claim(&mut self, cpu: usize) -> Result<u32, &'static str> {
        self.primary.claim(cpu)
    }

    /// Complete an interrupt
    pub fn complete(&mut self, irq: u32, cpu: usize) -> Result<(), &'static str> {
        self.primary.complete(irq, cpu)?;

        // Also try to complete with secondary controllers
        for controller in &mut self.secondary {
            let _ = controller.complete(irq, cpu);
        }

        Ok(())
    }

    /// Set interrupt affinity
    pub fn set_affinity(&mut self, irq: u32, cpu: usize) -> Result<(), &'static str> {
        self.primary.set_affinity(irq, cpu)
    }

    /// Get the primary controller type
    pub fn primary_type(&self) -> InterruptControllerType {
        self.primary.controller_type()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plic_controller() {
        let mut plic = PlicController::new(0x0C000000, 32, 4);

        assert_eq!(plic.controller_type(), InterruptControllerType::PLIC);
        assert_eq!(plic.num_sources, 32);
        assert_eq!(plic.num_contexts, 4);

        // Test priority operations
        plic.set_priority(1, 5).unwrap();
        assert_eq!(plic.get_priority(1).unwrap(), 5);

        // Test threshold operations
        plic.set_threshold(0, 0).unwrap();
        assert_eq!(plic.get_threshold(0), 0);

        // Test enable/disable
        plic.enable(1, 0).unwrap();
        assert!(plic.is_enabled(0, 1));
        plic.disable(1, 0).unwrap();
        assert!(!plic.is_enabled(0, 1));
    }

    #[test]
    fn test_aclint_controller() {
        let mut aclint = AclintController::new(0x02000000, 4);

        assert_eq!(aclint.controller_type(), InterruptControllerType::ACLINT);
        assert_eq!(aclint.num_harts, 4);

        // Test IPI operations
        aclint.send_ipi(0).unwrap();
        aclint.clear_ipi(0).unwrap();

        // Test timer operations
        aclint.set_timer(0, 0x123456789ABCDEF0).unwrap();
        let timer = aclint.get_timer(0).unwrap();
        assert!(timer >= 0x123456789ABCDEF0);
    }

    #[test]
    fn test_interrupt_manager() {
        let plic = Box::new(PlicController::new(0x0C000000, 32, 4));
        let mut manager = InterruptManager::new(plic);

        assert_eq!(manager.primary_type(), InterruptControllerType::PLIC);

        // Test adding secondary controller
        let aclint = Box::new(AclintController::new(0x02000000, 4));
        manager.add_secondary(aclint);
        assert_eq!(manager.secondary.len(), 1);
    }
}