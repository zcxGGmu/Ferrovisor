//! Architecture-specific code
//!
//! This module contains architecture-specific implementations
//! for different CPU architectures supported by Ferrovisor.

#[cfg(target_arch = "aarch64")]
pub mod arm64;

#[cfg(target_arch = "riscv64")]
pub mod riscv64;

#[cfg(target_arch = "x86_64")]
pub mod x86_64;

pub mod common;

use crate::Result;

/// Architecture-specific error type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Unsupported operation on this architecture
    Unsupported,
    /// Invalid CPU state
    InvalidCpuState,
    /// Memory management unit error
    MmuError,
    /// Cache operation error
    CacheError,
    /// TLB operation error
    TlbError,
    /// Architecture-specific error code
    Specific(u32),
}

/// Architecture-specific initialization
pub fn init() -> Result<()> {
    #[cfg(target_arch = "aarch64")]
    {
        arm64::init()?;
    }

    #[cfg(target_arch = "riscv64")]
    {
        riscv64::init()?;
    }

    #[cfg(target_arch = "x86_64")]
    {
        x86_64::init()?;
    }

    Ok(())
}

/// Architecture-specific main loop
pub fn run() -> ! {
    #[cfg(target_arch = "aarch64")]
    {
        arm64::run()
    }

    #[cfg(target_arch = "riscv64")]
    {
        riscv64::run()
    }

    #[cfg(target_arch = "x86_64")]
    {
        x86_64::run()
    }
}