//! Register access and management for ARM64
//!
//! Provides functions to read/write system registers and general-purpose registers.

use crate::ExceptionLevel;

/// General-purpose register index (X0-X30)
pub type GprIndex = u8;

/// Stack pointer
#[derive(Debug, Clone, Copy)]
pub struct StackPointer(pub u64);

/// Link register (X30)
#[derive(Debug, Clone, Copy)]
pub struct LinkRegister(pub u64);

/// Program counter
#[derive(Debug, Clone, Copy)]
pub struct ProgramCounter(pub u64);

/// Processor state (PSTATE)
#[derive(Debug, Clone, Copy)]
pub struct ProcessorState(pub u64);

impl ProcessorState {
    /// Create new PSTATE
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    /// Get SP field (stack pointer select)
    #[inline]
    pub fn sp(&self) -> bool {
        (self.0 & (1 << 0)) != 0
    }

    /// Set SP field
    #[inline]
    pub fn set_sp(&mut self, value: bool) {
        if value {
            self.0 |= 1 << 0;
        } else {
            self.0 &= !(1 << 0);
        }
    }
}

/// Read a 64-bit system register
///
/// # Safety
/// The caller must ensure the register exists and is accessible at current EL.
#[inline]
pub unsafe fn read_sysreg<const OP0: u8, const OP1: u8, const CRN: u8, const CRM: u8, const OP2: u8>() -> u64 {
    let value: u64;
    core::arch::asm!(
        "mrs {x}, S{OP0}_{OP1}_C{CRN}_C{CRM}_{OP2}",
        x = out(reg) value,
        OP0 = const OP0,
        OP1 = const OP1,
        CRN = const CRN,
        CRM = const CRM,
        OP2 = const OP2,
    );
    value
}

/// Write a 64-bit system register
///
/// # Safety
/// The caller must ensure the register exists and is writable at current EL.
#[inline]
pub unsafe fn write_sysreg<const OP0: u8, const OP1: u8, const CRN: u8, const CRM: u8, const OP2: u8>(value: u64) {
    core::arch::asm!(
        "msr S{OP0}_{OP1}_C{CRN}_C{CRM}_{OP2}, {x}",
        x = in(reg) value,
        OP0 = const OP0,
        OP1 = const OP1,
        CRN = const CRN,
        CRM = const CRM,
        OP2 = const OP2,
        options(nostack),
    );
}

/// EL2 system register access functions
pub mod el2 {
    use super::*;

    /// Read HCR_EL2 (Hypervisor Configuration Register)
    #[inline]
    pub fn read_hcr_el2() -> u64 {
        unsafe { read_sysreg::<3, 0, 4, 1, 0>() }
    }

    /// Write HCR_EL2
    #[inline]
    pub fn write_hcr_el2(value: u64) {
        unsafe { write_sysreg::<3, 0, 4, 1, 0>(value) }
    }

    /// Read VTTBR_EL2 (Virtualization Translation Table Base Register)
    #[inline]
    pub fn read_vttbr_el2() -> u64 {
        unsafe { read_sysreg::<3, 0, 2, 1, 0>() }
    }

    /// Write VTTBR_EL2
    #[inline]
    pub fn write_vttbr_el2(value: u64) {
        unsafe { write_sysreg::<3, 0, 2, 1, 0>(value) }
    }

    /// Read VTCR_EL2 (Virtualization Translation Control Register)
    #[inline]
    pub fn read_vtcr_el2() -> u64 {
        unsafe { read_sysreg::<3, 0, 2, 1, 2>() }
    }

    /// Write VTCR_EL2
    #[inline]
    pub fn write_vtcr_el2(value: u64) {
        unsafe { write_sysreg::<3, 0, 2, 1, 2>(value) }
    }

    /// Read SCTLR_EL2 (System Control Register)
    #[inline]
    pub fn read_sctlr_el2() -> u64 {
        unsafe { read_sysreg::<3, 0, 1, 0, 0>() }
    }

    /// Write SCTLR_EL2
    #[inline]
    pub fn write_sctlr_el2(value: u64) {
        unsafe { write_sysreg::<3, 0, 1, 0, 0>(value) }
    }

