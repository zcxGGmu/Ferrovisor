//! WFI (Wait For Interrupt) Instruction Handling for ARM64
//!
//! Provides WFI instruction trap handling for ARM64 virtualization.
//!
//! WFI is a hint instruction that suggests the processor is idle.
//! When HCR_EL2.TWI is set, WFI executed at EL0/EL1 traps to EL2.
//!
//! ## WFI Behavior
//!
//! - WFI causes the processor to enter a low-power state until:
//!   - An interrupt is pending
//!   - A debug exception is pending
//!   - An event occurs (implementation-defined)
//!
//! - In virtualization context, trapped WFI can be:
//!   - Handled by hypervisor (wait for virtual interrupt)
//!   - Ignored (treated as NOP)
//!   - Passed through to hardware
//!
//! ## References
//! - [ARM Architecture Reference Manual ARMv8-A](https://developer.arm.com/documentation/ddi0487/latest)
//! - [Xvisor WFI Implementation](https://github.com/xvisor/xvisor)

use crate::arch::arm64::ExceptionClass;

/// WFI instruction syndrome bit definitions
pub mod iss {
    /// ISS bit [0] - WFI vs WFE (0 = WFI, 1 = WFE)
    pub const WFI_WFE_TI_MASK: u32 = 0x00000001;
    pub const WFI_WFE_TI_SHIFT: u32 = 0;

    /// Check if trapped instruction is WFE
    #[inline]
    pub const fn is_wfe(iss: u32) -> bool {
        (iss & WFI_WFE_TI_MASK) != 0
    }

    /// Check if trapped instruction is WFI
    #[inline]
    pub const fn is_wfi(iss: u32) -> bool {
        (iss & WFI_WFE_TI_MASK) == 0
    }
}

/// HCR_EL2 TWI (Trap WFI) bit
pub mod hcr_el2 {
    /// HCR_EL2.TWI - Trap WFI instructions
    ///
    /// 0: WFI executed at EL0/EL1 is not trapped
    /// 1: WFI executed at EL0/EL1 is trapped to EL2
    pub const TWI_MASK: u64 = 0x00000010;
    pub const TWI_SHIFT: u64 = 4;

    /// Check if TWI is enabled
    #[inline]
    pub const fn is_tw_enabled(hcr_el2: u64) -> bool {
        (hcr_el2 & TWI_MASK) != 0
    }

    /// Enable TWI
    #[inline]
    pub const fn enable_twi(hcr_el2: u64) -> u64 {
        hcr_el2 | TWI_MASK
    }

    /// Disable TWI
    #[inline]
    pub const fn disable_twi(hcr_el2: u64) -> u64 {
        hcr_el2 & !TWI_MASK
    }
}

/// WFI wait timeout
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WfiTimeout {
    /// Wait indefinitely
    Indefinite,
    /// Wait with timeout in microseconds
    TimeoutUs(u32),
    /// Wait with timeout in milliseconds
    TimeoutMs(u32),
}

impl WfiTimeout {
    /// Create indefinite timeout
    pub const fn indefinite() -> Self {
        Self::Indefinite
    }

    /// Create timeout in microseconds
    pub const fn from_us(us: u32) -> Self {
        Self::TimeoutUs(us)
    }

    /// Create timeout in milliseconds
    pub const fn from_ms(ms: u32) -> Self {
        Self::TimeoutMs(ms)
    }

    /// Check if timeout is indefinite
    pub const fn is_indefinite(&self) -> bool {
        matches!(self, Self::Indefinite)
    }

    /// Get timeout in microseconds (if not indefinite)
    pub fn to_us(&self) -> Option<u32> {
        match self {
            Self::Indefinite => None,
            Self::TimeoutUs(us) => Some(*us),
            Self::TimeoutMs(ms) => Some(ms * 1000),
        }
    }
}

/// WFI wait result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WfiWaitResult {
    /// Wait completed successfully (interrupt/event occurred)
    Success,
    /// Wait timed out
    Timeout,
    /// Wait was interrupted
    Interrupted,
    /// Error occurred
    Error,
}

/// WFI handling mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WfiMode {
    /// Treat WFI as NOP (no operation)
    Nop,
    /// Pass through to hardware WFI
    PassThrough,
    /// Handle in hypervisor (wait for virtual interrupt)
    Handled,
}

/// WFI state for a VCPU
#[derive(Debug, Clone)]
pub struct WfiState {
    /// WFI handling mode
    pub mode: WfiMode,
    /// WFI is currently active
    pub active: bool,
    /// WFI wait timeout
    pub timeout: WfiTimeout,
    /// Number of WFI executions
    pub count: u64,
}

impl Default for WfiState {
    fn default() -> Self {
        Self {
            mode: WfiMode::Handled,
            active: false,
            timeout: WfiTimeout::Indefinite,
            count: 0,
        }
    }
}

