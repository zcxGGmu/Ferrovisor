//! RISC-V VM Debug Support
//!
//! This module provides debugging support for virtual machines including:
//! - VM breakpoint management
//! - VM memory inspection
//! - VM register state access
//! - VM single stepping
//! - VM trace collection

use crate::arch::riscv64::*;
use crate::arch::riscv64::debug::*;
use crate::arch::riscv64::virtualization::vm::{Vm, VmId};

/// VM debug configuration
#[derive(Debug, Clone)]
pub struct VmDebugConfig {
    /// Enable debug for this VM
    pub enabled: bool,
    /// Max breakpoints per VM
    pub max_breakpoints: u32,
    /// Max watchpoints per VM
    pub max_watchpoints: u32,
    /// Enable instruction tracing
    pub enable_instruction_trace: bool,
    /// Enable memory access tracing
    pub enable_memory_trace: bool,
    /// Enable exception tracing
    pub enable_exception_trace: bool,
    /// Trace buffer size
    pub trace_buffer_size: usize,
}

impl Default for VmDebugConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_breakpoints: 16,
            max_watchpoints: 8,
            enable_instruction_trace: false,
            enable_memory_trace: false,
            enable_exception_trace: true,
            trace_buffer_size: 32768, // 32KB
        }
    }
}

/// VM debug context
#[derive(Debug, Clone)]
pub struct VmDebugContext {
    /// VM ID
    pub vm_id: VmId,
    /// Debug configuration
    pub config: VmDebugConfig,
    /// VM breakpoints
    pub breakpoints: Vec<VmBreakpoint>,
    /// VM watchpoints
    pub watchpoints: Vec<VmWatchpoint>,
    /// Trace buffer
    pub trace_buffer: Vec<VmTraceEvent>,
    /// Debug statistics
    pub stats: VmDebugStats,
    /// Is VM currently halted
    pub halted: bool,
}

impl VmDebugContext {
    /// Create new VM debug context
    pub fn new(vm_id: VmId) -> Self {
        Self {
            vm_id,
            config: VmDebugConfig::default(),
            breakpoints: Vec::new(),
            watchpoints: Vec::new(),
            trace_buffer: Vec::new(),
            stats: VmDebugStats::default(),
            halted: false,
        }
    }

    /// Create VM debug context with custom config
    pub fn with_config(vm_id: VmId, config: VmDebugConfig) -> Self {
        Self {
            vm_id,
            config,
            breakpoints: Vec::new(),
            watchpoints: Vec::new(),
            trace_buffer: Vec::new(),
            stats: VmDebugStats::default(),
            halted: false,
        }
    }

    /// Configure debug settings
    pub fn configure(&mut self, config: VmDebugConfig) {
        self.config = config;
    }

    /// Add breakpoint
    pub fn add_breakpoint(&mut self, addr: u64) -> Result<u32, &'static str> {
        if self.breakpoints.len() >= self.config.max_breakpoints as usize {
            return Err("Maximum breakpoints reached");
        }

        let id = self.breakpoints.len() as u32;
        let bp = VmBreakpoint::new(id, addr);
        self.breakpoints.push(bp);

