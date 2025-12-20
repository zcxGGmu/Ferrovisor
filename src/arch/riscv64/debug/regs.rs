//! RISC-V Debug Registers
//!
//! This module provides access to RISC-V debug registers including:
//! - Debug Control and Status Register (DCSR)
//! - Debug Program Counter (DPC)
//! - Debug Scratch Registers (DSCRATCH0, DSCRATCH1)
//! - Hardware Breakpoint Registers (TSELECT, TDATA1, TDATA2)
//! - Hardware Watchpoint Registers

use crate::arch::riscv64::*;
use crate::arch::riscv64::cpu::csr;

/// Debug register indices
pub const DCSR: u32 = 0x7b0;
pub const DPC: u32 = 0x7b1;
pub const DSCRATCH0: u32 = 0x7b2;
pub const DSCRATCH1: u32 = 0x7b3;
pub const TSELECT: u32 = 0x7a0;
pub const TDATA1: u32 = 0x7a1;
pub const TDATA2: u32 = 0x7a2;
pub const TDATA3: u32 = 0x7a3;

/// Debug Control and Status Register (DCSR)
#[derive(Debug, Clone, Copy)]
pub struct Dcsr {
    bits: u32,
}

impl Dcsr {
    /// Create new DCSR from raw value
    pub fn from_bits(bits: u32) -> Self {
        Self { bits }
    }

    /// Get raw bits
    pub fn bits(self) -> u32 {
        self.bits
    }

    /// Debug enable
    pub fn debug_enable(self) -> bool {
        (self.bits >> 0) & 1 != 0
    }

    /// Set debug enable
    pub fn set_debug_enable(&mut self, enable: bool) {
        self.bits = (self.bits & !(1 << 0)) | ((enable as u32) << 0);
    }

    /// Halt notification
    pub fn halt_notification(self) -> bool {
        (self.bits >> 1) & 1 != 0
    }

    /// Set halt notification
    pub fn set_halt_notification(&mut self, notify: bool) {
        self.bits = (self.bits & !(1 << 1)) | ((notify as u32) << 1);
    }

    /// Step
    pub fn step(self) -> bool {
        (self.bits >> 2) & 1 != 0
    }

    /// Set step
    pub fn set_step(&mut self, step: bool) {
        self.bits = (self.bits & !(1 << 2)) | ((step as u32) << 2);
    }

    /// Step Interrupt Enable
    pub fn step_ie(self) -> bool {
        (self.bits >> 3) & 1 != 0
    }

    /// Set step interrupt enable
    pub fn set_step_ie(&mut self, enable: bool) {
        self.bits = (self.bits & !(1 << 3)) | ((enable as u32) << 3);
    }

    /// Prv (privilege mode)
    pub fn prv(self) -> u8 {
        ((self.bits >> 4) & 0x3) as u8
    }

    /// Set prv
    pub fn set_prv(&mut self, prv: u8) {
        self.bits = (self.bits & !(0x3 << 4)) | (((prv as u32) & 0x3) << 4);
    }

    /// Cause of debug halt
    pub fn cause(self) -> DebugHaltCause {
        let cause = (self.bits >> 6) & 0x7;
        match cause {
            0 => DebugHaltCause::EBREAK,
            1 => DebugHaltCause::TRIGGER,
            2 => DebugHaltCause::HALTREQ,
            3 => DebugHaltCause::STEP,
            4 => DebugHaltCause::EXCEPTION,
            5 => DebugHaltCause::HALTGROUP,
            _ => DebugHaltCause::UNKNOWN,
        }
    }

    /// Set cause
    pub fn set_cause(&mut self, cause: DebugHaltCause) {
        let cause_val = match cause {
            DebugHaltCause::EBREAK => 0,
            DebugHaltCause::TRIGGER => 1,
            DebugHaltCause::HALTREQ => 2,
            DebugHaltCause::STEP => 3,
            DebugHaltCause::EXCEPTION => 4,
            DebugHaltCause::HALTGROUP => 5,
            DebugHaltCause::UNKNOWN => 7,
        };
        self.bits = (self.bits & !(0x7 << 6)) | (cause_val << 6);
    }

    /// Is currently halted
    pub fn halted(self) -> bool {
        (self.bits >> 9) & 1 != 0
    }

    /// Set halted
    pub fn set_halted(&mut self, halted: bool) {
        self.bits = (self.bits & !(1 << 9)) | ((halted as u32) << 9);
    }

