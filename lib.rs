//! Ferrovisor - A Rust-based Type-1 Hypervisor
//!
//! This is the main library for the Ferrovisor hypervisor, providing
//! virtualization capabilities for ARM64, RISC-V, and x86_64 architectures.

#![no_std]
#![no_main]
#![feature(lang_items)]

extern crate alloc;

// Re-export alloc types globally
pub use alloc::vec::Vec;
pub use alloc::boxed::Box;
pub use alloc::string::String;
pub use alloc::format;

// Import allocator components
use alloc::alloc::{GlobalAlloc, Layout};

// Global allocator using our unified allocator
struct FerrovisorAllocator;

unsafe impl GlobalAlloc for FerrovisorAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        match crate::core::mm::allocator::allocate_with_config(
            layout.size(),
            crate::core::mm::allocator::AllocationConfig {
                strategy: crate::core::mm::allocator::AllocationStrategy::Auto,
                alignment: layout.align(),
                zero: false,
                reclaimable: true,
                tag: "global_alloc",
            }
        ) {
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => core::ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if let Some(ptr) = core::ptr::NonNull::new(ptr) {
            let _ = crate::core::mm::allocator::deallocate(
                ptr,
                layout.size(),
                crate::core::mm::allocator::AllocationStrategy::Auto
            );
        }
    }
}

#[global_allocator]
static ALLOCATOR: FerrovisorAllocator = FerrovisorAllocator;

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

// Fallback panic handler for when architecture-specific ones aren't available
#[cfg(not(any(target_arch = "aarch64", target_arch = "riscv64", target_arch = "x86_64")))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    #[cfg(feature = "debug")]
    {
        // Try to output panic info via UART if available
        if let Some(location) = info.location() {
            let _ = write!(
                core::fmt::Formatter::new(),
                "Panic at {}:{}: {}",
                location.file(),
                location.line(),
                info.message().unwrap_or(&"No message")
            );
        } else {
            let _ = write!(
                core::fmt::Formatter::new(),
                "Panic: {}",
                info.message().unwrap_or(&"No message")
            );
        }
    }

    loop {
        #[cfg(target_arch = "aarch64")]
        cortex_a::asm::wfe();

        #[cfg(target_arch = "riscv64")]
        riscv::asm::wfi();

        #[cfg(target_arch = "x86_64")]
        {
            unsafe { core::arch::asm!("hlt"); }
        }
    }
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
#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}
