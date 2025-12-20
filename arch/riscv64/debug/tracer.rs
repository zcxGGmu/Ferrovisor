//! RISC-V Program Tracer
//!
//! This module provides program tracing functionality including:
//! - Instruction tracing
//! - Branch tracing
//! - Memory access tracing
//! - Exception and interrupt tracing
//! - Trace buffer management

use crate::arch::riscv64::*;

/// Trace event type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceEventType {
    /// Instruction execution
    Instruction,
    /// Branch taken
    BranchTaken,
    /// Branch not taken
    BranchNotTaken,
    /// Memory read
    MemoryRead,
    /// Memory write,
    MemoryWrite,
    /// Exception taken
    Exception,
    /// Interrupt taken
    Interrupt,
    /// Context switch
    ContextSwitch,
    /// Custom event
    Custom,
}

/// Trace event
#[derive(Debug, Clone)]
pub struct TraceEvent {
    /// Event type
    pub event_type: TraceEventType,
    /// Timestamp
    pub timestamp: u64,
    /// Program counter
    pub pc: u64,
    /// Event-specific data
    pub data: u64,
    /// Additional info
    pub info: Option<String>,
}

impl TraceEvent {
    /// Create new trace event
    pub fn new(event_type: TraceEventType, pc: u64, data: u64) -> Self {
        Self {
            event_type,
            timestamp: get_timestamp(),
            pc,
            data,
            info: None,
        }
    }

    /// Create instruction trace event
    pub fn instruction(pc: u64, instruction: u32) -> Self {
        Self::new(TraceEventType::Instruction, pc, instruction as u64)
    }

    /// Create branch taken event
    pub fn branch_taken(pc: u64, target: u64) -> Self {
        Self::new(TraceEventType::BranchTaken, pc, target)
    }

    /// Create branch not taken event
    pub fn branch_not_taken(pc: u64) -> Self {
        Self::new(TraceEventType::BranchNotTaken, pc, pc + 4) // Assume 4-byte instruction
    }

    /// Create memory read event
    pub fn memory_read(pc: u64, addr: u64, size: u32) -> Self {
        Self::new(TraceEventType::MemoryRead, pc, addr | ((size as u64) << 56))
    }

    /// Create memory write event
    pub fn memory_write(pc: u64, addr: u64, size: u32) -> Self {
        Self::new(TraceEventType::MemoryWrite, pc, addr | ((size as u64) << 56))
    }

    /// Create exception event
    pub fn exception(pc: u64, cause: u32) -> Self {
        Self::new(TraceEventType::Exception, pc, cause as u64)
    }

    /// Create interrupt event
    pub fn interrupt(pc: u64, irq: u32) -> Self {
        Self::new(TraceEventType::Interrupt, pc, irq as u64)
    }

    /// Create context switch event
    pub fn context_switch(old_pc: u64, new_pc: u64) -> Self {
        Self::new(TraceEventType::ContextSwitch, old_pc, new_pc)
    }

    /// Get address from memory event data
    pub fn get_memory_address(&self) -> Option<u64> {
        match self.event_type {
            TraceEventType::MemoryRead | TraceEventType::MemoryWrite => {
                Some(self.data & 0x00FFFFFFFFFFFFFF)
            }
            _ => None,
        }
    }

    /// Get size from memory event data
    pub fn get_memory_size(&self) -> Option<u32> {
        match self.event_type {
            TraceEventType::MemoryRead | TraceEventType::MemoryWrite => {
                Some((self.data >> 56) as u32)
            }
            _ => None,
        }
    }

    /// Get cause from exception event data
    pub fn get_exception_cause(&self) -> Option<u32> {
        match self.event_type {
            TraceEventType::Exception => Some(self.data as u32),
            _ => None,
        }
    }

    /// Get IRQ number from interrupt event data
    pub fn get_interrupt_irq(&self) -> Option<u32> {
        match self.event_type {
            TraceEventType::Interrupt => Some(self.data as u32),
            _ => None,
        }
    }

    /// Get target PC from branch event data
    pub fn get_branch_target(&self) -> Option<u64> {
        match self.event_type {
            TraceEventType::BranchTaken => Some(self.data),
            _ => None,
        }
    }

    /// Get new PC from context switch event data
    pub fn get_context_target(&self) -> Option<u64> {
        match self.event_type {
            TraceEventType::ContextSwitch => Some(self.data),
            _ => None,
        }
    }
}

