//! RISC-V Register Definitions
//!
//! This module contains definitions for all RISC-V registers including:
//! - General purpose registers
//! - Floating point registers
//! - Control and status registers (CSRs)
//! - Vector registers (when V extension is supported)

use bitflags::bitflags;

/// RISC-V General Purpose Register Numbers
#[allow(dead_code)]
#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gpr {
    Zero = 0,   // hard-wired zero
    RA = 1,     // return address
    SP = 2,     // stack pointer
    GP = 3,     // global pointer
    TP = 4,     // thread pointer
    T0 = 5,     // temporary/alternate link register
    T1 = 6,     // temporary
    T2 = 7,     // temporary
    S0 = 8,     // saved register/frame pointer
    S1 = 9,     // saved register
    A0 = 10,    // function argument/return value
    A1 = 11,    // function argument/return value
    A2 = 12,    // function argument
    A3 = 13,    // function argument
    A4 = 14,    // function argument
    A5 = 15,    // function argument
    A6 = 16,    // function argument
    A7 = 17,    // function argument
    S2 = 18,    // saved register
    S3 = 19,    // saved register
    S4 = 20,    // saved register
    S5 = 21,    // saved register
    S6 = 22,    // saved register
    S7 = 23,    // saved register
    S8 = 24,    // saved register
    S9 = 25,    // saved register
    S10 = 26,   // saved register
    S11 = 27,   // saved register
    T3 = 28,    // temporary
    T4 = 29,    // temporary
    T5 = 30,    // temporary
    T6 = 31,    // temporary
}

impl Gpr {
    /// Get the ABI name for this register
    pub fn abi_name(&self) -> &'static str {
        match self {
            Gpr::Zero => "zero",
            Gpr::RA => "ra",
            Gpr::SP => "sp",
            Gpr::GP => "gp",
            Gpr::TP => "tp",
            Gpr::T0 => "t0",
            Gpr::T1 => "t1",
            Gpr::T2 => "t2",
            Gpr::S0 => "s0",
            Gpr::S1 => "s1",
            Gpr::A0 => "a0",
            Gpr::A1 => "a1",
            Gpr::A2 => "a2",
            Gpr::A3 => "a3",
            Gpr::A4 => "a4",
            Gpr::A5 => "a5",
            Gpr::A6 => "a6",
            Gpr::A7 => "a7",
            Gpr::S2 => "s2",
            Gpr::S3 => "s3",
            Gpr::S4 => "s4",
            Gpr::S5 => "s5",
            Gpr::S6 => "s6",
            Gpr::S7 => "s7",
            Gpr::S8 => "s8",
            Gpr::S9 => "s9",
            Gpr::S10 => "s10",
            Gpr::S11 => "s11",
            Gpr::T3 => "t3",
            Gpr::T4 => "t4",
            Gpr::T5 => "t5",
            Gpr::T6 => "t6",
        }
    }

    /// Check if this register is callee-saved
    pub fn is_callee_saved(&self) -> bool {
        matches!(
            self,
            Gpr::SP | Gpr::S0 | Gpr::S1 | Gpr::S2 | Gpr::S3 | Gpr::S4 | Gpr::S5 | Gpr::S6 | Gpr::S7 | Gpr::S8 | Gpr::S9 | Gpr::S10 | Gpr::S11
        )
    }

    /// Check if this register is caller-saved
    pub fn is_caller_saved(&self) -> bool {
        !self.is_callee_saved() && *self != Gpr::Zero && *self != Gpr::GP && *self != Gpr::TP
    }
}

/// Floating Point Register Numbers (F extension)
#[allow(dead_code)]
#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fpr {
    FT0 = 0,
    FT1 = 1,
    FT2 = 2,
    FT3 = 3,
    FT4 = 4,
    FT5 = 5,
    FT6 = 6,
    FT7 = 7,
    FS0 = 8,
    FS1 = 9,
    FA0 = 10,
    FA1 = 11,
    FA2 = 12,
    FA3 = 13,
    FA4 = 14,
    FA5 = 15,
    FA6 = 16,
    FA7 = 17,
    FS2 = 18,
    FS3 = 19,
    FS4 = 20,
    FS5 = 21,
    FS6 = 22,
    FS7 = 23,
    FS8 = 24,
    FS9 = 25,
    FS10 = 26,
    FS11 = 27,
    FT8 = 28,
    FT9 = 29,
    FT10 = 30,
    FT11 = 31,
}

