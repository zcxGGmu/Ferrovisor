//! Main scheduler implementation
//!
//! This module provides the core scheduler implementation for the hypervisor,
//! managing both VCPU threads and system threads.

use crate::{Result, Error};
use crate::core::sched::{Thread, ThreadId, Priority, ThreadState};
use crate::core::vmm::{VmId, VcpuId};
use crate::core::sync::SpinLock;
use crate::utils::list::{List, ListNode};
use crate::utils::list::impl_list_node;
use crate::utils::bitmap::Bitmap;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Maximum number of threads
pub const MAX_THREADS: usize = 512;

/// Scheduler statistics
#[derive(Debug, Clone, Copy)]
pub struct SchedulerStats {
    /// Total number of threads
    pub total_threads: usize,
    /// Number of running threads
    pub running_threads: usize,
    /// Number of ready threads
    pub ready_threads: usize,
    /// Number of blocked threads
    pub blocked_threads: usize,
    /// Total context switches
    pub context_switches: u64,
    /// Scheduler runs
    pub scheduler_runs: u64,
}

/// Thread control block
#[derive(Debug)]
pub struct ThreadControlBlock {
    /// Thread ID
    pub id: ThreadId,
    /// VM ID (if this is a VCPU thread)
    pub vm_id: Option<VmId>,
    /// VCPU ID (if this is a VCPU thread)
    pub vcpu_id: Option<VcpuId>,
    /// Thread state
    pub state: ThreadState,
    /// Thread priority
    pub priority: Priority,
    /// Time slice remaining
    pub time_slice: u32,
    /// Total CPU time used
    pub cpu_time: u64,
    /// Last run time
    pub last_run_time: u64,
    /// CPU affinity mask
    pub cpu_affinity: u64,
    /// List node for scheduler queues
    pub node: ListNode,
}

impl ThreadControlBlock {
    /// Create a new thread control block
    pub fn new(id: ThreadId, priority: Priority) -> Self {
        Self {
            id,
            vm_id: None,
            vcpu_id: None,
            state: ThreadState::Ready,
            priority,
            time_slice: 10, // Default 10ms time slice
            cpu_time: 0,
            last_run_time: 0,
            cpu_affinity: u64::MAX, // Run on any CPU
            node: ListNode::new(),
        }
    }

    /// Create a VCPU thread control block
    pub fn new_vcpu(id: ThreadId, vm_id: VmId, vcpu_id: VcpuId, priority: Priority) -> Self {
        let mut tcb = Self::new(id, priority);
        tcb.vm_id = Some(vm_id);
        tcb.vcpu_id = Some(vcpu_id);
        tcb
    }

    /// Check if this is a VCPU thread
    pub fn is_vcpu(&self) -> bool {
        self.vcpu_id.is_some()
    }

