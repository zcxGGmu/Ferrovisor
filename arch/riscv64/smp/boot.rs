//! RISC-V SMP Boot Support
//!
//! This module provides SMP boot functionality including:
//! - Secondary CPU initialization
/// - CPU boot sequence
/// - Boot configuration
/// - CPU ready synchronization

use crate::arch::riscv64::*;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Boot configuration
#[derive(Debug, Clone)]
pub struct BootConfig {
    /// Base address for secondary CPUs
    pub entry_point: usize,
    /// Stack pointer for secondary CPUs
    pub stack_top: usize,
    /// Device tree blob address
    pub dtb_address: usize,
    /// Boot arguments
    pub boot_args: usize,
}

impl Default for BootConfig {
    fn default() -> Self {
        Self {
            entry_point: 0,
            stack_top: 0,
            dtb_address: 0,
            boot_args: 0,
        }
    }
}

/// CPU boot state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuBootState {
    /// CPU has not started
    NotStarted,
    /// CPU is starting
    Starting,
    /// CPU has started but not ready
    Started,
    /// CPU is ready
    Ready,
    /// CPU failed to start
    Failed,
}

/// Per-CPU boot information
#[repr(C)]
pub struct CpuBootInfo {
    /// CPU ID (hart ID)
    pub cpu_id: usize,
    /// Boot configuration
    pub config: BootConfig,
    /// Current boot state
    pub state: CpuBootState,
    /// Error code if failed
    pub error_code: isize,
    /// Stack pointer for this CPU
    pub stack_pointer: usize,
}

impl CpuBootInfo {
    /// Create new CPU boot information
    pub const fn new(cpu_id: usize, config: BootConfig) -> Self {
        Self {
            cpu_id,
            config,
            state: CpuBootState::NotStarted,
            error_code: 0,
            stack_pointer: 0,
        }
    }
}

/// Global boot information for all CPUs
static mut CPU_BOOT_INFO: [CpuBootInfo; MAX_CPUS] = [
    CpuBootInfo::new(0, BootConfig::default());
    MAX_CPUS
];

/// Number of CPUs that have started
static CPUS_STARTED: AtomicUsize = AtomicUsize::new(0);

/// Primary CPU ready flag
static PRIMARY_READY: AtomicBool = AtomicBool::new(false);

/// Get CPU boot information
pub fn get_cpu_boot_info(cpu_id: usize) -> Option<&'static CpuBootInfo> {
    if cpu_id < MAX_CPUS {
        unsafe { Some(&CPU_BOOT_INFO[cpu_id]) }
    } else {
        None
    }
}

/// Get mutable CPU boot information
pub fn get_cpu_boot_info_mut(cpu_id: usize) -> Option<&'static mut CpuBootInfo> {
    if cpu_id < MAX_CPUS {
        unsafe { Some(&mut CPU_BOOT_INFO[cpu_id]) }
    } else {
        None
    }
}

/// Initialize boot system
pub fn init_boot_system() -> Result<(), &'static str> {
    log::info!("Initializing SMP boot system");

    // Initialize boot information for all CPUs
    for i in 0..MAX_CPUS {
        unsafe {
            CPU_BOOT_INFO[i] = CpuBootInfo::new(i, BootConfig::default());
        }
    }

    // Mark primary CPU as ready
    PRIMARY_READY.store(true, Ordering::SeqCst);

    log::info!("SMP boot system initialized");
    Ok(())
}

/// Configure boot for secondary CPUs
pub fn configure_secondary_boot(config: BootConfig) -> Result<(), &'static str> {
    log::info!("Configuring secondary CPU boot");

    // Set boot configuration for all secondary CPUs
    for i in 1..MAX_CPUS {
        if let Some(info) = get_cpu_boot_info_mut(i) {
            info.config = config.clone();
            info.stack_pointer = config.stack_top - (i * 64 * 1024); // 64KB stack per CPU
        }
    }

    log::info!("Secondary CPU boot configured");
    Ok(())
}

