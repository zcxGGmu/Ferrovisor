//! RISC-V CSR (Control and Status Register) Access
//!
//! This module provides safe and efficient access to RISC-V CSRs including:
//! - CSR read/write operations
//! - CSR bit manipulation helpers
//! - CSR definitions for all standard extensions
//! - Virtualization CSRs (H extension)
//! - Virtual Supervisor CSRs for nested virtualization
//! - Field-level atomic operations

use crate::arch::riscv64::*;
use bitflags::bitflags;

/// Exception codes for RISC-V
#[repr(usize)]
pub enum ExceptionCode {
    InstructionMisaligned = 0,
    InstructionAccessFault = 1,
    IllegalInstruction = 2,
    Breakpoint = 3,
    LoadMisaligned = 4,
    LoadAccessFault = 5,
    StoreMisaligned = 6,
    StoreAccessFault = 7,
    ECallFromUMode = 8,
    ECallFromSMode = 9,
    ECallFromMMode = 11,
    InstructionPageFault = 12,
    LoadPageFault = 13,
    StorePageFault = 15,
}

/// Interrupt causes for RISC-V
#[repr(usize)]
pub enum InterruptCause {
    SupervisorSoftware = 1,
    SupervisorTimer = 5,
    SupervisorExternal = 9,
}

impl TryFrom<usize> for ExceptionCode {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ExceptionCode::InstructionMisaligned),
            1 => Ok(ExceptionCode::InstructionAccessFault),
            2 => Ok(ExceptionCode::IllegalInstruction),
            3 => Ok(ExceptionCode::Breakpoint),
            4 => Ok(ExceptionCode::LoadMisaligned),
            5 => Ok(ExceptionCode::LoadAccessFault),
            6 => Ok(ExceptionCode::StoreMisaligned),
            7 => Ok(ExceptionCode::StoreAccessFault),
            8 => Ok(ExceptionCode::ECallFromUMode),
            9 => Ok(ExceptionCode::ECallFromSMode),
            11 => Ok(ExceptionCode::ECallFromMMode),
            12 => Ok(ExceptionCode::InstructionPageFault),
            13 => Ok(ExceptionCode::LoadPageFault),
            15 => Ok(ExceptionCode::StorePageFault),
            _ => Err(()),
        }
    }
}

impl TryFrom<usize> for InterruptCause {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(InterruptCause::SupervisorSoftware),
            5 => Ok(InterruptCause::SupervisorTimer),
            9 => Ok(InterruptCause::SupervisorExternal),
            _ => Err(()),
        }
    }
}

/// CSR address definitions
pub mod address {
    // User-level CSRs
    pub const USTATUS: usize = 0x000;
    pub const UIE: usize = 0x004;
    pub const UTVEC: usize = 0x005;

    // Supervisor-level CSRs
    pub const SSTATUS: usize = 0x100;
    pub const SEDELEG: usize = 0x102;
    pub const SIDELEG: usize = 0x103;
    pub const SIE: usize = 0x104;
    pub const STVEC: usize = 0x105;
    pub const SCOUNTEREN: usize = 0x106;
    pub const SSCRATCH: usize = 0x140;
    pub const SEPC: usize = 0x141;
    pub const SCAUSE: usize = 0x142;
    pub const STVAL: usize = 0x143;
    pub const SIP: usize = 0x144;
    pub const SATP: usize = 0x180;

    // Machine-level CSRs
    pub const MSTATUS: usize = 0x300;
    pub const MISA: usize = 0x301;
    pub const MEDELEG: usize = 0x302;
    pub const MIDELEG: usize = 0x303;
    pub const MIE: usize = 0x304;
    pub const MTVEC: usize = 0x305;
    pub const MCOUNTEREN: usize = 0x306;
    pub const MSCRATCH: usize = 0x340;
    pub const MEPC: usize = 0x341;
    pub const MCAUSE: usize = 0x342;
    pub const MTVAL: usize = 0x343;
    pub const MIP: usize = 0x344;

    // Machine Information and Timer CSRs
    pub const MVENDORID: usize = 0xF11;
    pub const MARCHID: usize = 0xF12;
    pub const MIMPID: usize = 0xF13;
    pub const MHARTID: usize = 0xF14;

    // Machine Counter/Timer CSRs
    pub const MCYCLE: usize = 0xB00;
    pub const MINSTRET: usize = 0xB02;
    pub const TIME: usize = 0xC01;
    pub const MTINST: usize = 0x34a;
    pub const MTVAL2: usize = 0x34b;

    // H-extension CSRs
    pub const HSTATUS: usize = 0x600;
    pub const HEDELEG: usize = 0x602;
    pub const HIDELEG: usize = 0x603;
    pub const HIE: usize = 0x604;
    pub const HCOUNTEREN: usize = 0x606;
    pub const HGEIE: usize = 0x607;
    pub const HTVAL: usize = 0x643;
    pub const HIP: usize = 0x644;
    pub const HVIP: usize = 0x645;
    pub const HTINST: usize = 0x64a;
    pub const HGEIP: usize = 0xe12;
    pub const HGATP: usize = 0x680;
    pub const HENVCFG: usize = 0x60a;

    // Virtual Supervisor CSRs
    pub const VSSTATUS: usize = 0x200;
    pub const VSIE: usize = 0x204;
    pub const VSTVEC: usize = 0x205;
    pub const VSSCRATCH: usize = 0x240;
    pub const VSEPC: usize = 0x241;
    pub const VSCAUSE: usize = 0x242;
    pub const VSTVAL: usize = 0x243;
    pub const VSIP: usize = 0x244;
    pub const VSATP: usize = 0x280;
}

/// CSR access macro for reading
#[macro_export]
macro_rules! read_csr {
    ($csr:expr) => {{
        let value: usize;
        core::arch::asm!(concat!("csrr {}, ", stringify!($csr)), out(reg) value);
        value
    }};
}

/// CSR access macro for writing
#[macro_export]
macro_rules! write_csr {
    ($csr:expr, $value:expr) => {
        core::arch::asm!(concat!("csrw ", stringify!($csr), ", {}"), in(reg) $value)
    };
}

/// CSR access macro for setting bits
#[macro_export]
macro_rules! set_csr {
    ($csr:expr, $bits:expr) => {
        core::arch::asm!(concat!("csrs ", stringify!($csr), ", {}"), in(reg) $bits)
    };
}

/// CSR access macro for clearing bits
#[macro_export]
macro_rules! clear_csr {
    ($csr:expr, $bits:expr) => {
        core::arch::asm!(concat!("csrc ", stringify!($csr), ", {}"), in(reg) $bits)
    };
}

