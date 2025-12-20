//! Device emulators
//!
//! This module contains emulators for virtual devices that
//! are presented to guest operating systems.

pub mod uart;
pub mod rtc;
pub mod gpio;

use crate::Result;
use crate::utils::spinlock::SpinLock;
use crate::utils::list::List;
use core::ptr::NonNull;

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

/// Emulator entry
#[derive(Debug)]
pub struct EmulatorEntry {
    /// Name of the emulator
    name: String,
    /// Base address
    base_addr: crate::core::mm::PhysAddr,
    /// Size of the address range
    size: u64,
    /// Pointer to emulator
    emulator: *const dyn Emulator,
    /// List node for linked list
    node: crate::utils::list::ListNode,
}

/// Emulator registry
static EMULATOR_REGISTRY: SpinLock<List> = SpinLock::new(List::new());

/// Initialize all emulators
pub fn init() -> Result<()> {
    crate::info!("Initializing device emulators");

    // Initialize UART emulator
    uart::init()?;

    // Initialize RTC emulator
    rtc::init()?;

    // Initialize GPIO emulator
    gpio::init()?;

    Ok(())
}

/// Register a device emulator with address range
pub fn register_emulator(
    name: &str,
    base_addr: crate::core::mm::PhysAddr,
    size: u64,
    emulator: &'static dyn Emulator,
) -> Result<()> {
    let entry = EmulatorEntry {
        name: name.to_string(),
        base_addr,
        size,
        emulator,
        node: crate::utils::list::ListNode::new(),
    };

    // For now, store it in a static array
    // TODO: Implement proper storage
    crate::info!("Registered emulator '{}' at 0x{:x}", name, base_addr);
    Ok(())
}

/// Find an emulator by physical address
pub fn find_emulator_by_addr(addr: crate::core::mm::PhysAddr) -> Option<&'static dyn Emulator> {
    // TODO: Implement proper lookup
    // For now, return None
    None
}

/// Handle a read from emulated device
pub fn handle_read(addr: crate::core::mm::PhysAddr, size: u32) -> Result<u64> {
    if let Some(emulator) = find_emulator_by_addr(addr) {
        emulator.read(addr - 0, size) // TODO: Calculate proper offset
    } else {
        Err(crate::Error::NotFound)
    }
}

/// Handle a write to emulated device
pub fn handle_write(addr: crate::core::mm::PhysAddr, value: u64, size: u32) -> Result<()> {
    // This would require mutable access to the emulator
    // TODO: Implement mutable access handling
    Err(crate::Error::NotImplemented)
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

// Re-export error types for convenience
pub use Error as EmulatorError;