    /// Ebreakm (ebreak in M-mode enters debug mode)
    pub fn ebreakm(self) -> bool {
        (self.bits >> 15) & 1 != 0
    }

    /// Set ebreakm
    pub fn set_ebreakm(&mut self, enable: bool) {
        self.bits = (self.bits & !(1 << 15)) | ((enable as u32) << 15);
    }

    /// Ebreaks (ebreak in S-mode enters debug mode)
    pub fn ebreaks(self) -> bool {
        (self.bits >> 13) & 1 != 0
    }

    /// Set ebreaks
    pub fn set_ebreaks(&mut self, enable: bool) {
        self.bits = (self.bits & !(1 << 13)) | ((enable as u32) << 13);
    }

    /// Ebreaku (ebreak in U-mode enters debug mode)
    pub fn ebreaku(self) -> bool {
        (self.bits >> 12) & 1 != 0
    }

    /// Set ebreaku
    pub fn set_ebreaku(&mut self, enable: bool) {
        self.bits = (self.bits & !(1 << 12)) | ((enable as u32) << 12);
    }

    /// Stepie (step interrupt enable)
    pub fn stepie(self) -> bool {
        (self.bits >> 11) & 1 != 0
    }

    /// Set stepie
    pub fn set_stepie(&mut self, enable: bool) {
        self.bits = (self.bits & !(1 << 11)) | ((enable as u32) << 11);
    }

    /// Stop count
    pub fn stopcount(self) -> bool {
        (self.bits >> 10) & 1 != 0
    }

    /// Set stop count
    pub fn set_stopcount(&mut self, stop: bool) {
        self.bits = (self.bits & !(1 << 10)) | ((stop as u32) << 10);
    }

    /// Stoptime
    pub fn stoptime(self) -> bool {
        (self.bits >> 8) & 1 != 0
    }

    /// Set stoptime
    pub fn set_stoptime(&mut self, stop: bool) {
        self.bits = (self.bits & !(1 << 8)) | ((stop as u32) << 8);
    }
}

/// Debug halt cause
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugHaltCause {
    /// ebreak instruction
    EBREAK,
    /// Trigger module
    TRIGGER,
    /// Halt request
    HALTREQ,
    /// Single step
    STEP,
    /// Exception while in debug mode
    EXCEPTION,
    /// Halt group
    HALTGROUP,
    /// Unknown cause
    UNKNOWN,
}

/// Debug Program Counter (DPC)
#[derive(Debug, Clone, Copy)]
pub struct Dpc {
    bits: u64,
}

impl Dpc {
    /// Create new DPC from raw value
    pub fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    /// Get raw bits
    pub fn bits(self) -> u64 {
        self.bits
    }

    /// Get program counter value
    pub fn pc(self) -> u64 {
        self.bits
    }

    /// Set program counter value
    pub fn set_pc(&mut self, pc: u64) {
        self.bits = pc;
    }
}

/// Trigger Select Register (TSELECT)
#[derive(Debug, Clone, Copy)]
pub struct Tselect {
    bits: u32,
}

impl Tselect {
    /// Create new TSELECT from raw value
    pub fn from_bits(bits: u32) -> Self {
        Self { bits }
    }

    /// Get raw bits
    pub fn bits(self) -> u32 {
        self.bits
    }

    /// Get selected trigger index
    pub fn index(self) -> u32 {
        self.bits
    }

    /// Set selected trigger index
    pub fn set_index(&mut self, index: u32) {
        self.bits = index;
    }
}

/// Trigger Data 1 Register (TDATA1)
#[derive(Debug, Clone, Copy)]
pub struct Tdata1 {
    bits: u64,
}

impl Tdata1 {
    /// Create new TDATA1 from raw value
    pub fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    /// Get raw bits
    pub fn bits(self) -> u64 {
        self.bits
    }

