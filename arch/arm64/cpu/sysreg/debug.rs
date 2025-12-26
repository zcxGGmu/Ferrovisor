//! Debug Register Emulation for ARM64
//!
//! Provides MDSCR_EL1 and debug register emulation for VCPU.
//! Reference: ARM DDI 0487I.a - Chapter D10 - Debug Registers

use crate::arch::arm64::cpu::sysreg::{SysRegEncoding, RegReadResult, RegWriteResult};

/// MDSCR_EL1 - Monitor Debug System Control Register
///
/// Controls debug behavior in the PE.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MdscrEl1 {
    pub raw: u64,
}

impl MdscrEl1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get MDE bit (Monitor Debug Enable)
    pub fn mde(&self) -> bool {
        (self.raw & (1 << 15)) != 0
    }

    /// Set MDE bit
    pub fn set_mde(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 15;
        } else {
            self.raw &= !(1 << 15);
        }
    }

    /// Get SSD bit (Single Step DCC)
    pub fn ssd(&self) -> bool {
        (self.raw & (1 << 16)) != 0
    }

    /// Get MDCR bit (Monitor Debug Config Return)
    pub fn mdcr(&self) -> bool {
        (self.raw & (1 << 17)) != 0
    }

    /// Get TDCC bit (Trap Debug Commits)
    pub fn tdcc(&self) -> bool {
        (self.raw & (1 << 18)) != 0
    }

    /// Set TDCC bit
    pub fn set_tdcc(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 18;
        } else {
            self.raw &= !(1 << 18);
        }
    }

    /// Get HPMN bit (Event Counter Partition Number)
    pub fn hpmn(&self) -> u64 {
        (self.raw >> 11) & 0x1F
    }

    /// Set HPMN bit
    pub fn set_hpmn(&mut self, value: u64) {
        self.raw = (self.raw & !(0x1F << 11)) | ((value & 0x1F) << 11);
    }

    /// Get SCC bit (Self-hosted Communications Control)
    pub fn scc(&self) -> bool {
        (self.raw & (1 << 20)) != 0
    }

    /// Set SCC bit
    pub fn set_scc(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 20;
        } else {
            self.raw &= !(1 << 20);
        }
    }

    /// Get RW bit (Record, Continue, and Restart)
    pub fn rw(&self) -> u64 {
        (self.raw >> 4) & 0xF
    }

    /// Set RW
    pub fn set_rw(&mut self, value: u64) {
        self.raw = (self.raw & !(0xF << 4)) | ((value & 0xF) << 4);
    }

    /// Create default MDSCR
    pub fn default() -> Self {
        Self::new(0x00000000)
    }
}

/// DBGBVR0_EL1 - Breakpoint Value Register 0
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Dbgbvr0El1 {
    pub raw: u64,
}

impl Dbgbvr0El1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get breakpoint address/value
    pub fn value(&self) -> u64 {
        self.raw & 0x0000_FFFF_FFFF_FFFC
    }

    /// Get BVR (Breakpoint Virtual Address)
    pub fn bvr(&self) -> u64 {
        self.raw & 0x0000_FFFF_FFFF_FFFC
    }

    /// Get BAS field (Byte Address Select)
    pub fn bas(&self) -> u64 {
        self.raw & 0xF
    }

    /// Create default breakpoint value
    pub fn default() -> Self {
        Self::new(0x00000000)
    }
}

/// DBGBCR0_EL1 - Breakpoint Control Register 0
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Dbgbcr0El1 {
    pub raw: u64,
}

impl Dbgbcr0El1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get E bit (Enable breakpoint)
    pub fn enabled(&self) -> bool {
        (self.raw & (1 << 0)) != 0
    }

    /// Set E bit
    pub fn set_enabled(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 0;
        } else {
            self.raw &= !(1 << 0);
        }
    }

    /// Get SSC (Security State Control)
    pub fn ssc(&self) -> u64 {
        (self.raw >> 1) & 0x3
    }

    /// Get HMC (Higher mode control)
    pub fn hmc(&self) -> bool {
        (self.raw & (1 << 13)) != 0
    }

    /// Get BT (Breakpoint Type)
    pub fn bt(&self) -> u64 {
        (self.raw >> 20) & 0xF
    }

    /// Get LBN (Linked Breakpoint Number)
    pub fn lbn(&self) -> u64 {
        (self.raw >> 16) & 0xF
    }

    /// Get PMC (Performance Monitor Control)
    pub fn pmc(&self) -> u64 {
        (self.raw >> 24) & 0xF
    }

    /// Create default breakpoint control
    pub fn default() -> Self {
        Self::new(0x00000000)
    }
}

/// DBGWVR0_EL1 - Watchpoint Value Register 0
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Dbgwvr0El1 {
    pub raw: u64,
}

impl Dbgwvr0El1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get watchpoint address/value
    pub fn value(&self) -> u64 {
        self.raw & 0x0000_FFFF_FFFF_FFFC
    }

    /// Create default watchpoint value
    pub fn default() -> Self {
        Self::new(0x00000000)
    }
}

