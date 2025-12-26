//! Memory Management Register Emulation for ARM64
//!
//! Provides TTBR0_EL1, TTBR1_EL1, TCR_EL1, MAIR_EL1, and AMAIR_EL1 emulation.
//! Reference: ARM DDI 0487I.a - Chapter D11 - Virtual Memory Control

use crate::arch::arm64::cpu::sysreg::{SysRegEncoding, RegReadResult, RegWriteResult};

/// TTBR0_EL1 - Translation Table Base Register 0
///
/// Holds the base address of the translation table for VA range [0x0000000000000000, 0x0000FFFFFFFFFFFF].
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Ttbr0El1 {
    pub raw: u64,
}

impl Ttbr0El1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Create TTBR0 from fields
    pub fn from_fields(baddr: u64, asid: u16) -> Self {
        let raw = (baddr & 0x0000_FFFF_FFFF_F000) | ((asid as u64) << 48);
        Self { raw }
    }

    /// Get base address of translation table
    pub fn baddr(&self) -> u64 {
        self.raw & 0x0000_FFFF_FFFF_F000
    }

    /// Set base address
    pub fn set_baddr(&mut self, addr: u64) {
        self.raw = (self.raw & 0xFFFF_F000_0000_FFFF) | (addr & 0x0000_FFFF_FFFF_F000);
    }

    /// Get ASID (Address Space Identifier)
    pub fn asid(&self) -> u16 {
        ((self.raw >> 48) & 0xFFFF) as u16
    }

    /// Set ASID
    pub fn set_asid(&mut self, asid: u16) {
        self.raw = (self.raw & 0x0000_FFFF_FFFF_FFFF) | ((asid as u64) << 48);
    }

    /// Create default TTBR0
    pub fn default() -> Self {
        Self::new(0x00000000)
    }
}

/// TTBR1_EL1 - Translation Table Base Register 1
///
/// Holds the base address of the translation table for VA range [0xFFFF000000000000, 0xFFFFFFFFFFFFFFFF].
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Ttbr1El1 {
    pub raw: u64,
}

impl Ttbr1El1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Create TTBR1 from fields
    pub fn from_fields(baddr: u64, asid: u16) -> Self {
        let raw = (baddr & 0x0000_FFFF_FFFF_F000) | ((asid as u64) << 48);
        Self { raw }
    }

    /// Get base address of translation table
    pub fn baddr(&self) -> u64 {
        self.raw & 0x0000_FFFF_FFFF_F000
    }

    /// Set base address
    pub fn set_baddr(&mut self, addr: u64) {
        self.raw = (self.raw & 0xFFFF_F000_0000_FFFF) | (addr & 0x0000_FFFF_FFFF_F000);
    }

    /// Get ASID (Address Space Identifier)
    pub fn asid(&self) -> u16 {
        ((self.raw >> 48) & 0xFFFF) as u16
    }

    /// Set ASID
    pub fn set_asid(&mut self, asid: u16) {
        self.raw = (self.raw & 0x0000_FFFF_FFFF_FFFF) | ((asid as u64) << 48);
    }

    /// Create default TTBR1
    pub fn default() -> Self {
        Self::new(0x00000000)
    }
}

/// TCR_EL1 - Translation Control Register
///
/// Controls translations for both TTBR0 and TTBR1 regions.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct TcrEl1 {
    pub raw: u64,
}

