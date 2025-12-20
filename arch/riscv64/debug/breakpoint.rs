//! RISC-V Hardware Breakpoints and Watchpoints
//!
//! This module provides support for hardware breakpoints and watchpoints including:
//! - Instruction breakpoints
//! - Data read/write watchpoints
//! - Breakpoint management and status
//! - Trigger module configuration

use crate::arch::riscv64::*;
use crate::arch::riscv64::debug::regs::*;

/// Breakpoint type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakpointType {
    /// Instruction breakpoint
    Instruction,
    /// Data read watchpoint
    DataRead,
    /// Data write watchpoint
    DataWrite,
    /// Data read/write watchpoint
    DataReadWrite,
    /// Address range match
    AddressRange,
}

/// Breakpoint status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakpointStatus {
    /// Breakpoint is not set
    NotSet,
    /// Breakpoint is set but not triggered
    Active,
    /// Breakpoint was triggered
    Triggered,
    /// Breakpoint is disabled
    Disabled,
}

/// Hardware breakpoint/watchpoint
#[derive(Debug, Clone)]
pub struct Breakpoint {
    /// Breakpoint ID
    pub id: u32,
    /// Breakpoint type
    pub bp_type: BreakpointType,
    /// Address or start of range
    pub address: u64,
    /// End of range (for range breakpoints)
    pub end_address: Option<u64>,
    /// Breakpoint status
    pub status: BreakpointStatus,
    /// Trigger count
    pub trigger_count: u64,
    /// Is temporary (auto-clear on trigger)
    pub temporary: bool,
    /// Associated trigger index in hardware
    pub trigger_index: Option<u32>,
}

impl Breakpoint {
    /// Create new breakpoint
    pub fn new(id: u32, bp_type: BreakpointType, address: u64) -> Self {
        Self {
            id,
            bp_type,
            address,
            end_address: None,
            status: BreakpointStatus::NotSet,
            trigger_count: 0,
            temporary: false,
            trigger_index: None,
        }
    }

    /// Create range breakpoint
    pub fn new_range(id: u32, start: u64, end: u64) -> Self {
        Self {
            id,
            bp_type: BreakpointType::AddressRange,
            address: start,
            end_address: Some(end),
            status: BreakpointStatus::NotSet,
            trigger_count: 0,
            temporary: false,
            trigger_index: None,
        }
    }

    /// Set as temporary
    pub fn set_temporary(&mut self, temporary: bool) {
        self.temporary = temporary;
    }

    /// Check if address matches this breakpoint
    pub fn matches(&self, addr: u64) -> bool {
        match self.end_address {
            Some(end) => addr >= self.address && addr <= end,
            None => addr == self.address,
        }
    }

    /// Trigger the breakpoint
    pub fn trigger(&mut self) {
        self.status = BreakpointStatus::Triggered;
        self.trigger_count += 1;
    }

    /// Reset breakpoint status
    pub fn reset(&mut self) {
        self.status = if self.trigger_index.is_some() {
            BreakpointStatus::Active
        } else {
            BreakpointStatus::NotSet
        };
    }

    /// Disable breakpoint
    pub fn disable(&mut self) {
        self.status = BreakpointStatus::Disabled;
    }

    /// Enable breakpoint
    pub fn enable(&mut self) {
        if self.trigger_index.is_some() {
            self.status = BreakpointStatus::Active;
        }
    }
}

/// Breakpoint manager
pub struct BreakpointManager {
    /// Maximum number of breakpoints
    max_breakpoints: u32,
    /// Maximum number of watchpoints
    max_watchpoints: u32,
    /// Breakpoints
    breakpoints: Vec<Breakpoint>,
    /// Watchpoints
    watchpoints: Vec<Breakpoint>,
    /// Free trigger indices
    free_triggers: Vec<u32>,
    /// Debug registers
    debug_regs: DebugRegisters,
}

impl BreakpointManager {
    /// Create new breakpoint manager
    pub fn new(max_breakpoints: u32, max_watchpoints: u32) -> Result<Self, &'static str> {
        let debug_regs = DebugRegisters::new()?;

        // Get total number of triggers
        let total_triggers = debug_regs.get_trigger_count();
        if (max_breakpoints + max_watchpoints) > total_triggers {
            return Err("Requested breakpoints exceed available triggers");
        }

