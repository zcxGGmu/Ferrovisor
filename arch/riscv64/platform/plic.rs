//! RISC-V PLIC (Platform-Level Interrupt Controller) Support
//!
//! This module provides PLIC support including:
//! - PLIC initialization and configuration
//! - Interrupt priority management
//! - Interrupt enable/disable
//! - Interrupt claiming and completion
//! - Context-specific interrupt handling

use crate::arch::riscv64::*;

/// PLIC configuration
#[derive(Debug, Clone)]
pub struct PlicConfig {
    /// PLIC base address
    pub base_address: u64,
    /// Number of interrupt sources
    pub num_sources: u32,
    /// Number of contexts
    pub num_contexts: u32,
    /// Maximum priority
    pub max_priority: u32,
    /// Enable external interrupts
    pub enable_external: bool,
}

impl Default for PlicConfig {
    fn default() -> Self {
        Self {
            base_address: 0x0c000000,
            num_sources: 32,
            num_contexts: 4,
            max_priority: 7,
            enable_external: true,
        }
    }
}

/// PLIC register offsets
pub mod plic_regs {
    // Priority registers (4 bytes each)
    pub const PRIORITY_BASE: usize = 0x000000;
    // Pending registers (1 byte each, 4 per word)
    pub const PENDING_BASE: usize = 0x001000;
    // Enable registers (4 bytes each per context)
    pub const ENABLE_BASE: usize = 0x002000;
    // Claim/complete registers (4 bytes each per context)
    pub const CLAIM_COMPLETE_BASE: usize = 0x200000;
    // Threshold register (4 bytes each per context)
    pub const THRESHOLD_BASE: usize = 0x200000;
}

/// PLIC driver
pub struct Plic {
    /// Base address
    base: u64,
    /// Configuration
    config: PlicConfig,
}

impl Plic {
    /// Create new PLIC driver
    pub fn new(base: u64, config: PlicConfig) -> Self {
        Self { base, config }
    }

    /// Initialize PLIC
    pub fn init(&mut self) -> Result<(), &'static str> {
        log::info!("Initializing PLIC at {:#x}", self.base);

        // Disable all interrupts for all contexts
        for context in 0..self.config.num_contexts {
            self.set_threshold(context, 0);
            self.disable_all_interrupts(context);
        }

        // Set all interrupt priorities to 0 (disabled)
        for source in 1..self.config.num_sources {
            self.set_priority(source, 0);
        }

