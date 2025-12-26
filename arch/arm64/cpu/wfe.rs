//! WFE (Wait For Event) Instruction Handling for ARM64
//!
//! Provides WFE instruction trap handling for ARM64 virtualization.
//!
//! WFE is a hint instruction that suggests the processor is waiting
//! for an event from another processor.
//!
//! ## WFE Behavior
//!
//! - WFE causes the processor to enter a low-power state until:
//!   - An event occurs (SEV instruction executed by another CPU)
//!   - An interrupt is pending
//!   - A debug exception is pending
//!
//! - The event mechanism is typically implemented using:
//!   - Monitors (Load-Exclusive / Store-Exclusive pairs)
//!   - Event registers (per-CPU event flags)
//!
//! - In virtualization context, trapped WFE can be:
//!   - Handled by hypervisor (yield scheduler)
//!   - Passed through to hardware
//!
//! ## References
//! - [ARM Architecture Reference Manual ARMv8-A](https://developer.arm.com/documentation/ddi0487/latest)
//! - [Xvisor WFE Implementation](https://github.com/xvisor/xvisor)

use crate::arch::arm64::ExceptionClass;
use super::wfi::iss; // Re-use ISS definitions from WFI module

/// HCR_EL2 TWE (Trap WFE) bit
pub mod hcr_el2 {
    /// HCR_EL2.TWE - Trap WFE instructions
    ///
    /// 0: WFE executed at EL0/EL1 is not trapped
    /// 1: WFE executed at EL0/EL1 is trapped to EL2
    pub const TWE_MASK: u64 = 0x00000400;
    pub const TWE_SHIFT: u64 = 10;

    /// Check if TWE is enabled
    #[inline]
    pub const fn is_twe_enabled(hcr_el2: u64) -> bool {
        (hcr_el2 & TWE_MASK) != 0
    }

    /// Enable TWE
    #[inline]
    pub const fn enable_twe(hcr_el2: u64) -> u64 {
        hcr_el2 | TWE_MASK
    }

    /// Disable TWE
    #[inline]
    pub const fn disable_twe(hcr_el2: u64) -> u64 {
        hcr_el2 & !TWE_MASK
    }
}

/// Event register state (per-CPU)
///
/// Each CPU has an event register that is set by SEV and cleared by WFE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventRegister(u8);

impl EventRegister {
    /// Create new event register (cleared)
    pub const fn new() -> Self {
        Self(0)
    }

    /// Check if event is pending
    pub const fn is_pending(&self) -> bool {
        self.0 != 0
    }

    /// Set event (SEV instruction)
    pub fn set(&mut self) {
        self.0 = 1;
    }

    /// Clear event (WFE instruction)
    pub fn clear(&mut self) {
        self.0 = 0;
    }

    /// Get raw value
    pub const fn raw(&self) -> u8 {
        self.0
    }
}

impl Default for EventRegister {
    fn default() -> Self {
        Self::new()
    }
}

/// WFE handling mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WfeMode {
    /// Treat WFE as NOP (no operation)
    Nop,
    /// Pass through to hardware WFE
    PassThrough,
    /// Yield to scheduler
    Yield,
}

/// WFE action result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WfeActionResult {
    /// Event was already pending, no wait occurred
    EventPending,
    /// Entered wait state
    EnteredWait,
    /// Wait completed (event received)
    EventReceived,
    /// Yielded to scheduler
    Yielded,
    /// No action taken
    None,
}

/// WFE state for a VCPU
#[derive(Debug, Clone)]
pub struct WfeState {
    /// WFE handling mode
    pub mode: WfeMode,
    /// Local event register
    pub event: EventRegister,
    /// Number of WFE executions
    pub count: u64,
    /// Number of SEV executions
    pub sev_count: u64,
    /// Number of yields performed
    pub yield_count: u64,
}

impl Default for WfeState {
    fn default() -> Self {
        Self {
            mode: WfeMode::Yield,
            event: EventRegister::new(),
            count: 0,
            sev_count: 0,
            yield_count: 0,
        }
    }
}

impl WfeState {
    /// Create new WFE state
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with specific mode
    pub fn with_mode(mode: WfeMode) -> Self {
        Self {
            mode,
            ..Self::default()
        }
    }

    /// Set WFE mode
    pub fn set_mode(&mut self, mode: WfeMode) {
        self.mode = mode;
    }

    /// Check if event is pending
    pub fn is_event_pending(&self) -> bool {
        self.event.is_pending()
    }

    /// Set event (SEV instruction)
    pub fn send_event(&mut self) {
        self.event.set();
        self.sev_count += 1;
    }

    /// Clear event
    pub fn clear_event(&mut self) {
        self.event.clear();
    }

