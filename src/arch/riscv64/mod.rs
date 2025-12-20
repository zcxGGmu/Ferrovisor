//! RISC-V 64-bit Architecture Support for Ferrovisor
//!
//! This module provides complete RISC-V 64-bit architecture support including:
//! - CPU register and CSR management
//! - MMU and memory management
//! - Interrupt and exception handling
//! - Virtualization extensions (H Extension)
//! - SMP support
//! - Device tree support

pub mod cpu;
pub mod mmu;
pub mod interrupt;
pub mod virtualization;
pub mod smp;
pub mod devtree;
pub mod platform;

// Re-export key types and functions
pub use cpu::*;
pub use mmu::*;
pub use interrupt::*;
pub use virtualization::*;
pub use smp::*;
pub use devtree::*;
pub use platform::*;

/// RISC-V 64-bit architecture version
pub const ARCH_VERSION: &str = "riscv64";

/// RISC-V 64-bit physical address width
pub const PA_WIDTH: usize = 56;

/// RISC-V 64-bit virtual address width
pub const VA_WIDTH: usize = 48;

/// Page size (4KB)
pub const PAGE_SIZE: usize = 4096;

/// Page shift
pub const PAGE_SHIFT: usize = 12;

/// Number of page table levels for Sv39
pub const SV39_LEVELS: usize = 3;

/// Number of page table levels for Sv48
pub const SV48_LEVELS: usize = 4;

/// Maximum number of CPUs supported
pub const MAX_CPUS: usize = 16;

/// RISC-V privilege levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(usize)]
pub enum PrivilegeLevel {
    User = 0,
    Supervisor = 1,
    Reserved = 2,
    Machine = 3,
}

/// RISC-V XLEN (register width) for different modes
pub const XLEN: usize = 64;

/// RISC-V instruction length (in bytes)
pub const INSN_LEN: usize = 4;

/// RISC-V exception codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// RISC-V interrupt causes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum InterruptCause {
    SupervisorSoftware = 1,
    MachineSoftware = 3,
    SupervisorTimer = 5,
    MachineTimer = 7,
    SupervisorExternal = 9,
    MachineExternal = 11,
}

/// CSR register addresses
pub mod csr {
    /// User Trap Setup
    pub const USTATUS: usize = 0x000;
    pub const UIE: usize = 0x004;
    pub const UTVEC: usize = 0x005;

    /// User Trap Handling
    pub const USCRATCH: usize = 0x040;
    pub const UEPC: usize = 0x041;
    pub const UCAUSE: usize = 0x042;
    pub const UTVAL: usize = 0x043;
    pub const UIP: usize = 0x044;

    /// Supervisor Trap Setup
    pub const SSTATUS: usize = 0x100;
    pub const SIE: usize = 0x104;
    pub const STVEC: usize = 0x105;
    pub const SCOUNTEREN: usize = 0x106;

    /// Supervisor Trap Handling
    pub const SSCRATCH: usize = 0x140;
    pub const SEPC: usize = 0x141;
    pub const SCAUSE: usize = 0x142;
    pub const STVAL: usize = 0x143;
    pub const SIP: usize = 0x144;

    /// Supervisor Protection and Translation
    pub const SATP: usize = 0x180;

    /// Machine Trap Setup
    pub const MSTATUS: usize = 0x300;
    pub const MISA: usize = 0x301;
    pub const MEDELEG: usize = 0x302;
    pub const MIDELEG: usize = 0x303;
    pub const MIE: usize = 0x304;
    pub const MTVEC: usize = 0x305;
    pub const MCOUNTEREN: usize = 0x306;

    /// Machine Trap Handling
    pub const MSCRATCH: usize = 0x340;
    pub const MEPC: usize = 0x341;
    pub const MCAUSE: usize = 0x342;
    pub const MTVAL: usize = 0x343;
    pub const MIP: usize = 0x344;

    /// Machine Memory Protection
    pub const PMPCFG0: usize = 0x3A0;
    pub const PMPCFG1: usize = 0x3A1;
    pub const PMPCFG2: usize = 0x3A2;
    pub const PMPCFG3: usize = 0x3A3;
    pub const PMPADDR0: usize = 0x3B0;
    // ... PMPADDR15 = 0x3BF

    /// Hypervisor Trap Setup
    pub const HSTATUS: usize = 0x600;
    pub const HEDELEG: usize = 0x602;
    pub const HIDELEG: usize = 0x603;
    pub const HIE: usize = 0x604;
    pub const HCOUNTEREN: usize = 0x606;
    pub const HGEIE: usize = 0x607;

    /// Hypervisor Trap Handling
    pub const HTVAL: usize = 0x643;
    pub const HIP: usize = 0x644;
    pub const HVIP: usize = 0x645;
    pub const HTINST: usize = 0x64A;
    pub const HGEIP: usize = 0xE12;

    /// Hypervisor Shadow CSRs
    pub const HSTATUS: usize = 0x600;
    pub const HIDELEG: usize = 0x603;
    pub const HVIP: usize = 0x645;

    /// Virtual Supervisor Registers
    pub const VSSTATUS: usize = 0x200;
    pub const VSIE: usize = 0x204;
    pub const VSTVEC: usize = 0x205;
    pub const VSSCRATCH: usize = 0x240;
    pub const VSEPC: usize = 0x241;
    pub const VSCAUSE: usize = 0x242;
    pub const VSTVAL: usize = 0x243;
    pub const VSIP: usize = 0x244;
    pub const VSATP: usize = 0x280;

    /// Debug/Trace Registers
    pub const TSELECT: usize = 0x7A0;
    pub const TDATA1: usize = 0x7A1;
    pub const TDATA2: usize = 0x7A2;
    pub const TDATA3: usize = 0x7A3;

    /// Machine Information Registers
    pub const MVENDORID: usize = 0xF11;
    pub const MARCHID: usize = 0xF12;
    pub const MIMPID: usize = 0xF13;
    pub const MHARTID: usize = 0xF14;
    pub const MCONFIGPTR: usize = 0xF15;

    /// Counters and Timers
    pub const MCYCLE: usize = 0xB00;
    pub const MINSTRET: usize = 0xB02;

    /// Machine Configuration Registers
    pub const MENVCFG: usize = 0x30A;
    pub const MSECCFG: usize = 0x747;
}

/// Initialize RISC-V 64-bit architecture
pub fn arch_init() -> Result<(), &'static str> {
    log::info!("Initializing RISC-V 64-bit architecture");

    // Initialize CPU management
    cpu::init()?;

    // Initialize MMU
    mmu::init()?;

    // Initialize interrupt handling
    interrupt::init()?;

    // Initialize virtualization
    virtualization::init()?;

    // Initialize SMP
    smp::init()?;

    log::info!("RISC-V 64-bit architecture initialized successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_riscv_constants() {
        assert_eq!(PAGE_SIZE, 4096);
        assert_eq!(PAGE_SHIFT, 12);
        assert_eq!(XLEN, 64);
        assert_eq!(VA_WIDTH, 48);
        assert_eq!(PA_WIDTH, 56);
    }

    #[test]
    fn test_privilege_levels() {
        assert_eq!(PrivilegeLevel::User as usize, 0);
        assert_eq!(PrivilegeLevel::Supervisor as usize, 1);
        assert_eq!(PrivilegeLevel::Machine as usize, 3);
    }
}