//! System register state for ARM64 VCPU
//!
//! Provides saved system register state per VCPU.
//! Reference: xvisor/arch/arm/include/arm_types.h

/// Saved system registers for ARM64 VCPU
///
/// These are the EL1/EL0 system registers that need to be
/// saved/restored on VCPU context switch.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SysRegs {
    /// SP_EL0 - Stack pointer for EL0
    pub sp_el0: u64,
    /// SP_EL1 - Stack pointer for EL1
    pub sp_el1: u64,
    /// ELR_EL1 - Exception Link Register
    pub elr_el1: u64,
    /// SPSR_EL1 - Saved Program Status Register
    pub spsr_el1: u64,

    /// MIDR_EL1 - Processor ID register
    pub midr_el1: u64,
    /// MPIDR_EL1 - Multiprocessor Affinity Register
    pub mpidr_el1: u64,

    /// SCTLR_EL1 - System Control Register
    pub sctlr_el1: u64,
    /// ACTLR_EL1 - Auxiliary Control Register
    pub actlr_el1: u64,
    /// CPACR_EL1 - Coprocessor Access Control Register
    pub cpacr_el1: u64,

    /// TTBR0_EL1 - Translation Table Base Register 0
    pub ttbr0_el1: u64,
    /// TTBR1_EL1 - Translation Table Base Register 1
    pub ttbr1_el1: u64,
    /// TCR_EL1 - Translation Control Register
    pub tcr_el1: u64,

    /// ESR_EL1 - Exception Syndrome Register
    pub esr_el1: u64,
    /// FAR_EL1 - Fault Address Register
    pub far_el1: u64,
    /// PAR_EL1 - Physical Address Register
    pub par_el1: u64,

    /// MAIR_EL1 - Memory Attribute Indirection Register
    pub mair_el1: u64,
    /// VBAR_EL1 - Vector Base Address Register
    pub vbar_el1: u64,

    /// CONTEXTIDR_EL1 - Context ID Register
    pub contextidr_el1: u64,

    /// TPIDR_EL0 - Thread ID Register (user read-write)
    pub tpidr_el0: u64,
    /// TPIDRRO_EL0 - Thread ID Register (user read-only)
    pub tpidrro_el0: u64,
    /// TPIDR_EL1 - Thread ID Register (privileged)
    pub tpidr_el1: u64,

    /// 32-bit mode specific SPSR registers
    pub spsr_abt: u32,
    pub spsr_und: u32,
    pub spsr_irq: u32,
    pub spsr_fiq: u32,

    /// DACR32_EL2 - Domain Access Control Register (AArch32)
    pub dacr32_el2: u32,
    /// IFSR32_EL2 - Instruction Fault Status (AArch32)
    pub ifsr32_el2: u32,

    /// TEECR32_EL1 - ThumbEE Control Register
    pub teecr32_el1: u32,
    /// TEEHBR32_EL1 - ThumbEE Handler Base Register
    pub teehbr32_el1: u32,

    /// FPEXC32_EL2 - Floating Point Exception Control
    pub fpexc32_el2: u32,
}

impl Default for SysRegs {
    fn default() -> Self {
        Self {
            sp_el0: 0,
            sp_el1: 0,
            elr_el1: 0,
            spsr_el1: 0,

            midr_el1: 0,
            mpidr_el1: 0,

            sctlr_el1: 0,
            actlr_el1: 0,
            cpacr_el1: 0,

            ttbr0_el1: 0,
            ttbr1_el1: 0,
            tcr_el1: 0,

            esr_el1: 0,
            far_el1: 0,
            par_el1: 0,

            mair_el1: 0,
            vbar_el1: 0,

            contextidr_el1: 0,

            tpidr_el0: 0,
            tpidrro_el0: 0,
            tpidr_el1: 0,

            spsr_abt: 0,
            spsr_und: 0,
            spsr_irq: 0,
            spsr_fiq: 0,

            dacr32_el2: 0,
            ifsr32_el2: 0,

            teecr32_el1: 0,
            teehbr32_el1: 0,

            fpexc32_el2: 0,
        }
    }
}

impl SysRegs {
    /// Create new zero-initialized system registers
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize with default safe values
    pub fn init_default() -> Self {
        let mut regs = Self::default();

        // Set default SCTLR_EL1 values
        // - MMU disabled initially
        // - Alignment checks disabled
        // - Endianness: little-endian
        regs.sctlr_el1 = 0xC00800;  // Standard reset value

        // Set default TCR_EL1 (translation disabled)
        // T0SZ=0, IRGN0=0, ORGN0=0, SH0=0, TG0=0, T1SZ=0, etc.
        regs.tcr_el1 = 0;

        regs
    }

