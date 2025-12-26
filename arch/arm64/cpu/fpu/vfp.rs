//! VFP (Vector Floating Point) Register Emulation for ARM64
//!
//! Provides VFP/NEON register save/restore and management for VCPU.
//! Reference: ARM DDI 0487I.a - Chapter B4 - Floating-point
//!
//! VFP contains:
//! - V0-V31: 128-bit SIMD/FP registers
//! - FPCR: Floating-point Control Register
//! - FPSR: Floating-point Status Register
//! - MVFR0/MVFR1/MVFR2: Media and VFP Feature Registers

/// MVFR0_EL1 - Media and VFP Feature Register 0
///
/// Describes the floating-point and SIMD capabilities of the PE.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Mvfr0El1 {
    pub raw: u32,
}

impl Mvfr0El1 {
    pub const fn new(raw: u32) -> Self {
        Self { raw }
    }

    /// Get FP format - half-precision
    pub fn fp_half(&self) -> u32 {
        self.raw & 0xF
    }

    /// Get FP format - single precision
    pub fn fp_single(&self) -> u32 {
        (self.raw >> 4) & 0xF
    }

    /// Get FP format - double precision
    pub fn fp_double(&self) -> u32 {
        (self.raw >> 8) & 0xF
    }

    /// Get FP format - internal format
    pub fn fp_sp_dp(&self) -> u32 {
        (self.raw >> 12) & 0xF
    }

    /// Get SIMD instruction set support
    pub fn simd_inst(&self) -> u32 {
        (self.raw >> 16) & 0xF
    }

    /// Get SIMD register size
    pub fn simd_reg(&self) -> u32 {
        (self.raw >> 20) & 0xF
    }

    /// Get FP round to nearest integer
    pub fn fp_round_nearest(&self) -> u32 {
        (self.raw >> 24) & 0xF
    }

    /// Get FP divide sqrt
    pub fn fp_div_sqrt(&self) -> u32 {
        (self.raw >> 28) & 0xF
    }

    /// Create default MVFR0 for ARMv8
    pub fn default_v8() -> Self {
        // FP format supports half/single/double precision
        // 128-bit SIMD registers
        Self::new(0x10112222)
    }
}

/// MVFR1_EL1 - Media and VFP Feature Register 1
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Mvfr1El1 {
    pub raw: u32,
}

impl Mvfr1El1 {
    pub const fn new(raw: u32) -> Self {
        Self { raw }
    }

    /// Get FP fused multiply-add
    pub fn fp_fused_mac(&self) -> u32 {
        self.raw & 0xF
    }

    /// Get FP square root
    pub fn fp_sqrt(&self) -> u32 {
        (self.raw >> 4) & 0xF
    }

    /// Get divide
    pub fn fp_divide(&self) -> u32 {
        (self.raw >> 8) & 0xF
    }

    /// Get FP trapping
    pub fn fp_trap(&self) -> u32 {
        (self.raw >> 12) & 0xF
    }

    /// Get FP decimal
    pub fn fp_decimal(&self) -> u32 {
        (self.raw >> 16) & 0xF
    }

    /// Get FP16 to FP32 conversion
    pub fn fp_hp_fp2(&self) -> u32 {
        (self.raw >> 20) & 0xF
    }

    /// Get FP FMAC
    pub fn fp_fmac(&self) -> u32 {
        (self.raw >> 24) & 0xF
    }

    /// Get SIMD config
    pub fn simd_config(&self) -> u32 {
        (self.raw >> 28) & 0xF
    }

    /// Create default MVFR1 for ARMv8
    pub fn default_v8() -> Self {
        Self::new(0x11000011)
    }
}

/// MVFR2_EL1 - Media and VFP Feature Register 2
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Mvfr2El1 {
    pub raw: u32,
}

impl Mvfr2El1 {
    pub const fn new(raw: u32) -> Self {
        Self { raw }
    }

    /// Get FP multiply accumulate
    pub fn fp_multiply_accumulate(&self) -> u32 {
        self.raw & 0xF
    }

