//! Simple spinlock implementation
//!
//! Provides a basic spinlock that busy-waits until the lock is acquired.
//! Suitable for short critical sections.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

/// A simple spinlock
pub struct SpinLock<T> {
    /// Atomic flag indicating if the lock is held
    locked: AtomicBool,
    /// The data protected by the lock
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for SpinLock<T> {}
unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    /// Create a new spinlock
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    /// Try to acquire the lock without blocking
    pub fn try_lock(&self) -> Option<SpinLockGuard<T>> {
        if self.locked.compare_exchange_weak(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).is_ok() {
            Some(SpinLockGuard { lock: self })
        } else {
            None
        }
    }

    /// Acquire the lock, blocking until it's available
    pub fn lock(&self) -> SpinLockGuard<T> {
        while !self.try_lock().is_some() {
            // Spin until we can acquire the lock
            #[cfg(target_arch = "aarch64")]
            cortex_a::asm::yield();

            #[cfg(target_arch = "riscv64")]
            riscv::asm::pause();

            #[cfg(target_arch = "x86_64")]
            x86_64::instructions::pause();
        }

        // The try_lock() call succeeded, so we can unwrap
        self.try_lock().unwrap()
    }

    /// Force unlock the lock (DANGEROUS!)
    ///
    /// # Safety
    /// This function is unsafe because it breaks the mutual exclusion
    /// guarantees of the lock. It should only be used in exceptional
    /// circumstances, such as during panic handling.
    pub unsafe fn force_unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }

    /// Check if the lock is currently held
    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Acquire)
    }
}

/// A guard that provides access to the data protected by a SpinLock
pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<'a, T> Deref for SpinLockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> DerefMut for SpinLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T> Drop for SpinLockGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
    }
}

/// A raw spinlock without associated data
pub struct RawSpinLock {
    locked: AtomicBool,
}

impl RawSpinLock {
    /// Create a new raw spinlock
    pub const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
        }
    }

    /// Acquire the lock
    pub fn lock(&self) {
        while self.locked.compare_exchange_weak(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).is_err() {
            #[cfg(target_arch = "aarch64")]
            cortex_a::asm::yield();

            #[cfg(target_arch = "riscv64")]
            riscv::asm::pause();

            #[cfg(target_arch = "x86_64")]
            x86_64::instructions::pause();
        }
    }

    /// Try to acquire the lock
    pub fn try_lock(&self) -> bool {
        self.locked.compare_exchange(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).is_ok()
    }

    /// Release the lock
    pub fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }

    /// Check if the lock is held
    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Acquire)
    }
}

unsafe impl Send for RawSpinLock {}
unsafe impl Sync for RawSpinLock {}

/// A simple ticket lock for fair spin waiting
pub struct TicketLock {
    /// Next ticket to be served
    next_ticket: AtomicUsize,
    /// Ticket currently being served
    serving: AtomicUsize,
}

impl TicketLock {
    /// Create a new ticket lock
    pub const fn new() -> Self {
        Self {
            next_ticket: AtomicUsize::new(0),
            serving: AtomicUsize::new(0),
        }
    }

    /// Acquire the lock
    pub fn lock(&self) -> TicketLockGuard {
        let ticket = self.next_ticket.fetch_add(1, Ordering::Acquire);
        while self.serving.load(Ordering::Acquire) != ticket {
            #[cfg(target_arch = "aarch64")]
            cortex_a::asm::yield();

            #[cfg(target_arch = "riscv64")]
            riscv::asm::pause();

            #[cfg(target_arch = "x86_64")]
            x86_64::instructions::pause();
        }
        TicketLockGuard { lock: self }
    }

    /// Try to acquire the lock
    pub fn try_lock(&self) -> Option<TicketLockGuard> {
        let ticket = self.next_ticket.load(Ordering::Acquire);
        if self.serving.load(Ordering::Acquire) == ticket {
            // Try to claim the ticket
            match self.next_ticket.compare_exchange(
                ticket,
                ticket + 1,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    // Double-check we're still being served
                    if self.serving.load(Ordering::Acquire) == ticket {
                        return Some(TicketLockGuard { lock: self });
                    }
                    // Lost the race, return the ticket
                    self.next_ticket.store(ticket, Ordering::Release);
                }
                Err(_) => {
                    // Someone else claimed the ticket
                }
            }
        }
        None
    }
}

unsafe impl Send for TicketLock {}
unsafe impl Sync for TicketLock {}

/// Guard for TicketLock
pub struct TicketLockGuard<'a> {
    lock: &'a TicketLock,
}

impl<'a> Drop for TicketLockGuard<'a> {
    fn drop(&mut self) {
        self.lock.serving.fetch_add(1, Ordering::Release);
    }
}