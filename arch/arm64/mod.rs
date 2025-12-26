//! ARM64 (AArch64) Architecture Support for Ferrovisor
//!
//! This module provides ARM64 architecture support including:
//! - CPU register and system register management
//! - MMU and memory management (Stage-2 translation)
//! - Interrupt and exception handling (GIC/VGIC)
//! - Virtualization extensions (EL2)
//! - SMP support (PSCI, Spin Table)
//! - Device tree support
//! - Timer support (Generic Timer)
//!
//! ## Architecture Overview
//!
//! ARM64 uses the AArch64 execution state with the following exception levels:
//! - EL0: Application level (User)
//! - EL1: OS kernel level (Supervisor)
//! - EL2: Hypervisor level (for virtualization)
//! - EL3: Secure monitor level (for TrustZone)
//!
//! Ferrovisor runs at EL2 to provide hardware-assisted virtualization.
//!
//! ## References
//! - [ARM Architecture Reference Manual ARMv8-A](https://developer.arm.com/documentation/ddi0487/latest)
//! - [ARM Generic Interrupt Controller Architecture Specification](https://developer.arm.com/documentation/ihi0069/latest)
//! - [Xvisor ARM Implementation](https://github.com/xvisor/xvisor)

pub mod cpu;
pub mod mmu;
pub mod interrupt;
pub mod smp;
pub mod platform;
pub mod psci;

// Re-export key types and functions
pub use cpu::*;
pub use mmu::*;
pub use interrupt::*;
pub use smp::*;
pub use platform::*;
pub use psci::*;

/// ARM64 architecture version
pub const ARCH_VERSION: &str = "arm64";

/// ARM64 physical address width (48-bit by default)
pub const PA_WIDTH: usize = 48;

/// ARM64 virtual address width (48-bit by default)
pub const VA_WIDTH: usize = 48;

/// Intermediate Physical Address width for Stage-2 translation
pub const IPA_WIDTH: usize = 40;

/// Page size (4KB)
pub const PAGE_SIZE: usize = 4096;

/// Page shift
pub const PAGE_SHIFT: usize = 12;

/// Number of page table levels for Stage-2 translation
pub const STAGE2_LEVELS: usize = 3;

/// Maximum number of CPUs supported
pub const MAX_CPUS: usize = 8;

/// ARM64 exception levels (EL)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ExceptionLevel {
    /// EL0 - Application level
    EL0 = 0,
    /// EL1 - OS kernel level
    EL1 = 1,
    /// EL2 - Hypervisor level
    EL2 = 2,
    /// EL3 - Secure monitor level
    EL3 = 3,
}

/// ARM64 PSTATE (Processor State) flags
bitflags::bitflags! {
    /// PSTATE flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PStateFlags: u64 {
        /// Negative condition flag
        const N = 1 << 31;
        /// Zero condition flag
        const Z = 1 << 30;
        /// Carry condition flag
        const C = 1 << 29;
        /// Overflow condition flag
        const V = 1 << 28;
        /// Debug mask bit
        const D = 1 << 9;
        /// Asynchronous abort mask bit
        const A = 1 << 8;
        /// IRQ mask bit
        const I = 1 << 7;
        /// FIQ mask bit
        const F = 1 << 6;
    }
}

/// ARM64 exception syndrome class
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ExceptionClass {
    /// Trapped MSR, MRS, or System instruction execution
    MsrMrsSystemInstruction = 0b000000,
    /// Trapped access to SVE/SIMD/FPU registers
    SimdFp = 0b000111,
    /// Trapped execution of MRS or MSR to EL3
    MsrMrsEl3 = 0b001001,
    /// Access to SVE functionality
    Sve = 0b001011,
    /// Trapped execution of MRS or MSR to EL2
    MsrMrsEl2 = 0b001101,
    /// Trapped execution of HVC instruction
    Hvc = 0b010110,
    /// Trapped MRS or MSR access to trace registers
    Trc = 0b011000,
    /// Trapped execution of SMC instruction
    Smc = 0b011111,
    /// Trapped execution of MRS or MSR to EL1
    MsrMrsEl1 = 0b100000,
    /// Trapped execution of EVT instruction
    Evt = 0b100100,
    /// Trapped IC IVAU instruction
    IcIvalu = 0b100101,
    /// Trapped DC CVAC, DC CVAP, or DC CVAU instruction
    DcCvau = 0b100110,
    /// Trapped DC CIVAC instruction
    DcCivac = 0b100111,
    /// Trapped DC ZVA instruction
    DcZva = 0b101000,
    /// Trapped access to feature registers
    FeatureTrap = 0b101101,
    /// Trapped execution of BRK instruction
    Brk = 0b111000,
    /// Trapped execution of other instructions
    Other = 0b111111,
    /// UNK encoding in condition code field
    Unknown = 0b000010,
}

