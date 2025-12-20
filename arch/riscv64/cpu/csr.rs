//! RISC-V CSR (Control and Status Register) Access
//!
//! This module provides safe and efficient access to RISC-V CSRs including:
//! - CSR read/write operations
//! - CSR bit manipulation helpers
//! - CSR definitions for all standard extensions
//! - Virtualization CSRs (H extension)

use crate::arch::riscv64::*;
use bitflags::bitflags;

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

impl CsrAccess<usize> for UsizeCsr {
    #[inline]
    fn read() -> usize {
        let value: usize;
        unsafe { core::arch::asm!("csrr {}, {}", out(reg) value, in(reg) Self::0) };
        value
    }

    #[inline]
    fn write(value: usize) {
        unsafe { core::arch::asm!("csrw {}, {}", in(reg) Self::0, in(reg) value) };
    }

    #[inline]
    fn set(bits: usize) {
        unsafe { core::arch::asm!("csrs {}, {}", in(reg) Self::0, in(reg) bits) };
    }

    #[inline]
    fn clear(bits: usize) {
        unsafe { core::arch::asm!("csrc {}, {}", in(reg) Self::0, in(reg) bits) };
    }

    #[inline]
    fn modify(clear: usize, set: usize) -> usize {
        let value: usize;
        unsafe {
            core::arch::asm!("csrrc {}, {}, {}", out(reg) value, in(reg) Self::0, in(reg) clear);
            core::arch::asm!("csrs {}, {}", in(reg) Self::0, in(reg) set);
        }
        value
    }
}

/// CSR access for u64
pub struct U64Csr(pub usize);

impl CsrAccess<u64> for U64Csr {
    #[inline]
    fn read() -> u64 {
        let value: u64;
        unsafe { core::arch::asm!("csrr {}, {}", out(reg) value, in(reg) Self::0) };
        value
    }

    #[inline]
    fn write(value: u64) {
        unsafe { core::arch::asm!("csrw {}, {}", in(reg) Self::0, in(reg) value) };
    }

    #[inline]
    fn set(bits: u64) {
        unsafe { core::arch::asm!("csrs {}, {}", in(reg) Self::0, in(reg) bits) };
    }

    #[inline]
    fn clear(bits: u64) {
        unsafe { core::arch::asm!("csrc {}, {}", in(reg) Self::0, in(reg) bits) };
    }

    #[inline]
    fn modify(clear: u64, set: u64) -> u64 {
        let value: u64;
        unsafe {
            core::arch::asm!("csrrc {}, {}, {}", out(reg) value, in(reg) Self::0, in(reg) clear);
            core::arch::asm!("csrs {}, {}", in(reg) Self::0, in(reg) set);
        }
        value
    }
}

/// Machine status register
pub struct MSTATUS;
impl MSTATUS {
    pub const CSR: usize = csr::MSTATUS;

    #[inline]
    pub fn read() -> Mstatus {
        let value = UsizeCsr(Self::CSR).read();
        Mstatus::from_bits_truncate(value)
    }

    #[inline]
    pub fn write(value: Mstatus) {
        UsizeCsr(Self::CSR).write(value.bits());
    }

    #[inline]
    pub fn set(bits: Mstatus) {
        UsizeCsr(Self::CSR).set(bits.bits());
    }

    #[inline]
    pub fn clear(bits: Mstatus) {
        UsizeCsr(Self::CSR).clear(bits.bits());
    }

    #[inline]
    pub fn modify(clear: Mstatus, set: Mstatus) {
        UsizeCsr(Self::CSR).modify(clear.bits(), set.bits());
    }
}

/// Supervisor status register
pub struct SSTATUS;
impl SSTATUS {
    pub const CSR: usize = csr::SSTATUS;

    #[inline]
    pub fn read() -> Sstatus {
        let value = UsizeCsr(Self::CSR).read();
        Sstatus::from_bits_truncate(value)
    }

    #[inline]
    pub fn write(value: Sstatus) {
        UsizeCsr(Self::CSR).write(value.bits());
    }

    #[inline]
    pub fn set(bits: Sstatus) {
        UsizeCsr(Self::CSR).set(bits.bits());
    }

