//! Mutex implementation for the hypervisor
//!
//! Provides a mutex that can be used to protect shared data.
//! Unlike spinlocks, mutexes can put the CPU to sleep while waiting.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use crate::core::sync::spinlock::SpinLock;
use crate::core::sched;
use crate::utils::list::{List, ListNode};

/// A mutex that can block waiting threads
pub struct Mutex<T> {
    /// The inner lock protecting the mutex state
    inner: SpinLock<MutexInner<T>>,
}

/// Inner state of the mutex
struct MutexInner<T> {
    /// The data protected by the mutex
    data: Option<T>,
    /// List of threads waiting for the mutex
    waiters: List,
    /// Flag indicating if the mutex is locked
    locked: bool,
}

/// Node representing a waiting thread
struct WaiterNode {
    /// List node for the waiters list
    node: ListNode,
    /// Pointer to the thread that is waiting
    thread: NonNull<sched::Thread>,
}

impl<T> Mutex<T> {
    /// Create a new mutex
    pub fn new(data: T) -> Self {
        Self {
            inner: SpinLock::new(MutexInner {
                data: Some(data),
                waiters: List::new(),
                locked: false,
            }),
        }
    }

    /// Try to acquire the mutex without blocking
    pub fn try_lock(&self) -> Option<MutexGuard<T>> {
        let mut inner = self.inner.lock();
        if !inner.locked {
            inner.locked = true;
            Some(MutexGuard {
                mutex: self,
                _guard: PhantomData,
            })
        } else {
            None
        }
    }

    /// Acquire the mutex, blocking if necessary
    pub fn lock(&self) -> MutexGuard<T> {
        // Try to acquire immediately
        if let Some(guard) = self.try_lock() {
            return guard;
        }

        // Need to wait
        let current_thread = sched::current_thread();
        let waiter_node = WaiterNode {
            node: ListNode::new(),
            thread: NonNull::from(current_thread),
        };

        // Add ourselves to the wait queue
        {
            let mut inner = self.inner.lock();
            inner.waiters.push_back(unsafe { NonNull::new_unchecked(
                &waiter_node.node as *const _ as *mut _
            ) });
        }

        // Block the current thread
        sched::block_current();

        // When we wake up, we should have the lock
        MutexGuard {
            mutex: self,
            _guard: PhantomData,
        }
    }

    /// Force unlock the mutex (DANGEROUS!)
    ///
    /// # Safety
    /// This breaks mutual exclusion guarantees. Only use during panic
    /// or other exceptional circumstances.
    pub unsafe fn force_unlock(&self) {
        let mut inner = self.inner.lock();
        inner.locked = false;

        // Wake up the first waiter if any
        if let Some(waiter_ptr) = inner.waiters.pop_front() {
            let waiter_node = unsafe {
                &*(waiter_ptr.as_ptr() as *const WaiterNode)
            };
            sched::unblock_thread(waiter_node.thread);
        }
    }
}

/// Guard that provides access to the data protected by a Mutex
pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
    _guard: core::marker::PhantomData<(&'a mut T,)>,
}

impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        let inner = self.mutex.inner.lock();
        // Safety: We know the data exists because we hold the lock
        unsafe { inner.data.as_ref().unwrap_unchecked() }
    }
}

impl<'a, T> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let mut inner = self.mutex.inner.lock();
        // Safety: We know the data exists because we hold the lock
        unsafe { inner.data.as_mut().unwrap_unchecked() }
    }
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        let mut inner = self.mutex.inner.lock();

        // Wake up the first waiter if any
        if let Some(waiter_ptr) = inner.waiters.pop_front() {
            let waiter_node = unsafe {
                &*(waiter_ptr.as_ptr() as *const WaiterNode)
            };
            sched::unblock_thread(waiter_node.thread);
        } else {
            // No waiters, unlock the mutex
            inner.locked = false;
        }
    }
}

unsafe impl<T: Send> Send for Mutex<T> {}
unsafe impl<T: Send> Sync for Mutex<T> {}

/// A reentrant mutex (recursive mutex)
pub struct ReentrantMutex<T> {
    /// The inner mutex
    inner: Mutex<ReentrantMutexInner<T>>,
}

/// Inner state of a reentrant mutex
struct ReentrantMutexInner<T> {
    /// The data protected by the mutex
    data: T,
    /// The thread that currently owns the mutex
    owner: Option<NonNull<sched::Thread>>,
    /// The number of times the mutex has been locked by the owner
    count: usize,
}

impl<T> ReentrantMutex<T> {
    /// Create a new reentrant mutex
    pub fn new(data: T) -> Self {
        Self {
            inner: Mutex::new(ReentrantMutexInner {
                data,
                owner: None,
                count: 0,
            }),
        }
    }

    /// Acquire the mutex
    pub fn lock(&self) -> ReentrantMutexGuard<T> {
        let current_thread = sched::current_thread();
        let mut inner = self.inner.lock();

        if let Some(owner) = inner.owner {
            if Some(NonNull::from(current_thread)) == owner {
                // Already owned by this thread, increment count
                inner.count += 1;
            } else {
                // Owned by another thread, need to wait
                // Release the lock before blocking
                drop(inner);
                // TODO: Implement blocking wait
                panic!("ReentrantMutex blocking not implemented");
            }
        } else {
            // Not owned, take ownership
            inner.owner = Some(NonNull::from(current_thread));
            inner.count = 1;
        }

        ReentrantMutexGuard {
            mutex: self,
            _guard: PhantomData,
        }
    }
}

/// Guard for ReentrantMutex
pub struct ReentrantMutexGuard<'a, T> {
    mutex: &'a ReentrantMutex<T>,
    _guard: core::marker::PhantomData<(&'a mut T,)>,
}

impl<'a, T> Deref for ReentrantMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        let inner = self.mutex.inner.lock();
        &inner.data
    }
}

impl<'a, T> DerefMut for ReentrantMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let mut inner = self.mutex.inner.lock();
        &mut inner.data
    }
}

impl<'a, T> Drop for ReentrantMutexGuard<'a, T> {
    fn drop(&mut self) {
        let mut inner = self.mutex.inner.lock();
        inner.count -= 1;
        if inner.count == 0 {
            // Release ownership
            inner.owner = None;
            // TODO: Wake up a waiting thread if any
        }
    }
}

unsafe impl<T: Send> Send for ReentrantMutex<T> {}
unsafe impl<T: Send> Sync for ReentrantMutex<T> {}

use core::marker::PhantomData;