/// ARM64 exception codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ExceptionCode {
    /// Exception from Current EL with SPx (same SP)
    ExceptionSameSp = 0x0,
    /// Exception from Current EL with SPx (different SP)
    ExceptionDiffSp = 0x1,
    /// Exception from lower EL using AArch64
    LowerELAArch64 = 0x2,
    /// Exception from lower EL using AArch32
    LowerELAArch32 = 0x3,
}

/// ARM64 interrupt types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum InterruptType {
    /// IRQ (normal interrupt)
    IRQ = 0x0,
    /// FIQ (fast interrupt)
    FIQ = 0x1,
    /// SError (system error)
    SError = 0x2,
}

/// System register encoding (Op0, Op1, CRn, CRm, Op2)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemRegEncoding {
    /// Op0 field
    pub op0: u8,
    /// Op1 field
    pub op1: u8,
    /// CRn field
    pub crn: u8,
    /// CRm field
    pub crm: u8,
    /// Op2 field
    pub op2: u8,
}

impl SystemRegEncoding {
    /// Create a new system register encoding
    pub const fn new(op0: u8, op1: u8, crn: u8, crm: u8, op2: u8) -> Self {
        Self {
            op0,
            op1,
            crn,
            crm,
            op2,
        }
    }

    /// Encode as a 32-bit value
    pub const fn encode(&self) -> u32 {
        ((self.op0 as u32) << 14)
            | ((self.op1 as u32) << 11)
            | ((self.crn as u32) << 7)
            | ((self.crm as u32) << 3)
            | (self.op2 as u32)
    }
}

/// EL2 system register addresses (CRm=0 for readability)
pub mod el2_regs {
    use super::SystemRegEncoding;

    /// HCR_EL2 - Hypervisor Configuration Register
    pub const HCR_EL2: SystemRegEncoding = SystemRegEncoding::new(3, 0, 4, 1, 0);

    /// VTTBR_EL2 - Virtualization Translation Table Base Register
    pub const VTTBR_EL2: SystemRegEncoding = SystemRegEncoding::new(3, 0, 2, 1, 0);

    /// VTCR_EL2 - Virtualization Translation Control Register
    pub const VTCR_EL2: SystemRegEncoding = SystemRegEncoding::new(3, 0, 2, 1, 2);

    /// SCTLR_EL2 - System Control Register (EL2)
    pub const SCTLR_EL2: SystemRegEncoding = SystemRegEncoding::new(3, 0, 1, 0, 0);

    /// CPTR_EL2 - Architectural Feature Trap Register (EL2)
    pub const CPTR_EL2: SystemRegEncoding = SystemRegEncoding::new(3, 0, 1, 1, 2);

    /// HSTR_EL2 - Hypervisor System Trap Register
    pub const HSTR_EL2: SystemRegEncoding = SystemRegEncoding::new(3, 0, 1, 1, 7);

    /// HACR_EL2 - Hypervisor Auxiliary Control Register
    pub const HACR_EL2: SystemRegEncoding = SystemRegEncoding::new(3, 0, 1, 0, 7);

    /// MDCR_EL2 - Monitor Debug Configuration Register (EL2)
    pub const MDCR_EL2: SystemRegEncoding = SystemRegEncoding::new(3, 0, 1, 3, 1);

    /// HCPTR_EL2 - Hypervisor Coprocessor Trap Register (ARMv7 compat)
    pub const HCPTR_EL2: SystemRegEncoding = SystemRegEncoding::new(3, 0, 1, 1, 2);

    /// HFGITR_EL2 - Fine-grained trap register for instruction execution
    pub const HFGITR_EL2: SystemRegEncoding = SystemRegEncoding::new(3, 0, 1, 6, 0);

