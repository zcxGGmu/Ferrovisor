//! Round-Robin (RR) scheduling algorithm
//!
//! This module implements the Round-Robin scheduling algorithm,
//! which gives each thread an equal time slice in a circular order.

use crate::core::sched::{Thread, ThreadId, Priority, ThreadControlBlock};
use crate::core::sync::SpinLock;
use crate::utils::list::{List, ListNode};
use core::ptr::NonNull;

/// Round-Robin queue for a specific priority level
pub struct RrQueue {
    /// Queue of threads
    queue: SpinLock<List>,
    /// Last scheduled thread
    last_scheduled: SpinLock<Option<ThreadId>>,
}

impl RrQueue {
    /// Create a new RR queue
    pub const fn new() -> Self {
        Self {
            queue: SpinLock::new(List::new()),
            last_scheduled: SpinLock::new(None),
        }
    }

    /// Add a thread to the queue
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

    /// Get the next thread to run
    pub fn next(&self) -> Option<&ThreadControlBlock> {
        let mut queue = self.queue.lock();
        queue.front().map(|node| unsafe {
            &*(node.as_ptr() as *const ListNode as *const ThreadControlBlock)
        })
    }

    /// Remove and return the next thread
    pub fn dequeue_next(&self) -> Option<&mut ThreadControlBlock> {
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
}

/// Round-Robin scheduler implementation
pub struct RoundRobinScheduler {
    /// RR queues for each priority level
    queues: [RrQueue; 5], // 5 priority levels
    /// Current priority being serviced
    current_priority: SpinLock<usize>,
    /// Time slice duration in milliseconds
    time_slice_ms: u32,
}

impl RoundRobinScheduler {
    /// Create a new Round-Robin scheduler
    pub const fn new(time_slice_ms: u32) -> Self {
        Self {
            queues: [
                RrQueue::new(), // Idle
                RrQueue::new(), // Low
                RrQueue::new(), // Normal
                RrQueue::new(), // High
                RrQueue::new(), // RealTime
            ],
            current_priority: SpinLock::new(2), // Start with Normal priority
            time_slice_ms,
        }
    }

    /// Get the time slice duration
    pub fn time_slice_ms(&self) -> u32 {
        self.time_slice_ms
    }

    /// Set the time slice duration
    pub fn set_time_slice_ms(&mut self, time_slice_ms: u32) {
        self.time_slice_ms = time_slice_ms;
    }

    /// Add a thread to the appropriate RR queue
    pub fn add_thread(&self, tcb: &mut ThreadControlBlock) {
        let priority_index = tcb.priority as usize;
        if priority_index < 5 {
            self.queues[priority_index].enqueue(tcb);
        }
    }

    /// Remove a thread from its RR queue
    pub fn remove_thread(&self, tcb: &mut ThreadControlBlock) {
        let priority_index = tcb.priority as usize;
        if priority_index < 5 {
            self.queues[priority_index].dequeue(tcb);
        }
    }

    /// Schedule the next thread to run
    pub fn schedule(&self, last_scheduled: Option<ThreadId>) -> Option<&ThreadControlBlock> {
        // Check RealTime priority first
        if !self.queues[4].is_empty() {
            return self.queues[4].next();
        }

        // Check High priority
        if !self.queues[3].is_empty() {
            return self.queues[3].next();
        }

        // Check Normal priority
        if !self.queues[2].is_empty() {
            // Use Round-Robin for normal priority
            return self.round_robin_schedule(2, last_scheduled);
        }

        // Check Low priority
        if !self.queues[1].is_empty() {
            return self.round_robin_schedule(1, last_scheduled);
        }

        // Check Idle priority
        if !self.queues[0].is_empty() {
            return self.round_robin_schedule(0, last_scheduled);
        }

        None
    }

    /// Round-Robin schedule for a specific priority level
    fn round_robin_schedule(&self, priority: usize, last_scheduled: Option<ThreadId>) -> Option<&ThreadControlBlock> {
        let queue = &self.queues[priority];

        // If queue has only one thread, return it
        if queue.len() == 1 {
            return queue.next();
        }

        // Find the thread that comes after the last scheduled one
        let mut found_last = false;
        let mut first_thread = None;
        let mut next_thread = None;

        let queue_list = queue.queue.lock();
        let mut current = queue_list.head;

        while let Some(node_ptr) = current {
            let tcb = unsafe {
                &*(node_ptr.as_ptr() as *const ListNode as *const ThreadControlBlock)
            };

            if let Some(last_id) = last_scheduled {
                if tcb.id == last_id {
                    found_last = true;
                } else if found_last && next_thread.is_none() {
                    next_thread = Some(tcb);
                    break;
                }
            }

            if first_thread.is_none() {
                first_thread = Some(tcb);
            }

            current = unsafe { tcb.node.next };
        }

        // If we didn't find the last thread or we've wrapped around, return the first thread
        next_thread.or(first_thread)
    }