impl TcrEl1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get T0SZ (Size offset for TTBR0)
    pub fn t0sz(&self) -> u64 {
        self.raw & 0x3F
    }

    /// Set T0SZ
    pub fn set_t0sz(&mut self, value: u64) {
        self.raw = (self.raw & !0x3F) | (value & 0x3F);
    }

    /// Get T1SZ (Size offset for TTBR1)
    pub fn t1sz(&self) -> u64 {
        (self.raw >> 16) & 0x3F
    }

    /// Set T1SZ
    pub fn set_t1sz(&mut self, value: u64) {
        self.raw = (self.raw & !(0x3F << 16)) | ((value & 0x3F) << 16);
    }

    /// Get TG0 (Granule size for TTBR0)
    pub fn tg0(&self) -> u64 {
        (self.raw >> 14) & 0x3
    }

    /// Set TG0
    /// 0 = 4KB, 1 = 64KB, 2 = 16KB
    pub fn set_tg0(&mut self, value: u64) {
        self.raw = (self.raw & !(0x3 << 14)) | ((value & 0x3) << 14);
    }

    /// Get TG1 (Granule size for TTBR1)
    pub fn tg1(&self) -> u64 {
        (self.raw >> 30) & 0x3
    }

    /// Set TG1
    pub fn set_tg1(&mut self, value: u64) {
        self.raw = (self.raw & !(0x3 << 30)) | ((value & 0x3) << 30);
    }

    /// Get EPD0 (Disable TTBR0 walks)
    pub fn epd0(&self) -> bool {
        (self.raw & (1 << 7)) != 0
    }

    /// Set EPD0
    pub fn set_epd0(&mut self, disabled: bool) {
        if disabled {
            self.raw |= 1 << 7;
        } else {
            self.raw &= !(1 << 7);
        }
    }

    /// Get EPD1 (Disable TTBR1 walks)
    pub fn epd1(&self) -> bool {
        (self.raw & (1 << 23)) != 0
    }

    /// Set EPD1
    pub fn set_epd1(&mut self, disabled: bool) {
        if disabled {
            self.raw |= 1 << 23;
        } else {
            self.raw &= !(1 << 23);
        }
    }

    /// Get IRGN0 (Inner region attributes for TTBR0)
    pub fn irgn0(&self) -> u64 {
        (self.raw >> 8) & 0x3
    }

    /// Set IRGN0
    pub fn set_irgn0(&mut self, value: u64) {
        self.raw = (self.raw & !(0x3 << 8)) | ((value & 0x3) << 8);
    }

    /// Get ORGN0 (Outer region attributes for TTBR0)
    pub fn orgn0(&self) -> u64 {
        (self.raw >> 10) & 0x3
    }

    /// Set ORGN0
    pub fn set_orgn0(&mut self, value: u64) {
        self.raw = (self.raw & !(0x3 << 10)) | ((value & 0x3) << 10);
    }

    /// Get SH0 (Shareability for TTBR0)
    pub fn sh0(&self) -> u64 {
        (self.raw >> 12) & 0x3
    }

    /// Set SH0
    pub fn set_sh0(&mut self, value: u64) {
        self.raw = (self.raw & !(0x3 << 12)) | ((value & 0x3) << 12);
    }

    /// Get IRGN1 (Inner region attributes for TTBR1)
    pub fn irgn1(&self) -> u64 {
        (self.raw >> 24) & 0x3
    }

    /// Set IRGN1
    pub fn set_irgn1(&mut self, value: u64) {
        self.raw = (self.raw & !(0x3 << 24)) | ((value & 0x3) << 24);
    }

    /// Get ORGN1 (Outer region attributes for TTBR1)
    pub fn orgn1(&self) -> u64 {
        (self.raw >> 26) & 0x3
    }

    /// Set ORGN1
    pub fn set_orgn1(&mut self, value: u64) {
        self.raw = (self.raw & !(0x3 << 26)) | ((value & 0x3) << 26);
    }

    /// Get SH1 (Shareability for TTBR1)
    pub fn sh1(&self) -> u64 {
        (self.raw >> 28) & 0x3
    }

    /// Set SH1
    pub fn set_sh1(&mut self, value: u64) {
        self.raw = (self.raw & !(0x3 << 28)) | ((value & 0x3) << 28);
    }

    /// Get AS (16-bit ASID enable)
    pub fn as_16bit(&self) -> bool {
        (self.raw & (1 << 36)) != 0
    }

    /// Set AS bit
    pub fn set_as_16bit(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 36;
        } else {
            self.raw &= !(1 << 36);
        }
    }

    /// Get TBI0 (Top Byte Ignore for TTBR0)
    pub fn tbi0(&self) -> bool {
        (self.raw & (1 << 37)) != 0
    }

    /// Set TBI0
    pub fn set_tbi0(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 37;
        } else {
            self.raw &= !(1 << 37);
        }
    }

    /// Get TBI1 (Top Byte Ignore for TTBR1)
    pub fn tbi1(&self) -> bool {
        (self.raw & (1 << 38)) != 0
    }

    /// Set TBI1
    pub fn set_tbi1(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 38;
        } else {
            self.raw &= !(1 << 38);
        }
    }

    /// Create default TCR for 48-bit VA with 4KB granule
    pub fn default_48bit() -> Self {
        // T0SZ=16, T1SZ=16, TG0=0 (4KB), TG1=1 (64KB), EPD1=1 (disable upper)
        // IRGN0=1 (WB-WA), ORGN0=1 (WB-WA), SH0=3 (Inner)
        Self::new(0x0055_5151_0040)
    }

    /// Create default TCR for 40-bit VA
    pub fn default_40bit() -> Self {
        // T0SZ=24, T1SZ=24, TG0=0 (4KB), TG1=1 (64KB), EPD1=1
        Self::new(0x0055_5551_0040)
    }

    /// Create default TCR
    pub fn default() -> Self {
        Self::default_48bit()
    }
}