    /// Read CPTR_EL2 (Architectural Feature Trap Register)
    #[inline]
    pub fn read_cptr_el2() -> u64 {
        unsafe { read_sysreg::<3, 0, 1, 1, 2>() }
    }

    /// Write CPTR_EL2
    #[inline]
    pub fn write_cptr_el2(value: u64) {
        unsafe { write_sysreg::<3, 0, 1, 1, 2>(value) }
    }

    /// Read HSTR_EL2 (Hypervisor System Trap Register)
    #[inline]
    pub fn read_hstr_el2() -> u64 {
        unsafe { read_sysreg::<3, 0, 1, 1, 7>() }
    }

    /// Write HSTR_EL2
    #[inline]
    pub fn write_hstr_el2(value: u64) {
        unsafe { write_sysreg::<3, 0, 1, 1, 7>(value) }
    }

    /// Read HPFAR_EL2 (Hypervisor IPA Fault Address Register)
    #[inline]
    pub fn read_hpfar_el2() -> u64 {
        unsafe { read_sysreg::<3, 0, 6, 0, 4>() }
    }

    /// Read HCRX_EL2 (Extended Hypervisor Configuration Register)
    #[inline]
    pub fn read_hcrx_el2() -> u64 {
        unsafe { read_sysreg::<3, 0, 1, 2, 4>() }
    }

    /// Write HCRX_EL2
    #[inline]
    pub fn write_hcrx_el2(value: u64) {
        unsafe { write_sysreg::<3, 0, 1, 2, 4>(value) }
    }
}

/// EL1 system register access functions (for VCPU emulation)
pub mod el1 {
    use super::*;

    /// Read SCTLR_EL1 (System Control Register)
    #[inline]
    pub fn read_sctlr_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 1, 0, 0>() }
    }

    /// Write SCTLR_EL1
    #[inline]
    pub fn write_sctlr_el1(value: u64) {
        unsafe { write_sysreg::<3, 0, 1, 0, 0>(value) }
    }

    /// Read TCR_EL1 (Translation Control Register)
    #[inline]
    pub fn read_tcr_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 2, 0, 2>() }
    }

    /// Write TCR_EL1
    #[inline]
    pub fn write_tcr_el1(value: u64) {
        unsafe { write_sysreg::<3, 0, 2, 0, 2>(value) }
    }

    /// Read TTBR0_EL1 (Translation Table Base Register 0)
    #[inline]
    pub fn read_ttbr0_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 2, 0, 0>() }
    }

    /// Write TTBR0_EL1
    #[inline]
    pub fn write_ttbr0_el1(value: u64) {
        unsafe { write_sysreg::<3, 0, 2, 0, 0>(value) }
    }

    /// Read TTBR1_EL1 (Translation Table Base Register 1)
    #[inline]
    pub fn read_ttbr1_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 2, 0, 1>() }
    }

    /// Write TTBR1_EL1
    #[inline]
    pub fn write_ttbr1_el1(value: u64) {
        unsafe { write_sysreg::<3, 0, 2, 0, 1>(value) }
    }

    /// Read MAIR_EL1 (Memory Attribute Indirection Register)
    #[inline]
    pub fn read_mair_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 10, 2, 0>() }
    }

    /// Write MAIR_EL1
    #[inline]
    pub fn write_mair_el1(value: u64) {
        unsafe { write_sysreg::<3, 0, 10, 2, 0>(value) }
    }

    /// Read VBAR_EL1 (Vector Base Address Register)
    #[inline]
    pub fn read_vbar_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 12, 0, 0>() }
    }

    /// Write VBAR_EL1
    #[inline]
    pub fn write_vbar_el1(value: u64) {
        unsafe { write_sysreg::<3, 0, 12, 0, 0>(value) }
    }

    /// Read ESR_EL1 (Exception Syndrome Register)
    #[inline]
    pub fn read_esr_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 5, 2, 0>() }
    }

    /// Read FAR_EL1 (Fault Address Register)
    #[inline]
    pub fn read_far_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 6, 0, 0>() }
    }

    /// Read ELR_EL1 (Exception Link Register)
    #[inline]
    pub fn read_elr_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 4, 0, 1>() }
    }

    /// Write ELR_EL1
    #[inline]
    pub fn write_elr_el1(value: u64) {
        unsafe { write_sysreg::<3, 0, 4, 0, 1>(value) }
    }
}

