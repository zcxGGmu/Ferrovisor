//! Coprocessor emulation for ARMv7/ARMv8-AArch32
//!
//! This module provides coprocessor emulation for ARM32 guest operating systems.
//! It implements CP14 (debug coprocessor) and CP15 (system control coprocessor).

pub mod cp15;
pub mod cp14;

// Re-export commonly used types
pub use cp15::{
    ArmCpuId, Cp15Encoding, Cp15Regs,
    Cp15IdRegs, Cp15CtrlRegs, Cp15MmuRegs, Cp15FaultRegs,
    Cp15TranslateRegs, Cp15PerfRegs, Cp15AttrRegs, Cp15TlsRegs,
};

pub use cp14::{Cp14Regs, Cp14ThumbEERegs, Cp14RegType, ARM_FEATURE_THUMB2EE, ArmFeatureExt};

/// Initialize coprocessor emulation
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing ARM32 coprocessor emulation");
    log::info!("ARM32 coprocessor emulation initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cp15_encoding() {
        let enc = Cp15Encoding::new(0, 0, 1, 0);
        assert_eq!(enc.opc1, 0);
        assert_eq!(enc.opc2, 0);
        assert_eq!(enc.crn, 1);
        assert_eq!(enc.crm, 0);
    }

    #[test]
    fn test_arm_cpu_id() {
        let id = ArmCpuId::CortexA9;
        assert!(id.is_cortex());
        assert_eq!(id.cortex_version(), 9);
    }
}