        Ok(id)
    }

    /// Remove breakpoint
    pub fn remove_breakpoint(&mut self, id: u32) -> Result<(), &'static str> {
        let index = self.breakpoints.iter()
            .position(|bp| bp.id == id)
            .ok_or("Breakpoint not found")?;

        self.breakpoints.remove(index);
        Ok(())
    }

    /// Add watchpoint
    pub fn add_watchpoint(&mut self, addr: u64, size: u32, access_type: MemoryAccessType) -> Result<u32, &'static str> {
        if self.watchpoints.len() >= self.config.max_watchpoints as usize {
            return Err("Maximum watchpoints reached");
        }

        let id = self.watchpoints.len() as u32;
        let wp = VmWatchpoint::new(id, addr, size, access_type);
        self.watchpoints.push(wp);

        Ok(id)
    }

    /// Remove watchpoint
    pub fn remove_watchpoint(&mut self, id: u32) -> Result<(), &'static str> {
        let index = self.watchpoints.iter()
            .position(|wp| wp.id == id)
            .ok_or("Watchpoint not found")?;

        self.watchpoints.remove(index);
        Ok(())
    }

    /// Check if address hits any breakpoint
    pub fn check_breakpoint(&mut self, addr: u64) -> Option<u32> {
        for bp in &mut self.breakpoints {
            if bp.enabled && bp.matches(addr) {
                bp.hit();
                self.stats.breakpoints_hit += 1;
                return Some(bp.id);
            }
        }
        None
    }

    /// Check if memory access hits any watchpoint
    pub fn check_watchpoint(&mut self, addr: u64, size: u32, access_type: MemoryAccessType) -> Option<u32> {
        for wp in &mut self.watchpoints {
            if wp.enabled && wp.matches(addr, size, access_type) {
                wp.hit();
                self.stats.watchpoints_hit += 1;
                return Some(wp.id);
            }
        }
        None
    }

    /// Add trace event
    pub fn add_trace_event(&mut self, event: VmTraceEvent) {
        // Check buffer size limit
        if self.trace_buffer.len() >= self.config.trace_buffer_size {
            self.trace_buffer.remove(0); // Remove oldest event
            self.stats.trace_dropped += 1;
        }

        // Check if event type is enabled
        if !self.is_trace_enabled(event.event_type) {
            return;
        }

        self.trace_buffer.push(event.clone());

        // Update statistics
        match event.event_type {
            VmTraceEventType::Instruction => self.stats.instructions_traced += 1,
            VmTraceEventType::MemoryRead | VmTraceEventType::MemoryWrite => self.stats.memory_accesses_traced += 1,
            VmTraceEventType::Exception => self.stats.exceptions_traced += 1,
            _ => {}
        }
    }

    /// Check if trace event type is enabled
    fn is_trace_enabled(&self, event_type: VmTraceEventType) -> bool {
        match event_type {
            VmTraceEventType::Instruction => self.config.enable_instruction_trace,
            VmTraceEventType::MemoryRead | VmTraceEventType::MemoryWrite => self.config.enable_memory_trace,
            VmTraceEventType::Exception => self.config.enable_exception_trace,
        }
    }

    /// Get trace events
    pub fn get_trace_events(&self) -> &[VmTraceEvent] {
        &self.trace_buffer
    }

    /// Clear trace buffer
    pub fn clear_trace(&mut self) {
        self.trace_buffer.clear();
    }

    /// Get breakpoint by ID
    pub fn get_breakpoint(&self, id: u32) -> Option<&VmBreakpoint> {
        self.breakpoints.iter().find(|bp| bp.id == id)
    }

    /// Get breakpoint by ID (mutable)
    pub fn get_breakpoint_mut(&mut self, id: u32) -> Option<&mut VmBreakpoint> {
        self.breakpoints.iter_mut().find(|bp| bp.id == id)
    }

    /// Get all breakpoints
    pub fn get_breakpoints(&self) -> &[VmBreakpoint] {
        &self.breakpoints
    }

    /// Get watchpoint by ID
    pub fn get_watchpoint(&self, id: u32) -> Option<&VmWatchpoint> {
        self.watchpoints.iter().find(|wp| wp.id == id)
    }

    /// Get watchpoint by ID (mutable)
    pub fn get_watchpoint_mut(&mut self, id: u32) -> Option<&mut VmWatchpoint> {
        self.watchpoints.iter_mut().find(|wp| wp.id == id)
    }

    /// Get all watchpoints
    pub fn get_watchpoints(&self) -> &[VmWatchpoint] {
        &self.watchpoints
    }

    /// Get debug statistics
    pub fn get_stats(&self) -> &VmDebugStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = VmDebugStats::default();
    }

    /// Halt the VM
    pub fn halt(&mut self) {
        self.halted = true;
        self.stats.halt_count += 1;
    }

    /// Resume the VM
    pub fn resume(&mut self) {
        self.halted = false;
        self.stats.resume_count += 1;
    }

    /// Single step the VM
    pub fn single_step(&mut self) {
        self.stats.single_step_count += 1;
        // Note: Actual single stepping would be handled by the VCPU
    }
}

/// VM breakpoint
#[derive(Debug, Clone)]
pub struct VmBreakpoint {
    /// Breakpoint ID
    pub id: u32,
    /// Breakpoint address
    pub address: u64,
    /// Is breakpoint enabled
    pub enabled: bool,
    /// Hit count
    pub hit_count: u64,
    /// Is temporary
    pub temporary: bool,
    /// Breakpoint condition
    pub condition: Option<String>,
}

