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
use core::sync::atomic::{AtomicUsize, AtomicU32, AtomicU64, Ordering};
use alloc::vec::Vec;

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

/// Advanced multi-core boot manager
pub struct MultiCoreBootManager {
    /// Boot configuration
    config: SmpConfig,
    /// Boot statistics
    stats: BootStatistics,
    /// Boot state tracking
    boot_states: [AtomicU32; crate::MAX_CPUS],
    /// Performance monitoring
    performance: BootPerformanceMonitor,
}

/// Boot statistics
#[derive(Debug, Default)]
pub struct BootStatistics {
    /// Total boot attempts
    pub total_attempts: AtomicUsize,
    /// Successful boots
    pub successful_boots: AtomicUsize,
    /// Failed boots
    pub failed_boots: AtomicUsize,
    /// Total boot time in cycles
    pub total_boot_time: AtomicU64,
    /// Average boot time per CPU
    pub avg_boot_time: AtomicU64,
    /// Peak concurrent boots
    pub peak_concurrent_boots: AtomicUsize,
}

/// Boot performance monitor
#[derive(Debug)]
pub struct BootPerformanceMonitor {
    /// Boot times per CPU
    boot_times: [AtomicU64; crate::MAX_CPUS],
    /// Boot start times per CPU
    boot_start_times: [AtomicU64; crate::MAX_CPUS],
    /// CPU readiness times
    readiness_times: [AtomicU64; crate::MAX_CPUS],
    /// Last boot timestamp
    last_boot_timestamp: AtomicU64,
}

impl MultiCoreBootManager {
    /// Create new multi-core boot manager
    pub fn new(config: SmpConfig) -> Self {
        Self {
            config,
            stats: BootStatistics::default(),
            boot_states: [const { AtomicU32::new(0) }; crate::MAX_CPUS],
            performance: BootPerformanceMonitor {
                boot_times: [const { AtomicU64::new(0) }; crate::MAX_CPUS],
                boot_start_times: [const { AtomicU64::new(0) }; crate::MAX_CPUS],
                readiness_times: [const { AtomicU64::new(0) }; crate::MAX_CPUS],
                last_boot_timestamp: AtomicU64::new(0),
            },
        }
    }

    /// Initialize multi-core boot system
    pub fn initialize(&mut self) -> Result<(), &'static str> {
        log::info!("Initializing multi-core boot manager");

        // Initialize SBI interface
        crate::arch::riscv64::smp::sbi::init()?;

        // Initialize boot system
        crate::arch::riscv64::smp::boot::init_boot_system()?;

        // Initialize IPI subsystem
        crate::arch::riscv64::smp::ipi::init()?;

        // Initialize primary CPU
        self.initialize_primary_cpu()?;

        // Initialize load balancer
        init_load_balancer(self.config.load_balancer)?;

