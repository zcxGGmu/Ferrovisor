//! RISC-V Debug Support Module
//!
//! This module provides comprehensive debugging support for RISC-V including:
//! - Debug register access and management
//! - Hardware breakpoints and watchpoints
//! - Single stepping and program tracing
//! - JTAG and RISC-V Debug Interface
//! - Virtual machine debugging support
//! - Core dump and crash analysis

pub mod regs;
pub mod breakpoint;
pub mod tracer;
pub mod jtag;
pub mod vm_debug;

use crate::arch::riscv64::*;
use regs::DebugRegisters;
use breakpoint::{BreakpointManager, BreakpointType};
use tracer::{Tracer, TraceEvent};

/// Debug configuration
#[derive(Debug, Clone)]
pub struct DebugConfig {
    /// Enable debug support
    pub enabled: bool,
    /// Number of hardware breakpoints
    pub hw_breakpoints: u32,
    /// Number of hardware watchpoints
    pub hw_watchpoints: u32,
    /// Enable program trace
    pub enable_trace: bool,
    /// Trace buffer size
    pub trace_buffer_size: usize,
    /// Enable JTAG interface
    pub enable_jtag: bool,
    /// Enable VM debugging
    pub enable_vm_debug: bool,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hw_breakpoints: 16,
            hw_watchpoints: 8,
            enable_trace: true,
            trace_buffer_size: 65536, // 64KB
            enable_jtag: true,
            enable_vm_debug: true,
        }
    }
}

/// Global debug state
static mut DEBUG_CONFIG: Option<DebugConfig> = None;
static mut DEBUG_REGISTERS: Option<DebugRegisters> = None;
static mut BREAKPOINT_MANAGER: Option<BreakpointManager> = None;
static mut TRACER: Option<Tracer> = None;

/// Initialize debug subsystem
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing RISC-V debug subsystem");

    // Initialize with default config
    let config = DebugConfig::default();
    init_with_config(config)?;

    log::info!("RISC-V debug subsystem initialized");
    Ok(())
}

/// Initialize debug subsystem with configuration
pub fn init_with_config(config: DebugConfig) -> Result<(), &'static str> {
    if !config.enabled {
        log::info!("Debug support is disabled");
        return Ok(());
    }

    log::info!("Initializing debug with {} breakpoints, {} watchpoints",
              config.hw_breakpoints, config.hw_watchpoints);

    // Store configuration
    unsafe {
        DEBUG_CONFIG = Some(config.clone());
    }

    // Initialize debug registers
    let debug_regs = DebugRegisters::new()?;
    unsafe {
        DEBUG_REGISTERS = Some(debug_regs);
    }

    // Initialize breakpoint manager
    let bp_manager = BreakpointManager::new(
        config.hw_breakpoints,
        config.hw_watchpoints,
    )?;
    unsafe {
        BREAKPOINT_MANAGER = Some(bp_manager);
    }

    // Initialize tracer if enabled
    if config.enable_trace {
        let tracer = Tracer::new(config.trace_buffer_size)?;
        unsafe {
            TRACER = Some(tracer);
        }
    }

    // Initialize JTAG interface if enabled
    if config.enable_jtag {
        jtag::init()?;
    }

    // Enable debug mode in hardware
    enable_debug_mode()?;

    log::info!("Debug subsystem initialized successfully");
    Ok(())
}

/// Get debug configuration
pub fn get_config() -> Option<DebugConfig> {
    unsafe { DEBUG_CONFIG.clone() }
}

/// Get debug registers
pub fn get_debug_registers() -> Option<&'static DebugRegisters> {
    unsafe { DEBUG_REGISTERS.as_ref() }
}

/// Get breakpoint manager
pub fn get_breakpoint_manager() -> Option<&'static BreakpointManager> {
    unsafe { BREAKPOINT_MANAGER.as_ref() }
}

/// Get tracer
pub fn get_tracer() -> Option<&'static Tracer> {
    unsafe { TRACER.as_ref() }
}