    /// Get WFI execution count
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Get SEV execution count
    pub fn sev_count(&self) -> u64 {
        self.sev_count
    }

    /// Get yield count
    pub fn yield_count(&self) -> u64 {
        self.yield_count
    }

    /// Increment WFE count
    pub fn increment_count(&mut self) {
        self.count += 1;
    }

    /// Increment yield count
    pub fn increment_yield(&mut self) {
        self.yield_count += 1;
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.count = 0;
        self.sev_count = 0;
        self.yield_count = 0;
    }
}

/// Global event broadcaster for SEV (Send Event)
///
/// In a multi-VCPU system, SEV needs to wake up other waiting VCPUs.
pub struct EventBroadcaster {
    /// Bitmap of CPUs with pending events
    events: alloc::collections::BTreeMap<u32, bool>,
    /// Maximum number of CPUs
    max_cpus: usize,
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self {
            events: alloc::collections::BTreeMap::new(),
            max_cpus: 256,
        }
    }
}

impl EventBroadcaster {
    /// Create new event broadcaster
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with maximum CPU count
    pub fn with_max_cpus(max: usize) -> Self {
        Self {
            events: alloc::collections::BTreeMap::new(),
            max_cpus: max,
        }
    }

    /// Send event to specific CPU
    pub fn send_event(&mut self, cpu_id: u32) {
        if (cpu_id as usize) < self.max_cpus {
            self.events.insert(cpu_id, true);
            log::debug!("Event Broadcaster: Event sent to CPU {}", cpu_id);
        }
    }

    /// Send event to all CPUs
    pub fn send_event_all(&mut self) {
        for cpu_id in 0..self.max_cpus {
            self.events.insert(cpu_id as u32, true);
        }
        log::debug!("Event Broadcaster: Event sent to all CPUs");
    }

    /// Check if CPU has pending event
    pub fn has_event(&self, cpu_id: u32) -> bool {
        self.events.get(&cpu_id).copied().unwrap_or(false)
    }

    /// Clear event for CPU
    pub fn clear_event(&mut self, cpu_id: u32) {
        self.events.insert(cpu_id, false);
    }

    /// Get all CPUs with pending events
    pub fn pending_events(&self) -> alloc::vec::Vec<u32> {
        self.events
            .iter()
            .filter(|(_, &pending)| pending)
            .map(|(&cpu_id, _)| cpu_id)
            .collect()
    }
}

/// WFE handler
pub struct WfeHandler {
    /// WFE state
    state: WfeState,
    /// Event broadcaster (for multi-VCPU systems)
    broadcaster: EventBroadcaster,
}

impl Default for WfeHandler {
    fn default() -> Self {
        Self {
            state: WfeState::default(),
            broadcaster: EventBroadcaster::new(),
        }
    }
}

impl WfeHandler {
    /// Create new WFE handler
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with specific mode
    pub fn with_mode(mode: WfeMode) -> Self {
        Self {
            state: WfeState::with_mode(mode),
            ..Self::default()
        }
    }

    /// Get WFE state
    pub fn state(&self) -> &WfeState {
        &self.state
    }

    /// Get mutable WFE state
    pub fn state_mut(&mut self) -> &mut WfeState {
        &mut self.state
    }

    /// Get event broadcaster
    pub fn broadcaster(&self) -> &EventBroadcaster {
        &self.broadcaster
    }

    /// Get mutable event broadcaster
    pub fn broadcaster_mut(&mut self) -> &mut EventBroadcaster {
        &mut self.broadcaster
    }

    /// Handle trapped WFE instruction
    ///
    /// Returns the action taken
    pub fn handle_wfe(&mut self, iss: u32, cpu_id: u32) -> Result<WfeActionResult, &'static str> {
        // Verify this is WFE (not WFI)
        if !iss::is_wfe(iss) {
            return Err("Not a WFE instruction (use WFI handler)");
        }

        log::debug!("WFE Handler: Handling WFE instruction (mode={:?})", self.state.mode);

        self.state.increment_count();

        // Check if we have a pending event from broadcaster
        if self.broadcaster.has_event(cpu_id) {
            log::debug!("WFE Handler: Event pending, clearing and returning");
            self.broadcaster.clear_event(cpu_id);
            return Ok(WfeActionResult::EventPending);
        }

