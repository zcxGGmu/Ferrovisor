//! RISC-V JTAG Debug Interface
//!
//! This module provides JTAG debug interface support including:
//! - JTAG TAP (Test Access Port) controller
//! - Debug module communication
//! - Abstract command interface
//! - Program buffer access
//! - Run control operations

use crate::arch::riscv64::*;

/// JTAG TAP controller states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapState {
    /// Test-Logic-Reset
    TestLogicReset,
    /// Run-Test/Idle
    RunTestIdle,
    /// Select-DR-Scan
    SelectDrScan,
    /// Capture-DR
    CaptureDr,
    /// Shift-DR
    ShiftDr,
    /// Exit1-DR
    Exit1Dr,
    /// Pause-DR
    PauseDr,
    /// Exit2-DR
    Exit2Dr,
    /// Update-DR
    UpdateDr,
    /// Select-IR-Scan
    SelectIrScan,
    /// Capture-IR
    CaptureIr,
    /// Shift-IR
    ShiftIr,
    /// Exit1-IR
    Exit1Ir,
    /// Pause-IR
    PauseIr,
    /// Exit2-IR
    Exit2Ir,
    /// Update-IR
    UpdateIr,
}

/// JTAG instruction register (IR) commands
pub mod ir {
    pub const BYPASS: u8 = 0x0F;
    pub const IDCODE: u8 = 0x01;
    pub const DTMCS: u8 = 0x10;
    pub const DMI: u8 = 0x11;
    pub const BYPASS_ALL: u8 = 0xFF;
}

/// Debug Module Interface (DMI) addresses
pub mod dmi {
    /// Debug module status register
    pub const DMSTATUS: u32 = 0x04;
    /// Debug module control register
    pub const DMCONTROL: u32 = 0x10;
    /// Abstract command 0 register
    pub const ABSTRACTCMD0: u32 = 0x20;
    /// Abstract command 1 register
    pub const ABSTRACTCMD1: u32 = 0x21;
    /// Abstract command control and status
    pub const COMMAND: u32 = 0x17;
    /// Abstract data 0 register
    pub const DATA0: u32 = 0x04;
    /// Abstract data 1 register
    pub const DATA1: u32 = 0x05;
    /// Program buffer 0 register
    pub const PROGBUF0: u32 = 0x20;
    /// Program buffer maximum
    pub const PROGBUF_MAX: u32 = 0x2F;
    /// Abstract CS registers
    pub const ABSTRACTCS: u32 = 0x16;
    /// Debug module CS register
    pub const DMCS: u32 = 0x00;
}

/// DTM (Debug Transport Module) Control and Status Register
#[derive(Debug, Clone, Copy)]
pub struct Dtmcs {
    bits: u32,
}

impl Dtmcs {
    /// Create new DTMCS from raw value
    pub fn from_bits(bits: u32) -> Self {
        Self { bits }
    }

    /// Get raw bits
    pub fn bits(self) -> u32 {
        self.bits
    }

    /// Get DTM version
    pub fn version(self) -> u8 {
        ((self.bits >> 0) & 0xF) as u8
    }

    /// Get number of address bits in DMI
    pub fn abits(self) -> u8 {
        ((self.bits >> 4) & 0x3F) as u8
    }

    /// Get DMI status busy flag
    pub fn dmibusy(self) -> bool {
        (self.bits >> 12) & 1 != 0
    }

    /// Get DMI operation error
    pub fn dmiop(self) -> u8 {
        ((self.bits >> 13) & 0x3) as u8
    }
}

/// DMSTATUS register fields
pub mod dmstatus {
    pub const IMPEBREAK: u32 = 1 << 22;
    pub const ALLHALTED: u32 = 1 << 19;
    pub const ANYHALTED: u32 = 1 << 18;
    pub const ALLRUNNING: u32 = 1 << 17;
    pub const ANYRUNNING: u32 = 1 << 16;
    pub const ALLRESUMEACK: u32 = 1 << 7;
    pub const ANYRESUMEACK: u32 = 1 << 6;
    pub const ALLNONEXISTENT: u32 = 1 << 5;
    pub const ANYNONEXISTENT: u32 = 1 << 4;
    pub const ALLUNAVAIL: u32 = 1 << 3;
    pub const ANYUNAVAIL: u32 = 1 << 2;
    pub const ALLAVAILENABLE: u32 = 1 << 1;
    pub const ANYAVAILENABLE: u32 = 1 << 0;
}