/// RISC-V CPU State structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CpuState {
    /// General purpose registers
    pub gpr: [usize; 32],
    /// Program counter
    pub pc: usize,
    /// Floating point registers (F extension)
    pub fpr: [u64; 32],
    /// FPU CSR
    pub fcsr: u32,
    /// Privilege mode
    pub privilege: u8,
    /// Reserved for alignment
    _reserved: [u8; 7],
}

impl Default for CpuState {
    fn default() -> Self {
        Self {
            gpr: [0; 32],
            pc: 0,
            fpr: [0; 32],
            fcsr: 0,
            privilege: 3, // Machine mode by default
            _reserved: [0; 7],
        }
    }
}

impl CpuState {
    /// Create a new CPU state
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a general purpose register
    pub fn get_gpr(&self, reg: Gpr) -> usize {
        self.gpr[reg as usize]
    }

    /// Set a general purpose register
    pub fn set_gpr(&mut self, reg: Gpr, value: usize) {
        self.gpr[reg as usize] = value;
    }

    /// Get the stack pointer
    pub fn get_sp(&self) -> usize {
        self.gpr[Gpr::SP as usize]
    }

    /// Set the stack pointer
    pub fn set_sp(&mut self, sp: usize) {
        self.gpr[Gpr::SP as usize] = sp;
    }

    /// Get the return address
    pub fn get_ra(&self) -> usize {
        self.gpr[Gpr::RA as usize]
    }

    /// Set the return address
    pub fn set_ra(&mut self, ra: usize) {
        self.gpr[Gpr::RA as usize] = ra;
    }

    /// Get the frame pointer
    pub fn get_fp(&self) -> usize {
        self.gpr[Gpr::S0 as usize]
    }

    /// Set the frame pointer
    pub fn set_fp(&mut self, fp: usize) {
        self.gpr[Gpr::S0 as usize] = fp;
    }

    /// Get a floating point register
    pub fn get_fpr(&self, reg: Fpr) -> u64 {
        self.fpr[reg as usize]
    }

    /// Set a floating point register
    pub fn set_fpr(&mut self, reg: Fpr, value: u64) {
        self.fpr[reg as usize] = value;
    }

    /// Get the current privilege level
    pub fn get_privilege(&self) -> crate::arch::riscv64::PrivilegeLevel {
        match self.privilege {
            0 => crate::arch::riscv64::PrivilegeLevel::User,
            1 => crate::arch::riscv64::PrivilegeLevel::Supervisor,
            3 => crate::arch::riscv64::PrivilegeLevel::Machine,
            _ => crate::arch::riscv64::PrivilegeLevel::Reserved,
        }
    }

    /// Set the privilege level
    pub fn set_privilege(&mut self, privilege: crate::arch::riscv64::PrivilegeLevel) {
        self.privilege = privilege as u8;
    }

    /// Save callee-saved registers to the state
    pub fn save_callee_saved(&mut self) {
        // This would typically be called in a function prologue
        // The actual implementation would be in assembly
    }

    /// Restore callee-saved registers from the state
    pub fn restore_callee_saved(&self) {
        // This would typically be called in a function epilogue
        // The actual implementation would be in assembly
    }
}

/// MSTATUS register flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Mstatus: usize {
        const UIE = 1 << 0;      // User Interrupt Enable
        const SIE = 1 << 1;      // Supervisor Interrupt Enable
        const MIE = 1 << 3;      // Machine Interrupt Enable
        const UPIE = 1 << 4;     // User Previous Interrupt Enable
        const SPIE = 1 << 5;     // Supervisor Previous Interrupt Enable
        const MPIE = 1 << 7;     // Machine Previous Interrupt Enable
        const SPP = 1 << 8;      // Supervisor Previous Privilege
        const MPP = 3 << 11;     // Machine Previous Privilege
        const FS = 3 << 13;      // Floating-point unit status
        const XS = 3 << 15;      // Extension unit status
        const MPRV = 1 << 17;    // Modify Privilege
        const SUM = 1 << 18;     // Supervisor User Memory access
        const MXR = 1 << 19;     // Make eXecutable Readable
        const TVM = 1 << 20;     // Trap Virtual Memory
        const TW = 1 << 21;      // Timeout Wait
        const TSR = 1 << 22;     // Trap SRET
        const UXL = 3 << 32;     // User mode XLEN
        const SXL = 3 << 34;     // Supervisor mode XLEN
        const SBE = 1 << 36;     // Stack-based exception
        const MBE = 1 << 37;     // Multiple branch exception
        const GVA = 1 << 38;     // Guest Virtual Address
        const MPV = 1 << 39;     // Machine Previous Virtualization
    }
}

