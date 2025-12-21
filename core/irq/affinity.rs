//! Advanced Interrupt Affinity Management
//!
//! This module provides comprehensive interrupt affinity management capabilities
//! including CPU topology awareness, load balancing, and dynamic affinity adjustment.

use crate::{Result, Error};
use crate::core::irq::{IrqNumber, Priority, IrqType, InterruptDescriptor};
use crate::core::sync::SpinLock;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};

/// Maximum number of CPUs supported
pub const MAX_CPUS: usize = 64;

/// CPU mask for affinity operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuMask {
    bits: u64,
}

impl CpuMask {
    /// Create an empty CPU mask
    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    /// Create a CPU mask with all bits set
    pub const fn all() -> Self {
        Self { bits: u64::MAX }
    }

    /// Create a CPU mask from a bit pattern
    pub const fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    /// Create a CPU mask for a single CPU
    pub fn from_cpu(cpu: u32) -> Self {
        Self { bits: 1u64 << cpu }
    }

    /// Get the underlying bits
    pub const fn bits(&self) -> u64 {
        self.bits
    }

    /// Check if a CPU is set in the mask
    pub fn contains(&self, cpu: u32) -> bool {
        (self.bits & (1u64 << cpu)) != 0
    }

    /// Set a CPU in the mask
    pub fn set(&mut self, cpu: u32) {
        self.bits |= 1u64 << cpu;
    }

    /// Clear a CPU in the mask
    pub fn clear(&mut self, cpu: u32) {
        self.bits &= !(1u64 << cpu);
    }

    /// Check if the mask is empty
    pub fn is_empty(&self) -> bool {
        self.bits == 0
    }

    /// Check if the mask has all CPUs
    pub fn is_all(&self) -> bool {
        self.bits == u64::MAX
    }

    /// Count the number of CPUs in the mask
    pub fn count(&self) -> u32 {
        self.bits.count_ones()
    }

    /// Get the first CPU in the mask
    pub fn first(&self) -> Option<u32> {
        if self.is_empty() {
            None
        } else {
            Some(self.bits.trailing_zeros())
        }
    }

    /// Get a random CPU from the mask
    pub fn random(&self) -> Option<u32> {
        if self.is_empty() {
            None
        } else {
            let count = self.count();
            let index = (crate::utils::random::u32() % count) as u32;
            let mut bits = self.bits;

            // Skip to the selected index
            for _ in 0..index {
                bits &= bits - 1; // Clear the lowest set bit
            }

            Some(bits.trailing_zeros())
        }
    }

    /// Perform bitwise AND with another mask
    pub fn and(&self, other: &CpuMask) -> CpuMask {
        CpuMask::from_bits(self.bits & other.bits)
    }

    /// Perform bitwise OR with another mask
    pub fn or(&self, other: &CpuMask) -> CpuMask {
        CpuMask::from_bits(self.bits | other.bits)
    }

    /// Perform bitwise XOR with another mask
    pub fn xor(&self, other: &CpuMask) -> CpuMask {
        CpuMask::from_bits(self.bits ^ other.bits)
    }

    /// Get the complement of the mask
    pub fn not(&self) -> CpuMask {
        CpuMask::from_bits(!self.bits)
    }

    /// Iterate over CPUs in the mask
    pub fn iter(&self) -> CpuIter {
        CpuIter { bits: self.bits }
    }
}

/// Iterator over CPUs in a mask
pub struct CpuIter {
    bits: u64,
}

impl Iterator for CpuIter {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bits == 0 {
            None
        } else {
            let cpu = self.bits.trailing_zeros();
            self.bits &= self.bits - 1; // Clear the lowest set bit
            Some(cpu)
        }
    }
}

impl Default for CpuMask {
    fn default() -> Self {
        Self::new()
    }
}

/// CPU topology information
#[derive(Debug, Clone)]
pub struct CpuTopology {
    /// Total number of CPUs
    pub total_cpus: u32,
    /// Number of packages/sockets
    pub packages: u32,
    /// Number of cores per package
    pub cores_per_package: u32,
    /// Number of threads per core
    pub threads_per_core: u32,
    /// CPU package mapping
    pub cpu_to_package: Vec<u32>,
    /// CPU core mapping
    pub cpu_to_core: Vec<u32>,
    /// CPU thread mapping
    pub cpu_to_thread: Vec<u32>,
    /// Package masks
    pub package_masks: Vec<CpuMask>,
    /// Core masks
    pub core_masks: Vec<CpuMask>,
}