/// Enable debug mode
pub fn enable_debug_mode() -> Result<(), &'static str> {
    log::debug!("Enabling RISC-V debug mode");

    // Enable debug mode in DCSR
    if let Some(debug_regs) = get_debug_registers() {
        let mut dcsr = debug_regs.read_dcsr();
        dcsr.set_debug_enable(true);
        debug_regs.write_dcsr(dcsr);
    }

    log::debug!("Debug mode enabled");
    Ok(())
}

/// Disable debug mode
pub fn disable_debug_mode() -> Result<(), &'static str> {
    log::debug!("Disabling RISC-V debug mode");

    // Disable debug mode in DCSR
    if let Some(debug_regs) = get_debug_registers() {
        let mut dcsr = debug_regs.read_dcsr();
        dcsr.set_debug_enable(false);
        debug_regs.write_dcsr(dcsr);
    }

    log::debug!("Debug mode disabled");
    Ok(())
}

/// Check if debug mode is enabled
pub fn is_debug_mode_enabled() -> bool {
    if let Some(debug_regs) = get_debug_registers() {
        let dcsr = debug_regs.read_dcsr();
        dcsr.debug_enable()
    } else {
        false
    }
}

/// Enter debug mode (halt the CPU)
pub fn enter_debug_mode() -> Result<(), &'static str> {
    log::debug!("Entering debug mode (halting CPU)");

    // Trigger debug halt
    if let Some(debug_regs) = get_debug_registers() {
        // Write halt request to DCSR
        let mut dcsr = debug_regs.read_dcsr();
        dcsr.set_halt(true);
        debug_regs.write_dcsr(dcsr);

        // Wait for CPU to halt
        while !dcsr.halted() {
            dcsr = debug_regs.read_dcsr();
            // Spin wait
            riscv::asm::pause();
        }
    }

    log::debug!("CPU halted in debug mode");
    Ok(())
}

/// Exit debug mode (resume the CPU)
pub fn exit_debug_mode() -> Result<(), &'static str> {
    log::debug!("Exiting debug mode (resuming CPU)");

    // Clear halt request
    if let Some(debug_regs) = get_debug_registers() {
        let mut dcsr = debug_regs.read_dcsr();
        dcsr.set_halt(false);
        debug_regs.write_dcsr(dcsr);
    }

    log::debug!("CPU resumed from debug mode");
    Ok(())
}

/// Set hardware breakpoint
pub fn set_breakpoint(addr: usize, bp_type: BreakpointType) -> Result<u32, &'static str> {
    log::debug!("Setting breakpoint at address {:#x}", addr);

    if let Some(bp_manager) = get_breakpoint_manager() {
        let bp_id = bp_manager.set_breakpoint(addr, bp_type)?;
        log::debug!("Breakpoint {} set at address {:#x}", bp_id, addr);
        Ok(bp_id)
    } else {
        Err("Breakpoint manager not initialized")
    }
}

/// Clear hardware breakpoint
pub fn clear_breakpoint(bp_id: u32) -> Result<(), &'static str> {
    log::debug!("Clearing breakpoint {}", bp_id);

    if let Some(bp_manager) = get_breakpoint_manager() {
        bp_manager.clear_breakpoint(bp_id)?;
        log::debug!("Breakpoint {} cleared", bp_id);
        Ok(())
    } else {
        Err("Breakpoint manager not initialized")
    }
}

/// Enable single stepping
pub fn enable_single_step() -> Result<(), &'static str> {
    log::debug!("Enabling single stepping");

    if let Some(debug_regs) = get_debug_registers() {
        let mut dcsr = debug_regs.read_dcsr();
        dcsr.set_step(true);
        debug_regs.write_dcsr(dcsr);
    }

    log::debug!("Single stepping enabled");
    Ok(())
}

