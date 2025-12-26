//! System register trap handling for ARM64
//!
//! Handles HSTR_EL2, CPTR_EL2, and other trap configurations.
//! Reference: xvisor/arch/arm/cpu/arm64/cpu_vcpu_sysregs.c

use super::state::{SysRegs, TrapState};
use super::dispatch::{SysRegEncoding, Cp15Encoding, RegReadResult, RegWriteResult};
use crate::Result;

/// HSTR_EL2 bit definitions
pub mod hstr_el2 {
    /// Trap CP15 accesses to specified c*n* registers
    /// Each bit n corresponds to CP15 c*n* registers
    pub const T0: u32 = 1 << 0;
    pub const T1: u32 = 1 << 1;
    pub const T2: u32 = 1 << 2;
    pub const T3: u32 = 1 << 3;
    pub const T4: u32 = 1 << 4;
    pub const T5: u32 = 1 << 5;
    pub const T6: u32 = 1 << 6;
    pub const T7: u32 = 1 << 7;
    pub const T8: u32 = 1 << 8;
    pub const T9: u32 = 1 << 9;
    pub const T10: u32 = 1 << 10;
    pub const T11: u32 = 1 << 11;
    pub const T12: u32 = 1 << 12;
    pub const T13: u32 = 1 << 13;
    pub const T14: u32 = 1 << 14;
    pub const T15: u32 = 1 << 15;
}

/// CPTR_EL2 bit definitions
pub mod cptr_el2 {
    /// Trap FP/SIMD accesses at EL1/EL0 (bit 10)
    pub const TFP: u32 = 1 << 10;
    /// Trap Advanced SIMD/Floating-point to EL2 (bit 10, alias for TFP)
    pub const TFP_SHIFT: u32 = 10;

    /// Trap system register accesses at EL1/EL0 (bit 20)
    pub const TTA: u32 = 1 << 20;
    /// Trap trace system register accesses (bit 20, alias for TTA)
    pub const TTA_SHIFT: u32 = 20;

    /// TCPAC - Trap EL1 access to CPACR (bit 31)
    pub const TCPAC: u32 = 1 << 31;
}

/// Trap types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapType {
    /// System register access trap (MRS/MSR)
    SysReg,
    /// CP15 access trap (MRC/MCR)
    Cp15,
    /// CP14 access trap (debug registers)
    Cp14,
    /// FP/SIMD access trap
    FpSimd,
    /// Trace access trap
    Trace,
}

/// Trap handler for system register accesses
pub struct TrapHandler {
    /// Trap state
    trap_state: TrapState,
}

impl TrapHandler {
    /// Create new trap handler
    pub fn new() -> Self {
        Self {
            trap_state: TrapState::new(),
        }
    }

    /// Initialize trap configuration for VCPU
    ///
    /// Sets up default trap bits for safe virtualization.
    pub fn init_traps(&mut self) {
        // Default trap configuration:
        // - Trap all CP15 accesses initially
        self.trap_state.hstr_el2 = 0xFFFF;

        // - Don't trap FP/SIMD by default (for performance)
        // - Don't trap trace registers by default

        log::debug!("Initialized trap state: HSTR=0x{:04x}, CPTR=0x{:08x}",
                    self.trap_state.hstr_el2, self.trap_state.cptr_el2);
    }

    /// Configure HSTR_EL2 trap bits
    ///
    /// # Arguments
    /// * `mask` - Trap mask (bits 0-15 for c0-c15)
    pub fn set_hstr_traps(&mut self, mask: u32) {
        self.trap_state.hstr_el2 = mask;
    }

    /// Configure CPTR_EL2 trap bits
    ///
    /// # Arguments
    /// * `tfp` - Trap FP/SIMD
    /// * `tta` - Trap trace registers
    /// * `tcpac` - Trap CPACR access
    pub fn set_cptr_traps(&mut self, tfp: bool, tta: bool, tcpac: bool) {
        self.trap_state.cptr_el2 = 0;
        if tfp {
            self.trap_state.cptr_el2 |= cptr_el2::TFP;
        }
        if tta {
            self.trap_state.cptr_el2 |= cptr_el2::TTA;
        }
        if tcpac {
            self.trap_state.cptr_el2 |= cptr_el2::TCPAC;
        }
    }

