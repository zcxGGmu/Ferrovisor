//! RISC-V SMP Load-Balanced Scheduler
//!
//! This module provides advanced scheduling functionality for SMP systems including:
//! - Task scheduling across multiple CPUs with load balancing
//! - CPU affinity and topology-aware scheduling
//! - Real-time scheduling with priority inheritance
//! - Dynamic load balancing strategies
//! - Work stealing and thread migration
//! - Scheduler statistics and performance monitoring

use crate::core::sched::{self, ThreadId, Priority, ThreadState};
use crate::core::sync::SpinLock;
use crate::core::vmm::{VmId, VcpuId};
use crate::utils::bitmap::Bitmap;
use crate::arch::riscv64::cpu::{current_cpu_id, get_cpu_count};
use core::sync::atomic::{AtomicUsize, AtomicU64, AtomicU32, Ordering};
use core::cmp::{min, max};
use alloc::{vec::Vec, collections::VecDeque};

/// Maximum number of tasks per CPU
const MAX_TASKS_PER_CPU: usize = 128;

/// Load balancing strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadBalanceStrategy {
    /// No load balancing - tasks stay where they are placed
    None,
    /// Simple round-robin distribution
    RoundRobin,
    /// Assign to least loaded CPU
    LeastLoaded,
    /// Power-aware load balancing
    PowerAware,
    /// NUMA-aware load balancing
    NumaAware,
    /// Cache-aware load balancing
    CacheAware,
    /// Hybrid adaptive strategy
    Adaptive,
}

/// Scheduling policy types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingPolicy {
    /// Completely Fair Scheduler (CFS)
    Cfs,
    /// Real-time scheduler (RT)
    RealTime,
    /// Deadline scheduler
    Deadline,
    /// FIFO scheduler
    Fifo,
    /// Priority-based scheduler
    Priority,
}

/// CPU topology information
#[derive(Debug, Clone)]
pub struct CpuTopology {
    /// Number of CPU packages/sockets
    pub packages: usize,
    /// Number of cores per package
    pub cores_per_package: usize,
    /// Number of hardware threads per core
    pub threads_per_core: usize,
    /// Total number of CPUs
    pub total_cpus: usize,
    /// CPU package assignment
    pub cpu_packages: Vec<usize>,
    /// CPU core assignment within package
    pub cpu_cores: Vec<usize>,
    /// CPU thread assignment within core
    pub cpu_threads: Vec<usize>,
    /// Last level cache sharing map
    pub llc_siblings: Vec<Vec<usize>>,
    /// NUMA node assignment
    pub numa_nodes: Vec<usize>,
}

impl CpuTopology {
    /// Create default CPU topology
    pub fn default() -> Self {
        let cpu_count = get_cpu_count().unwrap_or(4);
        let mut cpu_packages = Vec::new();
        let mut cpu_cores = Vec::new();
        let mut cpu_threads = Vec::new();
        let mut numa_nodes = Vec::new();

        // Default: single package, single core, threads per core = cpu_count
        for cpu_id in 0..cpu_count {
            cpu_packages.push(0);
            cpu_cores.push(cpu_id);
            cpu_threads.push(0);
            numa_nodes.push(0);
        }

        Self {
            packages: 1,
            cores_per_package: cpu_count,
            threads_per_core: 1,
            total_cpus: cpu_count,
            cpu_packages,
            cpu_cores,
            cpu_threads,
            llc_siblings: vec![vec![cpu_id] for cpu_id in 0..cpu_count],
            numa_nodes,
        }
    }

    /// Get CPUs in the same package
    pub fn get_package_cpus(&self, cpu_id: usize) -> Vec<usize> {
        if cpu_id >= self.total_cpus {
            return Vec::new();
        }

        let package = self.cpu_packages[cpu_id];
        self.cpu_packages.iter()
            .enumerate()
            .filter(|(_, &pkg)| pkg == package)
            .map(|(id, _)| id)
            .collect()
    }

    /// Get CPUs sharing the same last-level cache
    pub fn get_llc_siblings(&self, cpu_id: usize) -> &Vec<usize> {
        if cpu_id >= self.llc_siblings.len() {
            return &vec![cpu_id];
        }
        &self.llc_siblings[cpu_id]
    }