impl CpuTopology {
    /// Create a simple CPU topology
    pub fn new(total_cpus: u32) -> Self {
        let mut cpu_to_package = Vec::new();
        let mut cpu_to_core = Vec::new();
        let mut cpu_to_thread = Vec::new();

        for cpu in 0..total_cpus {
            cpu_to_package.push(cpu / 8);  // Assume 8 cores per package
            cpu_to_core.push(cpu % 8);
            cpu_to_thread.push(0);         // Assume no hyperthreading
        }

        let packages = (total_cpus + 7) / 8;
        let mut package_masks = Vec::new();
        let mut core_masks = Vec::new();

        // Create package masks
        for pkg in 0..packages {
            let mut mask = CpuMask::new();
            for cpu in 0..total_cpus {
                if cpu_to_package[cpu as usize] == pkg {
                    mask.set(cpu);
                }
            }
            package_masks.push(mask);
        }

        // Create core masks
        for core in 0..total_cpus {
            core_masks.push(CpuMask::from_cpu(core));
        }

        Self {
            total_cpus,
            packages,
            cores_per_package: 8,
            threads_per_core: 1,
            cpu_to_package,
            cpu_to_core,
            cpu_to_thread,
            package_masks,
            core_masks,
        }
    }

    /// Get CPUs in the same package as the given CPU
    pub fn get_package_cpus(&self, cpu: u32) -> CpuMask {
        if cpu as usize >= self.cpu_to_package.len() {
            CpuMask::new()
        } else {
            let package = self.cpu_to_package[cpu as usize];
            self.package_masks[package as usize]
        }
    }

    /// Get CPUs in the same core as the given CPU
    pub fn get_core_cpus(&self, cpu: u32) -> CpuMask {
        if cpu as usize >= self.cpu_to_core.len() {
            CpuMask::new()
        } else {
            let core = self.cpu_to_core[cpu as usize];
            self.core_masks[core as usize]
        }
    }

    /// Check if two CPUs are on the same package
    pub fn same_package(&self, cpu1: u32, cpu2: u32) -> bool {
        if cpu1 as usize >= self.cpu_to_package.len() || cpu2 as usize >= self.cpu_to_package.len() {
            false
        } else {
            self.cpu_to_package[cpu1 as usize] == self.cpu_to_package[cpu2 as usize]
        }
    }

    /// Check if two CPUs are on the same core
    pub fn same_core(&self, cpu1: u32, cpu2: u32) -> bool {
        if cpu1 as usize >= self.cpu_to_core.len() || cpu2 as usize >= self.cpu_to_core.len() {
            false
        } else {
            self.cpu_to_core[cpu1 as usize] == self.cpu_to_core[cpu2 as usize]
        }
    }
}

/// Per-CPU interrupt statistics
#[derive(Debug, Default)]
pub struct CpuIrqStats {
    /// Total interrupts handled
    pub total_interrupts: AtomicU64,
    /// Interrupts by priority level
    pub priority_counts: [AtomicU64; 5],
    /// Interrupts by type
    pub type_counts: [AtomicU64; 3],
    /// Last interrupt timestamp
    pub last_interrupt: AtomicU64,
    /// Average interrupt processing time (nanoseconds)
    pub avg_processing_time: AtomicU32,
    /// Number of spurious interrupts
    pub spurious_interrupts: AtomicU64,
}

impl CpuIrqStats {
    /// Create new CPU IRQ statistics
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an interrupt
    pub fn record_interrupt(&self, irq_type: IrqType, priority: Priority, processing_time_ns: u32) {
        self.total_interrupts.fetch_add(1, Ordering::Relaxed);

        let priority_idx = match priority {
            Priority::Lowest => 0,
            Priority::Low => 1,
            Priority::Normal => 2,
            Priority::High => 3,
            Priority::Highest => 4,
        };
        self.priority_counts[priority_idx].fetch_add(1, Ordering::Relaxed);

        let type_idx = match irq_type {
            IrqType::Software => 0,
            IrqType::Hardware => 1,
            IrqType::Ipi => 2,
        };
        self.type_counts[type_idx].fetch_add(1, Ordering::Relaxed);

        // Update average processing time
        let current_avg = self.avg_processing_time.load(Ordering::Relaxed);
        let new_avg = ((current_avg as u64) + processing_time_ns as u64) / 2;
        self.avg_processing_time.store(new_avg as u32, Ordering::Relaxed);

        // Update timestamp
        self.last_interrupt.store(crate::utils::time::timestamp_ns(), Ordering::Relaxed);
    }