impl WfiState {
    /// Create new WFI state
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with specific mode
    pub fn with_mode(mode: WfiMode) -> Self {
        Self {
            mode,
            ..Self::default()
        }
    }

    /// Create with timeout
    pub fn with_timeout(timeout: WfiTimeout) -> Self {
        Self {
            timeout,
            ..Self::default()
        }
    }

    /// Set WFI mode
    pub fn set_mode(&mut self, mode: WfiMode) {
        self.mode = mode;
    }

    /// Set timeout
    pub fn set_timeout(&mut self, timeout: WfiTimeout) {
        self.timeout = timeout;
    }

    /// Check if WFI is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Activate WFI state
    pub fn activate(&mut self) {
        self.active = true;
        self.count += 1;
    }

    /// Deactivate WFI state
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    /// Get WFI execution count
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Reset state
    pub fn reset(&mut self) {
        self.active = false;
        self.count = 0;
    }
}

/// WFI handler
pub struct WfiHandler {
    /// WFI state
    state: WfiState,
}

impl Default for WfiHandler {
    fn default() -> Self {
        Self {
            state: WfiState::default(),
        }
    }
}

impl WfiHandler {
    /// Create new WFI handler
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with specific mode
    pub fn with_mode(mode: WfiMode) -> Self {
        Self {
            state: WfiState::with_mode(mode),
        }
    }

    /// Get WFI state
    pub fn state(&self) -> &WfiState {
        &self.state
    }

    /// Get mutable WFI state
    pub fn state_mut(&mut self) -> &mut WfiState {
        &mut self.state
    }

    /// Handle trapped WFI instruction
    ///
    /// Returns whether to advance PC after handling
    pub fn handle_wfi(&mut self, iss: u32) -> Result<WfiWaitResult, &'static str> {
        // Verify this is WFI (not WFE)
        if iss::is_wfe(iss) {
            return Err("Not a WFI instruction (use WFE handler)");
        }

        log::debug!("WFI Handler: Handling WFI instruction (mode={:?})", self.state.mode);

        match self.state.mode {
            WfiMode::Nop => {
                // Treat as NOP - just advance PC
                log::debug!("WFI Handler: Treating WFI as NOP");
                Ok(WfiWaitResult::Success)
            }
            WfiMode::PassThrough => {
                // Execute actual WFI in hardware
                log::debug!("WFI Handler: Passing through to hardware WFI");
                unsafe { self.execute_hardware_wfi() };
                Ok(WfiWaitResult::Success)
            }
            WfiMode::Handled => {
                // Handle in hypervisor - wait for virtual interrupt
                log::debug!("WFI Handler: Handling WFI in hypervisor");
                self.state.activate();
                // In a real implementation, this would wait for a virtual interrupt
                // For now, just return success
                self.state.deactivate();
                Ok(WfiWaitResult::Success)
            }
        }
    }

    /// Wait for interrupt with timeout
    ///
    /// In a real implementation, this would:
    /// 1. Mark VCPU as waiting
    /// 2. Add VCPU to wait queue
    /// 3. Schedule other VCPUs
    /// 4. Return when interrupt arrives or timeout expires
    pub fn wait_for_interrupt(&mut self, timeout: WfiTimeout) -> WfiWaitResult {
        log::debug!("WFI Handler: Waiting for interrupt (timeout={:?})", timeout);

        self.state.activate();

        // TODO: Implement actual wait logic
        // For now, just return success immediately
        self.state.deactivate();
        WfiWaitResult::Success
    }

    /// Execute hardware WFI instruction
    ///
    /// # Safety
    ///
    /// This function executes the WFI instruction which affects processor state.
    #[inline]
    unsafe fn execute_hardware_wfi(&self) {
        core::arch::asm!("wfi", options(nomem, nostack));
    }

    /// Check if WFI should be trapped based on HCR_EL2.TWI
    ///
    /// Returns true if WFI should trap to EL2
    pub fn should_trap(hcr_el2: u64, exception_level: u8) -> bool {
        // Only trap WFI from EL0/EL1
        if exception_level > 1 {
            return false;
        }

        // Check HCR_EL2.TWI bit
        hcr_el2::is_tw_enabled(hcr_el2)
    }

    /// Configure HCR_EL2.TWI bit
    ///
    /// Returns updated HCR_EL2 value
    pub fn configure_trap(hcr_el2: u64, enable: bool) -> u64 {
        if enable {
            hcr_el2::enable_twi(hcr_el2)
        } else {
            hcr_el2::disable_twi(hcr_el2)
        }
    }

    /// Dump WFI state for debugging
    pub fn dump(&self) {
        log::info!("WFI Handler State:");
        log::info!("  Mode: {:?}", self.state.mode);
        log::info!("  Active: {}", self.state.active);
        log::info!("  Timeout: {:?}", self.state.timeout);
        log::info!("  Count: {}", self.state.count);
    }
}