/// Disable single stepping
pub fn disable_single_step() -> Result<(), &'static str> {
    log::debug!("Disabling single stepping");

    if let Some(debug_regs) = get_debug_registers() {
        let mut dcsr = debug_regs.read_dcsr();
        dcsr.set_step(false);
        debug_regs.write_dcsr(dcsr);
    }

    log::debug!("Single stepping disabled");
    Ok(())
}

/// Check if single stepping is enabled
pub fn is_single_stepping() -> bool {
    if let Some(debug_regs) = get_debug_registers() {
        let dcsr = debug_regs.read_dcsr();
        dcsr.step()
    } else {
        false
    }
}

/// Step one instruction
pub fn step_instruction() -> Result<(), &'static str> {
    // Enable single stepping
    enable_single_step()?;

    // Resume execution
    exit_debug_mode()?;

    // Wait for debug exception
    // Note: This would typically be handled in the debug exception handler

    Ok(())
}

/// Start tracing
pub fn start_trace() -> Result<(), &'static str> {
    log::debug!("Starting program trace");

    if let Some(tracer) = get_tracer() {
        tracer.start()?;
        log::debug!("Program trace started");
        Ok(())
    } else {
        Err("Tracer not initialized")
    }
}

/// Stop tracing
pub fn stop_trace() -> Result<Vec<TraceEvent>> {
    log::debug!("Stopping program trace");

    if let Some(tracer) = get_tracer() {
        let events = tracer.stop()?;
        log::debug!("Program trace stopped, collected {} events", events.len());
        Ok(events)
    } else {
        Err("Tracer not initialized")
    }
}

/// Get current trace events (without stopping)
pub fn get_trace_events() -> Result<Vec<TraceEvent>> {
    if let Some(tracer) = get_tracer() {
        Ok(tracer.get_events()?)
    } else {
        Err("Tracer not initialized")
    }
}

/// Read register value
pub fn read_register(reg_id: u32) -> Result<u64, &'static str> {
    if let Some(debug_regs) = get_debug_registers() {
        debug_regs.read_register(reg_id)
    } else {
        Err("Debug registers not initialized")
    }
}

/// Write register value
pub fn write_register(reg_id: u32, value: u64) -> Result<(), &'static str> {
    log::debug!("Writing value {:#x} to register {}", value, reg_id);

    if let Some(debug_regs) = get_debug_registers() {
        debug_regs.write_register(reg_id, value)?;
        log::debug!("Register {} written successfully", reg_id);
        Ok(())
    } else {
        Err("Debug registers not initialized")
    }
}

/// Read memory
pub fn read_memory(addr: usize, size: usize) -> Result<Vec<u8>, &'static str> {
    log::debug!("Reading {} bytes from address {:#x}", size, addr);

    // Validate address
    if !crate::arch::riscv64::mmu::is_valid_address(addr) {
        return Err("Invalid memory address");
    }

    let mut data = Vec::with_capacity(size);
    unsafe {
        let src = addr as *const u8;
        for i in 0..size {
            data.push(core::ptr::read_volatile(src.add(i)));
        }
    }

    log::debug!("Read {} bytes from address {:#x}", size, addr);
    Ok(data)
}

/// Write memory
pub fn write_memory(addr: usize, data: &[u8]) -> Result<(), &'static str> {
    log::debug!("Writing {} bytes to address {:#x}", data.len(), addr);

    // Validate address
    if !crate::arch::riscv64::mmu::is_valid_address(addr) {
        return Err("Invalid memory address");
    }

    unsafe {
        let dst = addr as *mut u8;
        for (i, &byte) in data.iter().enumerate() {
            core::ptr::write_volatile(dst.add(i), byte);
        }
    }

    log::debug!("Written {} bytes to address {:#x}", data.len(), addr);
    Ok(())
}