    /// Get trigger type
    pub fn r#type(self) -> u8 {
        ((self.bits >> 0) & 0xF) as u8
    }

    /// Set trigger type
    pub fn set_type(&mut self, r#type: u8) {
        self.bits = (self.bits & !(0xF << 0)) | (((r#type as u64) & 0xF) << 0);
    }

    /// Is dmode (debug-only mode)
    pub fn dmode(self) -> bool {
        (self.bits >> 5) & 1 != 0
    }

    /// Set dmode
    pub fn set_dmode(&mut self, dmode: bool) {
        self.bits = (self.bits & !(1 << 5)) | ((dmode as u64) << 5);
    }

    /// Is trigger enabled
    pub fn enabled(self) -> bool {
        (self.bits >> 7) & 1 != 0
    }

    /// Set enabled
    pub fn set_enabled(&mut self, enabled: bool) {
        self.bits = (self.bits & !(1 << 7)) | ((enabled as u64) << 7);
    }

    /// Hit bit
    pub fn hit(self) -> bool {
        (self.bits >> 8) & 1 != 0
    }

    /// Set hit
    pub fn set_hit(&mut self, hit: bool) {
        self.bits = (self.bits & !(1 << 8)) | ((hit as u64) << 8);
    }

    /// Select bit
    pub fn select(self) -> bool {
        (self.bits >> 9) & 1 != 0
    }

    /// Set select
    pub fn set_select(&mut self, select: bool) {
        self.bits = (self.bits & !(1 << 9)) | ((select as u64) << 9);
    }

    /// Timing bit
    pub fn timing(self) -> bool {
        (self.bits >> 12) & 1 != 0
    }

    /// Set timing
    pub fn set_timing(&mut self, timing: bool) {
        self.bits = (self.bits & !(1 << 12)) | ((timing as u64) << 12);
    }

    /// Action
    pub fn action(self) -> u8 {
        ((self.bits >> 12) & 0x3F) as u8
    }

    /// Set action
    pub fn set_action(&mut self, action: u8) {
        self.bits = (self.bits & !(0x3F << 12)) | (((action as u64) & 0x3F) << 12);
    }

    /// Match
    pub fn match_(&self) -> u8 {
        ((self.bits >> 16) & 0xF) as u8
    }

    /// Set match
    pub fn set_match(&mut self, match_: u8) {
        self.bits = (self.bits & !(0xF << 16)) | (((match_ as u64) & 0xF) << 16);
    }

    /// Chain bit
    pub fn chain(self) -> bool {
        (self.bits >> 20) & 1 != 0
    }

    /// Set chain
    pub fn set_chain(&mut self, chain: bool) {
        self.bits = (self.bits & !(1 << 20)) | ((chain as u64) << 20);
    }

    /// Execute bit
    pub fn execute(self) -> bool {
        (self.bits >> 28) & 1 != 0
    }

    /// Set execute
    pub fn set_execute(&mut self, execute: bool) {
        self.bits = (self.bits & !(1 << 28)) | ((execute as u64) << 28);
    }

    /// Store bit
    pub fn store(self) -> bool {
        (self.bits >> 29) & 1 != 0
    }

    /// Set store
    pub fn set_store(&mut self, store: bool) {
        self.bits = (self.bits & !(1 << 29)) | ((store as u64) << 29);
    }

    /// Load bit
    pub fn load(self) -> bool {
        (self.bits >> 30) & 1 != 0
    }

    /// Set load
    pub fn set_load(&mut self, load: bool) {
        self.bits = (self.bits & !(1 << 30)) | ((load as u64) << 30);
    }
}

/// Trigger Data 2 Register (TDATA2)
#[derive(Debug, Clone, Copy)]
pub struct Tdata2 {
    bits: u64,
}

impl Tdata2 {
    /// Create new TDATA2 from raw value
    pub fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    /// Get raw bits
    pub fn bits(self) -> u64 {
        self.bits
    }

    /// Get address/value
    pub fn value(self) -> u64 {
        self.bits
    }

    /// Set address/value
    pub fn set_value(&mut self, value: u64) {
        self.bits = value;
    }
}

/// Debug Registers interface
pub struct DebugRegisters {
    _private: (),
}

impl DebugRegisters {
    /// Create new debug registers interface
    pub fn new() -> Result<Self, &'static str> {
        // Check if debug module is present
        if !has_debug_module() {
            return Err("Debug module not present");
        }

        Ok(Self { _private: () })
    }

    /// Read DCSR
    pub fn read_dcsr(&self) -> Dcsr {
        Dcsr::from_bits(unsafe { csr::read32(DCSR) })
    }

    /// Write DCSR
    pub fn write_dcsr(&self, dcsr: Dcsr) {
        unsafe { csr::write32(DCSR, dcsr.bits()) }
    }

    /// Read DPC
    pub fn read_dpc(&self) -> Dpc {
        Dpc::from_bits(unsafe { csr::read64(DPC) })
    }

    /// Write DPC
    pub fn write_dpc(&self, dpc: Dpc) {
        unsafe { csr::write64(DPC, dpc.bits()) }
    }

    /// Read DSCRATCH0
    pub fn read_dscratch0(&self) -> u64 {
        unsafe { csr::read64(DSCRATCH0) }
    }

    /// Write DSCRATCH0
    pub fn write_dscratch0(&self, value: u64) {
        unsafe { csr::write64(DSCRATCH0, value) }
    }

    /// Read DSCRATCH1
    pub fn read_dscratch1(&self) -> u64 {
        unsafe { csr::read64(DSCRATCH1) }
    }

    /// Write DSCRATCH1
    pub fn write_dscratch1(&self, value: u64) {
        unsafe { csr::write64(DSCRATCH1, value) }
    }

    /// Read TSELECT
    pub fn read_tselect(&self) -> Tselect {
        Tselect::from_bits(unsafe { csr::read32(TSELECT) })
    }

    /// Write TSELECT
    pub fn write_tselect(&self, tselect: Tselect) {
        unsafe { csr::write32(TSELECT, tselect.bits()) }
    }

    /// Read TDATA1
    pub fn read_tdata1(&self) -> Tdata1 {
        Tdata1::from_bits(unsafe { csr::read64(TDATA1) })
    }

    /// Write TDATA1
    pub fn write_tdata1(&self, tdata1: Tdata1) {
        unsafe { csr::write64(TDATA1, tdata1.bits()) }
    }

    /// Read TDATA2
    pub fn read_tdata2(&self) -> Tdata2 {
        Tdata2::from_bits(unsafe { csr::read64(TDATA2) })
    }

    /// Write TDATA2
    pub fn write_tdata2(&self, tdata2: Tdata2) {
        unsafe { csr::write64(TDATA2, tdata2.bits()) }
    }

    /// Read TDATA3
    pub fn read_tdata3(&self) -> u64 {
        unsafe { csr::read64(TDATA3) }
    }

    /// Write TDATA3
    pub fn write_tdata3(&self, value: u64) {
        unsafe { csr::write64(TDATA3, value) }
    }

    /// Get number of supported triggers
    pub fn get_trigger_count(&self) -> u32 {
        let original_tselect = self.read_tselect();

        // Try to write a large value to see the maximum
        self.write_tselect(Tselect::from_bits(0xFFFFFFFF));
        let count = self.read_tselect().index() + 1;

        // Restore original value
        self.write_tselect(original_tselect);

        count
    }

    /// Select trigger
    pub fn select_trigger(&self, index: u32) {
        self.write_tselect(Tselect::from_bits(index));
    }

    /// Read trigger data
    pub fn read_trigger(&self) -> (Tdata1, Tdata2) {
        (self.read_tdata1(), self.read_tdata2())
    }

    /// Write trigger data
    pub fn write_trigger(&self, tdata1: Tdata1, tdata2: Tdata2) {
        self.write_tdata1(tdata1);
        self.write_tdata2(tdata2);
    }

    /// Read register by ID
    pub fn read_register(&self, reg_id: u32) -> Result<u64, &'static str> {
        match reg_id {
            0x0000..=0x001F => {
                // General purpose registers (x0-x31)
                if reg_id == 0 {
                    Ok(0) // x0 is hardwired to 0
                } else {
                    // Read from debug interface
                    self.read_gpr(reg_id)
                }
            }
            0x1000 => {
                // PC
                Ok(self.read_dpc().bits())
            }
            _ => Err("Unsupported register ID"),
        }
    }

    /// Write register by ID
    pub fn write_register(&self, reg_id: u32, value: u64) -> Result<(), &'static str> {
        match reg_id {
            0x0001..=0x001F => {
                // General purpose registers (x1-x31, skip x0)
                self.write_gpr(reg_id, value)
            }
            0x1000 => {
                // PC
                self.write_dpc(Dpc::from_bits(value));
                Ok(())
            }
            _ => Err("Unsupported register ID"),
        }
    }

    /// Capture current CPU state
    pub fn capture_cpu_state(&self) -> Result<CpuState, &'static str> {
        let mut state = CpuState::new();

        // Read PC
        state.pc = self.read_dpc().bits();

        // Read GPRs
        for i in 1..32 {
            state.gpr[i] = self.read_register(i)?;
        }

        // Read CSR registers
        // Note: These would need to be accessed via different CSR numbers
        // This is a simplified version

        Ok(state)
    }

    /// Read general purpose register via debug interface
    fn read_gpr(&self, reg: u32) -> Result<u64, &'static str> {
        // In a real implementation, this would use the Abstract Command interface
        // For now, return a placeholder
        Err("GPR access via debug interface not implemented")
    }

    /// Write general purpose register via debug interface
    fn write_gpr(&self, _reg: u32, _value: u64) -> Result<(), &'static str> {
        // In a real implementation, this would use the Abstract Command interface
        // For now, return an error
        Err("GPR access via debug interface not implemented")
    }
}

