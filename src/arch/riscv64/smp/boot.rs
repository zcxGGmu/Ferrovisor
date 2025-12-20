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
    let start_time = read_csr!(crate::arch::riscv64::csr::TIME);

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
        let current_time = read_csr!(crate::arch::riscv64::csr::TIME);
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

/// CPU hotplug support
pub mod hotplug {
    use super::*;

    /// Add a CPU to the system
    pub fn cpu_add(cpu_id: usize, config: BootConfig) -> Result<(), &'static str> {
        if cpu_id >= MAX_CPUS {
            return Err("Invalid CPU ID");
        }

        if let Some(info) = get_cpu_boot_info_mut(cpu_id) {
            info.config = config;
            info.state = CpuBootState::NotStarted;
            info.error_code = 0;
            info.stack_pointer = config.stack_top - (cpu_id * 64 * 1024);
        }

        log::info!("CPU {} added to system", cpu_id);
        Ok(())
    }

    /// Remove a CPU from the system
    pub fn cpu_remove(cpu_id: usize) -> Result<(), &'static str> {
        if cpu_id == 0 {
            return Err("Cannot remove primary CPU");
        }

        if cpu_id >= MAX_CPUS {
            return Err("Invalid CPU ID");
        }

        // Power off the CPU first
        poweroff_cpu(cpu_id)?;

        log::info!("CPU {} removed from system", cpu_id);
        Ok(())
    }

    /// Check if CPU can be hot-plugged
    pub fn cpu_can_hotplug(cpu_id: usize) -> bool {
        if cpu_id >= MAX_CPUS {
            return false;
        }

        if let Some(info) = get_cpu_boot_info(cpu_id) {
            info.state == CpuBootState::NotStarted
        } else {
            false
        }
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