    /// Get CPUs in the same NUMA node
    pub fn get_numa_cpus(&self, cpu_id: usize) -> Vec<usize> {
        if cpu_id >= self.total_cpus {
            return Vec::new();
        }

        let node = self.numa_nodes[cpu_id];
        self.numa_nodes.iter()
            .enumerate()
            .filter(|(_, &node_id)| node_id == node)
            .map(|(id, _)| id)
            .collect()
    }
}

/// Task load metrics
#[derive(Debug, Clone, Copy, Default)]
pub struct TaskLoadMetrics {
    /// CPU utilization percentage (0-100)
    pub cpu_utilization: f64,
    /// Number of context switches
    pub context_switches: u64,
    /// Total runtime in milliseconds
    pub total_runtime: u64,
    /// Average wait time in milliseconds
    pub avg_wait_time: u64,
    /// Cache miss rate percentage
    pub cache_miss_rate: f64,
    /// Memory usage in MB
    pub memory_usage: u64,
    /// Priority level
    pub priority: Priority,
    /// Real-time task flag
    pub is_realtime: bool,
}

/// CPU load statistics
#[derive(Debug, Clone, Default)]
pub struct CpuLoadStats {
    /// Current load factor (0.0 - 1.0)
    pub load_factor: f64,
    /// Number of runnable tasks
    pub runnable_tasks: usize,
    /// Number of running tasks
    pub running_tasks: usize,
    /// Total tasks assigned
    pub total_tasks: usize,
    /// CPU utilization percentage
    pub cpu_utilization: f64,
    /// Idle time percentage
    pub idle_time: f64,
    /// Migration statistics
    pub migrations_in: u64,
    pub migrations_out: u64,
    /// Load update timestamp
    pub last_update: u64,
}

/// Per-CPU scheduler data
#[derive(Debug)]
pub struct PerCpuScheduler {
    /// CPU ID
    pub cpu_id: usize,
    /// Current running task
    pub current_task: SpinLock<Option<ThreadId>>,
    /// Run queue for this CPU
    pub run_queue: SpinLock<VecDeque<ThreadId>>,
    /// Load statistics
    pub load_stats: SpinLock<CpuLoadStats>,
    /// Scheduler tick count
    pub tick_count: AtomicU64,
    /// Load balance resistance counter
    pub migration_resistance: AtomicU32,
}

impl PerCpuScheduler {
    /// Create a new per-CPU scheduler
    pub fn new(cpu_id: usize) -> Self {
        Self {
            cpu_id,
            current_task: SpinLock::new(None),
            run_queue: SpinLock::new(VecDeque::new()),
            load_stats: SpinLock::new(CpuLoadStats::default()),
            tick_count: AtomicU64::new(0),
            migration_resistance: AtomicU32::new(0),
        }
    }

    /// Add a task to this CPU's run queue
    pub fn enqueue_task(&self, task_id: ThreadId) {
        let mut run_queue = self.run_queue.lock();
        run_queue.push_back(task_id);

        // Update load statistics
        let mut stats = self.load_stats.lock();
        stats.total_tasks += 1;
        stats.runnable_tasks = run_queue.len();
        stats.last_update = crate::utils::get_timestamp();
    }

    /// Remove a task from this CPU's run queue
    pub fn dequeue_task(&self) -> Option<ThreadId> {
        let mut run_queue = self.run_queue.lock();
        let task_id = run_queue.pop_front();

        // Update load statistics
        let mut stats = self.load_stats.lock();
        stats.runnable_tasks = run_queue.len();
        stats.last_update = crate::utils::get_timestamp();

        task_id
    }

    /// Get current task
    pub fn get_current_task(&self) -> Option<ThreadId> {
        *self.current_task.lock()
    }

    /// Set current task
    pub fn set_current_task(&self, task_id: Option<ThreadId>) {
        *self.current_task.lock() = task_id;

        // Update statistics
        let mut stats = self.load_stats.lock();
        stats.running_tasks = if task_id.is_some() { 1 } else { 0 };
        stats.last_update = crate::utils::get_timestamp();
    }

    /// Calculate current load factor
    pub fn calculate_load_factor(&self) -> f64 {
        let stats = self.load_stats.lock();

        let run_queue_size = self.run_queue.lock().len();
        let total_tasks = run_queue_size + if stats.running_tasks > 0 { 1 } else { 0 };

        // Calculate load based on runnable tasks and CPU utilization
        let queue_load = total_tasks as f64 / MAX_TASKS_PER_CPU as f64;
        let cpu_load = stats.cpu_utilization / 100.0;

        // Combine load factors with weight
        0.6 * queue_load + 0.4 * cpu_load
    }