/// CSR access macro for read-modify-write with clear and set
#[macro_export]
macro_rules! modify_csr {
    ($csr:expr, $clear:expr, $set:expr) => {{
        let value: usize;
        core::arch::asm!(
            concat!("csrrc {}, ", stringify!($csr), ", {}"),
            out(reg) value,
            in(reg) $clear
        );
        core::arch::asm!(
            concat!("csrs ", stringify!($csr), ", {}"),
            in(reg) $set
        );
        value
    }};
}

/// CSR read/write trait for different data types
pub trait CsrAccess<T> {
    /// Read CSR value
    fn read() -> T;
    /// Write value to CSR
    fn write(value: T);
    /// Set bits in CSR
    fn set(bits: T);
    /// Clear bits in CSR
    fn clear(bits: T);
    /// Modify CSR (clear then set)
    fn modify(clear: T, set: T) -> T;
}

/// CSR access for usize
pub struct UsizeCsr(pub usize);

impl UsizeCsr {
    /// Read CSR value
    #[inline]
    pub fn read(&self) -> usize {
        let value: usize;
        unsafe { core::arch::asm!("csrr {}, {}", out(reg) value, in(reg) self.0) };
        value
    }

    /// Write value to CSR
    #[inline]
    pub fn write(&self, value: usize) {
        unsafe { core::arch::asm!("csrw {}, {}", in(reg) self.0, in(reg) value) };
    }

    /// Set bits in CSR
    #[inline]
    pub fn set(&self, bits: usize) {
        unsafe { core::arch::asm!("csrs {}, {}", in(reg) self.0, in(reg) bits) };
    }

    /// Clear bits in CSR
    #[inline]
    pub fn clear(&self, bits: usize) {
        unsafe { core::arch::asm!("csrc {}, {}", in(reg) self.0, in(reg) bits) };
    }

    /// Modify CSR (clear then set)
    #[inline]
    pub fn modify(&self, clear: usize, set: usize) -> usize {
        let value: usize;
        unsafe {
            core::arch::asm!("csrrc {}, {}, {}", out(reg) value, in(reg) self.0, in(reg) clear);
            core::arch::asm!("csrs {}, {}", in(reg) self.0, in(reg) set);
        }
        value
    }
}

/// CSR access for u64
pub struct U64Csr(pub usize);

impl U64Csr {
    /// Read CSR value as u64
    #[inline]
    pub fn read(&self) -> u64 {
        let value: u64;
        unsafe { core::arch::asm!("csrr {}, {}", out(reg) value, in(reg) self.0) };
        value
    }

    /// Write value to CSR
    #[inline]
    pub fn write(&self, value: u64) {
        unsafe { core::arch::asm!("csrw {}, {}", in(reg) self.0, in(reg) value) };
    }

    /// Set bits in CSR
    #[inline]
    pub fn set(&self, bits: u64) {
        unsafe { core::arch::asm!("csrs {}, {}", in(reg) self.0, in(reg) bits) };
    }

    /// Clear bits in CSR
    #[inline]
    pub fn clear(&self, bits: u64) {
        unsafe { core::arch::asm!("csrc {}, {}", in(reg) self.0, in(reg) bits) };
    }

    /// Modify CSR (clear then set)
    #[inline]
    pub fn modify(&self, clear: u64, set: u64) -> u64 {
        let value: u64;
        unsafe {
            core::arch::asm!("csrrc {}, {}, {}", out(reg) value, in(reg) self.0, in(reg) clear);
            core::arch::asm!("csrs {}, {}", in(reg) self.0, in(reg) set);
        }
        value
    }
}

/// MSTATUS register flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MstatusFlags: usize {
        const SIE = 1 << 1;      // Supervisor Interrupt Enable
        const MIE = 1 << 3;      // Machine Interrupt Enable
        const SPIE = 1 << 5;     // Supervisor Previous Interrupt Enable
        const UBE = 1 << 6;      // User Big Endian
        const MPIE = 1 << 7;     // Machine Previous Interrupt Enable
        const SPP = 1 << 8;      // Supervisor Previous Privilege
        const FS = 0x3 << 13;    // Floating-point Status
        const XS = 0x3 << 15;    // Extension Status
        const MPRV = 1 << 17;    // Modify Privilege
        const SUM = 1 << 18;     // Supervisor User Memory access
        const MXR = 1 << 19;     // Make eXecutable Readable
        const TVM = 1 << 20;    // Trap Virtual Memory
        const TW = 1 << 21;     // Timeout Wait
        const TSR = 1 << 22;    // Trap SRET
    }
}

/// Machine status register
pub struct MSTATUS;
impl MSTATUS {
    pub const CSR: usize = address::MSTATUS;

    #[inline]
    pub fn read() -> MstatusFlags {
        let csr = UsizeCsr(Self::CSR);
        let value = csr.read();
        MstatusFlags::from_bits_truncate(value)
    }

    #[inline]
    pub fn write(value: MstatusFlags) {
        let csr = UsizeCsr(Self::CSR);
        csr.write(value.bits());
    }

    #[inline]
    pub fn set(bits: MstatusFlags) {
        let csr = UsizeCsr(Self::CSR);
        csr.set(bits.bits());
    }

    #[inline]
    pub fn clear(bits: MstatusFlags) {
        let csr = UsizeCsr(Self::CSR);
        csr.clear(bits.bits());
    }

    #[inline]
    pub fn modify(clear: MstatusFlags, set: MstatusFlags) {
        let csr = UsizeCsr(Self::CSR);
        csr.modify(clear.bits(), set.bits());
    }
}

/// SSTATUS register flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SstatusFlags: usize {
        const SIE = 1 << 1;      // Supervisor Interrupt Enable
        const MIE = 1 << 3;      // Machine Interrupt Enable
        const SPIE = 1 << 5;     // Supervisor Previous Interrupt Enable
        const UBE = 1 << 6;      // User Big Endian
        const SPP = 1 << 8;      // Supervisor Previous Privilege
        const FS = 0x3 << 13;    // Floating-point Status
        const XS = 0x3 << 15;    // Extension Status
        const SUM = 1 << 18;     // Supervisor User Memory access
        const MXR = 1 << 19;     // Make eXecutable Readable
        const UXL = 0x3 << 32;   // User XLEN
    }
}

/// Supervisor status register
pub struct SSTATUS;
impl SSTATUS {
    pub const CSR: usize = address::SSTATUS;

    #[inline]
    pub fn read() -> SstatusFlags {
        let csr = UsizeCsr(Self::CSR);
        let value = csr.read();
        SstatusFlags::from_bits_truncate(value)
    }

    #[inline]
    pub fn write(value: SstatusFlags) {
        let csr = UsizeCsr(Self::CSR);
        csr.write(value.bits());
    }

    #[inline]
    pub fn set(bits: SstatusFlags) {
        let csr = UsizeCsr(Self::CSR);
        csr.set(bits.bits());
    }