    /// Save current system register state from hardware
    ///
    /// # Safety
    /// This function reads directly from system registers
    /// and should only be called when running in EL2.
    pub unsafe fn save_from_hw(&mut self) {
        use crate::arch::arm64::cpu::regs::*;

        self.sp_el0 = sp_el0_read();
        self.sp_el1 = sp_el1_read();
        self.elr_el1 = elr_el1_read();
        self.spsr_el1 = spsr_el1_read();

        self.midr_el1 = midr_el1_read();
        self.mpidr_el1 = mpidr_el1_read();

        self.sctlr_el1 = sctlr_el1_read();
        self.actlr_el1 = actlr_el1_read();
        self.cpacr_el1 = cpacr_el1_read();

        self.ttbr0_el1 = ttbr0_el1_read();
        self.ttbr1_el1 = ttbr1_el1_read();
        self.tcr_el1 = tcr_el1_read();

        self.esr_el1 = esr_el1_read();
        self.far_el1 = far_el1_read();
        self.par_el1 = par_el1_read();

        self.mair_el1 = mair_el1_read();
        self.vbar_el1 = vbar_el1_read();

        self.contextidr_el1 = contextidr_el1_read();

        self.tpidr_el0 = tpidr_el0_read();
        self.tpidrro_el0 = tpidrro_el0_read();
        self.tpidr_el1 = tpidr_el1_read();

        // Note: 32-bit registers need special handling
        // For now, keep them as-is
    }

    /// Restore system register state to hardware
    ///
    /// # Safety
    /// This function writes directly to system registers
    /// and should only be called when running in EL2.
    pub unsafe fn restore_to_hw(&self) {
        use crate::arch::arm64::cpu::regs::*;

        sp_el0_write(self.sp_el0);
        sp_el1_write(self.sp_el1);
        elr_el1_write(self.elr_el1);
        spsr_el1_write(self.spsr_el1);

        // MIDR/MPIDR are read-only, skip

        sctlr_el1_write(self.sctlr_el1);
        actlr_el1_write(self.actlr_el1);
        cpacr_el1_write(self.cpacr_el1);

        ttbr0_el1_write(self.ttbr0_el1);
        ttbr1_el1_write(self.ttbr1_el1);
        tcr_el1_write(self.tcr_el1);

        esr_el1_write(self.esr_el1);
        far_el1_write(self.far_el1);
        par_el1_write(self.par_el1);

        mair_el1_write(self.mair_el1);
        vbar_el1_write(self.vbar_el1);

        contextidr_el1_write(self.contextidr_el1);

        tpidr_el0_write(self.tpidr_el0);
        tpidrro_el0_write(self.tpidrro_el0);
        tpidr_el1_write(self.tpidr_el1);

        // Note: 32-bit registers need special handling
    }
}

/// System register trap state
///
/// Tracks which system registers are trapped and need emulation.
#[derive(Debug, Clone, Copy, Default)]
pub struct TrapState {
    /// HCR_EL2 trap bits
    pub hcr_el2_traps: u64,
    /// HSTR_EL2 trap bits
    pub hstr_el2: u32,
    /// CPTR_EL2 trap bits
    pub cptr_el2: u32,
}

impl TrapState {
    /// Create new trap state
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a system register access is trapped
    pub fn is_trapped_sysreg(&self, iss: u32) -> bool {
        // Check based on ISS encoding from ESR_EL2
        // This is a simplified check
        true
    }

    /// Check if CP15 access is trapped
    pub fn is_trapped_cp15(&self, crn: u32, crm: u32, opc1: u32, opc2: u32) -> bool {
        // Check HSTR_EL2 for CP15 traps
        let trap_bit = 1u32 << crn;
        (self.hstr_el2 & trap_bit) != 0
    }

    /// Check if CP14 access is trapped
    pub fn is_trapped_cp14(&self, crn: u32, crm: u32, opc1: u32, opc2: u32) -> bool {
        // CP14 traps are controlled by CPTR_EL2.TTA
        (self.cptr_el2 & (1 << 20)) != 0
    }

    /// Check if FP/SIMD access is trapped
    pub fn is_trapped_fpsimd(&self) -> bool {
        // CPTR_EL2.TFP (bit 10)
        (self.cptr_el2 & (1 << 10)) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sysregs_default() {
        let regs = SysRegs::default();
        assert_eq!(regs.sp_el0, 0);
        assert_eq!(regs.sctlr_el1, 0);
    }

    #[test]
    fn test_sysregs_init_default() {
        let regs = SysRegs::init_default();
        // SCTLR_EL1 should have reset value
        assert_eq!(regs.sctlr_el1, 0xC00800);
    }

    #[test]
    fn test_trap_state_default() {
        let trap = TrapState::default();
        assert_eq!(trap.hcr_el2_traps, 0);
        assert_eq!(trap.hstr_el2, 0);
        assert_eq!(trap.cptr_el2, 0);
    }

    #[test]
    fn test_trap_state_cp15() {
        let mut trap = TrapState::new();
        trap.hstr_el2 = 1 << 1; // Trap c1

        assert!(trap.is_trapped_cp15(1, 0, 0, 0));
        assert!(!trap.is_trapped_cp15(2, 0, 0, 0));
    }

    #[test]
    fn test_trap_state_fpsimd() {
        let mut trap = TrapState::new();
        assert!(!trap.is_trapped_fpsimd());

        trap.cptr_el2 = 1 << 10; // Set TFP bit
        assert!(trap.is_trapped_fpsimd());
    }
}
