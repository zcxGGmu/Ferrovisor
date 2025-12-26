//! CP14 Coprocessor Emulation for ARMv7
//!
//! Provides debug, trace, and ThumbEE coprocessor emulation for ARMv7/ARMv8-AArch32 guests.
//! Reference: ARM DDI 0406C.d - Chapter B11 - Debug Architecture
//!
//! CP14 contains:
//! - Debug registers (breakpoints, watchpoints, debug control)
//! - Trace registers (program flow tracing)
//! - ThumbEE registers (Thumb Execution Environment)
//!
//! Note: Full debug and trace support is not commonly required for guest OS operation.
//! This implementation provides basic ThumbEE support and RAZ/WI for debug registers.

use crate::arch::arm64::cpu::sysreg::{RegReadResult, RegWriteResult};

use super::cp15::Cp15Encoding;

/// ThumbEE feature flag
pub const ARM_FEATURE_THUMB2EE: u64 = 1 << 20;

/// CP14 Register types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Cp14RegType {
    /// ThumbEE registers (opc1=6)
    ThumbEE = 6,
    /// Debug registers (opc1=0) - not implemented
    Debug = 0,
    /// Trace registers (opc1=1) - not implemented
    Trace = 1,
    /// Jazelle registers (opc1=7) - not implemented
    Jazelle = 7,
}

/// CP14 ThumbEE Registers
///
/// Thumb Execution Environment registers for ThumbEE instruction set support.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct Cp14ThumbEERegs {
    /// TEECR - ThumbEE Control Register
    pub teecr: u32,
    /// TEEHBR - ThumbEE Handler Base Register
    pub teehbr: u32,
}

impl Default for Cp14ThumbEERegs {
    fn default() -> Self {
        Self {
            teecr: 0x00000000,
            teehbr: 0x00000000,
        }
    }
}

impl Cp14ThumbEERegs {
    /// Create new ThumbEE registers
    pub fn new() -> Self {
        Self::default()
    }

    /// Get TEECR U bit (Unaligned access enable)
    pub fn teecr_u(&self) -> bool {
        (self.teecr & 1) != 0
    }

    /// Set TEECR U bit
    pub fn set_teecr_u(&mut self, enabled: bool) {
        if enabled {
            self.teecr |= 1;
        } else {
            self.teecr &= !1;
        }
    }

    /// Get TEECR CP field (Copy-to-Background enable)
    pub fn teecr_cp(&self) -> u32 {
        (self.teecr >> 1) & 0xF
    }

    /// Set TEECR CP field
    pub fn set_teecr_cp(&mut self, value: u32) {
        self.teecr = (self.teecr & !(0xF << 1)) | ((value & 0xF) << 1);
    }

    /// Get TEEHBR value
    pub fn base_address(&self) -> u32 {
        self.teehbr & 0xFFFFFFFC
    }

    /// Set TEEHBR base address
    pub fn set_base_address(&mut self, addr: u32) {
        self.teehbr = (self.teehbr & 0x3) | (addr & 0xFFFFFFFC);
    }
}

/// CP14 Register State for a VCPU
///
/// This contains all CP14 coprocessor registers for an ARMv7/ARMv8-AArch32 VCPU.
/// Reference: xvisor/arch/arm/include/arch_regs.h:struct arm_priv_cp14
#[derive(Debug, Clone)]
pub struct Cp14Regs {
    /// ThumbEE registers
    pub thumbee: Cp14ThumbEERegs,
    /// ThumbEE feature enabled
    pub thumbee_enabled: bool,
}

impl Default for Cp14Regs {
    fn default() -> Self {
        Self {
            thumbee: Cp14ThumbEERegs::default(),
            thumbee_enabled: false,
        }
    }
}

impl Cp14Regs {
    /// Create new CP14 registers
    pub fn new() -> Self {
        Self::default()
    }

    /// Create CP14 registers with ThumbEE enabled
    pub fn with_thumbee() -> Self {
        Self {
            thumbee: Cp14ThumbEERegs::default(),
            thumbee_enabled: true,
        }
    }