    /// Record a spurious interrupt
    pub fn record_spurious(&self) {
        self.spurious_interrupts.fetch_add(1, Ordering::Relaxed);
    }

    /// Get interrupt rate per second
    pub fn get_interrupt_rate(&self) -> f64 {
        let total = self.total_interrupts.load(Ordering::Relaxed);
        let last = self.last_interrupt.load(Ordering::Relaxed);
        let now = crate::utils::time::timestamp_ns();

        if now > last {
            let elapsed_seconds = (now - last) as f64 / 1_000_000_000.0;
            if elapsed_seconds > 0.0 {
                total as f64 / elapsed_seconds
            } else {
                0.0
            }
        } else {
            0.0
        }
    }
}

/// Load balancing strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadBalanceStrategy {
    /// No load balancing
    None,
    /// Round-robin distribution
    RoundRobin,
    /// Least loaded CPU
    LeastLoaded,
    /// Package-aware distribution
    PackageAware,
    /// Core-aware distribution
    CoreAware,
    /// NUMA-aware distribution
    NumaAware,
}

/// Interrupt affinity manager
pub struct InterruptAffinityManager {
    /// CPU topology
    topology: CpuTopology,
    /// Online CPU mask
    online_cpus: SpinLock<CpuMask>,
    /// Active CPU mask
    active_cpus: SpinLock<CpuMask>,
    /// Per-CPU statistics
    cpu_stats: Vec<SpinLock<CpuIrqStats>>,
    /// Load balancing strategy
    strategy: AtomicU32,
    /// Round-robin counter
    rr_counter: AtomicU32,
    /// Per-IRQ affinity cache
    irq_affinity_cache: SpinLock<Vec<Option<CpuMask>>>,
}

/// Affinity hints for interrupts
#[derive(Debug, Clone, Copy)]
pub struct AffinityHints {
    /// Preferred CPUs (if available)
    pub preferred_cpus: CpuMask,
    /// Avoid these CPUs
    pub avoid_cpus: CpuMask,
    /// Require specific capabilities
    pub required_capabilities: u32,
    /// Whether this is high-frequency interrupt
    pub high_frequency: bool,
    /// Whether this is latency-sensitive
    pub latency_sensitive: bool,
}

impl AffinityHints {
    /// Create new affinity hints
    pub fn new() -> Self {
        Self {
            preferred_cpus: CpuMask::new(),
            avoid_cpus: CpuMask::new(),
            required_capabilities: 0,
            high_frequency: false,
            latency_sensitive: false,
        }
    }
}

impl Default for AffinityHints {
    fn default() -> Self {
        Self::new()
    }
}

impl InterruptAffinityManager {
    /// Create a new interrupt affinity manager
    pub fn new(total_cpus: u32) -> Self {
        let topology = CpuTopology::new(total_cpus);
        let mut cpu_stats = Vec::new();

        for _ in 0..total_cpus {
            cpu_stats.push(SpinLock::new(CpuIrqStats::new()));
        }

        let online_mask = if total_cpus >= 64 {
            CpuMask::all()
        } else {
            CpuMask::from_bits((1u64 << total_cpus) - 1)
        };

        Self {
            topology,
            online_cpus: SpinLock::new(online_mask),
            active_cpus: SpinLock::new(online_mask),
            cpu_stats,
            strategy: AtomicU32::new(LoadBalanceStrategy::LeastLoaded as u32),
            rr_counter: AtomicU32::new(0),
            irq_affinity_cache: SpinLock::new(vec![None; 1024]),
        }
    }

    /// Initialize the affinity manager
    pub fn init(&self) -> Result<()> {
        crate::info!("Initializing interrupt affinity manager for {} CPUs", self.topology.total_cpus);
        Ok(())
    }

    /// Get CPU topology
    pub fn topology(&self) -> &CpuTopology {
        &self.topology
    }

    /// Set online CPUs
    pub fn set_online_cpus(&self, mask: CpuMask) -> Result<()> {
        *self.online_cpus.lock() = mask;
        Ok(())
    }

    /// Set active CPUs
    pub fn set_active_cpus(&self, mask: CpuMask) -> Result<()> {
        *self.active_cpus.lock() = mask;
        Ok(())
    }