/// Generate core dump
pub fn generate_core_dump() -> Result<CoreDump, &'static str> {
    log::info!("Generating core dump");

    let mut core_dump = CoreDump::new();

    // Capture CPU state
    if let Some(debug_regs) = get_debug_registers() {
        core_dump.cpu_state = Some(debug_regs.capture_cpu_state()?);
    }

    // Capture memory regions
    core_dump.memory_regions = get_memory_regions();

    // Capture trace events if available
    if let Some(tracer) = get_tracer() {
        core_dump.trace_events = tracer.get_events().ok();
    }

    log::info!("Core dump generated successfully");
    Ok(core_dump)
}

/// Get memory regions for debugging
fn get_memory_regions() -> Vec<MemoryRegion> {
    let mut regions = Vec::new();

    // Add kernel memory region
    regions.push(MemoryRegion {
        base: 0x80000000,
        size: 0x10000000, // 256MB
        permissions: MemoryPermissions::ReadWriteExecute,
        name: "kernel".to_string(),
    });

    // Add device tree region
    regions.push(MemoryRegion {
        base: 0x41000000,
        size: 0x00100000, // 1MB
        permissions: MemoryPermissions::ReadOnly,
        name: "device_tree".to_string(),
    });

    regions
}

/// Memory region information
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    /// Base address
    pub base: u64,
    /// Size
    pub size: u64,
    /// Memory permissions
    pub permissions: MemoryPermissions,
    /// Region name
    pub name: String,
}

/// Memory permissions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPermissions {
    /// Read only
    ReadOnly,
    /// Read/Write
    ReadWrite,
    /// Read/Execute
    ReadExecute,
    /// Read/Write/Execute
    ReadWriteExecute,
}

/// Core dump structure
#[derive(Debug, Clone)]
pub struct CoreDump {
    /// CPU state at time of crash
    pub cpu_state: Option<CpuState>,
    /// Memory regions
    pub memory_regions: Vec<MemoryRegion>,
    /// Trace events leading to crash
    pub trace_events: Option<Vec<TraceEvent>>,
}

impl CoreDump {
    /// Create new core dump
    pub fn new() -> Self {
        Self {
            cpu_state: None,
            memory_regions: Vec::new(),
            trace_events: None,
        }
    }

    /// Save core dump to file
    pub fn save_to_file(&self, _path: &str) -> Result<(), &'static str> {
        // TODO: Implement core dump file saving
        Ok(())
    }

    /// Load core dump from file
    pub fn load_from_file(_path: &str) -> Result<Self, &'static str> {
        // TODO: Implement core dump file loading
        Err("Core dump loading not yet implemented")
    }
}

/// Debug statistics
#[derive(Debug, Clone, Default)]
pub struct DebugStats {
    /// Number of breakpoints hit
    pub breakpoints_hit: u64,
    /// Number of watchpoints hit
    pub watchpoints_hit: u64,
    /// Number of single steps taken
    pub single_steps: u64,
    /// Number of trace events collected
    pub trace_events: u64,
    /// Time spent in debug mode (in microseconds)
    pub debug_time_us: u64,
}

/// Get debug statistics
pub fn get_debug_stats() -> DebugStats {
    // TODO: Collect actual statistics
    DebugStats::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_config() {
        let config = DebugConfig::default();
        assert!(config.enabled);
        assert_eq!(config.hw_breakpoints, 16);
        assert_eq!(config.hw_watchpoints, 8);
        assert!(config.enable_trace);
        assert!(config.enable_jtag);
        assert!(config.enable_vm_debug);
    }

    #[test]
    fn test_memory_region() {
        let region = MemoryRegion {
            base: 0x80000000,
            size: 0x1000000,
            permissions: MemoryPermissions::ReadWriteExecute,
            name: "test".to_string(),
        };
        assert_eq!(region.base, 0x80000000);
        assert_eq!(region.size, 0x1000000);
        assert_eq!(region.permissions, MemoryPermissions::ReadWriteExecute);
        assert_eq!(region.name, "test");
    }

    #[test]
    fn test_core_dump() {
        let dump = CoreDump::new();
        assert!(dump.cpu_state.is_none());
        assert!(dump.memory_regions.is_empty());
        assert!(dump.trace_events.is_none());
    }
}