    #[inline]
    pub fn clear(bits: SstatusFlags) {
        let csr = UsizeCsr(Self::CSR);
        csr.clear(bits.bits());
    }

    #[inline]
    pub fn modify(clear: SstatusFlags, set: SstatusFlags) {
        let csr = UsizeCsr(Self::CSR);
        csr.modify(clear.bits(), set.bits());
    }
}

/// Machine interrupt-enable register
pub struct MIE;
impl MIE {
    pub const CSR: usize = address::MIE;

    #[inline]
    pub fn read() -> Mie {
        let value = UsizeCsr(Self::CSR).read();
        Mie::from_bits_truncate(value)
    }

    #[inline]
    pub fn write(value: Mie) {
        UsizeCsr(Self::CSR).write(value.bits());
    }

    #[inline]
    pub fn set(bits: Mie) {
        UsizeCsr(Self::CSR).set(bits.bits());
    }

    #[inline]
    pub fn clear(bits: Mie) {
        UsizeCsr(Self::CSR).clear(bits.bits());
    }

    #[inline]
    pub fn modify(clear: Mie, set: Mie) {
        UsizeCsr(Self::CSR).modify(clear.bits(), set.bits());
    }
}

/// Supervisor interrupt-enable register
pub struct SIE;
impl SIE {
    pub const CSR: usize = address::SIE;

    #[inline]
    pub fn read() -> Sie {
        let value = UsizeCsr(Self::CSR).read();
        Sie::from_bits_truncate(value)
    }

    #[inline]
    pub fn write(value: Sie) {
        UsizeCsr(Self::CSR).write(value.bits());
    }

    #[inline]
    pub fn set(bits: Sie) {
        UsizeCsr(Self::CSR).set(bits.bits());
    }

    #[inline]
    pub fn clear(bits: Sie) {
        UsizeCsr(Self::CSR).clear(bits.bits());
    }

    #[inline]
    pub fn modify(clear: Sie, set: Sie) {
        UsizeCsr(Self::CSR).modify(clear.bits(), set.bits());
    }
}

/// MIE register flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Mie: usize {
        const USIE = 1 << 0;     // User Software Interrupt Enable
        const SSIE = 1 << 1;     // Supervisor Software Interrupt Enable
        const MSIE = 1 << 3;     // Machine Software Interrupt Enable
        const UTIE = 1 << 4;     // User Timer Interrupt Enable
        const STIE = 1 << 5;     // Supervisor Timer Interrupt Enable
        const MTIE = 1 << 7;     // Machine Timer Interrupt Enable
        const UEIE = 1 << 8;     // User External Interrupt Enable
        const SEIE = 1 << 9;     // Supervisor External Interrupt Enable
        const MEIE = 1 << 11;    // Machine External Interrupt Enable
        const SGEIE = 1 << 12;   // Supervisor Guest External Interrupt Enable
        const LCOFIE = 1 << 13;  // Local Counter Overflow Interrupt Enable
    }
}

/// SIE register flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Sie: usize {
        const USIE = 1 << 0;     // User Software Interrupt Enable
        const SSIE = 1 << 1;     // Supervisor Software Interrupt Enable
        const UTIE = 1 << 4;     // User Timer Interrupt Enable
        const STIE = 1 << 5;     // Supervisor Timer Interrupt Enable
        const UEIE = 1 << 8;     // User External Interrupt Enable
        const SEIE = 1 << 9;     // Supervisor External Interrupt Enable
        const LCOFIE = 1 << 13;  // Local Counter Overflow Interrupt Enable
    }
}

/// Machine interrupt-pending register
pub struct MIP;
impl MIP {
    pub const CSR: usize = address::MIP;

    #[inline]
    pub fn read() -> Mip {
        let value = UsizeCsr(Self::CSR).read();
        Mip::from_bits_truncate(value)
    }

    #[inline]
    pub fn write(value: Mip) {
        UsizeCsr(Self::CSR).write(value.bits());
    }

    #[inline]
    pub fn set(bits: Mip) {
        UsizeCsr(Self::CSR).set(bits.bits());
    }

    #[inline]
    pub fn clear(bits: Mip) {
        UsizeCsr(Self::CSR).clear(bits.bits());
    }

    #[inline]
    pub fn modify(clear: Mip, set: Mip) {
        UsizeCsr(Self::CSR).modify(clear.bits(), set.bits());
    }
}

/// Supervisor interrupt-pending register
pub struct SIP;
impl SIP {
    pub const CSR: usize = address::SIP;

    #[inline]
    pub fn read() -> Sip {
        let value = UsizeCsr(Self::CSR).read();
        Sip::from_bits_truncate(value)
    }

    #[inline]
    pub fn write(value: Sip) {
        UsizeCsr(Self::CSR).write(value.bits());
    }

    #[inline]
    pub fn set(bits: Sip) {
        UsizeCsr(Self::CSR).set(bits.bits());
    }

    #[inline]
    pub fn clear(bits: Sip) {
        UsizeCsr(Self::CSR).clear(bits.bits());
    }

    #[inline]
    pub fn modify(clear: Sip, set: Sip) {
        UsizeCsr(Self::CSR).modify(clear.bits(), set.bits());
    }
}

/// MIP register flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Mip: usize {
        const USIP = 1 << 0;     // User Software Interrupt Pending
        const SSIP = 1 << 1;     // Supervisor Software Interrupt Pending
        const MSIP = 1 << 3;     // Machine Software Interrupt Pending
        const UTIP = 1 << 4;     // User Timer Interrupt Pending
        const STIP = 1 << 5;     // Supervisor Timer Interrupt Pending
        const MTIP = 1 << 7;     // Machine Timer Interrupt Pending
        const UEIP = 1 << 8;     // User External Interrupt Pending
        const SEIP = 1 << 9;     // Supervisor External Interrupt Pending
        const MEIP = 1 << 11;    // Machine External Interrupt Pending
        const SGEIP = 1 << 12;   // Supervisor Guest External Interrupt Pending
        const LCOFIP = 1 << 13;  // Local Counter Overflow Interrupt Pending
    }
}

/// SIP register flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Sip: usize {
        const USIP = 1 << 0;     // User Software Interrupt Pending
        const SSIP = 1 << 1;     // Supervisor Software Interrupt Pending
        const UTIP = 1 << 4;     // User Timer Interrupt Pending
        const STIP = 1 << 5;     // Supervisor Timer Interrupt Pending
        const UEIP = 1 << 8;     // User External Interrupt Pending
        const SEIP = 1 << 9;     // Supervisor External Interrupt Pending
        const LCOFIP = 1 << 13;  // Local Counter Overflow Interrupt Pending
    }
}

/// SATP (Supervisor Address Translation and Protection) register
pub struct SATP;
impl SATP {
    pub const CSR: usize = address::SATP;

