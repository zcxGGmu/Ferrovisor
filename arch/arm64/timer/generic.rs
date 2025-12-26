//! ARM Generic Timer Driver
//!
//! Provides low-level access to ARM Generic Timer registers.
//!
//! ## Register Access
//!
//! This module provides functions to read/write all Generic Timer registers:
//! - System Counter: CNTVCT (virtual), CNTPCT (physical)
//! - Control Registers: CNTV_CTL, CNTP_CTL, CNTHP_CTL
//! - Value Registers: CNTV_CVAL, CNTP_CVAL, CNTHP_CVAL
//! - Timer Value Registers: CNTV_TVAL, CNTP_TVAL, CNTHP_TVAL
//! - Frequency Register: CNTFRQ
//! - Timer Offset: CNTVOFF_EL2
//!
//! ## References
//! - [ARM Architecture Reference Manual ARMv8-A](https://developer.arm.com/documentation/ddi0487/latest)

use super::{ctrl, TimerType, TIMER_MAX_TICKS};

/// Generic Timer register addresses (for read/write)
#[derive(Debug, Clone, Copy)]
pub enum TimerReg {
    /// Control register
    Ctl,
    /// Compare value register (absolute)
    Cval,
    /// Timer value register (relative)
    Tval,
}

/// Physical Timer registers (EL2/EL3)
pub mod physical {
    use super::TimerReg;

    /// Read Physical Timer control register
    #[inline]
    pub fn read_ctl() -> u64 {
        let val: u64;
        unsafe {
            core::arch::asm!("mrs {}, cntp_ctl_el0", out(reg) val);
        }
        val
    }

    /// Write Physical Timer control register
    #[inline]
    pub fn write_ctl(val: u64) {
        unsafe {
            core::arch::asm!("msr cntp_ctl_el0, {}", in(reg) val);
        }
    }

    /// Read Physical Timer compare value
    #[inline]
    pub fn read_cval() -> u64 {
        let val: u64;
        unsafe {
            core::arch::asm!("mrs {}, cntp_cval_el0", out(reg) val);
        }
        val
    }

    /// Write Physical Timer compare value
    #[inline]
    pub fn write_cval(val: u64) {
        unsafe {
            core::arch::asm!("msr cntp_cval_el0, {}", in(reg) val);
        }
    }

    /// Read Physical Timer value (ticks until event)
    #[inline]
    pub fn read_tval() -> i64 {
        let val: i64;
        unsafe {
            core::arch::asm!("mrs {}, cntp_tval_el0", out(reg) val);
        }
        val
    }

    /// Write Physical Timer value
    #[inline]
    pub fn write_tval(val: i64) {
        unsafe {
            core::arch::asm!("msr cntp_tval_el0, {}", in(reg) val);
        }
    }

    /// Read Physical Counter
    #[inline]
    pub fn read_counter() -> u64 {
        let val: u64;
        unsafe {
            core::arch::asm!("mrs {}, cntpct_el0", out(reg) val);
        }
        val & TIMER_MAX_TICKS
    }
}

/// Virtual Timer registers (EL1/EL0)
pub mod virtual_ {
    /// Read Virtual Timer control register
    #[inline]
    pub fn read_ctl() -> u64 {
        let val: u64;
        unsafe {
            core::arch::asm!("mrs {}, cntv_ctl_el0", out(reg) val);
        }
        val
    }

    /// Write Virtual Timer control register
    #[inline]
    pub fn write_ctl(val: u64) {
        unsafe {
            core::arch::asm!("msr cntv_ctl_el0, {}", in(reg) val);
        }
    }

    /// Read Virtual Timer compare value
    #[inline]
    pub fn read_cval() -> u64 {
        let val: u64;
        unsafe {
            core::arch::asm!("mrs {}, cntv_cval_el0", out(reg) val);
        }
        val
    }

    /// Write Virtual Timer compare value
    #[inline]
    pub fn write_cval(val: u64) {
        unsafe {
            core::arch::asm!("msr cntv_cval_el0, {}", in(reg) val);
        }
    }

    /// Read Virtual Timer value (ticks until event)
    #[inline]
    pub fn read_tval() -> i64 {
        let val: i64;
        unsafe {
            core::arch::asm!("mrs {}, cntv_tval_el0", out(reg) val);
        }
        val
    }

    /// Write Virtual Timer value
    #[inline]
    pub fn write_tval(val: i64) {
        unsafe {
            core::arch::asm!("msr cntv_tval_el0, {}", in(reg) val);
        }
    }

    /// Read Virtual Counter
    #[inline]
    pub fn read_counter() -> u64 {
        let val: u64;
        unsafe {
            core::arch::asm!("mrs {}, cntvct_el0", out(reg) val);
        }
        val & TIMER_MAX_TICKS
    }
}