/// Boot sequence entry point for secondary CPUs
#[no_mangle]
pub extern "C" fn secondary_cpu_entry() -> ! {
    let cpu_id = current_cpu_id();
    log::info!("Secondary CPU {} starting", cpu_id);

    // Get boot information
    let boot_info = get_cpu_boot_info(cpu_id)
        .expect("Boot information not found for this CPU");

    // Mark CPU as starting
    let info_mut = get_cpu_boot_info_mut(cpu_id)
        .expect("Cannot get mutable boot info");
    info_mut.state = CpuBootState::Starting;

    // Set up stack
    let stack_top = boot_info.stack_pointer;
    unsafe {
        // Set stack pointer
        core::arch::asm!("mv sp, {}", in(reg) stack_top);
    }

    // Initialize this CPU
    if let Err(e) = init_secondary_cpu(cpu_id, &boot_info.config) {
        // Mark as failed
        info_mut.state = CpuBootState::Failed;
        info_mut.error_code = e.as_ptr() as isize;

        log::error!("Secondary CPU {} failed to initialize: {}", cpu_id, e);

        // Halt the CPU
        halt_cpu();
    }

    // Mark CPU as ready
    info_mut.state = CpuBootState::Ready;

    // Increment started CPUs counter
    CPUS_STARTED.fetch_add(1, Ordering::SeqCst);

    log::info!("Secondary CPU {} ready", cpu_id);

    // Jump to the main entry point
    let entry_point = boot_info.config.entry_point;

    unsafe {
        core::arch::asm!(
            "jalr zero, {}",
            in(reg) entry_point,
            options(noreturn)
        );
    }
}

/// Initialize a secondary CPU
fn init_secondary_cpu(cpu_id: usize, config: &BootConfig) -> Result<(), &'static str> {
    log::debug!("Initializing secondary CPU {}", cpu_id);

    // Initialize CPU state
    crate::arch::riscv64::cpu::state::init()?;

    // Initialize MMU for this CPU
    crate::arch::riscv64::mmu::init()?;

    // Initialize interrupt handling for this CPU
    crate::arch::riscv64::interrupt::init()?;

    // Initialize virtualization if enabled
    if crate::arch::riscv64::virtualization::has_h_extension() {
        crate::arch::riscv64::virtualization::init()?;
    }

    // Enable interrupts
    crate::arch::riscv64::interrupt::enable_external_interrupts();

    // Enable machine-mode interrupts
    let mut mstatus = crate::arch::riscv64::cpu::csr::MSTATUS::read();
    mstatus |= 1 << 3; // MIE bit
    crate::arch::riscv64::cpu::csr::MSTATUS::write(mstatus);

    log::debug!("Secondary CPU {} initialized successfully", cpu_id);
    Ok(())
}

/// Start a secondary CPU
pub fn start_secondary_cpu(cpu_id: usize) -> Result<(), &'static str> {
    if cpu_id == 0 {
        return Err("Cannot start primary CPU");
    }

    if cpu_id >= MAX_CPUS {
        return Err("Invalid CPU ID");
    }

    // Check if CPU is already started
    let boot_info = get_cpu_boot_info(cpu_id)
        .ok_or("Boot information not found")?;

    if boot_info.state != CpuBootState::NotStarted {
        return Err("CPU already started or failed");
    }

    log::info!("Starting secondary CPU {}", cpu_id);

    // Get SBI services
    use crate::arch::riscv64::smp::sbi::*;

    // Start the CPU using SBI
    let entry_point = secondary_cpu_entry as usize;
    let start_arg = cpu_id;

    sbi_hart_start(cpu_id, entry_point, start_arg)?;

    log::info!("Secondary CPU {} start command sent", cpu_id);
    Ok(())
}

/// Wait for secondary CPU to be ready
pub fn wait_for_cpu_ready(cpu_id: usize, timeout_ms: u64) -> Result<(), &'static str> {
    let start_time = read_csr!(crate::arch::riscv64::cpu::csr::TIME);

    loop {
        if let Some(boot_info) = get_cpu_boot_info(cpu_id) {
            match boot_info.state {
                CpuBootState::Ready => {
                    log::info!("CPU {} is ready", cpu_id);
                    return Ok(());
                }
                CpuBootState::Failed => {
                    return Err("CPU failed to start");
                }
                _ => {
                    // Continue waiting
                }
            }
        }

        // Check timeout
        let current_time = read_csr!(crate::arch::riscv64::cpu::csr::TIME);
        let elapsed = current_time.wrapping_sub(start_time);

        // Simple timeout check (assuming 10MHz timer)
        if elapsed > timeout_ms * 10_000 {
            return Err("Timeout waiting for CPU to be ready");
        }

        // Small delay
        for _ in 0..1000 {
            crate::arch::riscv64::cpu::asm::nop();
        }
    }
}

