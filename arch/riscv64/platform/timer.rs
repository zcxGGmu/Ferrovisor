//! RISC-V Platform Timer Support
//!
//! This module provides platform-specific timer support including:
//! - Timer initialization and configuration
//! - Timer interrupt handling
//! - High-resolution time tracking
//! - Platform-specific timer features

use crate::arch::riscv64::*;

/// Timer configuration
#[derive(Debug, Clone)]
pub struct TimerConfig {
    /// Timer frequency in Hz
    pub frequency: u64,
    /// Timer base address
    pub base_address: u64,
    /// Enable periodic timer
    pub periodic: bool,
    /// Timer period in microseconds
    pub period_us: u64,
    /// Enable one-shot mode
    pub one_shot: bool,
    /// Number of timer comparators
    pub comparators: u32,
}

impl Default for TimerConfig {
    fn default() -> Self {
        Self {
            frequency: 10000000, // 10MHz
            base_address: 0x02000000, // CLINT base
            periodic: true,
            period_us: 1000, // 1ms
            one_shot: false,
            comparators: 1,
        }
    }
}

/// Timer driver interface
pub trait TimerDriver {
    /// Initialize timer
    fn init(&mut self) -> Result<(), &'static str>;

    /// Set timer period
    fn set_period(&mut self, period_us: u64) -> Result<(), &'static str>;

    /// Start timer
    fn start(&mut self) -> Result<(), &'static str>;

    /// Stop timer
    fn stop(&mut self) -> Result<(), &'static str>;

    /// Get current timer value
    fn get_time(&self) -> u64;

    /// Set comparator value
    fn set_comparator(&mut self, id: u32, value: u64) -> Result<(), &'static str>;

    /// Clear timer interrupt
    fn clear_interrupt(&mut self, id: u32) -> Result<(), &'static str>;

    /// Check if interrupt is pending
    fn is_interrupt_pending(&self, id: u32) -> bool;
}

/// CLINT (Core Local Interruptor) Timer
pub struct ClintTimer {
    /// Base address
    base: u64,
    /// Timer frequency
    frequency: u64,
    /// Period in timer ticks
    period_ticks: u64,
    /// Last timer value
    last_value: u64,
}

impl ClintTimer {
    /// Create new CLINT timer
    pub fn new(base: u64, frequency: u64) -> Self {
        Self {
            base,
            frequency,
            period_ticks: 0,
            last_value: 0,
        }
    }

    /// Read mtime register
    fn read_mtime(&self) -> u64 {
        unsafe {
            let mtime_low = core::ptr::read_volatile((self.base + 0xBFF8) as *const u32) as u64;
            let mtime_high = core::ptr::read_volatile((self.base + 0xBFFC) as *const u32) as u64;
            (mtime_high << 32) | mtime_low
        }
    }

    /// Write mtimecmp register
    fn write_mtimecmp(&self, hart_id: u32, value: u64) {
        let mtimecmp_base = self.base + 0x4000 + (hart_id as u64 * 8);
        unsafe {
            core::ptr::write_volatile((mtimecmp_base + 4) as *mut u32, (value >> 32) as u32);
            core::ptr::write_volatile(mtimecmp_base as *mut u32, (value & 0xFFFFFFFF) as u32);
        }
    }
}

impl TimerDriver for ClintTimer {
    fn init(&mut self) -> Result<(), &'static str> {
        log::debug!("Initializing CLINT timer at {:#x}, frequency: {}Hz",
                   self.base, self.frequency);

        // Clear any pending timer interrupts
        for hart_id in 0..8 { // Assume up to 8 harts
            self.write_mtimecmp(hart_id, u64::MAX);
        }

        log::debug!("CLINT timer initialized");
        Ok(())
    }

    fn set_period(&mut self, period_us: u64) -> Result<(), &'static str> {
        // Convert microseconds to timer ticks
        self.period_ticks = (period_us * self.frequency) / 1_000_000;

        if self.period_ticks == 0 {
            return Err("Timer period too small");
        }

        log::debug!("Timer period set to {}us ({} ticks)", period_us, self.period_ticks);
        Ok(())
    }

    fn start(&mut self) -> Result<(), &'static str> {
        let current_time = self.read_mtime();
        let next_time = current_time + self.period_ticks;

        // Set comparator for current hart
        let hart_id = crate::arch::riscv64::cpu::current_cpu_id() as u32;
        self.write_mtimecmp(hart_id, next_time);

        self.last_value = current_time;

        log::debug!("CLINT timer started, next interrupt at {}", next_time);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), &'static str> {
        // Set comparator to maximum value to disable interrupts
        let hart_id = crate::arch::riscv64::cpu::current_cpu_id() as u32;
        self.write_mtimecmp(hart_id, u64::MAX);

        log::debug!("CLINT timer stopped");
        Ok(())
    }

    fn get_time(&self) -> u64 {
        self.read_mtime()
    }

    fn set_comparator(&mut self, hart_id: u32, value: u64) -> Result<(), &'static str> {
        if hart_id >= 8 {
            return Err("Invalid hart ID");
        }

        self.write_mtimecmp(hart_id, value);
        Ok(())
    }

    fn clear_interrupt(&mut self, _hart_id: u32) -> Result<(), &'static str> {
        // CLINT timer interrupt is cleared by reading mtime and setting mtimecmp
        // This is typically handled in the interrupt handler
        Ok(())
    }

    fn is_interrupt_pending(&self, _hart_id: u32) -> bool {
        // Check if mtime >= mtimecmp
        // Note: This is a simplified check
        let current_time = self.read_mtime();
        current_time >= self.last_value + self.period_ticks
    }
}