        // Initialize free trigger list
        let mut free_triggers = Vec::new();
        for i in 0..total_triggers {
            free_triggers.push(i);
        }

        Ok(Self {
            max_breakpoints,
            max_watchpoints,
            breakpoints: Vec::new(),
            watchpoints: Vec::new(),
            free_triggers,
            debug_regs,
        })
    }

    /// Set a breakpoint
    pub fn set_breakpoint(&mut self, addr: usize, bp_type: BreakpointType) -> Result<u32, &'static str> {
        // Determine if this is a breakpoint or watchpoint
        let is_watchpoint = match bp_type {
            BreakpointType::Instruction => false,
            _ => true,
        };

        // Check capacity
        if is_watchpoint {
            if self.watchpoints.len() >= self.max_watchpoints as usize {
                return Err("Maximum watchpoints reached");
            }
        } else {
            if self.breakpoints.len() >= self.max_breakpoints as usize {
                return Err("Maximum breakpoints reached");
            }
        }

        // Allocate trigger
        let trigger_index = self.free_triggers.pop()
            .ok_or("No available triggers")?;

        // Create breakpoint
        let id = if is_watchpoint {
            self.watchpoints.len() as u32
        } else {
            self.breakpoints.len() as u32
        };

        let mut bp = Breakpoint::new(id, bp_type, addr as u64);
        bp.trigger_index = Some(trigger_index);

        // Configure hardware trigger
        self.configure_trigger(trigger_index, &bp)?;

        // Store breakpoint
        if is_watchpoint {
            self.watchpoints.push(bp);
        } else {
            self.breakpoints.push(bp);
        }

        Ok(id)
    }

    /// Clear a breakpoint
    pub fn clear_breakpoint(&mut self, id: u32) -> Result<(), &'static str> {
        // Search breakpoints first
        for i in 0..self.breakpoints.len() {
            if self.breakpoints[i].id == id {
                let bp = &self.breakpoints[i];
                if let Some(trigger_index) = bp.trigger_index {
                    self.clear_trigger(trigger_index);
                    self.free_triggers.push(trigger_index);
                }
                self.breakpoints.remove(i);
                return Ok(());
            }
        }

        // Search watchpoints
        for i in 0..self.watchpoints.len() {
            if self.watchpoints[i].id == id {
                let wp = &self.watchpoints[i];
                if let Some(trigger_index) = wp.trigger_index {
                    self.clear_trigger(trigger_index);
                    self.free_triggers.push(trigger_index);
                }
                self.watchpoints.remove(i);
                return Ok(());
            }
        }

        Err("Breakpoint not found")
    }

    /// Get breakpoint by ID
    pub fn get_breakpoint(&self, id: u32) -> Option<&Breakpoint> {
        self.breakpoints.iter()
            .find(|bp| bp.id == id)
            .or_else(|| self.watchpoints.iter().find(|wp| wp.id == id))
    }

    /// Get breakpoint by ID (mutable)
    pub fn get_breakpoint_mut(&mut self, id: u32) -> Option<&mut Breakpoint> {
        self.breakpoints.iter_mut()
            .find(|bp| bp.id == id)
            .or_else(|| self.watchpoints.iter_mut().find(|wp| wp.id == id))
    }

    /// Get all breakpoints
    pub fn get_breakpoints(&self) -> &[Breakpoint] {
        &self.breakpoints
    }

    /// Get all watchpoints
    pub fn get_watchpoints(&self) -> &[Breakpoint] {
        &self.watchpoints
    }

    /// Check if any breakpoint/watchpoint triggered
    pub fn check_triggers(&mut self) -> Vec<u32> {
        let mut triggered = Vec::new();

        // Check all breakpoints
        for bp in &mut self.breakpoints {
            if let Some(trigger_index) = bp.trigger_index {
                self.debug_regs.select_trigger(trigger_index);
                let tdata1 = self.debug_regs.read_tdata1();

                if tdata1.hit() {
                    bp.trigger();
                    triggered.push(bp.id);

                    // Clear hit bit
                    let mut tdata1_mut = tdata1;
                    tdata1_mut.set_hit(false);
                    self.debug_regs.write_tdata1(tdata1_mut);

                    // If temporary, clear it
                    if bp.temporary {
                        self.free_triggers.push(trigger_index);
                    }
                }
            }
        }

        // Check all watchpoints
        for wp in &mut self.watchpoints {
            if let Some(trigger_index) = wp.trigger_index {
                self.debug_regs.select_trigger(trigger_index);
                let tdata1 = self.debug_regs.read_tdata1();

                if tdata1.hit() {
                    wp.trigger();
                    triggered.push(wp.id);

                    // Clear hit bit
                    let mut tdata1_mut = tdata1;
                    tdata1_mut.set_hit(false);
                    self.debug_regs.write_tdata1(tdata1_mut);

                    // If temporary, clear it
                    if wp.temporary {
                        self.free_triggers.push(trigger_index);
                    }
                }
            }
        }

        // Remove temporary breakpoints that triggered
        self.breakpoints.retain(|bp| {
            if bp.temporary && bp.status == BreakpointStatus::Triggered {
                if let Some(trigger_index) = bp.trigger_index {
                    self.clear_trigger(trigger_index);
                }
                false
            } else {
                true
            }
        });

        self.watchpoints.retain(|wp| {
            if wp.temporary && wp.status == BreakpointStatus::Triggered {
                if let Some(trigger_index) = wp.trigger_index {
                    self.clear_trigger(trigger_index);
                }
                false
            } else {
                true
            }
        });

        triggered
    }

    /// Configure hardware trigger
    fn configure_trigger(&mut self, trigger_index: u32, bp: &Breakpoint) -> Result<(), &'static str> {
        self.debug_regs.select_trigger(trigger_index);

        // Configure TDATA1
        let mut tdata1 = Tdata1::from_bits(0);

        // Set trigger type (2 = address/data match)
        tdata1.set_type(2);

        // Set debug-only mode
        tdata1.set_dmode(true);

        // Enable the trigger
        tdata1.set_enabled(true);

        // Clear hit bit
        tdata1.set_hit(false);

        // Configure based on breakpoint type
        match bp.bp_type {
            BreakpointType::Instruction => {
                tdata1.set_execute(true);
            }
            BreakpointType::DataRead => {
                tdata1.set_load(true);
            }
            BreakpointType::DataWrite => {
                tdata1.set_store(true);
            }
            BreakpointType::DataReadWrite => {
                tdata1.set_load(true);
                tdata1.set_store(true);
            }
            BreakpointType::AddressRange => {
                // Address range uses special match mode
                tdata1.set_execute(true);
                tdata1.set_match(0); // Exact match for start address
            }
        }

        // Set timing (before execution)
        tdata1.set_timing(true);

        // Set action (break into debug mode)
        tdata1.set_action(0);

        self.debug_regs.write_tdata1(tdata1);

        // Configure TDATA2 (address)
        let mut tdata2 = Tdata2::from_bits(bp.address);
        self.debug_regs.write_tdata2(tdata2);

        // For range breakpoints, might need TDATA3 for end address
        if let Some(end_addr) = bp.end_address {
            let mut tdata3 = end_addr;
            self.debug_regs.write_tdata3(tdata3);
        }

        Ok(())
    }

    /// Clear hardware trigger
    fn clear_trigger(&mut self, trigger_index: u32) {
        self.debug_regs.select_trigger(trigger_index);

        // Disable the trigger
        let mut tdata1 = self.debug_regs.read_tdata1();
        tdata1.set_enabled(false);
        self.debug_regs.write_tdata1(tdata1);
    }

    /// Get breakpoint statistics
    pub fn get_stats(&self) -> BreakpointStats {
        BreakpointStats {
            total_breakpoints: self.breakpoints.len() as u32,
            active_breakpoints: self.breakpoints.iter()
                .filter(|bp| bp.status == BreakpointStatus::Active)
                .count() as u32,
            triggered_breakpoints: self.breakpoints.iter()
                .filter(|bp| bp.status == BreakpointStatus::Triggered)
                .count() as u32,
            total_watchpoints: self.watchpoints.len() as u32,
            active_watchpoints: self.watchpoints.iter()
                .filter(|wp| wp.status == BreakpointStatus::Active)
                .count() as u32,
            triggered_watchpoints: self.watchpoints.iter()
                .filter(|wp| wp.status == BreakpointStatus::Triggered)
                .count() as u32,
            free_triggers: self.free_triggers.len() as u32,
        }
    }

    /// Reset all breakpoints and watchpoints
    pub fn reset(&mut self) {
        // Clear all triggers
        for bp in &self.breakpoints {
            if let Some(trigger_index) = bp.trigger_index {
                self.clear_trigger(trigger_index);
                self.free_triggers.push(trigger_index);
            }
        }

        for wp in &self.watchpoints {
            if let Some(trigger_index) = wp.trigger_index {
                self.clear_trigger(trigger_index);
                self.free_triggers.push(trigger_index);
            }
        }

        self.breakpoints.clear();
        self.watchpoints.clear();
    }
}