    /// Reset time slice
    pub fn reset_time_slice(&mut self) {
        self.time_slice = match self.priority {
            Priority::Idle => 5,
            Priority::Low => 8,
            Priority::Normal => 10,
            Priority::High => 15,
            Priority::RealTime => 20,
        };
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

// Implement ListNode for ThreadControlBlock
impl_list_node!(ThreadControlBlock, node);

/// Ready queue for each priority level
pub struct ReadyQueue {
    /// Array of queues for each priority level
    queues: [List; 5], // 5 priority levels
    /// Bitmap to quickly find non-empty queues
    bitmap: Bitmap,
}

impl ReadyQueue {
    /// Create a new ready queue
    pub fn new() -> Self {
        Self {
            queues: [
                List::new(), // Idle
                List::new(), // Low
                List::new(), // Normal
                List::new(), // High
                List::new(), // RealTime
            ],
            bitmap: unsafe {
                Bitmap::new(core::ptr::null_mut(), 5)
            },
        }
    }

    /// Add a thread to the appropriate queue
    pub fn enqueue(&mut self, tcb: &mut ThreadControlBlock) {
        let priority_index = tcb.priority as usize;
        if priority_index >= 5 {
            return;
        }

        self.queues[priority_index].push_back(unsafe {
            NonNull::new_unchecked(&mut tcb.node as *mut ListNode)
        });
        self.bitmap.set_bit(priority_index);
    }

    /// Remove a thread from its queue
    pub fn dequeue(&mut self, tcb: &mut ThreadControlBlock) -> bool {
        let priority_index = tcb.priority as usize;
        if priority_index >= 5 {
            return false;
        }

        let list = &mut self.queues[priority_index];
        let node_ptr = unsafe {
            NonNull::new_unchecked(&mut tcb.node as *mut ListNode)
        };

        if list.remove(node_ptr) {
            if list.is_empty() {
                self.bitmap.clear_bit(priority_index);
            }
            true
        } else {
            false
        }
    }

    /// Get the highest priority thread
    pub fn peek(&mut self) -> Option<&mut ThreadControlBlock> {
        if let Some(index) = self.bitmap.find_first_set() {
            let list = &mut self.queues[index];
            if let Some(node_ptr) = list.front() {
                unsafe {
                    let tcb_ptr = node_ptr as *const ListNode as *const u8
                        as *const ThreadControlBlock;
                    Some(&mut *(tcb_ptr as *mut ThreadControlBlock))
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Remove and return the highest priority thread
    pub fn dequeue_highest(&mut self) -> Option<&mut ThreadControlBlock> {
        if let Some(index) = self.bitmap.find_first_set() {
            let list = &mut self.queues[index];
            if let Some(node_ptr) = list.pop_front() {
                if list.is_empty() {
                    self.bitmap.clear_bit(index);
                }

                unsafe {
                    let tcb_ptr = node_ptr as *const ListNode as *const u8
                        as *const ThreadControlBlock;
                    Some(&mut *(tcb_ptr as *mut ThreadControlBlock))
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Check if the queue is empty
    pub fn is_empty(&self) -> bool {
        self.bitmap.count_zeros() == 5
    }
}

/// Main scheduler
pub struct Scheduler {
    /// Thread control blocks
    threads: SpinLock<[Option<NonNull<ThreadControlBlock>>; MAX_THREADS]>,
    /// Thread ID allocation bitmap
    thread_id_bitmap: SpinLock<Bitmap>,
    /// Ready queues
    ready_queue: SpinLock<ReadyQueue>,
    /// Current running thread per CPU
    current_thread: SpinLock<[Option<ThreadId>; 64]>, // Max 64 CPUs
    /// Idle threads per CPU
    idle_threads: SpinLock<[ThreadId; 64]>,
    /// Statistics
    stats: SpinLock<SchedulerStats>,
    /// Scheduler tick counter
    tick_counter: AtomicUsize,
}

impl Scheduler {
    /// Create a new scheduler
    pub const fn new() -> Self {
        Self {
            threads: SpinLock::new([None; MAX_THREADS]),
            thread_id_bitmap: SpinLock::new(unsafe {
                Bitmap::new(core::ptr::null_mut(), MAX_THREADS)
            }),
            ready_queue: SpinLock::new(ReadyQueue::new()),
            current_thread: SpinLock::new([None; 64]),
            idle_threads: SpinLock::new([0; 64]),
            stats: SpinLock::new(SchedulerStats {
                total_threads: 0,
                running_threads: 0,
                ready_threads: 0,
                blocked_threads: 0,
                context_switches: 0,
                scheduler_runs: 0,
            }),
            tick_counter: AtomicUsize::new(0),
        }
    }

    /// Initialize the scheduler
    pub fn init(&self) -> Result<()> {
        crate::info!("Initializing scheduler");

        // Create idle threads for each CPU
        for cpu_id in 0..64 {
            let idle_tid = self.create_thread(None, None, Priority::Idle)?;
            let mut idle_threads = self.idle_threads.lock();
            idle_threads[cpu_id] = idle_tid;

            // Set the idle thread state to running
            if let Some(tcb) = self.get_thread(idle_tid) {
                unsafe {
                    let tcb_mut = tcb.as_mut();
                    tcb_mut.state = ThreadState::Running;
                }
            }
        }

        crate::info!("Scheduler initialized with {} idle threads", 64);
        Ok(())
    }

    /// Create a new thread
    pub fn create_thread(
        &self,
        vm_id: Option<VmId>,
        vcpu_id: Option<VcpuId>,
        priority: Priority,
    ) -> Result<ThreadId> {
        // Allocate thread ID
        let tid = {
            let mut bitmap = self.thread_id_bitmap.lock();
            if let Some(index) = bitmap.find_and_set() {
                index as ThreadId
            } else {
                return Err(Error::ResourceUnavailable);
            }
        };

        // Create thread control block
        let tcb = if let (Some(vm), Some(vcpu)) = (vm_id, vcpu_id) {
            ThreadControlBlock::new_vcpu(tid, vm, vcpu, priority)
        } else {
            ThreadControlBlock::new(tid, priority)
        };

        // Store thread
        {
            let mut threads = self.threads.lock();
            threads[tid as usize] = NonNull::new(Box::into_raw(Box::new(tcb)) as *mut ThreadControlBlock);
        }

        // Add to ready queue
        if let Some(tcb) = self.get_thread(tid) {
            unsafe {
                let tcb_mut = tcb.as_mut();
                tcb_mut.reset_time_slice();
                tcb_mut.state = ThreadState::Ready;

                let mut ready_queue = self.ready_queue.lock();
                ready_queue.enqueue(tcb_mut);
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.lock();
            stats.total_threads += 1;
            stats.ready_threads += 1;
        }

        Ok(tid)
    }

    /// Destroy a thread
    pub fn destroy_thread(&self, tid: ThreadId) -> Result<()> {
        // Remove from ready queue
        if let Some(tcb) = self.get_thread(tid) {
            unsafe {
                let tcb_mut = tcb.as_mut();

                if tcb_mut.state == ThreadState::Ready {
                    let mut ready_queue = self.ready_queue.lock();
                    ready_queue.dequeue(tcb_mut);
                }
            }
        }

        // Free thread control block
        {
            let mut threads = self.threads.lock();
            if let Some(tcb_ptr) = threads[tid as usize] {
                let _ = unsafe { Box::from_raw(tcb_ptr.as_ptr()) };
                threads[tid as usize] = None;
            }
        }

        // Free thread ID
        {
            let mut bitmap = self.thread_id_bitmap.lock();
            bitmap.clear_bit(tid as usize);
        }

        // Update statistics
        {
            let mut stats = self.stats.lock();
            stats.total_threads = stats.total_threads.saturating_sub(1);
        }

        Ok(())
    }

    /// Get a thread by ID
    pub fn get_thread(&self, tid: ThreadId) -> Option<NonNull<ThreadControlBlock>> {
        let threads = self.threads.lock();
        threads[tid as usize]
    }

    /// Schedule next thread to run on current CPU
    pub fn schedule(&self, cpu_id: usize) -> Result<Option<ThreadId>> {
        let current_time = crate::utils::get_timestamp();

        // Get current thread
        let current_tid = {
            let current_threads = self.current_thread.lock();
            current_threads.get(cpu_id).copied()
        };

        // Preempt current thread if it's still running
        if let Some(tid) = current_tid {
            if let Some(tcb) = self.get_thread(tid) {
                unsafe {
                    let tcb_mut = tcb.as_mut();

                    if tcb_mut.state == ThreadState::Running {
                        // Update CPU time
                        tcb_mut.cpu_time += current_time - tcb_mut.last_run_time;

                        // Check time slice
                        if !tcb_mut.dec_time_slice() {
                            // Time slice expired
                            tcb_mut.state = ThreadState::Ready;
                            tcb_mut.reset_time_slice();

                            // Add back to ready queue
                            let mut ready_queue = self.ready_queue.lock();
                            ready_queue.enqueue(tcb_mut);
                        } else {
                            // Still has time slice, continue running
                            tcb_mut.last_run_time = current_time;
                            return Ok(Some(tid));
                        }
                    }
                }
            }
        }

        // Get next thread from ready queue
        let next_tid = {
            let mut ready_queue = self.ready_queue.lock();
            if let Some(tcb) = ready_queue.dequeue_highest() {
                unsafe {
                    let tcb_mut = tcb.as_mut();
                    tcb_mut.state = ThreadState::Running;
                    tcb_mut.last_run_time = current_time;
                    Some(tcb_mut.id)
                }
            } else {
                // No ready threads, use idle thread
                let idle_threads = self.idle_threads.lock();
                idle_threads.get(cpu_id).copied()
            }
        };

        // Update current thread
        {
            let mut current_threads = self.current_thread.lock();
            current_threads[cpu_id] = next_tid;
        }

        // Update statistics
        {
            let mut stats = self.stats.lock();
            stats.scheduler_runs += 1;
            if current_tid != next_tid {
                stats.context_switches += 1;
            }
        }

        Ok(next_tid)
    }

    /// Block the current thread
    pub fn block_current(&self, cpu_id: usize) -> Result<()> {
        let current_tid = {
            let current_threads = self.current_thread.lock();
            current_threads.get(cpu_id).copied()
        };

        if let Some(tid) = current_tid {
            if let Some(tcb) = self.get_thread(tid) {
                unsafe {
                    let tcb_mut = tcb.as_mut();
                    tcb_mut.state = ThreadState::Blocked;
                }
            }
        }

        // Force reschedule
        self.schedule(cpu_id)?;

        Ok(())
    }

    /// Unblock a thread
    pub fn unblock_thread(&self, tid: ThreadId) -> Result<()> {
        if let Some(tcb) = self.get_thread(tid) {
            unsafe {
                let tcb_mut = tcb.as_mut();

                if tcb_mut.state == ThreadState::Blocked {
                    tcb_mut.state = ThreadState::Ready;
                    tcb_mut.reset_time_slice();

                    // Add to ready queue
                    let mut ready_queue = self.ready_queue.lock();
                    ready_queue.enqueue(tcb_mut);

                    // Update statistics
                    let mut stats = self.stats.lock();
                    stats.ready_threads += 1;
                    stats.blocked_threads = stats.blocked_threads.saturating_sub(1);
                }
            }
        }

        Ok(())
    }

    /// Handle timer tick
    pub fn handle_tick(&self) -> Result<()> {
        let tick = self.tick_counter.fetch_add(1, Ordering::Relaxed);

        // Check each CPU's current thread
        for cpu_id in 0..64 {
            let current_tid = {
                let current_threads = self.current_thread.lock();
                current_threads.get(cpu_id).copied()
            };

            if let Some(tid) = current_tid {
                if let Some(tcb) = self.get_thread(tid) {
                    unsafe {
                        let tcb_mut = tcb.as_mut();

                        if tcb_mut.state == ThreadState::Running {
                            // Decrement time slice
                            if !tcb_mut.dec_time_slice() {
                                // Time slice expired, trigger reschedule
                                tcb_mut.state = ThreadState::Ready;
                                tcb_mut.reset_time_slice();

                                let mut ready_queue = self.ready_queue.lock();
                                ready_queue.enqueue(tcb_mut);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Get scheduler statistics
    pub fn get_stats(&self) -> SchedulerStats {
        *self.stats.lock()
    }

    /// Get current thread on a CPU
    pub fn get_current_thread(&self, cpu_id: usize) -> Option<ThreadId> {
        let current_threads = self.current_thread.lock();
        current_threads.get(cpu_id).copied()
    }

    /// Yield the current CPU
    pub fn yield_current(&self, cpu_id: usize) -> Result<()> {
        let current_tid = {
            let current_threads = self.current_thread.lock();
            current_threads.get(cpu_id).copied()
        };

        if let Some(tid) = current_tid {
            if let Some(tcb) = self.get_thread(tid) {
                unsafe {
                    let tcb_mut = tcb.as_mut();

                    if tcb_mut.state == ThreadState::Running {
                        tcb_mut.state = ThreadState::Ready;
                        tcb_mut.reset_time_slice();

                        // Add to end of ready queue
                        let mut ready_queue = self.ready_queue.lock();
                        ready_queue.enqueue(tcb_mut);
                    }
                }
            }
        }

        // Force reschedule
        self.schedule(cpu_id)
    }
}

/// Global scheduler instance
static mut SCHEDULER: Option<Scheduler> = None;
static SCHEDULER_INIT: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Initialize the global scheduler
pub fn init() -> Result<()> {
    unsafe {
        if SCHEDULER.is_none() {
            SCHEDULER = Some(Scheduler::new());
            SCHEDULER_INIT.store(true, core::sync::atomic::Ordering::Release);

            // Initialize the scheduler
            SCHEDULER.as_ref().unwrap().init()?;
        }
    }
    Ok(())
}

/// Get the global scheduler
pub fn get() -> &'static Scheduler {
    unsafe {
        SCHEDULER.as_ref().unwrap()
    }
}

/// Create a new thread
pub fn create_thread(vm_id: Option<VmId>, vcpu_id: Option<VcpuId>, priority: Priority) -> Result<ThreadId> {
    get().create_thread(vm_id, vcpu_id, priority)
}

/// Destroy a thread
pub fn destroy_thread(tid: ThreadId) -> Result<()> {
    get().destroy_thread(tid)
}

/// Schedule next thread
pub fn schedule(cpu_id: usize) -> Result<Option<ThreadId>> {
    get().schedule(cpu_id)
}

/// Block current thread
pub fn block_current(cpu_id: usize) -> Result<()> {
    get().block_current(cpu_id)
}

/// Unblock a thread
pub fn unblock_thread(tid: ThreadId) -> Result<()> {
    get().unblock_thread(tid)
}

/// Handle timer tick
pub fn handle_tick() -> Result<()> {
    get().handle_tick()
}

/// Get current thread
pub fn current_thread() -> Option<ThreadId> {
    // Get current CPU ID
    let cpu_id = crate::core::cpu_id();
    get().get_current_thread(cpu_id)
}

/// Yield current thread
pub fn yield_current() -> Result<()> {
    let cpu_id = crate::core::cpu_id();
    get().yield_current(cpu_id)
}

/// Get scheduler statistics
pub fn get_stats() -> SchedulerStats {
    get().get_stats()
}