/// Information registers
pub mod info {
    use super::*;

    /// Read MIDR_EL1 (Main ID Register)
    #[inline]
    pub fn read_midr_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 0, 0, 0>() }
    }

    /// Read MPIDR_EL1 (Multiprocessor Affinity Register)
    #[inline]
    pub fn read_mpidr_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 0, 0, 5>() }
    }

    /// Read ID_AA64PFR0_EL1 (Processor Feature Register 0)
    #[inline]
    pub fn read_id_aa64pfr0_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 0, 4, 0>() }
    }

    /// Read ID_AA64DFR0_EL1 (Debug Feature Register 0)
    #[inline]
    pub fn read_id_aa64dfr0_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 0, 5, 0>() }
    }

    /// Read ID_AA64ISAR0_EL1 (Instruction Set Attribute Register 0)
    #[inline]
    pub fn read_id_aa64isar0_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 0, 6, 0>() }
    }

    /// Read ID_AA64MMFR0_EL1 (Memory Model Feature Register 0)
    #[inline]
    pub fn read_id_aa64mmfr0_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 0, 7, 0>() }
    }

    /// Read ID_AA64MMFR1_EL1 (Memory Model Feature Register 1)
    #[inline]
    pub fn read_id_aa64mmfr1_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 0, 7, 1>() }
    }

    /// Read ID_AA64MMFR2_EL1 (Memory Model Feature Register 2)
    #[inline]
    pub fn read_id_aa64mmfr2_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 0, 7, 2>() }
    }

    /// Read ID_AA64ISAR1_EL1 (Instruction Set Attribute Register 1)
    #[inline]
    pub fn read_id_aa64isar1_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 0, 6, 1>() }
    }

    /// Read ID_AA64ISAR2_EL1 (Instruction Set Attribute Register 2)
    #[inline]
    pub fn read_id_aa64isar2_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 0, 6, 2>() }
    }

    /// Read ID_AA64PFR1_EL1 (Processor Feature Register 1)
    #[inline]
    pub fn read_id_aa64pfr1_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 0, 4, 1>() }
    }

    /// Read ID_AA64DFR1_EL1 (Debug Feature Register 1)
    #[inline]
    pub fn read_id_aa64dfr1_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 0, 5, 1>() }
    }

    /// Read ID_AA64ZFR0_EL1 (SVE Feature Register)
    #[inline]
    pub fn read_id_aa64zfr0_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 0, 4, 4>() }
    }

    /// Read ID_AA64SMFR0_EL1 (SME Feature Register)
    #[inline]
    pub fn read_id_aa64smfr0_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 0, 4, 5>() }
    }

    /// Read ID_AA64FR0_EL1 (Floating-point Feature Register)
    #[inline]
    pub fn read_id_aa64fr0_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 0, 4, 6>() }
    }

    /// Read MVFR0_EL1 (Media and VFP Feature Register 0)
    #[inline]
    pub fn read_mvfr0_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 0, 3, 0>() }
    }

    /// Read MVFR1_EL1 (Media and VFP Feature Register 1)
    #[inline]
    pub fn read_mvfr1_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 0, 3, 1>() }
    }

    /// Read REVIDR_EL1 (Revision Register)
    #[inline]
    pub fn read_revidr_el1() -> u64 {
        unsafe { read_sysreg::<3, 0, 0, 0, 6>() }
    }

    /// Read CNTFRQ_EL0 (Counter Frequency Register)
    #[inline]
    pub fn read_cntfrq_el0() -> u64 {
        unsafe { read_sysreg::<3, 3, 14, 0, 0>() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processor_state() {
        let mut pstate = ProcessorState::new(0);
        assert!(!pstate.sp());
        pstate.set_sp(true);
        assert!(pstate.sp());
    }
}