        match self.state.mode {
            WfeMode::Nop => {
                // Treat as NOP - just advance PC
                log::debug!("WFE Handler: Treating WFE as NOP");
                Ok(WfeActionResult::None)
            }
            WfeMode::PassThrough => {
                // Execute actual WFE in hardware
                log::debug!("WFE Handler: Passing through to hardware WFE");
                unsafe { self.execute_hardware_wfe() };
                Ok(WfeActionResult::EnteredWait)
            }
            WfeMode::Yield => {
                // Yield to scheduler
                log::debug!("WFE Handler: Yielding to scheduler");
                self.state.increment_yield();
                self.yield_scheduler();
                Ok(WfeActionResult::Yielded)
            }
        }
    }

    /// Handle SEV (Send Event) instruction
    ///
    /// This sends an event to other waiting CPUs/VCPUs
    pub fn handle_sev(&mut self, target_cpu: Option<u32>) {
        match target_cpu {
            Some(cpu_id) => {
                // Send event to specific CPU
                self.broadcaster.send_event(cpu_id);
            }
            None => {
                // Send event to all CPUs
                self.broadcaster.send_event_all();
            }
        }

        self.state.send_event();
        log::debug!("WFE Handler: SEV executed (target={:?})", target_cpu);
    }

    /// Handle SEVL (Send Event Local) instruction
    ///
    /// This sends an event only to the local CPU
    pub fn handle_sevl(&mut self) {
        self.state.send_event();
        log::debug!("WFE Handler: SEVL executed");
    }

    /// Execute hardware WFE instruction
    ///
    /// # Safety
    ///
    /// This function executes the WFE instruction which affects processor state.
    #[inline]
    unsafe fn execute_hardware_wfe(&self) {
        core::arch::asm!("wfe", options(nomem, nostack));
    }

    /// Execute hardware SEV instruction
    ///
    /// # Safety
    ///
    /// This function executes the SEV instruction which affects other processors.
    #[inline]
    unsafe fn execute_hardware_sev(&self) {
        core::arch::asm!("sev", options(nostack));
    }

    /// Execute hardware SEVL instruction
    ///
    /// # Safety
    ///
    /// This function executes the SEVL instruction which sets local event.
    #[inline]
    unsafe fn execute_hardware_sevl(&self) {
        core::arch::asm!("sevl", options(nostack));
    }

    /// Yield to scheduler
    ///
    /// This is called when WFE is configured to yield instead of waiting.
    fn yield_scheduler(&self) {
        // TODO: Implement actual scheduler yield
        // For now, this is a placeholder
        log::debug!("WFE Handler: Scheduler yield (placeholder)");
    }

    /// Check if WFE should be trapped based on HCR_EL2.TWE
    ///
    /// Returns true if WFE should trap to EL2
    pub fn should_trap(hcr_el2: u64, exception_level: u8) -> bool {
        // Only trap WFE from EL0/EL1
        if exception_level > 1 {
            return false;
        }

        // Check HCR_EL2.TWE bit
        hcr_el2::is_twe_enabled(hcr_el2)
    }

    /// Configure HCR_EL2.TWE bit
    ///
    /// Returns updated HCR_EL2 value
    pub fn configure_trap(hcr_el2: u64, enable: bool) -> u64 {
        if enable {
            hcr_el2::enable_twe(hcr_el2)
        } else {
            hcr_el2::disable_twe(hcr_el2)
        }
    }

    /// Dump WFE state for debugging
    pub fn dump(&self) {
        log::info!("WFE Handler State:");
        log::info!("  Mode: {:?}", self.state.mode);
        log::info!("  Event Pending: {}", self.state.is_event_pending());
        log::info!("  WFE Count: {}", self.state.count());
        log::info!("  SEV Count: {}", self.state.sev_count());
        log::info!("  Yield Count: {}", self.state.yield_count());
        log::info!("  Pending Events: {:?}", self.broadcaster.pending_events());
    }
}

/// Helper function to check if exception is WFE trap
///
/// Returns true if exception class indicates WFI/WFE trap with WFE bit set
pub fn is_wfe_trap(exception_class: ExceptionClass, iss: u32) -> bool {
    match exception_class {
        ExceptionClass::Brk => false,
        ExceptionClass::Hvc => false,
        ExceptionClass::Smc => false,
        ExceptionClass::MsrMrsEl1 => {
            // Check if it's a WFI/WFE trap (ISS bit 0 = 1 for WFE)
            iss::is_wfe(iss)
        }
        _ => false,
    }
}

/// Helper function to handle WFE from exception handler
///
/// This is intended to be called from the top-level exception handler
/// when a WFE trap is detected.
pub fn handle_wfe_trap(
    handler: &mut WfeHandler,
    iss: u32,
    cpu_id: u32,
) -> Result<bool, &'static str> {
    handler.handle_wfe(iss, cpu_id)?;
    Ok(true) // Always advance PC after WFE
}

