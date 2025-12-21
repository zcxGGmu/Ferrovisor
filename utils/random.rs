//! Random number utilities
//!
//! This module provides pseudo-random number generation functions.

/// Simple pseudo-random number generator state
static mut RANDOM_STATE: u32 = 0xdeadbeef;

/// Simple linear congruential generator
pub fn lcg() -> u32 {
    unsafe {
        RANDOM_STATE = RANDOM_STATE.wrapping_mul(1103515245).wrapping_add(12345);
        RANDOM_STATE
    }
}

/// Generate a random u32 value
pub fn u32() -> u32 {
    lcg()
}

/// Generate a random u64 value
pub fn u64() -> u64 {
    ((u32() as u64) << 32) | (u32() as u64)
}

/// Generate a random boolean value
pub fn bool() -> bool {
    (u32() & 1) == 1
}

/// Generate a random value in a range
pub fn range(min: u32, max: u32) -> u32 {
    if min >= max {
        return min;
    }
    min + (u32() % (max - min))
}

/// Generate a random value in an inclusive range
pub fn range_inclusive(min: u32, max: u32) -> u32 {
    if min >= max {
        return min;
    }
    min + (u32() % (max - min + 1))
}

/// Seed the random number generator
pub fn seed(seed: u32) {
    unsafe {
        RANDOM_STATE = seed;
    }
}

/// Initialize random generator with timestamp
pub fn init() {
    seed(crate::utils::get_timestamp() as u32);
}