    /// Get online CPUs
    pub fn get_online_cpus(&self) -> CpuMask {
        *self.online_cpus.lock()
    }

    /// Get active CPUs
    pub fn get_active_cpus(&self) -> CpuMask {
        *self.active_cpus.lock()
    }

    /// Check if a CPU is online
    pub fn is_cpu_online(&self, cpu: u32) -> bool {
        self.online_cpus.lock().contains(cpu)
    }

    /// Check if a CPU is active
    pub fn is_cpu_active(&self, cpu: u32) -> bool {
        self.active_cpus.lock().contains(cpu)
    }

    /// Set load balancing strategy
    pub fn set_strategy(&self, strategy: LoadBalanceStrategy) {
        self.strategy.store(strategy as u32, Ordering::Relaxed);
    }

    /// Get current load balancing strategy
    pub fn get_strategy(&self) -> LoadBalanceStrategy {
        match self.strategy.load(Ordering::Relaxed) {
            0 => LoadBalanceStrategy::None,
            1 => LoadBalanceStrategy::RoundRobin,
            2 => LoadBalanceStrategy::LeastLoaded,
            3 => LoadBalanceStrategy::PackageAware,
            4 => LoadBalanceStrategy::CoreAware,
            5 => LoadBalanceStrategy::NumaAware,
            _ => LoadBalanceStrategy::LeastLoaded,
        }
    }

    /// Calculate interrupt load for a CPU
    pub fn calculate_cpu_load(&self, cpu: u32) -> f64 {
        if cpu as usize >= self.cpu_stats.len() {
            return 0.0;
        }

        let stats = self.cpu_stats[cpu as usize].lock();
        stats.get_interrupt_rate()
    }

    /// Get the least loaded CPU from a mask
    pub fn get_least_loaded_cpu(&self, mask: &CpuMask) -> Option<u32> {
        let mut best_cpu = None;
        let mut best_load = f64::MAX;

        for cpu in mask.iter() {
            if self.is_cpu_active(cpu) {
                let load = self.calculate_cpu_load(cpu);
                if load < best_load {
                    best_load = load;
                    best_cpu = Some(cpu);
                }
            }
        }

        best_cpu
    }

    /// Select target CPU based on load balancing strategy
    pub fn select_target_cpu(&self, irq: IrqNumber, hints: &AffinityHints) -> Option<u32> {
        let strategy = self.get_strategy();
        let online_cpus = self.get_online_cpus();
        let active_cpus = self.get_active_cpus();

        // Apply hints
        let mut available = online_cpus.and(&active_cpus);
        available = available.and(&hints.avoid_cpus.not());

        if !hints.preferred_cpus.is_empty() {
            available = available.and(&hints.preferred_cpus);
        }

        if available.is_empty() {
            return None;
        }

        match strategy {
            LoadBalanceStrategy::None => available.first(),
            LoadBalanceStrategy::RoundRobin => {
                let counter = self.rr_counter.fetch_add(1, Ordering::Relaxed);
                let cpus: Vec<u32> = available.iter().collect();
                if cpus.is_empty() {
                    None
                } else {
                    Some(cpus[(counter as usize) % cpus.len()])
                }
            }
            LoadBalanceStrategy::LeastLoaded => {
                self.get_least_loaded_cpu(&available)
            }
            LoadBalanceStrategy::PackageAware => {
                // Prefer CPUs in the same package for cache locality
                if let Some(current_cpu) = crate::arch::cpu::get_current_cpu_id() {
                    let package_cpus = self.topology.get_package_cpus(current_cpu);
                    let package_available = available.and(&package_cpus);

                    if !package_available.is_empty() {
                        self.get_least_loaded_cpu(&package_available)
                            .or_else(|| package_available.first())
                    } else {
                        self.get_least_loaded_cpu(&available)
                            .or_else(|| available.first())
                    }
                } else {
                    self.get_least_loaded_cpu(&available)
                        .or_else(|| available.first())
                }
            }
            LoadBalanceStrategy::CoreAware => {
                // Prefer different cores for better parallelism
                if let Some(current_cpu) = crate::arch::cpu::get_current_cpu_id() {
                    let current_core_cpus = self.topology.get_core_cpus(current_cpu);
                    let different_cores = available.and(&current_core_cpus.not());

                    if !different_cores.is_empty() {
                        self.get_least_loaded_cpu(&different_cores)
                            .or_else(|| different_cores.first())
                    } else {
                        self.get_least_loaded_cpu(&available)
                            .or_else(|| available.first())
                    }
                } else {
                    self.get_least_loaded_cpu(&available)
                        .or_else(|| available.first())
                }
            }
            LoadBalanceStrategy::NumaAware => {
                // Similar to package-aware for now
                self.select_target_cpu(irq, hints)
            }
        }
    }

