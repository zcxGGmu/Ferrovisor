//! RISC-V IPI (Inter-Processor Interrupt) Support
//!
//! This module provides IPI functionality including:
//! - IPI sending and receiving
/// - IPI types and handling
/// - IPI registration system
/// - Cross-CPU signaling

use crate::arch::riscv64::*;
use bitflags::bitflags;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// IPI types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum IpiType {
    /// Reschedule IPI
    Reschedule = 0,
    /// TLB shootdown IPI
    TlbShootdown = 1,
    /// Function call IPI
    FunctionCall = 2,
    /// Stop CPU IPI
    Stop = 3,
    /// Debug IPI
    Debug = 4,
    /// Timer broadcast IPI
    Timer = 5,
    /// Wake up IPI
    WakeUp = 6,
    /// Custom IPI (user-defined)
    Custom = 7,
    /// CPU hotplug suspend IPI
    Suspend = 8,
    /// CPU hotplug resume IPI
    Resume = 9,
    /// CPU hotplug shutdown IPI
    Shutdown = 10,
    /// CPU hotplug add IPI
    Add = 11,
    /// CPU hotplug remove IPI
    Remove = 12,
    /// VM migration IPI
    VmMigrate = 13,
    /// Memory pressure IPI
    MemoryPressure = 14,
    /// Maximum IPI type
    Max = 15,
}

/// IPI flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct IpiFlags: u32 {
        /// High priority IPI
        const HIGH_PRIORITY = 1 << 0;
        /// One-shot IPI
        const ONE_SHOT = 1 << 1;
        /// Pending IPI
        const PENDING = 1 << 2;
        /// Handled IPI
        const HANDLED = 1 << 3;
    }
}

/// IPI handler function type
pub type IpiHandler = fn(cpu_id: usize, data: u64) -> Result<(), &'static str>;

/// Per-CPU IPI state
#[derive(Debug)]
pub struct CpuIpiState {
    /// Pending IPIs
    pending: AtomicU32,
    /// IPI flags per type
    flags: [AtomicU32; IpiType::Max as usize],
    /// IPI data per type
    data: [AtomicU64; IpiType::Max as usize],
    /// IPI handlers
    handlers: [Option<IpiHandler>; IpiType::Max as usize],
    /// IPI count statistics
    ipi_counts: [AtomicU64; IpiType::Max as usize],
}

impl CpuIpiState {
    /// Create a new CPU IPI state
    pub const fn new() -> Self {
        Self {
            pending: AtomicU32::new(0),
            flags: [const { AtomicU32::new(0) }; IpiType::Max as usize],
            data: [const { AtomicU64::new(0) }; IpiType::Max as usize],
            handlers: [None; IpiType::Max as usize],
            ipi_counts: [const { AtomicU64::new(0) }; IpiType::Max as usize],
        }
    }

    /// Check if an IPI type is pending
    pub fn is_pending(&self, ipi_type: IpiType) -> bool {
        let pending = self.pending.load(Ordering::SeqCst);
        (pending & (1 << ipi_type as u32)) != 0
    }

    /// Mark an IPI type as pending
    pub fn set_pending(&self, ipi_type: IpiType) {
        let mut pending = self.pending.load(Ordering::SeqCst);
        loop {
            let new_pending = pending | (1 << ipi_type as u32);
            match self.pending.compare_exchange_weak(
                pending,
                new_pending,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(actual) => pending = actual,
            }
        }

        // Set pending flag
        self.flags[ipi_type as usize].store(IpiFlags::PENDING.bits(), Ordering::SeqCst);
    }

    /// Clear an IPI type
    pub fn clear_pending(&self, ipi_type: IpiType) {
        let mut pending = self.pending.load(Ordering::SeqCst);
        loop {
            let new_pending = pending & !(1 << ipi_type as u32);
            match self.pending.compare_exchange_weak(
                pending,
                new_pending,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(actual) => pending = actual,
            }
        }

        // Clear flags
        self.flags[ipi_type as usize].store(0, Ordering::SeqCst);
    }