    #[inline]
    pub fn read() -> usize {
        UsizeCsr(Self::CSR).read()
    }

    #[inline]
    pub fn write(value: usize) {
        UsizeCsr(Self::CSR).write(value);
    }

    #[inline]
    pub fn make(ppn: usize, asid: usize, mode: usize) -> usize {
        (ppn << 44) | ((asid & 0xFFFF) << 16) | (mode & 0xF)
    }

    #[inline]
    pub fn extract_ppn(value: usize) -> usize {
        value >> 44
    }

    #[inline]
    pub fn extract_asid(value: usize) -> usize {
        (value >> 16) & 0xFFFF
    }

    #[inline]
    pub fn extract_mode(value: usize) -> usize {
        value & 0xF
    }
}

/// HSTATUS register
pub struct HSTATUS;
impl HSTATUS {
    pub const CSR: usize = address::HSTATUS;

    #[inline]
    pub fn read() -> HstatusFlags {
        let value = UsizeCsr(Self::CSR).read();
        HstatusFlags::from_bits_truncate(value)
    }

    #[inline]
    pub fn write(value: HstatusFlags) {
        UsizeCsr(Self::CSR).write(value.bits());
    }

    #[inline]
    pub fn set(bits: HstatusFlags) {
        UsizeCsr(Self::CSR).set(bits.bits());
    }

    #[inline]
    pub fn clear(bits: HstatusFlags) {
        UsizeCsr(Self::CSR).clear(bits.bits());
    }

    #[inline]
    pub fn modify(clear: HstatusFlags, set: HstatusFlags) {
        UsizeCsr(Self::CSR).modify(clear.bits(), set.bits());
    }
}

/// MEDELEG (Machine Exception Delegation) register
pub struct MEDELEG;
impl MEDELEG {
    pub const CSR: usize = address::MEDELEG;

    #[inline]
    pub fn read() -> usize {
        let csr = UsizeCsr(Self::CSR);
        csr.read()
    }

    #[inline]
    pub fn write(value: usize) {
        let csr = UsizeCsr(Self::CSR);
        csr.write(value);
    }

    #[inline]
    pub fn set(bits: usize) {
        let csr = UsizeCsr(Self::CSR);
        csr.set(bits);
    }

    #[inline]
    pub fn clear(bits: usize) {
        let csr = UsizeCsr(Self::CSR);
        csr.clear(bits);
    }
}

/// MIDELEG (Machine Interrupt Delegation) register
pub struct MIDELEG;
impl MIDELEG {
    pub const CSR: usize = address::MIDELEG;

    #[inline]
    pub fn read() -> usize {
        let csr = UsizeCsr(Self::CSR);
        csr.read()
    }

    #[inline]
    pub fn write(value: usize) {
        let csr = UsizeCsr(Self::CSR);
        csr.write(value);
    }

    #[inline]
    pub fn set(bits: usize) {
        let csr = UsizeCsr(Self::CSR);
        csr.set(bits);
    }

    #[inline]
    pub fn clear(bits: usize) {
        let csr = UsizeCsr(Self::CSR);
        csr.clear(bits);
    }
}

/// CSR initialization
pub fn init() -> Result<(), &'static str> {
    log::debug!("Initializing CSR access");

    // Set up initial machine mode configuration
    MSTATUS::write(MstatusFlags::MIE);

    // Configure interrupt delegation (delegate supervisor interrupts to S-mode)
    MEDELEG::write(
        (1 << ExceptionCode::InstructionMisaligned as usize) |
        (1 << ExceptionCode::InstructionAccessFault as usize) |
        (1 << ExceptionCode::IllegalInstruction as usize) |
        (1 << ExceptionCode::Breakpoint as usize) |
        (1 << ExceptionCode::LoadMisaligned as usize) |
        (1 << ExceptionCode::LoadAccessFault as usize) |
        (1 << ExceptionCode::StoreMisaligned as usize) |
        (1 << ExceptionCode::StoreAccessFault as usize) |
        (1 << ExceptionCode::ECallFromUMode as usize) |
        (1 << ExceptionCode::InstructionPageFault as usize) |
        (1 << ExceptionCode::LoadPageFault as usize) |
        (1 << ExceptionCode::StorePageFault as usize)
    );

    // Delegate supervisor interrupts to S-mode
    MIDELEG::write(
        (1 << InterruptCause::SupervisorSoftware as usize) |
        (1 << InterruptCause::SupervisorTimer as usize) |
        (1 << InterruptCause::SupervisorExternal as usize)
    );

    log::debug!("CSR access initialized");
    Ok(())
}

// ===== H-Extension CSRs =====

/// HSTATUS register flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct HstatusFlags: usize {
        const VTSR = 1 << 22;    // Virtual SSTATUS Read
        const VTW = 1 << 21;     // Virtual Timer Write
        const VTVM = 1 << 20;    // Virtual Trap Virtual Memory
        const VGEIN = 0x3F << 12; // Virtual Guest External Interrupt Number
        const HU = 1 << 9;       // Hypervisor User mode
        const SPVP = 1 << 8;     // Supervisor Previous Virtual Privilege
        const SPV = 1 << 7;      // Supervisor Previous Virtualization
        const GVA = 1 << 6;      // Guest Virtual Address
        const VSBE = 1 << 5;     // Virtual Supervisor Big Endian
    }
}

/// HEDELEG register flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Hedeleg: usize {
        const INSTRUCTION_MISALIGNED = 1 << 0;     // 0
        const INSTRUCTION_ACCESS_FAULT = 1 << 1;   // 1
        const ILLEGAL_INSTRUCTION = 1 << 2;        // 2
        const BREAKPOINT = 1 << 3;                 // 3
        const LOAD_MISALIGNED = 1 << 4;            // 4
        const LOAD_ACCESS_FAULT = 1 << 5;          // 5
        const STORE_MISALIGNED = 1 << 6;           // 6
        const STORE_ACCESS_FAULT = 1 << 7;         // 7
        const ECALL_FROM_UMODE = 1 << 8;           // 8
        const ECALL_FROM_SMODE = 1 << 9;           // 9
        const ECALL_FROM_MMODE = 1 << 11;          // 11
        const INSTRUCTION_PAGE_FAULT = 1 << 12;    // 12
        const LOAD_PAGE_FAULT = 1 << 13;           // 13
        const STORE_PAGE_FAULT = 1 << 15;          // 15
    }
}

/// HEDELEG register (Hypervisor Exception Delegation)
pub struct HEDELEG;
impl HEDELEG {
    pub const CSR: usize = address::HEDELEG;

    #[inline]
    pub fn read() -> Hedeleg {
        let value = UsizeCsr(Self::CSR).read();
        Hedeleg::from_bits_truncate(value)
    }

