//! Virtual Timer for ARM64
//!
//! Provides virtual timer support for guest VMs.
//!
//! ## Virtual Timer Overview
//!
//! The virtual timer (CNTV) is used by guest OS/Applications:
//! - EL1/EL0 accessible (when not trapped)
//! - Offset by CNTVOFF_EL2 for virtualization
//! - Can be trapped to EL2 for emulation
//!
//! ## Virtualization
//!
//! When trapping is enabled (CNTHCTL_EL2.EL1TVCT):
//! - Timer registers trap to EL2
//! - Hypervisor emulates timer behavior
//! - Timer interrupts are injected as virtual IRQs
//!
//! ## References
//! - [ARM Architecture Reference Manual ARMv8-A](https://developer.arm.com/documentation/ddi0487/latest)

use super::generic::{self, TimerType, TimerReg, virtual_};

/// Virtual Timer state for a VCPU
#[derive(Debug, Clone)]
pub struct VirtualTimerState {
    /// Virtual control register value
    pub ctl: u64,
    /// Virtual compare value
    pub cval: u64,
    /// Virtual offset
    pub offset: u64,
    /// Timer is enabled
    pub enabled: bool,
    /// Interrupt is masked
    pub masked: bool,
    /// Physical IRQ number (for injection)
    pub irq: u32,
}

impl Default for VirtualTimerState {
    fn default() -> Self {
        Self {
            ctl: 0,
            cval: 0,
            offset: 0,
            enabled: false,
            masked: true,
            irq: 27, // Default virtual timer IRQ
        }
    }
}

impl VirtualTimerState {
    /// Create new virtual timer state
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with specific IRQ
    pub fn with_irq(irq: u32) -> Self {
        Self {
            irq,
            ..Self::default()
        }
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
        let counter = self.read_virtual_counter();
        let cval = self.cval;

        if counter >= cval {
            return 0; // Timer expired
        }

        (cval - counter) as i64
    }

    /// Read virtual counter (with offset)
    pub fn read_virtual_counter(&self) -> u64 {
        let phys_cnt = generic::physical::read_counter();
        (phys_cnt.wrapping_sub(self.offset)) & super::TIMER_MAX_TICKS
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

    /// Set virtual offset
    pub fn set_offset(&mut self, offset: u64) {
        self.offset = offset;
    }

    /// Get virtual offset
    pub fn offset(&self) -> u64 {
        self.offset
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
        let counter = self.read_virtual_counter();
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
        self.ctl = virtual_::read_ctl();
        self.cval = virtual_::read_cval();
        self.offset = super::generic::offset::read();
        self.enabled = self.is_enabled();
        self.masked = self.is_masked();
    }

    /// Restore timer state to hardware
    pub fn restore(&self) {
        // Set virtual offset first
        super::generic::offset::write(self.offset);

        // Set compare value
        virtual_::write_cval(self.cval);

        // Set control register
        virtual_::write_ctl(self.ctl);
    }

    /// Stop timer (disable and mask)
    pub fn stop(&mut self) {
        self.disable();
        self.mask();
    }

    /// Start timer (enable and unmask)
    pub fn start(&mut self) {
        self.unmask();
        self.enable();
    }
}

/// Virtual Timer context
pub struct VirtualTimerContext {
    /// Timer state
    pub state: VirtualTimerState,
    /// Physical timer IRQ (for virtualization)
    pub phys_irq: u32,
}

impl Default for VirtualTimerContext {
    fn default() -> Self {
        Self {
            state: VirtualTimerState::default(),
            phys_irq: 30, // Default physical timer IRQ
        }
    }
}

impl VirtualTimerContext {
    /// Create new virtual timer context
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with specific IRQs
    pub fn with_irqs(virt_irq: u32, phys_irq: u32) -> Self {
        Self {
            state: VirtualTimerState::with_irq(virt_irq),
            phys_irq,
        }
    }