    /// Set interrupt affinity
    pub fn set_irq_affinity(&self, irq: IrqNumber, mask: CpuMask, force: bool) -> Result<()> {
        if mask.is_empty() {
            return Err(Error::InvalidArgument);
        }

        // Check if any CPUs in the mask are online/active
        let online_cpus = self.get_online_cpus();
        let active_cpus = self.get_active_cpus();
        let available = mask.and(&online_cpus).and(&active_cpus);

        if available.is_empty() && !force {
            return Err(Error::InvalidState);
        }

        // Update cache
        let mut cache = self.irq_affinity_cache.lock();
        if (irq as usize) < cache.len() {
            cache[irq as usize] = Some(mask);
        }

        crate::debug!("Set affinity for IRQ {} to mask {:#x}", irq, mask.bits());
        Ok(())
    }

    /// Get interrupt affinity
    pub fn get_irq_affinity(&self, irq: IrqNumber) -> Option<CpuMask> {
        let cache = self.irq_affinity_cache.lock();
        if (irq as usize) < cache.len() {
            cache[irq as usize]
        } else {
            None
        }
    }

    /// Calculate optimal affinity for an interrupt
    pub fn calculate_optimal_affinity(&self, descriptor: &InterruptDescriptor) -> CpuMask {
        let mut hints = AffinityHints::new();

        // Set hints based on interrupt characteristics
        match descriptor.irq_type {
            IrqType::Hardware => {
                if descriptor.priority == Priority::Highest {
                    hints.latency_sensitive = true;
                    hints.high_frequency = true;
                }
            }
            IrqType::Ipi => {
                hints.high_frequency = true;
            }
            IrqType::Software => {
                // Software interrupts can go anywhere
            }
        }

        // Select target CPU
        if let Some(target_cpu) = self.select_target_cpu(descriptor.irq, &hints) {
            CpuMask::from_cpu(target_cpu)
        } else {
            // Fallback to all online CPUs
            self.get_online_cpus()
        }
    }

    /// Record interrupt statistics
    pub fn record_interrupt(&self, cpu: u32, irq: IrqNumber, descriptor: &InterruptDescriptor, processing_time_ns: u32) {
        if cpu as usize >= self.cpu_stats.len() {
            return;
        }

        let stats = &self.cpu_stats[cpu as usize];
        stats.lock().record_interrupt(descriptor.irq_type, descriptor.priority, processing_time_ns);
    }

    /// Record spurious interrupt
    pub fn record_spurious_interrupt(&self, cpu: u32) {
        if cpu as usize >= self.cpu_stats.len() {
            return;
        }

        let stats = &self.cpu_stats[cpu as usize];
        stats.lock().record_spurious();
    }

    /// Get CPU statistics
    pub fn get_cpu_stats(&self, cpu: u32) -> Option<CpuIrqStats> {
        if cpu as usize >= self.cpu_stats.len() {
            None
        } else {
            Some(CpuIrqStats {
                total_interrupts: AtomicU64::new(self.cpu_stats[cpu as usize].lock().total_interrupts.load(Ordering::Relaxed)),
                priority_counts: [
                    AtomicU64::new(self.cpu_stats[cpu as usize].lock().priority_counts[0].load(Ordering::Relaxed)),
                    AtomicU64::new(self.cpu_stats[cpu as usize].lock().priority_counts[1].load(Ordering::Relaxed)),
                    AtomicU64::new(self.cpu_stats[cpu as usize].lock().priority_counts[2].load(Ordering::Relaxed)),
                    AtomicU64::new(self.cpu_stats[cpu as usize].lock().priority_counts[3].load(Ordering::Relaxed)),
                    AtomicU64::new(self.cpu_stats[cpu as usize].lock().priority_counts[4].load(Ordering::Relaxed)),
                ],
                type_counts: [
                    AtomicU64::new(self.cpu_stats[cpu as usize].lock().type_counts[0].load(Ordering::Relaxed)),
                    AtomicU64::new(self.cpu_stats[cpu as usize].lock().type_counts[1].load(Ordering::Relaxed)),
                    AtomicU64::new(self.cpu_stats[cpu as usize].lock().type_counts[2].load(Ordering::Relaxed)),
                ],
                last_interrupt: AtomicU64::new(self.cpu_stats[cpu as usize].lock().last_interrupt.load(Ordering::Relaxed)),
                avg_processing_time: AtomicU32::new(self.cpu_stats[cpu as usize].lock().avg_processing_time.load(Ordering::Relaxed)),
                spurious_interrupts: AtomicU64::new(self.cpu_stats[cpu as usize].lock().spurious_interrupts.load(Ordering::Relaxed)),
            })
        }
    }