/// Start all secondary CPUs
pub fn start_all_secondary_cpus() -> Result<usize, &'static str> {
    log::info!("Starting all secondary CPUs");

    let mut started_count = 0;

    for cpu_id in 1..MAX_CPUS {
        match start_secondary_cpu(cpu_id) {
            Ok(_) => {
                started_count += 1;
            }
            Err(e) => {
                log::warn!("Failed to start CPU {}: {}", cpu_id, e);
                // Continue trying to start other CPUs
            }
        }
    }

    log::info!("Sent start commands to {} secondary CPUs", started_count);
    Ok(started_count)
}

/// Wait for all secondary CPUs to be ready
pub fn wait_for_all_cpus_ready(timeout_ms: u64) -> Result<usize, &'static str> {
    log::info!("Waiting for all secondary CPUs to be ready");

    let mut ready_count = 0;

    for cpu_id in 1..MAX_CPUS {
        match wait_for_cpu_ready(cpu_id, timeout_ms) {
            Ok(_) => {
                ready_count += 1;
                log::info!("CPU {} is ready", cpu_id);
            }
            Err(e) => {
                log::warn!("CPU {} failed to become ready: {}", cpu_id, e);
            }
        }
    }

    log::info!("{} secondary CPUs are ready", ready_count);
    Ok(ready_count)
}

/// Get number of started CPUs
pub fn get_started_cpus_count() -> usize {
    CPUS_STARTED.load(Ordering::SeqCst)
}

/// Check if primary CPU is ready
pub fn is_primary_ready() -> bool {
    PRIMARY_READY.load(Ordering::SeqCst)
}

/// Check if a CPU is ready
pub fn is_cpu_ready(cpu_id: usize) -> bool {
    if let Some(boot_info) = get_cpu_boot_info(cpu_id) {
        boot_info.state == CpuBootState::Ready
    } else {
        false
    }
}

/// Get CPU boot state
pub fn get_cpu_boot_state(cpu_id: usize) -> Option<CpuBootState> {
    get_cpu_boot_info(cpu_id).map(|info| info.state)
}

/// Reset a CPU
pub fn reset_cpu(cpu_id: usize) -> Result<(), &'static str> {
    if cpu_id >= MAX_CPUS {
        return Err("Invalid CPU ID");
    }

    log::info!("Resetting CPU {}", cpu_id);

    // Reset boot state
    if let Some(info) = get_cpu_boot_info_mut(cpu_id) {
        info.state = CpuBootState::NotStarted;
        info.error_code = 0;
    }

    // Use SBI to reset the CPU
    use crate::arch::riscv64::smp::sbi::*;
    sbi_hart_start(cpu_id, 0, 0)?; // Start at address 0 to reset

    log::info!("CPU {} reset command sent", cpu_id);
    Ok(())
}

/// Halt the current CPU
pub fn halt_cpu() -> ! {
    // Disable interrupts
    let mut mstatus = crate::arch::riscv64::cpu::csr::MSTATUS::read();
    mstatus &= !(1 << 3); // Clear MIE bit
    crate::arch::riscv64::cpu::csr::MSTATUS::write(mstatus);

    // Wait forever
    loop {
        crate::arch::riscv64::cpu::asm::wfi();
    }
}

/// Power off a CPU
pub fn poweroff_cpu(cpu_id: usize) -> Result<(), &'static str> {
    if cpu_id >= MAX_CPUS {
        return Err("Invalid CPU ID");
    }

    if cpu_id == current_cpu_id() {
        // Cannot power off self
        return Err("Cannot power off current CPU");
    }

    log::info!("Powering off CPU {}", cpu_id);

    // Use SBI to stop the CPU
    use crate::arch::riscv64::smp::sbi::*;
    sbi_hart_stop(cpu_id)?;

    // Mark CPU as not started
    if let Some(info) = get_cpu_boot_info_mut(cpu_id) {
        info.state = CpuBootState::NotStarted;
    }

    // Decrease started CPUs counter
    CPUS_STARTED.fetch_sub(1, Ordering::SeqCst);

    log::info!("CPU {} powered off", cpu_id);
    Ok(())
}

/// Enhanced CPU hotplug support with advanced features
pub mod hotplug {
    use super::*;
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// Hotplug operation types
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum HotplugOp {
        /// Add CPU to system
        Add,
        /// Remove CPU from system
        Remove,
        /// Reset CPU
        Reset,
        /// Suspend CPU
        Suspend,
        /// Resume CPU
        Resume,
    }