    #[inline]
    pub fn write(value: Hedeleg) {
        UsizeCsr(Self::CSR).write(value.bits());
    }

    #[inline]
    pub fn set(bits: Hedeleg) {
        UsizeCsr(Self::CSR).set(bits.bits());
    }

    #[inline]
    pub fn clear(bits: Hedeleg) {
        UsizeCsr(Self::CSR).clear(bits.bits());
    }

    /// Delegate all standard exceptions to supervisor
    #[inline]
    pub fn delegate_all_standard() {
        let hedeleg = Hedeleg::INSTRUCTION_MISALIGNED |
                     Hedeleg::INSTRUCTION_ACCESS_FAULT |
                     Hedeleg::ILLEGAL_INSTRUCTION |
                     Hedeleg::BREAKPOINT |
                     Hedeleg::LOAD_MISALIGNED |
                     Hedeleg::LOAD_ACCESS_FAULT |
                     Hedeleg::STORE_MISALIGNED |
                     Hedeleg::STORE_ACCESS_FAULT |
                     Hedeleg::ECALL_FROM_UMODE |
                     Hedeleg::ECALL_FROM_SMODE |
                     Hedeleg::INSTRUCTION_PAGE_FAULT |
                     Hedeleg::LOAD_PAGE_FAULT |
                     Hedeleg::STORE_PAGE_FAULT;
        Self::write(hedeleg);
    }

    /// Check if an exception is delegated
    #[inline]
    pub fn is_delegated(exception_code: ExceptionCode) -> bool {
        let hedeleg = Self::read();
        hedeleg.contains(Hedeleg::from_bits(1 << exception_code as usize).unwrap())
    }
}

/// HIDELEG register flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Hideleg: usize {
        const SSIP = 1 << 1;     // Supervisor Software Interrupt Pending
        const VSSIP = 1 << 2;    // Virtual Supervisor Software Interrupt Pending
        const STIP = 1 << 5;     // Supervisor Timer Interrupt Pending
        const VSTIP = 1 << 6;    // Virtual Supervisor Timer Interrupt Pending
        const SEIP = 1 << 9;     // Supervisor External Interrupt Pending
        const VSEIP = 1 << 10;   // Virtual Supervisor External Interrupt Pending
        const SGEIP = 1 << 12;   // Supervisor Guest External Interrupt Pending
    }
}

/// HIDELEG register
pub struct HIDELEG;
impl HIDELEG {
    pub const CSR: usize = address::HIDELEG;

    #[inline]
    pub fn read() -> Hideleg {
        let value = UsizeCsr(Self::CSR).read();
        Hideleg::from_bits_truncate(value)
    }

    #[inline]
    pub fn write(value: Hideleg) {
        UsizeCsr(Self::CSR).write(value.bits());
    }

    #[inline]
    pub fn set(bits: Hideleg) {
        UsizeCsr(Self::CSR).set(bits.bits());
    }

    #[inline]
    pub fn clear(bits: Hideleg) {
        UsizeCsr(Self::CSR).clear(bits.bits());
    }

    /// Delegate all standard supervisor interrupts
    #[inline]
    pub fn delegate_all_standard() {
        let hideleg = Hideleg::SSIP |
                     Hideleg::VSSIP |
                     Hideleg::STIP |
                     Hideleg::VSTIP |
                     Hideleg::SEIP |
                     Hideleg::VSEIP;
        Self::write(hideleg);
    }

    /// Check if an interrupt is delegated
    #[inline]
    pub fn is_delegated(interrupt: InterruptCause) -> bool {
        let hideleg = Self::read();
        match interrupt {
            InterruptCause::SupervisorSoftware => hideleg.contains(Hideleg::SSIP),
            InterruptCause::SupervisorTimer => hideleg.contains(Hideleg::STIP),
            InterruptCause::SupervisorExternal => hideleg.contains(Hideleg::SEIP),
        }
    }
}

/// HIE register flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Hie: usize {
        const VSSIE = 1 << 2;    // Virtual Supervisor Software Interrupt Enable
        const VSTIE = 1 << 6;    // Virtual Supervisor Timer Interrupt Enable
        const VSEIE = 1 << 10;   // Virtual Supervisor External Interrupt Enable
        const SGEIE = 1 << 12;   // Supervisor Guest External Interrupt Enable
    }
}

/// HIE register
pub struct HIE;
impl HIE {
    pub const CSR: usize = address::HIE;

    #[inline]
    pub fn read() -> Hie {
        let value = UsizeCsr(Self::CSR).read();
        Hie::from_bits_truncate(value)
    }

    #[inline]
    pub fn write(value: Hie) {
        UsizeCsr(Self::CSR).write(value.bits());
    }

    #[inline]
    pub fn set(bits: Hie) {
        UsizeCsr(Self::CSR).set(bits.bits());
    }

    #[inline]
    pub fn clear(bits: Hie) {
        UsizeCsr(Self::CSR).clear(bits.bits());
    }
}

/// HIP register flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Hip: usize {
        const VSSIP = 1 << 2;    // Virtual Supervisor Software Interrupt Pending
        const VSTIP = 1 << 6;    // Virtual Supervisor Timer Interrupt Pending
        const VSEIP = 1 << 10;   // Virtual Supervisor External Interrupt Pending
        const SGEIP = 1 << 12;   // Supervisor Guest External Interrupt Pending
    }
}

/// HIP register
pub struct HIP;
impl HIP {
    pub const CSR: usize = address::HIP;

    #[inline]
    pub fn read() -> Hip {
        let value = UsizeCsr(Self::CSR).read();
        Hip::from_bits_truncate(value)
    }

    #[inline]
    pub fn write(value: Hip) {
        UsizeCsr(Self::CSR).write(value.bits());
    }

    #[inline]
    pub fn set(bits: Hip) {
        UsizeCsr(Self::CSR).set(bits.bits());
    }

    #[inline]
    pub fn clear(bits: Hip) {
        UsizeCsr(Self::CSR).clear(bits.bits());
    }
}

/// HVIP register flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Hvip: usize {
        const VSSIP = 1 << 2;    // Virtual Supervisor Software Interrupt Pending
        const VSTIP = 1 << 6;    // Virtual Supervisor Timer Interrupt Pending
        const VSEIP = 1 << 10;   // Virtual Supervisor External Interrupt Pending
    }
}

/// HVIP register
pub struct HVIP;
impl HVIP {
    pub const CSR: usize = address::HVIP;

    #[inline]
    pub fn read() -> Hvip {
        let value = UsizeCsr(Self::CSR).read();
        Hvip::from_bits_truncate(value)
    }

    #[inline]
    pub fn write(value: Hvip) {
        UsizeCsr(Self::CSR).write(value.bits());
    }

    #[inline]
    pub fn set(bits: Hvip) {
        UsizeCsr(Self::CSR).set(bits.bits());
    }