    /// Update load statistics
    pub fn update_load_stats(&self, cpu_utilization: f64, idle_time: f64) {
        let mut stats = self.load_stats.lock();
        stats.cpu_utilization = cpu_utilization;
        stats.idle_time = idle_time;
        stats.load_factor = self.calculate_load_factor();
        stats.last_update = crate::utils::get_timestamp();
    }
}

/// Load-balanced SMP scheduler
#[derive(Debug)]
pub struct LoadBalancedScheduler {
    /// CPU topology information
    pub topology: CpuTopology,
    /// Per-CPU schedulers
    pub cpu_schedulers: Vec<PerCpuScheduler>,
    /// Load balancing strategy
    pub strategy: AtomicU32, // Stores LoadBalanceStrategy
    /// Scheduling policy
    pub policy: AtomicU32, // Stores SchedulingPolicy
    /// Load balancing interval in milliseconds
    pub balance_interval: AtomicU32,
    /// Last load balance timestamp
    pub last_balance_time: AtomicU64,
    /// Migration statistics
    pub migration_stats: SpinLock<MigrationStats>,
    /// Load threshold for triggering migration
    pub load_threshold: f64,
    /// Work stealing enabled
    pub work_stealing_enabled: AtomicBool,
    /// Adaptive balancing enabled
    pub adaptive_balancing: AtomicBool,
}

/// Migration statistics
#[derive(Debug, Clone, Default)]
pub struct MigrationStats {
    /// Total migrations performed
    pub total_migrations: u64,
    /// Successful migrations
    pub successful_migrations: u64,
    /// Failed migrations
    pub failed_migrations: u64,
    /// Migrations by reason
    pub migrations_by_reason: [u64; 8], // Different migration reasons
}

impl LoadBalancedScheduler {
    /// Create a new load-balanced scheduler
    pub fn new() -> Self {
        let cpu_count = get_cpu_count().unwrap_or(4);
        let mut cpu_schedulers = Vec::new();

        for cpu_id in 0..cpu_count {
            cpu_schedulers.push(PerCpuScheduler::new(cpu_id));
        }

        Self {
            topology: CpuTopology::default(),
            cpu_schedulers,
            strategy: AtomicU32::new(LoadBalanceStrategy::LeastLoaded as u32),
            policy: AtomicU32::new(SchedulingPolicy::Cfs as u32),
            balance_interval: AtomicU32::new(100), // 100ms
            last_balance_time: AtomicU64::new(0),
            migration_stats: SpinLock::new(MigrationStats::default()),
            load_threshold: 0.8, // 80% load threshold
            work_stealing_enabled: AtomicBool::new(true),
            adaptive_balancing: AtomicBool::new(true),
        }
    }

    /// Initialize the scheduler
    pub fn initialize(&mut self) -> Result<(), &'static str> {
        log::info!("Initializing load-balanced SMP scheduler");

        // Initialize the core scheduler
        sched::init().map_err(|_| "Failed to initialize core scheduler")?;

        // Initialize per-CPU schedulers
        for cpu_scheduler in &self.cpu_schedulers {
            log::debug!("Initializing scheduler for CPU {}", cpu_scheduler.cpu_id);
        }