/// MAIR_EL1 - Memory Attribute Indirection Register
///
/// Provides memory attribute encodings for page table walks.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MairEl1 {
    pub raw: u64,
}

impl MairEl1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get attribute value for index 0-7
    pub fn attr(&self, index: u64) -> u8 {
        ((self.raw >> (index * 8)) & 0xFF) as u8
    }

    /// Set attribute value for index
    pub fn set_attr(&mut self, index: u64, value: u8) {
        let shift = index * 8;
        self.raw = (self.raw & !(0xFF << shift)) | ((value as u64) << shift);
    }

    /// Create default MAIR with standard attributes
    ///
    /// Attr0: Device-nGnRnE
    /// Attr1: Normal WB-WA
    /// Attr2: Normal WT
    /// Attr3: Normal NC
    pub fn default() -> Self {
        // Attr0 = 0x00 (Device-nGnRnE)
        // Attr1 = 0xFF (Normal WB-WA, Inner/Outer)
        // Attr2 = 0xBB (Normal WT)
        // Attr3 = 0x44 (Normal NC)
        Self::new(0xFF4400BB)
    }

    /// Device-nGnRnE attribute
    pub const DEVICE_NGNRNE: u8 = 0x00;

    /// Device-nGnRE attribute
    pub const DEVICE_NGNRE: u8 = 0x04;

    /// Device-GRE attribute
    pub const DEVICE_GRE: u8 = 0x0C;

    /// Normal WB-WA Inner/Outer attribute
    pub const NORMAL_WB_WA: u8 = 0xFF;

    /// Normal WT Inner/Outer attribute
    pub const NORMAL_WT: u8 = 0xBB;

    /// Normal NC attribute
    pub const NORMAL_NC: u8 = 0x44;
}

/// AMAIR_EL1 - Auxiliary Memory Attribute Indirection Register
///
/// Implementation-defined memory attributes.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct AmairEl1 {
    pub raw: u64,
}

impl AmairEl1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get attribute value for index 0-7
    pub fn attr(&self, index: u64) -> u8 {
        ((self.raw >> (index * 8)) & 0xFF) as u8
    }

    /// Set attribute value for index
    pub fn set_attr(&mut self, index: u64, value: u8) {
        let shift = index * 8;
        self.raw = (self.raw & !(0xFF << shift)) | ((value as u64) << shift);
    }

    /// Create default AMAIR (implementation-specific)
    pub fn default() -> Self {
        Self::new(0x00000000)
    }
}

/// Memory management register state for a VCPU
#[derive(Debug, Clone)]
pub struct MemoryMgmtRegs {
    /// TTBR0_EL1
    pub ttbr0: Ttbr0El1,
    /// TTBR1_EL1
    pub ttbr1: Ttbr1El1,
    /// TCR_EL1
    pub tcr: TcrEl1,
    /// MAIR_EL1
    pub mair: MairEl1,
    /// AMAIR_EL1
    pub amair: AmairEl1,
}

impl Default for MemoryMgmtRegs {
    fn default() -> Self {
        Self {
            ttbr0: Ttbr0El1::default(),
            ttbr1: Ttbr1El1::default(),
            tcr: TcrEl1::default(),
            mair: MairEl1::default(),
            amair: AmairEl1::default(),
        }
    }
}

impl MemoryMgmtRegs {
    /// Create new memory management registers with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize from hardware registers
    pub fn from_hw() -> Self {
        let mut regs = Self::new();

        #[cfg(target_arch = "aarch64")]
        unsafe {
            let mut ttbr0: u64;
            let mut ttbr1: u64;
            let mut tcr: u64;
            let mut mair: u64;
            let mut amair: u64;

            core::arch::asm!("mrs {}, ttbr0_el1", out(reg) ttbr0);
            core::arch::asm!("mrs {}, ttbr1_el1", out(reg) ttbr1);
            core::arch::asm!("mrs {}, tcr_el1", out(reg) tcr);
            core::arch::asm!("mrs {}, mair_el1", out(reg) mair);
            core::arch::asm!("mrs {}, amair_el1", out(reg) amair);

            regs.ttbr0 = Ttbr0El1::new(ttbr0);
            regs.ttbr1 = Ttbr1El1::new(ttbr1);
            regs.tcr = TcrEl1::new(tcr);
            regs.mair = MairEl1::new(mair);
            regs.amair = AmairEl1::new(amair);
        }

        regs
    }

