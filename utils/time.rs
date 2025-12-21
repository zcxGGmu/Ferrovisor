//! Time utilities
//!
//! This module provides time-related utility functions used throughout the hypervisor.

/// Get timestamp in nanoseconds
pub fn timestamp_ns() -> u64 {
    crate::utils::get_timestamp() * 1000 // Assuming get_timestamp returns microseconds
}

/// Get timestamp in microseconds
pub fn timestamp_us() -> u64 {
    crate::utils::get_timestamp()
}

/// Get timestamp in milliseconds
pub fn timestamp_ms() -> u64 {
    crate::utils::get_timestamp() / 1000
}

/// Simple delay function (busy-wait)
pub fn delay_us(microseconds: u32) {
    let start = crate::utils::get_timestamp();
    let end = start + microseconds as u64;

    while crate::utils::get_timestamp() < end {
        crate::utils::spin(10);
    }
}

/// Simple delay function in milliseconds
pub fn delay_ms(milliseconds: u32) {
    delay_us(milliseconds * 1000);
}