    /// Hotplug operation status
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum HotplugStatus {
        /// Operation completed successfully
        Success,
        /// Operation failed
        Failed,
        /// Operation in progress
        InProgress,
        /// Operation not supported
        NotSupported,
    }

    /// Hotplug operation request
    #[derive(Debug)]
    pub struct HotplugRequest {
        /// CPU ID
        pub cpu_id: usize,
        /// Operation type
        pub operation: HotplugOp,
        /// Request timestamp
        pub timestamp: u64,
        /// Requester ID
        pub requester: u32,
        /// Configuration for add operations
        pub config: Option<BootConfig>,
        /// Error code if failed
        pub error_code: isize,
        /// Operation status
        pub status: HotplugStatus,
    }

    impl HotplugRequest {
        /// Create new hotplug request
        pub fn new(cpu_id: usize, operation: HotplugOp, requester: u32) -> Self {
            Self {
                cpu_id,
                operation,
                timestamp: read_csr!(crate::arch::riscv64::cpu::csr::TIME),
                requester,
                config: None,
                error_code: 0,
                status: HotplugStatus::InProgress,
            }
        }

        /// Complete the request successfully
        pub fn complete_success(&mut self) {
            self.status = HotplugStatus::Success;
            self.error_code = 0;
        }

        /// Mark the request as failed
        pub fn complete_failure(&mut self, error_code: isize) {
            self.status = HotplugStatus::Failed;
            self.error_code = error_code;
        }
    }

    /// Hotplug statistics
    #[derive(Debug, Default)]
    pub struct HotplugStats {
        /// Total operations performed
        pub total_operations: AtomicUsize,
        /// Successful operations
        pub successful_operations: AtomicUsize,
        /// Failed operations
        pub failed_operations: AtomicUsize,
        /// Current online CPUs
        pub online_cpus: AtomicUsize,
        /// Peak online CPUs
        pub peak_online_cpus: AtomicUsize,
        /// Total CPU additions
        pub cpu_additions: AtomicUsize,
        /// Total CPU removals
        pub cpu_removals: AtomicUsize,
        /// Total CPU resets
        pub cpu_resets: AtomicUsize,
    }

    impl HotplugStats {
        /// Get success rate as percentage
        pub fn success_rate(&self) -> f64 {
            let total = self.total_operations.load(Ordering::Relaxed);
            if total == 0 {
                return 0.0;
            }
            let successful = self.successful_operations.load(Ordering::Relaxed);
            (successful as f64 / total as f64) * 100.0
        }

        /// Update statistics for successful operation
        pub fn record_success(&self, operation: HotplugOp) {
            self.total_operations.fetch_add(1, Ordering::Relaxed);
            self.successful_operations.fetch_add(1, Ordering::Relaxed);

            match operation {
                HotplugOp::Add => self.cpu_additions.fetch_add(1, Ordering::Relaxed),
                HotplugOp::Remove => self.cpu_removals.fetch_add(1, Ordering::Relaxed),
                HotplugOp::Reset => self.cpu_resets.fetch_add(1, Ordering::Relaxed),
                _ => 0,
            };

            // Update peak online CPUs
            let current_online = self.online_cpus.load(Ordering::Relaxed);
            let mut peak = self.peak_online_cpus.load(Ordering::Relaxed);
            while current_online > peak {
                match self.peak_online_cpus.compare_exchange_weak(
                    peak, current_online, Ordering::Relaxed, Ordering::Relaxed
                ) {
                    Ok(_) => break,
                    Err(actual) => peak = actual,
                }
            }
        }

        /// Update statistics for failed operation
        pub fn record_failure(&self) {
            self.total_operations.fetch_add(1, Ordering::Relaxed);
            self.failed_operations.fetch_add(1, Ordering::Relaxed);
        }

        /// Update online CPU count
        pub fn update_online_count(&self, delta: isize) {
            if delta > 0 {
                self.online_cpus.fetch_add(delta as usize, Ordering::Relaxed);
            } else {
                self.online_cpus.fetch_sub(delta.unsigned_abs() as usize, Ordering::Relaxed);
            }
        }
    }

    /// Global hotplug statistics
    static HOTPLUG_STATS: HotplugStats = HotplugStats::default();

    /// Current hotplug requests (pending operations)
    static mut PENDING_REQUESTS: [Option<HotplugRequest>; 16] = [None; 16];
    static mut PENDING_COUNT: AtomicUsize = AtomicUsize::new(0);