    #[inline]
    pub fn clear(bits: Hvip) {
        UsizeCsr(Self::CSR).clear(bits.bits());
    }
}

/// HGEIE register flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Hgeie: usize {
        const VSSIE = 1 << 2;    // Virtual Supervisor Software Interrupt Enable
        const VSTIE = 1 << 6;    // Virtual Supervisor Timer Interrupt Enable
        const VSEIE = 1 << 10;   // Virtual Supervisor External Interrupt Enable
        const SGEIE = 1 << 12;   // Supervisor Guest External Interrupt Enable
    }
}

/// HGEIE register
pub struct HGEIE;
impl HGEIE {
    pub const CSR: usize = address::HGEIE;

    #[inline]
    pub fn read() -> Hgeie {
        let value = UsizeCsr(Self::CSR).read();
        Hgeie::from_bits_truncate(value)
    }

    #[inline]
    pub fn write(value: Hgeie) {
        UsizeCsr(Self::CSR).write(value.bits());
    }

    #[inline]
    pub fn set(bits: Hgeie) {
        UsizeCsr(Self::CSR).set(bits.bits());
    }

    #[inline]
    pub fn clear(bits: Hgeie) {
        UsizeCsr(Self::CSR).clear(bits.bits());
    }
}

/// HGEIP register flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Hgeip: usize {
        const VSSIP = 1 << 2;    // Virtual Supervisor Software Interrupt Pending
        const VSTIP = 1 << 6;    // Virtual Supervisor Timer Interrupt Pending
        const VSEIP = 1 << 10;   // Virtual Supervisor External Interrupt Pending
        const SGEIP = 1 << 12;   // Supervisor Guest External Interrupt Pending
    }
}

/// HGEIP register
pub struct HGEIP;
impl HGEIP {
    pub const CSR: usize = address::HGEIP;

    #[inline]
    pub fn read() -> Hgeip {
        let value = UsizeCsr(Self::CSR).read();
        Hgeip::from_bits_truncate(value)
    }

    #[inline]
    pub fn write(value: Hgeip) {
        UsizeCsr(Self::CSR).write(value.bits());
    }

    #[inline]
    pub fn set(bits: Hgeip) {
        UsizeCsr(Self::CSR).set(bits.bits());
    }

    #[inline]
    pub fn clear(bits: Hgeip) {
        UsizeCsr(Self::CSR).clear(bits.bits());
    }
}

// ===== Virtual Supervisor CSRs =====

/// VSSTATUS register flags (same as SSTATUS but for virtual supervisor)
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct VsstatusFlags: usize {
        const SIE = 1 << 1;      // Supervisor Interrupt Enable
        const MIE = 1 << 3;      // Machine Interrupt Enable
        const SPIE = 1 << 5;     // Supervisor Previous Interrupt Enable
        const SPP = 1 << 8;      // Supervisor Previous Privilege
        const SUM = 1 << 18;     // Supervisor User Memory access
        const MXR = 1 << 19;     // Make eXecutable Readable
    }
}

/// VSSTATUS register
pub struct VSSTATUS;
impl VSSTATUS {
    pub const CSR: usize = address::VSSTATUS;

    #[inline]
    pub fn read() -> VsstatusFlags {
        let csr = UsizeCsr(Self::CSR);
        let value = csr.read();
        VsstatusFlags::from_bits_truncate(value)
    }

    #[inline]
    pub fn write(value: VsstatusFlags) {
        let csr = UsizeCsr(Self::CSR);
        csr.write(value.bits());
    }

    #[inline]
    pub fn set(bits: VsstatusFlags) {
        let csr = UsizeCsr(Self::CSR);
        csr.set(bits.bits());
    }

    #[inline]
    pub fn clear(bits: VsstatusFlags) {
        let csr = UsizeCsr(Self::CSR);
        csr.clear(bits.bits());
    }
}

/// VSTVEC register
pub struct VSTVEC;
impl VSTVEC {
    pub const CSR: usize = address::VSTVEC;

    #[inline]
    pub fn read() -> usize {
        UsizeCsr(Self::CSR).read()
    }

    #[inline]
    pub fn write(value: usize) {
        UsizeCsr(Self::CSR).write(value);
    }

    /// Make VSTVEC value from base and mode
    #[inline]
    pub fn make(base: usize, mode: usize) -> usize {
        (base & !0x3) | (mode & 0x3)
    }

    /// Extract base address
    #[inline]
    pub fn extract_base(value: usize) -> usize {
        value & !0x3
    }

    /// Extract mode
    #[inline]
    pub fn extract_mode(value: usize) -> usize {
        value & 0x3
    }
}

/// VSTVEC mode values
impl VSTVEC {
    pub const MODE_DIRECT: usize = 0;
    pub const MODE_VECTORED: usize = 1;
}

/// VSSCRATCH register
pub struct VSSCRATCH;
impl VSSCRATCH {
    pub const CSR: usize = address::VSSCRATCH;

    #[inline]
    pub fn read() -> usize {
        UsizeCsr(Self::CSR).read()
    }

    #[inline]
    pub fn write(value: usize) {
        UsizeCsr(Self::CSR).write(value);
    }
}

/// VSEPC register
pub struct VSEPC;
impl VSEPC {
    pub const CSR: usize = address::VSEPC;

    #[inline]
    pub fn read() -> usize {
        UsizeCsr(Self::CSR).read()
    }

    #[inline]
    pub fn write(value: usize) {
        UsizeCsr(Self::CSR).write(value);
    }
}

/// VSCAUSE register
pub struct VSCAUSE;
impl VSCAUSE {
    pub const CSR: usize = address::VSCAUSE;

    #[inline]
    pub fn read() -> usize {
        UsizeCsr(Self::CSR).read()
    }

    #[inline]
    pub fn write(value: usize) {
        UsizeCsr(Self::CSR).write(value);
    }

    /// Extract exception code
    #[inline]
    pub fn extract_code(value: usize) -> usize {
        value & 0x7FFFFFFF
    }

    /// Extract interrupt flag
    #[inline]
    pub fn extract_interrupt(value: usize) -> bool {
        (value & (1 << 31)) != 0
    }
}

/// VSTVAL register
pub struct VSTVAL;
impl VSTVAL {
    pub const CSR: usize = address::VSTVAL;

    #[inline]
    pub fn read() -> usize {
        UsizeCsr(Self::CSR).read()
    }

    #[inline]
    pub fn write(value: usize) {
        UsizeCsr(Self::CSR).write(value);
    }
}

/// VSATP register
pub struct VSATP;
impl VSATP {
    pub const CSR: usize = address::VSATP;

    #[inline]
    pub fn read() -> usize {
        UsizeCsr(Self::CSR).read()
    }

