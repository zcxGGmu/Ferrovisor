//! Device drivers
//!
//! This module contains device drivers for various hardware
//! components and virtualization interfaces.

pub mod base;
pub mod virtio;
pub mod platform;

use crate::Result;

/// Driver error type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Device not found
    DeviceNotFound,
    /// Device initialization failed
    InitFailed,
    /// Invalid device state
    InvalidState,
    /// Operation not supported
    NotSupported,
    /// I/O error
    IoError,
    /// Timeout
    Timeout,
    /// Driver-specific error
    Specific(u32),
}

/// Initialize all drivers
pub fn init() -> Result<()> {
    // Initialize platform-specific drivers first
    platform::init()?;

    // Initialize base driver framework
    base::init()?;

    // Initialize VirtIO drivers
    virtio::init()?;

    Ok(())
}

/// Register a device driver
pub fn register_driver(name: &str, driver: &dyn base::Driver) -> Result<()> {
    base::register_driver(name, driver)
}

/// Find a device by name
pub fn find_device(name: &str) -> Result<base::DeviceRef> {
    base::find_device(name)
}