    /// HDFGRTR_EL2 - Fine-grained trap control for reads
    pub const HDFGRTR_EL2: SystemRegEncoding = SystemRegEncoding::new(3, 0, 3, 1, 4);

    /// HDFGWTR_EL2 - Fine-grained trap control for writes
    pub const HDFGWTR_EL2: SystemRegEncoding = SystemRegEncoding::new(3, 0, 3, 1, 6);

    /// HPFAR_EL2 - Hypervisor IPA Fault Address Register
    pub const HPFAR_EL2: SystemRegEncoding = SystemRegEncoding::new(3, 0, 6, 0, 4);

    /// HCRX_EL2 - Extended Hypervisor Configuration Register
    pub const HCRX_EL2: SystemRegEncoding = SystemRegEncoding::new(3, 0, 1, 2, 4);

    /// TTBR0_EL2 - Translation Table Base Register 0 (EL2)
    pub const TTBR0_EL2: SystemRegEncoding = SystemRegEncoding::new(3, 0, 2, 0, 0);

    /// VMPIDR_EL2 - Virtualization Multiprocessor ID Register
    pub const VMPIDR_EL2: SystemRegEncoding = SystemRegEncoding::new(3, 0, 0, 0, 5);

    /// VPIDR_EL2 - Virtualization Processor ID Register
    pub const VPIDR_EL2: SystemRegEncoding = SystemRegEncoding::new(3, 0, 0, 0, 0);

    /// CNTVOFF_EL2 - Counter-timer Virtual Offset Register
    pub const CNTVOFF_EL2: SystemRegEncoding = SystemRegEncoding::new(3, 3, 14, 0, 3);

    /// CNTHCTL_EL2 - Counter-timer Hypervisor Control Register
    pub const CNTHCTL_EL2: SystemRegEncoding = SystemRegEncoding::new(3, 0, 14, 1, 0);
}

/// Initialize ARM64 architecture
pub fn arch_init() -> Result<(), &'static str> {
    // log::info!("Initializing ARM64 architecture");

    // Initialize CPU management (EL2 setup)
    cpu::init()?;

    // Initialize MMU (Stage-2 translation)
    mmu::init()?;

    // Initialize interrupt handling (GIC/VGIC)
    interrupt::init()?;

    // Initialize SMP
    smp::init()?;

    // Initialize platform-specific code
    platform::init()?;

    // log::info!("ARM64 architecture initialized successfully");
    Ok(())
}

/// ARM64 panic handler
pub fn panic(info: &core::panic::PanicInfo) -> ! {
    // TODO: Output panic info via UART
    // For now, just halt
    loop {
        // Wait For Event (low power mode)
        unsafe { core::arch::asm!("wfe") };
    }
}

/// ARM64 exception handling personality
pub extern "C" fn eh_personality() {
    loop {
        unsafe { core::arch::asm!("wfe") };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arm64_constants() {
        assert_eq!(PAGE_SIZE, 4096);
        assert_eq!(PAGE_SHIFT, 12);
        assert_eq!(VA_WIDTH, 48);
        assert_eq!(IPA_WIDTH, 40);
    }

    #[test]
    fn test_exception_levels() {
        assert_eq!(ExceptionLevel::EL0 as u8, 0);
        assert_eq!(ExceptionLevel::EL1 as u8, 1);
        assert_eq!(ExceptionLevel::EL2 as u8, 2);
        assert_eq!(ExceptionLevel::EL3 as u8, 3);
    }

    #[test]
    fn test_pstate_flags() {
        let flags = PStateFlags::N | PStateFlags::Z | PStateFlags::C | PStateFlags::V;
        assert!(flags.contains(PStateFlags::N));
        assert!(flags.contains(PStateFlags::Z));
        assert!(flags.contains(PStateFlags::C));
        assert!(flags.contains(PStateFlags::V));
    }

    #[test]
    fn test_system_reg_encoding() {
        let reg = SystemRegEncoding::new(3, 0, 4, 1, 0); // HCR_EL2
        assert_eq!(reg.op0, 3);
        assert_eq!(reg.op1, 0);
        assert_eq!(reg.crn, 4);
        assert_eq!(reg.crm, 1);
        assert_eq!(reg.op2, 0);
        assert_eq!(reg.encode(), 0b11 << 14 | 0b0000 << 11 | 0b0100 << 7 | 0b0001 << 3 | 0b0000);
    }
}
