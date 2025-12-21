//! Synchronization primitives
//!
//! This module provides synchronization primitives suitable for
//! use in the hypervisor kernel environment.

use crate::Result;

pub mod mutex;
pub mod spinlock;
pub mod semaphore;

// Re-export SpinLock for convenience
pub use spinlock::SpinLock;

/// Initialize synchronization subsystem
pub fn init() -> Result<()> {
    // Initialize any global synchronization state
    Ok(())
}