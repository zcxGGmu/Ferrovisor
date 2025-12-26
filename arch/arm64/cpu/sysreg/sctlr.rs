//! System Control Register Emulation for ARM64
//!
//! Provides SCTLR_EL1 and ACTLR_EL1 register emulation.
//! Reference: ARM DDI 0487I.a - Chapter D12 - System Control Registers

use crate::arch::arm64::cpu::sysreg::{SysRegEncoding, RegReadResult, RegWriteResult};

/// SCTLR_EL1 - System Control Register EL1
///
/// Controls system-level behaviors including MMU, alignment, and caching.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SctlrEl1 {
    pub raw: u64,
}

impl SctlrEl1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get M bit (MMU enable for EL1/EL0)
    pub fn m(&self) -> bool {
        (self.raw & (1 << 0)) != 0
    }

    /// Set M bit
    pub fn set_m(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 0;
        } else {
            self.raw &= !(1 << 0);
        }
    }

    /// Get A bit (Alignment check enable)
    pub fn a(&self) -> bool {
        (self.raw & (1 << 1)) != 0
    }

    /// Set A bit
    pub fn set_a(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 1;
        } else {
            self.raw &= !(1 << 1);
        }
    }

    /// Get C bit (Data cache enable for EL1/EL0)
    pub fn c(&self) -> bool {
        (self.raw & (1 << 2)) != 0
    }

    /// Set C bit
    pub fn set_c(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 2;
        } else {
            self.raw &= !(1 << 2);
        }
    }

    /// Get SA bit (Stack alignment check enable)
    pub fn sa(&self) -> bool {
        (self.raw & (1 << 3)) != 0
    }

    /// Set SA bit
    pub fn set_sa(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 3;
        } else {
            self.raw &= !(1 << 3);
        }
    }

    /// Get I bit (Instruction cache enable for EL1/EL0)
    pub fn i(&self) -> bool {
        (self.raw & (1 << 12)) != 0
    }

    /// Set I bit
    pub fn set_i(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 12;
        } else {
            self.raw &= !(1 << 12);
        }
    }

    /// Get WXN bit (Write implies XN)
    pub fn wxn(&self) -> bool {
        (self.raw & (1 << 19)) != 0
    }

    /// Set WXN bit
    pub fn set_wxn(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 19;
        } else {
            self.raw &= !(1 << 19);
        }
    }

    /// Get EE bit (Exception endianness)
    pub fn ee(&self) -> bool {
        (self.raw & (1 << 25)) != 0
    }

    /// Set EE bit
    pub fn set_ee(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 25;
        } else {
            self.raw &= !(1 << 25);
        }
    }

    /// Get UCI bit (EL0 access to Cache instructions)
    pub fn uci(&self) -> bool {
        (self.raw & (1 << 26)) != 0
    }

    /// Set UCI bit
    pub fn set_uci(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 26;
        } else {
            self.raw &= !(1 << 26);
        }
    }

    /// Get EO bit (Exception endianness for EL0)
    pub fn eo(&self) -> bool {
        (self.raw & (1 << 27)) != 0
    }

    /// Set EO bit
    pub fn set_eo(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 27;
        } else {
            self.raw &= !(1 << 27);
        }
    }

    /// Get UWXN bit (Write implies XN for EL0)
    pub fn uwxn(&self) -> bool {
        (self.raw & (1 << 28)) != 0
    }

    /// Set UWXN bit
    pub fn set_uwxn(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 28;
        } else {
            self.raw &= !(1 << 28);
        }
    }

    /// Get PAN bit (Privileged Access Never)
    pub fn pan(&self) -> bool {
        (self.raw & (1 << 22)) != 0
    }

    /// Set PAN bit
    pub fn set_pan(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 22;
        } else {
            self.raw &= !(1 << 22);
        }
    }

    /// Get EPD bits (Table walks for EL0/EL1 disabled)
    pub fn epd0(&self) -> bool {
        (self.raw & (1 << 7)) != 0
    }

    pub fn epd1(&self) -> bool {
        (self.raw & (1 << 23)) != 0
    }

    /// Get TCF bits (Tag Check Fault)
    pub fn tcf(&self) -> u64 {
        (self.raw >> 36) & 0x3
    }

    /// Get TIDCP bit (EL0 access to DC CVAP)
    pub fn tidcp(&self) -> bool {
        (self.raw & (1 << 58)) != 0
    }

    /// Create default SCTLR value for EL1
    ///
    /// Typical values: MMU disabled, caches disabled, AArch64 execution
    pub fn default_el1() -> Self {
        // M=0, A=0, C=0, I=0, WXN=0, EE=0 (little endian)
        Self::new(0x00C50078)
    }

    /// Create MMU-enabled SCTLR value
    pub fn with_mmu() -> Self {
        // M=1, A=0, C=1, I=1, WXN=0, EE=0
        Self::new(0x00C51878)
    }
}