/// Breakpoint statistics
#[derive(Debug, Clone, Default)]
pub struct BreakpointStats {
    /// Total number of breakpoints
    pub total_breakpoints: u32,
    /// Number of active breakpoints
    pub active_breakpoints: u32,
    /// Number of triggered breakpoints
    pub triggered_breakpoints: u32,
    /// Total number of watchpoints
    pub total_watchpoints: u32,
    /// Number of active watchpoints
    pub active_watchpoints: u32,
    /// Number of triggered watchpoints
    pub triggered_watchpoints: u32,
    /// Number of free triggers
    pub free_triggers: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breakpoint_creation() {
        let bp = Breakpoint::new(0, BreakpointType::Instruction, 0x80000000);
        assert_eq!(bp.id, 0);
        assert_eq!(bp.bp_type, BreakpointType::Instruction);
        assert_eq!(bp.address, 0x80000000);
        assert_eq!(bp.status, BreakpointStatus::NotSet);
        assert_eq!(bp.trigger_count, 0);
        assert!(!bp.temporary);
    }

    #[test]
    fn test_range_breakpoint() {
        let bp = Breakpoint::new_range(0, 0x80000000, 0x80001000);
        assert_eq!(bp.id, 0);
        assert_eq!(bp.bp_type, BreakpointType::AddressRange);
        assert_eq!(bp.address, 0x80000000);
        assert_eq!(bp.end_address, Some(0x80001000));
    }

