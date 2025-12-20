//! Semaphore implementation
//!
//! Provides counting semaphores for controlling access to resources.

use core::ptr::NonNull;
use crate::core::sync::spinlock::SpinLock;
use crate::core::sched;
use crate::utils::list::{List, ListNode};

/// A counting semaphore
pub struct Semaphore {
    /// Inner state protected by a spinlock
    inner: SpinLock<SemaphoreInner>,
}

/// Inner state of the semaphore
struct SemaphoreInner {
    /// Current count of the semaphore
    count: isize,
    /// Maximum count (0 means no limit)
    max_count: isize,
    /// List of waiting threads
    waiters: List,
}

/// Node representing a waiting thread
struct WaiterNode {
    /// List node for the waiters list
    node: ListNode,
    /// Pointer to the thread that is waiting
    thread: NonNull<sched::Thread>,
}

impl Semaphore {
    /// Create a new semaphore with the specified initial count
    pub fn new(initial_count: isize) -> Self {
        Self {
            inner: SpinLock::new(SemaphoreInner {
                count: initial_count,
                max_count: 0, // No limit by default
                waiters: List::new(),
            }),
        }
    }

    /// Create a new semaphore with initial and maximum count
    pub fn with_max(initial_count: isize, max_count: isize) -> Self {
        Self {
            inner: SpinLock::new(SemaphoreInner {
                count: initial_count,
                max_count,
                waiters: List::new(),
            }),
        }
    }

    /// Acquire the semaphore (decrement count)
    ///
    /// Blocks if the count would go negative
    pub fn acquire(&self) {
        let mut inner = self.inner.lock();

        if inner.count > 0 {
            // Semaphore available, decrement count
            inner.count -= 1;
            return;
        }

        // Need to wait
        let current_thread = sched::current_thread();
        let waiter_node = WaiterNode {
            node: ListNode::new(),
            thread: NonNull::from(current_thread),
        };

        // Add to wait queue
        inner.waiters.push_back(unsafe { NonNull::new_unchecked(
            &waiter_node.node as *const _ as *mut _
        ) });

        // Release lock before blocking
        drop(inner);

        // Block the current thread
        sched::block_current();

        // When we wake up, we have acquired the semaphore
    }

    /// Try to acquire the semaphore without blocking
    ///
    /// Returns true if acquired, false if not available
    pub fn try_acquire(&self) -> bool {
        let mut inner = self.inner.lock();
        if inner.count > 0 {
            inner.count -= 1;
            true
        } else {
            false
        }
    }

    /// Release the semaphore (increment count)
    ///
    /// Wakes up a waiting thread if any
    pub fn release(&self) {
        let mut inner = self.inner.lock();

        // Check if there's a waiting thread
        if let Some(waiter_ptr) = inner.waiters.pop_front() {
            let waiter_node = unsafe {
                &*(waiter_ptr.as_ptr() as *const WaiterNode)
            };
            // Wake up the waiting thread
            sched::unblock_thread(waiter_node.thread);
            // Count stays the same (transfer to the waiting thread)
        } else {
            // No waiting threads, increment count
            inner.count += 1;
            // Check max limit
            if inner.max_count > 0 && inner.count > inner.max_count {
                // Exceeded maximum, this shouldn't happen in correct usage
                inner.count = inner.max_count;
            }
        }
    }

    /// Get the current count
    pub fn count(&self) -> isize {
        self.inner.lock().count
    }

    /// Check if the semaphore is available (count > 0)
    pub fn is_available(&self) -> bool {
        self.inner.lock().count > 0
    }
}

unsafe impl Send for Semaphore {}
unsafe impl Sync for Semaphore {}

/// A binary semaphore (mutex)
pub type BinarySemaphore = Semaphore;

impl BinarySemaphore {
    /// Create a new binary semaphore (initially unlocked)
    pub fn new_binary() -> Self {
        Self::new(1)
    }

    /// Create a new binary semaphore that starts locked
    pub fn new_locked() -> Self {
        Self::new(0)
    }

    /// Lock the binary semaphore
    pub fn lock(&self) {
        self.acquire();
    }

    /// Try to lock the binary semaphore
    pub fn try_lock(&self) -> bool {
        self.try_acquire()
    }

    /// Unlock the binary semaphore
    pub fn unlock(&self) {
        self.release();
    }
}

/// A simple event flag
pub struct Event {
    /// Whether the event is set
    set: SpinLock<bool>,
    /// List of waiting threads
    waiters: SpinLock<List>,
}

impl Event {
    /// Create a new event (initially not set)
    pub fn new() -> Self {
        Self {
            set: SpinLock::new(false),
            waiters: SpinLock::new(List::new()),
        }
    }

    /// Create a new event that starts set
    pub fn new_set() -> Self {
        Self {
            set: SpinLock::new(true),
            waiters: SpinLock::new(List::new()),
        }
    }

    /// Wait for the event to be set
    pub fn wait(&self) {
        let mut set = self.set.lock();
        if *set {
            return; // Event is already set
        }

        // Need to wait
        drop(set);

        let current_thread = sched::current_thread();
        let waiter_node = WaiterNode {
            node: ListNode::new(),
            thread: NonNull::from(current_thread),
        };

        // Add to wait queue
        {
            let mut waiters = self.waiters.lock();
            waiters.push_back(unsafe { NonNull::new_unchecked(
                &waiter_node.node as *const _ as *mut _
            ) });
        }

        // Block the current thread
        sched::block_current();
    }

    /// Try to wait for the event without blocking
    ///
    /// Returns true if the event is set, false otherwise
    pub fn try_wait(&self) -> bool {
        *self.set.lock()
    }

    /// Set the event
    pub fn set(&self) {
        // Mark as set
        *self.set.lock() = true;

        // Wake up all waiting threads
        let mut waiters = self.waiters.lock();
        while let Some(waiter_ptr) = waiters.pop_front() {
            let waiter_node = unsafe {
                &*(waiter_ptr.as_ptr() as *const WaiterNode)
            };
            sched::unblock_thread(waiter_node.thread);
        }
    }

    /// Clear the event
    pub fn clear(&self) {
        *self.set.lock() = false;
    }

    /// Check if the event is set
    pub fn is_set(&self) -> bool {
        *self.set.lock()
    }
}

unsafe impl Send for Event {}
unsafe impl Sync for Event {}

/// A barrier that blocks a specified number of threads
pub struct Barrier {
    /// Number of threads required to pass the barrier
    required: usize,
    /// Current number of waiting threads
    waiting: SpinLock<usize>,
    /// Event used to signal when all threads have arrived
    event: Event,
}

impl Barrier {
    /// Create a new barrier for the specified number of threads
    pub fn new(required: usize) -> Self {
        Self {
            required,
            waiting: SpinLock::new(0),
            event: Event::new(),
        }
    }

    /// Wait at the barrier
    ///
    /// Returns true for exactly one thread (the one that completes the barrier)
    pub fn wait(&self) -> bool {
        let mut waiting = self.waiting.lock();
        *waiting += 1;

        if *waiting == self.required {
            // All threads have arrived, reset and wake everyone
            *waiting = 0;
            self.event.set();
            true
        } else {
            // Need to wait for other threads
            drop(waiting);
            self.event.wait();
            false
        }
    }
}

unsafe impl Send for Barrier {}
unsafe impl Sync for Barrier {}