        log::info!("PLIC initialized with {} sources, {} contexts",
                  self.config.num_sources, self.config.num_contexts);
        Ok(())
    }

    /// Set interrupt priority
    pub fn set_priority(&self, interrupt_id: u32, priority: u32) -> Result<(), &'static str> {
        if interrupt_id == 0 || interrupt_id >= self.config.num_sources {
            return Err("Invalid interrupt ID");
        }

        if priority > self.config.max_priority {
            return Err("Priority too high");
        }

        let addr = self.base + (plic_regs::PRIORITY_BASE as u64) + (interrupt_id as u64 * 4);

        unsafe {
            core::ptr::write_volatile(addr as *mut u32, priority);
        }

        Ok(())
    }

    /// Get interrupt priority
    pub fn get_priority(&self, interrupt_id: u32) -> Result<u32, &'static str> {
        if interrupt_id == 0 || interrupt_id >= self.config.num_sources {
            return Err("Invalid interrupt ID");
        }

        let addr = self.base + (plic_regs::PRIORITY_BASE as u64) + (interrupt_id as u64 * 4);

        unsafe {
            Ok(core::ptr::read_volatile(addr as *const u32))
        }
    }

    /// Enable interrupt for specific context
    pub fn enable_interrupt(&self, context: u32, interrupt_id: u32) -> Result<(), &'static str> {
        if context >= self.config.num_contexts {
            return Err("Invalid context");
        }

        if interrupt_id == 0 || interrupt_id >= self.config.num_sources {
            return Err("Invalid interrupt ID");
        }

        let word_offset = (interrupt_id / 32) * 4;
        let bit_offset = interrupt_id % 32;

        let addr = self.base + (plic_regs::ENABLE_BASE as u64) +
                  (context as u64 * 0x80) + (word_offset as u64);

        unsafe {
            let mut value = core::ptr::read_volatile(addr as *const u32);
            value |= 1 << bit_offset;
            core::ptr::write_volatile(addr as *mut u32, value);
        }

        Ok(())
    }

    /// Disable interrupt for specific context
    pub fn disable_interrupt(&self, context: u32, interrupt_id: u32) -> Result<(), &'static str> {
        if context >= self.config.num_contexts {
            return Err("Invalid context");
        }

        if interrupt_id == 0 || interrupt_id >= self.config.num_sources {
            return Err("Invalid interrupt ID");
        }

        let word_offset = (interrupt_id / 32) * 4;
        let bit_offset = interrupt_id % 32;

        let addr = self.base + (plic_regs::ENABLE_BASE as u64) +
                  (context as u64 * 0x80) + (word_offset as u64);

        unsafe {
            let mut value = core::ptr::read_volatile(addr as *const u32);
            value &= !(1 << bit_offset);
            core::ptr::write_volatile(addr as *mut u32, value);
        }

        Ok(())
    }

    /// Enable all interrupts for specific context
    pub fn enable_all_interrupts(&self, context: u32) -> Result<(), &'static str> {
        if context >= self.config.num_contexts {
            return Err("Invalid context");
        }

        let num_words = (self.config.num_sources + 31) / 32;

        for word in 0..num_words {
            let addr = self.base + (plic_regs::ENABLE_BASE as u64) +
                      (context as u64 * 0x80) + (word as u64 * 4);

            // Set all bits except bit 0 (which is reserved)
            let value = if word == 0 { 0xFFFFFFFE } else { 0xFFFFFFFF };

            unsafe {
                core::ptr::write_volatile(addr as *mut u32, value);
            }
        }

        Ok(())
    }

    /// Disable all interrupts for specific context
    pub fn disable_all_interrupts(&self, context: u32) -> Result<(), &'static str> {
        if context >= self.config.num_contexts {
            return Err("Invalid context");
        }

        let num_words = (self.config.num_sources + 31) / 32;

        for word in 0..num_words {
            let addr = self.base + (plic_regs::ENABLE_BASE as u64) +
                      (context as u64 * 0x80) + (word as u64 * 4);

            unsafe {
                core::ptr::write_volatile(addr as *mut u32, 0);
            }
        }

        Ok(())
    }

    /// Set priority threshold for specific context
    pub fn set_threshold(&self, context: u32, threshold: u32) -> Result<(), &'static str> {
        if context >= self.config.num_contexts {
            return Err("Invalid context");
        }

        if threshold > self.config.max_priority {
            return Err("Threshold too high");
        }

        let addr = self.base + (plic_regs::THRESHOLD_BASE as u64) +
                  (context as u64 * 0x1000);

        unsafe {
            core::ptr::write_volatile(addr as *mut u32, threshold);
        }

        Ok(())
    }

    /// Get priority threshold for specific context
    pub fn get_threshold(&self, context: u32) -> Result<u32, &'static str> {
        if context >= self.config.num_contexts {
            return Err("Invalid context");
        }

        let addr = self.base + (plic_regs::THRESHOLD_BASE as u64) +
                  (context as u64 * 0x1000);

        unsafe {
            Ok(core::ptr::read_volatile(addr as *const u32))
        }
    }

    /// Check if interrupt is pending
    pub fn is_pending(&self, interrupt_id: u32) -> Result<bool, &'static str> {
        if interrupt_id == 0 || interrupt_id >= self.config.num_sources {
            return Err("Invalid interrupt ID");
        }

        let word_offset = interrupt_id / 32;
        let bit_offset = interrupt_id % 32;

        let addr = self.base + (plic_regs::PENDING_BASE as u64) + (word_offset as u64 * 4);

        unsafe {
            let value = core::ptr::read_volatile(addr as *const u32);
            Ok((value & (1 << bit_offset)) != 0)
        }
    }

    /// Claim interrupt for specific context
    pub fn claim_interrupt(&self, context: u32) -> Result<u32, &'static str> {
        if context >= self.config.num_contexts {
            return Err("Invalid context");
        }

        let addr = self.base + (plic_regs::CLAIM_COMPLETE_BASE as u64) +
                  (context as u64 * 0x1000);

        unsafe {
            let interrupt_id = core::ptr::read_volatile(addr as *const u32);
            Ok(interrupt_id)
        }
    }

    /// Complete interrupt for specific context
    pub fn complete_interrupt(&self, context: u32, interrupt_id: u32) -> Result<(), &'static str> {
        if context >= self.config.num_contexts {
            return Err("Invalid context");
        }

        if interrupt_id == 0 || interrupt_id >= self.config.num_sources {
            return Err("Invalid interrupt ID");
        }

        let addr = self.base + (plic_regs::CLAIM_COMPLETE_BASE as u64) +
                  (context as u64 * 0x1000);

        unsafe {
            core::ptr::write_volatile(addr as *mut u32, interrupt_id);
        }

        Ok(())
    }

    /// Get the highest priority pending interrupt for context
    pub fn get_highest_pending(&self, context: u32) -> Result<Option<u32>, &'static str> {
        if context >= self.config.num_contexts {
            return Err("Invalid context");
        }

        let threshold = self.get_threshold(context)?;

        // Check all pending interrupts
        for source in 1..self.config.num_sources {
            if self.is_pending(source)? {
                let priority = self.get_priority(source)?;
                if priority > threshold {
                    return Ok(Some(source));
                }
            }
        }

        Ok(None)
    }

    /// Configure interrupt for context
    pub fn configure_interrupt(&self, context: u32, interrupt_id: u32,
                               priority: u32, enable: bool) -> Result<(), &'static str> {
        // Set priority
        self.set_priority(interrupt_id, priority)?;

        // Enable or disable
        if enable {
            self.enable_interrupt(context, interrupt_id)?;
        } else {
            self.disable_interrupt(context, interrupt_id)?;
        }

        Ok(())
    }

    /// Get configuration
    pub fn get_config(&self) -> &PlicConfig {
        &self.config
    }
}

