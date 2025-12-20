//! FIFO (First-In-First-Out) scheduling algorithm
//!
//! This module implements the FIFO scheduling algorithm,
//! which schedules threads in the order they become ready.

use crate::core::sched::{Thread, ThreadId, Priority, ThreadControlBlock};
use crate::core::sync::SpinLock;
use crate::utils::list::{List, ListNode};
use core::ptr::NonNull;

/// FIFO queue for a specific priority level
pub struct FifoQueue {
    /// Queue of threads
    queue: SpinLock<List>,
}

impl FifoQueue {
    /// Create a new FIFO queue
    pub const fn new() -> Self {
        Self {
            queue: SpinLock::new(List::new()),
        }
    }

    /// Add a thread to the queue (FIFO - add to back)
    pub fn enqueue(&self, tcb: &mut ThreadControlBlock) {
        let mut queue = self.queue.lock();
        queue.push_back(unsafe {
            NonNull::new_unchecked(&mut tcb.node as *mut ListNode)
        });
    }

    /// Remove a thread from the queue
    pub fn dequeue(&self, tcb: &mut ThreadControlBlock) -> bool {
        let mut queue = self.queue.lock();
        let node_ptr = unsafe {
            NonNull::new_unchecked(&mut tcb.node as *mut ListNode)
        };
        queue.remove(node_ptr)
    }

    /// Get the first thread (front of queue)
    pub fn peek(&self) -> Option<&ThreadControlBlock> {
        let queue = self.queue.lock();
        queue.front().map(|node| unsafe {
            &*(node.as_ptr() as *const ListNode as *const ThreadControlBlock)
        })
    }

    /// Remove and return the first thread
    pub fn dequeue_front(&self) -> Option<&mut ThreadControlBlock> {
        let mut queue = self.queue.lock();
        queue.pop_front().map(|node| unsafe {
            &mut *(node.as_ptr() as *mut ListNode as *mut ThreadControlBlock)
        })
    }

    /// Check if the queue is empty
    pub fn is_empty(&self) -> bool {
        self.queue.lock().is_empty()
    }

    /// Get the length of the queue
    pub fn len(&self) -> usize {
        self.queue.lock().len()
    }

    /// Clear the queue
    pub fn clear(&self) {
        let mut queue = self.queue.lock();
        while let Some(_) = queue.pop_front() {}
    }
}

/// FIFO scheduler implementation
pub struct FifoScheduler {
    /// FIFO queues for each priority level
    queues: [FifoQueue; 5], // 5 priority levels
    /// Current priority being serviced
    current_priority: SpinLock<usize>,
}

impl FifoScheduler {
    /// Create a new FIFO scheduler
    pub const fn new() -> Self {
        Self {
            queues: [
                FifoQueue::new(), // Idle
                FifoQueue::new(), // Low
                FifoQueue::new(), // Normal
                FifoQueue::new(), // High
                FifoQueue::new(), // RealTime
            ],
            current_priority: SpinLock::new(4), // Start with highest priority
        }
    }

    /// Add a thread to the appropriate FIFO queue
    pub fn add_thread(&self, tcb: &mut ThreadControlBlock) {
        let priority_index = tcb.priority as usize;
        if priority_index < 5 {
            self.queues[priority_index].enqueue(tcb);
        }
    }

    /// Remove a thread from its FIFO queue
    pub fn remove_thread(&self, tcb: &mut ThreadControlBlock) {
        let priority_index = tcb.priority as usize;
        if priority_index < 5 {
            self.queues[priority_index].dequeue(tcb);
        }
    }

    /// Schedule the next thread to run (FIFO: always pick highest priority)
    pub fn schedule(&self) -> Option<&ThreadControlBlock> {
        // Check from highest to lowest priority (FIFO always picks highest)
        for i in (0..5).rev() {
            if let Some(thread) = self.queues[i].peek() {
                return Some(thread);
            }
        }
        None
    }

    /// Pick and remove the next thread to run
    pub fn pick_next(&self) -> Option<&mut ThreadControlBlock> {
        // Check from highest to lowest priority
        for i in (0..5).rev() {
            if let Some(thread) = self.queues[i].dequeue_front() {
                return Some(thread);
            }
        }
        None
    }

    /// Re-queue a thread (FIFO: add to back of its priority queue)
    pub fn requeue(&self, tcb: &mut ThreadControlBlock) {
        self.add_thread(tcb);
    }

