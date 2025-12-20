//! RISC-V CLINT (Core Local Interruptor) Support
//!
//! This module provides CLINT support including:
//! - CLINT initialization and configuration
//! - Timer interrupt management
//! - Software interrupt management
//! - Per-hart interrupt handling

use crate::arch::riscv64::*;

/// CLINT configuration
#[derive(Debug, Clone)]
pub struct ClintConfig {
    /// CLINT base address
    pub base_address: u64,
    /// Number of harts supported
    pub num_harts: u32,
    /// Timer frequency in Hz
    pub timer_frequency: u64,
    /// Enable timer interrupts
    pub enable_timer_interrupts: bool,
    /// Enable software interrupts
    pub enable_software_interrupts,
}

impl Default for ClintConfig {
    fn default() -> Self {
        Self {
            base_address: 0x02000000,
            num_harts: 8,
            timer_frequency: 10000000, // 10MHz
            enable_timer_interrupts: true,
            enable_software_interrupts: true,
        }
    }
}

/// CLINT register offsets
pub mod clint_regs {
    pub const MSIP0: usize = 0x0000; // Hart 0 software interrupt pending
    pub const MTIMECMP0: usize = 0x4000; // Hart 0 timer comparator
    pub const MTIME: usize = 0xBFF8; // Timer value
}

/// CLINT driver
pub struct Clint {
    /// Base address
    base: u64,
    /// Configuration
    config: ClintConfig,
}

impl Clint {
    /// Create new CLINT driver
    pub fn new(base: u64, config: ClintConfig) -> Self {
        Self { base, config }
    }

    /// Initialize CLINT
    pub fn init(&mut self) -> Result<(), &'static str> {
        log::info!("Initializing CLINT at {:#x}", self.base);

        // Clear all software interrupt pending bits
        for hart_id in 0..self.config.num_harts {
            self.clear_software_interrupt(hart_id);
        }

        // Set timer comparators to maximum value (disable timer interrupts initially)
        for hart_id in 0..self.config.num_harts {
            self.set_timer_comparator(hart_id, u64::MAX);
        }