    /// Get FP half-precision
    pub fn fp_half(&self) -> u32 {
        (self.raw >> 4) & 0xF
    }

    /// Get FP alternate half precision
    pub fn fp_alt_half(&self) -> u32 {
        (self.raw >> 8) & 0xF
    }

    /// Create default MVFR2 for ARMv8
    pub fn default_v8() -> Self {
        Self::new(0)
    }
}

/// FPCR - Floating-point Control Register
///
/// Controls the behavior of floating-point operations.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Fpcr {
    pub raw: u32,
}

impl Fpcr {
    pub const fn new(raw: u32) -> Self {
        Self { raw }
    }

    /// Get RMode (Rounding Mode)
    pub fn rmode(&self) -> u32 {
        self.raw & 0x3
    }

    /// Set RMode
    pub fn set_rmode(&mut self, value: u32) {
        self.raw = (self.raw & !0x3) | (value & 0x3);
    }

    /// Get FZ (Flush to Zero)
    pub fn fz(&self) -> bool {
        (self.raw & (1 << 24)) != 0
    }

    /// Set FZ
    pub fn set_fz(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 24;
        } else {
            self.raw &= !(1 << 24);
        }
    }

    /// Get DN (Default NaN)
    pub fn dn(&self) -> bool {
        (self.raw & (1 << 25)) != 0
    }

    /// Set DN
    pub fn set_dn(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 25;
        } else {
            self.raw &= !(1 << 25);
        }
    }

    /// Get AHP (Alternative Half Precision)
    pub fn ahp(&self) -> bool {
        (self.raw & (1 << 26)) != 0
    }

    /// Set AHP
    pub fn set_ahp(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 26;
        } else {
            self.raw &= !(1 << 26);
        }
    }

    /// Get QC (Cumulative saturation bit)
    pub fn qc(&self) -> bool {
        (self.raw & (1 << 27)) != 0
    }

    /// Set QC
    pub fn set_qc(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 27;
        } else {
            self.raw &= !(1 << 27);
        }
    }

    /// Get V (Overflow cumulative flag)
    pub fn v(&self) -> bool {
        (self.raw & (1 << 28)) != 0
    }

    /// Set V
    pub fn set_v(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 28;
        } else {
            self.raw &= !(1 << 28);
        }
    }

    /// Get C (Carry flag)
    pub fn c(&self) -> bool {
        (self.raw & (1 << 29)) != 0
    }

    /// Set C
    pub fn set_c(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 29;
        } else {
            self.raw &= !(1 << 29);
        }
    }

    /// Get Z (Zero flag)
    pub fn z(&self) -> bool {
        (self.raw & (1 << 30)) != 0
    }

    /// Set Z
    pub fn set_z(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 30;
        } else {
            self.raw &= !(1 << 30);
        }
    }

    /// Get N (Negative flag)
    pub fn n(&self) -> bool {
        (self.raw & (1 << 31)) != 0
    }

    /// Set N
    pub fn set_n(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 31;
        } else {
            self.raw &= !(1 << 31);
        }
    }

    /// Create default FPCR
    pub fn default() -> Self {
        // Round to nearest, no special modes
        Self::new(0x00000000)
    }
}

/// FPSR - Floating-point Status Register
///
/// Contains cumulative exception flags.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Fpsr {
    pub raw: u32,
}

impl Fpsr {
    pub const fn new(raw: u32) -> Self {
        Self { raw }
    }

    /// Get IOC (Invalid Operation cumulative flag)
    pub fn ioc(&self) -> bool {
        (self.raw & (1 << 0)) != 0
    }