/// DMCONTROL register fields
pub mod dmcontrol {
    pub const HALTREQ: u32 = 1 << 31;
    pub const RESUMEREQ: u32 = 1 << 30;
    pub const HARVESTE: u32 = 1 << 28;
    pub const HASEL: u32 = 1 << 26;
    pub const HARTSELHI: u32 = 0x3FF << 16;
    pub const HARTSELLO: u32 = 0xFFF << 0;
    pub const ACKHAVERESET: u32 = 1 << 28;
    pub const NDRESET: u32 = 1 << 1;
    pub const FULLRESET: u32 = 1 << 0;
}

/// Abstract command types
pub const ABSTRACT_ACCESS_REGISTER: u32 = 0x0;
pub const ABSTRACT_ACCESS_MEMORY: u32 = 0x2;
pub const ABSTRACT_QUICK_ACCESS: u32 = 0x3;

/// Abstract command control
pub const ABSTRACT_CMD_TYPE_SHIFT: u32 = 24;
pub const ABSTRACT_CMD_SIZE: u32 = 8;
pub const ABSTRACT_CMD_TYPE_MASK: u32 = 0xF << ABSTRACT_CMD_TYPE_SHIFT;

/// Abstract command register access fields
pub const ABSTRACT_REG_WRITE: u32 = 1 << 0;
pub const ABSTRACT_REG_READ: u32 = 1 << 1;
pub const ABSTRACT_REG_SIZE_SHIFT: u32 = 2;
pub const ABSTRACT_REG_SIZE_MASK: u32 = 0x3 << ABSTRACT_REG_SIZE_SHIFT;

/// JTAG TAP controller
pub struct TapController {
    /// Current state
    state: TapState,
    /// IR register value
    ir: u8,
    /// IR register length
    ir_length: u8,
}

impl TapController {
    /// Create new TAP controller
    pub fn new() -> Self {
        Self {
            state: TapState::TestLogicReset,
            ir: 0,
            ir_length: 5, // Default IR length
        }
    }

    /// Get current state
    pub fn get_state(&self) -> TapState {
        self.state
    }

    /// Reset TAP controller
    pub fn reset(&mut self) {
        // Send 5 TMS cycles with TDI=1 to enter Test-Logic-Reset
        self.state = TapState::TestLogicReset;
        self.ir = 0;
    }

    /// Move to Run-Test/Idle state
    pub fn go_to_idle(&mut self) {
        match self.state {
            TapState::TestLogicReset => self.state = TapState::RunTestIdle,
            TapState::RunTestIdle => {}
            _ => self.transition_to_state(TapState::RunTestIdle),
        }
    }

    /// Select DR scan
    pub fn select_dr_scan(&mut self) {
        self.transition_to_state(TapState::SelectDrScan);
    }

    /// Select IR scan
    pub fn select_ir_scan(&mut self) {
        self.transition_to_state(TapState::SelectIrScan);
    }

    /// Shift data register
    pub fn shift_dr(&mut self, data: u64, length: u32) -> u64 {
        self.go_to_idle();
        self.select_dr_scan();
        self.transition_to_state(TapState::CaptureDr);
        self.transition_to_state(TapState::ShiftDr);

        // Shift data (simplified)
        let _ = data; // Would be shifted through TDI/TDO

        self.transition_to_state(TapState::Exit1Dr);
        self.transition_to_state(TapState::UpdateDr);
        self.go_to_idle();

        data // Return what was read back (simplified)
    }

    /// Shift instruction register
    pub fn shift_ir(&mut self, instruction: u8) {
        self.go_to_idle();
        self.select_ir_scan();
        self.transition_to_state(TapState::CaptureIr);
        self.transition_to_state(TapState::ShiftIr);

        self.ir = instruction;

        self.transition_to_state(TapState::Exit1Ir);
        self.transition_to_state(TapState::UpdateIr);
        self.go_to_idle();
    }

    /// Transition to specific state
    fn transition_to_state(&mut self, target: TapState) {
        self.state = target;
        // In a real implementation, this would generate the correct TMS/TDO sequence
    }
}

/// JTAG debug interface
pub struct JtagDebugInterface {
    /// TAP controller
    tap: TapController,
    /// Debug module base address
    dm_base: u64,
    /// Abstract command count
    abstract_cmd_count: u32,
    /// Program buffer size
    progbuf_size: u32,
}

