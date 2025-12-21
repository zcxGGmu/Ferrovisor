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
    cortex_a::asm::dmb(cortex_a::asm::SY);

    #[cfg(target_arch = "riscv64")]
    riscv::asm::fence(riscv::asm::Ordering::RLR, riscv::asm::Ordering::RLR);

    #[cfg(target_arch = "x86_64")]
    x86_64::instructions::lfence();
}

/// Write memory barrier
#[inline]
pub fn wmb() {
    #[cfg(target_arch = "aarch64")]
    cortex_a::asm::dmb(cortex_a::asm::ST);

    #[cfg(target_arch = "riscv64")]
    riscv::asm::fence(riscv::asm::Ordering::LRW, riscv::asm::Ordering::LRW);

    #[cfg(target_arch = "x86_64")]
    x86_64::instructions::sfence();
}

/// Full memory barrier
#[inline]
pub fn mb() {
    #[cfg(target_arch = "aarch64")]
    cortex_a::asm::dmb(cortex_a::asm::SY);

    #[cfg(target_arch = "riscv64")]
    riscv::asm::fence(riscv::asm::Ordering::RAW, riscv::asm::Ordering::RAW);

    #[cfg(target_arch = "x86_64")]
    x86_64::instructions::mfence();
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
        use x86_64::asm::rdtsc;
        unsafe { rdtsc() }
    }
}

/// Spin for a number of iterations
#[inline]
pub fn spin(iterations: u32) {
    for _ in 0..iterations {
        #[cfg(target_arch = "aarch64")]
        cortex_a::asm::nop();

        #[cfg(target_arch = "riscv64")]
        riscv::asm::nop();

        #[cfg(target_arch = "x86_64")]
        x86_64::instructions::nop();
    }
}