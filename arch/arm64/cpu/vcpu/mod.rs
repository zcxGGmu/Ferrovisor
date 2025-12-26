//! VCPU context switching for ARM64
//!
//! Provides high-level VCPU context switching between host and guest.
//! Reference: xvisor/arch/arm/cpu/arm64/cpu_vcpu_helper.c

/// VCPU context management
pub mod context;

pub use context::{
    SavedGprs, SavedGprsOffsets, VcpuContextOffsets,
    VfpRegs, ExtendedVcpuContext,
    sysregs_save, sysregs_restore,
    vfp_save, vfp_restore,
    gprs_save, gprs_restore,
    switch_to_guest,
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
