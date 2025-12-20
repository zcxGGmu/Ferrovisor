//! RISC-V SMP Module
//!
//! This module provides symmetric multiprocessing support including:
//! - Multi-core initialization
//! - Inter-processor interrupts
//! - CPU hotplug
//! - Load balancing

pub mod boot;
pub mod ipi;
pub mod sbi;
pub mod scheduler;

pub use boot::*;
pub use ipi::*;
pub use scheduler::*;

use crate::arch::riscv64::*;

/// SMP configuration
#[derive(Debug, Clone)]
pub struct SmpConfig {
    /// Maximum number of CPUs
    pub max_cpus: usize,
    /// Number of CPUs to start at boot
    pub boot_cpus: usize,
    /// Enable CPU hotplug
    pub enable_hotplug: bool,
    /// Load balancing algorithm
    pub load_balancer: LoadBalancerType,
}

impl Default for SmpConfig {
    fn default() -> Self {
        Self {
            max_cpus: MAX_CPUS,
            boot_cpus: 1,
            enable_hotplug: true,
            load_balancer: LoadBalancerType::RoundRobin,
        }
    }
}

/// Load balancer types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadBalancerType {
    /// No load balancing
    None,
    /// Round-robin scheduling
    RoundRobin,
    /// Least loaded
    LeastLoaded,
    /// CPU affinity based
    Affinity,
}

/// SMP state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmpState {
    /// Not initialized
    Uninitialized,
    /// Initialized but not started
    Initialized,
    /// Running
    Running,
    /// Stopped
    Stopped,
}

/// Global SMP state
static mut SMP_STATE: SmpState = SmpState::Uninitialized;

/// SMP configuration
static mut SMP_CONFIG: Option<SmpConfig> = None;

/// Number of online CPUs
static mut ONLINE_CPUS: AtomicUsize = AtomicUsize::new(0);

/// CPU mask of online CPUs
static mut ONLINE_CPU_MASK: AtomicUsize = AtomicUsize::new(0);

/// Load balancer instance
static mut LOAD_BALANCER: Option<Box<dyn LoadBalancer>> = None;

/// Initialize SMP subsystem
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing RISC-V SMP subsystem");

    // Set default configuration
    let config = SmpConfig::default();
    init_with_config(config)?;

    log::info!("RISC-V SMP subsystem initialized successfully");
    Ok(())
}

/// Initialize SMP subsystem with configuration
pub fn init_with_config(config: SmpConfig) -> Result<(), &'static str> {
    log::info!("Initializing SMP with config: max_cpus={}, boot_cpus={}",
             config.max_cpus, config.boot_cpus);

    // Store configuration
    unsafe {
        SMP_CONFIG = Some(config.clone());
    }

    // Initialize SBI for SMP operations
    sbi::init()?;

    // Initialize boot system
    boot::init_boot_system()?;

    // Initialize IPI subsystem
    ipi::init()?;

    // Initialize load balancer
    init_load_balancer(config.load_balancer)?;

    // Start secondary CPUs if configured
    if config.boot_cpus > 1 {
        start_secondary_cpus(config.boot_cpus)?;
    }

    // Update SMP state
    unsafe {
        SMP_STATE = SmpState::Running;
    }

    log::info!("SMP initialization complete");
    Ok(())
}

/// Initialize load balancer
fn init_load_balancer(lb_type: LoadBalancerType) -> Result<(), &'static str> {
    let balancer: Box<dyn LoadBalancer> = match lb_type {
        LoadBalancerType::None => Box::new(NoLoadBalancer::new()),
        LoadBalancerType::RoundRobin => Box::new(RoundRobinLoadBalancer::new()),
        LoadBalancerType::LeastLoaded => Box::new(LeastLoadedLoadBalancer::new()),
        LoadBalancerType::Affinity => Box::new(AffinityLoadBalancer::new()),
    };

    unsafe {
        LOAD_BALANCER = Some(balancer);
    }

    log::debug!("Load balancer initialized: {:?}", lb_type);
    Ok(())
}

/// Start secondary CPUs
fn start_secondary_cpus(num_cpus: usize) -> Result<(), &'static str> {
    log::info!("Starting {} secondary CPUs", num_cpus - 1);

    // Configure boot for secondary CPUs
    let boot_config = boot::BootConfig {
        entry_point: 0x80000000, // This would be set to actual entry point
        stack_top: 0x90000000,
        dtb_address: 0x41000000,
        boot_args: 0,
    };

    boot::configure_secondary_boot(boot_config)?;

    // Start each secondary CPU
    let mut started = 0;
    for cpu_id in 1..num_cpus.min(MAX_CPUS) {
        match boot::start_secondary_cpu(cpu_id) {
            Ok(_) => {
                started += 1;
                // Mark CPU as online
                mark_cpu_online(cpu_id);
            }
            Err(e) => {
                log::error!("Failed to start CPU {}: {}", cpu_id, e);
            }
        }
    }

    // Wait for secondary CPUs to be ready
    let ready = boot::wait_for_all_cpus_ready(5000)?; // 5 second timeout

    log::info!("Started {} secondary CPUs, {} are ready", started, ready);
    Ok(())
}