    /// Check if CP15 access is trapped
    ///
    /// # Arguments
    /// * `encoding` - CP15 register encoding
    ///
    /// # Returns
    /// * `true` if access is trapped
    pub fn is_cp15_trapped(&self, encoding: Cp15Encoding) -> bool {
        let trap_bit = 1u32 << (encoding.crn & 0xF);
        (self.trap_state.hstr_el2 & trap_bit) != 0
    }

    /// Check if system register access is trapped
    ///
    /// # Arguments
    /// * `encoding` - System register encoding
    ///
    /// # Returns
    /// * `true` if access is trapped
    pub fn is_sysreg_trapped(&self, encoding: SysRegEncoding) -> bool {
        // Most EL1 system registers should be trapped for virtualization
        // Check based on register type
        match (encoding.op0, encoding.crn) {
            // ID registers (c0) - not trapped
            (3, 0) => false,

            // System control (c1) - trapped
            (3, 1) => true,

            // MMU registers (c2) - trapped
            (3, 2) => true,

            // Exception handling (c5, c6) - trapped
            (3, 5) | (3, 6) => true,

            // Other registers - trapped by default
            _ => true,
        }
    }

    /// Check if FP/SIMD access is trapped
    pub fn is_fpsimd_trapped(&self) -> bool {
        (self.trap_state.cptr_el2 & cptr_el2::TFP) != 0
    }

    /// Handle trapped system register read
    ///
    /// # Arguments
    /// * `encoding` - System register encoding
    /// * `state` - System register state
    ///
    /// # Returns
    /// * Register value or error
    pub fn handle_sysreg_read(
        &self,
        encoding: SysRegEncoding,
        state: &SysRegs,
    ) -> Result<u64, &'static str> {
        // Handle specific emulated registers
        match (encoding.op0, encoding.op1, encoding.crn, encoding.crm, encoding.op2) {
            // ACTLR_EL1 - Read from saved state
            (3, 0, 1, 0, 1) => Ok(state.actlr_el1),

            // ICC_SRE_EL1 - RAZ (emulated for GICv3 compatibility)
            (3, 0, 12, 12, 5) => Ok(0),

            _ => {
                log::warn!("Unimplemented sysreg read: Op0={} Op1={} CRn={} CRm={} Op2={}",
                          encoding.op0, encoding.op1, encoding.crn, encoding.crm, encoding.op2);
                Err("Unimplemented system register read")
            }
        }
    }

    /// Handle trapped system register write
    ///
    /// # Arguments
    /// * `encoding` - System register encoding
    /// * `value` - Value to write
    /// * `state` - System register state (mutable)
    ///
    /// # Returns
    /// * Success or error
    pub fn handle_sysreg_write(
        &self,
        encoding: SysRegEncoding,
        value: u64,
        state: &mut SysRegs,
    ) -> Result<(), &'static str> {
        // Handle specific emulated registers
        match (encoding.op0, encoding.op1, encoding.crn, encoding.crm, encoding.op2) {
            // ACTLR_EL1 - Write to saved state
            (3, 0, 1, 0, 1) => {
                state.actlr_el1 = value;
                Ok(())
            }

            // ICC_SRE_EL1 - WI (emulated for GICv3 compatibility)
            (3, 0, 12, 12, 5) => {
                // Ignore writes
                Ok(())
            }

            // Cache maintenance operations
            // These are typically NOP in virtualization context
            (3, 0, 7, _, _) => {
                // DCISW, DCCISW, DCCSW - emulate as NOP
                log::debug!("Cache maintenance op (emulated as NOP)");
                Ok(())
            }

            _ => {
                log::warn!("Unimplemented sysreg write: Op0={} Op1={} CRn={} CRm={} Op2={} value={:#x}",
                          encoding.op0, encoding.op1, encoding.crn, encoding.crm, encoding.op2, value);
                Err("Unimplemented system register write")
            }
        }
    }

    /// Handle trapped CP15 read
    ///
    /// # Arguments
    /// * `encoding` - CP15 register encoding
    /// * `state` - System register state
    ///
    /// # Returns
    /// * Register value or error
    pub fn handle_cp15_read(
        &self,
        encoding: Cp15Encoding,
        state: &SysRegs,
    ) -> Result<u32, &'static str> {
        match (encoding.crn, encoding.opc1, encoding.opc2, encoding.crm) {
            // MIDR
            (0, _, 0, 0) => Ok((state.midr_el1 & 0xFFFFFFFF) as u32),

            // MPIDR
            (0, _, 0, 5) => Ok((state.mpidr_el1 & 0xFFFFFFFF) as u32),

            // SCTLR
            (1, 0, 0, 0) => Ok(state.sctlr_el1 as u32),

            // ACTLR
            (1, 0, 1, 0) => Ok(state.actlr_el1 as u32),

            // CPACR
            (1, 0, 2, 0) => Ok(state.cpacr_el1 as u32),

            // CBAR - Configuration Base Address
            // Return a dummy value for compatibility
            (15, 4, _, _) => Ok(0),

            _ => {
                log::warn!("Unimplemented CP15 read: c{} opc1={} opc2={} c{}",
                          encoding.crn, encoding.opc1, encoding.opc2, encoding.crm);
                Err("Unimplemented CP15 read")
            }
        }
    }

    /// Handle trapped CP15 write
    ///
    /// # Arguments
    /// * `encoding` - CP15 register encoding
    /// * `value` - Value to write
    /// * `state` - System register state (mutable)
    ///
    /// # Returns
    /// * Success or error
    pub fn handle_cp15_write(
        &self,
        encoding: Cp15Encoding,
        value: u32,
        state: &mut SysRegs,
    ) -> Result<(), &'static str> {
        match (encoding.crn, encoding.crm, encoding.opc1, encoding.opc2) {
            // SCTLR
            (1, 0, 0, 0) => {
                state.sctlr_el1 = value as u64;
                Ok(())
            }

            // ACTLR
            (1, 0, 0, 1) => {
                state.actlr_el1 = value as u64;
                Ok(())
            }

            // CPACR
            (1, 0, 0, 2) => {
                state.cpacr_el1 = value as u64;
                Ok(())
            }

            // Cache maintenance operations
            // DCISW, DCCISW, DCCSW - emulate as NOP
            (7, 6, 0, 2) |  // DCISW
            (7, 14, 0, 2) | // DCCISW
            (7, 10, 0, 2)   // DCCSW
            => {
                log::debug!("CP15 cache maintenance (emulated as NOP)");
                Ok(())
            }

            _ => {
                log::warn!("Unimplemented CP15 write: c{} c{} opc1={} opc2={} value={:#x}",
                          encoding.crn, encoding.crm, encoding.opc1, encoding.opc2, value);
                Err("Unimplemented CP15 write")
            }
        }
    }

    /// Get trap state
    pub fn trap_state(&self) -> &TrapState {
        &self.trap_state
    }

    /// Get mutable trap state
    pub fn trap_state_mut(&mut self) -> &mut TrapState {
        &mut self.trap_state
    }
}