    #[inline]
    pub fn write(value: usize) {
        UsizeCsr(Self::CSR).write(value);
    }

    /// Make VSATP value from PPN, ASID, and mode
    #[inline]
    pub fn make(ppn: usize, asid: usize, mode: usize) -> usize {
        (ppn << 44) | ((asid & 0xFFFF) << 16) | (mode & 0x1F)
    }

    /// Extract PPN
    #[inline]
    pub fn extract_ppn(value: usize) -> usize {
        value >> 44
    }

    /// Extract ASID
    #[inline]
    pub fn extract_asid(value: usize) -> usize {
        (value >> 16) & 0xFFFF
    }

    /// Extract mode
    #[inline]
    pub fn extract_mode(value: usize) -> usize {
        value & 0x1F
    }
}

/// VSATP mode values
impl VSATP {
    pub const MODE_BARE: usize = 0;
    pub const MODE_SV32: usize = 1;
    pub const MODE_SV39: usize = 8;
    pub const MODE_SV48: usize = 9;
}

/// VSIP register flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Vsip: usize {
        const SSIP = 1 << 1;     // Supervisor Software Interrupt Pending
        const STIP = 1 << 5;     // Supervisor Timer Interrupt Pending
        const SEIP = 1 << 9;     // Supervisor External Interrupt Pending
    }
}

/// VSIP register
pub struct VSIP;
impl VSIP {
    pub const CSR: usize = address::VSIP;

    #[inline]
    pub fn read() -> Vsip {
        let value = UsizeCsr(Self::CSR).read();
        Vsip::from_bits_truncate(value)
    }

    #[inline]
    pub fn write(value: Vsip) {
        UsizeCsr(Self::CSR).write(value.bits());
    }

    #[inline]
    pub fn set(bits: Vsip) {
        UsizeCsr(Self::CSR).set(bits.bits());
    }

    #[inline]
    pub fn clear(bits: Vsip) {
        UsizeCsr(Self::CSR).clear(bits.bits());
    }
}

/// VSIE register flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Vsie: usize {
        const SSIE = 1 << 1;     // Supervisor Software Interrupt Enable
        const STIE = 1 << 5;     // Supervisor Timer Interrupt Enable
        const SEIE = 1 << 9;     // Supervisor External Interrupt Enable
    }
}

/// VSIE register
pub struct VSIE;
impl VSIE {
    pub const CSR: usize = address::VSIE;

    #[inline]
    pub fn read() -> Vsie {
        let value = UsizeCsr(Self::CSR).read();
        Vsie::from_bits_truncate(value)
    }

    #[inline]
    pub fn write(value: Vsie) {
        UsizeCsr(Self::CSR).write(value.bits());
    }

    #[inline]
    pub fn set(bits: Vsie) {
        UsizeCsr(Self::CSR).set(bits.bits());
    }

    #[inline]
    pub fn clear(bits: Vsie) {
        UsizeCsr(Self::CSR).clear(bits.bits());
    }
}

// ===== Field-level Operations =====

/// CSR field operations for precise bit field manipulation
pub struct CsrField {
    csr: usize,
    mask: usize,
    shift: u32,
}

impl CsrField {
    /// Create a new CSR field
    #[inline]
    pub const fn new(csr: usize, mask: usize, shift: u32) -> Self {
        Self { csr, mask, shift }
    }

    /// Read the field value
    #[inline]
    pub fn read(&self) -> usize {
        (UsizeCsr(self.csr).read() >> self.shift) & self.mask
    }

    /// Write the field value
    #[inline]
    pub fn write(&self, value: usize) {
        let current = UsizeCsr(self.csr).read();
        let new_value = (current & !(self.mask << self.shift)) | ((value & self.mask) << self.shift);
        UsizeCsr(self.csr).write(new_value);
    }

    /// Set bits in the field
    #[inline]
    pub fn set_bits(&self, bits: usize) {
        self.write(self.read() | bits);
    }

    /// Clear bits in the field
    #[inline]
    pub fn clear_bits(&self, bits: usize) {
        self.write(self.read() & !bits);
    }

    /// Check if bits are set in the field
    #[inline]
    pub fn is_set(&self, bits: usize) -> bool {
        (self.read() & bits) != 0
    }

    /// Check if all bits are set in the field
    #[inline]
    pub fn is_all_set(&self, bits: usize) -> bool {
        (self.read() & bits) == bits
    }

    /// Atomically read and set bits in the field
    #[inline]
    pub fn read_set_bits(&self, bits: usize) -> usize {
        let old = UsizeCsr(self.csr).read();
        UsizeCsr(self.csr).set((bits & self.mask) << self.shift);
        (old >> self.shift) & self.mask
    }

    /// Atomically read and clear bits in the field
    #[inline]
    pub fn read_clear_bits(&self, bits: usize) -> usize {
        let old = UsizeCsr(self.csr).read();
        UsizeCsr(self.csr).clear((bits & self.mask) << self.shift);
        (old >> self.shift) & self.mask
    }
}

/// Predefined CSR fields for common operations
pub mod fields {
    use super::*;

    /// MSTATUS fields
    pub mod mstatus {
        use super::address::MSTATUS;
        pub const SIE: CsrField = CsrField::new(MSTATUS, 1, 1);
        pub const MIE: CsrField = CsrField::new(MSTATUS, 1, 3);
        pub const MPIE: CsrField = CsrField::new(MSTATUS, 1, 7);
        pub const MPP: CsrField = CsrField::new(MSTATUS, 0x3, 8);
        pub const FS: CsrField = CsrField::new(MSTATUS, 0x3, 13);
        pub const XS: CsrField = CsrField::new(MSTATUS, 0x3, 15);
        pub const MPRV: CsrField = CsrField::new(MSTATUS, 1, 17);
        pub const SUM: CsrField = CsrField::new(MSTATUS, 1, 18);
        pub const MXR: CsrField = CsrField::new(MSTATUS, 1, 19);
        pub const TVM: CsrField = CsrField::new(MSTATUS, 1, 20);
        pub const TW: CsrField = CsrField::new(MSTATUS, 1, 21);
        pub const TSR: CsrField = CsrField::new(MSTATUS, 1, 22);
    }

    /// SSTATUS fields
    pub mod sstatus {
        use super::address::SSTATUS;
        pub const SIE: CsrField = CsrField::new(SSTATUS, 1, 1);
        pub const SPIE: CsrField = CsrField::new(SSTATUS, 1, 5);
        pub const SPP: CsrField = CsrField::new(SSTATUS, 1, 8);
        pub const FS: CsrField = CsrField::new(SSTATUS, 0x3, 13);
        pub const XS: CsrField = CsrField::new(SSTATUS, 0x3, 15);
        pub const SUM: CsrField = CsrField::new(SSTATUS, 1, 18);
        pub const MXR: CsrField = CsrField::new(SSTATUS, 1, 19);
        pub const UXL: CsrField = CsrField::new(SSTATUS, 0x3, 32);
    }