impl JtagDebugInterface {
    /// Create new JTAG debug interface
    pub fn new(dm_base: u64) -> Result<Self, &'static str> {
        let mut jtag = Self {
            tap: TapController::new(),
            dm_base,
            abstract_cmd_count: 0,
            progbuf_size: 0,
        };

        jtag.initialize()?;
        Ok(jtag)
    }

    /// Initialize JTAG interface
    fn initialize(&mut self) -> Result<(), &'static str> {
        log::debug!("Initializing JTAG debug interface");

        // Reset TAP controller
        self.tap.reset();
        self.tap.go_to_idle();

        // Read IDCODE
        self.tap.shift_ir(ir::IDCODE);
        let _idcode = self.tap.shift_dr(0, 32);

        // Read DTMCS to get capabilities
        self.tap.shift_ir(ir::DTMCS);
        let dtmcs_bits = self.tap.shift_dr(0, 32);
        let dtmcs = Dtmcs::from_bits(dtmcs_bits as u32);

        log::debug!("DTM version: {}, address bits: {}", dtmcs.version(), dtmcs.abits());

        // Switch to DMI access
        self.tap.shift_ir(ir::DMI);

        // Read debug module status
        let dmstatus = self.read_dmi(dmi::DMSTATUS)?;

        // Check if debug module is present
        if dmstatus == 0xFFFFFFFF {
            return Err("Debug module not accessible");
        }

        // Read abstract command count
        let abstractcs = self.read_dmi(dmi::ABSTRACTCS)?;
        self.abstract_cmd_count = ((abstractcs >> 24) & 0x7F) as u32;

        // Read program buffer size
        let progbufsize = (abstractcs >> 8) & 0x0F;
        self.progbuf_size = progbufsize as u32;

        log::debug!("Abstract commands: {}, Program buffer size: {}",
                   self.abstract_cmd_count, self.progbuf_size);

        Ok(())
    }

    /// Read DMI register
    fn read_dmi(&mut self, addr: u32) -> Result<u32, &'static str> {
        let dmi_value = (addr << 2) | (1 << 0); // Read operation
        let _result = self.tap.shift_dr(dmi_value as u64, 41);

        // In a real implementation, wait for operation to complete
        // and read the result

        Ok(0) // Placeholder
    }

    /// Write DMI register
    fn write_dmi(&mut self, addr: u32, data: u32) -> Result<(), &'static str> {
        let dmi_value = ((addr << 2) | (data << 2)) | (0 << 0); // Write operation
        let _result = self.tap.shift_dr(dmi_value as u64, 41);

        // In a real implementation, wait for operation to complete

        Ok(())
    }

    /// Halt the target
    pub fn halt(&mut self) -> Result<(), &'static str> {
        log::debug!("Halting target via JTAG");

        // Set halt request in DMCONTROL
        let mut dmcontrol = self.read_dmi(dmi::DMCONTROL)?;
        dmcontrol |= dmcontrol::HALTREQ;
        self.write_dmi(dmi::DMCONTROL, dmcontrol)?;

        // Wait for halt to be acknowledged
        loop {
            let dmstatus = self.read_dmi(dmi::DMSTATUS)?;
            if (dmstatus & dmstatus::ALLHALTED) != 0 {
                break;
            }
        }

        // Clear halt request
        dmcontrol &= !dmcontrol::HALTREQ;
        self.write_dmi(dmi::DMCONTROL, dmcontrol)?;

        log::debug!("Target halted");
        Ok(())
    }

    /// Resume the target
    pub fn resume(&mut self) -> Result<(), &'static str> {
        log::debug!("Resuming target via JTAG");

        // Set resume request in DMCONTROL
        let mut dmcontrol = self.read_dmi(dmi::DMCONTROL)?;
        dmcontrol |= dmcontrol::RESUMEREQ;
        self.write_dmi(dmi::DMCONTROL, dmcontrol)?;

        // Wait for resume to be acknowledged
        loop {
            let dmstatus = self.read_dmi(dmi::DMSTATUS)?;
            if (dmstatus & dmstatus::ALLRUNNING) != 0 {
                break;
            }
        }

        // Clear resume request
        dmcontrol &= !dmcontrol::RESUMEREQ;
        self.write_dmi(dmi::DMCONTROL, dmcontrol)?;

        log::debug!("Target resumed");
        Ok(())
    }

    /// Read GPR register
    pub fn read_gpr(&mut self, reg_num: u32) -> Result<u64, &'static str> {
        if reg_num >= 32 {
            return Err("Invalid register number");
        }

        // Use abstract command to read register
        let cmd = self.build_abstract_read_reg(reg_num)?;
        self.write_dmi(dmi::COMMAND, cmd)?;

        // Wait for command to complete
        loop {
            let cmd_status = self.read_dmi(dmi::ABSTRACTCS)?;
            if (cmd_status & 0x80000000) == 0 { // busy bit cleared
                break;
            }
        }

        // Read result from DATA0
        let value = self.read_dmi(dmi::DATA0)?;
        Ok(value as u64)
    }

    /// Write GPR register
    pub fn write_gpr(&mut self, reg_num: u32, value: u64) -> Result<(), &'static str> {
        if reg_num >= 32 {
            return Err("Invalid register number");
        }

        // Write value to DATA0
        self.write_dmi(dmi::DATA0, (value & 0xFFFFFFFF) as u32)?;
        self.write_dmi(dmi::DATA1, ((value >> 32) & 0xFFFFFFFF) as u32)?;

        // Use abstract command to write register
        let cmd = self.build_abstract_write_reg(reg_num)?;
        self.write_dmi(dmi::COMMAND, cmd)?;

        // Wait for command to complete
        loop {
            let cmd_status = self.read_dmi(dmi::ABSTRACTCS)?;
            if (cmd_status & 0x80000000) == 0 { // busy bit cleared
                break;
            }
        }

        Ok(())
    }

    /// Build abstract command for reading register
    fn build_abstract_read_reg(&self, reg_num: u32) -> Result<u32, &'static str> {
        let mut cmd = ABSTRACT_ACCESS_REGISTER << ABSTRACT_CMD_TYPE_SHIFT;
        cmd |= ABSTRACT_REG_READ;
        cmd |= 2 << ABSTRACT_REG_SIZE_SHIFT; // 32-bit access
        cmd |= reg_num << 16; // Register number

        Ok(cmd)
    }

    /// Build abstract command for writing register
    fn build_abstract_write_reg(&self, reg_num: u32) -> Result<u32, &'static str> {
        let mut cmd = ABSTRACT_ACCESS_REGISTER << ABSTRACT_CMD_TYPE_SHIFT;
        cmd |= ABSTRACT_REG_WRITE;
        cmd |= 2 << ABSTRACT_REG_SIZE_SHIFT; // 32-bit access
        cmd |= reg_num << 16; // Register number

        Ok(cmd)
    }

    /// Execute program buffer
    pub fn execute_program_buffer(&mut self, program: &[u32]) -> Result<(), &'static str> {
        if program.len() > self.progbuf_size as usize {
            return Err("Program too large for program buffer");
        }

        // Write program to program buffer
        for (i, &instruction) in program.iter().enumerate() {
            let addr = dmi::PROGBUF0 + i as u32;
            self.write_dmi(addr, instruction)?;
        }

        // Set abstract command to execute program buffer
        let cmd = ABSTRACT_QUICK_ACCESS << ABSTRACT_CMD_TYPE_SHIFT;
        self.write_dmi(dmi::COMMAND, cmd)?;

        // Wait for completion
        loop {
            let cmd_status = self.read_dmi(dmi::ABSTRACTCS)?;
            if (cmd_status & 0x80000000) == 0 { // busy bit cleared
                break;
            }
        }

        Ok(())
    }

    /// Read memory word
    pub fn read_memory(&mut self, addr: u64) -> Result<u32, &'static str> {
        // Use program buffer to read memory
        let program = [
            0x00002383, // ld t2, 0(tp)
            0x00129073, // sb t2, 0(sp) (store to data0)
            0x00000013, // nop
        ];

        // Set address in tp register
        self.write_gpr(4, addr)?; // tp = x4

        // Execute program
        self.execute_program_buffer(&program)?;

        // Read result from DATA0
        self.read_dmi(dmi::DATA0)
    }

    /// Write memory word
    pub fn write_memory(&mut self, addr: u64, data: u32) -> Result<(), &'static str> {
        // Use program buffer to write memory
        let program = [
            0x00002303, // lw t1, 0(tp)
            0x00602023, // sw t1, 0(sp) (from data0)
            0x00000013, // nop
        ];

        // Set address in tp register
        self.write_gpr(4, addr)?; // tp = x4

        // Write data to DATA0
        self.write_dmi(dmi::DATA0, data)?;

        // Execute program
        self.execute_program_buffer(&program)
    }

    /// Get JTAG status
    pub fn get_status(&self) -> JtagStatus {
        JtagStatus {
            connected: true,
            target_halted: false, // Would need to check DMSTATUS
            target_running: true,
            error_count: 0,
        }
    }
}