    /// Get hotplug statistics
    pub fn get_hotplug_stats() -> &'static HotplugStats {
        &HOTPLUG_STATS
    }

    /// Add a CPU to the system with enhanced features
    pub fn cpu_add(cpu_id: usize, config: BootConfig) -> Result<HotplugRequest, &'static str> {
        if cpu_id >= MAX_CPUS {
            return Err("Invalid CPU ID");
        }

        if cpu_id == 0 {
            return Err("Cannot add primary CPU");
        }

        // Check if CPU is already online
        if crate::arch::riscv64::smp::is_cpu_online(cpu_id) {
            return Err("CPU already online");
        }

        log::info!("Adding CPU {} to system", cpu_id);

        // Create hotplug request
        let mut request = HotplugRequest::new(cpu_id, HotplugOp::Add, 0);
        request.config = Some(config.clone());

        // Initialize CPU boot information
        if let Some(info) = get_cpu_boot_info_mut(cpu_id) {
            info.config = config.clone();
            info.state = CpuBootState::NotStarted;
            info.error_code = 0;
            info.stack_pointer = config.stack_top - (cpu_id * 64 * 1024);
        }

        // Try to start the CPU
        match start_secondary_cpu(cpu_id) {
            Ok(_) => {
                // Wait for CPU to be ready with timeout
                match wait_for_cpu_ready(cpu_id, 5000) {
                    Ok(_) => {
                        request.complete_success();
                        HOTPLUG_STATS.record_success(HotplugOp::Add);
                        HOTPLUG_STATS.update_online_count(1);

                        // Mark CPU as online in SMP subsystem
                        crate::arch::riscv64::smp::mark_cpu_online(cpu_id);

                        log::info!("CPU {} successfully added and ready", cpu_id);
                    }
                    Err(e) => {
                        request.complete_failure(e.as_ptr() as isize);
                        HOTPLUG_STATS.record_failure();
                        log::error!("CPU {} failed to become ready: {}", cpu_id, e);
                    }
                }
            }
            Err(e) => {
                request.complete_failure(e.as_ptr() as isize);
                HOTPLUG_STATS.record_failure();
                log::error!("Failed to start CPU {}: {}", cpu_id, e);
            }
        }

        Ok(request)
    }

    /// Remove a CPU from the system with enhanced features
    pub fn cpu_remove(cpu_id: usize) -> Result<HotplugRequest, &'static str> {
        if cpu_id == 0 {
            return Err("Cannot remove primary CPU");
        }

        if cpu_id >= MAX_CPUS {
            return Err("Invalid CPU ID");
        }

        if !crate::arch::riscv64::smp::is_cpu_online(cpu_id) {
            return Err("CPU not online");
        }

        log::info!("Removing CPU {} from system", cpu_id);

        let mut request = HotplugRequest::new(cpu_id, HotplugOp::Remove, 0);

        // Check if CPU can be safely removed
        if !cpu_can_remove_safely(cpu_id) {
            request.complete_failure(-1);
            HOTPLUG_STATS.record_failure();
            return Err("CPU cannot be safely removed (currently in use)");
        }

        // Gracefully shutdown the CPU
        match graceful_shutdown_cpu(cpu_id) {
            Ok(_) => {
                request.complete_success();
                HOTPLUG_STATS.record_success(HotplugOp::Remove);
                HOTPLUG_STATS.update_online_count(-1);

                // Mark CPU as offline in SMP subsystem
                crate::arch::riscv64::smp::mark_cpu_offline(cpu_id);

                log::info!("CPU {} successfully removed", cpu_id);
            }
            Err(e) => {
                request.complete_failure(e.as_ptr() as isize);
                HOTPLUG_STATS.record_failure();
                log::error!("Failed to remove CPU {}: {}", cpu_id, e);
            }
        }

        Ok(request)
    }

    /// Reset a CPU with enhanced features
    pub fn cpu_reset(cpu_id: usize) -> Result<HotplugRequest, &'static str> {
        if cpu_id >= MAX_CPUS {
            return Err("Invalid CPU ID");
        }

        log::info!("Resetting CPU {}", cpu_id);

        let mut request = HotplugRequest::new(cpu_id, HotplugOp::Reset, 0);

        // Check if CPU is online
        let was_online = crate::arch::riscv64::smp::is_cpu_online(cpu_id);

        // Reset the CPU
        match reset_cpu(cpu_id) {
            Ok(_) => {
                // If CPU was online, wait for it to become ready again
                if was_online {
                    match wait_for_cpu_ready(cpu_id, 5000) {
                        Ok(_) => {
                            request.complete_success();
                        }
                        Err(e) => {
                            request.complete_failure(e.as_ptr() as isize);
                        }
                    }
                } else {
                    request.complete_success();
                }

                if request.status == HotplugStatus::Success {
                    HOTPLUG_STATS.record_success(HotplugOp::Reset);
                    log::info!("CPU {} successfully reset", cpu_id);
                }
            }
            Err(e) => {
                request.complete_failure(e.as_ptr() as isize);
                HOTPLUG_STATS.record_failure();
                log::error!("Failed to reset CPU {}: {}", cpu_id, e);
            }
        }

        Ok(request)
    }

    /// Suspend a CPU
    pub fn cpu_suspend(cpu_id: usize) -> Result<HotplugRequest, &'static str> {
        if cpu_id >= MAX_CPUS {
            return Err("Invalid CPU ID");
        }

        if cpu_id == 0 {
            return Err("Cannot suspend primary CPU");
        }

        if !crate::arch::riscv64::smp::is_cpu_online(cpu_id) {
            return Err("CPU not online");
        }

        log::info!("Suspending CPU {}", cpu_id);

        let mut request = HotplugRequest::new(cpu_id, HotplugOp::Suspend, 0);

        // Send suspend IPI to CPU
        match crate::arch::riscv64::smp::send_ipi(cpu_id, crate::arch::riscv64::smp::ipi::IpiType::Suspend as u32) {
            Ok(_) => {
                // Wait for CPU to acknowledge suspension
                // In a real implementation, this would involve more sophisticated coordination
                request.complete_success();
                HOTPLUG_STATS.record_success(HotplugOp::Suspend);
                log::info!("CPU {} suspended", cpu_id);
            }
            Err(e) => {
                request.complete_failure(e.as_ptr() as isize);
                HOTPLUG_STATS.record_failure();
                log::error!("Failed to suspend CPU {}: {}", cpu_id, e);
            }
        }

        Ok(request)
    }

    /// Resume a suspended CPU
    pub fn cpu_resume(cpu_id: usize) -> Result<HotplugRequest, &'static str> {
        if cpu_id >= MAX_CPUS {
            return Err("Invalid CPU ID");
        }

        log::info!("Resuming CPU {}", cpu_id);

        let mut request = HotplugRequest::new(cpu_id, HotplugOp::Resume, 0);

        // Send resume IPI to CPU
        match crate::arch::riscv64::smp::send_ipi(cpu_id, crate::arch::riscv64::smp::ipi::IpiType::Resume as u32) {
            Ok(_) => {
                // Wait for CPU to acknowledge resume
                match wait_for_cpu_ready(cpu_id, 2000) {
                    Ok(_) => {
                        request.complete_success();
                        HOTPLUG_STATS.record_success(HotplugOp::Resume);
                        log::info!("CPU {} resumed", cpu_id);
                    }
                    Err(e) => {
                        request.complete_failure(e.as_ptr() as isize);
                    }
                }
            }
            Err(e) => {
                request.complete_failure(e.as_ptr() as isize);
                HOTPLUG_STATS.record_failure();
                log::error!("Failed to resume CPU {}: {}", cpu_id, e);
            }
        }

        Ok(request)
    }

    /// Check if CPU can be hot-plugged
    pub fn cpu_can_hotplug(cpu_id: usize) -> bool {
        if cpu_id >= MAX_CPUS {
            return false;
        }

        if let Some(info) = get_cpu_boot_info(cpu_id) {
            info.state == CpuBootState::NotStarted || info.state == CpuBootState::Failed
        } else {
            false
        }
    }

    /// Check if CPU can be safely removed
    fn cpu_can_remove_safely(cpu_id: usize) -> bool {
        // Check if CPU has any VCPU assigned
        if let Some(per_cpu) = crate::arch::riscv64::cpu::state::cpu_data(cpu_id) {
            if per_cpu.get_vcpu_id().is_some() {
                return false;
            }
        }

        // Check if CPU has any pending work
        // In a real implementation, this would check scheduler queues, etc.

        true
    }

    /// Gracefully shutdown a CPU
    fn graceful_shutdown_cpu(cpu_id: usize) -> Result<(), &'static str> {
        log::debug!("Gracefully shutting down CPU {}", cpu_id);

        // Send shutdown IPI
        crate::arch::riscv64::smp::send_ipi(cpu_id, crate::arch::riscv64::smp::ipi::IpiType::Shutdown as u32)?;

        // Wait for CPU to acknowledge shutdown
        let start_time = read_csr!(crate::arch::riscv64::cpu::csr::TIME);
        let timeout = 10_000_000; // ~1 second at 10MHz

        loop {
            if let Some(info) = get_cpu_boot_info(cpu_id) {
                if info.state == CpuBootState::NotStarted {
                    return Ok(());
                }
            }

            let current_time = read_csr!(crate::arch::riscv64::cpu::csr::TIME);
            if current_time.wrapping_sub(start_time) > timeout {
                break; // Timeout
            }

            // Small delay
            for _ in 0..1000 {
                crate::arch::riscv64::cpu::asm::nop();
            }
        }

        // Force power off if graceful shutdown failed
        poweroff_cpu(cpu_id)
    }

    /// Batch hotplug operations
    pub fn batch_hotplug_operations(operations: Vec<(usize, HotplugOp, Option<BootConfig>)>) -> Vec<HotplugRequest> {
        let mut results = Vec::with_capacity(operations.len());

        for (cpu_id, operation, config) in operations {
            let result = match operation {
                HotplugOp::Add => {
                    if let Some(cfg) = config {
                        cpu_add(cpu_id, cfg)
                    } else {
                        let mut req = HotplugRequest::new(cpu_id, operation, 0);
                        req.complete_failure(-2); // No configuration provided
                        Ok(req)
                    }
                }
                HotplugOp::Remove => cpu_remove(cpu_id),
                HotplugOp::Reset => cpu_reset(cpu_id),
                HotplugOp::Suspend => cpu_suspend(cpu_id),
                HotplugOp::Resume => cpu_resume(cpu_id),
            };

            match result {
                Ok(req) => results.push(req),
                Err(e) => {
                    let mut req = HotplugRequest::new(cpu_id, operation, 0);
                    req.complete_failure(e.as_ptr() as isize);
                    results.push(req);
                }
            }
        }

        results
    }

    /// Validate CPU hotplug request
    pub fn validate_hotplug_request(request: &HotplugRequest) -> Result<(), &'static str> {
        // Validate CPU ID
        if request.cpu_id >= MAX_CPUS {
            return Err("Invalid CPU ID");
        }

        // Validate operation based on current state
        match request.operation {
            HotplugOp::Add => {
                if crate::arch::riscv64::smp::is_cpu_online(request.cpu_id) {
                    return Err("CPU already online");
                }
                if request.config.is_none() {
                    return Err("Boot configuration required for add operation");
                }
            }
            HotplugOp::Remove => {
                if !crate::arch::riscv64::smp::is_cpu_online(request.cpu_id) {
                    return Err("CPU not online");
                }
                if request.cpu_id == 0 {
                    return Err("Cannot remove primary CPU");
                }
            }
            HotplugOp::Reset => {
                // Reset is always allowed
            }
            HotplugOp::Suspend => {
                if !crate::arch::riscv64::smp::is_cpu_online(request.cpu_id) {
                    return Err("CPU not online");
                }
                if request.cpu_id == 0 {
                    return Err("Cannot suspend primary CPU");
                }
            }
            HotplugOp::Resume => {
                // Resume can be attempted even if CPU appears offline (might be suspended)
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_config() {
        let config = BootConfig::default();
        assert_eq!(config.entry_point, 0);
        assert_eq!(config.stack_top, 0);
    }

    #[test]
    fn test_cpu_boot_info() {
        let config = BootConfig {
            entry_point: 0x80000000,
            stack_top: 0x90000000,
            dtb_address: 0x41000000,
            boot_args: 0,
        };

        let info = CpuBootInfo::new(1, config);
        assert_eq!(info.cpu_id, 1);
        assert_eq!(info.config.entry_point, 0x80000000);
        assert_eq!(info.state, CpuBootState::NotStarted);
    }

    #[test]
    fn test_cpu_boot_state() {
        let mut info = CpuBootInfo::new(1, BootConfig::default());

        assert_eq!(info.state, CpuBootState::NotStarted);

        info.state = CpuBootState::Ready;
        assert_eq!(info.state, CpuBootState::Ready);
    }
}