/// Get SMP configuration
pub fn get_config() -> Option<SmpConfig> {
    unsafe { SMP_CONFIG.clone() }
}

/// Get SMP state
pub fn get_state() -> SmpState {
    unsafe { SMP_STATE }
}

/// Get number of online CPUs
pub fn num_online_cpus() -> usize {
    unsafe { ONLINE_CPUS.load(Ordering::SeqCst) }
}

/// Get online CPU mask
pub fn get_online_cpu_mask() -> usize {
    unsafe { ONLINE_CPU_MASK.load(Ordering::SeqCst) }
}

/// Check if a CPU is online
pub fn is_cpu_online(cpu_id: usize) -> bool {
    if cpu_id >= MAX_CPUS {
        return false;
    }

    let mask = get_online_cpu_mask();
    (mask & (1 << cpu_id)) != 0
}

/// Mark a CPU as online
pub fn mark_cpu_online(cpu_id: usize) {
    if cpu_id < MAX_CPUS {
        unsafe {
            ONLINE_CPUS.fetch_add(1, Ordering::SeqCst);
            ONLINE_CPU_MASK.fetch_or(1 << cpu_id, Ordering::SeqCst);
        }
        log::debug!("CPU {} marked as online", cpu_id);
    }
}

/// Mark a CPU as offline
pub fn mark_cpu_offline(cpu_id: usize) {
    if cpu_id < MAX_CPUS {
        unsafe {
            ONLINE_CPUS.fetch_sub(1, Ordering::SeqCst);
            ONLINE_CPU_MASK.fetch_and(!(1 << cpu_id), Ordering::SeqCst);
        }
        log::debug!("CPU {} marked as offline", cpu_id);
    }
}

/// Send IPI to target CPU
pub fn send_ipi(cpu_id: usize, ipi_type: u32) -> Result<(), &'static str> {
    if let Ok(ipi_type_enum) = ipi::IpiType::try_from(ipi_type) {
        ipi::send_ipi(cpu_id, ipi_type_enum, 0)
    } else {
        Err("Invalid IPI type")
    }
}

/// Send IPI to multiple CPUs
pub fn send_ipi_to_many(cpu_ids: &[usize], ipi_type: u32) -> Result<(), &'static str> {
    if let Ok(ipi_type_enum) = ipi::IpiType::try_from(ipi_type) {
        ipi::send_ipi_to_many(cpu_ids, ipi_type_enum, 0)
    } else {
        Err("Invalid IPI type")
    }
}

/// Broadcast IPI to all online CPUs
pub fn broadcast_ipi(ipi_type: u32, exclude_self: bool) -> Result<(), &'static str> {
    if let Ok(ipi_type_enum) = ipi::IpiType::try_from(ipi_type) {
        ipi::broadcast_ipi(ipi_type_enum, 0, exclude_self)
    } else {
        Err("Invalid IPI type")
    }
}

/// Select CPU for task scheduling
pub fn select_cpu(task_affinity: Option<usize>) -> Option<usize> {
    if let Some(ref balancer) = unsafe { LOAD_BALANCER.as_ref() } {
        balancer.select_cpu(task_affinity)
    } else {
        // No load balancer, return current CPU
        Some(crate::arch::riscv64::cpu::current_cpu_id())
    }
}

/// Update CPU load statistics
pub fn update_cpu_load(cpu_id: usize, load: f64) {
    if let Some(ref balancer) = unsafe { LOAD_BALANCER.as_ref() } {
        balancer.update_load(cpu_id, load);
    }
}

/// Get CPU load statistics
pub fn get_cpu_load(cpu_id: usize) -> Option<f64> {
    if let Some(ref balancer) = unsafe { LOAD_BALANCER.as_ref() } {
        balancer.get_load(cpu_id)
    } else {
        None
    }
}

/// Load balancer trait
pub trait LoadBalancer {
    /// Select a CPU for a task
    fn select_cpu(&self, affinity: Option<usize>) -> Option<usize>;

    /// Update CPU load
    fn update_load(&self, cpu_id: usize, load: f64);

    /// Get CPU load
    fn get_load(&self, cpu_id: usize) -> Option<f64>;
}

/// No load balancer (always use current CPU)
pub struct NoLoadBalancer {
    _private: (),
}