/// Hypervisor Physical Timer registers (EL2)
pub mod hyp_physical {
    /// Read Hypervisor Physical Timer control register
    #[inline]
    pub fn read_ctl() -> u64 {
        let val: u64;
        unsafe {
            core::arch::asm!("mrs {}, cnthp_ctl_el2", out(reg) val);
        }
        val
    }

    /// Write Hypervisor Physical Timer control register
    #[inline]
    pub fn write_ctl(val: u64) {
        unsafe {
            core::arch::asm!("msr cnthp_ctl_el2, {}", in(reg) val);
        }
    }

    /// Read Hypervisor Physical Timer compare value
    #[inline]
    pub fn read_cval() -> u64 {
        let val: u64;
        unsafe {
            core::arch::asm!("mrs {}, cnthp_cval_el2", out(reg) val);
        }
        val
    }

    /// Write Hypervisor Physical Timer compare value
    #[inline]
    pub fn write_cval(val: u64) {
        unsafe {
            core::arch::asm!("msr cnthp_cval_el2, {}", in(reg) val);
        }
    }

    /// Read Hypervisor Physical Timer value
    #[inline]
    pub fn read_tval() -> i64 {
        let val: i64;
        unsafe {
            core::arch::asm!("mrs {}, cnthp_tval_el2", out(reg) val);
        }
        val
    }

    /// Write Hypervisor Physical Timer value
    #[inline]
    pub fn write_tval(val: i64) {
        unsafe {
            core::arch::asm!("msr cnthp_tval_el2, {}", in(reg) val);
        }
    }
}

/// Timer virtual offset register
pub mod offset {
    /// Read virtual timer offset
    #[inline]
    pub fn read() -> u64 {
        let val: u64;
        unsafe {
            core::arch::asm!("mrs {}, cntvoff_el2", out(reg) val);
        }
        val
    }

    /// Write virtual timer offset
    #[inline]
    pub fn write(val: u64) {
        unsafe {
            core::arch::asm!("msr cntvoff_el2, {}", in(reg) val);
        }
    }
}

/// Generic Timer state for a specific timer type
#[derive(Debug, Clone)]
pub struct GenericTimerState {
    /// Timer type
    pub timer_type: TimerType,
    /// Control register value
    pub ctl: u64,
    /// Compare value
    pub cval: u64,
    /// Timer is enabled
    pub enabled: bool,
    /// Interrupt is masked
    pub masked: bool,
    /// Interrupt is pending
    pub pending: bool,
}

impl GenericTimerState {
    /// Create new timer state
    pub fn new(timer_type: TimerType) -> Self {
        Self {
            timer_type,
            ctl: 0,
            cval: 0,
            enabled: false,
            masked: true,
            pending: false,
        }
    }

    /// Check if timer interrupt is pending
    pub fn is_pending(&self) -> bool {
        self.ctl & ctrl::ISTATUS != 0
    }

    /// Check if timer is enabled
    pub fn is_enabled(&self) -> bool {
        self.ctl & ctrl::ENABLE != 0
    }

    /// Check if interrupt is masked
    pub fn is_masked(&self) -> bool {
        self.ctl & ctrl::IMASK != 0
    }

    /// Enable timer
    pub fn enable(&mut self) {
        self.ctl |= ctrl::ENABLE;
        self.enabled = true;
    }

    /// Disable timer
    pub fn disable(&mut self) {
        self.ctl &= !ctrl::ENABLE;
        self.enabled = false;
    }

    /// Unmask interrupt
    pub fn unmask(&mut self) {
        self.ctl &= !ctrl::IMASK;
        self.masked = false;
    }

    /// Mask interrupt
    pub fn mask(&mut self) {
        self.ctl |= ctrl::IMASK;
        self.masked = true;
    }

    /// Set compare value
    pub fn set_cval(&mut self, cval: u64) {
        self.cval = cval;
    }

    /// Get compare value
    pub fn cval(&self) -> u64 {
        self.cval
    }

    /// Get control register value
    pub fn ctl_value(&self) -> u64 {
        self.ctl
    }
}

/// Read timer register based on timer type
pub fn read_reg(timer_type: TimerType, reg: TimerReg) -> u64 {
    match (timer_type, reg) {
        (TimerType::Physical, TimerReg::Ctl) => physical::read_ctl() as u64,
        (TimerType::Physical, TimerReg::Cval) => physical::read_cval(),
        (TimerType::Physical, TimerReg::Tval) => physical::read_tval() as u64,

        (TimerType::Virtual, TimerReg::Ctl) => virtual_::read_ctl() as u64,
        (TimerType::Virtual, TimerReg::Cval) => virtual_::read_cval(),
        (TimerType::Virtual, TimerReg::Tval) => virtual_::read_tval() as u64,

        (TimerType::HypPhysical, TimerReg::Ctl) => hyp_physical::read_ctl() as u64,
        (TimerType::HypPhysical, TimerReg::Cval) => hyp_physical::read_cval(),
        (TimerType::HypPhysical, TimerReg::Tval) => hyp_physical::read_tval() as u64,

        _ => 0,
    }
}