    /// Pick and remove the next thread to run
    pub fn pick_next(&self) -> Option<&mut ThreadControlBlock> {
        // Try RealTime priority first
        if !self.queues[4].is_empty() {
            return self.queues[4].dequeue_next();
        }

        // Try High priority
        if !self.queues[3].is_empty() {
            return self.queues[3].dequeue_next();
        }

        // Try Normal priority
        if !self.queues[2].is_empty() {
            return self.queues[2].dequeue_next();
        }

        // Try Low priority
        if !self.queues[1].is_empty() {
            return self.queues[1].dequeue_next();
        }

        // Try Idle priority
        if !self.queues[0].is_empty() {
            return self.queues[0].dequeue_next();
        }

        None
    }

    /// Re-queue a thread after its time slice expires
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

    /// Get the next priority level that has ready threads
    pub fn next_priority_with_threads(&self) -> Option<Priority> {
        // Check from highest to lowest priority
        for (i, queue) in self.queues.iter().enumerate() {
            if !queue.is_empty() {
                return Some(Priority::from_usize(i));
            }
        }
        None
    }
}

/// Convert usize to Priority
impl Priority {
    pub fn from_usize(value: usize) -> Self {
        match value {
            0 => Priority::Idle,
            1 => Priority::Low,
            2 => Priority::Normal,
            3 => Priority::High,
            4 => Priority::RealTime,
            _ => Priority::Normal,
        }
    }
}

/// RR Scheduler statistics
#[derive(Debug, Clone, Copy)]
pub struct RrStats {
    /// Time slice duration in milliseconds
    pub time_slice_ms: u32,
    /// Number of threads per priority level
    pub threads_per_priority: [usize; 5],
    /// Total number of context switches
    pub context_switches: u64,
    /// Average time slice usage (in milliseconds)
    pub avg_time_slice_usage: f64,
}

/// RR Scheduler with statistics tracking
pub struct TrackedRrScheduler {
    /// Base RR scheduler
    scheduler: RoundRobinScheduler,
    /// Statistics
    stats: SpinLock<RrStats>,
}

impl TrackedRrScheduler {
    /// Create a new tracked RR scheduler
    pub const fn new(time_slice_ms: u32) -> Self {
        Self {
            scheduler: RoundRobinScheduler::new(time_slice_ms),
            stats: SpinLock::new(RrStats {
                time_slice_ms,
                threads_per_priority: [0; 5],
                context_switches: 0,
                avg_time_slice_usage: 0.0,
            }),
        }
    }

    /// Add a thread and update statistics
    pub fn add_thread(&self, tcb: &mut ThreadControlBlock) {
        let priority = tcb.priority as usize;
        if priority < 5 {
            {
                let mut stats = self.stats.lock();
                stats.threads_per_priority[priority] += 1;
            }
            self.scheduler.add_thread(tcb);
        }
    }

    /// Remove a thread and update statistics
    pub fn remove_thread(&self, tcb: &mut ThreadControlBlock) {
        let priority = tcb.priority as usize;
        if priority < 5 {
            self.scheduler.remove_thread(tcb);
            {
                let mut stats = self.stats.lock();
                stats.threads_per_priority[priority] = stats.threads_per_priority[priority].saturating_sub(1);
            }
        }
    }

    /// Schedule next thread and track context switches
    pub fn schedule(&self, last_scheduled: Option<ThreadId>) -> Option<&ThreadControlBlock> {
        let next_thread = self.scheduler.schedule(last_scheduled);

        if next_thread.is_some() {
            let mut stats = self.stats.lock();
            stats.context_switches += 1;
        }

        next_thread
    }

    /// Pick next thread for execution
    pub fn pick_next(&self) -> Option<&mut ThreadControlBlock> {
        self.scheduler.pick_next()
    }

    /// Re-queue a thread
    pub fn requeue(&self, tcb: &mut ThreadControlBlock) {
        self.scheduler.requeue(tcb);
    }

    /// Get scheduler statistics
    pub fn get_stats(&self) -> RrStats {
        *self.stats.lock()
    }

    /// Update time slice usage statistics
    pub fn update_time_slice_usage(&self, actual_usage_ms: u32) {
        let mut stats = self.stats.lock();

        // Simple moving average
        let alpha = 0.1; // Smoothing factor
        stats.avg_time_slice_usage =
            stats.avg_time_slice_usage * (1.0 - alpha) +
            (actual_usage_ms as f64) * alpha;
    }

    /// Get the base scheduler
    pub fn base(&self) -> &RoundRobinScheduler {
        &self.scheduler
    }
}

/// Create a Round-Robin scheduler with the specified time slice
pub fn create_rr_scheduler(time_slice_ms: u32) -> TrackedRrScheduler {
    TrackedRrScheduler::new(time_slice_ms)
}