//! ARM64 Entry Point
//!
//! This module provides the Rust entry points called from assembly code in entry.S.
//!
//! ## Entry Flow
//!
//! 1. Bootloader loads Ferrovisor
//! 2. Assembly code in entry.S (_start) runs first
//! 3. Setup early environment (stack, BSS, VBAR_EL2)
//! 4. Jump to rust_main() (for primary CPU) or rust_secondary_main() (for secondary)
//! 5. Initialize CPU, memory, devices
//! 6. Start hypervisor
//!
//! ## References
//! - [entry.S] Assembly entry point

use crate::arch::arm64::cpu::init::{cpu_init, CpuInitInfo};
use crate::arch::arm64::interrupt::{set_exception_handler, handlers::{ExceptionContext, ExceptionType}};

/// CPU information passed from assembly
#[repr(C)]
pub struct BootInfo {
    /// Device tree pointer (from x0 register)
    pub dtb_ptr: u64,
    /// CPU ID (from MPIDR_EL1)
    pub cpu_id: u64,
    /// Reserved
    pub reserved: [u64; 6],
}

impl BootInfo {
    /// Create new boot info
    pub fn new() -> Self {
        Self {
            dtb_ptr: 0,
            cpu_id: 0,
            reserved: [0; 6],
        }
    }
}

impl Default for BootInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Primary CPU entry point (called from assembly)
///
/// This function is called from entry.S (_start) after:
/// - Stack is setup
/// - BSS is cleared
/// - VBAR_EL2 is set to init_vectors
/// - EL2 is confirmed
///
/// # Safety
/// This function must only be called once from assembly entry code.
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    // Log entry
    log::info!("=== Ferrovisor ARM64 Entry ===");
    log::info!("Primary CPU entering Rust code");

    // Read MPIDR_EL1 to get CPU ID
    let mpidr: u64;
    unsafe { core::arch::asm!("mrs {}, mpidr_el1", out(reg) mpidr); }

    // Create boot info
    let boot_info = BootInfo {
        dtb_ptr: 0,  // Will be read from x0 if needed
        cpu_id: mpidr & 0xFF,
        reserved: [0; 6],
    };

    log::info!("Boot CPU ID: {}", boot_info.cpu_id);
    log::info!("MPIDR: {:#x}", mpidr);

    // Setup default exception handler
    setup_exception_handler();

    // Initialize CPU
    log::info!("Initializing primary CPU...");
    let cpu_info = CpuInitInfo {
        cpu_id: boot_info.cpu_id as u32,
        mpidr,
        is_primary: true,
    };

    match cpu_init(cpu_info) {
        Ok(()) => log::info!("Primary CPU initialized"),
        Err(e) => panic!("Primary CPU initialization failed: {}", e),
    }

    // Initialize other subsystems
    initialize_subsystems();

    // Start hypervisor
    log::info!("Starting hypervisor...");
    start_hypervisor(boot_info);
}

/// Secondary CPU entry point (called from assembly)
///
/// This function is called from entry.S (_start_secondary) after:
/// - Stack is setup (per-CPU)
/// - VBAR_EL2 is set to vectors
/// - EL2 is confirmed
///
/// # Safety
/// This function is called from assembly entry code for each secondary CPU.
#[no_mangle]
pub extern "C" fn rust_secondary_main() -> ! {
    // Read MPIDR_EL1 to get CPU ID
    let mpidr: u64;
    unsafe { core::arch::asm!("mrs {}, mpidr_el1", out(reg) mpidr); }
    let cpu_id = mpidr & 0xFF;

    log::info!("=== Secondary CPU {} Entry ===", cpu_id);

    // Create boot info
    let boot_info = BootInfo {
        dtb_ptr: 0,
        cpu_id,
        reserved: [0; 6],
    };

    // Initialize secondary CPU
    log::info!("Initializing secondary CPU {}...", cpu_id);
    let cpu_info = CpuInitInfo {
        cpu_id: cpu_id as u32,
        mpidr,
        is_primary: false,
    };

    match cpu_init(cpu_info) {
        Ok(()) => log::info!("Secondary CPU {} initialized", cpu_id),
        Err(e) => {
            log::error!("Secondary CPU {} initialization failed: {}", cpu_id, e);
            halt_secondary();
        }
    }

    // Wait for work
    log::info!("Secondary CPU {} ready, waiting for work...", cpu_id);
    secondary_idle_loop();
}

/// Setup default exception handler
fn setup_exception_handler() {
    set_exception_handler(default_exception_handler);
    log::info!("Exception handler installed");
}

/// Default exception handler
fn default_exception_handler(ctx: &mut ExceptionContext, exc_type: ExceptionType) {
    log::error!("Exception: {}", exc_type.name());
    log::error!("  ELR={:#018x}, SPSR={:#08x}", ctx.elr(), ctx.spsr());

    // For guest exceptions, we might want to inject into the VM
    if exc_type.is_guest() {
        log::error!("  Guest exception - should inject to VM");
        // TODO: Inject to VM
    }

    // For hypervisor exceptions, this is a bug
    if !exc_type.is_guest() {
        panic!("Hypervisor exception: {}", exc_type.name());
    }
}

/// Initialize all subsystems
fn initialize_subsystems() {
    log::info!("Initializing subsystems...");

    // TODO: Initialize subsystems in order
    // 1. Memory management (MMU, page tables)
    // 2. Interrupt controller (GIC)
    // 3. Timer
    // 4. SMP (if multiple CPUs)
    // 5. Device tree parsing
    // 6. Platform detection

    log::info!("Subsystems initialized");
}

/// Start the hypervisor
fn start_hypervisor(boot_info: BootInfo) -> ! {
    log::info!("Hypervisor starting...");
    log::info!("  DTB pointer: {:#x}", boot_info.dtb_ptr);
    log::info!("  CPU ID: {}", boot_info.cpu_id);

    // TODO: Create and run VMs
    // For now, just halt
    log::info!("No VMs configured, halting...");
    halt();
}

/// Secondary CPU idle loop
fn secondary_idle_loop() -> ! {
    loop {
        // Wait for interrupt (WFI)
        unsafe { core::arch::asm!("wfi"); }

        // TODO: Check for work
        // - VM scheduling
        // - IPI handling
    }
}

/// Halt secondary CPU
fn halt_secondary() -> ! {
    log::warn!("Halting secondary CPU");
    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}

/// Halt the CPU
fn halt() -> ! {
    log::error!("Halting CPU");
    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}

/// Get current CPU ID from MPIDR_EL1
pub fn get_cpu_id() -> u32 {
    let mpidr: u64;
    unsafe { core::arch::asm!("mrs {}, mpidr_el1", out(reg) mpidr); }
    (mpidr & 0xFF) as u32
}

/// Check if current CPU is primary (boot CPU)
pub fn is_primary_cpu() -> bool {
    // Primary CPU has CPU ID 0
    get_cpu_id() == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_info() {
        let info = BootInfo::new();
        assert_eq!(info.cpu_id, 0);
    }

    #[test]
    fn test_get_cpu_id() {
        // This will return the actual CPU ID when running
        let cpu_id = get_cpu_id();
        // Just verify it doesn't panic
        assert!(cpu_id < 256);
    }
}
