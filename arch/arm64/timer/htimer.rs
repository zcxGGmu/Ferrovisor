//! EL2 Hypervisor Timer for ARM64
//!
//! Provides hypervisor timer support for EL2 scheduling.
//!
//! ## Hypervisor Timer Overview
//!
//! The hypervisor physical timer (CNTHP) is used by EL2:
//! - EL2 only (for hypervisor scheduling)
//! - Not accessible to guest
//! - Used for preempting VCPUs
//! - Can generate hypervisor timer interrupts
//!
//! ## Usage
//!
//! Hypervisor timer is typically used for:
//! - VCPU time slice scheduling
//! - Watchdog timers
//! - Timeout operations
//!
//! ## References
//! - [ARM Architecture Reference Manual ARMv8-A](https://developer.arm.com/documentation/ddi0487/latest)

use super::generic::{self, TimerType, TimerReg, hyp_physical};

/// Hypervisor Timer state
#[derive(Debug, Clone)]
pub struct HypTimerState {
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

impl Default for HypTimerState {
    fn default() -> Self {
        Self {
            ctl: 0,
            cval: 0,
            enabled: false,
            masked: true,
            pending: false,
        }
    }
}

impl HypTimerState {
    /// Create new hypervisor timer state
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if timer interrupt is pending
    pub fn is_pending(&self) -> bool {
        self.ctl & super::ctrl::ISTATUS != 0
    }

    /// Check if timer is enabled
    pub fn is_enabled(&self) -> bool {
        self.ctl & super::ctrl::ENABLE != 0
    }

    /// Check if interrupt is masked
    pub fn is_masked(&self) -> bool {
        self.ctl & super::ctrl::IMASK != 0
    }

    /// Get remaining ticks
    pub fn remaining_ticks(&self) -> i64 {
        let counter = hyp_physical::read_counter();
        let cval = self.cval;

        if counter >= cval {
            return 0; // Timer expired
        }

        (cval - counter) as i64
    }

    /// Enable timer
    pub fn enable(&mut self) {
        self.ctl |= super::ctrl::ENABLE;
        self.enabled = true;
    }

    /// Disable timer
    pub fn disable(&mut self) {
        self.ctl &= !super::ctrl::ENABLE;
        self.enabled = false;
    }

    /// Unmask interrupt
    pub fn unmask(&mut self) {
        self.ctl &= !super::ctrl::IMASK;
        self.masked = false;
    }

    /// Mask interrupt
    pub fn mask(&mut self) {
        self.ctl |= super::ctrl::IMASK;
        self.masked = true;
    }

    /// Set compare value
    pub fn set_cval(&mut self, cval: u64) {
        self.cval = cval;
    }

    /// Get control register value
    pub fn ctl_value(&self) -> u64 {
        self.ctl
    }

    /// Program timer to expire after specified ticks
    ///
    /// # Parameters
    /// - `ticks`: Ticks until expiration
    pub fn set_timer_ticks(&mut self, ticks: i64) {
        let counter = hyp_physical::read_counter();
        let cval = counter.wrapping_add(ticks as u64) & super::TIMER_MAX_TICKS;
        self.set_cval(cval);
    }

    /// Program timer to expire at specific counter value
    ///
    /// # Parameters
    /// - `cval`: Compare value (absolute)
    pub fn set_timer_cval(&mut self, cval: u64) {
        self.set_cval(cval);
    }

    /// Save timer state from hardware
    pub fn save(&mut self) {
        self.ctl = hyp_physical::read_ctl();
        self.cval = hyp_physical::read_cval();
        self.enabled = self.is_enabled();
        self.masked = self.is_masked();
        self.pending = self.is_pending();
    }

    /// Restore timer state to hardware
    pub fn restore(&self) {
        hyp_physical::write_cval(self.cval);
        hyp_physical::write_ctl(self.ctl);
    }

    /// Stop timer (disable and mask)
    pub fn stop(&mut self) {
        self.disable();
        self.mask();
    }

    /// Start timer (enable and unmask)
    pub fn start(&mut self) {
        // Set compare value first
        hyp_physical::write_cval(self.cval);

        // Enable and unmask
        let mut ctl = self.ctl;
        ctl &= !super::ctrl::IMASK;     // Unmask
        ctl |= super::ctrl::ENABLE;     // Enable
        hyp_physical::write_ctl(ctl);
    }
}

/// Hypervisor Timer callback trait
///
/// Implement this trait to handle hypervisor timer events.
pub trait HypTimerCallback {
    /// Called when hypervisor timer expires
    fn timer_expired(&mut self);
}

/// Global hypervisor timer state
static mut HTIMER_STATE: Option<HypTimerState> = None;

/// Hypervisor Timer context
pub struct HypTimerContext {
    /// Timer state
    pub state: HypTimerState,
    /// Callback for timer events
    callback: Option<*mut dyn HypTimerCallback>,
}

impl Default for HypTimerContext {
    fn default() -> Self {
        Self {
            state: HypTimerState::default(),
            callback: None,
        }
    }
}

impl HypTimerContext {
    /// Create new hypervisor timer context
    pub fn new() -> Self {
        Self::default()
    }