/// Trace configuration
#[derive(Debug, Clone)]
pub struct TraceConfig {
    /// Trace buffer size
    pub buffer_size: usize,
    /// Enable instruction tracing
    pub trace_instructions: bool,
    /// Enable branch tracing
    pub trace_branches: bool,
    /// Enable memory tracing
    pub trace_memory: bool,
    /// Enable exception tracing
    pub trace_exceptions: bool,
    /// Enable interrupt tracing
    pub trace_interrupts: bool,
    /// Enable context switch tracing
    pub trace_context_switches: bool,
    /// Filter by address range
    pub address_filter: Option<(u64, u64)>,
    /// Event filter mask
    pub event_filter: u32,
}

impl Default for TraceConfig {
    fn default() -> Self {
        Self {
            buffer_size: 65536, // 64KB
            trace_instructions: true,
            trace_branches: true,
            trace_memory: false,
            trace_exceptions: true,
            trace_interrupts: true,
            trace_context_switches: true,
            address_filter: None,
            event_filter: 0xFFFFFFFF, // All events enabled
        }
    }
}

/// Trace buffer
struct TraceBuffer {
    /// Event storage
    events: Vec<TraceEvent>,
    /// Write index
    write_index: usize,
    /// Read index
    read_index: usize,
    /// Number of events in buffer
    count: usize,
    /// Total events written
    total_written: u64,
    /// Buffer overrun count
    overruns: u64,
}

impl TraceBuffer {
    /// Create new trace buffer
    fn new(size: usize) -> Self {
        Self {
            events: vec![TraceEvent::instruction(0, 0); size],
            write_index: 0,
            read_index: 0,
            count: 0,
            total_written: 0,
            overruns: 0,
        }
    }

    /// Push event to buffer
    fn push(&mut self, event: TraceEvent) -> bool {
        let success = self.count < self.events.len();

        self.events[self.write_index] = event;
        self.write_index = (self.write_index + 1) % self.events.len();
        self.total_written += 1;

        if success {
            self.count += 1;
        } else {
            self.overruns += 1;
            // Drop oldest event
            self.read_index = (self.read_index + 1) % self.events.len();
        }

        success
    }

    /// Pop event from buffer
    fn pop(&mut self) -> Option<TraceEvent> {
        if self.count == 0 {
            return None;
        }

        let event = self.events[self.read_index].clone();
        self.read_index = (self.read_index + 1) % self.events.len();
        self.count -= 1;

        Some(event)
    }

    /// Get all events from buffer (without removing)
    fn peek_all(&self) -> Vec<TraceEvent> {
        let mut result = Vec::with_capacity(self.count);
        let mut index = self.read_index;

        for _ in 0..self.count {
            result.push(self.events[index].clone());
            index = (index + 1) % self.events.len();
        }

        result
    }

    /// Clear buffer
    fn clear(&mut self) {
        self.write_index = 0;
        self.read_index = 0;
        self.count = 0;
        self.overruns = 0;
    }

    /// Get buffer statistics
    fn get_stats(&self) -> TraceBufferStats {
        TraceBufferStats {
            buffer_size: self.events.len(),
            events_in_buffer: self.count,
            total_events: self.total_written,
            overruns: self.overruns,
            utilization: (self.count as f64) / (self.events.len() as f64),
        }
    }
}

/// Trace buffer statistics
#[derive(Debug, Clone, Default)]
pub struct TraceBufferStats {
    /// Buffer size
    pub buffer_size: usize,
    /// Number of events currently in buffer
    pub events_in_buffer: usize,
    /// Total number of events written
    pub total_events: u64,
    /// Number of buffer overruns
    pub overruns: u64,
    /// Buffer utilization (0.0 to 1.0)
    pub utilization: f64,
}

/// Program tracer
pub struct Tracer {
    /// Trace configuration
    config: TraceConfig,
    /// Trace buffer
    buffer: TraceBuffer,
    /// Is tracing active
    active: bool,
    /// Trace statistics
    stats: TraceStats,
}

/// Trace statistics
#[derive(Debug, Clone, Default)]
pub struct TraceStats {
    /// Instructions traced
    pub instructions: u64,
    /// Branches traced
    pub branches: u64,
    /// Memory accesses traced
    pub memory_accesses: u64,
    /// Exceptions traced
    pub exceptions: u64,
    /// Interrupts traced
    pub interrupts: u64,
    /// Context switches traced
    pub context_switches: u64,
    /// Custom events traced
    pub custom_events: u64,
}

