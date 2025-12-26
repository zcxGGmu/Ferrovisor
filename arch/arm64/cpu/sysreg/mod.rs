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

// Re-export commonly used types
pub use state::{SysRegs, TrapState};
pub use dispatch::{SysRegEncoding, Cp15Encoding, RegReadResult, RegWriteResult, SysRegDispatcher};
pub use trap::{TrapHandler, TrapType};

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