    #[inline]
    pub fn clear(bits: Sstatus) {
        UsizeCsr(Self::CSR).clear(bits.bits());
    }

    #[inline]
    pub fn modify(clear: Sstatus, set: Sstatus) {
        UsizeCsr(Self::CSR).modify(clear.bits(), set.bits());
    }
}

/// Machine interrupt-enable register
pub struct MIE;
impl MIE {
    pub const CSR: usize = csr::MIE;

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
    pub const CSR: usize = csr::SIE;

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
    pub const CSR: usize = csr::MIP;

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
    pub const CSR: usize = csr::SIP;

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
    pub const CSR: usize = csr::SATP;

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
    pub const CSR: usize = csr::HSTATUS;

    #[inline]
    pub fn read() -> Hstatus {
        let value = UsizeCsr(Self::CSR).read();
        Hstatus::from_bits_truncate(value)
    }

    #[inline]
    pub fn write(value: Hstatus) {
        UsizeCsr(Self::CSR).write(value.bits());
    }

    #[inline]
    pub fn set(bits: Hstatus) {
        UsizeCsr(Self::CSR).set(bits.bits());
    }

    #[inline]
    pub fn clear(bits: Hstatus) {
        UsizeCsr(Self::CSR).clear(bits.bits());
    }

    #[inline]
    pub fn modify(clear: Hstatus, set: Hstatus) {
        UsizeCsr(Self::CSR).modify(clear.bits(), set.bits());
    }
}

/// CSR operations for virtualization
pub mod virtualization {
    use super::*;

    /// HEDELEG (Hypervisor Exception Delegation) register
    pub struct HEDELEG;
    impl HEDELEG {
        pub const CSR: usize = csr::HEDELEG;

        #[inline]
        pub fn read() -> usize {
            UsizeCsr(Self::CSR).read()
        }

        #[inline]
        pub fn write(value: usize) {
            UsizeCsr(Self::CSR).write(value);
        }

        #[inline]
        pub fn set(bits: usize) {
            UsizeCsr(Self::CSR).set(bits);
        }

        #[inline]
        pub fn clear(bits: usize) {
            UsizeCsr(Self::CSR).clear(bits);
        }
    }

    /// HIDELEG (Hypervisor Interrupt Delegation) register
    pub struct HIDELEG;
    impl HIDELEG {
        pub const CSR: usize = csr::HIDELEG;

        #[inline]
        pub fn read() -> usize {
            UsizeCsr(Self::CSR).read()
        }

        #[inline]
        pub fn write(value: usize) {
            UsizeCsr(Self::CSR).write(value);
        }

        #[inline]
        pub fn set(bits: usize) {
            UsizeCsr(Self::CSR).set(bits);
        }

        #[inline]
        pub fn clear(bits: usize) {
            UsizeCsr(Self::CSR).clear(bits);
        }
    }

    /// HGATP (Hypervisor Guest Address Translation and Protection) register
    pub struct HGATP;
    impl HGATP {
        pub const CSR: usize = 0x680;

        #[inline]
        pub fn read() -> usize {
            UsizeCsr(Self::CSR).read()
        }

        #[inline]
        pub fn write(value: usize) {
            UsizeCsr(Self::CSR).write(value);
        }

        #[inline]
        pub fn make(ppn: usize, vmid: usize, mode: usize) -> usize {
            (ppn << 44) | ((vmid & 0xFFFF) << 12) | (mode & 0xF)
        }

        #[inline]
        pub fn extract_ppn(value: usize) -> usize {
            value >> 44
        }

        #[inline]
        pub fn extract_vmid(value: usize) -> usize {
            (value >> 12) & 0xFFFF
        }

        #[inline]
        pub fn extract_mode(value: usize) -> usize {
            value & 0xF
        }
    }
}

/// CSR initialization
pub fn init() -> Result<(), &'static str> {
    log::debug!("Initializing CSR access");

    // Set up initial machine mode configuration
    MSTATUS::write(Mstatus::MIE);

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

#[cfg(test)]
mod tests {
    use super::*;

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
}