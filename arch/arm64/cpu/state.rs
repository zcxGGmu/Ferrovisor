//! CPU state management for ARM64
//!
//! Provides structures for managing CPU state including VCPU context.

use crate::ExceptionLevel;

/// Saved general-purpose registers
#[derive(Debug, Clone, Copy)]
pub struct SavedGprs {
    /// X0-X28 (29 registers)
    pub x: [u64; 29],
    /// X29 (Frame pointer)
    pub fp: u64,
    /// X30 (Link register)
    pub lr: u64,
}

impl Default for SavedGprs {
    fn default() -> Self {
        Self {
            x: [0; 29],
            fp: 0,
            lr: 0,
        }
    }
}

/// Saved special registers
#[derive(Debug, Clone, Copy)]
pub struct SavedSpecialRegs {
    /// Stack pointer (SP)
    pub sp: u64,
    /// Program counter (PC)
    pub pc: u64,
    /// Processor state (PSTATE)
    pub pstate: u64,
}

impl Default for SavedSpecialRegs {
    fn default() -> Self {
        Self {
            sp: 0,
            pc: 0,
            pstate: 0x3C5, // Default PSTATE (EL2h, IRQ/FIQ masked)
        }
    }
}

/// EL1 system registers (for VCPU)
#[derive(Debug, Clone, Copy)]
pub struct SavedEl1SysRegs {
    /// SP_EL0
    pub sp_el0: u64,
    /// SP_EL1
    pub sp_el1: u64,
    /// ELR_EL1 (Exception Link Register)
    pub elr_el1: u64,
    /// SPSR_EL1 (Saved Processor Status Register)
    pub spsr_el1: u64,
    /// SCTLR_EL1 (System Control Register)
    pub sctlr_el1: u64,
    /// ACTLR_EL1 (Auxiliary Control Register)
    pub actlr_el1: u64,
    /// CPACR_EL1 (Coprocessor Access Control)
    pub cpacr_el1: u64,
    /// TTBR0_EL1 (Translation Table Base Register 0)
    pub ttbr0_el1: u64,
    /// TTBR1_EL1 (Translation Table Base Register 1)
    pub ttbr1_el1: u64,
    /// TCR_EL1 (Translation Control Register)
    pub tcr_el1: u64,
    /// ESR_EL1 (Exception Syndrome Register)
    pub esr_el1: u64,
    /// FAR_EL1 (Fault Address Register)
    pub far_el1: u64,
    /// PAR_EL1 (Physical Address Register)
    pub par_el1: u64,
    /// MAIR_EL1 (Memory Attribute Indirection Register)
    pub mair_el1: u64,
    /// AMAIR_EL1 (Auxiliary Memory Attribute Register)
    pub amair_el1: u64,
    /// VBAR_EL1 (Vector Base Address Register)
    pub vbar_el1: u64,
    /// CONTEXTIDR_EL1 (Context ID Register)
    pub contextidr_el1: u64,
    /// TPIDR_EL0 (Thread ID Register User)
    pub tpidr_el0: u64,
    /// TPIDR_EL1 (Thread ID Register Privileged)
    pub tpidr_el1: u64,
    /// TPIDRRO_EL0 (Thread ID Register Read-Only User)
    pub tpidrro_el0: u64,
}

impl Default for SavedEl1SysRegs {
    fn default() -> Self {
        Self {
            sp_el0: 0,
            sp_el1: 0,
            elr_el1: 0,
            spsr_el1: 0,
            sctlr_el1: 0x30C50830, // Default SCTLR_EL1 value
            actlr_el1: 0,
            cpacr_el1: 0,
            ttbr0_el1: 0,
            ttbr1_el1: 0,
            tcr_el1: 0,
            esr_el1: 0,
            far_el1: 0,
            par_el1: 0,
            mair_el1: 0,
            amair_el1: 0,
            vbar_el1: 0,
            contextidr_el1: 0,
            tpidr_el0: 0,
            tpidr_el1: 0,
            tpidrro_el0: 0,
        }
    }
}

/// Saved VFP/SIMD registers
#[derive(Debug, Clone, Copy)]
pub struct SavedVfpRegs {
    /// FPSR (Floating-point Status Register)
    pub fpsr: u32,
    /// FPCR (Floating-point Control Register)
    pub fpcr: u32,
    /// MVFR0 (Media and VFP Feature Register 0)
    pub mvfr0: u32,
    /// MVFR1 (Media and VFP Feature Register 1)
    pub mvfr1: u32,
    /// MVFR2 (Media and VFP Feature Register 2)
    pub mvfr2: u32,
    /// V registers (V0-V31, 128-bit each)
    pub v: [u128; 32],
}