    /// HSTATUS fields
    pub mod hstatus {
        use super::address::HSTATUS;
        pub const VTSR: CsrField = CsrField::new(HSTATUS, 1, 22);
        pub const VTW: CsrField = CsrField::new(HSTATUS, 1, 21);
        pub const VTVM: CsrField = CsrField::new(HSTATUS, 1, 20);
        pub const VGEIN: CsrField = CsrField::new(HSTATUS, 0x3F, 12);
        pub const HU: CsrField = CsrField::new(HSTATUS, 1, 9);
        pub const SPVP: CsrField = CsrField::new(HSTATUS, 1, 8);
        pub const SPV: CsrField = CsrField::new(HSTATUS, 1, 7);
        pub const GVA: CsrField = CsrField::new(HSTATUS, 1, 6);
        pub const VSBE: CsrField = CsrField::new(HSTATUS, 1, 5);
    }

    /// HGATP fields
    pub mod hgatp {
        use super::address::HGATP;
        pub const MODE: CsrField = CsrField::new(HGATP, 0xF, 60);
        pub const VMID: CsrField = CsrField::new(HGATP, 0x3FFF, 12);
        pub const PPN: CsrField = CsrField::new(HGATP, 0xFFFFFFFFFFF, 0);
    }

    /// VSATP fields
    pub mod vsatp {
        use super::address::VSATP;
        pub const MODE: CsrField = CsrField::new(VSATP, 0x1F, 60);
        pub const ASID: CsrField = CsrField::new(VSATP, 0xFFFF, 16);
        pub const PPN: CsrField = CsrField::new(VSATP, 0xFFFFFFFFFFF, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_h_extension_csrs() {
        // Test HSTATUS operations
        let hstatus = HstatusFlags::VTVM | HstatusFlags::SPV;
        HSTATUS::write(hstatus);
        let read_hstatus = HSTATUS::read();
        assert!(read_hstatus.contains(HstatusFlags::VTVM));
        assert!(read_hstatus.contains(HstatusFlags::SPV));
    }

    #[test]
    fn test_virtual_supervisor_csrs() {
        // Test VSSTATUS operations
        let vsstatus = VsstatusFlags::SIE | VsstatusFlags::SUM;
        VSSTATUS::write(vsstatus);
        let read_vsstatus = VSSTATUS::read();
        assert!(read_vsstatus.contains(VsstatusFlags::SIE));
        assert!(read_vsstatus.contains(VsstatusFlags::SUM));

        // Test VSTVEC operations
        let base = 0x80000000;
        let mode = VSTVEC::MODE_DIRECT;
        let vstvec = VSTVEC::make(base, mode);
        VSTVEC::write(vstvec);
        assert_eq!(VSTVEC::extract_base(VSTVEC::read()), base);
        assert_eq!(VSTVEC::extract_mode(VSTVEC::read()), mode);
    }

    #[test]
    fn test_csr_field_operations() {
        // Test MSTATUS field operations
        let mpp_field = fields::mstatus::MPP;
        mpp_field.write(0x3); // Machine mode
        assert_eq!(mpp_field.read(), 0x3);

        // Test atomic field operations
        let old_value = mpp_field.read_set_bits(0x1);
        assert_eq!(old_value, 0x3);
        assert_eq!(mpp_field.read(), 0x3);
    }

    #[test]
    fn test_satp_operations() {
        let ppn = 0x87654321;
        let asid = 0x1234;
        let mode = 8; // Sv39

        let satp_value = SATP::make(ppn, asid, mode);
        assert_eq!(SATP::extract_ppn(satp_value), ppn);
        assert_eq!(SATP::extract_asid(satp_value), asid);
        assert_eq!(SATP::extract_mode(satp_value), mode);
    }

    #[test]
    fn test_mstatus_flags() {
        let mstatus = Mstatus::MIE | Mstatus::MPP;
        assert!(mstatus.contains(Mstatus::MIE));
        assert!(mstatus.contains(Mstatus::MPP));
    }

    #[test]
    fn test_hgatp_operations() {
        let ppn = 0x87654321;
        let vmid = 0x5678;
        let mode = 9; // Sv48

        let hgatp_value = virtualization::HGATP::make(ppn, vmid, mode);
        assert_eq!(virtualization::HGATP::extract_ppn(hgatp_value), ppn);
        assert_eq!(virtualization::HGATP::extract_vmid(hgatp_value), vmid);
        assert_eq!(virtualization::HGATP::extract_mode(hgatp_value), mode);
    }

    #[test]
    fn test_hedeleg_delegation() {
        // Test HEDELEG bitflags
        let hedeleg = Hedeleg::ILLEGAL_INSTRUCTION | Hedeleg::BREAKPOINT;
        HEDELEG::write(hedeleg);
        let read_hedeleg = HEDELEG::read();
        assert!(read_hedeleg.contains(Hedeleg::ILLEGAL_INSTRUCTION));
        assert!(read_hedeleg.contains(Hedeleg::BREAKPOINT));

        // Test delegation check
        assert!(HEDELEG::is_delegated(ExceptionCode::IllegalInstruction));
        assert!(HEDELEG::is_delegated(ExceptionCode::Breakpoint));
        assert!(!HEDELEG::is_delegated(ExceptionCode::InstructionMisaligned));

        // Test delegate_all_standard
        HEDELEG::delegate_all_standard();
        let standard_hedeleg = HEDELEG::read();
        assert!(standard_hedeleg.contains(Hedeleg::ILLEGAL_INSTRUCTION));
        assert!(standard_hedeleg.contains(Hedeleg::ECALL_FROM_UMODE));
    }

    #[test]
    fn test_hideleg_delegation() {
        // Test HIDELEG bitflags
        let hideleg = Hideleg::SSIP | Hideleg::SEIP;
        HIDELEG::write(hideleg);
        let read_hideleg = HIDELEG::read();
        assert!(read_hideleg.contains(Hideleg::SSIP));
        assert!(read_hideleg.contains(Hideleg::SEIP));

        // Test delegation check
        assert!(HIDELEG::is_delegated(InterruptCause::SupervisorSoftware));
        assert!(HIDELEG::is_delegated(InterruptCause::SupervisorExternal));
        assert!(!HIDELEG::is_delegated(InterruptCause::SupervisorTimer));

        // Test delegate_all_standard
        HIDELEG::delegate_all_standard();
        let standard_hideleg = HIDELEG::read();
        assert!(standard_hideleg.contains(Hideleg::SSIP));
        assert!(standard_hideleg.contains(Hideleg::STIP));
        assert!(standard_hideleg.contains(Hideleg::SEIP));
    }
}