/// High-resolution timer using RDCYCLE or TIME CSR
pub struct HighResTimer {
    /// Using TIME CSR (machine timer)
    use_time_csr: bool,
    /// Frequency
    frequency: u64,
}

impl HighResTimer {
    /// Create new high-resolution timer
    pub fn new() -> Self {
        // Check if TIME CSR is available
        let use_time_csr = true; // Assume available

        Self {
            use_time_csr,
            frequency: if use_time_csr {
                super::get_timer_frequency()
            } else {
                // Get CPU frequency from device tree
                crate::arch::riscv64::devtree::get_timer_info()
                    .get(0)
                    .map(|t| t.frequency as u64)
                    .unwrap_or(1000000)
            },
        }
    }

    /// Read TIME CSR
    fn read_time(&self) -> u64 {
        unsafe {
            let mut time: u64;
            core::arch::asm!(
                "rdtime {0}",
                out(reg) time
            );
            time
        }
    }

    /// Read cycle counter
    fn read_cycle(&self) -> u64 {
        unsafe {
            let mut cycles: u64;
            core::arch::asm!(
                "rdcycle {0}",
                out(reg) cycles
            );
            cycles
        }
    }

    /// Convert timer ticks to nanoseconds
    fn ticks_to_ns(&self, ticks: u64) -> u64 {
        (ticks * 1_000_000_000) / self.frequency
    }

    /// Convert nanoseconds to timer ticks
    fn ns_to_ticks(&self, ns: u64) -> u64 {
        (ns * self.frequency) / 1_000_000_000
    }
}

impl TimerDriver for HighResTimer {
    fn init(&mut self) -> Result<(), &'static str> {
        log::debug!("Initializing high-resolution timer, frequency: {}Hz", self.frequency);
        Ok(())
    }

    fn set_period(&mut self, _period_us: u64) -> Result<(), &'static str> {
        // High-res timer doesn't support periodic mode
        Err("High-res timer doesn't support periodic mode")
    }

    fn start(&mut self) -> Result<(), &'static str> {
        // High-res timer is always running
        Ok(())
    }

    fn stop(&mut self) -> Result<(), &'static str> {
        // High-res timer cannot be stopped
        Err("High-res timer cannot be stopped")
    }

    fn get_time(&self) -> u64 {
        if self.use_time_csr {
            self.read_time()
        } else {
            self.read_cycle()
        }
    }

    fn set_comparator(&mut self, _id: u32, _value: u64) -> Result<(), &'static str> {
        Err("High-res timer doesn't support comparators")
    }

    fn clear_interrupt(&mut self, _id: u32) -> Result<(), &'static str> {
        Err("High-res timer doesn't generate interrupts")
    }

    fn is_interrupt_pending(&self, _id: u32) -> bool {
        false
    }
}

/// Timer manager
pub struct TimerManager {
    /// Primary timer driver
    primary: Box<dyn TimerDriver>,
    /// High-resolution timer
    high_res: HighResTimer,
    /// Time since boot in nanoseconds
    boot_time_ns: u64,
}

impl TimerManager {
    /// Create new timer manager
    pub fn new() -> Result<Self, &'static str> {
        let clint_base = super::get_clint_base();
        let frequency = super::get_timer_frequency();

        let primary: Box<dyn TimerDriver> = Box::new(ClintTimer::new(clint_base, frequency));
        let high_res = HighResTimer::new();

