//! System register emulation for ARM64
//!
//! Provides system register trap handling, state management, and
//! access dispatching for VCPU system register emulation.
//! Reference: xvisor/arch/arm/cpu/arm64/cpu_vcpu_sysregs.c

/// System register state structures
pub mod state;

/// System register access dispatcher
pub mod dispatch;

/// Trap handling for system registers
pub mod trap;

/// ID register emulation
pub mod id_regs;

/// System control register emulation (SCTLR, ACTLR, CPACR)
pub mod sctlr;

/// Memory management register emulation (TTBR, TCR, MAIR, AMAIR)
pub mod mm;

/// Debug register emulation (MDSCR, breakpoint, watchpoint)
pub mod debug;

// Re-export commonly used types
pub use state::{SysRegs, TrapState};
pub use dispatch::{SysRegEncoding, Cp15Encoding, RegReadResult, RegWriteResult, SysRegDispatcher};
pub use trap::{TrapHandler, TrapType};

// Re-export ID registers
pub use id_regs::{
    IdRegisters, IdAa64Pfr0El1, IdAa64Pfr1El1, IdAa64Dfr0El1, IdAa64Dfr1El1,
    IdAa64Isar0El1, IdAa64Isar1El1, IdAa64Isar2El1,
    IdAa64Mmfr0El1, IdAa64Mmfr1El1, IdAa64Mmfr2El1,
    MidrEl1, MpidrEl1, RevidrEl1,
};

// Re-export system control registers
pub use sctlr::{SystemControlRegs, SctlrEl1, ActlrEl1, CpacrEl1};

// Re-export memory management registers
pub use mm::{MemoryMgmtRegs, Ttbr0El1, Ttbr1El1, TcrEl1, MairEl1, AmairEl1};

// Re-export debug registers
pub use debug::{DebugRegs, MdscrEl1, Dbgbcr0El1, Dbgwcr0El1};

/// Initialize system register emulation
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing system register emulation");
    log::info!("System register emulation initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sysregs_size() {
        // Ensure SysRegs has expected size
        assert_eq!(core::mem::size_of::<SysRegs>(), 232);
    }

    #[test]
    fn test_sys_reg_encoding_size() {
        assert_eq!(core::mem::size_of::<SysRegEncoding>(), 5);
        assert_eq!(core::mem::size_of::<Cp15Encoding>(), 4);
    }
}