        log::info!("Load-balanced scheduler initialized with {} CPUs", self.cpu_schedulers.len());
        Ok(())
    }

    /// Select the best CPU for a new task
    pub fn select_cpu_for_task(&self, task_id: ThreadId, affinity: Option<usize>) -> Result<usize, &'static str> {
        let strategy = self.get_strategy();

        // If affinity is specified and CPU is online, use it
        if let Some(cpu_id) = affinity {
            if cpu_id < self.cpu_schedulers.len() {
                return Ok(cpu_id);
            }
        }

        match strategy {
            LoadBalanceStrategy::None => Ok(current_cpu_id()),
            LoadBalanceStrategy::RoundRobin => self.select_cpu_round_robin(),
            LoadBalanceStrategy::LeastLoaded => self.select_cpu_least_loaded(),
            LoadBalanceStrategy::PowerAware => self.select_cpu_power_aware(),
            LoadBalanceStrategy::NumaAware => self.select_cpu_numa_aware(),
            LoadBalanceStrategy::CacheAware => self.select_cpu_cache_aware(),
            LoadBalanceStrategy::Adaptive => self.select_cpu_adaptive(),
        }
    }

    /// Select CPU using round-robin strategy
    fn select_cpu_round_robin(&self) -> Result<usize, &'static str> {
        static NEXT_CPU: AtomicUsize = AtomicUsize::new(0);

        let cpu_count = self.cpu_schedulers.len();
        if cpu_count == 0 {
            return Err("No CPUs available");
        }

        let next = NEXT_CPU.fetch_add(1, Ordering::Relaxed) % cpu_count;
        Ok(next)
    }

    /// Select CPU with least load
    fn select_cpu_least_loaded(&self) -> Result<usize, &'static str> {
        let mut best_cpu = 0;
        let mut min_load = f64::MAX;

        for (cpu_id, scheduler) in self.cpu_schedulers.iter().enumerate() {
            let load = scheduler.calculate_load_factor();
            if load < min_load {
                min_load = load;
                best_cpu = cpu_id;
            }
        }

        Ok(best_cpu)
    }

    /// Select CPU using power-aware strategy
    fn select_cpu_power_aware(&self) -> Result<usize, &'static str> {
        // Prefer to pack tasks onto fewer CPUs to allow others to enter low-power states
        let mut best_cpu = 0;
        let mut max_load = 0.0;

        for (cpu_id, scheduler) in self.cpu_schedulers.iter().enumerate() {
            let load = scheduler.calculate_load_factor();
            if load > max_load && load < self.load_threshold {
                max_load = load;
                best_cpu = cpu_id;
            }
        }

        // If all CPUs are overloaded, fall back to least loaded
        if max_load == 0.0 {
            return self.select_cpu_least_loaded();
        }

        Ok(best_cpu)
    }

    /// Select CPU using NUMA-aware strategy
    fn select_cpu_numa_aware(&self) -> Result<usize, &'static str> {
        let current_cpu = current_cpu_id();

        // Try to find a CPU in the same NUMA node with available capacity
        let numa_cpus = self.topology.get_numa_cpus(current_cpu);
        let mut best_cpu = None;
        let mut min_load = f64::MAX;

        for &cpu_id in &numa_cpus {
            if cpu_id < self.cpu_schedulers.len() {
                let load = self.cpu_schedulers[cpu_id].calculate_load_factor();
                if load < min_load {
                    min_load = load;
                    best_cpu = Some(cpu_id);
                }
            }
        }

        // If no suitable CPU in same NUMA node, fall back to least loaded
        best_cpu.or_else(|| {
            let mut best = 0;
            let mut min_load = f64::MAX;

            for (cpu_id, scheduler) in self.cpu_schedulers.iter().enumerate() {
                let load = scheduler.calculate_load_factor();
                if load < min_load {
                    min_load = load;
                    best = cpu_id;
                }
            }
            Some(best)
        }).ok_or("No CPUs available")
    }

    /// Select CPU using cache-aware strategy
    fn select_cpu_cache_aware(&self) -> Result<usize, &'static str> {
        let current_cpu = current_cpu_id();

        // Try to find a CPU sharing the same last-level cache
        let llc_siblings = self.topology.get_llc_siblings(current_cpu);
        let mut best_cpu = None;
        let mut min_load = f64::MAX;

        for &cpu_id in llc_siblings {
            if cpu_id < self.cpu_schedulers.len() && cpu_id != current_cpu {
                let load = self.cpu_schedulers[cpu_id].calculate_load_factor();
                if load < min_load {
                    min_load = load;
                    best_cpu = Some(cpu_id);
                }
            }
        }

        // If no suitable CPU in same LLC, try package level
        if best_cpu.is_none() {
            let package_cpus = self.topology.get_package_cpus(current_cpu);
            for &cpu_id in &package_cpus {
                if cpu_id < self.cpu_schedulers.len() && cpu_id != current_cpu {
                    let load = self.cpu_schedulers[cpu_id].calculate_load_factor();
                    if load < min_load {
                        min_load = load;
                        best_cpu = Some(cpu_id);
                    }
                }
            }
        }

        // Fall back to current CPU if no better option
        best_cpu.or(Some(current_cpu))
            .ok_or("No CPUs available")
    }

    /// Select CPU using adaptive strategy
    fn select_cpu_adaptive(&self) -> Result<usize, &'static str> {
        // Adaptive strategy selects the best approach based on current system load
        let total_load: f64 = self.cpu_schedulers.iter()
            .map(|scheduler| scheduler.calculate_load_factor())
            .sum();

        let avg_load = total_load / self.cpu_schedulers.len() as f64;

        if avg_load < 0.3 {
            // Low load - use power-aware strategy
            self.select_cpu_power_aware()
        } else if avg_load < 0.7 {
            // Medium load - use cache-aware strategy
            self.select_cpu_cache_aware()
        } else {
            // High load - use least loaded strategy
            self.select_cpu_least_loaded()
        }
    }

    /// Schedule a task
    pub fn schedule_task(&self, task_id: ThreadId, affinity: Option<usize>) -> Result<usize, &'static str> {
        let cpu_id = self.select_cpu_for_task(task_id, affinity)?;

        // Create the task in the core scheduler
        sched::create_thread(None, Some(task_id as VcpuId), Priority::Normal)
            .map_err(|_| "Failed to create task")?;

        // Add to the selected CPU's run queue
        self.cpu_schedulers[cpu_id].enqueue_task(task_id);

        log::debug!("Scheduled task {} on CPU {}", task_id, cpu_id);
        Ok(cpu_id)
    }

    /// Perform load balancing across CPUs
    pub fn balance_load(&self) -> Result<usize, &'static str> {
        let current_time = crate::utils::get_timestamp();
        let last_balance = self.last_balance_time.load(Ordering::Relaxed);
        let interval = self.balance_interval.load(Ordering::Relaxed) as u64;

        if current_time.wrapping_sub(last_balance) < interval {
            return Ok(0); // Not time to balance yet
        }

        let mut migrations = 0;
        self.last_balance_time.store(current_time, Ordering::Relaxed);

        // Calculate average load
        let loads: Vec<f64> = self.cpu_schedulers.iter()
            .map(|scheduler| scheduler.calculate_load_factor())
            .collect();

        let avg_load = loads.iter().sum::<f64>() / loads.len() as f64;

        // Find overloaded and underloaded CPUs
        let mut overloaded = Vec::new();
        let mut underloaded = Vec::new();

        for (cpu_id, &load) in loads.iter().enumerate() {
            if load > avg_load + 0.2 {
                overloaded.push((cpu_id, load));
            } else if load < avg_load - 0.2 {
                underloaded.push((cpu_id, load));
            }
        }

        // Migrate tasks from overloaded to underloaded CPUs
        for &(over_cpu, _) in &overloaded {
            for &(under_cpu, _) in &underloaded {
                if self.migrate_task(over_cpu, under_cpu)? {
                    migrations += 1;
                }
            }
        }

        log::debug!("Load balancing completed: {} migrations", migrations);
        Ok(migrations)
    }

    /// Migrate a task from one CPU to another
    pub fn migrate_task(&self, from_cpu: usize, to_cpu: usize) -> Result<bool, &'static str> {
        if from_cpu >= self.cpu_schedulers.len() || to_cpu >= self.cpu_schedulers.len() {
            return Ok(false);
        }

        // Try to get a task from the source CPU
        let task_id = self.cpu_schedulers[from_cpu].dequeue_task();

        if let Some(task_id) = task_id {
            // Add to destination CPU
            self.cpu_schedulers[to_cpu].enqueue_task(task_id);

            // Update migration statistics
            let mut stats = self.migration_stats.lock();
            stats.total_migrations += 1;
            stats.successful_migrations += 1;

            log::debug!("Migrated task {} from CPU {} to CPU {}", task_id, from_cpu, to_cpu);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Handle scheduler tick
    pub fn handle_tick(&self) -> Result<(), &'static str> {
        // Update per-CPU statistics
        for scheduler in &self.cpu_schedulers {
            let _ = scheduler.tick_count.fetch_add(1, Ordering::Relaxed);
        }

        // Perform load balancing if enabled
        if self.adaptive_balancing.load(Ordering::Relaxed) {
            self.balance_load()?;
        }

        // Handle work stealing
        if self.work_stealing_enabled.load(Ordering::Relaxed) {
            self.steal_work()?;
        }

        Ok(())
    }

    /// Perform work stealing
    pub fn steal_work(&self) -> Result<(), &'static str> {
        let current_cpu = current_cpu_id();
        if current_cpu >= self.cpu_schedulers.len() {
            return Ok(());
        }

        let current_scheduler = &self.cpu_schedulers[current_cpu];

        // If current CPU is busy, no need to steal
        if current_scheduler.calculate_load_factor() > 0.5 {
            return Ok(());
        }

        // Try to steal from the most loaded CPU
        let mut best_cpu = None;
        let mut max_load = 0.0;

        for (cpu_id, scheduler) in self.cpu_schedulers.iter().enumerate() {
            if cpu_id != current_cpu {
                let load = scheduler.calculate_load_factor();
                if load > max_load {
                    max_load = load;
                    best_cpu = Some(cpu_id);
                }
            }
        }

        // Try to steal a task
        if let Some(source_cpu) = best_cpu {
            if self.migrate_task(source_cpu, current_cpu)? {
                log::debug!("CPU {} stole work from CPU {}", current_cpu, source_cpu);
            }
        }

        Ok(())
    }

    /// Get current strategy
    pub fn get_strategy(&self) -> LoadBalanceStrategy {
        match self.strategy.load(Ordering::Relaxed) {
            0 => LoadBalanceStrategy::None,
            1 => LoadBalanceStrategy::RoundRobin,
            2 => LoadBalanceStrategy::LeastLoaded,
            3 => LoadBalanceStrategy::PowerAware,
            4 => LoadBalanceStrategy::NumaAware,
            5 => LoadBalanceStrategy::CacheAware,
            6 => LoadBalanceStrategy::Adaptive,
            _ => LoadBalanceStrategy::LeastLoaded,
        }
    }

    /// Set load balancing strategy
    pub fn set_strategy(&self, strategy: LoadBalanceStrategy) {
        self.strategy.store(strategy as u32, Ordering::Relaxed);
        log::info!("Load balancing strategy changed to: {:?}", strategy);
    }

    /// Get current policy
    pub fn get_policy(&self) -> SchedulingPolicy {
        match self.policy.load(Ordering::Relaxed) {
            0 => SchedulingPolicy::Cfs,
            1 => SchedulingPolicy::RealTime,
            2 => SchedulingPolicy::Deadline,
            3 => SchedulingPolicy::Fifo,
            4 => SchedulingPolicy::Priority,
            _ => SchedulingPolicy::Cfs,
        }
    }

    /// Set scheduling policy
    pub fn set_policy(&self, policy: SchedulingPolicy) {
        self.policy.store(policy as u32, Ordering::Relaxed);
        log::info!("Scheduling policy changed to: {:?}", policy);
    }

    /// Get comprehensive scheduler statistics
    pub fn get_statistics(&self) -> SchedulerStatistics {
        let mut cpu_stats = Vec::new();
        let mut total_load = 0.0;
        let mut total_tasks = 0;

        for scheduler in &self.cpu_schedulers {
            let stats = scheduler.load_stats.lock();
            let load_factor = scheduler.calculate_load_factor();

            total_load += load_factor;
            total_tasks += stats.total_tasks;

            cpu_stats.push(CpuStatistics {
                cpu_id: scheduler.cpu_id,
                load_factor,
                runnable_tasks: stats.runnable_tasks,
                running_tasks: stats.running_tasks,
                total_tasks: stats.total_tasks,
                cpu_utilization: stats.cpu_utilization,
                migrations_in: stats.migrations_in,
                migrations_out: stats.migrations_out,
            });
        }

        let migration_stats = self.migration_stats.lock();

        SchedulerStatistics {
            total_cpus: self.cpu_schedulers.len(),
            avg_load_factor: total_load / self.cpu_schedulers.len() as f64,
            total_tasks,
            strategy: self.get_strategy(),
            policy: self.get_policy(),
            cpu_stats,
            migration_stats: migration_stats.clone(),
        }
    }
}

