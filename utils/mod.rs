//! Utility functions and data structures
//!
//! This module contains various utility functions, data structures,
//! and helper code used throughout the hypervisor.

pub mod log;
pub mod console;
pub mod bitmap;
pub mod list;
pub mod time;
pub mod random;

// Re-export commonly used utilities
pub use self::log::*;
pub use self::bitmap::Bitmap;
pub use self::list::List;

/// Utility macros
#[macro_export]
macro_rules! align_up {
    ($addr:expr, $align:expr) => {
        (($addr + $align - 1) / $align * $align)
    };
}

#[macro_export]
macro_rules! align_down {
    ($addr:expr, $align:expr) => {
        ($addr / $align * $align)
    };
}

#[macro_export]
macro_rules! is_aligned {
    ($addr:expr, $align:expr) => {
        $addr % $align == 0
    };
}

/// Read-only memory barrier
#[inline]
pub fn rmb() {
    #[cfg(target_arch = "aarch64")]
    unsafe { core::arch::asm!("dmb sy") };

    #[cfg(target_arch = "riscv64")]
    unsafe { core::arch::asm!("fence r, r") };

    #[cfg(target_arch = "x86_64")]
    unsafe { core::arch::asm!("lfence") };
}

/// Write memory barrier
#[inline]
pub fn wmb() {
    #[cfg(target_arch = "aarch64")]
    unsafe { core::arch::asm!("dmb st") };

    #[cfg(target_arch = "riscv64")]
    unsafe { core::arch::asm!("fence w, w") };

    #[cfg(target_arch = "x86_64")]
    unsafe { core::arch::asm!("sfence") };
}

/// Full memory barrier
#[inline]
pub fn mb() {
    #[cfg(target_arch = "aarch64")]
    unsafe { core::arch::asm!("dmb sy") };

    #[cfg(target_arch = "riscv64")]
    unsafe { core::arch::asm!("fence rw, rw") };

    #[cfg(target_arch = "x86_64")]
    unsafe { core::arch::asm!("mfence") };
}

/// Get a timestamp counter
#[inline]
pub fn get_timestamp() -> u64 {
    #[cfg(target_arch = "aarch64")]
    {
        let mut cnt: u64;
        unsafe {
            core::arch::asm!(
                "mrs {}, cntvct_el0",
                out(reg) cnt,
                options(nomem, nostack, preserves_flags)
            );
        }
        cnt
    }

    #[cfg(target_arch = "riscv64")]
    {
        riscv::register::time::read()
    }

    #[cfg(target_arch = "x86_64")]
    {
        unsafe {
            let mut rax: u64;
            core::arch::asm!(
                "rdtsc",
                out("rax") rax,
                options(nomem, nostack)
            );
            rax
        }
    }
}

/// Spin for a number of iterations
#[inline]
pub fn spin(iterations: u32) {
    for _ in 0..iterations {
        #[cfg(target_arch = "aarch64")]
        unsafe { core::arch::asm!("nop") };

        #[cfg(target_arch = "riscv64")]
        unsafe { core::arch::asm!("nop") };

        #[cfg(target_arch = "x86_64")]
        unsafe { core::arch::asm!("nop") };
    }
}