//! Configuration module
//!
//! Build-time configuration options

include!(concat!(env!("OUT_DIR"), "/config.rs"));

/// Architecture configuration
pub mod arch {
    pub const PAGE_SIZE: usize = 4096;
    pub const PAGE_SHIFT: usize = 12;
}

/// Memory configuration
pub mod memory {
    pub const HEAP_SIZE: usize = 64 * 1024 * 1024; // 64MB
    pub const STACK_SIZE: usize = 64 * 1024; // 64KB per CPU
}

/// Feature flags
pub mod features {
    pub const DEBUG: bool = cfg!(feature = "debug");
    pub const VERBOSE: bool = cfg!(feature = "verbose");
    pub const LOG: bool = true;
}