impl VmBreakpoint {
    /// Create new VM breakpoint
    pub fn new(id: u32, address: u64) -> Self {
        Self {
            id,
            address,
            enabled: true,
            hit_count: 0,
            temporary: false,
            condition: None,
        }
    }

    /// Check if address matches this breakpoint
    pub fn matches(&self, addr: u64) -> bool {
        self.address == addr
    }

    /// Trigger breakpoint hit
    pub fn hit(&mut self) {
        self.hit_count += 1;
    }

    /// Enable/disable breakpoint
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Set as temporary
    pub fn set_temporary(&mut self, temporary: bool) {
        self.temporary = temporary;
    }

    /// Set condition
    pub fn set_condition(&mut self, condition: String) {
        self.condition = Some(condition);
    }
}

/// Memory access type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryAccessType {
    /// Read access
    Read,
    /// Write access
    Write,
    /// Read/write access
    ReadWrite,
}

/// VM watchpoint
#[derive(Debug, Clone)]
pub struct VmWatchpoint {
    /// Watchpoint ID
    pub id: u32,
    /// Watchpoint address
    pub address: u64,
    /// Watchpoint size
    pub size: u32,
    /// Access type
    pub access_type: MemoryAccessType,
    /// Is watchpoint enabled
    pub enabled: bool,
    /// Hit count
    pub hit_count: u64,
}

impl VmWatchpoint {
    /// Create new VM watchpoint
    pub fn new(id: u32, address: u64, size: u32, access_type: MemoryAccessType) -> Self {
        Self {
            id,
            address,
            size,
            access_type,
            enabled: true,
            hit_count: 0,
        }
    }

    /// Check if memory access matches this watchpoint
    pub fn matches(&self, addr: u64, size: u32, access_type: MemoryAccessType) -> bool {
        // Check address range
        if addr < self.address || addr >= self.address + self.size as u64 {
            return false;
        }

        // Check if access overlaps with watchpoint
        let access_end = addr + size as u64;
        let wp_end = self.address + self.size as u64;
        if access_end <= self.address || wp_end <= addr {
            return false;
        }

        // Check access type
        match self.access_type {
            MemoryAccessType::Read => access_type == MemoryAccessType::Read,
            MemoryAccessType::Write => access_type == MemoryAccessType::Write,
            MemoryAccessType::ReadWrite => true,
        }
    }

    /// Trigger watchpoint hit
    pub fn hit(&mut self) {
        self.hit_count += 1;
    }

    /// Enable/disable watchpoint
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

/// VM trace event type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmTraceEventType {
    /// Instruction execution
    Instruction,
    /// Memory read
    MemoryRead,
    /// Memory write
    MemoryWrite,
    /// Exception
    Exception,
    /// Interrupt
    Interrupt,
    /// Context switch
    ContextSwitch,
    /// VM exit
    VmExit,
    /// VM entry
    VmEntry,
}

/// VM trace event
#[derive(Debug, Clone)]
pub struct VmTraceEvent {
    /// Event type
    pub event_type: VmTraceEventType,
    /// Timestamp
    pub timestamp: u64,
    /// VCPU ID
    pub vcpu_id: u32,
    /// Program counter
    pub pc: u64,
    /// Event-specific data
    pub data: u64,
    /// Additional info
    pub info: Option<String>,
}

impl VmTraceEvent {
    /// Create new VM trace event
    pub fn new(event_type: VmTraceEventType, vcpu_id: u32, pc: u64, data: u64) -> Self {
        Self {
            event_type,
            timestamp: get_vm_timestamp(),
            vcpu_id,
            pc,
            data,
            info: None,
        }
    }

    /// Create instruction trace event
    pub fn instruction(vcpu_id: u32, pc: u64, instruction: u32) -> Self {
        Self::new(VmTraceEventType::Instruction, vcpu_id, pc, instruction as u64)
    }

    /// Create memory read trace event
    pub fn memory_read(vcpu_id: u32, pc: u64, addr: u64, size: u32) -> Self {
        let data = addr | ((size as u64) << 56);
        Self::new(VmTraceEventType::MemoryRead, vcpu_id, pc, data)
    }

    /// Create memory write trace event
    pub fn memory_write(vcpu_id: u32, pc: u64, addr: u64, size: u32) -> Self {
        let data = addr | ((size as u64) << 56);
        Self::new(VmTraceEventType::MemoryWrite, vcpu_id, pc, data)
    }