    /// Get IPI data
    pub fn get_data(&self, ipi_type: IpiType) -> u64 {
        self.data[ipi_type as usize].load(Ordering::SeqCst)
    }

    /// Set IPI data
    pub fn set_data(&self, ipi_type: IpiType, data: u64) {
        self.data[ipi_type as usize].store(data, Ordering::SeqCst);
    }

    /// Register IPI handler
    pub fn register_handler(&mut self, ipi_type: IpiType, handler: IpiHandler) {
        self.handlers[ipi_type as usize] = Some(handler);
    }

    /// Get IPI handler
    pub fn get_handler(&self, ipi_type: IpiType) -> Option<IpiHandler> {
        self.handlers[ipi_type as usize]
    }

    /// Increment IPI count
    pub fn increment_count(&self, ipi_type: IpiType) {
        self.ipi_counts[ipi_type as usize].fetch_add(1, Ordering::SeqCst);
    }

    /// Get IPI count
    pub fn get_count(&self, ipi_type: IpiType) -> u64 {
        self.ipi_counts[ipi_type as usize].load(Ordering::SeqCst)
    }

    /// Get all pending IPIs
    pub fn get_pending_ipis(&self) -> Vec<IpiType> {
        let pending = self.pending.load(Ordering::SeqCst);
        let mut ipis = Vec::new();

        for i in 0..IpiType::Max as usize {
            if (pending & (1 << i)) != 0 {
                if let Ok(ipi_type) = IpiType::try_from(i as u32) {
                    ipis.push(ipi_type);
                }
            }
        }

        ipis
    }
}

impl Default for CpuIpiState {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-CPU IPI states
static mut CPU_IPI_STATES: [CpuIpiState; MAX_CPUS] = [CpuIpiState::new(); MAX_CPUS];

/// IPI mask for broadcasting
static IPI_MASK_ALL: AtomicU32 = AtomicU32::new(0);

/// Initialize IPI subsystem
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing IPI subsystem");

    let current_cpu = crate::arch::riscv64::cpu::current_cpu_id();

    // Set default IPI handlers
    if current_cpu == 0 {
        // Only primary CPU sets up default handlers
        setup_default_handlers();
    }

    // Clear any pending IPIs for this CPU
    clear_all_pending_ipis();

    // Enable IPI interrupt in local interrupt controller
    enable_ipi_interrupt();

    log::info!("IPI subsystem initialized for CPU {}", current_cpu);
    Ok(())
}

/// Get CPU IPI state
pub fn get_cpu_ipi_state(cpu_id: usize) -> Option<&'static CpuIpiState> {
    if cpu_id < MAX_CPUS {
        unsafe { Some(&CPU_IPI_STATES[cpu_id]) }
    } else {
        None
    }
}

/// Get mutable CPU IPI state
pub fn get_cpu_ipi_state_mut(cpu_id: usize) -> Option<&'static mut CpuIpiState> {
    if cpu_id < MAX_CPUS {
        unsafe { Some(&mut CPU_IPI_STATES[cpu_id]) }
    } else {
        None
    }
}

/// Send an IPI to a specific CPU
pub fn send_ipi(target_cpu: usize, ipi_type: IpiType, data: u64) -> Result<(), &'static str> {
    if target_cpu >= MAX_CPUS {
        return Err("Invalid target CPU ID");
    }

    let ipi_state = get_cpu_ipi_state(target_cpu)
        .ok_or("Target CPU IPI state not found")?;

    // Set IPI data and mark as pending
    ipi_state.set_data(ipi_type, data);
    ipi_state.set_pending(ipi_type);
    ipi_state.increment_count(ipi_type);

    // Use hardware interrupt controller to send IPI
    crate::arch::riscv64::interrupt::send_ipi(target_cpu)?;

    log::debug!("Sent IPI type {} to CPU {} with data {:#x}",
                ipi_type as u32, target_cpu, data);

    Ok(())
}