        log::info!("CLINT initialized for {} harts", self.config.num_harts);
        Ok(())
    }

    /// Read mtime register (64-bit)
    pub fn read_mtime(&self) -> u64 {
        unsafe {
            // Read high word first
            let high = core::ptr::read_volatile((self.base + clint_regs::MTIME + 4) as *const u32) as u64;
            // Read low word
            let low = core::ptr::read_volatile((self.base + clint_regs::MTIME) as *const u32) as u64;
            // Read high word again to detect rollover
            let high2 = core::ptr::read_volatile((self.base + clint_regs::MTIME + 4) as *const u32) as u64;

            if high != high2 {
                // Timer rolled over between reads, read again
                let low = core::ptr::read_volatile((self.base + clint_regs::MTIME) as *const u32) as u64;
                (high2 << 32) | low
            } else {
                (high << 32) | low
            }
        }
    }

    /// Write mtimecmp register for specific hart (64-bit)
    pub fn write_mtimecmp(&self, hart_id: u32, value: u64) -> Result<(), &'static str> {
        if hart_id >= self.config.num_harts {
            return Err("Invalid hart ID");
        }

        let mtimecmp_base = self.base + (clint_regs::MTIMECMP0 as u64) + (hart_id as u64 * 8);

        unsafe {
            // Write high word first, then low word
            core::ptr::write_volatile((mtimecmp_base + 4) as *mut u32, (value >> 32) as u32);
            core::ptr::write_volatile(mtimecmp_base as *mut u32, (value & 0xFFFFFFFF) as u32);
        }

        Ok(())
    }

    /// Set timer comparator for specific hart
    pub fn set_timer_comparator(&self, hart_id: u32, value: u64) -> Result<(), &'static str> {
        self.write_mtimecmp(hart_id, value)
    }

    /// Get timer comparator for specific hart
    pub fn get_timer_comparator(&self, hart_id: u32) -> Result<u64, &'static str> {
        if hart_id >= self.config.num_harts {
            return Err("Invalid hart ID");
        }

        let mtimecmp_base = self.base + (clint_regs::MTIMECMP0 as u64) + (hart_id as u64 * 8);

        unsafe {
            let high = core::ptr::read_volatile((mtimecmp_base + 4) as *const u32) as u64;
            let low = core::ptr::read_volatile(mtimecmp_base as *const u32) as u64;
            Ok((high << 32) | low)
        }
    }

    /// Set software interrupt for specific hart
    pub fn set_software_interrupt(&self, hart_id: u32) -> Result<(), &'static str> {
        if hart_id >= self.config.num_harts {
            return Err("Invalid hart ID");
        }

        if !self.config.enable_software_interrupts {
            return Err("Software interrupts disabled");
        }

        let msip_base = self.base + (clint_regs::MSIP0 as u64) + (hart_id as u64 * 4);

        unsafe {
            core::ptr::write_volatile(msip_base as *mut u32, 1);
        }

        Ok(())
    }

    /// Clear software interrupt for specific hart
    pub fn clear_software_interrupt(&self, hart_id: u32) {
        if hart_id >= self.config.num_harts {
            return;
        }

        let msip_base = self.base + (clint_regs::MSIP0 as u64) + (hart_id as u64 * 4);

        unsafe {
            core::ptr::write_volatile(msip_base as *mut u32, 0);
        }
    }

    /// Check if software interrupt is pending for specific hart
    pub fn is_software_interrupt_pending(&self, hart_id: u32) -> Result<bool, &'static str> {
        if hart_id >= self.config.num_harts {
            return Err("Invalid hart ID");
        }

        let msip_base = self.base + (clint_regs::MSIP0 as u64) + (hart_id as u64 * 4);

        unsafe {
            let value = core::ptr::read_volatile(msip_base as *const u32);
            Ok(value != 0)
        }
    }

    /// Calculate timer period in timer ticks
    pub fn calculate_period_ticks(&self, period_us: u64) -> u64 {
        (period_us * self.config.timer_frequency) / 1_000_000
    }

    /// Calculate next timer interrupt time
    pub fn calculate_next_timer(&self, period_us: u64) -> u64 {
        let current = self.read_mtime();
        let period_ticks = self.calculate_period_ticks(period_us);
        current + period_ticks
    }

    /// Enable timer interrupts for specific hart
    pub fn enable_timer_interrupts(&self, hart_id: u32, enable: bool) -> Result<(), &'static str> {
        if !self.config.enable_timer_interrupts {
            return Err("Timer interrupts disabled");
        }

        if enable {
            // Set comparator to next time
            let next = self.calculate_next_timer(1000); // 1ms default
            self.set_timer_comparator(hart_id, next)?;
        } else {
            // Disable by setting to maximum
            self.set_timer_comparator(hart_id, u64::MAX)?;
        }

        Ok(())
    }

    /// Send IPI (Inter-Processor Interrupt) to target hart
    pub fn send_ipi(&self, target_hart: u32) -> Result<(), &'static str> {
        self.set_software_interrupt(target_hart)
    }

    /// Broadcast IPI to all other harts
    pub fn broadcast_ipi(&self, source_hart: u32) -> Result<(), &'static str> {
        for hart_id in 0..self.config.num_harts {
            if hart_id != source_hart {
                self.set_software_interrupt(hart_id)?;
            }
        }
        Ok(())
    }

    /// Handle timer interrupt for specific hart
    pub fn handle_timer_interrupt(&self, hart_id: u32, period_us: u64) -> Result<(), &'static str> {
        // Calculate next timer time
        let next_time = self.calculate_next_timer(period_us);

        // Set new comparator
        self.set_timer_comparator(hart_id, next_time)?;

        Ok(())
    }

    /// Get time until next timer interrupt
    pub fn get_time_until_next_timer(&self, hart_id: u32) -> Result<u64, &'static str> {
        let current = self.read_mtime();
        let comparator = self.get_timer_comparator(hart_id)?;

        if comparator > current {
            Ok(comparator - current)
        } else {
            Ok(0) // Timer should have fired already
        }
    }

    /// Get configuration
    pub fn get_config(&self) -> &ClintConfig {
        &self.config
    }
}