    /// Create exception trace event
    pub fn exception(vcpu_id: u32, pc: u64, cause: u32) -> Self {
        Self::new(VmTraceEventType::Exception, vcpu_id, pc, cause as u64)
    }

    /// Create VM exit trace event
    pub fn vm_exit(vcpu_id: u32, pc: u64, exit_reason: u32) -> Self {
        Self::new(VmTraceEventType::VmExit, vcpu_id, pc, exit_reason as u64)
    }

    /// Get address from memory event data
    pub fn get_memory_address(&self) -> Option<u64> {
        match self.event_type {
            VmTraceEventType::MemoryRead | VmTraceEventType::MemoryWrite => {
                Some(self.data & 0x00FFFFFFFFFFFFFF)
            }
            _ => None,
        }
    }

    /// Get size from memory event data
    pub fn get_memory_size(&self) -> Option<u32> {
        match self.event_type {
            VmTraceEventType::MemoryRead | VmTraceEventType::MemoryWrite => {
                Some((self.data >> 56) as u32)
            }
            _ => None,
        }
    }

    /// Get exception cause
    pub fn get_exception_cause(&self) -> Option<u32> {
        match self.event_type {
            VmTraceEventType::Exception => Some(self.data as u32),
            _ => None,
        }
    }

    /// Get VM exit reason
    pub fn get_vm_exit_reason(&self) -> Option<u32> {
        match self.event_type {
            VmTraceEventType::VmExit => Some(self.data as u32),
            _ => None,
        }
    }
}

/// VM debug statistics
#[derive(Debug, Clone, Default)]
pub struct VmDebugStats {
    /// Instructions traced
    pub instructions_traced: u64,
    /// Memory accesses traced
    pub memory_accesses_traced: u64,
    /// Exceptions traced
    pub exceptions_traced: u64,
    /// Breakpoints hit
    pub breakpoints_hit: u64,
    /// Watchpoints hit
    pub watchpoints_hit: u64,
    /// Halt count
    pub halt_count: u64,
    /// Resume count
    pub resume_count: u64,
    /// Single step count
    pub single_step_count: u64,
    /// Trace events dropped
    pub trace_dropped: u64,
}

/// VM debug manager
pub struct VmDebugManager {
    /// VM debug contexts
    vm_contexts: spin::Mutex<std::collections::HashMap<VmId, VmDebugContext>>,
    /// Global debug configuration
    global_config: VmDebugConfig,
}

impl VmDebugManager {
    /// Create new VM debug manager
    pub fn new() -> Self {
        Self {
            vm_contexts: spin::Mutex::new(std::collections::HashMap::new()),
            global_config: VmDebugConfig::default(),
        }
    }