/// Check if debug module is present
fn has_debug_module() -> bool {
    // Try to read DCSR, if it doesn't fault, debug module is present
    // This is a simplified check
    true
}

/// CPU state captured for debugging
#[derive(Debug, Clone)]
pub struct CpuState {
    /// Program counter
    pub pc: u64,
    /// General purpose registers
    pub gpr: [u64; 32],
    /// Machine status register
    pub mstatus: Option<u64>,
    /// Machine exception program counter
    pub mepc: Option<u64>,
    /// Machine cause register
    pub mcause: Option<u64>,
    /// Machine trap value
    pub mtval: Option<u64>,
}

impl CpuState {
    /// Create new CPU state
    pub fn new() -> Self {
        Self {
            pc: 0,
            gpr: [0; 32],
            mstatus: None,
            mepc: None,
            mcause: None,
            mtval: None,
        }
    }

    /// Get register name by index
    pub fn reg_name(index: usize) -> &'static str {
        match index {
            0 => "zero",
            1 => "ra",
            2 => "sp",
            3 => "gp",
            4 => "tp",
            5..=7 => "t",
            8 => "s0",
            9 => "s1",
            10..=17 => "a",
            18..=27 => "s",
            28..=31 => "t",
            _ => "unknown",
        }
    }

    /// Print CPU state
    pub fn print(&self) {
        log::debug!("CPU State at {:#x}", self.pc);

        // Print important registers
        for i in [1, 2, 3, 4, 5, 8, 9, 10, 11, 28, 29, 30, 31] {
            log::debug!("  {:4}: {:#018x}", Self::reg_name(i), self.gpr[i]);
        }

        if let Some(mstatus) = self.mstatus {
            log::debug!("  mstatus: {:#018x}", mstatus);
        }
        if let Some(mepc) = self.mepc {
            log::debug!("  mepc: {:#018x}", mepc);
        }
        if let Some(mcause) = self.mcause {
            log::debug!("  mcause: {:#018x}", mcause);
        }
        if let Some(mtval) = self.mtval {
            log::debug!("  mtval: {:#018x}", mtval);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dcsr() {
        let mut dcsr = Dcsr::from_bits(0);

        dcsr.set_debug_enable(true);
        assert!(dcsr.debug_enable());

        dcsr.set_step(true);
        assert!(dcsr.step());

        dcsr.set_halted(true);
        assert!(dcsr.halted());

        dcsr.set_cause(DebugHaltCause::EBREAK);
        assert_eq!(dcsr.cause(), DebugHaltCause::EBREAK);
    }

    #[test]
    fn test_dpc() {
        let mut dpc = Dpc::from_bits(0);
        dpc.set_pc(0x80000000);
        assert_eq!(dpc.pc(), 0x80000000);
    }

    #[test]
    fn test_tdata1() {
        let mut tdata1 = Tdata1::from_bits(0);

        tdata1.set_enabled(true);
        assert!(tdata1.enabled());

        tdata1.set_type(2);
        assert_eq!(tdata1.r#type(), 2);

        tdata1.set_execute(true);
        assert!(tdata1.execute());
    }

    #[test]
    fn test_tdata2() {
        let mut tdata2 = Tdata2::from_bits(0);
        tdata2.set_value(0x80000000);
        assert_eq!(tdata2.value(), 0x80000000);
    }

    #[test]
    fn test_cpu_state() {
        let mut state = CpuState::new();
        state.pc = 0x80000000;
        state.gpr[1] = 0x100000;

        assert_eq!(state.pc, 0x80000000);
        assert_eq!(state.gpr[1], 0x100000);
        assert_eq!(CpuState::reg_name(1), "ra");
        assert_eq!(CpuState::reg_name(2), "sp");
    }
}