/// Global CLINT instance
static mut CLINT: Option<Clint> = None;
static CLINT_INIT: spin::Once<()> = spin::Once::new();

/// Initialize CLINT subsystem
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing CLINT subsystem");

    CLINT_INIT.call_once(|| {
        let base = super::get_clint_base();
        let config = if let Some(platform_config) = super::get_platform_configurations() {
            platform_config.clint.clone()
        } else {
            ClintConfig::default()
        };

        let mut clint = Clint::new(base, config);
        if let Ok(()) = clint.init() {
            unsafe {
                CLINT = Some(clint);
            }
        }
    });

    log::info!("CLINT subsystem initialized");
    Ok(())
}

/// Get CLINT instance
pub fn get_clint() -> Option<&'static Clint> {
    unsafe { CLINT.as_ref() }
}

/// Get mutable CLINT instance
pub fn get_clint_mut() -> Option<&'static mut Clint> {
    unsafe { CLINT.as_mut() }
}

/// Send IPI to specific hart
pub fn send_ipi(hart_id: u32) -> Result<(), &'static str> {
    if let Some(clint) = get_clint() {
        clint.send_ipi(hart_id)
    } else {
        Err("CLINT not initialized")
    }
}

/// Broadcast IPI to all harts
pub fn broadcast_ipi(source_hart: u32) -> Result<(), &'static str> {
    if let Some(clint) = get_clint() {
        clint.broadcast_ipi(source_hart)
    } else {
        Err("CLINT not initialized")
    }
}

/// Get current time
pub fn get_time() -> u64 {
    if let Some(clint) = get_clint() {
        clint.read_mtime()
    } else {
        0
    }
}

/// Set timer for specific hart
pub fn set_timer(hart_id: u32, period_us: u64) -> Result<(), &'static str> {
    if let Some(clint) = get_clint() {
        let next_time = clint.calculate_next_timer(period_us);
        clint.set_timer_comparator(hart_id, next_time)
    } else {
        Err("CLINT not initialized")
    }
}

/// Handle timer interrupt
pub fn handle_timer_interrupt(hart_id: u32, period_us: u64) -> Result<(), &'static str> {
    if let Some(clint) = get_clint() {
        clint.handle_timer_interrupt(hart_id, period_us)
    } else {
        Err("CLINT not initialized")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clint_config() {
        let config = ClintConfig::default();
        assert_eq!(config.base_address, 0x02000000);
        assert_eq!(config.num_harts, 8);
        assert_eq!(config.timer_frequency, 10000000);
        assert!(config.enable_timer_interrupts);
        assert!(config.enable_software_interrupts);
    }

    #[test]
    fn test_clint() {
        let config = ClintConfig::default();
        let clint = Clint::new(0x02000000, config);
        assert_eq!(clint.base, 0x02000000);
        assert_eq!(clint.config.num_harts, 8);
    }

    #[test]
    fn test_period_calculation() {
        let config = ClintConfig::default();
        let clint = Clint::new(0x02000000, config);

        // Test 1ms period at 10MHz
        let period_ticks = clint.calculate_period_ticks(1000);
        assert_eq!(period_ticks, 10000);

        // Test 1 second period at 10MHz
        let period_ticks = clint.calculate_period_ticks(1000000);
        assert_eq!(period_ticks, 10000000);
    }

    #[test]
    fn test_next_timer_calculation() {
        let config = ClintConfig::default();
        let clint = Clint::new(0x02000000, config);

        // Mock current time
        let current_time = 1000000;
        let period_us = 1000; // 1ms

        let next = clint.calculate_next_timer(period_us);
        // Should be current_time + 10000 ticks (1ms at 10MHz)
        assert!(next > current_time);
    }
}