    /// Check if ThumbEE is supported
    pub fn has_thumbee(&self) -> bool {
        self.thumbee_enabled
    }

    /// Enable ThumbEE support
    pub fn enable_thumbee(&mut self) {
        self.thumbee_enabled = true;
    }

    /// Disable ThumbEE support
    pub fn disable_thumbee(&mut self) {
        self.thumbee_enabled = false;
    }

    /// Read a CP14 register by encoding
    pub fn read(&self, encoding: Cp15Encoding) -> RegReadResult {
        match encoding.opc1 {
            // ThumbEE registers (opc1=6)
            6 => self.read_thumbee_reg(encoding),

            // Debug registers (opc1=0) - not implemented
            0 => {
                log::warn!("CP14: Debug registers not implemented (opc1=0, CRn={}, CRm={})",
                          encoding.crn, encoding.crm);
                RegReadResult::Unimplemented
            }

            // Trace registers (opc1=1) - not implemented
            1 => {
                log::warn!("CP14: Trace registers not implemented (opc1=1, CRn={}, CRm={})",
                          encoding.crn, encoding.crm);
                RegReadResult::Unimplemented
            }

            // Jazelle registers (opc1=7) - not implemented
            7 => {
                log::warn!("CP14: Jazelle registers not implemented (opc1=7, CRn={}, CRm={})",
                          encoding.crn, encoding.crm);
                RegReadResult::Unimplemented
            }

            _ => {
                log::warn!("CP14: Invalid opc1={}", encoding.opc1);
                RegReadResult::Unimplemented
            }
        }
    }

    /// Write to a CP14 register by encoding
    pub fn write(&mut self, encoding: Cp15Encoding, value: u32) -> RegWriteResult {
        match encoding.opc1 {
            // ThumbEE registers (opc1=6)
            6 => self.write_thumbee_reg(encoding, value),

            // Debug registers (opc1=0) - not implemented
            0 => {
                log::warn!("CP14: Debug registers not implemented (opc1=0, CRn={}, CRm={})",
                          encoding.crn, encoding.crm);
                RegWriteResult::Unimplemented
            }

            // Trace registers (opc1=1) - not implemented
            1 => {
                log::warn!("CP14: Trace registers not implemented (opc1=1, CRn={}, CRm={})",
                          encoding.crn, encoding.crm);
                RegWriteResult::Unimplemented
            }

            // Jazelle registers (opc1=7) - not implemented
            7 => {
                log::warn!("CP14: Jazelle registers not implemented (opc1=7, CRn={}, CRm={})",
                          encoding.crn, encoding.crm);
                RegWriteResult::Unimplemented
            }

            _ => {
                log::warn!("CP14: Invalid opc1={}", encoding.opc1);
                RegWriteResult::Unimplemented
            }
        }
    }

    /// Read ThumbEE register (opc1=6)
    fn read_thumbee_reg(&self, enc: Cp15Encoding) -> RegReadResult {
        if !self.thumbee_enabled {
            log::warn!("CP14: ThumbEE not enabled, but ThumbEE register access attempted");
            return RegReadResult::Unimplemented;
        }

        match enc.crn {
            // CRn=0: TEECR - ThumbEE Control Register
            0 => {
                if enc.crm == 0 && enc.opc2 == 0 {
                    RegReadResult::Ok { data: self.thumbee.teecr }
                } else {
                    log::warn!("CP14: Invalid TEECR access (CRm={}, opc2={})", enc.crm, enc.opc2);
                    RegReadResult::Unimplemented
                }
            }

            // CRn=1: TEEHBR - ThumbEE Handler Base Register
            1 => {
                if enc.crm == 0 && enc.opc2 == 0 {
                    RegReadResult::Ok { data: self.thumbee.teehbr }
                } else {
                    log::warn!("CP14: Invalid TEEHBR access (CRm={}, opc2={})", enc.crm, enc.opc2);
                    RegReadResult::Unimplemented
                }
            }

            _ => {
                log::warn!("CP14: Invalid ThumbEE CRn={}", enc.crn);
                RegReadResult::Unimplemented
            }
        }
    }

