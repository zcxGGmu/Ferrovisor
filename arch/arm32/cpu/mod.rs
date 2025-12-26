//! ARM32 CPU emulation module
//!
//! This module provides ARMv7/ARMv8-AArch32 CPU emulation support
//! for running 32-bit ARM guest operating systems on ARM64 hosts.

pub mod coproc;

// Re-export commonly used types
pub use coproc::*;

/// Initialize ARM32 CPU emulation
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing ARM32 CPU emulation");
    coproc::init()?;
    log::info!("ARM32 CPU emulation initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arm32_init() {
        assert!(init().is_ok());
    }
}
