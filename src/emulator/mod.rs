//! Device emulators
//!
//! This module contains emulators for virtual devices that
//! are presented to guest operating systems.

pub mod uart;
pub mod rtc;
pub mod gpio;

use crate::Result;

/// Emulator error type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Emulation not supported
    NotSupported,
    /// Invalid configuration
    InvalidConfig,
    /// Device not found
    DeviceNotFound,
    /// Invalid register access
    InvalidAccess,
    /// Bus error
    BusError,
    /// Emulator-specific error
    Specific(u32),
}

/// Initialize all emulators
pub fn init() -> Result<()> {
    // Initialize UART emulator
    uart::init()?;

    // Initialize RTC emulator
    rtc::init()?;

    // Initialize GPIO emulator
    gpio::init()?;

    Ok(())
}

/// Register a device emulator
pub fn register_emulator(name: &str, emulator: &dyn Emulator) -> Result<()> {
    // TODO: Implement emulator registry
    Ok(())
}

/// Trait for device emulators
pub trait Emulator {
    /// Get emulator name
    fn name(&self) -> &str;

    /// Read from device register
    fn read(&self, offset: u64, size: u32) -> Result<u64>;

    /// Write to device register
    fn write(&mut self, offset: u64, value: u64, size: u32) -> Result<()>;

    /// Reset device
    fn reset(&mut self) -> Result<()>;
}