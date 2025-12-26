//! Lazy FPU Switching for ARM64
//!
//! Provides lazy FPU context switching to improve performance.
//! Instead of saving/restoring FPU state on every context switch,
//! we only save/restore when the guest actually uses FPU instructions.
//!
//! Reference: ARM DDI 0487I.a - Chapter D11 - VFP and Advanced SIMD
//!
//! Lazy FPU works by:
//! 1. Setting CPTR_EL2.TFP to trap FP/SIMD instructions
//! 2. On first FP instruction trap, save host FPU state
//! 3. Restore guest FPU state
//! 4. Clear CPTR_EL2.TFP to allow guest FP instructions
//! 5. On context switch back, save guest state and restore host state

use super::{vfp::VfpRegs, neon::NeonContext};

/// CPTR_EL2 bit definitions
/// Architectural Feature Trap Control Register (EL2)
#[derive(Debug, Clone, Copy)]
pub struct CptrEl2 {
    pub raw: u64,
}

impl CptrEl2 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get TFP bit (Trap FP/SIMD at EL1 and EL0)
    pub fn tfp(&self) -> u64 {
        (self.raw >> 10) & 0x3
    }

    /// Set TFP bit
    /// 0 = No trap
    /// 1 = Trap AdvSIMD instructions
    /// 2 = Trap all FP/SIMD instructions
    /// 3 = Trap all FP/SIMD instructions including EL2
    pub fn set_tfp(&mut self, value: u64) {
        self.raw = (self.raw & !(0x3 << 10)) | ((value & 0x3) << 10);
    }

    /// Get TTA bit (Trap Trace System accesses at EL1 and EL0)
    pub fn tta(&self) -> bool {
        (self.raw & (1 << 20)) != 0
    }

    /// Set TTA bit
    pub fn set_tta(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 20;
        } else {
            self.raw &= !(1 << 20);
        }
    }

    /// Create CPTR_EL2 with FP traps enabled
    pub fn with_fp_trap() -> Self {
        // TFP = 0b11 (Trap all FP instructions)
        Self::new(0xC00)
    }

    /// Create CPTR_EL2 without FP traps
    pub fn without_fp_trap() -> Self {
        Self::new(0x000)
    }
}

/// FPU trap information
#[derive(Debug, Clone)]
pub struct FpuTrapInfo {
    /// Exception syndrome
    pub esr: u64,
    /// Fault address
    pub far: u64,
    /// Exception level
    pub el: u8,
    /// Instruction length (0 = 16-bit, 1 = 32-bit)
    pub il: u8,
    /// ISS (Instruction Specific Syndrome)
    pub iss: u32,
}

impl FpuTrapInfo {
    /// Create trap info from ESR_EL2
    pub fn from_esr(esr: u64) -> Self {
        Self {
            esr,
            far: 0,
            el: 2,
            il: ((esr >> 25) & 0x1) as u8,
            iss = (esr & 0x1FFFFFF) as u32,
        }
    }

    /// Check if this is an FP/SIMD trap
    pub fn is_fp_trap(&self) -> bool {
        // EC = 0b000000 (Instruction Abort from lower EL)
        // EC = 0b000111 (SVE/SIMD/FP trap)
        let ec = (self.esr >> 26) & 0x3F;
        ec == 0b000111 || (ec == 0 && self.iss == 0)
    }

    /// Check if this is an SVE trap
    pub fn is_sve_trap(&self) -> bool {
        // ISS bit 14 indicates SVE access
        (self.iss & (1 << 14)) != 0
    }

    /// Check if this is an AdvSIMD trap
    pub fn is_simd_trap(&self) -> bool {
        // ISS bit 13 indicates AdvSIMD access
        (self.iss & (1 << 13)) != 0
    }
}

/// Lazy FPU state
#[derive(Debug, Clone)]
pub enum LazyFpuState {
    /// FPU not used yet
    Clean,
    /// Guest FPU state loaded, traps disabled
    Active,
    /// Guest FPU state modified, needs save
    Dirty,
}