    /// Get queue statistics
    pub fn get_queue_stats(&self) -> [usize; 5] {
        [
            self.queues[0].len(),
            self.queues[1].len(),
            self.queues[2].len(),
            self.queues[3].len(),
            self.queues[4].len(),
        ]
    }

    /// Check if any threads are ready
    pub fn has_ready_threads(&self) -> bool {
        for queue in self.queues.iter() {
            if !queue.is_empty() {
                return true;
            }
        }
        false
    }

    /// Clear all queues
    pub fn clear_all(&self) {
        for queue in self.queues.iter() {
            queue.clear();
        }
    }

    /// Get total number of threads
    pub fn total_threads(&self) -> usize {
        self.get_queue_stats().iter().sum()
    }

    /// Get threads by priority level
    pub fn threads_by_priority(&self, priority: Priority) -> usize {
        let index = priority as usize;
        if index < 5 {
            self.queues[index].len()
        } else {
            0
        }
    }
}

/// FIFO Scheduler with fairness enhancement
///
/// This variant adds aging to prevent starvation of lower priority threads
pub struct FairFifoScheduler {
    /// Base FIFO scheduler
    base: FifoScheduler,
    /// Aging counter for each priority level
    aging_counters: SpinLock<[u32; 5]>,
    /// Aging threshold
    aging_threshold: u32,
}

impl FairFifoScheduler {
    /// Create a new fair FIFO scheduler
    pub const fn new(aging_threshold: u32) -> Self {
        Self {
            base: FifoScheduler::new(),
            aging_counters: SpinLock::new([0; 5]),
            aging_threshold,
        }
    }

    /// Get the aging threshold
    pub fn aging_threshold(&self) -> u32 {
        self.aging_threshold
    }

    /// Set the aging threshold
    pub fn set_aging_threshold(&mut self, threshold: u32) {
        self.aging_threshold = threshold;
    }

    /// Add a thread with aging consideration
    pub fn add_thread(&self, tcb: &mut ThreadControlBlock) {
        let priority = tcb.priority as usize;
        if priority < 5 {
            self.base.add_thread(tcb);

            // Reset aging counter for this priority
            let mut counters = self.aging_counters.lock();
            counters[priority] = 0;
        }
    }

    /// Remove a thread
    pub fn remove_thread(&self, tcb: &mut ThreadControlBlock) {
        self.base.remove_thread(tcb);
    }

    /// Schedule with aging to prevent starvation
    pub fn schedule(&self) -> Option<&ThreadControlBlock> {
        // First, update aging counters
        self.update_aging();

        // Check from highest to lowest priority
        for i in (0..5).rev() {
            // Check if we should boost lower priorities due to aging
            let should_boost = self.should_boost_priority(i);

            if should_boost || !self.base.queues[i].is_empty() {
                if let Some(thread) = self.base.queues[i].peek() {
                    return Some(thread);
                }
            }
        }
        None
    }

    /// Pick next thread with aging
    pub fn pick_next(&self) -> Option<&mut ThreadControlBlock> {
        // Update aging first
        self.update_aging();

        for i in (0..5).rev() {
            if self.should_boost_priority(i) || !self.base.queues[i].is_empty() {
                if let Some(thread) = self.base.queues[i].dequeue_front() {
                    // Reset aging for this priority
                    let mut counters = self.aging_counters.lock();
                    counters[i] = 0;
                    return Some(thread);
                }
            }
        }
        None
    }

    /// Re-queue a thread
    pub fn requeue(&self, tcb: &mut ThreadControlBlock) {
        self.add_thread(tcb);
    }

    /// Update aging counters
    fn update_aging(&self) {
        let mut counters = self.aging_counters.lock();

        // Increment counters for all priorities
        for i in 0..5 {
            if counters[i] < u32::MAX {
                counters[i] += 1;
            }
        }
    }

    /// Check if we should boost lower priorities
    fn should_boost_priority(&self, priority: usize) -> bool {
        let counters = self.aging_counters.lock();

        // Boost if a lower priority has aged more than the threshold
        for i in 0..priority {
            if counters[i] >= self.aging_threshold {
                // Also boost all priorities up to the aged one
                return true;
            }
        }
        false
    }

    /// Get aging statistics
    pub fn get_aging_stats(&self) -> [u32; 5] {
        *self.aging_counters.lock()
    }