/// ACTLR_EL1 - Auxiliary Control Register EL1
///
/// Implementation-defined features and behavior configuration.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ActlrEl1 {
    pub raw: u64,
}

impl ActlrEl1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get CP15BEN bit (CP15 barrier enable)
    pub fn cp15ben(&self) -> bool {
        (self.raw & (1 << 5)) != 0
    }

    /// Set CP15BEN bit
    pub fn set_cp15ben(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 5;
        } else {
            self.raw &= !(1 << 5);
        }
    }

    /// Get AME bit (AMU enable)
    pub fn ame(&self) -> bool {
        (self.raw & (1 << 13)) != 0
    }

    /// Set AME bit
    pub fn set_ame(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 13;
        } else {
            self.raw &= !(1 << 13);
        }
    }

    /// Get TTBR0_EL1 and TTBR1_EL1 walk sharing enable
    pub fn ttbr0_el1_sh(&self) -> bool {
        (self.raw & (1 << 14)) != 0
    }

    /// Get Enable hardware updates to the PTE AF bit
    pub fn hafdbs(&self) -> bool {
        (self.raw & (1 << 29)) != 0
    }

    /// Set HAFDBS bit
    pub fn set_hafdbs(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 29;
        } else {
            self.raw &= !(1 << 29);
        }
    }

    /// Get Enable updated PTE dirty state mechanism
    pub pub fn pdu(&self) -> bool {
        (self.raw & (1 << 33)) != 0
    }

    /// Set PDU bit
    pub fn set_pdu(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 33;
        } else {
            self.raw &= !(1 << 33);
        }
    }

    /// Get Enable updated PTE AF mechanism
    pub fn enrc(&self) -> bool {
        (self.raw & (1 << 34)) != 0
    }

    /// Set ENRC bit
    pub fn set_enrc(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 34;
        } else {
            self.raw &= !(1 << 34);
        }
    }

    /// Get Enable PTE dirtied by load mechanism
    pub fn ddis(&self) -> bool {
        (self.raw & (1 << 35)) != 0
    }

    /// Set DDIS bit
    pub fn set_ddis(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 35;
        } else {
            self.raw &= !(1 << 35);
        }
    }

    /// Create default ACTLR value
    pub fn default() -> Self {
        Self::new(0x00000000)
    }
}

/// CPACR_EL1 - Coprocessor Access Control Register
///
/// Controls access to SIMD, FP, and SVE instructions.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct CpacrEl1 {
    pub raw: u64,
}

impl CpacrEl1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get FP enable for EL0
    pub fn fp_el0(&self) -> u64 {
        (self.raw >> 0) & 0x3
    }

    /// Set FP enable for EL0
    pub fn set_fp_el0(&mut self, value: u64) {
        self.raw = (self.raw & !0x3) | (value & 0x3);
    }

    /// Get SIMD enable for EL0
    pub fn simd_el0(&self) -> u64 {
        (self.raw >> 0) & 0x3
    }

    /// Set SIMD enable for EL0
    pub fn set_simd_el0(&mut self, value: u64) {
        self.raw = (self.raw & !0x3) | (value & 0x3);
    }

    /// Get trap EL1 accesses to SVE
    pub fn trap_sve_el1(&self) -> bool {
        (self.raw & (1 << 2)) != 0
    }

    /// Get trap EL0 accesses to SVE
    pub fn trap_sve_el0(&self) -> bool {
        (self.raw & (1 << 3)) != 0
    }

    /// Get trap EL1 accesses to SIMD/F
    pub fn trap_simd_f_el1(&self) -> u64 {
        (self.raw >> 20) & 0x3
    }

    /// Set trap EL1 accesses to SIMD/F
    pub fn set_trap_simd_f_el1(&mut self, value: u64) {
        self.raw = (self.raw & !(0x3 << 20)) | ((value & 0x3) << 20);
    }

    /// Get trap EL0 accesses to SIMD/F
    pub fn trap_simd_f_el0(&self) -> u64 {
        (self.raw >> 22) & 0x3
    }

    /// Set trap EL0 accesses to SIMD/F
    pub fn set_trap_simd_f_el0(&mut self, value: u64) {
        self.raw = (self.raw & !(0x3 << 22)) | ((value & 0x3) << 22);
    }

    /// Create default CPACR value (SIMD/FP enabled at all levels)
    pub fn default() -> Self {
        // FP=SIMD=0x3 (full access) at EL0, EL1
        Self::new(0x00300000)
    }

    /// Create CPACR with all traps disabled
    pub fn full_access() -> Self {
        Self::new(0x00333333)
    }
}

/// System control register state for a VCPU
#[derive(Debug, Clone)]
pub struct SystemControlRegs {
    /// SCTLR_EL1
    pub sctlr: SctlrEl1,
    /// ACTLR_EL1
    pub actlr: ActlrEl1,
    /// CPACR_EL1
    pub cpacr: CpacrEl1,
}