/// Lazy FPU context for VCPU
///
/// Manages lazy FPU switching for a single VCPU.
#[derive(Debug, Clone)]
pub struct LazyFpuContext {
    /// VFP registers
    pub vfp: VfpRegs,
    /// NEON context
    pub neon: super::neon::NeonContext,
    /// Current state
    pub state: LazyFpuState,
    /// FPU enabled for this VCPU
    pub enabled: bool,
    /// CPTR_EL2 value
    pub cptr: CptrEl2,
    /// Last used VCPU ID (for tracking)
    pub last_vcpu_id: Option<u32>,
}

impl Default for LazyFpuContext {
    fn default() -> Self {
        Self {
            vfp: VfpRegs::new(),
            neon: super::neon::NeonContext::new(),
            state: LazyFpuState::Clean,
            enabled: true,
            cptr: CptrEl2::with_fp_trap(),
            last_vcpu_id: None,
        }
    }
}

impl LazyFpuContext {
    /// Create new lazy FPU context
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with FPU disabled
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Check if FPU is active
    pub fn is_active(&self) -> bool {
        matches!(self.state, LazyFpuState::Active) || matches!(self.state, LazyFpuState::Dirty)
    }

    /// Check if FPU state is dirty
    pub fn is_dirty(&self) -> bool {
        matches!(self.state, LazyFpuState::Dirty)
    }

    /// Mark state as dirty
    pub fn mark_dirty(&mut self) {
        if matches!(self.state, LazyFpuState::Active) {
            self.state = LazyFpuState::Dirty;
        }
    }

    /// Enable FPU for this VCPU
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable FPU for this VCPU
    pub fn disable(&mut self) {
        self.enabled = false;
        self.state = LazyFpuState::Clean;
    }

    /// Handle FPU trap
    ///
    /// Called when guest executes FP/SIMD instruction and triggers trap.
    pub fn handle_trap(&mut self, trap_info: &FpuTrapInfo) -> Result<(), &'static str> {
        if !self.enabled {
            return Err("FPU not enabled for this VCPU");
        }

        log::debug!("Lazy FPU: Handling trap (state={:?})", self.state);

        match self.state {
            LazyFpuState::Clean => {
                // First FP instruction - activate FPU
                self.activate()?;
            }
            LazyFpuState::Active | LazyFpuState::Dirty => {
                // FPU already active - shouldn't happen
                log::warn!("Lazy FPU: Trap while active (this shouldn't happen)");
            }
        }

