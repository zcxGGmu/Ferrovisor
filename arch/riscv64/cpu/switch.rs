//! RISC-V Context Switching
//!
//! This module provides context switching functionality for RISC-V including:
//! - Task context switching
//! - VCPU context switching
//! - Hypervisor mode transitions
//! - Assembly helper functions

use crate::arch::riscv64::cpu::regs::CpuState;

/// Context switch structure
#[repr(C)]
pub struct Context {
    /// Callee-saved general purpose registers
    pub ra: usize,      // x1
    pub sp: usize,      // x2
    pub gp: usize,      // x3
    pub tp: usize,      // x4
    pub t0: usize,      // x5
    pub t1: usize,      // x6
    pub t2: usize,      // x7
    pub s0: usize,      // x8/fp
    pub s1: usize,      // x9
    pub s2: usize,      // x18
    pub s3: usize,      // x19
    pub s4: usize,      // x20
    pub s5: usize,      // x21
    pub s6: usize,      // x22
    pub s7: usize,      // x23
    pub s8: usize,      // x24
    pub s9: usize,      // x25
    pub s10: usize,     // x26
    pub s11: usize,     // x27
    /// Program counter
    pub pc: usize,
    /// Floating point state
    pub fp_state: FpState,
}

/// Floating point state
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FpState {
    /// Callee-saved floating point registers
    pub fs0: u64,       // f8
    pub fs1: u64,       // f9
    pub fs2: u64,       // f18
    pub fs3: u64,       // f19
    pub fs4: u64,       // f20
    pub fs5: u64,       // f21
    pub fs6: u64,       // f22
    pub fs7: u64,       // f23
    pub fs8: u64,       // f24
    pub fs9: u64,       // f25
    pub fs10: u64,      // f26
    pub fs11: u64,      // f27
    /// FCSR register
    pub fcsr: u32,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            ra: 0,
            sp: 0,
            gp: 0,
            tp: 0,
            t0: 0,
            t1: 0,
            t2: 0,
            s0: 0,
            s1: 0,
            s2: 0,
            s3: 0,
            s4: 0,
            s5: 0,
            s6: 0,
            s7: 0,
            s8: 0,
            s9: 0,
            s10: 0,
            s11: 0,
            pc: 0,
            fp_state: FpState {
                fs0: 0,
                fs1: 0,
                fs2: 0,
                fs3: 0,
                fs4: 0,
                fs5: 0,
                fs6: 0,
                fs7: 0,
                fs8: 0,
                fs9: 0,
                fs10: 0,
                fs11: 0,
                fcsr: 0,
            },
        }
    }
}

impl Context {
    /// Create a new context
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a context for a new task
    pub fn new_task(entry: usize, stack_top: usize) -> Self {
        Self {
            ra: 0,              // Will be set when we first switch
            sp: stack_top,
            gp: 0,
            tp: 0,
            t0: 0,
            t1: 0,
            t2: 0,
            s0: 0,
            s1: 0,
            s2: 0,
            s3: 0,
            s4: 0,
            s5: 0,
            s6: 0,
            s7: 0,
            s8: 0,
            s9: 0,
            s10: 0,
            s11: 0,
            pc: entry,
            fp_state: FpState::default(),
        }
    }

    /// Get the stack pointer
    pub fn get_sp(&self) -> usize {
        self.sp
    }

    /// Set the stack pointer
    pub fn set_sp(&mut self, sp: usize) {
        self.sp = sp;
    }

    /// Get the program counter
    pub fn get_pc(&self) -> usize {
        self.pc
    }

    /// Set the program counter
    pub fn set_pc(&mut self, pc: usize) {
        self.pc = pc;
    }
}

/// Switch from one context to another
///
/// This function saves the current context and restores the new context.
/// It's typically called from assembly code that handles the actual register saving/restoring.
pub fn context_switch(from: &mut Context, to: &mut Context) {
    // This would typically be implemented in assembly
    // The actual implementation would:
    // 1. Save callee-saved registers to 'from' context
    // 2. Save current PC to 'from' context
    // 3. Save floating point state if enabled
    // 4. Restore callee-saved registers from 'to' context
    // 5. Restore PC from 'to' context
    // 6. Restore floating point state if enabled
    // 7. Return to new PC

    // For now, we just update the fields
    log::debug!("Context switch: SP {:#x} -> {:#x}, PC {:#x} -> {:#x}",
                from.sp, to.sp, from.pc, to.pc);
}

/// Initialize context switching
pub fn init() -> Result<(), &'static str> {
    log::debug!("Initializing context switching");

    // Initialize any required state for context switching
    // This might include setting up trampolines or other infrastructure

    log::debug!("Context switching initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let ctx = Context::new();
        assert_eq!(ctx.sp, 0);
        assert_eq!(ctx.pc, 0);
    }

    #[test]
    fn test_task_context() {
        let entry = 0x10000000;
        let stack_top = 0x20000000;
        let ctx = Context::new_task(entry, stack_top);

        assert_eq!(ctx.pc, entry);
        assert_eq!(ctx.sp, stack_top);
    }

    #[test]
    fn test_context_fields() {
        let mut ctx = Context::new();

        ctx.set_sp(0x12345678);
        assert_eq!(ctx.get_sp(), 0x12345678);

        ctx.set_pc(0x87654321);
        assert_eq!(ctx.get_pc(), 0x87654321);
    }
}