/// Send IPI to multiple CPUs
pub fn send_ipi_to_many(
    target_cpus: &[usize],
    ipi_type: IpiType,
    data: u64,
) -> Result<(), &'static str> {
    let mut errors = Vec::new();

    for &cpu_id in target_cpus {
        if let Err(e) = send_ipi(cpu_id, ipi_type, data) {
            errors.push((cpu_id, e));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        log::error!("Failed to send IPI to some CPUs: {:?}", errors);
        Err("Failed to send IPI to some CPUs")
    }
}

/// Broadcast IPI to all CPUs
pub fn broadcast_ipi(ipi_type: IpiType, data: u64, exclude_self: bool) -> Result<(), &'static str> {
    let current_cpu = crate::arch::riscv64::cpu::current_cpu_id();
    let mut target_cpus = Vec::new();

    for cpu_id in 0..MAX_CPUS {
        if !exclude_self || cpu_id != current_cpu {
            target_cpus.push(cpu_id);
        }
    }

    send_ipi_to_many(&target_cpus, ipi_type, data)
}

/// Send reschedule IPI to a CPU
pub fn send_reschedule_ipi(target_cpu: usize) -> Result<(), &'static str> {
    send_ipi(target_cpu, IpiType::Reschedule, 0)
}

/// Send TLB shootdown IPI to CPUs
pub fn send_tlb_shootdown_ipi(
    target_cpus: &[usize],
    addr: usize,
    asid: u16,
) -> Result<(), &'static str> {
    let data = ((asid as u64) << 48) | (addr as u64);
    send_ipi_to_many(target_cpus, IpiType::TlbShootdown, data)
}

/// Send function call IPI to a CPU
pub fn send_function_call_ipi(
    target_cpu: usize,
    func: usize,
    arg: usize,
) -> Result<(), &'static str> {
    let data = ((arg as u64) << 32) | (func as u64);
    send_ipi(target_cpu, IpiType::FunctionCall, data)
}

/// Send stop IPI to a CPU
pub fn send_stop_ipi(target_cpu: usize) -> Result<(), &'static str> {
    send_ipi(target_cpu, IpiType::Stop, 0)
}

/// Send wake up IPI to a CPU
pub fn send_wake_up_ipi(target_cpu: usize) -> Result<(), &'static str> {
    send_ipi(target_cpu, IpiType::WakeUp, 0)
}

/// Handle incoming IPI
pub fn handle_ipi() -> Result<(), &'static str> {
    let current_cpu = crate::arch::riscv64::cpu::current_cpu_id();
    let ipi_state = get_cpu_ipi_state(current_cpu)
        .ok_or("CPU IPI state not found")?;

    // Get all pending IPIs
    let pending_ipis = ipi_state.get_pending_ipis();

    if pending_ipis.is_empty() {
        return Ok(());
    }

    log::debug!("Handling {} pending IPIs on CPU {}", pending_ipis.len(), current_cpu);

    // Handle each pending IPI
    for ipi_type in pending_ipis {
        let data = ipi_state.get_data(ipi_type);
        let handler = ipi_state.get_handler(ipi_type);

        if let Some(handler) = handler {
            if let Err(e) = handler(current_cpu, data) {
                log::error!("IPI handler failed for type {} on CPU {}: {}",
                           ipi_type as u32, current_cpu, e);
            }
        } else {
            log::warn!("No handler for IPI type {} on CPU {}", ipi_type as u32, current_cpu);
        }

        // Clear the IPI
        ipi_state.clear_pending(ipi_type);
    }

    Ok(())
}

/// Clear all pending IPIs
pub fn clear_all_pending_ipis() {
    let current_cpu = crate::arch::riscv64::cpu::current_cpu_id();

    if let Some(ipi_state) = get_cpu_ipi_state(current_cpu) {
        for i in 0..IpiType::Max as usize {
            if let Ok(ipi_type) = IpiType::try_from(i as u32) {
                ipi_state.clear_pending(ipi_type);
            }
        }
    }
}