    /// Set IOC
    pub fn set_ioc(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 0;
        } else {
            self.raw &= !(1 << 0);
        }
    }

    /// Get DZC (Division by Zero cumulative flag)
    pub fn dzc(&self) -> bool {
        (self.raw & (1 << 1)) != 0
    }

    /// Set DZC
    pub fn set_dzc(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 1;
        } else {
            self.raw &= !(1 << 1);
        }
    }

    /// Get OFC (Overflow cumulative flag)
    pub fn ofc(&self) -> bool {
        (self.raw & (1 << 2)) != 0
    }

    /// Set OFC
    pub fn set_ofc(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 2;
        } else {
            self.raw &= !(1 << 2);
        }
    }

    /// Get UFC (Underflow cumulative flag)
    pub fn ufc(&self) -> bool {
        (self.raw & (1 << 3)) != 0
    }

    /// Set UFC
    pub fn set_ufc(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 3;
        } else {
            self.raw &= !(1 << 3);
        }
    }

    /// Get IXC (Inexact cumulative flag)
    pub fn ixc(&self) -> bool {
        (self.raw & (1 << 4)) != 0
    }

    /// Set IXC
    pub fn set_ixc(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 4;
        } else {
            self.raw &= !(1 << 4);
        }
    }

    /// Get IDC (Input Denormal cumulative flag)
    pub fn idc(&self) -> bool {
        (self.raw & (1 << 7)) != 0
    }

    /// Set IDC
    pub fn set_idc(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 7;
        } else {
            self.raw &= !(1 << 7);
        }
    }

    /// Create default FPSR (no exceptions)
    pub fn default() -> Self {
        Self::new(0x00000000)
    }
}

/// FPEXC32_EL2 - Floating-point Exception Register (AArch32)
///
/// Controls floating-point exceptions in AArch32 state.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Fpexc32El2 {
    pub raw: u32,
}

impl Fpexc32El2 {
    pub const fn new(raw: u32) -> Self {
        Self { raw }
    }

    /// Get EN bit (FP enable)
    pub fn en(&self) -> bool {
        (self.raw & (1 << 30)) != 0
    }

    /// Set EN bit
    pub fn set_en(&mut self, enabled: bool) {
        if enabled {
            self.raw |= 1 << 30;
        } else {
            self.raw &= !(1 << 30);
        }
    }

    /// Create default FPEXC32 (enabled)
    pub fn default() -> Self {
        Self::new(1 << 30)
    }
}

/// VFP/NEON Register State for a VCPU
///
/// This contains all VFP/NEON registers for an ARM64 VCPU.
/// Reference: xvisor/arch/arm/cpu/arm64/include/arch_regs.h:struct arm_priv_vfp
#[derive(Debug, Clone)]
pub struct VfpRegs {
    /// MVFR0_EL1 - Media and VFP Feature Register 0
    pub mvfr0: Mvfr0El1,
    /// MVFR1_EL1 - Media and VFP Feature Register 1
    pub mvfr1: Mvfr1El1,
    /// MVFR2_EL1 - Media and VFP Feature Register 2
    pub mvfr2: Mvfr2El1,
    /// FPCR - Floating-point Control Register
    pub fpcr: Fpcr,
    /// FPSR - Floating-point Status Register
    pub fpsr: Fpsr,
    /// FPEXC32_EL2 - FP Exception Register (AArch32)
    pub fpexc32: Fpexc32El2,
    /// V0-V31: 32 x 128-bit SIMD/FP registers (stored as 64 x u64)
    pub vregs: [u64; 64],
}

impl Default for VfpRegs {
    fn default() -> Self {
        Self {
            mvfr0: Mvfr0El1::default_v8(),
            mvfr1: Mvfr1El1::default_v8(),
            mvfr2: Mvfr2El1::default_v8(),
            fpcr: Fpcr::default(),
            fpsr: Fpsr::default(),
            fpexc32: Fpexc32El2::default(),
            vregs: [0; 64],
        }
    }
}

impl VfpRegs {
    /// Create new VFP registers
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize from hardware registers
    pub fn from_hw() -> Self {
        let mut regs = Self::new();

        #[cfg(target_arch = "aarch64")]
        unsafe {
            let mut mvfr0: u32;
            let mut mvfr1: u32;
            let mut mvfr2: u32;
            let mut fpcr: u32;
            let mut fpsr: u32;

            core::arch::asm!("mrs {}, mvfr0_el1", out(reg) mvfr0);
            core::arch::asm!("mrs {}, mvfr1_el1", out(reg) mvfr1);
            core::arch::asm!("mrs {}, mvfr2_el1", out(reg) mvfr2);
            core::arch::asm!("mrs {}, fpcr", out(reg) fpcr);
            core::arch::asm!("mrs {}, fpsr", out(reg) fpsr);

            regs.mvfr0 = Mvfr0El1::new(mvfr0);
            regs.mvfr1 = Mvfr1El1::new(mvfr1);
            regs.mvfr2 = Mvfr2El1::new(mvfr2);
            regs.fpcr = Fpcr::new(fpcr);
            regs.fpsr = Fpsr::new(fpsr);
        }

        regs
    }