/// DBGWCR0_EL1 - Watchpoint Control Register 0
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Dbgwcr0El1 {
    pub raw: u64,
}

impl Dbgwcr0El1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get E bit (Enable watchpoint)
    pub fn enabled(&self) -> bool {
        (self.raw & (1 << 0)) != 0
    }

    /// Set E bit
    pub fn set_enabled(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 0;
        } else {
            self.raw &= !(1 << 0);
        }
    }

    /// Get SSC (Security State Control)
    pub fn ssc(&self) -> u64 {
        (self.raw >> 1) & 0x3
    }

    /// Get HMC (Higher mode control)
    pub fn hmc(&self) -> bool {
        (self.raw & (1 << 13)) != 0
    }

    /// Get BAS (Byte Address Select)
    pub fn bas(&self) -> u64 {
        (self.raw >> 5) & 0xFF
    }

    /// Get LBN (Linked Watchpoint Number)
    pub fn lbn(&self) -> u64 {
        (self.raw >> 16) & 0xF
    }

    /// Get WT (Watchpoint Type)
    pub fn wt(&self) -> u64 {
        (self.raw >> 20) & 0x3
    }

    /// Get MASK (Address mask)
    pub fn mask(&self) -> u64 {
        (self.raw >> 24) & 0x1F
    }

    /// Create default watchpoint control
    pub fn default() -> Self {
        Self::new(0x00000000)
    }
}

/// MDCCINT_EL1 - Monitor Debug Commits Interrupt Enable Register
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MdccintEl1 {
    pub raw: u64,
}

impl MdccintEl1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get HWE (Halting Debug Event enable)
    pub fn hwe(&self) -> bool {
        (self.raw & (1 << 0)) != 0
    }

    /// Set HWE bit
    pub fn set_hwe(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 0;
        } else {
            self.raw &= !(1 << 0);
        }
    }

    /// Get SME (Secure Monitor Enable)
    pub fn sme(&self) -> bool {
        (self.raw & (1 << 1)) != 0
    }

    /// Create default MDCCINT
    pub fn default() -> Self {
        Self::new(0x00000000)
    }
}

/// OSDTRRX_EL1 - OS Double-Tap Register
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct OsdtrrxEl1 {
    pub raw: u64,
}

impl OsdtrrxEl1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get TX (Transfer data)
    pub fn tx(&self) -> u32 {
        self.raw as u32
    }

    /// Set TX
    pub fn set_tx(&mut self, value: u32) {
        self.raw = value as u64;
    }

    /// Create default OSDTRRX
    pub fn default() -> Self {
        Self::new(0x00000000)
    }
}

/// Debug register state for a VCPU
#[derive(Debug, Clone)]
pub struct DebugRegs {
    /// MDSCR_EL1
    pub mdscr: MdscrEl1,
    /// MDCCINT_EL1
    pub mdccint: MdccintEl1,
    /// OSDTRRX_EL1
    pub osdtrrx: OsdtrrxEl1,
    /// Breakpoint value registers (simplified, only 0)
    pub dbgbvr0: Dbgbvr0El1,
    /// Breakpoint control registers (simplified, only 0)
    pub dbgbcr0: Dbgbcr0El1,
    /// Watchpoint value registers (simplified, only 0)
    pub dbgwvr0: Dbgwvr0El1,
    /// Watchpoint control registers (simplified, only 0)
    pub dbgwcr0: Dbgwcr0El1,
}

impl Default for DebugRegs {
    fn default() -> Self {
        Self {
            mdscr: MdscrEl1::default(),
            mdccint: MdccintEl1::default(),
            osdtrrx: OsdtrrxEl1::default(),
            dbgbvr0: Dbgbvr0El1::default(),
            dbgbcr0: Dbgbcr0El1::default(),
            dbgwvr0: Dbgwvr0El1::default(),
            dbgwcr0: Dbgwcr0El1::default(),
        }
    }
}

impl DebugRegs {
    /// Create new debug registers with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize from hardware registers
    pub fn from_hw() -> Self {
        let mut regs = Self::new();

        #[cfg(target_arch = "aarch64")]
        unsafe {
            let mut mdscr: u64;
            core::arch::asm!("mrs {}, mdscr_el1", out(reg) mdscr);
            regs.mdscr = MdscrEl1::new(mdscr);
        }

        regs
    }

