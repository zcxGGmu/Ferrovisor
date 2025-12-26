//! System register access dispatcher for ARM64
//!
//! Decodes Op0, Op1, CRn, CRm, Op2 and routes register accesses.
//! Reference: ARM DDI 0487I.a, D13.1

use super::state::SysRegs;
use crate::Result;

/// System register encoding
///
/// Represents the encoding used for MRS/MSR instructions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SysRegEncoding {
    /// Op0 - [0:2] (Encodes the register name)
    pub op0: u8,
    /// Op1 - [16:18] (Encodes security state)
    pub op1: u8,
    /// CRn - [12:14] (Register class)
    pub crn: u8,
    /// CRm - [8:10] (Register number within class)
    pub crm: u8,
    /// Op2 - [5:7] (Register number)
    pub op2: u8,
}

impl SysRegEncoding {
    /// Create new system register encoding
    pub fn new(op0: u8, op1: u8, crn: u8, crm: u8, op2: u8) -> Self {
        Self {
            op0,
            op1,
            crn,
            crm,
            op2,
        }
    }

    /// Create from ISS (Instruction Specific Syndrome)
    ///
    /// The ISS field in ESR_EL2 contains the system register encoding
    /// for trapped MRS/MSR instructions.
    pub fn from_iss(iss: u32) -> Self {
        Self {
            op0: ((iss >> 14) & 0x3) as u8,
            op1: ((iss >> 11) & 0x7) as u8,
            crn: ((iss >> 10) & 0xF) as u8,
            crm: ((iss >> 1) & 0xF) as u8,
            op2: ((iss >> 5) & 0x7) as u8,
        }
    }

    /// Convert to ISS value
    pub fn to_iss(&self) -> u32 {
        let mut iss = 0u32;
        iss |= (self.op0 as u32 & 0x3) << 14;
        iss |= (self.op1 as u32 & 0x7) << 11;
        iss |= (self.crn as u32 & 0xF) << 10;
        iss |= (self.crm as u32 & 0xF) << 1;
        iss |= (self.op2 as u32 & 0x7) << 5;
        iss
    }

    /// Check if this is a valid system register encoding
    pub fn is_valid(&self) -> bool {
        matches!(self.op0, 2 | 3)
    }
}

/// CP15 register encoding (AArch32)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cp15Encoding {
    /// Opcode 1
    pub opc1: u8,
    /// Opcode 2
    pub opc2: u8,
    /// CRn
    pub crn: u8,
    /// CRm
    pub crm: u8,
}

impl Cp15Encoding {
    /// Create new CP15 encoding
    pub fn new(opc1: u8, opc2: u8, crn: u8, crm: u8) -> Self {
        Self {
            opc1,
            opc2,
            crn,
            crm,
        }
    }
}

/// System register read result
#[derive(Debug, Clone, Copy)]
pub enum RegReadResult {
    /// Successfully read, value in data
    Ok { data: u64 },
    /// Register not found or not implemented
    NotFound,
    /// Register trapped, needs emulation
    Trapped,
    /// Access error
    Error { code: u32 },
}

/// System register write result
#[derive(Debug, Clone, Copy)]
pub enum RegWriteResult {
    /// Successfully written
    Ok,
    /// Register not found or not implemented
    NotFound,
    /// Register trapped, needs emulation
    Trapped,
    /// Write ignored (RAZ/WI)
    Ignored,
    /// Access error
    Error { code: u32 },
}

/// System register dispatcher
pub struct SysRegDispatcher {
    /// System register state
    state: SysRegs,
}

impl SysRegDispatcher {
    /// Create new dispatcher with given state
    pub fn new(state: SysRegs) -> Self {
        Self { state }
    }