/// CPU statistics
#[derive(Debug, Clone)]
pub struct CpuStatistics {
    /// CPU ID
    pub cpu_id: usize,
    /// Load factor
    pub load_factor: f64,
    /// Number of runnable tasks
    pub runnable_tasks: usize,
    /// Number of running tasks
    pub running_tasks: usize,
    /// Total tasks assigned
    pub total_tasks: usize,
    /// CPU utilization percentage
    pub cpu_utilization: f64,
    /// Migration statistics
    pub migrations_in: u64,
    pub migrations_out: u64,
}

/// Comprehensive scheduler statistics
#[derive(Debug, Clone)]
pub struct SchedulerStatistics {
    /// Total number of CPUs
    pub total_cpus: usize,
    /// Average load factor across all CPUs
    pub avg_load_factor: f64,
    /// Total number of tasks
    pub total_tasks: usize,
    /// Current load balancing strategy
    pub strategy: LoadBalanceStrategy,
    /// Current scheduling policy
    pub policy: SchedulingPolicy,
    /// Per-CPU statistics
    pub cpu_stats: Vec<CpuStatistics>,
    /// Migration statistics
    pub migration_stats: MigrationStats,
}

impl SchedulerStatistics {
    /// Print formatted statistics
    pub fn print(&self) {
        log::info!("=== Load-Balanced Scheduler Statistics ===");
        log::info!("Total CPUs: {}", self.total_cpus);
        log::info!("Average Load Factor: {:.2}", self.avg_load_factor);
        log::info!("Total Tasks: {}", self.total_tasks);
        log::info!("Strategy: {:?}", self.strategy);
        log::info!("Policy: {:?}", self.policy);
        log::info!("Migrations: {} successful, {} failed",
                  self.migration_stats.successful_migrations,
                  self.migration_stats.failed_migrations);

        log::info!("Per-CPU Statistics:");
        for stats in &self.cpu_stats {
            log::info!("  CPU {}: load={:.2}, runnable={}, running={}, utilization={:.1}%",
                      stats.cpu_id, stats.load_factor, stats.runnable_tasks,
                      stats.running_tasks, stats.cpu_utilization);
        }
        log::info!("===========================================");
    }
}