impl Default for TrapHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trap_handler_init() {
        let handler = TrapHandler::new();
        handler.init_traps();
        assert_eq!(handler.trap_state().hstr_el2, 0xFFFF);
    }

    #[test]
    fn test_hstr_bits() {
        assert_eq!(hstr_el2::T0, 1 << 0);
        assert_eq!(hstr_el2::T1, 1 << 1);
        assert_eq!(hstr_el2::T15, 1 << 15);
    }

    #[test]
    fn test_cptr_bits() {
        assert_eq!(cptr_el2::TFP, 1 << 10);
        assert_eq!(cptr_el2::TTA, 1 << 20);
        assert_eq!(cptr_el2::TCPAC, 1 << 31);
    }

    #[test]
    fn test_cp15_trap_check() {
        let mut handler = TrapHandler::new();
        handler.init_traps();

        let enc = Cp15Encoding::new(0, 0, 1, 0);
        assert!(handler.is_cp15_trapped(enc));
    }

    #[test]
    fn test_set_cptr_traps() {
        let mut handler = TrapHandler::new();
        handler.set_cptr_traps(true, false, false);

        assert!(handler.is_fpsimd_trapped());
        assert!((handler.trap_state().cptr_el2 & cptr_el2::TTA) != 0);
    }

    #[test]
    fn test_handle_sysreg_read() {
        let handler = TrapHandler::new();
        let state = SysRegs::init_default();
        let enc = SysRegEncoding::new(3, 0, 1, 0, 1); // ACTLR_EL1

        let result = handler.handle_sysreg_read(enc, &state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_sysreg_write() {
        let handler = TrapHandler::new();
        let mut state = SysRegs::init_default();
        let enc = SysRegEncoding::new(3, 0, 1, 0, 1); // ACTLR_EL1

        let result = handler.handle_sysreg_write(enc, 0x1234, &mut state);
        assert!(result.is_ok());
        assert_eq!(state.actlr_el1, 0x1234);
    }
}