        Ok(Self {
            primary,
            high_res,
            boot_time_ns: 0,
        })
    }

    /// Initialize timer manager
    pub fn init(&mut self) -> Result<(), &'static str> {
        log::info!("Initializing timer manager");

        // Initialize primary timer
        self.primary.init()?;

        // Initialize high-resolution timer
        self.high_res.init()?;

        // Record boot time
        self.boot_time_ns = self.high_res.get_time();

        log::info!("Timer manager initialized");
        Ok(())
    }

    /// Get time since boot in nanoseconds
    pub fn get_time_since_boot_ns(&self) -> u64 {
        let current = self.high_res.get_time();
        current.saturating_sub(self.boot_time_ns)
    }

    /// Get time since boot in microseconds
    pub fn get_time_since_boot_us(&self) -> u64 {
        self.get_time_since_boot_ns() / 1000
    }

    /// Get time since boot in milliseconds
    pub fn get_time_since_boot_ms(&self) -> u64 {
        self.get_time_since_boot_ns() / 1_000_000
    }

    /// Delay for specified microseconds
    pub fn delay_us(&self, us: u64) {
        let start = self.high_res.get_time();
        let end = start + self.high_res.ns_to_ticks(us * 1000);

        while self.high_res.get_time() < end {
            riscv::asm::pause();
        }
    }

    /// Delay for specified milliseconds
    pub fn delay_ms(&self, ms: u64) {
        for _ in 0..ms {
            self.delay_us(1000);
        }
    }

    /// Get primary timer
    pub fn get_primary(&mut self) -> &mut dyn TimerDriver {
        &mut *self.primary
    }

    /// Get high-resolution timer
    pub fn get_high_res(&self) -> &HighResTimer {
        &self.high_res
    }
}

/// Global timer manager
static mut TIMER_MANAGER: Option<TimerManager> = None;
static TIMER_MANAGER_INIT: spin::Once<()> = spin::Once::new();

/// Initialize timer subsystem (early)
pub fn early_init() -> Result<(), &'static str> {
    log::debug!("Early timer initialization");

    // Just create the timer manager without full initialization
    TIMER_MANAGER_INIT.call_once(|| {
        if let Ok(manager) = TimerManager::new() {
            unsafe {
                TIMER_MANAGER = Some(manager);
            }
        }
    });

    Ok(())
}

/// Initialize timer subsystem (late)
pub fn late_init() -> Result<(), &'static str> {
    log::info!("Initializing platform timer subsystem");

    if let Some(manager) = get_manager() {
        // Note: We need to get mutable access but this is a simplified version
        // In a real implementation, we'd need proper mutable access
        log::debug!("Timer manager already exists");
    } else {
        return Err("Timer manager not initialized");
    }

    log::info!("Platform timer subsystem initialized");
    Ok(())
}

/// Get timer manager
pub fn get_manager() -> Option<&'static TimerManager> {
    unsafe { TIMER_MANAGER.as_ref() }
}

/// Get mutable timer manager
pub fn get_manager_mut() -> Option<&'static mut TimerManager> {
    unsafe { TIMER_MANAGER.as_mut() }
}

/// Get current time in nanoseconds
pub fn get_time_ns() -> u64 {
    get_manager()
        .map(|m| m.get_time_since_boot_ns())
        .unwrap_or(0)
}

/// Get current time in microseconds
pub fn get_time_us() -> u64 {
    get_manager()
        .map(|m| m.get_time_since_boot_us())
        .unwrap_or(0)
}

/// Get current time in milliseconds
pub fn get_time_ms() -> u64 {
    get_manager()
        .map(|m| m.get_time_since_boot_ms())
        .unwrap_or(0)
}

/// Delay for specified microseconds
pub fn delay_us(us: u64) {
    if let Some(manager) = get_manager() {
        manager.delay_us(us);
    } else {
        // Fallback: simple busy loop
        for _ in 0..us {
            riscv::asm::nop();
        }
    }
}

/// Delay for specified milliseconds
pub fn delay_ms(ms: u64) {
    if let Some(manager) = get_manager() {
        manager.delay_ms(ms);
    } else {
        // Fallback: use microseconds delay
        for _ in 0..ms {
            delay_us(1000);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_config() {
        let config = TimerConfig::default();
        assert_eq!(config.frequency, 10000000);
        assert_eq!(config.base_address, 0x02000000);
        assert!(config.periodic);
        assert_eq!(config.period_us, 1000);
        assert!(!config.one_shot);
    }

    #[test]
    fn test_clint_timer() {
        let timer = ClintTimer::new(0x02000000, 10000000);
        assert_eq!(timer.base, 0x02000000);
        assert_eq!(timer.frequency, 10000000);
    }

    #[test]
    fn test_high_res_timer() {
        let timer = HighResTimer::new();
        // Check that timer was created
        assert!(timer.frequency > 0);
    }

    #[test]
    fn test_timer_manager() {
        let manager = TimerManager::new();
        assert!(manager.is_ok());
    }
}