/// Helper function to check if exception is WFI trap
///
/// Returns true if exception class indicates WFI/WFE trap
pub fn is_wfi_trap(exception_class: ExceptionClass, iss: u32) -> bool {
    match exception_class {
        ExceptionClass::Brk => false,
        ExceptionClass::Hvc => false,
        ExceptionClass::Smc => false,
        ExceptionClass::MsrMrsEl1 => {
            // Check if it's a WFI/WFE trap (ISS bit 0 = 0 for WFI)
            iss::is_wfi(iss)
        }
        _ => false,
    }
}

/// Helper function to handle WFI from exception handler
///
/// This is intended to be called from the top-level exception handler
/// when a WFI trap is detected.
pub fn handle_wfi_trap(handler: &mut WfiHandler, iss: u32) -> Result<bool, &'static str> {
    handler.handle_wfi(iss)?;
    Ok(true) // Always advance PC after WFI
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iss_bits() {
        // WFI has TI bit = 0
        assert!(iss::is_wfi(0x00000000));
        assert!(!iss::is_wfe(0x00000000));

        // WFE has TI bit = 1
        assert!(iss::is_wfe(0x00000001));
        assert!(!iss::is_wfi(0x00000001));
    }

    #[test]
    fn test_hcr_el2_twi() {
        // Enable TWI
        let hcr = 0x00000000u64;
        let hcr = hcr_el2::enable_twi(hcr);
        assert_eq!(hcr, 0x00000010);
        assert!(hcr_el2::is_tw_enabled(hcr));

        // Disable TWI
        let hcr = hcr_el2::disable_twi(hcr);
        assert_eq!(hcr, 0x00000000);
        assert!(!hcr_el2::is_tw_enabled(hcr));
    }

    #[test]
    fn test_wfi_timeout() {
        let timeout = WfiTimeout::indefinite();
        assert!(timeout.is_indefinite());
        assert!(timeout.to_us().is_none());

        let timeout = WfiTimeout::from_us(1000);
        assert!(!timeout.is_indefinite());
        assert_eq!(timeout.to_us(), Some(1000));

        let timeout = WfiTimeout::from_ms(1);
        assert_eq!(timeout.to_us(), Some(1000));
    }

    #[test]
    fn test_wfi_state() {
        let mut state = WfiState::new();
        assert!(!state.is_active());
        assert_eq!(state.count(), 0);

        state.activate();
        assert!(state.is_active());
        assert_eq!(state.count(), 1);

        state.activate();
        assert_eq!(state.count(), 2);

        state.deactivate();
        assert!(!state.is_active());
        assert_eq!(state.count(), 2);

        state.reset();
        assert_eq!(state.count(), 0);
    }

    #[test]
    fn test_wfi_state_mode() {
        let state = WfiState::with_mode(WfiMode::PassThrough);
        assert_eq!(state.mode, WfiMode::PassThrough);

        let state = WfiState::with_timeout(WfiTimeout::from_ms(100));
        assert_eq!(state.timeout.to_us(), Some(100000));
    }

    #[test]
    fn test_wfi_handler() {
        let mut handler = WfiHandler::new();

        // Test NOP mode
        handler.state.set_mode(WfiMode::Nop);
        let result = handler.handle_wfi(0).unwrap();
        assert_eq!(result, WfiWaitResult::Success);

        // Test with WFE ISS (should fail)
        let result = handler.handle_wfi(1);
        assert!(result.is_err());
    }

    #[test]
    fn test_should_trap() {
        // EL2 should not trap
        assert!(!WfiHandler::should_trap(0x10, 2));

        // EL0/EL1 with TWI enabled should trap
        assert!(WfiHandler::should_trap(0x10, 0));
        assert!(WfiHandler::should_trap(0x10, 1));

        // EL0/EL1 with TWI disabled should not trap
        assert!(!WfiHandler::should_trap(0x00, 0));
        assert!(!WfiHandler::should_trap(0x00, 1));
    }

    #[test]
    fn test_configure_trap() {
        let hcr = 0x00000000u64;

        let hcr = WfiHandler::configure_trap(hcr, true);
        assert!(hcr_el2::is_tw_enabled(hcr));

        let hcr = WfiHandler::configure_trap(hcr, false);
        assert!(!hcr_el2::is_tw_enabled(hcr));
    }

    #[test]
    fn test_hardware_wfi() {
        // This test just verifies the function compiles and doesn't crash
        let handler = WfiHandler::new();
        unsafe {
            handler.execute_hardware_wfi();
        }
        // WFI executed successfully
    }
}