    /// Get timer state
    pub fn state(&self) -> &HypTimerState {
        &self.state
    }

    /// Get mutable timer state
    pub fn state_mut(&mut self) -> &mut HypTimerState {
        &mut self.state
    }

    /// Set callback
    pub fn set_callback(&mut self, callback: *mut dyn HypTimerCallback) {
        self.callback = Some(callback);
    }

    /// Clear callback
    pub fn clear_callback(&mut self) {
        self.callback = None;
    }

    /// Check if timer has expired
    pub fn has_expired(&self) -> bool {
        let counter = hyp_physical::read_counter();
        counter >= self.state.cval
    }

    /// Handle timer interrupt
    ///
    /// Returns true if interrupt was handled.
    pub fn handle_irq(&mut self) -> bool {
        // Read control register to check if interrupt is pending
        let ctl = hyp_physical::read_ctl();

        if ctl & super::ctrl::ISTATUS != 0 {
            log::debug!("Hyp Timer: Interrupt pending");

            // Disable timer
            let mut ctrl_val = hyp_physical::read_ctl();
            ctrl_val |= super::ctrl::IMASK;
            ctrl_val &= !super::ctrl::ENABLE;
            hyp_physical::write_ctl(ctrl_val);

            // Call callback if registered
            if let Some(cb_ptr) = self.callback {
                unsafe {
                    let cb = &mut *cb_ptr;
                    cb.timer_expired();
                }
            }

            true
        } else {
            false
        }
    }

    /// Save context
    pub fn save(&mut self) {
        self.state.save();
    }

    /// Restore context
    pub fn restore(&self) {
        self.state.restore();
    }
}

/// Initialize hypervisor timer
pub fn init() -> Result<(), &'static str> {
    log::info!("Hyp Timer: Initializing");

    // Ensure timer is stopped
    stop_timer();

    log::info!("Hyp Timer: Initialized");
    Ok(())
}

/// Get hypervisor timer context
pub fn context() -> Option<&'static HypTimerContext> {
    unsafe { HTIMER_STATE.as_ref().map(|s| s as *const HypTimerContext as *const _ as *const HypTimerContext) }
}

/// Get mutable hypervisor timer context
pub fn context_mut() -> Option<&'static mut HypTimerContext> {
    unsafe { HTIMER_STATE.as_mut().map(|s| s as *mut HypTimerContext as *mut _ as *mut HypTimerContext) }
}

/// Stop hypervisor timer
pub fn stop_timer() {
    let mut ctl = hyp_physical::read_ctl();
    ctl |= super::ctrl::IMASK;      // Mask interrupt
    ctl &= !super::ctrl::ENABLE;    // Disable timer
    hyp_physical::write_ctl(ctl);
}

/// Start hypervisor timer with timeout
///
/// # Parameters
/// - `ticks`: Ticks until expiration
pub fn start_timer_ticks(ticks: i64) {
    let counter = hyp_physical::read_counter();
    let cval = counter.wrapping_add(ticks as u64) & super::TIMER_MAX_TICKS;

    // Write compare value
    hyp_physical::write_cval(cval);

    // Enable and unmask timer
    let mut ctl = hyp_physical::read_ctl();
    ctl &= !super::ctrl::IMASK;     // Unmask
    ctl |= super::ctrl::ENABLE;     // Enable
    hyp_physical::write_ctl(ctl);
}

/// Start hypervisor timer with absolute compare value
///
/// # Parameters
/// - `cval`: Compare value (absolute)
pub fn start_timer_cval(cval: u64) {
    // Write compare value
    hyp_physical::write_cval(cval);

    // Enable and unmask timer
    let mut ctl = hyp_physical::read_ctl();
    ctl &= !super::ctrl::IMASK;     // Unmask
    ctl |= super::ctrl::ENABLE;     // Enable
    hyp_physical::write_ctl(ctl);
}

/// Check if hypervisor timer has expired
pub fn has_expired() -> bool {
    let ctl = hyp_physical::read_ctl();
    (ctl & super::ctrl::ISTATUS) != 0
}

/// Get remaining ticks
pub fn remaining_ticks() -> i64 {
    let counter = hyp_physical::read_counter();
    let cval = hyp_physical::read_cval();

    if counter >= cval {
        return 0;
    }

    (cval - counter) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hyp_timer_state() {
        let mut state = HypTimerState::new();
        assert!(!state.is_enabled());
        assert!(state.is_masked());

        state.enable();
        assert!(state.is_enabled());

        state.unmask();
        assert!(!state.is_masked());

        state.set_cval(0x1000);
        assert_eq!(state.cval, 0x1000);
    }

    #[test]
    fn test_hyp_timer_context() {
        let ctx = HypTimerContext::new();
        assert!(!ctx.state().is_enabled());
        assert!(!ctx.has_expired());
    }

    #[test]
    fn test_timer_programming() {
        // These just verify the functions compile
        stop_timer();
        start_timer_ticks(1000);
        start_timer_cval(0x10000000);

        let _expired = has_expired();
        let _remaining = remaining_ticks();
    }
}
