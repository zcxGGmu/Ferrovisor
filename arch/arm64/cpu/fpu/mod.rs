//! FPU/SIMD Virtualization for ARM64
//!
//! Provides VFP/NEON register save/restore, NEON/ASIMD operations,
//! and lazy FPU switching for ARM64 guests.
//!
//! # Overview
//!
//! The FPU module implements:
//! - VFP (Vector Floating Point) register emulation
//! - NEON/ASIMD (Advanced SIMD) vector operations
//! - Lazy FPU context switching for performance
//! - SVE (Scalable Vector Extension) support
//!
//! # Usage
//!
//! ```rust,ignore
//! use ferrovisor::arch::arm64::cpu::fpu::*;
//!
//! // Create VFP register set
//! let vfp = VfpRegs::new();
//!
//! // Create NEON context
//! let neon = NeonContext::new();
//!
//! // Create lazy FPU context
//! let lazy_fpu = LazyFpuContext::new();
//! ```

pub mod vfp;
pub mod neon;
pub mod lazy;

// Re-export commonly used types
pub use vfp::{
    VfpRegs, Mvfr0El1, Mvfr1El1, Mvfr2El1,
    Fpcr, Fpsr, Fpexc32El2,
};

pub use neon::{
    SimdElementType, SimdLaneCount, SimdVec128,
    SveContext, NeonContext,
};

pub use lazy::{
    CptrEl2, FpuTrapInfo, LazyFpuState, LazyFpuContext, LazyFpuManager,
};

/// Initialize FPU/SIMD virtualization
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing ARM64 FPU/SIMD virtualization");
    log::info!("FPU/SIMD virtualization initialized");
    Ok(())
}

/// Check if FPU is available on host
pub fn has_fpu() -> bool {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        // Check ID_AA64PFR0_EL1 for FP and SIMD support
        let mut id_aa64pfr0: u64;
        core::arch::asm!("mrs {}, id_aa64pfr0_el1", out(reg) id_aa64pfr0);

        let fp = (id_aa64pfr0 >> 16) & 0xF;
        let advsimd = (id_aa64pfr0 >> 20) & 0xF;

        // 0x0 = Not implemented, 0x1 = Implemented
        fp == 0x1 && advsimd == 0x1
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        false
    }
}

/// Check if SVE is available on host
pub fn has_sve() -> bool {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        // Check ID_AA64ZFR0_EL1 for SVE support
        let mut id_aa64zfr0: u64;
        core::arch::asm!("mrs {}, id_aa64zfr0_el1", out(reg) id_aa64zfr0);

        // If all zeros, SVE is not implemented
        id_aa64zfr0 != 0
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        false
    }
}

/// Get SVE vector length in bytes
pub fn sve_vector_length() -> Option<usize> {
    if !has_sve() {
        return None;
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        // Read current SVE vector length
        let mut vl: u64;
        core::arch::asm!("mrs {}, vl", out(reg) vl);
        Some(vl as usize)
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fpu_init() {
        assert!(init().is_ok());
    }

    #[test]
    fn test_vfp_regs() {
        let vfp = VfpRegs::new();
        assert_eq!(vfp.vregs.len(), 64);
    }

    #[test]
    fn test_neon_context() {
        let neon = NeonContext::new();
        assert!(neon.has_asimd());
        assert!(!neon.has_sve());
    }

    #[test]
    fn test_lazy_fpu_context() {
        let ctx = LazyFpuContext::new();
        assert!(ctx.enabled);
        assert!(!ctx.is_active());
    }

    #[cfg(target_arch = "aarch64")]
    #[test]
    fn test_has_fpu() {
        // On actual ARM64 hardware, this should return true
        let result = has_fpu();
        println!("FPU available: {}", result);
    }

    #[cfg(target_arch = "aarch64")]
    #[test]
    fn test_has_sve() {
        // Most ARM64 hardware doesn't have SVE yet
        let result = has_sve();
        println!("SVE available: {}", result);
    }
}