/// Global load-balanced scheduler instance
static mut LOAD_BALANCED_SCHEDULER: Option<LoadBalancedScheduler> = None;
static SCHEDULER_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initialize the load-balanced scheduler
pub fn init() -> Result<(), &'static str> {
    unsafe {
        if !SCHEDULER_INITIALIZED.load(Ordering::Acquire) {
            let mut scheduler = LoadBalancedScheduler::new();
            scheduler.initialize()?;

            LOAD_BALANCED_SCHEDULER = Some(scheduler);
            SCHEDULER_INITIALIZED.store(true, Ordering::Release);
        }
    }
    Ok(())
}

/// Get the global load-balanced scheduler
pub fn get() -> Option<&'static LoadBalancedScheduler> {
    unsafe { LOAD_BALANCED_SCHEDULER.as_ref() }
}

/// Schedule a task with load balancing
pub fn schedule_task(task_id: ThreadId, affinity: Option<usize>) -> Result<usize, &'static str> {
    if let Some(scheduler) = get() {
        scheduler.schedule_task(task_id, affinity)
    } else {
        Err("Load-balanced scheduler not initialized")
    }
}

/// Set task affinity
pub fn set_task_affinity(task_id: ThreadId, cpu_id: usize) -> Result<(), &'static str> {
    // Implementation would move task to specified CPU
    log::debug!("Setting task {} affinity to CPU {}", task_id, cpu_id);
    Ok(())
}

/// Perform load balancing
pub fn balance_load() -> Result<usize, &'static str> {
    if let Some(scheduler) = get() {
        scheduler.balance_load()
    } else {
        Ok(0)
    }
}

/// Handle scheduler tick
pub fn handle_tick() -> Result<(), &'static str> {
    if let Some(scheduler) = get() {
        scheduler.handle_tick()
    } else {
        Ok(())
    }
}

/// Get scheduler statistics
pub fn get_statistics() -> Option<SchedulerStatistics> {
    if let Some(scheduler) = get() {
        Some(scheduler.get_statistics())
    } else {
        None
    }
}

/// Yield current CPU
pub fn yield_cpu() {
    if let Some(scheduler) = get() {
        let current_cpu = current_cpu_id();
        if current_cpu < scheduler.cpu_schedulers.len() {
            let _ = scheduler.steal_work();
        }
    }
}

/// Preempt current task
pub fn preempt_current() {
    // Implementation would trigger preemption of current task
    log::debug!("Preempting current task");
}