/// SSTATUS register flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Sstatus: usize {
        const UIE = 1 << 0;      // User Interrupt Enable
        const SIE = 1 << 1;      // Supervisor Interrupt Enable
        const UPIE = 1 << 4;     // User Previous Interrupt Enable
        const SPIE = 1 << 5;     // Supervisor Previous Interrupt Enable
        const SPP = 1 << 8;      // Supervisor Previous Privilege
        const FS = 3 << 13;      // Floating-point unit status
        const XS = 3 << 15;      // Extension unit status
        const SUM = 1 << 18;     // Supervisor User Memory access
        const MXR = 1 << 19;     // Make eXecutable Readable
        const UXL = 3 << 32;     // User mode XLEN
        const SBE = 1 << 36;     // Stack-based exception
        const MBE = 1 << 37;     // Multiple branch exception
        const GVA = 1 << 38;     // Guest Virtual Address
    }
}

/// HSTATUS register flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Hstatus: usize {
        const VSBE = 1 << 5;     // Virtual SBE
        const GVA = 1 << 6;      // Guest Virtual Access
        const VGEIN = 0x3FF << 7; // Virtual Guest External Interrupt Number
        const HU = 1 << 17;      // Hypervisor in User mode
        const SPVP = 1 << 18;    // Supervisor Previous Virtual Privilege
        const SPV = 1 << 19;     // Supervisor Previous Virtualization
        const VTSR = 1 << 22;    // Virtual Trap SRET
        const VTW = 1 << 21;     // Virtual Timeout Wait
        const VTVM = 1 << 20;    // Virtual Trap Virtual Memory
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpr_properties() {
        assert!(Gpr::S0.is_callee_saved());
        assert!(Gpr::T0.is_caller_saved());
        assert!(!Gpr::Zero.is_callee_saved());
        assert!(!Gpr::Zero.is_caller_saved());
    }

    #[test]
    fn test_gpr_names() {
        assert_eq!(Gpr::RA.abi_name(), "ra");
        assert_eq!(Gpr::SP.abi_name(), "sp");
        assert_eq!(Gpr::T0.abi_name(), "t0");
    }

    #[test]
    fn test_cpu_state() {
        let mut state = CpuState::new();

        // Test GPR access
        state.set_gpr(Gpr::A0, 42);
        assert_eq!(state.get_gpr(Gpr::A0), 42);

        // Test SP access
        state.set_sp(0x80000000);
        assert_eq!(state.get_sp(), 0x80000000);

        // Test RA access
        state.set_ra(0x1000);
        assert_eq!(state.get_ra(), 0x1000);

        // Test privilege level
        state.set_privilege(crate::arch::riscv64::PrivilegeLevel::Supervisor);
        assert_eq!(state.get_privilege(), crate::arch::riscv64::PrivilegeLevel::Supervisor);
    }

    #[test]
    fn test_mstatus_flags() {
        let mut mstatus = Mstatus::empty();

        mstatus.insert(Mstatus::MIE);
        assert!(mstatus.contains(Mstatus::MIE));

        mstatus.insert(Mstatus::MPP);
        assert!(mstatus.contains(Mstatus::MPP));
    }

    #[test]
    fn test_sstatus_flags() {
        let mut sstatus = Sstatus::empty();

        sstatus.insert(Sstatus::SIE);
        assert!(sstatus.contains(Sstatus::SIE));

        sstatus.insert(Sstatus::SPP);
        assert!(sstatus.contains(Sstatus::SPP));
    }
}