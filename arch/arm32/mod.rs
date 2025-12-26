//! ARM32 Architecture Module
//!
//! This module provides ARMv7/ARMv8-AArch32 architecture support for running
//! 32-bit ARM guest operating systems on ARM64 hosts.
//!
//! # Overview
//!
//! The ARM32 module implements:
//! - CP15 coprocessor emulation (system control, MMU, cache operations)
//! - CP14 coprocessor emulation (debug registers)
//! - Banked register switching for different ARM modes
//! - VFP/NEON context management
//!
//! # Usage
//!
//! ```rust,ignore
//! use ferrovisor::arch::arm32;
//!
//! // Initialize ARM32 support
//! arm32::init().unwrap();
//!
//! // Create CP15 registers for a Cortex-A9 VCPU
//! let cp15_regs = arm32::Cp15Regs::for_cpu(arm32::ArmCpuId::CortexA9, 0);
//! ```

pub mod cpu;

// Re-export commonly used types
pub use cpu::{coproc, ArmCpuId, Cp15Regs, Cp15Encoding};

/// Initialize ARM32 architecture support
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing ARM32 architecture support");
    cpu::init()?;
    log::info!("ARM32 architecture support initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arm32_arch_init() {
        assert!(init().is_ok());
    }

    #[test]
    fn test_arm32_cp15_regs() {
        let regs = Cp15Regs::for_cpu(ArmCpuId::CortexA9, 0);
        // Verify MIDR is set correctly
        assert_ne!(regs.id.midr, 0);
    }
}