        log::info!("Multi-core boot manager initialized");
        Ok(())
    }

    /// Initialize primary CPU
    fn initialize_primary_cpu(&mut self) -> Result<(), &'static str> {
        let cpu_id = 0;

        // Mark primary CPU as booting
        self.set_cpu_state(cpu_id, CpuState::Booting);

        let start_time = crate::arch::riscv64::cpu::csr::TIME::read();

        // Initialize primary CPU subsystems
        crate::arch::riscv64::cpu::state::init()?;
        crate::arch::riscv64::mmu::init()?;
        crate::arch::riscv64::interrupt::init()?;

        // Check for virtualization support
        if crate::arch::riscv64::virtualization::has_h_extension() {
            crate::arch::riscv64::virtualization::init()?;
        }

        let end_time = crate::arch::riscv64::cpu::csr::TIME::read();
        let boot_time = end_time.wrapping_sub(start_time);

        // Update statistics
        self.performance.boot_times[cpu_id].store(boot_time, Ordering::SeqCst);
        self.stats.total_attempts.fetch_add(1, Ordering::SeqCst);
        self.stats.successful_boots.fetch_add(1, Ordering::SeqCst);
        self.stats.total_boot_time.fetch_add(boot_time, Ordering::SeqCst);

        // Mark primary CPU as ready
        self.set_cpu_state(cpu_id, CpuState::Running);
        mark_cpu_online(cpu_id);

        log::info!("Primary CPU {} initialized in {} cycles", cpu_id, boot_time);
        Ok(())
    }

    /// Start secondary CPUs
    pub fn start_secondary_cpus(&mut self) -> Result<usize, &'static str> {
        if self.config.boot_cpus <= 1 {
            return Ok(0);
        }

        log::info!("Starting {} secondary CPUs", self.config.boot_cpus - 1);

        // Configure secondary CPU boot
        let boot_config = crate::arch::riscv64::smp::boot::BootConfig {
            entry_point: 0x80000000, // Would be set to actual entry point
            stack_top: 0x90000000,
            dtb_address: 0x41000000,
            boot_args: 0,
        };

        crate::arch::riscv64::smp::boot::configure_secondary_boot(boot_config)?;

        let start_time = crate::arch::riscv64::cpu::csr::TIME::read();
        let mut started_count = 0;
        let mut concurrent_boots = 0;

        // Start secondary CPUs in parallel if possible
        for cpu_id in 1..self.config.boot_cpus.min(crate::MAX_CPUS) {
            // Mark CPU as booting
            self.set_cpu_state(cpu_id, CpuState::Booting);
            self.performance.boot_start_times[cpu_id].store(start_time, Ordering::SeqCst);
            concurrent_boots += 1;

            // Start the CPU
            match crate::arch::riscv64::smp::boot::start_secondary_cpu(cpu_id) {
                Ok(_) => {
                    started_count += 1;
                }
                Err(e) => {
                    log::error!("Failed to start CPU {}: {}", cpu_id, e);
                    self.set_cpu_state(cpu_id, CpuState::Failed);
                    self.stats.failed_boots.fetch_add(1, Ordering::SeqCst);
                }
            }
        }

        // Update peak concurrent boots
        let current_peak = self.stats.peak_concurrent_boots.load(Ordering::SeqCst);
        if concurrent_boots > current_peak {
            self.stats.peak_concurrent_boots.store(concurrent_boots, Ordering::SeqCst);
        }

        log::info!("Started {} secondary CPUs", started_count);
        Ok(started_count)
    }

    /// Wait for all CPUs to be ready
    pub fn wait_for_all_cpus_ready(&mut self, timeout_ms: u64) -> Result<usize, &'static str> {
        let start_time = crate::arch::riscv64::cpu::csr::TIME::read();
        let mut ready_count = 0;

        log::info!("Waiting for CPUs to be ready (timeout: {}ms)", timeout_ms);

        for cpu_id in 0..self.config.boot_cpus.min(crate::MAX_CPUS) {
            if cpu_id == 0 {
                // Primary CPU is already ready
                ready_count += 1;
                continue;
            }

            match crate::arch::riscv64::smp::boot::wait_for_cpu_ready(cpu_id, timeout_ms) {
                Ok(_) => {
                    ready_count += 1;
                    let end_time = crate::arch::riscv64::cpu::csr::TIME::read();
                    let boot_time = end_time.wrapping_sub(
                        self.performance.boot_start_times[cpu_id].load(Ordering::SeqCst)
                    );
                    let ready_time = end_time.wrapping_sub(start_time);

                    self.performance.boot_times[cpu_id].store(boot_time, Ordering::SeqCst);
                    self.performance.readiness_times[cpu_id].store(ready_time, Ordering::SeqCst);
                    self.set_cpu_state(cpu_id, CpuState::Running);

                    log::debug!("CPU {} ready after {} cycles (ready in {} cycles)",
                               cpu_id, boot_time, ready_time);
                }
                Err(e) => {
                    log::warn!("CPU {} failed to become ready: {}", cpu_id, e);
                    self.set_cpu_state(cpu_id, CpuState::Failed);
                }
            }
        }

        // Update statistics
        let total_ready_time = crate::arch::riscv64::cpu::csr::TIME::read().wrapping_sub(start_time);
        self.performance.last_boot_timestamp.store(total_ready_time, Ordering::SeqCst);

        if ready_count == self.config.boot_cpus {
            // Calculate average boot time
            let total_time = self.stats.total_boot_time.load(Ordering::SeqCst);
            self.stats.avg_boot_time.store(total_time / ready_count as u64, Ordering::SeqCst);
        }

        log::info!("{} CPUs ready out of {} requested", ready_count, self.config.boot_cpus);
        Ok(ready_count)
    }

    /// Perform complete multi-core boot sequence
    pub fn boot_all_cpus(&mut self) -> Result<usize, &'static str> {
        log::info!("Starting multi-core boot sequence for {} CPUs", self.config.boot_cpus);

        let start_time = crate::arch::riscv64::cpu::csr::TIME::read();

        // Initialize primary CPU
        self.initialize_primary_cpu()?;

        // Start secondary CPUs
        let started = self.start_secondary_cpus()?;

        // Wait for all CPUs to be ready
        let ready = self.wait_for_all_cpus_ready(5000)?; // 5 second timeout

        let total_time = crate::arch::riscv64::cpu::csr::TIME::read().wrapping_sub(start_time);

        log::info!("Multi-core boot completed: {}/{} CPUs ready in {} cycles",
                    ready, self.config.boot_cpus, total_time);

        // Update SMP state
        unsafe {
            SMP_STATE = SmpState::Running;
        }

        Ok(ready)
    }

    /// Perform dynamic CPU hotplug
    pub fn hotplug_cpu(&mut self, cpu_id: usize, operation: crate::arch::riscv64::smp::boot::hotplug::HotplugOp) ->
        Result<crate::arch::riscv64::smp::boot::hotplug::HotplugRequest, &'static str> {

        match operation {
            crate::arch::riscv64::smp::boot::hotplug::HotplugOp::Add => {
                let config = crate::arch::riscv64::smp::boot::BootConfig::default();
                crate::arch::riscv64::smp::boot::hotplug::cpu_add(cpu_id, config)
            }
            crate::arch::riscv64::smp::boot::hotplug::HotplugOp::Remove => {
                crate::arch::riscv64::smp::boot::hotplug::cpu_remove(cpu_id)
            }
            crate::arch::riscv64::smp::boot::hotplug::HotplugOp::Reset => {
                crate::arch::riscv64::smp::boot::hotplug::cpu_reset(cpu_id)
            }
            crate::arch::riscv64::smp::boot::hotplug::HotplugOp::Suspend => {
                crate::arch::riscv64::smp::boot::hotplug::cpu_suspend(cpu_id)
            }
            crate::arch::riscv64::smp::boot::hotplug::HotplugOp::Resume => {
                crate::arch::riscv64::smp::boot::hotplug::cpu_resume(cpu_id)
            }
        }
    }

    /// Get comprehensive boot statistics
    pub fn get_boot_statistics(&self) -> BootStatisticsReport {
        let total_attempts = self.stats.total_attempts.load(Ordering::SeqCst);
        let successful = self.stats.successful_boots.load(Ordering::SeqCst);
        let failed = self.stats.failed_boots.load(Ordering::SeqCst);
        let total_time = self.stats.total_boot_time.load(Ordering::SeqCst);
        let avg_time = self.stats.avg_boot_time.load(Ordering::SeqCst);
        let peak_concurrent = self.stats.peak_concurrent_boots.load(Ordering::SeqCst);

        let success_rate = if total_attempts > 0 {
            (successful as f64 / total_attempts as f64) * 100.0
        } else {
            0.0
        };

        let mut per_cpu_stats = Vec::new();
        for cpu_id in 0..crate::MAX_CPUS {
            let boot_time = self.performance.boot_times[cpu_id].load(Ordering::SeqCst);
            let ready_time = self.performance.readiness_times[cpu_id].load(Ordering::SeqCst);
            let state = self.boot_states[cpu_id].load(Ordering::SeqCst);

            if boot_time > 0 || state != 0 {
                per_cpu_stats.push(PerCpuBootStats {
                    cpu_id,
                    boot_time,
                    ready_time,
                    state: CpuState::from(state),
                });
            }
        }

        BootStatisticsReport {
            total_attempts,
            successful_boots: successful,
            failed_boots: failed,
            success_rate,
            total_boot_time: total_time,
            avg_boot_time: avg_time,
            peak_concurrent_boots: peak_concurrent,
            per_cpu_stats,
        }
    }

    /// Set CPU state
    fn set_cpu_state(&self, cpu_id: usize, state: CpuState) {
        self.boot_states[cpu_id].store(state as u32, Ordering::SeqCst);
    }

    /// Get boot configuration
    pub fn config(&self) -> &SmpConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: SmpConfig) {
        self.config = config;
    }
}