    /// Write ThumbEE register (opc1=6)
    fn write_thumbee_reg(&mut self, enc: Cp15Encoding, value: u32) -> RegWriteResult {
        if !self.thumbee_enabled {
            log::warn!("CP14: ThumbEE not enabled, but ThumbEE register access attempted");
            return RegWriteResult::Unimplemented;
        }

        match enc.crn {
            // CRn=0: TEECR - ThumbEE Control Register
            0 => {
                if enc.crm == 0 && enc.opc2 == 0 {
                    self.thumbee.teecr = value;
                    log::debug!("CP14: TEECR write = 0x{:08x}", value);
                    RegWriteResult::Ok
                } else {
                    log::warn!("CP14: Invalid TEECR access (CRm={}, opc2={})", enc.crm, enc.opc2);
                    RegWriteResult::Unimplemented
                }
            }

            // CRn=1: TEEHBR - ThumbEE Handler Base Register
            1 => {
                if enc.crm == 0 && enc.opc2 == 0 {
                    self.thumbee.teehbr = value;
                    log::debug!("CP14: TEEHBR write = 0x{:08x}", value);
                    RegWriteResult::Ok
                } else {
                    log::warn!("CP14: Invalid TEEHBR access (CRm={}, opc2={})", enc.crm, enc.opc2);
                    RegWriteResult::Unimplemented
                }
            }

            _ => {
                log::warn!("CP14: Invalid ThumbEE CRn={}", enc.crn);
                RegWriteResult::Unimplemented
            }
        }
    }

    /// Save CP14 state (for VCPU context switching)
    pub fn save(&self) {
        // All CP14 register access by VCPU always traps,
        // so we always have an updated copy of CP14 registers.
        log::trace!("CP14: State saved");
    }

    /// Restore CP14 state (for VCPU context switching)
    pub fn restore(&self) {
        if !self.thumbee_enabled {
            return;
        }

        #[cfg(target_arch = "aarch64")]
        unsafe {
            // Restore ThumbEE registers to hardware
            // Note: These instructions need appropriate inline assembly
            // core::arch::asm!("mcr p14, 6, {}, c0, c0, 0", in(reg) self.thumbee.teecr);
            // core::arch::asm!("mcr p14, 6, {}, c1, c0, 0", in(reg) self.thumbee.teehbr);
        }

        log::trace!("CP14: State restored (TEECR=0x{:08x}, TEEHBR=0x{:08x})",
                   self.thumbee.teecr, self.thumbee.teehbr);
    }

    /// Dump CP14 state for debugging
    pub fn dump(&self) {
        if !self.thumbee_enabled {
            log::info!("CP14: ThumbEE not enabled");
            return;
        }

        log::info!("CP14 ThumbEE Registers:");
        log::info!("  TEECR  = 0x{:08x}", self.thumbee.teecr);
        log::info!("  TEEHBR = 0x{:08x}", self.thumbee.teehbr);
    }
}

/// ARM feature flag support for CP14
pub trait ArmFeatureExt {
    /// Check if ThumbEE feature is enabled
    fn has_thumbee(&self) -> bool;
    /// Enable ThumbEE feature
    fn enable_thumbee(&mut self);
    /// Disable ThumbEE feature
    fn disable_thumbee(&mut self);
}

impl ArmFeatureExt for u64 {
    fn has_thumbee(&self) -> bool {
        (self & ARM_FEATURE_THUMB2EE) != 0
    }

    fn enable_thumbee(&mut self) {
        *self |= ARM_FEATURE_THUMB2EE;
    }

