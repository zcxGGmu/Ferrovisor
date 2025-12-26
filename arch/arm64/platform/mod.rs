//! Platform-specific support for ARM64
//!
//! Provides board and platform initialization for various ARM64 platforms:
//! - QEMU virt platform
//! - ARM Foundation v8 model
//! - Other ARM64 boards (optional)

pub mod qemu_virt;
pub mod foundation_v8;

pub use qemu_virt::*;
pub use foundation_v8::*;

/// Platform trait - common interface for all platforms
pub trait Platform {
    /// Get platform name
    fn name(&self) -> &str;

    /// Get platform compatible string
    fn compatible(&self) -> &str;

    /// Get memory layout
    fn memory_layout(&self) -> &[(u64, u64)];

    /// Get GIC base address
    fn gic_base(&self) -> u64;

    /// Get GIC version
    fn gic_version(&self) -> u32;

    /// Get UART base address
    fn uart_base(&self) -> Option<u64>;

    /// Early initialization
    fn early_init(&mut self) -> Result<(), &'static str>;

    /// Final initialization
    fn final_init(&mut self) -> Result<(), &'static str>;
}

/// Detect current platform from device tree
pub fn detect_platform() -> Result<&'static dyn Platform, &'static str> {
    // Try QEMU virt first
    if let Ok(qemu) = qemu_virt::QemuVirtPlatform::probe() {
        return Ok(&qemu);
    }

    // Try Foundation v8
    if let Ok(foundation) = foundation_v8::FoundationV8Platform::probe() {
        return Ok(&foundation);
    }

    Err("No supported platform detected")
}

/// Initialize platform
pub fn init() -> Result<(), &'static str> {
    log::info!("Platform: Detecting ARM64 platform");

    // Detect platform
    let platform = detect_platform()?;
    log::info!("Platform: Detected {}", platform.name());

    // Early init
    platform.early_init()?;

    log::info!("Platform: {} initialized successfully", platform.name());
    Ok(())
}

/// Default platform (used if device tree is not available)
pub static DEFAULT_PLATFORM: Option<&'static dyn Platform> = None;

/// Get current platform
pub fn get_platform() -> Option<&'static dyn Platform> {
    if let Some(p) = DEFAULT_PLATFORM {
        Some(p)
    } else {
        detect_platform().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detect() {
        // This just verifies the interface exists
        // Actual detection requires a device tree
        let result = detect_platform();
        // May fail on systems without device tree
        drop(result);
    }
}