    /// Migrate interrupt to different CPU
    pub fn migrate_interrupt(&self, irq: IrqNumber, new_cpu: u32) -> Result<()> {
        if !self.is_cpu_active(new_cpu) {
            return Err(Error::InvalidState);
        }

        let new_mask = CpuMask::from_cpu(new_cpu);
        self.set_irq_affinity(irq, new_mask, false)
    }

    /// Balance all interrupts
    pub fn balance_interrupts(&self, descriptors: &[InterruptDescriptor]) -> Result<usize> {
        let strategy = self.get_strategy();
        if strategy == LoadBalanceStrategy::None {
            return Ok(0);
        }

        let mut migrated = 0;

        for descriptor in descriptors {
            let current_affinity = self.get_irq_affinity(descriptor.irq)
                .unwrap_or_else(|| CpuMask::all());

            let optimal_affinity = self.calculate_optimal_affinity(descriptor);

            if current_affinity != optimal_affinity {
                if self.set_irq_affinity(descriptor.irq, optimal_affinity, false).is_ok() {
                    migrated += 1;
                }
            }
        }

        crate::info!("Interrupt balancing completed: {} interrupts migrated", migrated);
        Ok(migrated)
    }

    /// Get system-wide interrupt statistics
    pub fn get_system_stats(&self) -> SystemIrqStats {
        let mut total_stats = SystemIrqStats::default();

        for (cpu_id, stats_lock) in self.cpu_stats.iter().enumerate() {
            let stats = stats_lock.lock();
            total_stats.total_interrupts += stats.total_interrupts.load(Ordering::Relaxed);
            total_stats.spurious_interrupts += stats.spurious_interrupts.load(Ordering::Relaxed);
            total_stats.active_cpus += 1;

            let load = stats.get_interrupt_rate();
            if load > total_stats.max_cpu_load {
                total_stats.max_cpu_load = load;
            }
            total_stats.total_cpu_load += load;
        }

        if total_stats.active_cpus > 0 {
            total_stats.avg_cpu_load = total_stats.total_cpu_load / total_stats.active_cpus as f64;
        }

        total_stats
    }
}

/// System-wide interrupt statistics
#[derive(Debug, Default)]
pub struct SystemIrqStats {
    /// Total interrupts across all CPUs
    pub total_interrupts: u64,
    /// Total spurious interrupts
    pub spurious_interrupts: u64,
    /// Number of active CPUs
    pub active_cpus: u32,
    /// Average CPU load (interrupts per second)
    pub avg_cpu_load: f64,
    /// Maximum CPU load
    pub max_cpu_load: f64,
    /// Total CPU load
    pub total_cpu_load: f64,
}

/// Global interrupt affinity manager
static mut AFFINITY_MANAGER: Option<InterruptAffinityManager> = None;
static AFFINITY_MANAGER_INIT: SpinLock<bool> = SpinLock::new(false);

/// Initialize the global interrupt affinity manager
pub fn init(total_cpus: u32) -> Result<()> {
    let mut init_guard = AFFINITY_MANAGER_INIT.lock();

    if *init_guard {
        return Ok(());
    }

    let manager = InterruptAffinityManager::new(total_cpus);
    manager.init()?;

    unsafe {
        AFFINITY_MANAGER = Some(manager);
    }

    *init_guard = true;
    crate::info!("Global interrupt affinity manager initialized");
    Ok(())
}

/// Get the global interrupt affinity manager
pub fn get() -> Option<&'static InterruptAffinityManager> {
    unsafe { AFFINITY_MANAGER.as_ref() }
}

/// Get the global interrupt affinity manager (panic if not initialized)
pub fn get_expect() -> &'static InterruptAffinityManager {
    get().expect("Interrupt affinity manager not initialized")
}