/// Per-CPU boot statistics
#[derive(Debug, Clone)]
pub struct PerCpuBootStats {
    /// CPU ID
    pub cpu_id: usize,
    /// Boot time in cycles
    pub boot_time: u64,
    /// Readiness time in cycles
    pub readiness_time: u64,
    /// Current state
    pub state: CpuState,
}

/// Comprehensive boot statistics report
#[derive(Debug, Clone)]
pub struct BootStatisticsReport {
    /// Total boot attempts
    pub total_attempts: usize,
    /// Successful boots
    pub successful_boots: usize,
    /// Failed boots
    pub failed_boots: usize,
    /// Success rate as percentage
    pub success_rate: f64,
    /// Total boot time in cycles
    pub total_boot_time: u64,
    /// Average boot time per CPU in cycles
    pub avg_boot_time: u64,
    /// Peak concurrent boots
    pub peak_concurrent_boots: usize,
    /// Per-CPU statistics
    pub per_cpu_stats: Vec<PerCpuBootStats>,
}

impl BootStatisticsReport {
    /// Print formatted boot statistics
    pub fn print(&self) {
        log::info!("=== Multi-Core Boot Statistics ===");
        log::info!("Total Attempts: {}", self.total_attempts);
        log::info!("Successful Boots: {}", self.successful_boots);
        log::info!("Failed Boots: {}", self.failed_boots);
        log::info!("Success Rate: {:.2}%", self.success_rate);
        log::info!("Total Boot Time: {} cycles", self.total_boot_time);
        log::info!("Average Boot Time: {} cycles/CPU", self.avg_boot_time);
        log::info!("Peak Concurrent Boots: {}", self.peak_concurrent_boots);

        log::info!("Per-CPU Statistics:");
        for stats in &self.per_cpu_stats {
            log::info!("  CPU {}: state={:?}, boot_time={}, ready_time={}",
                      stats.cpu_id, stats.state, stats.boot_time, stats.readiness_time);
        }
        log::info!("================================");
    }
}

