//! RISC-V Virtual Devices Module
//!
//! This module provides virtual device implementations for RISC-V including:
//! - Virtual UART
/// - Virtual NIC
/// - VirtIO devices
/// - Platform devices

use crate::arch::riscv64::virtualization::vm::*;
use crate::arch::riscv64::virtualization::vcpu::*;

/// Re-export virtual device traits
pub use super::vm::VirtualDevice;
pub use super::vm::VmDeviceConfig;

/// Virtual UART device
pub mod uart;

/// Virtual NIC device
pub mod nic;

/// VirtIO device framework
pub mod virtio;

/// Platform virtual devices
pub mod platform;

/// Initialize virtual device subsystem
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing virtual device subsystem");

    // Initialize device frameworks
    uart::init()?;
    nic::init()?;
    virtio::init()?;
    platform::init()?;

    log::info!("Virtual device subsystem initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_init() {
        // Test device initialization
        assert!(init().is_ok());
    }
}