/// Enable IPI interrupt in local interrupt controller
fn enable_ipi_interrupt() {
    // This would enable the software interrupt in the local interrupt controller
    // The implementation depends on the specific interrupt controller (ACLINT, etc.)

    // For ACLINT, we set the SSIP bit
    let mut sip = crate::arch::riscv64::cpu::csr::SIP::read();
    sip |= crate::arch::riscv64::cpu::csr::Sip::SSIP;
    crate::arch::riscv64::cpu::csr::SIP::write(sip);

    // Enable software interrupt in SIE
    let mut sie = crate::arch::riscv64::cpu::csr::SIE::read();
    sie |= crate::arch::riscv64::cpu::csr::Sie::SSIE;
    crate::arch::riscv64::cpu::csr::SIE::write(sie);
}

/// Setup default IPI handlers
fn setup_default_handlers() {
    // Register default handlers for common IPI types
    register_ipi_handler(IpiType::Reschedule, reschedule_ipi_handler);
    register_ipi_handler(IpiType::TlbShootdown, tlb_shootdown_ipi_handler);
    register_ipi_handler(IpiType::Stop, stop_ipi_handler);
    register_ipi_handler(IpiType::WakeUp, wake_up_ipi_handler);

    // Register handlers for hotplug IPI types
    register_ipi_handler(IpiType::Suspend, suspend_ipi_handler);
    register_ipi_handler(IpiType::Resume, resume_ipi_handler);
    register_ipi_handler(IpiType::Shutdown, shutdown_ipi_handler);
    register_ipi_handler(IpiType::Add, add_cpu_ipi_handler);
    register_ipi_handler(IpiType::Remove, remove_cpu_ipi_handler);

    // Register handlers for virtualization IPI types
    register_ipi_handler(IpiType::VmMigrate, vm_migrate_ipi_handler);
    register_ipi_handler(IpiType::MemoryPressure, memory_pressure_ipi_handler);
}

/// Register an IPI handler
pub fn register_ipi_handler(ipi_type: IpiType, handler: IpiHandler) {
    // Register on all CPUs
    for cpu_id in 0..MAX_CPUS {
        if let Some(ipi_state) = get_cpu_ipi_state_mut(cpu_id) {
            ipi_state.register_handler(ipi_type, handler);
        }
    }
}

/// Reschedule IPI handler
fn reschedule_ipi_handler(_cpu_id: usize, _data: u64) -> Result<(), &'static str> {
    log::debug!("Received reschedule IPI");

    // Trigger scheduler
    // In a real implementation, this would set a flag to cause scheduler to run
    crate::arch::riscv64::cpu::asm::nop(); // Placeholder

    Ok(())
}

/// TLB shootdown IPI handler
fn tlb_shootdown_ipi_handler(_cpu_id: usize, data: u64) -> Result<(), &'static str> {
    let addr = (data & 0xFFFFFFFF) as usize;
    let asid = ((data >> 48) & 0xFFFF) as u16;

    log::debug!("TLB shootdown IPI: addr={:#x}, asid={}", addr, asid);

    // Invalidate TLB entries
    if addr == 0 {
        // Invalidate all TLB entries for ASID
        crate::arch::riscv64::cpu::asm::sfence_vma_asid(0, asid as usize);
    } else {
        // Invalidate specific address
        crate::arch::riscv64::cpu::asm::sfence_vma_addr_asid(addr, asid as usize);
    }

    Ok(())
}

/// Stop IPI handler
fn stop_ipi_handler(cpu_id: usize, _data: u64) -> Result<(), &'static str> {
    log::info!("CPU {} received stop IPI, halting", cpu_id);

    // Halt the CPU
    crate::arch::riscv64::smp::boot::halt_cpu()
}