/// CPU boot state for multi-core management
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuState {
    /// CPU not initialized
    Uninitialized = 0,
    /// CPU booting
    Booting = 1,
    /// CPU running
    Running = 2,
    /// CPU failed to boot
    Failed = 3,
    /// CPU suspended
    Suspended = 4,
    /// CPU offline
    Offline = 5,
}

impl From<u32> for CpuState {
    fn from(value: u32) -> Self {
        match value {
            0 => CpuState::Uninitialized,
            1 => CpuState::Booting,
            2 => CpuState::Running,
            3 => CpuState::Failed,
            4 => CpuState::Suspended,
            5 => CpuState::Offline,
            _ => CpuState::Uninitialized,
        }
    }
}

/// Global multi-core boot manager
static mut BOOT_MANAGER: Option<MultiCoreBootManager> = None;

/// Get global multi-core boot manager
pub fn get_boot_manager() -> Option<&'static MultiCoreBootManager> {
    unsafe { BOOT_MANAGER.as_ref() }
}

/// Get mutable global multi-core boot manager
pub fn get_boot_manager_mut() -> Option<&'static mut MultiCoreBootManager> {
    unsafe { BOOT_MANAGER.as_mut() }
}

/// Initialize multi-core boot system
pub fn init_multi_core_boot(config: SmpConfig) -> Result<(), &'static str> {
    log::info!("Initializing multi-core boot system with config: {:?}", config);

    let mut manager = MultiCoreBootManager::new(config);

    // Initialize the boot manager
    manager.initialize()?;

    // Store global reference
    unsafe {
        BOOT_MANAGER = Some(manager);
    }

    log::info!("Multi-core boot system initialized");
    Ok(())
}

/// Perform complete multi-core boot
pub fn boot_all_cpus() -> Result<usize, &'static str> {
    if let Some(manager) = get_boot_manager_mut() {
        manager.boot_all_cpus()
    } else {
        Err("Multi-core boot manager not initialized")
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

/// Atomic wrapper for f64 values
#[derive(Debug)]
pub struct AtomicF64 {
    bits: AtomicU64,
}

impl AtomicF64 {
    pub const fn new(value: f64) -> Self {
        Self {
            bits: AtomicU64::new(value.to_bits()),
        }
    }

    pub fn store(&self, value: f64, ordering: Ordering) {
        self.bits.store(value.to_bits(), ordering);
    }

    pub fn load(&self, ordering: Ordering) -> f64 {
        f64::from_bits(self.bits.load(ordering))
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