    /// Reset aging counters
    pub fn reset_aging(&self) {
        let mut counters = self.aging_counters.lock();
        for counter in counters.iter_mut() {
            *counter = 0;
        }
    }

    /// Get the base FIFO scheduler
    pub fn base(&self) -> &FifoScheduler {
        &self.base
    }
}

/// FIFO Scheduler statistics
#[derive(Debug, Clone, Copy)]
pub struct FifoStats {
    /// Number of threads per priority level
    pub threads_per_priority: [usize; 5],
    /// Total number of threads
    pub total_threads: usize,
    /// Number of context switches
    pub context_switches: u64,
    /// Average wait time in queue (in ticks)
    pub avg_wait_time: f64,
}

/// FIFO Scheduler with tracking capabilities
pub struct TrackedFifoScheduler {
    /// Base scheduler
    scheduler: FifoScheduler,
    /// Statistics
    stats: SpinLock<FifoStats>,
    /// Thread wait times tracking
    wait_times: SpinLock<Vec<(ThreadId, u64)>>, // (thread_id, wait_start_time)
}

impl TrackedFifoScheduler {
    /// Create a new tracked FIFO scheduler
    pub const fn new() -> Self {
        Self {
            scheduler: FifoScheduler::new(),
            stats: SpinLock::new(FifoStats {
                threads_per_priority: [0; 5],
                total_threads: 0,
                context_switches: 0,
                avg_wait_time: 0.0,
            }),
            wait_times: SpinLock::new(Vec::new()),
        }
    }

    /// Add a thread and start tracking wait time
    pub fn add_thread(&self, tcb: &mut ThreadControlBlock) {
        let current_time = crate::utils::get_timestamp();
        let priority = tcb.priority as usize;

        if priority < 5 {
            {
                let mut stats = self.stats.lock();
                stats.threads_per_priority[priority] += 1;
                stats.total_threads += 1;
            }

            // Track wait time start
            {
                let mut wait_times = self.wait_times.lock();
                wait_times.push((tcb.id, current_time));
            }

            self.scheduler.add_thread(tcb);
        }
    }

    /// Remove a thread
    pub fn remove_thread(&self, tcb: &mut ThreadControlBlock) {
        let priority = tcb.priority as usize;
        if priority < 5 {
            self.scheduler.remove_thread(tcb);
            {
                let mut stats = self.stats.lock();
                stats.threads_per_priority[priority] = stats.threads_per_priority[priority].saturating_sub(1);
                stats.total_threads = stats.total_threads.saturating_sub(1);
            }
        }
    }

    /// Schedule next thread and update statistics
    pub fn schedule(&self) -> Option<&ThreadControlBlock> {
        let next_thread = self.scheduler.schedule();

        if let Some(thread) = next_thread {
            let current_time = crate::utils::get_timestamp();

            // Update wait time
            {
                let mut wait_times = self.wait_times.lock();
                if let Some(index) = wait_times.iter().position(|(tid, _)| *tid == thread.id) {
                    let wait_duration = current_time - wait_times[index].1;
                    let mut stats = self.stats.lock();

                    // Update average wait time (simple moving average)
                    let alpha = 0.1;
                    stats.avg_wait_time = stats.avg_wait_time * (1.0 - alpha) +
                                         (wait_duration as f64) * alpha;

                    wait_times.remove(index);
                }
            }

            let mut stats = self.stats.lock();
            stats.context_switches += 1;
        }

        next_thread
    }

    /// Pick next thread
    pub fn pick_next(&self) -> Option<&mut ThreadControlBlock> {
        self.scheduler.pick_next()
    }

    /// Re-queue a thread
    pub fn requeue(&self, tcb: &mut ThreadControlBlock) {
        self.add_thread(tcb);
    }

    /// Get scheduler statistics
    pub fn get_stats(&self) -> FifoStats {
        *self.stats.lock()
    }

    /// Get the base scheduler
    pub fn base(&self) -> &FifoScheduler {
        &self.scheduler
    }
}

/// Create a FIFO scheduler
pub fn create_fifo_scheduler() -> TrackedFifoScheduler {
    TrackedFifoScheduler::new()
}

/// Create a fair FIFO scheduler with aging
pub fn create_fair_fifo_scheduler(aging_threshold: u32) -> FairFifoScheduler {
    FairFifoScheduler::new(aging_threshold)
}