impl Default for SavedVfpRegs {
    fn default() -> Self {
        Self {
            fpsr: 0,
            fpcr: 0,
            mvfr0: 0,
            mvfr1: 0,
            mvfr2: 0,
            v: [0; 32],
        }
    }
}

/// ARM-specific private context
#[derive(Debug, Clone, Copy)]
pub struct ArmPrivContext {
    /// HCR_EL2 (Hypervisor Configuration Register)
    pub hcr_el2: u64,
    /// VTTBR_EL2 (Virtualization Translation Table Base Register)
    pub vttbr_el2: u64,
    /// VTCR_EL2 (Virtualization Translation Control Register)
    pub vtcr_el2: u64,
    /// SCTLR_EL2 (System Control Register EL2)
    pub sctlr_el2: u64,
    /// CPTR_EL2 (Architectural Feature Trap Register)
    pub cptr_el2: u64,
    /// HSTR_EL2 (Hypervisor System Trap Register)
    pub hstr_el2: u64,
    /// HACR_EL2 (Hypervisor Auxiliary Control Register)
    pub hacr_el2: u64,
    /// MDCR_EL2 (Monitor Debug Configuration Register)
    pub mdcr_el2: u64,
}

impl Default for ArmPrivContext {
    fn default() -> Self {
        Self {
            hcr_el2: 0,
            vttbr_el2: 0,
            vtcr_el2: 0x80000000, // Default VTCR_EL2 (T0SZ=31)
            sctlr_el2: 0x30C50830, // Default SCTLR_EL2
            cptr_el2: 0,
            hstr_el2: 0,
            hacr_el2: 0,
            mdcr_el2: 0,
        }
    }
}

/// VCPU context
#[derive(Debug, Clone, Copy)]
pub struct VcpuContext {
    /// Saved general-purpose registers
    pub gprs: SavedGprs,
    /// Saved special registers
    pub special: SavedSpecialRegs,
    /// Saved EL1 system registers
    pub el1_sysregs: SavedEl1SysRegs,
    /// Saved VFP/SIMD registers
    pub vfp: SavedVfpRegs,
    /// ARM private context
    pub priv_ctx: ArmPrivContext,
}

impl Default for VcpuContext {
    fn default() -> Self {
        Self {
            gprs: SavedGprs::default(),
            special: SavedSpecialRegs::default(),
            el1_sysregs: SavedEl1SysRegs::default(),
            vfp: SavedVfpRegs::default(),
            priv_ctx: ArmPrivContext::default(),
        }
    }
}

impl VcpuContext {
    /// Create a new VCPU context
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset the context
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Save current host context
    ///
    /// # Safety
    /// Must be called from EL2 with interrupts disabled
    pub unsafe fn save_host(&mut self) {
        // Save general-purpose registers
        // (This is typically done in assembly during context switch)
        // Here we define the structure layout

        // Note: EL1 registers reading from EL2 would trap
        // This function is a placeholder for the structure definition
    }

    /// Restore guest context
    ///
    /// # Safety
    /// Must be called from EL2 with interrupts disabled
    pub unsafe fn restore_guest(&self) {
        // Restore EL1 system registers
        // Note: This would be done in assembly during context switch
    }

    /// Save guest context
    ///
    /// # Safety
    /// Must be called from EL2 with interrupts disabled
    pub unsafe fn save_guest(&mut self) {
        // Save EL1 system registers from guest
        // Note: This would be done in assembly during context switch
    }

    /// Restore host context
    ///
    /// # Safety
    /// Must be called from EL2 with interrupts disabled
    pub unsafe fn restore_host(&self) {
        // Restore EL1 system registers for host
        // Note: This would be done in assembly during context switch
    }
}

/// Initialize CPU state management
pub fn init() -> Result<(), &'static str> {
    log::debug!("ARM64 CPU state management initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vcpu_context_default() {
        let ctx = VcpuContext::default();
        assert_eq!(ctx.gprs.x[0], 0);
        assert_eq!(ctx.special.sp, 0);
        assert_eq!(ctx.special.pc, 0);
    }

    #[test]
    fn test_vcpu_context_new() {
        let ctx = VcpuContext::new();
        assert_eq!(ctx.gprs.x[0], 0);
    }

    #[test]
    fn test_vcpu_context_reset() {
        let mut ctx = VcpuContext::new();
        ctx.gprs.x[0] = 42;
        ctx.gprs.x[1] = 100;
        ctx.reset();
        assert_eq!(ctx.gprs.x[0], 0);
        assert_eq!(ctx.gprs.x[1], 0);
    }
}