impl Default for SystemControlRegs {
    fn default() -> Self {
        Self {
            sctlr: SctlrEl1::default_el1(),
            actlr: ActlrEl1::default(),
            cpacr: CpacrEl1::default(),
        }
    }
}

impl SystemControlRegs {
    /// Create new system control registers with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize from hardware registers
    pub fn from_hw() -> Self {
        let mut regs = Self::new();

        #[cfg(target_arch = "aarch64")]
        unsafe {
            let mut sctlr: u64;
            let mut actlr: u64;
            let mut cpacr: u64;

            core::arch::asm!("mrs {}, sctlr_el1", out(reg) sctlr);
            core::arch::asm!("mrs {}, actlr_el1", out(reg) actlr);
            core::arch::asm!("mrs {}, cpacr_el1", out(reg) cpacr);

            regs.sctlr = SctlrEl1::new(sctlr);
            regs.actlr = ActlrEl1::new(actlr);
            regs.cpacr = CpacrEl1::new(cpacr);
        }

        regs
    }

    /// Read a system control register by encoding
    pub fn read_ctrl_reg(&self, encoding: SysRegEncoding) -> RegReadResult {
        match (encoding.op0, encoding.op1, encoding.crn, encoding.crm, encoding.op2) {
            // SCTLR_EL1: op0=3, op1=0, crn=1, crm=0, op2=0
            (3, 0, 1, 0, 0) => RegReadResult::Ok { data: self.sctlr.raw },
            // ACTLR_EL1: op0=3, op1=0, crn=1, crm=0, op2=1
            (3, 0, 1, 0, 1) => RegReadResult::Ok { data: self.actlr.raw },
            // CPACR_EL1: op0=3, op1=0, crn=1, crm=0, op2=2
            (3, 0, 1, 0, 2) => RegReadResult::Ok { data: self.cpacr.raw },
            _ => RegReadResult::Unimplemented,
        }
    }

    /// Write to a system control register
    pub fn write_ctrl_reg(&mut self, encoding: SysRegEncoding, value: u64) -> RegWriteResult {
        match (encoding.op0, encoding.op1, encoding.crn, encoding.crm, encoding.op2) {
            // SCTLR_EL1
            (3, 0, 1, 0, 0) => {
                self.sctlr = SctlrEl1::new(value);
                RegWriteResult::Ok
            }
            // ACTLR_EL1
            (3, 0, 1, 0, 1) => {
                self.actlr = ActlrEl1::new(value);
                RegWriteResult::Ok
            }
            // CPACR_EL1
            (3, 0, 1, 0, 2) => {
                self.cpacr = CpacrEl1::new(value);
                RegWriteResult::Ok
            }
            _ => RegWriteResult::Unimplemented,
        }
    }

    /// Enable MMU
    pub fn enable_mmu(&mut self) {
        self.sctlr.set_m(true);
        self.sctlr.set_c(true);
        self.sctlr.set_i(true);
    }

    /// Disable MMU
    pub fn disable_mmu(&mut self) {
        self.sctlr.set_m(false);
        self.sctlr.set_c(false);
        self.sctlr.set_i(false);
    }

    /// Check if MMU is enabled
    pub fn is_mmu_enabled(&self) -> bool {
        self.sctlr.m()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sctlr_creation() {
        let sctlr = SctlrEl1::default_el1();
        assert!(!sctlr.m());  // MMU disabled by default
        assert!(!sctlr.c());
        assert!(!sctlr.i());
    }

    #[test]
    fn test_sctlr_enable_mmu() {
        let mut sctlr = SctlrEl1::default_el1();
        assert!(!sctlr.m());

        sctlr.set_m(true);
        assert!(sctlr.m());
    }

    #[test]
    fn test_system_ctrl_regs_read() {
        let regs = SystemControlRegs::new();
        let encoding = SysRegEncoding {
            op0: 3, op1: 0, crn: 1, crm: 0, op2: 0
        };

        match regs.read_ctrl_reg(encoding) {
            RegReadResult::Ok { data } => assert_eq!(data, regs.sctlr.raw),
            _ => panic!("Expected Ok result"),
        }
    }

    #[test]
    fn test_system_ctrl_regs_write() {
        let mut regs = SystemControlRegs::new();
        let encoding = SysRegEncoding {
            op0: 3, op1: 0, crn: 1, crm: 0, op2: 0
        };

        assert!(matches!(regs.write_ctrl_reg(encoding, 0x12345678),
                         RegWriteResult::Ok));
        assert_eq!(regs.sctlr.raw, 0x12345678);
    }

    #[test]
    fn test_cpacr_defaults() {
        let cpacr = CpacrEl1::default();
        // FP/SIMD should be enabled at EL0 by default
        assert_eq!(cpacr.fp_el0(), 0x3);
        assert_eq!(cpacr.simd_el0(), 0x3);
    }
}