    /// Read system register
    ///
    /// # Arguments
    /// * `encoding` - System register encoding
    ///
    /// # Returns
    /// * Read result
    pub fn read_sysreg(&mut self, encoding: SysRegEncoding) -> RegReadResult {
        if !encoding.is_valid() {
            return RegReadResult::Error { code: 1 };
        }

        // Match based on Op0, Op1, CRn, CRm, Op2
        let result = match (encoding.op0, encoding.op1, encoding.crn, encoding.crm, encoding.op2) {
            // MIDR_EL1 - Main ID Register (Op0=3, Op1=0, CRn=0, CRm=0, Op2=0)
            (3, 0, 0, 0, 0) => RegReadResult::Ok { data: self.state.midr_el1 },

            // MPIDR_EL1 - Multiprocessor Affinity (Op0=3, Op1=0, CRn=0, CRm=0, Op2=5)
            (3, 0, 0, 0, 5) => RegReadResult::Ok { data: self.state.mpidr_el1 },

            // SCTLR_EL1 - System Control (Op0=3, Op1=0, CRn=1, CRm=0, Op2=0)
            (3, 0, 1, 0, 0) => RegReadResult::Ok { data: self.state.sctlr_el1 },

            // ACTLR_EL1 - Auxiliary Control (Op0=3, Op1=0, CRn=1, CRm=0, Op2=1)
            (3, 0, 1, 0, 1) => RegReadResult::Ok { data: self.state.actlr_el1 },

            // CPACR_EL1 - Coprocessor Access Control (Op0=3, Op1=0, CRn=1, CRm=0, Op2=2)
            (3, 0, 1, 0, 2) => RegReadResult::Ok { data: self.state.cpacr_el1 },

            // TTBR0_EL1 - Translation Table Base 0 (Op0=3, Op1=0, CRn=2, CRm=0, Op2=0)
            (3, 0, 2, 0, 0) => RegReadResult::Ok { data: self.state.ttbr0_el1 },

            // TTBR1_EL1 - Translation Table Base 1 (Op0=3, Op1=0, CRn=2, CRm=0, Op2=1)
            (3, 0, 2, 0, 1) => RegReadResult::Ok { data: self.state.ttbr1_el1 },

            // TCR_EL1 - Translation Control (Op0=3, Op1=0, CRn=2, CRm=0, Op2=2)
            (3, 0, 2, 0, 2) => RegReadResult::Ok { data: self.state.tcr_el1 },

            // ESR_EL1 - Exception Syndrome (Op0=3, Op1=0, CRn=5, CRm=2, Op2=0)
            (3, 0, 5, 2, 0) => RegReadResult::Ok { data: self.state.esr_el1 },

            // FAR_EL1 - Fault Address (Op0=3, Op1=0, CRn=6, CRm=0, Op2=0)
            (3, 0, 6, 0, 0) => RegReadResult::Ok { data: self.state.far_el1 },

            // PAR_EL1 - Physical Address (Op0=3, Op1=0, CRn=7, CRm=4, Op2=0)
            (3, 0, 7, 4, 0) => RegReadResult::Ok { data: self.state.par_el1 },

            // MAIR_EL1 - Memory Attributes (Op0=3, Op1=0, CRn=10, CRm=2, Op2=0)
            (3, 0, 10, 2, 0) => RegReadResult::Ok { data: self.state.mair_el1 },

            // VBAR_EL1 - Vector Base Address (Op0=3, Op1=0, CRn=12, CRm=0, Op2=0)
            (3, 0, 12, 0, 0) => RegReadResult::Ok { data: self.state.vbar_el1 },

            // CONTEXTIDR_EL1 - Context ID (Op0=3, Op1=0, CRn=13, CRm=0, Op2=1)
            (3, 0, 13, 0, 1) => RegReadResult::Ok { data: self.state.contextidr_el1 },

            // TPIDR_EL0 - Thread ID User RW (Op0=3, Op1=3, CRn=13, CRm=0, Op2=2)
            (3, 3, 13, 0, 2) => RegReadResult::Ok { data: self.state.tpidr_el0 },

            // TPIDRRO_EL0 - Thread ID User RO (Op0=3, Op1=3, CRn=13, CRm=0, Op2=3)
            (3, 3, 13, 0, 3) => RegReadResult::Ok { data: self.state.tpidrro_el0 },

            // TPIDR_EL1 - Thread ID Privileged (Op0=3, Op1=0, CRn=13, CRm=0, Op2=4)
            (3, 0, 13, 0, 4) => RegReadResult::Ok { data: self.state.tpidr_el1 },

            // ICC_SRE_EL1 - GICv3 System Register Enable (RAZ/WI)
            // Emulated as RAZ/WI for compatibility
            (3, 0, 12, 12, 5) => RegReadResult::Ok { data: 0 },

            _ => RegReadResult::NotFound,
        };

        result
    }