/// JTAG status
#[derive(Debug, Clone)]
pub struct JtagStatus {
    /// Is JTAG connected
    pub connected: bool,
    /// Is target halted
    pub target_halted: bool,
    /// Is target running
    pub target_running: bool,
    /// Number of errors
    pub error_count: u32,
}

/// Initialize JTAG debug interface
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing RISC-V JTAG debug interface");

    // JTAG initialization is done on-demand
    log::info!("RISC-V JTAG debug interface initialized");
    Ok(())
}

/// Create JTAG debug interface
pub fn create_interface(dm_base: u64) -> Result<JtagDebugInterface, &'static str> {
    JtagDebugInterface::new(dm_base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dtmcs() {
        let dtmcs = Dtmcs::from_bits(0x00000010);
        assert_eq!(dtmcs.version(), 0);
        assert_eq!(dtmcs.abits(), 0);
        assert!(!dtmcs.dmibusy());
        assert_eq!(dtmcs.dmiop(), 0);
    }

    #[test]
    fn test_tap_controller() {
        let mut tap = TapController::new();
        assert_eq!(tap.get_state(), TapState::TestLogicReset);

        tap.reset();
        assert_eq!(tap.get_state(), TapState::TestLogicReset);

        tap.go_to_idle();
        assert_eq!(tap.get_state(), TapState::RunTestIdle);

        tap.select_dr_scan();
        assert_eq!(tap.get_state(), TapState::SelectDrScan);

        tap.select_ir_scan();
        assert_eq!(tap.get_state(), TapState::SelectIrScan);
    }

    #[test]
    fn test_jtag_status() {
        let status = JtagStatus {
            connected: true,
            target_halted: true,
            target_running: false,
            error_count: 0,
        };
        assert!(status.connected);
        assert!(status.target_halted);
        assert!(!status.target_running);
        assert_eq!(status.error_count, 0);
    }

    #[test]
    fn test_ir_commands() {
        assert_eq!(ir::BYPASS, 0x0F);
        assert_eq!(ir::IDCODE, 0x01);
        assert_eq!(ir::DTMCS, 0x10);
        assert_eq!(ir::DMI, 0x11);
    }

    #[test]
    fn test_dmi_addresses() {
        assert_eq!(dmi::DMSTATUS, 0x04);
        assert_eq!(dmi::DMCONTROL, 0x10);
        assert_eq!(dmi::ABSTRACTCMD0, 0x20);
        assert_eq!(dmi::DATA0, 0x04);
        assert_eq!(dmi::PROGBUF0, 0x20);
    }

    #[test]
    fn test_dmcontrol_fields() {
        assert_eq!(dmcontrol::HALTREQ, 1 << 31);
        assert_eq!(dmcontrol::RESUMEREQ, 1 << 30);
        assert_eq!(dmcontrol::NDRESET, 1 << 1);
        assert_eq!(dmcontrol::FULLRESET, 1 << 0);
    }

    #[test]
    fn test_dmstatus_fields() {
        assert_eq!(dmstatus::IMPEBREAK, 1 << 22);
        assert_eq!(dmstatus::ALLHALTED, 1 << 19);
        assert_eq!(dmstatus::ANYHALTED, 1 << 18);
        assert_eq!(dmstatus::ALLRUNNING, 1 << 17);
        assert_eq!(dmstatus::ANYRUNNING, 1 << 16);
    }

    #[test]
    fn test_abstract_commands() {
        assert_eq!(ABSTRACT_ACCESS_REGISTER, 0x0);
        assert_eq!(ABSTRACT_ACCESS_MEMORY, 0x2);
        assert_eq!(ABSTRACT_QUICK_ACCESS, 0x3);
    }
}