/// Write timer register based on timer type
pub fn write_reg(timer_type: TimerType, reg: TimerReg, val: u64) {
    match (timer_type, reg) {
        (TimerType::Physical, TimerReg::Ctl) => physical::write_ctl(val),
        (TimerType::Physical, TimerReg::Cval) => physical::write_cval(val),
        (TimerType::Physical, TimerReg::Tval) => physical::write_tval(val as i64),

        (TimerType::Virtual, TimerReg::Ctl) => virtual_::write_ctl(val),
        (TimerType::Virtual, TimerReg::Cval) => virtual_::write_cval(val),
        (TimerType::Virtual, TimerReg::Tval) => virtual_::write_tval(val as i64),

        (TimerType::HypPhysical, TimerReg::Ctl) => hyp_physical::write_ctl(val),
        (TimerType::HypPhysical, TimerReg::Cval) => hyp_physical::write_cval(val),
        (TimerType::HypPhysical, TimerReg::Tval) => hyp_physical::write_tval(val as i64),

        _ => {}
    }
}

/// Stop timer (disable and mask interrupt)
pub fn stop_timer(timer_type: TimerType) {
    let mut ctl = read_reg(timer_type, TimerReg::Ctl);
    ctl |= ctrl::IMASK;      // Mask interrupt
    ctl &= !ctrl::ENABLE;    // Disable timer
    write_reg(timer_type, TimerReg::Ctl, ctl);
}

/// Start timer (enable and unmask interrupt)
pub fn start_timer(timer_type: TimerType) {
    let mut ctl = read_reg(timer_type, TimerReg::Ctl);
    ctl &= !ctrl::IMASK;     // Unmask interrupt
    ctl |= ctrl::ENABLE;     // Enable timer
    write_reg(timer_type, TimerReg::Ctl, ctl);
}

/// Set timer to expire after specified ticks
///
/// # Parameters
/// - `timer_type`: Which timer to program
/// - `ticks`: Ticks until expiration (relative)
pub fn set_timer_ticks(timer_type: TimerType, ticks: i64) {
    write_reg(timer_type, TimerReg::Tval, ticks as u64);
}

/// Set timer to expire at specific counter value
///
/// # Parameters
/// - `timer_type`: Which timer to program
/// - `cval`: Compare value (absolute)
pub fn set_timer_cval(timer_type: TimerType, cval: u64) {
    write_reg(timer_type, TimerReg::Cval, cval);
}

/// Get timer state
pub fn get_timer_state(timer_type: TimerType) -> GenericTimerState {
    let ctl = read_reg(timer_type, TimerReg::Ctl);
    let cval = read_reg(timer_type, TimerReg::Cval);

    GenericTimerState {
        timer_type,
        ctl,
        cval,
        enabled: ctl & ctrl::ENABLE != 0,
        masked: ctl & ctrl::IMASK != 0,
        pending: ctl & ctrl::ISTATUS != 0,
    }
}

/// Initialize Generic Timer driver
pub fn init() -> Result<(), &'static str> {
    log::info!("Generic Timer: Initializing driver");

    // Read and log counter frequency
    let freq = super::read_counter_freq();
    log::info!("Generic Timer: Counter frequency = {} Hz ({} MHz)",
               freq, freq / 1_000_000);

    // Ensure timers are disabled initially
    stop_timer(TimerType::Physical);
    stop_timer(TimerType::Virtual);
    stop_timer(TimerType::HypPhysical);

    log::info!("Generic Timer: Driver initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_physical_timer_access() {
        // These just verify the functions compile
        let _ctl = physical::read_ctl();
        let _cval = physical::read_cval();
        let _tval = physical::read_tval();
        let _cnt = physical::read_counter();
    }

    #[test]
    fn test_virtual_timer_access() {
        // These just verify the functions compile
        let _ctl = virtual_::read_ctl();
        let _cval = virtual_::read_cval();
        let _tval = virtual_::read_tval();
        let _cnt = virtual_::read_counter();
    }

    #[test]
    fn test_hyp_physical_timer_access() {
        // These just verify the functions compile
        let _ctl = hyp_physical::read_ctl();
        let _cval = hyp_physical::read_cval();
        let _tval = hyp_physical::read_tval();
    }

    #[test]
    fn test_timer_state() {
        let mut state = GenericTimerState::new(TimerType::Virtual);
        assert!(!state.is_enabled());
        assert!(state.is_masked());

        state.enable();
        assert!(state.is_enabled());

        state.unmask();
        assert!(!state.is_masked());

        state.set_cval(0x1000);
        assert_eq!(state.cval(), 0x1000);
    }
}