    /// Write system register
    ///
    /// # Arguments
    /// * `encoding` - System register encoding
    /// * `data` - Value to write
    ///
    /// # Returns
    /// * Write result
    pub fn write_sysreg(&mut self, encoding: SysRegEncoding, data: u64) -> RegWriteResult {
        if !encoding.is_valid() {
            return RegWriteResult::Error { code: 1 };
        }

        // Match based on Op0, Op1, CRn, CRm, Op2
        let result = match (encoding.op0, encoding.op1, encoding.crn, encoding.crm, encoding.op2) {
            // SCTLR_EL1 - System Control
            (3, 0, 1, 0, 0) => {
                self.state.sctlr_el1 = data;
                RegWriteResult::Ok
            }

            // ACTLR_EL1 - Auxiliary Control
            (3, 0, 1, 0, 1) => {
                self.state.actlr_el1 = data;
                RegWriteResult::Ok
            }

            // CPACR_EL1 - Coprocessor Access Control
            (3, 0, 1, 0, 2) => {
                self.state.cpacr_el1 = data;
                RegWriteResult::Ok
            }

            // TTBR0_EL1 - Translation Table Base 0
            (3, 0, 2, 0, 0) => {
                self.state.ttbr0_el1 = data;
                RegWriteResult::Ok
            }

            // TTBR1_EL1 - Translation Table Base 1
            (3, 0, 2, 0, 1) => {
                self.state.ttbr1_el1 = data;
                RegWriteResult::Ok
            }

            // TCR_EL1 - Translation Control
            (3, 0, 2, 0, 2) => {
                self.state.tcr_el1 = data;
                RegWriteResult::Ok
            }

            // ESR_EL1 - Exception Syndrome (mostly RO, some bits RW)
            (3, 0, 5, 2, 0) => {
                self.state.esr_el1 = data;
                RegWriteResult::Ok
            }

            // FAR_EL1 - Fault Address
            (3, 0, 6, 0, 0) => {
                self.state.far_el1 = data;
                RegWriteResult::Ok
            }

            // PAR_EL1 - Physical Address
            (3, 0, 7, 4, 0) => {
                self.state.par_el1 = data;
                RegWriteResult::Ok
            }

            // MAIR_EL1 - Memory Attributes
            (3, 0, 10, 2, 0) => {
                self.state.mair_el1 = data;
                RegWriteResult::Ok
            }

            // VBAR_EL1 - Vector Base Address
            (3, 0, 12, 0, 0) => {
                self.state.vbar_el1 = data;
                RegWriteResult::Ok
            }

            // CONTEXTIDR_EL1 - Context ID
            (3, 0, 13, 0, 1) => {
                self.state.contextidr_el1 = data;
                RegWriteResult::Ok
            }

            // TPIDR_EL0 - Thread ID User RW
            (3, 3, 13, 0, 2) => {
                self.state.tpidr_el0 = data;
                RegWriteResult::Ok
            }

            // TPIDRRO_EL0 - Thread ID User RO (writes ignored)
            (3, 3, 13, 0, 3) => RegWriteResult::Ignored,

            // TPIDR_EL1 - Thread ID Privileged
            (3, 0, 13, 0, 4) => {
                self.state.tpidr_el1 = data;
                RegWriteResult::Ok
            }

            // ICC_SRE_EL1 - GICv3 System Register Enable (RAZ/WI)
            // Emulated as RAZ/WI for compatibility
            (3, 0, 12, 12, 5) => RegWriteResult::Ignored,

            _ => RegWriteResult::NotFound,
        };

        result
    }

    /// Read CP15 register (AArch32)
    ///
    /// # Arguments
    /// * `encoding` - CP15 register encoding
    ///
    /// # Returns
    /// * Read result
    pub fn read_cp15(&mut self, encoding: Cp15Encoding) -> RegReadResult {
        // CP15 c0 - ID registers
        if encoding.crn == 0 {
            return match (encoding.opc1, encoding.opc2, encoding.crm) {
                // MIDR (c0, c0, 0, 0)
                (_, 0, 0) => RegReadResult::Ok { data: self.state.midr_el1 as u64 },
                // MPIDR (c0, c0, 0, 5)
                (_, 0, 5) => RegReadResult::Ok { data: self.state.mpidr_el1 as u64 },
                _ => RegReadResult::NotFound,
            };
        }

        // CP15 c1 - System control
        if encoding.crn == 1 {
            return match (encoding.opc1, encoding.opc2) {
                // SCTLR (c1, c0, 0, 0)
                (0, 0) => RegReadResult::Ok { data: self.state.sctlr_el1 as u64 },
                // ACTLR (c1, c0, 0, 1)
                (0, 1) => RegReadResult::Ok { data: self.state.actlr_el1 as u64 },
                // CPACR (c1, c0, 0, 2)
                (0, 2) => RegReadResult::Ok { data: self.state.cpacr_el1 as u64 },
                _ => RegReadResult::NotFound,
            };
        }

        RegReadResult::NotFound
    }