impl NoLoadBalancer {
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl LoadBalancer for NoLoadBalancer {
    fn select_cpu(&self, _affinity: Option<usize>) -> Option<usize> {
        Some(crate::arch::riscv64::cpu::current_cpu_id())
    }

    fn update_load(&self, _cpu_id: usize, _load: f64) {
        // No load tracking
    }

    fn get_load(&self, _cpu_id: usize) -> Option<f64> {
        None
    }
}

/// Round-robin load balancer
pub struct RoundRobinLoadBalancer {
    next_cpu: AtomicUsize,
}

impl RoundRobinLoadBalancer {
    pub fn new() -> Self {
        Self {
            next_cpu: AtomicUsize::new(0),
        }
    }
}

impl LoadBalancer for RoundRobinLoadBalancer {
    fn select_cpu(&self, affinity: Option<usize>) -> Option<usize> {
        // If affinity is specified and CPU is online, use it
        if let Some(cpu_id) = affinity {
            if is_cpu_online(cpu_id) {
                return Some(cpu_id);
            }
        }

        // Otherwise use round-robin
        let num_cpus = num_online_cpus();
        if num_cpus == 0 {
            return None;
        }

        let current = self.next_cpu.fetch_add(1, Ordering::SeqCst);
        let cpu_id = current % num_cpus;

        // Find the actual CPU ID at this position
        let mask = get_online_cpu_mask();
        let mut pos = 0;
        for i in 0..MAX_CPUS {
            if (mask & (1 << i)) != 0 {
                if pos == cpu_id {
                    return Some(i);
                }
                pos += 1;
            }
        }

        None
    }

    fn update_load(&self, _cpu_id: usize, _load: f64) {
        // Round-robin doesn't track load
    }

    fn get_load(&self, _cpu_id: usize) -> Option<f64> {
        None
    }
}

/// Least loaded load balancer
pub struct LeastLoadedLoadBalancer {
    cpu_loads: [AtomicF64; MAX_CPUS],
}

impl LeastLoadedLoadBalancer {
    pub fn new() -> Self {
        Self {
            cpu_loads: [const { AtomicF64::new(0.0) }; MAX_CPUS],
        }
    }
}

impl LoadBalancer for LeastLoadedLoadBalancer {
    fn select_cpu(&self, affinity: Option<usize>) -> Option<usize> {
        // If affinity is specified and CPU is online, use it
        if let Some(cpu_id) = affinity {
            if is_cpu_online(cpu_id) {
                return Some(cpu_id);
            }
        }

        // Find CPU with minimum load
        let mask = get_online_cpu_mask();
        let mut min_load = 1.0;
        let mut selected_cpu = None;

        for i in 0..MAX_CPUS {
            if (mask & (1 << i)) != 0 {
                let load = self.cpu_loads[i].load(Ordering::SeqCst);
                if selected_cpu.is_none() || load < min_load {
                    min_load = load;
                    selected_cpu = Some(i);
                }
            }
        }

        selected_cpu
    }

    fn update_load(&self, cpu_id: usize, load: f64) {
        if cpu_id < MAX_CPUS {
            self.cpu_loads[cpu_id].store(load, Ordering::SeqCst);
        }
    }

    fn get_load(&self, cpu_id: usize) -> Option<f64> {
        if cpu_id < MAX_CPUS {
            Some(self.cpu_loads[cpu_id].load(Ordering::SeqCst))
        } else {
            None
        }
    }
}

/// Affinity-based load balancer
pub struct AffinityLoadBalancer {
    _private: (),
}

impl AffinityLoadBalancer {
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl LoadBalancer for AffinityLoadBalancer {
    fn select_cpu(&self, affinity: Option<usize>) -> Option<usize> {
        // Always respect affinity if specified
        if let Some(cpu_id) = affinity {
            if is_cpu_online(cpu_id) {
                return Some(cpu_id);
            }
        }

        // Fall back to round-robin if no affinity
        let balancer = RoundRobinLoadBalancer::new();
        balancer.select_cpu(None)
    }

    fn update_load(&self, _cpu_id: usize, _load: f64) {
        // Affinity doesn't track load
    }

    fn get_load(&self, _cpu_id: usize) -> Option<f64> {
        None
    }
}

// Fallback for AtomicF64 if not available
use core::sync::atomic::AtomicU64 as AtomicF64;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smp_config() {
        let config = SmpConfig::default();
        assert_eq!(config.max_cpus, MAX_CPUS);
        assert_eq!(config.boot_cpus, 1);
        assert!(config.enable_hotplug);
        assert_eq!(config.load_balancer, LoadBalancerType::RoundRobin);
    }

    #[test]
    fn test_cpu_online() {
        let current_cpu = crate::arch::riscv64::cpu::current_cpu_id();

        // Primary CPU should be online after initialization
        assert!(is_cpu_online(current_cpu));

        let num_cpus = num_online_cpus();
        assert!(num_cpus >= 1);
    }

    #[test]
    fn test_load_balancers() {
        let no_lb = NoLoadBalancer::new();
        let rr_lb = RoundRobinLoadBalancer::new();
        let ll_lb = LeastLoadedLoadBalancer::new();
        let aff_lb = AffinityLoadBalancer::new();

        // Test no load balancer
        let cpu = no_lb.select_cpu(None);
        assert!(cpu.is_some());

        // Test round-robin
        let cpu = rr_lb.select_cpu(None);
        assert!(cpu.is_some());

        // Test least loaded
        let cpu = ll_lb.select_cpu(None);
        assert!(cpu.is_some());

        // Test affinity
        let cpu = aff_lb.select_cpu(Some(0));
        assert!(cpu.is_some());
    }
}