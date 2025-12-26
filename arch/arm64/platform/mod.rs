//! Platform-specific support for ARM64
//!
//! Provides board and platform initialization.

/// QEMU virt platform
pub mod qemu_virt;

/// Foundation v8 model
pub mod foundation_v8;

/// Initialize platform
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing ARM64 platform");
    log::info!("ARM64 platform initialized");
    Ok(())
}
