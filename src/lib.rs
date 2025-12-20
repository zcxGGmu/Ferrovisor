#![no_std]
#![no_main]
#![feature(lang_items)]
#![feature(panic_info_message)]
#![feature(const_fn)]
#![feature(alloc_error_handler)]

//! Ferrovisor - A Rust-based Type-1 Hypervisor
//!
//! This is the main library for the Ferrovisor hypervisor, providing
//! virtualization capabilities for ARM64, RISC-V, and x86_64 architectures.

// Include generated configuration
include!(concat!(env!("OUT_DIR"), "/config.rs"));

// Core modules
#[macro_use]
mod utils;
mod config;
mod core;
mod drivers;
mod emulator;
mod arch;

// Re-export key modules for convenience
pub use arch::*;
pub use core::*;
pub use drivers::*;
pub use emulator::*;
pub use utils::*;

/// Ferrovisor version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Ferrovisor initialization
pub fn init() -> Result<(), Error> {
    // Initialize architecture-specific code
    arch::init()?;

    // Initialize core components
    core::init()?;

    // Initialize drivers
    drivers::init()?;

    // Initialize emulators
    emulator::init()?;

    log::info!("Ferrovisor v{} initialized successfully", VERSION);

    Ok(())
}

/// Main hypervisor loop
pub fn run() -> ! {
    log::info!("Starting Ferrovisor main loop");

    // Architecture-specific main loop
    arch::run()
}

/// Common error type for Ferrovisor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Invalid argument
    InvalidArgument,
    /// Out of memory
    OutOfMemory,
    /// Not found
    NotFound,
    /// Permission denied
    PermissionDenied,
    /// Resource busy
    ResourceBusy,
    /// Resource unavailable
    ResourceUnavailable,
    /// Timeout
    Timeout,
    /// Not implemented
    NotImplemented,
    /// Not initialized
    NotInitialized,
    /// Invalid state
    InvalidState,
    /// Architecture-specific error
    ArchError(arch::Error),
    /// Core error
    CoreError(core::Error),
    /// Driver error
    DriverError(drivers::Error),
}

impl From<arch::Error> for Error {
    fn from(err: arch::Error) -> Self {
        Error::ArchError(err)
    }
}

impl From<core::Error> for Error {
    fn from(err: core::Error) -> Self {
        Error::CoreError(err)
    }
}

impl From<drivers::Error> for Error {
    fn from(err: drivers::Error) -> Self {
        Error::DriverError(err)
    }
}

/// Result type alias
pub type Result<T> = core::result::Result<T, Error>;

// Panic handler
#[cfg(target_arch = "aarch64")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    arch::aarch64::panic(info)
}

#[cfg(target_arch = "riscv64")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    arch::riscv64::panic(info)
}

#[cfg(target_arch = "x86_64")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    arch::x86_64::panic(info)
}

// Language items
#[cfg(target_arch = "aarch64")]
#[lang = "eh_personality"]
extern "C" fn eh_personality() {
    arch::aarch64::eh_personality()
}

#[cfg(target_arch = "riscv64")]
#[lang = "eh_personality"]
extern "C" fn eh_personality() {
    arch::riscv64::eh_personality()
}

#[cfg(target_arch = "x86_64")]
#[lang = "eh_personality"]
extern "C" fn eh_personality() {
    arch::x86_64::eh_personality()
}

// Alloc error handler
#[cfg(feature = "allocator")]
#[alloc_error_handler]
fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}