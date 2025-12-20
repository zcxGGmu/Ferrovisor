#![no_std]
#![no_main]
#![feature(lang_items)]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

//! Ferrovisor - A Rust-based Type-1 Hypervisor
//!
//! This is the main library for the Ferrovisor hypervisor, providing
//! virtualization capabilities for ARM64, RISC-V, and x86_64 architectures.

// Core modules
#[macro_use]
pub mod utils;
pub mod config;

// Architecture-specific code
pub mod arch;

// Core hypervisor modules
pub mod core;

// Device drivers
pub mod drivers;

// Device emulators
pub mod emulator;

// Common libraries
pub mod libs;

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

// ... rest of the original lib.rs content ...