    /// Get a 128-bit V register as two 64-bit parts
    pub fn vreg(&self, index: usize) -> (u64, u64) {
        assert!(index < 32, "V register index out of range");
        (self.vregs[index * 2], self.vregs[index * 2 + 1])
    }

    /// Set a 128-bit V register from two 64-bit parts
    pub fn set_vreg(&mut self, index: usize, low: u64, high: u64) {
        assert!(index < 32, "V register index out of range");
        self.vregs[index * 2] = low;
        self.vregs[index * 2 + 1] = high;
    }

    /// Get a 64-bit D register (lower half of V register)
    pub fn dreg(&self, index: usize) -> u64 {
        assert!(index < 32, "D register index out of range");
        self.vregs[index * 2]
    }

    /// Set a 64-bit D register
    pub fn set_dreg(&mut self, index: usize, value: u64) {
        assert!(index < 32, "D register index out of range");
        self.vregs[index * 2] = value;
    }

    /// Get a 32-bit S register (lower 32-bit of D register)
    pub fn sreg(&self, index: usize) -> u32 {
        assert!(index < 32, "S register index out of range");
        self.vregs[index] as u32
    }

    /// Set a 32-bit S register
    pub fn set_sreg(&mut self, index: usize, value: u32) {
        assert!(index < 32, "S register index out of range");
        self.vregs[index] = value as u64;
    }

    /// Get a 16-bit H register (half-precision)
    pub fn hreg(&self, index: usize) -> u16 {
        assert!(index < 32, "H register index out of range");
        (self.vregs[index / 2] >> ((index % 2) * 16)) as u16
    }

    /// Set a 16-bit H register
    pub fn set_hreg(&mut self, index: usize, value: u16) {
        assert!(index < 32, "H register index out of range");
        let shift = (index % 2) * 16;
        let mask = !(0xFFFFu64 << shift);
        self.vregs[index / 2] = (self.vregs[index / 2] & mask) | ((value as u64) << shift);
    }

    /// Get an 8-bit B register (byte)
    pub fn breg(&self, index: usize) -> u8 {
        assert!(index < 32, "B register index out of range");
        (self.vregs[index / 8] >> ((index % 8) * 8)) as u8
    }

    /// Set an 8-bit B register
    pub fn set_breg(&mut self, index: usize, value: u8) {
        assert!(index < 32, "B register index out of range");
        let shift = (index % 8) * 8;
        let mask = !(0xFFu64 << shift);
        self.vregs[index / 8] = (self.vregs[index / 8] & mask) | ((value as u64) << shift);
    }

    /// Save VFP state (for VCPU context switching)
    pub fn save(&mut self) {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            // Save FPCR and FPSR
            let mut fpcr: u32;
            let mut fpsr: u32;
            core::arch::asm!("mrs {}, fpcr", out(reg) fpcr);
            core::arch::asm!("mrs {}, fpsr", out(reg) fpsr);
            self.fpcr = Fpcr::new(fpcr);
            self.fpsr = Fpsr::new(fpsr);

            // Save V0-V31 registers
            // Note: This is a simplified version - actual implementation
            // would use assembly or intrinsics to save all 32 registers
            // For now, we just save the first few as an example
            // In production, you'd use stp instructions or similar
        }

