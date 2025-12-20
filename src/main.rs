//! Ferrovisor main entry point
//!
//! This file contains the entry point and early initialization
//! for the Ferrovisor hypervisor.

#![no_std]
#![no_main]

use ferrovisor::{init, run, Error};

/// Early entry point for ARM64
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Disable interrupts early
    cortex_a::asm::dsb(cortex_a::asm::SY);
    cortex_a::asm::isb(cortex_a::asm::SY);

    // Initialize early console
    if cfg!(feature = "debug") {
        // Early debug output
        unsafe {
            // Simple debug output before console is ready
            core::ptr::write_volatile(0x9000000 as *mut u8, b'B');
            core::ptr::write_volatile(0x9000000 as *mut u8, b'o');
            core::ptr::write_volatile(0x9000000 as *mut u8, b'o');
            core::ptr::write_volatile(0x9000000 as *mut u8, b't');
            core::ptr::write_volatile(0x9000000 as *mut u8, b'\n');
        }
    }

    // Call the main initialization
    main_entry()
}

/// Early entry point for RISC-V
#[cfg(target_arch = "riscv64")]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Early debug output
    if cfg!(feature = "debug") {
        unsafe {
            // Simple debug output
            core::ptr::write_volatile(0x10000000 as *mut u8, b'R');
            core::ptr::write_volatile(0x10000000 as *mut u8, b'I');
            core::ptr::write_volatile(0x10000000 as *mut u8, b'S');
            core::ptr::write_volatile(0x10000000 as *mut u8, b'C');
            core::ptr::write_volatile(0x10000000 as *mut u8, b'V');
            core::ptr::write_volatile(0x10000000 as *mut u8, b'\n');
        }
    }

    main_entry()
}

/// Early entry point for x86_64
#[cfg(target_arch = "x86_64")]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // x86_64 early initialization would go here
    main_entry()
}

/// Main entry point - common for all architectures
fn main_entry() -> ! {
    // Initialize Ferrovisor
    match init() {
        Ok(_) => {
            // Initialization successful, enter main loop
            run()
        }
        Err(e) => {
            // Initialization failed
            panic!("Ferrovisor initialization failed: {:?}", e);
        }
    }
}

/// Early panic handler before full console is ready
#[inline(never)]
#[cold]
fn early_panic(msg: &str) -> ! {
    if cfg!(target_arch = "aarch64") {
        // Output to UART at 0x9000000
        for byte in msg.as_bytes() {
            unsafe {
                core::ptr::write_volatile(0x9000000 as *mut u8, *byte);
            }
        }
        // Output panic message end marker
        unsafe {
            for byte in b" - PANIC!\n" {
                core::ptr::write_volatile(0x9000000 as *mut u8, *byte);
            }
        }
    } else if cfg!(target_arch = "riscv64") {
        // Output to UART at 0x10000000
        for byte in msg.as_bytes() {
            unsafe {
                core::ptr::write_volatile(0x10000000 as *mut u8, *byte);
            }
        }
        // Output panic message end marker
        unsafe {
            for byte in b" - PANIC!\n" {
                core::ptr::write_volatile(0x10000000 as *mut u8, *byte);
            }
        }
    }

    // Halt the system
    loop {
        #[cfg(target_arch = "aarch64")]
        cortex_a::asm::wfe();

        #[cfg(target_arch = "riscv64")]
        riscv::asm::wfi();

        #[cfg(target_arch = "x86_64")]
        x86_64::instructions::hlt();
    }
}