/// Wake up IPI handler
fn wake_up_ipi_handler(cpu_id: usize, _data: u64) -> Result<(), &'static str> {
    log::debug!("CPU {} received wake up IPI", cpu_id);

    // Wake up the CPU
    // In a real implementation, this would wake up a sleeping CPU
    crate::arch::riscv64::cpu::asm::nop(); // Placeholder

    Ok(())
}

/// CPU suspend IPI handler
fn suspend_ipi_handler(cpu_id: usize, _data: u64) -> Result<(), &'static str> {
    log::info!("CPU {} received suspend IPI", cpu_id);

    // Save current CPU state
    crate::arch::riscv64::cpu::state::save_to_per_cpu(crate::arch::riscv64::cpu::state::save_state());

    // Mark CPU as suspended in SMP subsystem
    crate::arch::riscv64::cpu::state::this_cpu().mark_offline();

    // Disable interrupts and wait for resume
    crate::arch::riscv64::interrupt::disable_external_interrupts();
    let mut mstatus = crate::arch::riscv64::cpu::csr::MSTATUS::read();
    mstatus &= !(1 << 3); // Clear MIE bit
    crate::arch::riscv64::cpu::csr::MSTATUS::write(mstatus);

    // Wait for resume IPI
    loop {
        crate::arch::riscv64::cpu::asm::wfi();

        // Check for resume IPI
        if let Some(ipi_state) = get_cpu_ipi_state(cpu_id) {
            if ipi_state.is_pending(IpiType::Resume) {
                ipi_state.clear_pending(IpiType::Resume);
                break;
            }
        }
    }

    // Re-enable interrupts and mark as online
    crate::arch::riscv64::interrupt::enable_external_interrupts();
    crate::arch::riscv64::cpu::state::this_cpu().mark_online();

    log::info!("CPU {} resumed from suspension", cpu_id);
    Ok(())
}

/// CPU resume IPI handler
fn resume_ipi_handler(cpu_id: usize, _data: u64) -> Result<(), &'static str> {
    log::info!("CPU {} received resume IPI", cpu_id);

    // This handler is mainly for cleanup after resume
    // The actual resume logic is handled in suspend_ipi_handler

    Ok(())
}

/// CPU shutdown IPI handler
fn shutdown_ipi_handler(cpu_id: usize, _data: u64) -> Result<(), &'static str> {
    log::info!("CPU {} received shutdown IPI", cpu_id);

    // Mark CPU as offline in SMP subsystem
    crate::arch::riscv64::cpu::state::this_cpu().mark_offline();

    // Clean up any resources
    if let Some(per_cpu) = crate::arch::riscv64::cpu::state::cpu_data(cpu_id) {
        per_cpu.clear_vcpu();
    }

    // Halt the CPU
    crate::arch::riscv64::smp::boot::halt_cpu()
}

/// CPU add IPI handler
fn add_cpu_ipi_handler(cpu_id: usize, data: u64) -> Result<(), &'static str> {
    let entry_point = (data & 0xFFFFFFFF) as usize;
    let stack_top = ((data >> 32) & 0xFFFFFFFF) as usize;

    log::info!("CPU {} received add IPI: entry={:#x}, stack={:#x}",
              cpu_id, entry_point, stack_top);

    // This would be handled by the boot system
    // In a real implementation, this might trigger re-initialization

    Ok(())
}

/// CPU remove IPI handler
fn remove_cpu_ipi_handler(cpu_id: usize, _data: u64) -> Result<(), &'static str> {
    log::info!("CPU {} received remove IPI", cpu_id);

    // This is mainly for cleanup
    // The actual removal logic is handled elsewhere

    Ok(())
}