    /// Read a debug register by encoding
    pub fn read_debug_reg(&self, encoding: SysRegEncoding) -> RegReadResult {
        match (encoding.op0, encoding.op1, encoding.crn, encoding.crm, encoding.op2) {
            // MDSCR_EL1: op0=2, op1=0, crn=0, crm=2, op2=2
            (2, 0, 0, 2, 2) => RegReadResult::Ok { data: self.mdscr.raw },
            // MDCCINT_EL1: op0=2, op1=0, crn=0, crm=2, op2=0
            (2, 0, 0, 2, 0) => RegReadResult::Ok { data: self.mdccint.raw },
            // OSDTRRX_EL1: op0=2, op1=0, crn=0, crm=3, op2=0
            (2, 0, 0, 3, 0) => RegReadResult::Ok { data: self.osdtrrx.raw },
            // DBGBCR0_EL1: op0=2, op1=0, crn=0, crm=0, op2=5
            (2, 0, 0, 0, 5) => RegReadResult::Ok { data: self.dbgbcr0.raw },
            // DBGWVR0_EL1: op0=2, op1=0, crn=0, crm=6, op2=0
            (2, 0, 0, 6, 0) => RegReadResult::Ok { data: self.dbgwvr0.raw },
            // DBGWCR0_EL1: op0=2, op1=0, crn=0, crm=6, op2=1
            (2, 0, 0, 6, 1) => RegReadResult::Ok { data: self.dbgwcr0.raw },
            _ => RegReadResult::Unimplemented,
        }
    }

    /// Write to a debug register
    pub fn write_debug_reg(&mut self, encoding: SysRegEncoding, value: u64) -> RegWriteResult {
        match (encoding.op0, encoding.op1, encoding.crn, encoding.crm, encoding.op2) {
            // MDSCR_EL1
            (2, 0, 0, 2, 2) => {
                self.mdscr = MdscrEl1::new(value);
                RegWriteResult::Ok
            }
            // MDCCINT_EL1
            (2, 0, 0, 2, 0) => {
                self.mdccint = MdccintEl1::new(value);
                RegWriteResult::Ok
            }
            // OSDTRRX_EL1
            (2, 0, 0, 3, 0) => {
                self.osdtrrx = OsdtrrxEl1::new(value);
                RegWriteResult::Ok
            }
            // DBGBCR0_EL1
            (2, 0, 0, 0, 5) => {
                self.dbgbcr0 = Dbgbcr0El1::new(value);
                RegWriteResult::Ok
            }
            // DBGWVR0_EL1
            (2, 0, 0, 6, 0) => {
                self.dbgwvr0 = Dbgwvr0El1::new(value);
                RegWriteResult::Ok
            }
            // DBGWCR0_EL1
            (2, 0, 0, 6, 1) => {
                self.dbgwcr0 = Dbgwcr0El1::new(value);
                RegWriteResult::Ok
            }
            _ => RegWriteResult::Unimplemented,
        }
    }

    /// Enable debug monitoring
    pub fn enable_monitoring(&mut self) {
        self.mdscr.set_mde(true);
    }

    /// Disable debug monitoring
    pub fn disable_monitoring(&mut self) {
        self.mdscr.set_mde(false);
    }

    /// Check if monitoring is enabled
    pub fn is_monitoring_enabled(&self) -> bool {
        self.mdscr.mde()
    }

    /// Enable single stepping
    pub fn enable_single_step(&mut self) {
        self.mdscr.set_ssd(true);
    }

    /// Disable single stepping
    pub fn disable_single_step(&mut self) {
        self.mdscr.set_ssd(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mdscr_creation() {
        let mdscr = MdscrEl1::default();
        assert!(!mdscr.mde()); // Monitoring disabled by default
    }

    #[test]
    fn test_mdscr_enable_mde() {
        let mut mdscr = MdscrEl1::default();
        assert!(!mdscr.mde());

        mdscr.set_mde(true);
        assert!(mdscr.mde());
    }

    #[test]
    fn test_breakpoint_control() {
        let bcr = Dbgbcr0El1::default();
        assert!(!bcr.enabled()); // Breakpoint disabled by default
    }

    #[test]
    fn test_watchpoint_control() {
        let wcr = Dbgwcr0El1::default();
        assert!(!wcr.enabled()); // Watchpoint disabled by default
    }

    #[test]
    fn test_debug_regs_read() {
        let regs = DebugRegs::new();
        let encoding = SysRegEncoding {
            op0: 2, op1: 0, crn: 0, crm: 2, op2: 2
        };

        match regs.read_debug_reg(encoding) {
            RegReadResult::Ok { data } => assert_eq!(data, regs.mdscr.raw),
            _ => panic!("Expected Ok result"),
        }
    }

    #[test]
    fn test_debug_regs_write() {
        let mut regs = DebugRegs::new();
        let encoding = SysRegEncoding {
            op0: 2, op1: 0, crn: 0, crm: 2, op2: 2
        };

        assert!(matches!(regs.write_debug_reg(encoding, 0x12345678),
                         RegWriteResult::Ok));
        assert_eq!(regs.mdscr.raw, 0x12345678);
    }

    #[test]
    fn test_enable_monitoring() {
        let mut regs = DebugRegs::new();
        assert!(!regs.is_monitoring_enabled());

        regs.enable_monitoring();
        assert!(regs.is_monitoring_enabled());
    }
}