        Ok(())
    }

    /// Activate FPU for this VCPU
    ///
    /// Saves host FPU state, restores guest FPU state, and clears traps.
    pub fn activate(&mut self) -> Result<(), &'static str> {
        if !self.enabled {
            return Err("FPU not enabled");
        }

        log::trace!("Lazy FPU: Activating for VCPU");

        // In real implementation:
        // 1. Save host FPU state
        // 2. Restore guest FPU state
        // 3. Clear CPTR_EL2.TFP
        // 4. Set state to Active

        #[cfg(target_arch = "aarch64")]
        unsafe {
            // Restore guest FPCR and FPSR
            core::arch::asm!("msr fpcr, {}", in(reg) self.vfp.fpcr.raw);
            core::arch::asm!("msr fpsr, {}", in(reg) self.vfp.fpsr.raw);

            // Clear CPTR_EL2.TFP to allow guest FP instructions
            let cptr_val = CptrEl2::without_fp_trap().raw;
            core::arch::asm!("msr cptr_el2, {}", in(reg) cptr_val);
            self.cptr = CptrEl2::without_fp_trap();
        }

        self.state = LazyFpuState::Active;

        Ok(())
    }

    /// Deactivate FPU for this VCPU
    ///
    /// Saves guest FPU state, restores host FPU state, and enables traps.
    pub fn deactivate(&mut self) -> Result<(), &'static str> {
        if !self.enabled {
            return Err("FPU not enabled");
        }

        log::trace!("Lazy FPU: Deactivating for VCPU");

        // Only save if state is dirty or active
        if matches!(self.state, LazyFpuState::Dirty | LazyFpuState::Active) {
            // Save guest FPU state
            self.save();
        }

        // Set CPTR_EL2.TFP to enable traps
        #[cfg(target_arch = "aarch64")]
        unsafe {
            let cptr_val = CptrEl2::with_fp_trap().raw;
            core::arch::asm!("msr cptr_el2, {}", in(reg) cptr_val);
            self.cptr = CptrEl2::with_fp_trap();
        }

        self.state = LazyFpuState::Clean;

        Ok(())
    }

    /// Save FPU state
    pub fn save(&mut self) {
        log::trace!("Lazy FPU: Saving state");

        #[cfg(target_arch = "aarch64")]
        unsafe {
            // Read current FPCR and FPSR
            let mut fpcr: u32;
            let mut fpsr: u32;
            core::arch::asm!("mrs {}, fpcr", out(reg) fpcr);
            core::arch::asm!("mrs {}, fpsr", out(reg) fpsr);

            self.vfp.fpcr.raw = fpcr;
            self.vfp.fpsr.raw = fpsr;
        }

        self.state = LazyFpuState::Clean;
    }

    /// Restore FPU state
    pub fn restore(&self) {
        log::trace!("Lazy FPU: Restoring state");

        #[cfg(target_arch = "aarch64")]
        unsafe {
            // Restore FPCR and FPSR
            core::arch::asm!("msr fpcr, {}", in(reg) self.vfp.fpcr.raw);
            core::arch::asm!("msr fpsr, {}", in(reg) self.vfp.fpsr.raw);
        }
    }

    /// Reset FPU context
    pub fn reset(&mut self) {
        self.vfp = VfpRegs::new();
        self.neon = super::neon::NeonContext::new();
        self.state = LazyFpuState::Clean;
        self.cptr = CptrEl2::with_fp_trap();
    }

    /// Get current CPTR_EL2 value
    pub fn cptr_el2(&self) -> u64 {
        self.cptr.raw
    }

    /// Dump FPU state for debugging
    pub fn dump(&self) {
        log::info!("Lazy FPU Context:");
        log::info!("  Enabled: {}", self.enabled);
        log::info!("  State: {:?}", self.state);
        log::info!("  CPTR_EL2: 0x{:03x}", self.cptr.raw);
        self.vfp.dump();
    }
}

/// Global lazy FPU manager
///
/// Tracks which VCPU currently owns the FPU.
#[derive(Debug)]
pub struct LazyFpuManager {
    /// Current active VCPU ID
    current_vcpu: Option<u32>,
    /// Host FPU state (saved when first VCPU activates)
    host_state: Option<VfpRegs>,
}

impl Default for LazyFpuManager {
    fn default() -> Self {
        Self {
            current_vcpu: None,
            host_state: None,
        }
    }
}

impl LazyFpuManager {
    /// Create new lazy FPU manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Switch to VCPU
    ///
    /// Called during VCPU context switch.
    pub fn switch_to(&mut self, vcpu_id: u32, ctx: &mut LazyFpuContext) -> Result<(), &'static str> {
        // If switching to same VCPU, nothing to do
        if self.current_vcpu == Some(vcpu_id) {
            return Ok(());
        }

        log::trace!("Lazy FPU: Switching to VCPU {}", vcpu_id);

        // Save previous VCPU state if active
        if let Some(prev_id) = self.current_vcpu {
            if prev_id != vcpu_id {
                // In real implementation, we'd save the previous VCPU's state here
                log::trace!("Lazy FPU: Saving VCPU {} state", prev_id);
            }
        }

        // Mark new VCPU as current
        self.current_vcpu = Some(vcpu_id);
        ctx.last_vcpu_id = Some(vcpu_id);

        // Ensure traps are enabled for lazy switching
        ctx.cptr = CptrEl2::with_fp_trap();
        ctx.state = LazyFpuState::Clean;

        #[cfg(target_arch = "aarch64")]
        unsafe {
            // Set CPTR_EL2.TFP to enable traps
            core::arch::asm!("msr cptr_el2, {}", in(reg) ctx.cptr.raw);
        }

