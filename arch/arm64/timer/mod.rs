//! ARM Generic Timer Support for ARM64
//!
//! Provides ARM Generic Timer (ARCH_TIMER) support for ARM64 systems.
//!
//! ## ARM Generic Timer Overview
//!
//! The ARM Generic Timer provides:
//! - **System Counter**: Global monotonic counter shared across all CPUs
//! - **Physical Timer (CNTP)**: EL3/EL2 timer for secure/hypervisor use
//! - **Virtual Timer (CNTV)**: EL1/EL0 timer for OS/Applications
//! - **Hypervisor Timer (CNTHP)**: EL2 timer for hypervisor scheduling
//!
//! ## Timer Registers
//!
//! Each timer has control and value registers:
//! - `CNT*_CTL`: Control register (enable, mask, status)
//! - `CNT*_CVAL`: Compare value register (absolute)
//! - `CNT*_TVAL**: Timer value register (relative)
//! - `CNT*_CT`: Counter register (read-only)
//!
//! ## Timer Interrupts
//!
//! - Physical Timer: Secure/Non-secure PHYSTIMER IRQ
//! - Virtual Timer: VIRQ (virtualized to guest)
//! - Hypervisor Timer: HYPTIMER IRQ for hypervisor
//!
//! ## References
//! - [ARM Architecture Reference Manual ARMv8-A](https://developer.arm.com/documentation/ddi0487/latest)
//! - [Xvisor Generic Timer](https://github.com/xvisor/xvisor)

/// Generic Timer module
pub mod generic;

/// Virtual Timer module
pub mod virtual_timer;

/// Hypervisor Timer module
pub mod htimer;

// Re-export key types
pub use generic::*;
pub use virtual_timer::*;
pub use htimer::*;

/// ARM Generic Timer frequency
pub const TIMER_DEFAULT_HZ: u32 = 62_500_000; // 62.5 MHz (common on ARMv8)

/// Maximum timer ticks (56-bit counter)
pub const TIMER_MAX_TICKS: u64 = 0x00FFFFFFFFFFFFFFu64;

/// Timer control register bits
pub mod ctrl {
    /// Timer enable bit
    pub const ENABLE: u64 = 1 << 0;
    /// Timer interrupt mask bit
    pub const IMASK: u64 = 1 << 1;
    /// Timer interrupt status bit
    pub const ISTATUS: u64 = 1 << 2;
}

/// Timer types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TimerType {
    /// Physical Timer (CNTP)
    Physical = 0,
    /// Virtual Timer (CNTV)
    Virtual = 1,
    /// Hypervisor Physical Timer (CNTHP)
    HypPhysical = 2,
    /// Hypervisor Virtual Timer (CNTHV)
    HypVirtual = 3,
}

impl TimerType {
    /// Get timer type name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Physical => "Physical",
            Self::Virtual => "Virtual",
            Self::HypPhysical => "HypPhysical",
            Self::HypVirtual => "HypVirtual",
        }
    }
}

/// Read system counter
///
/// Returns the current system counter value.
#[inline]
pub fn read_counter() -> u64 {
    let cnt: u64;
    unsafe {
        core::arch::asm!("mrs {}, cntvct_el0", out(reg) cnt);
    }
    cnt
}

/// Read counter frequency
///
/// Returns the counter frequency in Hz.
#[inline]
pub fn read_counter_freq() -> u64 {
    let freq: u64;
    unsafe {
        core::arch::asm!("mrs {}, cntfrq_el0", out(reg) freq);
    }
    freq
}

/// Get timer tick in nanoseconds
///
/// Converts timer ticks to nanoseconds based on counter frequency.
pub fn ticks_to_ns(ticks: u64, freq: u64) -> u64 {
    if freq == 0 {
        return 0;
    }
    // ticks * 1e9 / freq
    const BILLION: u64 = 1_000_000_000;
    let mut result = ticks;
    result = result.saturating_mul(BILLION);
    result / freq
}

/// Get timer ticks from nanoseconds
///
/// Converts nanoseconds to timer ticks based on counter frequency.
pub fn ns_to_ticks(ns: u64, freq: u64) -> u64 {
    if freq == 0 {
        return 0;
    }
    // ns * freq / 1e9
    const BILLION: u64 = 1_000_000_000;
    let mut result = ns;
    result = result.saturating_mul(freq);
    result / BILLION
}

/// Get timer ticks from microseconds
pub fn us_to_ticks(us: u64, freq: u64) -> u64 {
    if freq == 0 {
        return 0;
    }
    // us * freq / 1e6
    const MILLION: u64 = 1_000_000;
    let mut result = us;
    result = result.saturating_mul(freq);
    result / MILLION
}

/// Initialize ARM Generic Timer
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing ARM Generic Timer");

    // Get counter frequency
    let freq = read_counter_freq();
    log::info!("Timer: Counter frequency = {} Hz ({} MHz)", freq, freq / 1_000_000);

    // Initialize sub-modules
    generic::init()?;
    virtual_timer::init()?;
    htimer::init()?;

    log::info!("ARM Generic Timer initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_type_names() {
        assert_eq!(TimerType::Physical.name(), "Physical");
        assert_eq!(TimerType::Virtual.name(), "Virtual");
        assert_eq!(TimerType::HypPhysical.name(), "HypPhysical");
        assert_eq!(TimerType::HypVirtual.name(), "HypVirtual");
    }

    #[test]
    fn test_ticks_conversion() {
        let freq = 62_500_000; // 62.5 MHz
        let ticks = 62_500_000; // 1 second worth of ticks

        let ns = ticks_to_ns(ticks, freq);
        assert_eq!(ns, 1_000_000_000); // 1 second in ns

        let us = ticks / 1000;
        let ticks_back = us_to_ticks(us, freq);
        assert_eq!(ticks_back, ticks / 1000);
    }

    #[test]
    fn test_ns_to_ticks() {
        let freq = 100_000_000; // 100 MHz
        let ns = 1_000_000_000; // 1 second

        let ticks = ns_to_ticks(ns, freq);
        assert_eq!(ticks, 100_000_000); // 1 second worth of ticks

        let ns = 1_000_000; // 1 millisecond
        let ticks = ns_to_ticks(ns, freq);
        assert_eq!(ticks, 100_000); // 1ms worth of ticks
    }
}