impl Tracer {
    /// Create new tracer
    pub fn new(buffer_size: usize) -> Result<Self, &'static str> {
        if buffer_size == 0 {
            return Err("Buffer size cannot be zero");
        }

        Ok(Self {
            config: TraceConfig::default(),
            buffer: TraceBuffer::new(buffer_size),
            active: false,
            stats: TraceStats::default(),
        })
    }

    /// Configure tracer
    pub fn configure(&mut self, config: TraceConfig) {
        self.config = config;
    }

    /// Get configuration
    pub fn get_config(&self) -> &TraceConfig {
        &self.config
    }

    /// Start tracing
    pub fn start(&mut self) -> Result<(), &'static str> {
        if self.active {
            return Err("Tracer already active");
        }

        // Clear buffer and stats
        self.buffer.clear();
        self.stats = TraceStats::default();

        self.active = true;
        log::debug!("Tracer started");
        Ok(())
    }

    /// Stop tracing
    pub fn stop(&mut self) -> Result<Vec<TraceEvent>, &'static str> {
        if !self.active {
            return Err("Tracer not active");
        }

        self.active = false;

        // Get all events from buffer
        let events = self.buffer.peek_all();

        log::debug!("Tracer stopped, collected {} events", events.len());
        Ok(events)
    }

    /// Check if tracing is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Trace event
    pub fn trace_event(&mut self, event: TraceEvent) {
        if !self.active {
            return;
        }

        // Check event filter
        let event_bit = 1 << (event.event_type as u32);
        if (self.config.event_filter & event_bit) == 0 {
            return;
        }

        // Check address filter
        if let Some((start, end)) = self.config.address_filter {
            if event.pc < start || event.pc > end {
                return;
            }
        }

        // Check specific event types
        match event.event_type {
            TraceEventType::Instruction if !self.config.trace_instructions => return,
            TraceEventType::BranchTaken | TraceEventType::BranchNotTaken
                if !self.config.trace_branches => return,
            TraceEventType::MemoryRead | TraceEventType::MemoryWrite
                if !self.config.trace_memory => return,
            TraceEventType::Exception if !self.config.trace_exceptions => return,
            TraceEventType::Interrupt if !self.config.trace_interrupts => return,
            TraceEventType::ContextSwitch if !self.config.trace_context_switches => return,
            _ => {}
        }

        // Add event to buffer
        if self.buffer.push(event.clone()) {
            // Update statistics
            match event.event_type {
                TraceEventType::Instruction => self.stats.instructions += 1,
                TraceEventType::BranchTaken | TraceEventType::BranchNotTaken => self.stats.branches += 1,
                TraceEventType::MemoryRead | TraceEventType::MemoryWrite => self.stats.memory_accesses += 1,
                TraceEventType::Exception => self.stats.exceptions += 1,
                TraceEventType::Interrupt => self.stats.interrupts += 1,
                TraceEventType::ContextSwitch => self.stats.context_switches += 1,
                TraceEventType::Custom => self.stats.custom_events += 1,
            }
        }
    }

    /// Trace instruction execution
    pub fn trace_instruction(&mut self, pc: u64, instruction: u32) {
        let event = TraceEvent::instruction(pc, instruction);
        self.trace_event(event);
    }

    /// Trace branch taken
    pub fn trace_branch_taken(&mut self, pc: u64, target: u64) {
        let event = TraceEvent::branch_taken(pc, target);
        self.trace_event(event);
    }

    /// Trace branch not taken
    pub fn trace_branch_not_taken(&mut self, pc: u64) {
        let event = TraceEvent::branch_not_taken(pc);
        self.trace_event(event);
    }

    /// Trace memory read
    pub fn trace_memory_read(&mut self, pc: u64, addr: u64, size: u32) {
        let event = TraceEvent::memory_read(pc, addr, size);
        self.trace_event(event);
    }

    /// Trace memory write
    pub fn trace_memory_write(&mut self, pc: u64, addr: u64, size: u32) {
        let event = TraceEvent::memory_write(pc, addr, size);
        self.trace_event(event);
    }

    /// Trace exception
    pub fn trace_exception(&mut self, pc: u64, cause: u32) {
        let event = TraceEvent::exception(pc, cause);
        self.trace_event(event);
    }

    /// Trace interrupt
    pub fn trace_interrupt(&mut self, pc: u64, irq: u32) {
        let event = TraceEvent::interrupt(pc, irq);
        self.trace_event(event);
    }

    /// Trace context switch
    pub fn trace_context_switch(&mut self, old_pc: u64, new_pc: u64) {
        let event = TraceEvent::context_switch(old_pc, new_pc);
        self.trace_event(event);
    }

    /// Trace custom event
    pub fn trace_custom(&mut self, pc: u64, data: u64, info: String) {
        let mut event = TraceEvent::new(TraceEventType::Custom, pc, data);
        event.info = Some(info);
        self.trace_event(event);
    }

    /// Get events from buffer
    pub fn get_events(&self) -> Result<Vec<TraceEvent>, &'static str> {
        if self.active {
            return Err("Cannot get events while tracer is active");
        }
        Ok(self.buffer.peek_all())
    }

    /// Get buffer statistics
    pub fn get_buffer_stats(&self) -> TraceBufferStats {
        self.buffer.get_stats()
    }

    /// Get trace statistics
    pub fn get_stats(&self) -> &TraceStats {
        &self.stats
    }

    /// Clear trace buffer
    pub fn clear(&mut self) {
        if !self.active {
            self.buffer.clear();
        }
    }

    /// Set address filter
    pub fn set_address_filter(&mut self, start: Option<u64>, end: Option<u64>) {
        self.config.address_filter = match (start, end) {
            (Some(s), Some(e)) if s <= e => Some((s, e)),
            (Some(s), None) => Some((s, u64::MAX)),
            (None, Some(e)) => Some((0, e)),
            (None, None) => None,
            _ => None,
        };
    }

    /// Enable/disable specific event type
    pub fn set_event_enabled(&mut self, event_type: TraceEventType, enabled: bool) {
        let bit = 1 << (event_type as u32);
        if enabled {
            self.config.event_filter |= bit;
        } else {
            self.config.event_filter &= !bit;
        }
    }

    /// Check if event type is enabled
    pub fn is_event_enabled(&self, event_type: TraceEventType) -> bool {
        let bit = 1 << (event_type as u32);
        (self.config.event_filter & bit) != 0
    }
}