/// Helper function to handle SEV instruction
///
/// This is intended to be called when a SEV instruction is trapped.
pub fn handle_sev_trap(handler: &mut WfeHandler, target_cpu: Option<u32>) -> Result<(), &'static str> {
    handler.handle_sev(target_cpu);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_register() {
        let mut event = EventRegister::new();
        assert!(!event.is_pending());

        event.set();
        assert!(event.is_pending());

        event.clear();
        assert!(!event.is_pending());
    }

    #[test]
    fn test_hcr_el2_twe() {
        // Enable TWE
        let hcr = 0x00000000u64;
        let hcr = hcr_el2::enable_twe(hcr);
        assert_eq!(hcr, 0x00000400);
        assert!(hcr_el2::is_twe_enabled(hcr));

        // Disable TWE
        let hcr = hcr_el2::disable_twe(hcr);
        assert_eq!(hcr, 0x00000000);
        assert!(!hcr_el2::is_twe_enabled(hcr));
    }

    #[test]
    fn test_wfe_state() {
        let mut state = WfeState::new();
        assert_eq!(state.count(), 0);
        assert!(!state.is_event_pending());

        state.increment_count();
        assert_eq!(state.count(), 1);

        state.send_event();
        assert!(state.is_event_pending());
        assert_eq!(state.sev_count(), 1);
    }

    #[test]
    fn test_event_broadcaster() {
        let mut broadcaster = EventBroadcaster::with_max_cpus(4);

        // Send event to CPU 0
        broadcaster.send_event(0);
        assert!(broadcaster.has_event(0));
        assert!(!broadcaster.has_event(1));

        // Send event to all
        broadcaster.send_event_all();
        assert!(broadcaster.has_event(0));
        assert!(broadcaster.has_event(1));
        assert!(broadcaster.has_event(2));
        assert!(broadcaster.has_event(3));

        // Clear event for CPU 0
        broadcaster.clear_event(0);
        assert!(!broadcaster.has_event(0));
        assert!(broadcaster.has_event(1));
    }

    #[test]
    fn test_wfe_handler() {
        let mut handler = WfeHandler::new();

        // Test NOP mode
        handler.state.set_mode(WfeMode::Nop);
        let result = handler.handle_wfe(1, 0).unwrap();
        assert_eq!(result, WfeActionResult::None);

        // Test with WFI ISS (should fail)
        let result = handler.handle_wfe(0, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_wfe_handler_with_pending_event() {
        let mut handler = WfeHandler::new();
        handler.broadcaster.send_event(0);

        let result = handler.handle_wfe(1, 0).unwrap();
        assert_eq!(result, WfeActionResult::EventPending);
    }

    #[test]
    fn test_sev_handling() {
        let mut handler = WfeHandler::new();

        // SEV to specific CPU
        handler.handle_sev(Some(1));
        assert!(handler.broadcaster.has_event(1));

        // SEV to all CPUs
        handler.handle_sev(None);
        assert!(handler.broadcaster.has_event(0));
        assert!(handler.broadcaster.has_event(1));
    }

    #[test]
    fn test_sevl_handling() {
        let mut handler = WfeHandler::new();
        handler.handle_sevl();
        assert!(handler.state.is_event_pending());
    }

    #[test]
    fn test_should_trap() {
        // EL2 should not trap
        assert!(!WfeHandler::should_trap(0x400, 2));

        // EL0/EL1 with TWE enabled should trap
        assert!(WfeHandler::should_trap(0x400, 0));
        assert!(WfeHandler::should_trap(0x400, 1));

        // EL0/EL1 with TWE disabled should not trap
        assert!(!WfeHandler::should_trap(0x000, 0));
        assert!(!WfeHandler::should_trap(0x000, 1));
    }

    #[test]
    fn test_configure_trap() {
        let hcr = 0x00000000u64;

        let hcr = WfeHandler::configure_trap(hcr, true);
        assert!(hcr_el2::is_twe_enabled(hcr));

        let hcr = WfeHandler::configure_trap(hcr, false);
        assert!(!hcr_el2::is_twe_enabled(hcr));
    }

    #[test]
    fn test_hardware_instructions() {
        let handler = WfeHandler::new();

        // These tests verify the functions compile and don't crash
        unsafe {
            handler.execute_hardware_wfe();
            handler.execute_hardware_sev();
            handler.execute_hardware_sevl();
        }
        // Instructions executed successfully
    }

    #[test]
    fn test_is_wfe_trap() {
        // WFE has ISS bit 0 set
        assert!(is_wfe_trap(ExceptionClass::MsrMrsEl1, 0x00000001));
        assert!(!is_wfe_trap(ExceptionClass::MsrMrsEl1, 0x00000000));

        // Other exception classes are not WFE traps
        assert!(!is_wfe_trap(ExceptionClass::Brk, 0x00000001));
        assert!(!is_wfe_trap(ExceptionClass::Hvc, 0x00000001));
    }
}