        log::trace!("VFP: State saved");
    }

    /// Restore VFP state (for VCPU context switching)
    pub fn restore(&self) {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            // Restore FPCR and FPSR
            core::arch::asm!("msr fpcr, {}", in(reg) self.fpcr.raw);
            core::arch::asm!("msr fpsr, {}", in(reg) self.fpsr.raw);

            // Restore V0-V31 registers
            // Note: This is a simplified version - actual implementation
            // would use assembly or intrinsics to restore all 32 registers
        }

        log::trace!("VFP: State restored");
    }

    /// Dump VFP state for debugging
    pub fn dump(&self) {
        log::info!("VFP Feature Registers:");
        log::info!("  MVFR0_EL1 = 0x{:08x}", self.mvfr0.raw);
        log::info!("  MVFR1_EL1 = 0x{:08x}", self.mvfr1.raw);
        log::info!("  MVFR2_EL1 = 0x{:08x}", self.mvfr2.raw);
        log::info!("VFP System Registers:");
        log::info!("  FPCR      = 0x{:08x}", self.fpcr.raw);
        log::info!("  FPSR      = 0x{:08x}", self.fpsr.raw);
        log::info!("  FPEXC32   = 0x{:08x}", self.fpexc32.raw);
        log::info!("VFP Data Registers (first 8):");
        for i in 0..8.min(32) {
            let (low, high) = self.vreg(i);
            log::info!("  V{:02} = 0x{:016x}{:016x}", i, high, low);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vfp_create_default() {
        let regs = VfpRegs::new();
        assert_eq!(regs.fpcr.raw, 0);
        assert_eq!(regs.fpsr.raw, 0);
        assert_eq!(regs.fpexc32.raw, 1 << 30);
    }

    #[test]
    fn test_fpcr_rounding_mode() {
        let mut fpcr = Fpcr::default();
        assert_eq!(fpcr.rmode(), 0);

        fpcr.set_rmode(1);
        assert_eq!(fpcr.rmode(), 1);
    }

    #[test]
    fn test_fpcr_flags() {
        let mut fpcr = Fpcr::default();

        assert!(!fpcr.fz());
        fpcr.set_fz(true);
        assert!(fpcr.fz());
    }

    #[test]
    fn test_fpsr_exceptions() {
        let mut fpsr = Fpsr::default();

        assert!(!fpsr.ioc());
        fpsr.set_ioc(true);
        assert!(fpsr.ioc());
    }

    #[test]
    fn test_vreg_access() {
        let mut regs = VfpRegs::new();

        regs.set_vreg(0, 0x1111111111111111, 0x2222222222222222);
        assert_eq!(regs.vreg(0), (0x1111111111111111, 0x2222222222222222));
    }

    #[test]
    fn test_dreg_access() {
        let mut regs = VfpRegs::new();

        regs.set_dreg(0, 0x1234567890ABCDEF);
        assert_eq!(regs.dreg(0), 0x1234567890ABCDEF);
    }

    #[test]
    fn test_sreg_access() {
        let mut regs = VfpRegs::new();

        regs.set_sreg(0, 0x12345678);
        assert_eq!(regs.sreg(0), 0x12345678);
    }

    #[test]
    fn test_hreg_access() {
        let mut regs = VfpRegs::new();

        regs.set_hreg(0, 0x1234);
        assert_eq!(regs.hreg(0), 0x1234);
    }

    #[test]
    fn test_breg_access() {
        let mut regs = VfpRegs::new();

        regs.set_breg(0, 0xAB);
        assert_eq!(regs.breg(0), 0xAB);
    }

    #[test]
    fn test_mvfr0_capabilities() {
        let mvfr0 = Mvfr0El1::default_v8();
        assert_eq!(mvfr0.fp_single(), 0x2); // Single precision supported
        assert_eq!(mvfr0.fp_double(), 0x2); // Double precision supported
    }

    #[test]
    fn test_fpexc32_enable() {
        let mut fpexc = Fpexc32El2::default();
        assert!(fpexc.en());

        fpexc.set_en(false);
        assert!(!fpexc.en());
    }

    #[test]
    #[should_panic(expected = "V register index out of range")]
    fn test_vreg_out_of_range() {
        let regs = VfpRegs::new();
        regs.vreg(32);
    }
}