    #[test]
    fn test_breakpoint_matching() {
        let bp = Breakpoint::new_range(0, 0x80000000, 0x80001000);
        assert!(bp.matches(0x80000000));
        assert!(bp.matches(0x80000500));
        assert!(bp.matches(0x80001000));
        assert!(!bp.matches(0x80002000));

        let bp2 = Breakpoint::new(1, BreakpointType::Instruction, 0x80001000);
        assert!(!bp2.matches(0x80000000));
        assert!(bp2.matches(0x80001000));
    }

    #[test]
    fn test_breakpoint_trigger() {
        let mut bp = Breakpoint::new(0, BreakpointType::Instruction, 0x80000000);
        assert_eq!(bp.status, BreakpointStatus::NotSet);
        assert_eq!(bp.trigger_count, 0);

        bp.trigger();
        assert_eq!(bp.status, BreakpointStatus::Triggered);
        assert_eq!(bp.trigger_count, 1);

        bp.reset();
        assert_eq!(bp.status, BreakpointStatus::NotSet);
        assert_eq!(bp.trigger_count, 1);
    }

    #[test]
    fn test_breakpoint_enable_disable() {
        let mut bp = Breakpoint::new(0, BreakpointType::Instruction, 0x80000000);
        bp.trigger_index = Some(0);

        assert_eq!(bp.status, BreakpointStatus::NotSet);

        bp.enable();
        assert_eq!(bp.status, BreakpointStatus::Active);

        bp.disable();
        assert_eq!(bp.status, BreakpointStatus::Disabled);
    }

    #[test]
    fn test_breakpoint_stats() {
        let stats = BreakpointStats {
            total_breakpoints: 10,
            active_breakpoints: 8,
            triggered_breakpoints: 2,
            total_watchpoints: 4,
            active_watchpoints: 3,
            triggered_watchpoints: 1,
            free_triggers: 2,
        };
        assert_eq!(stats.total_breakpoints, 10);
        assert_eq!(stats.active_breakpoints, 8);
        assert_eq!(stats.triggered_breakpoints, 2);
        assert_eq!(stats.total_watchpoints, 4);
        assert_eq!(stats.active_watchpoints, 3);
        assert_eq!(stats.triggered_watchpoints, 1);
        assert_eq!(stats.free_triggers, 2);
    }
}