    /// Create new VM debug context
    pub fn create_vm_context(&self, vm_id: VmId) -> Result<VmDebugContext, &'static str> {
        let context = VmDebugContext::new(vm_id);
        self.register_vm_context(vm_id, context.clone())?;
        Ok(context)
    }

    /// Create VM debug context with config
    pub fn create_vm_context_with_config(&self, vm_id: VmId, config: VmDebugConfig) -> Result<VmDebugContext, &'static str> {
        let context = VmDebugContext::with_config(vm_id, config);
        self.register_vm_context(vm_id, context.clone())?;
        Ok(context)
    }

    /// Register VM debug context
    fn register_vm_context(&self, vm_id: VmId, context: VmDebugContext) -> Result<(), &'static str> {
        let mut contexts = self.vm_contexts.lock();
        if contexts.contains_key(&vm_id) {
            return Err("VM debug context already exists");
        }
        contexts.insert(vm_id, context);
        Ok(())
    }

    /// Remove VM debug context
    pub fn remove_vm_context(&self, vm_id: VmId) -> Option<VmDebugContext> {
        let mut contexts = self.vm_contexts.lock();
        contexts.remove(&vm_id)
    }

    /// Get VM debug context
    pub fn get_vm_context(&self, vm_id: VmId) -> Option<VmDebugContext> {
        let contexts = self.vm_contexts.lock();
        contexts.get(&vm_id).cloned()
    }

    /// Update VM debug context
    pub fn update_vm_context<F>(&self, vm_id: VmId, updater: F) -> Result<(), &'static str>
    where
        F: FnOnce(&mut VmDebugContext),
    {
        let mut contexts = self.vm_contexts.lock();
        let context = contexts.get_mut(&vm_id).ok_or("VM debug context not found")?;
        updater(context);
        Ok(())
    }

    /// Get all VM debug contexts
    pub fn get_all_contexts(&self) -> Vec<VmDebugContext> {
        let contexts = self.vm_contexts.lock();
        contexts.values().cloned().collect()
    }

    /// Get global debug configuration
    pub fn get_global_config(&self) -> &VmDebugConfig {
        &self.global_config
    }

    /// Set global debug configuration
    pub fn set_global_config(&mut self, config: VmDebugConfig) {
        self.global_config = config;
    }

    /// Get global debug statistics
    pub fn get_global_stats(&self) -> VmDebugStats {
        let contexts = self.vm_contexts.lock();
        let mut total_stats = VmDebugStats::default();

        for context in contexts.values() {
            let stats = context.get_stats();
            total_stats.instructions_traced += stats.instructions_traced;
            total_stats.memory_accesses_traced += stats.memory_accesses_traced;
            total_stats.exceptions_traced += stats.exceptions_traced;
            total_stats.breakpoints_hit += stats.breakpoints_hit;
            total_stats.watchpoints_hit += stats.watchpoints_hit;
            total_stats.halt_count += stats.halt_count;
            total_stats.resume_count += stats.resume_count;
            total_stats.single_step_count += stats.single_step_count;
            total_stats.trace_dropped += stats.trace_dropped;
        }

        total_stats
    }
}

/// Global VM debug manager
static VM_DEBUG_MANAGER: spin::Once<VmDebugManager> = spin::Once::new();

/// Get VM debug manager
pub fn get_vm_debug_manager() -> &'static VmDebugManager {
    VM_DEBUG_MANAGER.call_once(|| VmDebugManager::new())
}

/// Initialize VM debug support
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing RISC-V VM debug support");

    // Initialize global VM debug manager
    let _manager = get_vm_debug_manager();

    log::info!("RISC-V VM debug support initialized");
    Ok(())
}