        Ok(())
    }

    /// Save host FPU state
    ///
    /// Called when first VCPU activates FPU.
    pub fn save_host(&mut self) {
        if self.host_state.is_none() {
            log::trace!("Lazy FPU: Saving host state");
            self.host_state = Some(VfpRegs::from_hw());
        }
    }

    /// Restore host FPU state
    ///
    /// Called when exiting to host.
    pub fn restore_host(&self) {
        if let Some(host_state) = &self.host_state {
            log::trace!("Lazy FPU: Restoring host state");
            host_state.restore();
        }
    }

    /// Reset manager (clear current VCPU)
    pub fn reset(&mut self) {
        self.current_vcpu = None;
    }

    /// Get current VCPU ID
    pub fn current_vcpu(&self) -> Option<u32> {
        self.current_vcpu
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cptr_el2_tfp() {
        let mut cptr = CptrEl2::new(0);
        assert_eq!(cptr.tfp(), 0);

        cptr.set_tfp(0b11);
        assert_eq!(cptr.tfp(), 0b11);
    }

    #[test]
    fn test_cptr_el2_with_fp_trap() {
        let cptr = CptrEl2::with_fp_trap();
        assert_eq!(cptr.tfp(), 0b11);
    }

    #[test]
    fn test_cptr_el2_without_fp_trap() {
        let cptr = CptrEl2::without_fp_trap();
        assert_eq!(cptr.tfp(), 0);
    }

    #[test]
    fn test_fpu_trap_info() {
        let esr = (0b000111u64 << 26) | 0x1; // FP trap
        let trap = FpuTrapInfo::from_esr(esr);
        assert!(trap.is_fp_trap());
    }

    #[test]
    fn test_lazy_fpu_context_create() {
        let ctx = LazyFpuContext::new();
        assert!(ctx.enabled);
        assert!(!ctx.is_active());
        assert_eq!(ctx.cptr.tfp(), 0b11);
    }

    #[test]
    fn test_lazy_fpu_context_disabled() {
        let ctx = LazyFpuContext::disabled();
        assert!(!ctx.enabled);
    }

    #[test]
    fn test_lazy_fpu_activate() {
        let mut ctx = LazyFpuContext::new();
        assert!(!ctx.is_active());

        // Note: activate() will fail on non-aarch64 platforms
        #[cfg(target_arch = "aarch64")]
        {
            let _ = ctx.activate();
            assert!(ctx.is_active());
        }
    }

    #[test]
    fn test_lazy_fpu_deactivate() {
        let mut ctx = LazyFpuContext::new();
        ctx.state = LazyFpuState::Active;

        // Note: deactivate() will fail on non-aarch64 platforms
        #[cfg(target_arch = "aarch64")]
        {
            let _ = ctx.deactivate();
            assert!(!ctx.is_active());
        }
    }

    #[test]
    fn test_lazy_fpu_mark_dirty() {
        let mut ctx = LazyFpuContext::new();
        ctx.state = LazyFpuState::Active;

        ctx.mark_dirty();
        assert!(ctx.is_dirty());
    }

    #[test]
    fn test_lazy_fpu_manager_create() {
        let manager = LazyFpuManager::new();
        assert!(manager.current_vcpu().is_none());
    }

    #[test]
    fn test_lazy_fpu_manager_switch() {
        let mut manager = LazyFpuManager::new();
        let mut ctx = LazyFpuContext::new();

        let _ = manager.switch_to(1, &mut ctx);
        assert_eq!(manager.current_vcpu(), Some(1));

        // Switch to same VCPU - should be no-op
        let _ = manager.switch_to(1, &mut ctx);
        assert_eq!(manager.current_vcpu(), Some(1));
    }

    #[test]
    fn test_lazy_fpu_reset() {
        let mut ctx = LazyFpuContext::new();
        ctx.state = LazyFpuState::Dirty;

        ctx.reset();
        assert!(matches!(ctx.state, LazyFpuState::Clean));
    }
}
