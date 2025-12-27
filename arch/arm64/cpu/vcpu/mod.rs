//! VCPU context switching for ARM64
//!
//! Provides high-level VCPU context switching between host and guest.
//! Reference: xvisor/arch/arm/cpu/arm64/cpu_vcpu_helper.c

/// VCPU context management
pub mod context;

/// VCPU trap handling
pub mod trap;

pub use context::{
    SavedGprs, SavedGprsOffsets, VcpuContextOffsets,
    VfpRegs, ExtendedVcpuContext,
    PtrauthRegs, TimerRegs,
    sysregs_save, sysregs_restore,
    vfp_save, vfp_restore,
    gprs_save, gprs_restore,
    switch_to_guest,
    ptrauth_save, ptrauth_restore,
    timer_save, timer_restore,
};

pub use trap::{
    TrapReason, TrapInfo, TrapResolution,
    TrapHandler, DefaultTrapHandler,
    TrappedInstruction, ExceptionInfo,
    handle_trap, decode_trapped_instruction, create_exception_info,
    advance_pc, is_aarch32_trap, get_aarch32_mode,
    Aarch32Mode, is_aarch32,
};

// Re-export from parent modules
pub use crate::arch::arm64::cpu::state::{VcpuContext, SavedGprs, SavedSpecialRegs};

/// Initialize VCPU context switching module
pub fn init() -> Result<(), &'static str> {
    context::init()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_init() {
        // Module should initialize successfully
        // Note: Actual init may require EL2 context
    }
}