/// Get current timestamp
fn get_timestamp() -> u64 {
    // In a real implementation, this would read from a hardware timer
    // For now, use a simple counter
    use core::sync::atomic::{AtomicU64, Ordering};
    static TIMESTAMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    TIMESTAMP_COUNTER.fetch_add(1, Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_event_creation() {
        let event = TraceEvent::new(TraceEventType::Instruction, 0x80000000, 0x00000013);
        assert_eq!(event.event_type, TraceEventType::Instruction);
        assert_eq!(event.pc, 0x80000000);
        assert_eq!(event.data, 0x00000013);
    }

    #[test]
    fn test_instruction_event() {
        let event = TraceEvent::instruction(0x80000000, 0x00000013);
        assert_eq!(event.event_type, TraceEventType::Instruction);
        assert_eq!(event.pc, 0x80000000);
        assert_eq!(event.data, 0x00000013);
    }

    #[test]
    fn test_branch_event() {
        let taken = TraceEvent::branch_taken(0x80000000, 0x80000100);
        assert_eq!(taken.event_type, TraceEventType::BranchTaken);
        assert_eq!(taken.get_branch_target(), Some(0x80000100));

        let not_taken = TraceEvent::branch_not_taken(0x80000004);
        assert_eq!(not_taken.event_type, TraceEventType::BranchNotTaken);
        assert_eq!(not_taken.get_branch_target(), None);
    }

    #[test]
    fn test_memory_event() {
        let read = TraceEvent::memory_read(0x80000000, 0x10000000, 4);
        assert_eq!(read.event_type, TraceEventType::MemoryRead);
        assert_eq!(read.get_memory_address(), Some(0x10000000));
        assert_eq!(read.get_memory_size(), Some(4));

        let write = TraceEvent::memory_write(0x80000004, 0x10000004, 8);
        assert_eq!(write.event_type, TraceEventType::MemoryWrite);
        assert_eq!(write.get_memory_address(), Some(0x10000004));
        assert_eq!(write.get_memory_size(), Some(8));
    }

    #[test]
    fn test_exception_event() {
        let event = TraceEvent::exception(0x80000000, 8);
        assert_eq!(event.event_type, TraceEventType::Exception);
        assert_eq!(event.get_exception_cause(), Some(8));
    }

    #[test]
    fn test_interrupt_event() {
        let event = TraceEvent::interrupt(0x80000000, 5);
        assert_eq!(event.event_type, TraceEventType::Interrupt);
        assert_eq!(event.get_interrupt_irq(), Some(5));
    }

    #[test]
    fn test_context_switch_event() {
        let event = TraceEvent::context_switch(0x80000000, 0x90000000);
        assert_eq!(event.event_type, TraceEventType::ContextSwitch);
        assert_eq!(event.get_context_target(), Some(0x90000000));
    }

    #[test]
    fn test_tracer_creation() {
        let tracer = Tracer::new(1024).unwrap();
        assert_eq!(tracer.get_config().buffer_size, 1024);
        assert!(!tracer.is_active());
    }

    #[test]
    fn test_tracer_start_stop() {
        let mut tracer = Tracer::new(1024).unwrap();

        assert!(!tracer.is_active());
        tracer.start().unwrap();
        assert!(tracer.is_active());

        // Cannot start twice
        assert!(tracer.start().is_err());

        let events = tracer.stop().unwrap();
        assert!(!tracer.is_active());
        assert!(events.is_empty()); // No events traced
    }

    #[test]
    fn test_tracer_trace_instruction() {
        let mut tracer = Tracer::new(1024).unwrap();
        tracer.start().unwrap();

        tracer.trace_instruction(0x80000000, 0x00000013);
        tracer.trace_instruction(0x80000004, 0x0000137);

        let events = tracer.stop().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, TraceEventType::Instruction);
        assert_eq!(events[0].pc, 0x80000000);
    }

    #[test]
    fn test_trace_config() {
        let config = TraceConfig::default();
        assert_eq!(config.buffer_size, 65536);
        assert!(config.trace_instructions);
        assert!(config.trace_branches);
        assert!(!config.trace_memory);
        assert!(config.trace_exceptions);
        assert!(config.trace_interrupts);
        assert!(config.trace_context_switches);
        assert_eq!(config.address_filter, None);
        assert_eq!(config.event_filter, 0xFFFFFFFF);
    }

    #[test]
    fn test_trace_buffer_stats() {
        let mut tracer = Tracer::new(100).unwrap();
        tracer.start().unwrap();

        // Add some events
        for i in 0..10 {
            tracer.trace_instruction(0x80000000 + (i as u64 * 4), 0x00000013);
        }

        let stats = tracer.get_buffer_stats();
        assert_eq!(stats.buffer_size, 100);
        assert_eq!(stats.events_in_buffer, 10);
        assert_eq!(stats.total_events, 10);
        assert_eq!(stats.overruns, 0);
        assert_eq!(stats.utilization, 0.1);
    }

    #[test]
    fn test_tracer_stats() {
        let mut tracer = Tracer::new(1024).unwrap();
        tracer.start().unwrap();

        // Trace different event types
        tracer.trace_instruction(0x80000000, 0x00000013);
        tracer.trace_branch_taken(0x80000004, 0x80000100);
        tracer.trace_memory_read(0x80000008, 0x10000000, 4);
        tracer.trace_exception(0x8000000C, 8);
        tracer.trace_interrupt(0x80000010, 5);
        tracer.trace_context_switch(0x80000014, 0x90000000);
        tracer.trace_custom(0x80000018, 0x12345678, "test".to_string());

        let stats = tracer.get_stats();
        assert_eq!(stats.instructions, 1);
        assert_eq!(stats.branches, 1);
        assert_eq!(stats.memory_accesses, 1);
        assert_eq!(stats.exceptions, 1);
        assert_eq!(stats.interrupts, 1);
        assert_eq!(stats.context_switches, 1);
        assert_eq!(stats.custom_events, 1);
    }

    #[test]
    fn test_event_filter() {
        let mut tracer = Tracer::new(1024).unwrap();

        // Disable instruction tracing
        tracer.set_event_enabled(TraceEventType::Instruction, false);
        assert!(!tracer.is_event_enabled(TraceEventType::Instruction));
        assert!(tracer.is_event_enabled(TraceEventType::BranchTaken));

        // Re-enable
        tracer.set_event_enabled(TraceEventType::Instruction, true);
        assert!(tracer.is_event_enabled(TraceEventType::Instruction));
    }

    #[test]
    fn test_address_filter() {
        let mut tracer = Tracer::new(1024).unwrap();

        tracer.set_address_filter(Some(0x80000000), Some(0x8000FFFF));
        assert_eq!(tracer.get_config().address_filter, Some((0x80000000, 0x8000FFFF)));

        tracer.set_address_filter(None, None);
        assert_eq!(tracer.get_config().address_filter, None);
    }
}