    /// Write CP15 register (AArch32)
    ///
    /// # Arguments
    /// * `encoding` - CP15 register encoding
    /// * `data` - Value to write
    ///
    /// # Returns
    /// * Write result
    pub fn write_cp15(&mut self, encoding: Cp15Encoding, data: u32) -> RegWriteResult {
        // CP15 c1 - System control
        if encoding.crn == 1 {
            return match (encoding.opc1, encoding.opc2) {
                // SCTLR (c1, c0, 0, 0)
                (0, 0) => {
                    self.state.sctlr_el1 = data as u64;
                    RegWriteResult::Ok
                }
                // ACTLR (c1, c0, 0, 1)
                (0, 1) => {
                    self.state.actlr_el1 = data as u64;
                    RegWriteResult::Ok
                }
                // CPACR (c1, c0, 0, 2)
                (0, 2) => {
                    self.state.cpacr_el1 = data as u64;
                    RegWriteResult::Ok
                }
                _ => RegWriteResult::NotFound,
            };
        }

        // CP15 c7 - Cache operations
        if encoding.crn == 7 {
            // Handle cache maintenance operations
            return match (encoding.crm, encoding.opc2) {
                // ICIMVAU - Invalidate instruction cache by VA to PoU
                (5, 1) => RegWriteResult::Ignored,
                // DCIMVAC - Invalidate data cache by VA to PoC
                (6, 1) => RegWriteResult::Ignored,
                // DCCMVAC - Clean data cache by VA to PoC
                (10, 1) => RegWriteResult::Ignored,
                // DCCIMVAC - Clean and invalidate data cache by VA to PoC
                (14, 1) => RegWriteResult::Ignored,
                _ => RegWriteResult::NotFound,
            };
        }

        RegWriteResult::NotFound
    }

    /// Get reference to system register state
    pub fn state(&self) -> &SysRegs {
        &self.state
    }

    /// Get mutable reference to system register state
    pub fn state_mut(&mut self) -> &mut SysRegs {
        &mut self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sys_reg_encoding() {
        let enc = SysRegEncoding::new(3, 0, 1, 0, 0);
        assert_eq!(enc.op0, 3);
        assert_eq!(enc.op1, 0);
        assert_eq!(enc.crn, 1);
        assert_eq!(enc.crm, 0);
        assert_eq!(enc.op2, 0);
        assert!(enc.is_valid());
    }

    #[test]
    fn test_sys_reg_encoding_iss() {
        let enc = SysRegEncoding::new(3, 0, 1, 0, 0);
        let iss = enc.to_iss();
        let enc2 = SysRegEncoding::from_iss(iss);
        assert_eq!(enc.op0, enc2.op0);
        assert_eq!(enc.op1, enc2.op1);
        assert_eq!(enc.crn, enc2.crn);
        assert_eq!(enc.crm, enc2.crm);
        assert_eq!(enc.op2, enc2.op2);
    }

    #[test]
    fn test_dispatcher_read() {
        let state = SysRegs::new();
        let mut disp = SysRegDispatcher::new(state);
        let enc = SysRegEncoding::new(3, 0, 1, 0, 0); // SCTLR_EL1

        let result = disp.read_sysreg(enc);
        assert!(matches!(result, RegReadResult::Ok { .. }));
    }

    #[test]
    fn test_dispatcher_write() {
        let state = SysRegs::new();
        let mut disp = SysRegDispatcher::new(state);
        let enc = SysRegEncoding::new(3, 0, 1, 0, 0); // SCTLR_EL1

        let result = disp.write_sysreg(enc, 0xC00800);
        assert!(matches!(result, RegWriteResult::Ok));
        assert_eq!(disp.state().sctlr_el1, 0xC00800);
    }

    #[test]
    fn test_cp15_encoding() {
        let enc = Cp15Encoding::new(0, 0, 1, 0);
        assert_eq!(enc.opc1, 0);
        assert_eq!(enc.opc2, 0);
        assert_eq!(enc.crn, 1);
        assert_eq!(enc.crm, 0);
    }

    #[test]
    fn test_invalid_encoding() {
        let enc = SysRegEncoding::new(1, 0, 1, 0, 0); // Op0=1 is invalid
        assert!(!enc.is_valid());
    }
}