    /// Get virtual timer state
    pub fn state(&self) -> &VirtualTimerState {
        &self.state
    }

    /// Get mutable virtual timer state
    pub fn state_mut(&mut self) -> &mut VirtualTimerState {
        &mut self.state
    }

    /// Check if timer has expired
    pub fn has_expired(&self) -> bool {
        let counter = self.state.read_virtual_counter();
        counter >= self.state.cval
    }

    /// Inject virtual timer IRQ to guest
    ///
    /// Returns true if interrupt was injected.
    pub fn inject_irq(&self) -> bool {
        // In a real implementation, this would inject the virtual IRQ
        // to the VCPU via the interrupt controller
        log::debug!("Virtual Timer: Injecting IRQ {}", self.state.irq);
        true
    }

    /// Handle physical timer interrupt
    ///
    /// Called when the physical timer backing this virtual timer expires.
    pub fn handle_phys_irq(&mut self) -> bool {
        log::debug!("Virtual Timer: Physical IRQ {} received", self.phys_irq);

        if self.has_expired() {
            // Timer expired, inject virtual IRQ
            self.inject_irq();

            // If timer is periodic, reprogram it
            // For now, stop the timer
            self.state.stop();

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

/// Global virtual timer context (per VCPU)
static mut VTIMER_CONTEXT: Option<VirtualTimerContext> = None;

/// Initialize virtual timer
pub fn init() -> Result<(), &'static str> {
    log::info!("Virtual Timer: Initializing");

    // Create default context
    let ctx = VirtualTimerContext::new();

    unsafe {
        VTIMER_CONTEXT = Some(ctx);
    }

    log::info!("Virtual Timer: Initialized");
    Ok(())
}

/// Get virtual timer context
pub fn context() -> Option<&'static VirtualTimerContext> {
    unsafe { VTIMER_CONTEXT.as_ref() }
}

/// Get mutable virtual timer context
pub fn context_mut() -> Option<&'static mut VirtualTimerContext> {
    unsafe { VTIMER_CONTEXT.as_mut() }
}

/// Create virtual timer context for a VCPU
pub fn create_context(virt_irq: u32, phys_irq: u32) -> VirtualTimerContext {
    VirtualTimerContext::with_irqs(virt_irq, phys_irq)
}

/// Program virtual timer
///
/// # Parameters
/// - `ticks`: Ticks until expiration
pub fn program_timer(ticks: i64) -> Result<(), &'static str> {
    if let Some(ctx) = context_mut() {
        ctx.state_mut().set_timer_ticks(ticks);
        Ok(())
    } else {
        Err("Virtual timer context not initialized")
    }
}

/// Read virtual timer counter
pub fn read_counter() -> u64 {
    if let Some(ctx) = context() {
        ctx.state().read_virtual_counter()
    } else {
        super::read_counter()
    }
}

/// Check if virtual timer has expired
pub fn has_expired() -> bool {
    if let Some(ctx) = context() {
        ctx.has_expired()
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_timer_state() {
        let mut state = VirtualTimerState::new();
        assert!(!state.is_enabled());
        assert!(state.is_masked());

        state.enable();
        assert!(state.is_enabled());

        state.unmask();
        assert!(!state.is_masked());

        state.set_cval(0x1000);
        assert_eq!(state.cval, 0x1000);

        state.set_offset(0x1000);
        assert_eq!(state.offset(), 0x1000);
    }

    #[test]
    fn test_virtual_timer_context() {
        let ctx = VirtualTimerContext::with_irqs(27, 30);
        assert_eq!(ctx.state().irq, 27);
        assert_eq!(ctx.phys_irq, 30);
    }

    #[test]
    fn test_timer_ticks() {
        let mut state = VirtualTimerState::new();
        state.set_timer_ticks(1000);
        // Timer is programmed to expire in 1000 ticks
        assert!(state.cval > 0);
    }
}