/// Global PLIC instance
static mut PLIC: Option<Plic> = None;
static PLIC_INIT: spin::Once<()> = spin::Once::new();

/// Initialize PLIC subsystem
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing PLIC subsystem");

    PLIC_INIT.call_once(|| {
        let base = super::get_plic_base();
        let config = if let Some(platform_config) = super::get_platform_configurations() {
            platform_config.plic.clone()
        } else {
            PlicConfig::default()
        };

        let mut plic = Plic::new(base, config);
        if let Ok(()) = plic.init() {
            unsafe {
                PLIC = Some(plic);
            }
        }
    });

    log::info!("PLIC subsystem initialized");
    Ok(())
}

/// Get PLIC instance
pub fn get_plic() -> Option<&'static Plic> {
    unsafe { PLIC.as_ref() }
}

/// Get mutable PLIC instance
pub fn get_plic_mut() -> Option<&'static mut Plic> {
    unsafe { PLIC.as_mut() }
}

/// Set interrupt priority
pub fn set_priority(interrupt_id: u32, priority: u32) -> Result<(), &'static str> {
    if let Some(plic) = get_plic() {
        plic.set_priority(interrupt_id, priority)
    } else {
        Err("PLIC not initialized")
    }
}

/// Enable interrupt for context
pub fn enable_interrupt(context: u32, interrupt_id: u32) -> Result<(), &'static str> {
    if let Some(plic) = get_plic() {
        plic.enable_interrupt(context, interrupt_id)
    } else {
        Err("PLIC not initialized")
    }
}

/// Disable interrupt for context
pub fn disable_interrupt(context: u32, interrupt_id: u32) -> Result<(), &'static str> {
    if let Some(plic) = get_plic() {
        plic.disable_interrupt(context, interrupt_id)
    } else {
        Err("PLIC not initialized")
    }
}

/// Claim interrupt for context
pub fn claim_interrupt(context: u32) -> Result<u32, &'static str> {
    if let Some(plic) = get_plic() {
        plic.claim_interrupt(context)
    } else {
        Err("PLIC not initialized")
    }
}

/// Complete interrupt for context
pub fn complete_interrupt(context: u32, interrupt_id: u32) -> Result<(), &'static str> {
    if let Some(plic) = get_plic() {
        plic.complete_interrupt(context, interrupt_id)
    } else {
        Err("PLIC not initialized")
    }
}

/// Configure interrupt
pub fn configure_interrupt(context: u32, interrupt_id: u32,
                           priority: u32, enable: bool) -> Result<(), &'static str> {
    if let Some(plic) = get_plic() {
        plic.configure_interrupt(context, interrupt_id, priority, enable)
    } else {
        Err("PLIC not initialized")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plic_config() {
        let config = PlicConfig::default();
        assert_eq!(config.base_address, 0x0c000000);
        assert_eq!(config.num_sources, 32);
        assert_eq!(config.num_contexts, 4);
        assert_eq!(config.max_priority, 7);
        assert!(config.enable_external);
    }

    #[test]
    fn test_plic() {
        let config = PlicConfig::default();
        let plic = Plic::new(0x0c000000, config);
        assert_eq!(plic.base, 0x0c000000);
        assert_eq!(plic.config.num_sources, 32);
    }

    #[test]
    fn test_priority_validation() {
        let config = PlicConfig::default();
        let plic = Plic::new(0x0c000000, config);

        // Valid priority
        assert!(plic.set_priority(1, 5).is_ok());

        // Invalid interrupt ID
        assert!(plic.set_priority(0, 5).is_err());
        assert!(plic.set_priority(32, 5).is_err());

        // Priority too high
        assert!(plic.set_priority(1, 8).is_err());
    }
}