/// VM migration IPI handler
fn vm_migrate_ipi_handler(cpu_id: usize, data: u64) -> Result<(), &'static str> {
    let target_cpu = (data & 0xFFFFFFFF) as usize;
    let vm_id = ((data >> 32) & 0xFFFF) as u16;
    let vcpu_id = ((data >> 48) & 0xFFFF) as u16;

    log::info!("CPU {} received VM migration IPI: target_cpu={}, vm_id={}, vcpu_id={}",
              cpu_id, target_cpu, vm_id, vcpu_id);

    // Migrate VCPU from this CPU to target CPU
    if let Some(per_cpu) = crate::arch::riscv64::cpu::state::cpu_data(cpu_id) {
        if per_cpu.get_vcpu_id() == Some(vcpu_id as usize) {
            // Clear current VCPU assignment
            per_cpu.clear_vcpu();

            // In a real implementation, this would:
            // 1. Save VCPU state
            // 2. Transfer VCPU context to target CPU
            // 3. Notify target CPU

            log::info!("VCPU {} migrated from CPU {} to CPU {}", vcpu_id, cpu_id, target_cpu);
        }
    }

    Ok(())
}

/// Memory pressure IPI handler
fn memory_pressure_ipi_handler(cpu_id: usize, data: u64) -> Result<(), &'static str> {
    let pressure_level = (data & 0xFF) as u8;
    let reclaim_target = ((data >> 8) & 0xFFFFFFFF) as usize;

    log::info!("CPU {} received memory pressure IPI: level={}, target={:#x}",
              cpu_id, pressure_level, reclaim_target);

    // Handle memory pressure based on level
    match pressure_level {
        0 => {
            // Low pressure - minor cleanup
            log::debug!("Low memory pressure detected on CPU {}", cpu_id);
        }
        1 => {
            // Medium pressure - aggressive cleanup
            log::debug!("Medium memory pressure detected on CPU {}, performing cleanup", cpu_id);
        }
        2 => {
            // High pressure - emergency cleanup
            log::warn!("High memory pressure detected on CPU {}, performing emergency cleanup", cpu_id);
        }
        _ => {
            log::error!("Invalid memory pressure level: {}", pressure_level);
        }
    }

    // In a real implementation, this would trigger memory reclamation
    crate::arch::riscv64::cpu::asm::nop(); // Placeholder

    Ok(())
}

/// Get IPI statistics
pub fn get_ipi_stats(cpu_id: usize) -> Result<Vec<(IpiType, u64)>, &'static str> {
    let ipi_state = get_cpu_ipi_state(cpu_id)
        .ok_or("CPU IPI state not found")?;

    let mut stats = Vec::new();

    for i in 0..IpiType::Max as usize {
        if let Ok(ipi_type) = IpiType::try_from(i as u32) {
            let count = ipi_state.get_count(ipi_type);
            if count > 0 {
                stats.push((ipi_type, count));
            }
        }
    }

    Ok(stats)
}

/// Clear IPI statistics
pub fn clear_ipi_stats(cpu_id: usize) -> Result<(), &'static str> {
    let ipi_state = get_cpu_ipi_state(cpu_id)
        .ok_or("CPU IPI state not found")?;

    for i in 0..IpiType::Max as usize {
        ipi_state.ipi_counts[i].store(0, Ordering::SeqCst);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_ipi_state() {
        let ipi_state = CpuIpiState::new();

        assert!(!ipi_state.is_pending(IpiType::Reschedule));

        ipi_state.set_pending(IpiType::Reschedule);
        assert!(ipi_state.is_pending(IpiType::Reschedule));

        ipi_state.clear_pending(IpiType::Reschedule);
        assert!(!ipi_state.is_pending(IpiType::Reschedule));
    }

    #[test]
    fn test_ipi_type() {
        assert_eq!(IpiType::Reschedule as u32, 0);
        assert_eq!(IpiType::TlbShootdown as u32, 1);
        assert_eq!(IpiType::Stop as u32, 3);
    }

    #[test]
    fn test_ipi_flags() {
        let mut flags = IpiFlags::empty();
        assert!(!flags.contains(IpiFlags::HIGH_PRIORITY));

        flags.insert(IpiFlags::HIGH_PRIORITY | IpiFlags::ONE_SHOT);
        assert!(flags.contains(IpiFlags::HIGH_PRIORITY));
        assert!(flags.contains(IpiFlags::ONE_SHOT));
    }
}