    /// Read a memory management register by encoding
    pub fn read_mm_reg(&self, encoding: SysRegEncoding) -> RegReadResult {
        match (encoding.op0, encoding.op1, encoding.crn, encoding.crm, encoding.op2) {
            // TTBR0_EL1: op0=3, op1=0, crn=2, crm=0, op2=0
            (3, 0, 2, 0, 0) => RegReadResult::Ok { data: self.ttbr0.raw },
            // TTBR1_EL1: op0=3, op1=0, crn=2, crm=0, op2=1
            (3, 0, 2, 0, 1) => RegReadResult::Ok { data: self.ttbr1.raw },
            // TCR_EL1: op0=3, op1=0, crn=2, crm=0, op2=2
            (3, 0, 2, 0, 2) => RegReadResult::Ok { data: self.tcr.raw },
            // MAIR_EL1: op0=3, op1=0, crn=10, crm=2, op2=0
            (3, 0, 10, 2, 0) => RegReadResult::Ok { data: self.mair.raw },
            // AMAIR_EL1: op0=3, op1=0, crn=10, crm=3, op2=0
            (3, 0, 10, 3, 0) => RegReadResult::Ok { data: self.amair.raw },
            _ => RegReadResult::Unimplemented,
        }
    }

    /// Write to a memory management register
    pub fn write_mm_reg(&mut self, encoding: SysRegEncoding, value: u64) -> RegWriteResult {
        match (encoding.op0, encoding.op1, encoding.crn, encoding.crm, encoding.op2) {
            // TTBR0_EL1
            (3, 0, 2, 0, 0) => {
                self.ttbr0 = Ttbr0El1::new(value);
                RegWriteResult::Ok
            }
            // TTBR1_EL1
            (3, 0, 2, 0, 1) => {
                self.ttbr1 = Ttbr1El1::new(value);
                RegWriteResult::Ok
            }
            // TCR_EL1
            (3, 0, 2, 0, 2) => {
                self.tcr = TcrEl1::new(value);
                RegWriteResult::Ok
            }
            // MAIR_EL1
            (3, 0, 10, 2, 0) => {
                self.mair = MairEl1::new(value);
                RegWriteResult::Ok
            }
            // AMAIR_EL1
            (3, 0, 10, 3, 0) => {
                self.amair = AmairEl1::new(value);
                RegWriteResult::Ok
            }
            _ => RegWriteResult::Unimplemented,
        }
    }

    /// Invalidate TLB for this VCPU (helper function)
    pub fn invalidate_tlb(&self) {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            // TLBI ASIDE1IS - Invalidate EL1&0 TLBs
            core::arch::asm!("tlbi aside1is");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ttbr_creation() {
        let ttbr0 = Ttbr0El1::from_fields(0x4000_0000, 0x1234);
        assert_eq!(ttbr0.baddr(), 0x4000_0000);
        assert_eq!(ttbr0.asid(), 0x1234);
    }

    #[test]
    fn test_tcr_defaults() {
        let tcr = TcrEl1::default();
        assert_eq!(tcr.t0sz(), 16); // 48-bit VA
        assert_eq!(tcr.t1sz(), 16);
        assert!(!tcr.tbi0()); // TBI disabled
    }

    #[test]
    fn test_mair_defaults() {
        let mair = MairEl1::default();
        assert_eq!(mair.attr(0), MairEl1::DEVICE_NGNRNE);
        assert_eq!(mair.attr(1), MairEl1::NORMAL_WB_WA);
    }

    #[test]
    fn test_mm_regs_read() {
        let regs = MemoryMgmtRegs::new();
        let encoding = SysRegEncoding {
            op0: 3, op1: 0, crn: 2, crm: 0, op2: 2
        };

        match regs.read_mm_reg(encoding) {
            RegReadResult::Ok { data } => assert_eq!(data, regs.tcr.raw),
            _ => panic!("Expected Ok result"),
        }
    }

    #[test]
    fn test_mm_regs_write() {
        let mut regs = MemoryMgmtRegs::new();
        let encoding = SysRegEncoding {
            op0: 3, op1: 0, crn: 2, crm: 0, op2: 2
        };

        assert!(matches!(regs.write_mm_reg(encoding, 0x12345678),
                         RegWriteResult::Ok));
        assert_eq!(regs.tcr.raw, 0x12345678);
    }

    #[test]
    fn test_tcr_enable_tbi() {
        let mut tcr = TcrEl1::default();
        assert!(!tcr.tbi0());

        tcr.set_tbi0(true);
        assert!(tcr.tbi0());
    }
}
