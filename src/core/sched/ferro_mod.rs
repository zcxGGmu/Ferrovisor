//! Scheduler module
//!
//! This module provides the core scheduling functionality for the hypervisor,
//! managing VCPU threads and orphan threads.

use crate::Result;

pub mod scheduler;
pub mod rr;
pub mod fifo;

/// Thread ID type
pub type ThreadId = u64;

/// Thread priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Idle priority (lowest)
    Idle = 0,
    /// Low priority
    Low = 1,
    /// Normal priority
    Normal = 2,
    /// High priority
    High = 3,
    /// Real-time priority (highest)
    RealTime = 4,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

/// Thread states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    /// Thread is ready to run
    Ready,
    /// Thread is currently running
    Running,
    /// Thread is blocked (waiting for something)
    Blocked,
    /// Thread has finished execution
    Terminated,
}

/// Thread information
#[derive(Debug)]
pub struct Thread {
    /// Unique thread ID
    id: ThreadId,
    /// Thread name
    name: &'static str,
    /// Thread state
    state: ThreadState,
    /// Thread priority
    priority: Priority,
    /// CPU affinity (which CPUs this thread can run on)
    cpu_affinity: u64,
    /// Time slice remaining
    time_slice: u32,
    /// Total CPU time consumed
    cpu_time: u64,
    /// Context-specific data
    context_data: *mut u8,
}

/// Thread handle
pub struct ThreadHandle {
    thread: *mut Thread,
}

impl Thread {
    /// Create a new thread
    pub fn new(
        id: ThreadId,
        name: &'static str,
        priority: Priority,
        context_data: *mut u8,
    ) -> Self {
        Self {
            id,
            name,
            state: ThreadState::Ready,
            priority,
            cpu_affinity: u64::MAX, // Run on any CPU by default
            time_slice: 10, // Default time slice
            cpu_time: 0,
            context_data,
        }
    }

    /// Get the thread ID
    pub fn id(&self) -> ThreadId {
        self.id
    }

    /// Get the thread name
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Get the thread state
    pub fn state(&self) -> ThreadState {
        self.state
    }

    /// Get the thread priority
    pub fn priority(&self) -> Priority {
        self.priority
    }

    /// Set the thread priority
    pub fn set_priority(&mut self, priority: Priority) {
        self.priority = priority;
    }

    /// Get the CPU affinity mask
    pub fn cpu_affinity(&self) -> u64 {
        self.cpu_affinity
    }

    /// Set the CPU affinity mask
    pub fn set_cpu_affinity(&mut self, affinity: u64) {
        self.cpu_affinity = affinity;
    }

    /// Get the context-specific data
    pub fn context_data(&self) -> *mut u8 {
        self.context_data
    }

    /// Get the remaining time slice
    pub fn time_slice(&self) -> u32 {
        self.time_slice
    }

    /// Reset the time slice
    pub fn reset_time_slice(&mut self) {
        self.time_slice = 10; // Reset to default
    }

    /// Decrement time slice
    pub fn dec_time_slice(&mut self) -> bool {
        if self.time_slice > 0 {
            self.time_slice -= 1;
            self.time_slice > 0
        } else {
            false
        }
    }
}

unsafe impl Send for Thread {}
unsafe impl Sync for Thread {}

/// Initialize the scheduler
pub fn init() -> Result<()> {
    scheduler::init()
}

/// Get the current thread ID
pub fn current_thread_id() -> Option<ThreadId> {
    scheduler::current_thread()
}

/// Schedule the next thread to run
pub fn schedule(cpu_id: usize) -> Result<Option<ThreadId>, crate::Error> {
    scheduler::schedule(cpu_id)
}

/// Block the current thread
pub fn block_current(cpu_id: usize) -> Result<(), crate::Error> {
    scheduler::block_current(cpu_id)
}

/// Unblock a thread
pub fn unblock_thread(tid: ThreadId) -> Result<(), crate::Error> {
    scheduler::unblock_thread(tid)
}

/// Yield the current CPU time slice
pub fn yield_current() -> Result<(), crate::Error> {
    let cpu_id = crate::core::cpu_id();
    scheduler::yield_current(cpu_id)
}

/// Create a new thread
pub fn create_thread(
    vm_id: Option<crate::core::vmm::VmId>,
    vcpu_id: Option<crate::core::vmm::VcpuId>,
    priority: Priority,
) -> Result<ThreadId, crate::Error> {
    scheduler::create_thread(vm_id, vcpu_id, priority)
}

/// Destroy a thread
pub fn destroy_thread(tid: ThreadId) -> Result<(), crate::Error> {
    scheduler::destroy_thread(tid)
}

/// Handle scheduler tick
pub fn handle_tick() -> Result<(), crate::Error> {
    scheduler::handle_tick()
}

/// Get scheduler statistics
pub fn get_stats() -> scheduler::SchedulerStats {
    scheduler::get_stats()
}

use core::ptr::NonNull;