/// Get VM timestamp
fn get_vm_timestamp() -> u64 {
    // In a real implementation, this would read from a hardware timer
    use core::sync::atomic::{AtomicU64, Ordering};
    static VM_TIMESTAMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    VM_TIMESTAMP_COUNTER.fetch_add(1, Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_debug_config() {
        let config = VmDebugConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_breakpoints, 16);
        assert_eq!(config.max_watchpoints, 8);
        assert!(!config.enable_instruction_trace);
        assert!(!config.enable_memory_trace);
        assert!(config.enable_exception_trace);
        assert_eq!(config.trace_buffer_size, 32768);
    }

    #[test]
    fn test_vm_debug_context() {
        let vm_id = VmId::new(1);
        let mut context = VmDebugContext::new(vm_id);
        assert_eq!(context.vm_id, vm_id);
        assert!(!context.halted);
        assert!(context.breakpoints.is_empty());
        assert!(context.watchpoints.is_empty());
        assert!(context.trace_buffer.is_empty());
    }

    #[test]
    fn test_vm_breakpoint() {
        let mut bp = VmBreakpoint::new(0, 0x80000000);
        assert_eq!(bp.id, 0);
        assert_eq!(bp.address, 0x80000000);
        assert!(bp.enabled);
        assert_eq!(bp.hit_count, 0);
        assert!(!bp.temporary);
        assert!(bp.condition.is_none());

        assert!(bp.matches(0x80000000));
        assert!(!bp.matches(0x80000004));

        bp.hit();
        assert_eq!(bp.hit_count, 1);

        bp.set_enabled(false);
        assert!(!bp.enabled);
    }

    #[test]
    fn test_vm_watchpoint() {
        let mut wp = VmWatchpoint::new(0, 0x10000000, 0x1000, MemoryAccessType::Write);
        assert_eq!(wp.id, 0);
        assert_eq!(wp.address, 0x10000000);
        assert_eq!(wp.size, 0x1000);
        assert_eq!(wp.access_type, MemoryAccessType::Write);
        assert!(wp.enabled);
        assert_eq!(wp.hit_count, 0);

        // Test matching
        assert!(wp.matches(0x10000000, 4, MemoryAccessType::Write));
        assert!(wp.matches(0x1000F00, 1, MemoryAccessType::Write));
        assert!(!wp.matches(0x1000F00, 1, MemoryAccessType::Read));
        assert!(!wp.matches(0x20000000, 4, MemoryAccessType::Write));

        wp.hit();
        assert_eq!(wp.hit_count, 1);
    }

    #[test]
    fn test_vm_trace_event() {
        let event = VmTraceEvent::instruction(0, 0x80000000, 0x00000013);
        assert_eq!(event.event_type, VmTraceEventType::Instruction);
        assert_eq!(event.vcpu_id, 0);
        assert_eq!(event.pc, 0x80000000);
        assert_eq!(event.data, 0x00000013);

        let mem_event = VmTraceEvent::memory_read(0, 0x80000004, 0x10000000, 4);
        assert_eq!(mem_event.event_type, VmTraceEventType::MemoryRead);
        assert_eq!(mem_event.get_memory_address(), Some(0x10000000));
        assert_eq!(mem_event.get_memory_size(), Some(4));

        let exc_event = VmTraceEvent::exception(0, 0x80000008, 8);
        assert_eq!(exc_event.event_type, VmTraceEventType::Exception);
        assert_eq!(exc_event.get_exception_cause(), Some(8));
    }

    #[test]
    fn test_vm_debug_stats() {
        let stats = VmDebugStats::default();
        assert_eq!(stats.instructions_traced, 0);
        assert_eq!(stats.memory_accesses_traced, 0);
        assert_eq!(stats.exceptions_traced, 0);
        assert_eq!(stats.breakpoints_hit, 0);
        assert_eq!(stats.watchpoints_hit, 0);
        assert_eq!(stats.halt_count, 0);
        assert_eq!(stats.resume_count, 0);
        assert_eq!(stats.single_step_count, 0);
        assert_eq!(stats.trace_dropped, 0);
    }

    #[test]
    fn test_vm_debug_context_operations() {
        let mut context = VmDebugContext::new(VmId::new(1));

        // Add breakpoint
        let bp_id = context.add_breakpoint(0x80000000).unwrap();
        assert_eq!(bp_id, 0);
        assert_eq!(context.breakpoints.len(), 1);

        // Check breakpoint hit
        let hit_bp = context.check_breakpoint(0x80000000);
        assert_eq!(hit_bp, Some(bp_id));
        assert_eq!(context.breakpoints[0].hit_count, 1);

        // Add watchpoint
        let wp_id = context.add_watchpoint(0x10000000, 0x1000, MemoryAccessType::Write).unwrap();
        assert_eq!(wp_id, 0);
        assert_eq!(context.watchpoints.len(), 1);

        // Check watchpoint hit
        let hit_wp = context.check_watchpoint(0x10000100, 4, MemoryAccessType::Write);
        assert_eq!(hit_wp, Some(wp_id));
        assert_eq!(context.watchpoints[0].hit_count, 1);

        // Add trace event
        let event = VmTraceEvent::instruction(0, 0x80000000, 0x00000013);
        context.add_trace_event(event);
        assert_eq!(context.trace_buffer.len(), 1);
        assert_eq!(context.stats.instructions_traced, 1);

        // Halt and resume
        assert!(!context.halted);
        context.halt();
        assert!(context.halted);
        assert_eq!(context.stats.halt_count, 1);

        context.resume();
        assert!(!context.halted);
        assert_eq!(context.stats.resume_count, 1);

        // Single step
        context.single_step();
        assert_eq!(context.stats.single_step_count, 1);
    }

    #[test]
    fn test_vm_debug_manager() {
        let manager = VmDebugManager::new();
        let vm_id = VmId::new(1);

        // Create VM context
        let context = manager.create_vm_context(vm_id).unwrap();
        assert_eq!(context.vm_id, vm_id);

        // Get VM context
        let retrieved = manager.get_vm_context(vm_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().vm_id, vm_id);

        // Update VM context
        manager.update_vm_context(vm_id, |ctx| ctx.halt()).unwrap();
        let updated = manager.get_vm_context(vm_id).unwrap();
        assert!(updated.halted);

        // Remove VM context
        let removed = manager.remove_vm_context(vm_id);
        assert!(removed.is_some());

        let not_found = manager.get_vm_context(vm_id);
        assert!(not_found.is_none());
    }
}