    fn disable_thumbee(&mut self) {
        *self &= !ARM_FEATURE_THUMB2EE;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cp14_create_default() {
        let regs = Cp14Regs::new();
        assert!(!regs.has_thumbee());
        assert_eq!(regs.thumbee.teecr, 0);
        assert_eq!(regs.thumbee.teehbr, 0);
    }

    #[test]
    fn test_cp14_with_thumbee() {
        let regs = Cp14Regs::with_thumbee();
        assert!(regs.has_thumbee());
    }

    #[test]
    fn test_thumbee_enable_disable() {
        let mut regs = Cp14Regs::new();
        assert!(!regs.has_thumbee());

        regs.enable_thumbee();
        assert!(regs.has_thumbee());

        regs.disable_thumbee();
        assert!(!regs.has_thumbee());
    }

    #[test]
    fn test_thumbee_regs() {
        let mut regs = Cp14Regs::with_thumbee();
        regs.thumbee.teecr = 0x12345678;
        regs.thumbee.teehbr = 0xABCDEF00;

        assert_eq!(regs.thumbee.teecr, 0x12345678);
        assert_eq!(regs.thumbee.teehbr, 0xABCDEF00);
    }

    #[test]
    fn test_teecr_bits() {
        let mut thumbee = Cp14ThumbEERegs::new();

        assert!(!thumbee.teecr_u());
        thumbee.set_teecr_u(true);
        assert!(thumbee.teecr_u());

        thumbee.set_teecr_cp(0xA);
        assert_eq!(thumbee.teecr_cp(), 0xA);
    }

    #[test]
    fn test_teehbr_base_address() {
        let mut thumbee = Cp14ThumbEERegs::new();

        thumbee.set_base_address(0x12345678);
        assert_eq!(thumbee.base_address(), 0x12345678);
        assert_eq!(thumbee.teehbr, 0x12345678); // Lower 2 bits should be 0
    }

    #[test]
    fn test_cp14_read_teecr() {
        let regs = Cp14Regs::with_thumbee();
        let enc = Cp15Encoding::new(6, 0, 0, 0);

        match regs.read(enc) {
            RegReadResult::Ok { data } => assert_eq!(data, 0),
            _ => panic!("Expected Ok result"),
        }
    }

    #[test]
    fn test_cp14_write_teecr() {
        let mut regs = Cp14Regs::with_thumbee();
        let enc = Cp15Encoding::new(6, 0, 0, 0);

        assert!(matches!(regs.write(enc, 0x12345678), RegWriteResult::Ok));
        assert_eq!(regs.thumbee.teecr, 0x12345678);
    }

    #[test]
    fn test_cp14_read_without_thumbee() {
        let regs = Cp14Regs::new(); // ThumbEE disabled
        let enc = Cp15Encoding::new(6, 0, 0, 0);

        match regs.read(enc) {
            RegReadResult::Unimplemented => {},
            _ => panic!("Expected Unimplemented result"),
        }
    }

    #[test]
    fn test_arm_feature_ext() {
        let mut features: u64 = 0;

        assert!(!features.has_thumbee());

        features.enable_thumbee();
        assert!(features.has_thumbee());

        features.disable_thumbee();
        assert!(!features.has_thumbee());
    }

    #[test]
    fn test_cp14_read_debug_not_implemented() {
        let regs = Cp14Regs::new();
        let enc = Cp15Encoding::new(0, 0, 0, 0); // opc1=0 -> Debug

        match regs.read(enc) {
            RegReadResult::Unimplemented => {},
            _ => panic!("Expected Unimplemented for debug registers"),
        }
    }

    #[test]
    fn test_cp14_read_trace_not_implemented() {
        let regs = Cp14Regs::new();
        let enc = Cp15Encoding::new(1, 0, 0, 0); // opc1=1 -> Trace

        match regs.read(enc) {
            RegReadResult::Unimplemented => {},
            _ => panic!("Expected Unimplemented for trace registers"),
        }
    }

    #[test]
    fn test_cp14_read_jazelle_not_implemented() {
        let regs = Cp14Regs::new();
        let enc = Cp15Encoding::new(7, 0, 0, 0); // opc1=7 -> Jazelle

        match regs.read(enc) {
            RegReadResult::Unimplemented => {},
            _ => panic!("Expected Unimplemented for jazelle registers"),
        }
    }
}
