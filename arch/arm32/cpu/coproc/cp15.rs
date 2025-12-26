//! CP15 Coprocessor Emulation for ARMv7
//!
//! Provides system control coprocessor (CP15) emulation for ARMv7/ARMv8-AArch32 guests.
//! Reference: ARM DDI 0406C.d - Chapter B3 - System Control Programmers' Model
//!
//! CP15 contains:
//! - Identification registers (MIDR, MPIDR, cache type ID registers)
//! - System control registers (SCTLR, CPACR)
//! - MMU registers (TTBR0, TTBR1, TTBCR, DACR)
//! - Fault status/address registers
//! - Performance monitor registers
//! - TLS registers

use crate::arch::arm64::cpu::sysreg::{RegReadResult, RegWriteResult};

/// CPU ID values for ARM processors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ArmCpuId {
    Arm1026 = 0x4106A265,
    Arm1136 = 0x4117B363,
    Arm1136R2 = 0x4117B364,
    Arm11MPCore = 0x410FB024,
    CortexA8 = 0x410FC080,
    CortexA9 = 0x410FC090,
    CortexA7 = 0x410FC070,
    CortexA15 = 0x410FC0F0,
    ArmV7 = 0x410FC000, /* Generic ARMv7 */
    Unknown = 0x00000000,
}

impl ArmCpuId {
    pub fn from_u32(val: u32) -> Self {
        match val & 0xFF0FFFF0 {
            0x4106A260 => Self::Arm1026,
            0x4117B360 => Self::Arm1136,
            0x410FB020 => Self::Arm11MPCore,
            0x410FC080 => Self::CortexA8,
            0x410FC090 => Self::CortexA9,
            0x410FC070 => Self::CortexA7,
            0x410FC0F0 => Self::CortexA15,
            _ => Self::Unknown,
        }
    }

    /// Get Cortex version
    pub fn cortex_version(&self) -> u32 {
        match self {
            Self::CortexA8 => 8,
            Self::CortexA9 => 9,
            Self::CortexA7 => 7,
            Self::CortexA15 => 15,
            _ => 0,
        }
    }

    /// Check if this is a Cortex-A series CPU
    pub fn is_cortex(&self) -> bool {
        matches!(self, Self::CortexA8 | Self::CortexA9 | Self::CortexA7 | Self::CortexA15)
    }
}

/// CP15 register encoding for coprocessor instructions
///
/// The encoding format is: MCR/MRC p15, <opc1>, <Rt>, <CRn>, <CRm>, <opc2>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Cp15Encoding {
    /// Operand 1 (0-7)
    pub opc1: u8,
    /// Operand 2 (0-7)
    pub opc2: u8,
    /// Coprocessor Register N (0-15)
    pub crn: u8,
    /// Coprocessor Register M (0-15)
    pub crm: u8,
}

impl Cp15Encoding {
    pub const fn new(opc1: u8, opc2: u8, crn: u8, crm: u8) -> Self {
        Self { opc1, opc2, crn, crm }
    }

    /// Create from raw instruction encoding
    pub fn from_inst(inst: u32) -> Self {
        Self {
            opc1: ((inst >> 21) & 0x7) as u8,
            crn: ((inst >> 16) & 0xF) as u8,
            crm: ((inst >> 0) & 0xF) as u8,
            opc2: ((inst >> 5) & 0x7) as u8,
        }
    }

    /// Convert to instruction encoding
    pub fn to_inst(&self, rt: u8) -> u32 {
        // MRC/MCR p15 instruction template
        // This is a simplified version for reference
        (0xEE000000) // Base MRC/MCR pattern
            | ((self.opc1 as u32) << 21)
            | ((self.crn as u32) << 16)
            | ((self.crm as u32) << 0)
            | ((self.opc2 as u32) << 5)
            | ((rt as u32) << 12)
    }
}

/// CP15 Identification Registers
///
/// Contains CPU feature and identification registers
#[derive(Debug, Clone)]
#[repr(C)]
pub struct Cp15IdRegs {
    /// MIDR - Main ID Register
    pub midr: u32,
    /// MPIDR - Multiprocessor ID Register
    pub mpidr: u32,
    /// CTR - Cache Type Register
    pub cachetype: u32,
    /// PFR0 - Processor Feature Register 0
    pub pfr0: u32,
    /// PFR1 - Processor Feature Register 1
    pub pfr1: u32,
    /// DFR0 - Debug Feature Register 0
    pub dfr0: u32,
    /// AFR0 - Auxiliary Feature Register 0
    pub afr0: u32,
    /// MMFR0 - Memory Model Feature Register 0
    pub mmfr0: u32,
    /// MMFR1 - Memory Model Feature Register 1
    pub mmfr1: u32,
    /// MMFR2 - Memory Model Feature Register 2
    pub mmfr2: u32,
    /// MMFR3 - Memory Model Feature Register 3
    pub mmfr3: u32,
    /// ISAR0 - Instruction Set Attribute Register 0
    pub isar0: u32,
    /// ISAR1 - Instruction Set Attribute Register 1
    pub isar1: u32,
    /// ISAR2 - Instruction Set Attribute Register 2
    pub isar2: u32,
    /// ISAR3 - Instruction Set Attribute Register 3
    pub isar3: u32,
    /// ISAR4 - Instruction Set Attribute Register 4
    pub isar4: u32,
    /// ISAR5 - Instruction Set Attribute Register 5
    pub isar5: u32,
    /// CCSIDR - Cache Size ID Registers (16 entries)
    pub ccsid: [u32; 16],
    /// CLIDR - Cache Level ID Register
    pub clid: u32,
    /// CSSELR - Cache Size Selection Register
    pub cssel: u32,
}

impl Default for Cp15IdRegs {
    fn default() -> Self {
        Self {
            midr: 0x410FC000,  // Generic ARMv7
            mpidr: 0x80000000, // CPU 0, MP extensions
            cachetype: 0x8444c004,
            pfr0: 0x00001131,
            pfr1: 0x00011011,
            dfr0: 0x02010555,
            afr0: 0x00000000,
            mmfr0: 0x10201105,
            mmfr1: 0x20000000,
            mmfr2: 0x01240000,
            mmfr3: 0x02102211,
            isar0: 0x02101110,
            isar1: 0x13112111,
            isar2: 0x21232041,
            isar3: 0x11112131,
            isar4: 0x10011142,
            isar5: 0x00000000,
            ccsid: [0; 16],
            clid: 0x0a200023,
            cssel: 0x00000000,
        }
    }
}

impl Cp15IdRegs {
    /// Create ID registers for specific CPU
    pub fn for_cpu(cpuid: ArmCpuId) -> Self {
        let mut regs = Self::default();
        regs.midr = cpuid as u32;

        match cpuid {
            ArmCpuId::CortexA8 => {
                regs.cachetype = 0x82048004;
                regs.pfr0 = 0x1031;
                regs.pfr1 = 0x11;
                regs.dfr0 = 0x400;
                regs.mmfr0 = 0x31100003;
                regs.mmfr1 = 0x20000000;
                regs.mmfr2 = 0x01202000;
                regs.mmfr3 = 0x11;
                regs.isar0 = 0x00101111;
                regs.isar1 = 0x12112111;
                regs.isar2 = 0x21232031;
                regs.isar3 = 0x11112131;
                regs.isar4 = 0x00111142;
                regs.clid = (1 << 27) | (2 << 24) | 3;
                regs.ccsid[0] = 0xe007e01a; // 16K L1 dcache
                regs.ccsid[1] = 0x2007e01a; // 16K L1 icache
                regs.ccsid[2] = 0xf0000000; // No L2 icache
            }
            ArmCpuId::CortexA9 => {
                // Fake PartNum and Revision for ARM32 Linux compatibility
                regs.midr = (ArmCpuId::CortexA9 as u32) & 0xFF00FFFF;
                regs.cachetype = 0x80038003;
                regs.pfr0 = 0x1031;
                regs.pfr1 = 0x11;
                regs.dfr0 = 0x000;
                regs.mmfr0 = 0x00100103;
                regs.mmfr1 = 0x20000000;
                regs.mmfr2 = 0x01230000;
                regs.mmfr3 = 0x00002111;
                regs.isar0 = 0x00101111;
                regs.isar1 = 0x13112111;
                regs.isar2 = 0x21232041;
                regs.isar3 = 0x11112131;
                regs.isar4 = 0x00111142;
                regs.clid = (1 << 27) | (1 << 24) | 3;
                regs.ccsid[0] = 0xe00fe015; // 16K L1 dcache
                regs.ccsid[1] = 0x200fe015; // 16K L1 icache
            }
            ArmCpuId::CortexA7 | ArmCpuId::CortexA15 => {
                regs.cachetype = 0x8444c004;
                regs.pfr0 = 0x00001131;
                regs.pfr1 = 0x00011011;
                regs.dfr0 = 0x02010555;
                regs.mmfr0 = 0x10201105;
                regs.mmfr1 = 0x20000000;
                regs.mmfr2 = 0x01240000;
                regs.mmfr3 = 0x02102211;
                regs.isar0 = 0x02101110;
                regs.isar1 = 0x13112111;
                regs.isar2 = 0x21232041;
                regs.isar3 = 0x11112131;
                regs.isar4 = 0x10011142;
                regs.clid = 0x0a200023;
                regs.ccsid[0] = 0x701fe00a; // 32K L1 dcache
                regs.ccsid[1] = 0x201fe00a; // 32K L1 icache
                regs.ccsid[2] = 0x711fe07a; // 4096K L2 unified cache
            }
            _ => {
                // Use defaults (Generic ARMv7)
            }
        }

        regs
    }
}

/// CP15 System Control Registers
#[derive(Debug, Clone)]
#[repr(C)]
pub struct Cp15CtrlRegs {
    /// SCTLR - System Control Register
    pub sctlr: u32,
    /// CPACR - Coprocessor Access Control Register
    pub cpacr: u32,
}

impl Default for Cp15CtrlRegs {
    fn default() -> Self {
        Self {
            sctlr: 0x00C50078, // Default ARMv7 SCTLR (MMU disabled)
            cpacr: 0x00000000, // CP10/CP11 (VFP) access denied
        }
    }
}

/// CP15 MMU Registers
#[derive(Debug, Clone)]
#[repr(C)]
pub struct Cp15MmuRegs {
    /// TTBCR - Translation Table Base Control Register
    pub ttbcr: u32,
    /// TTBR0 - Translation Table Base Register 0
    pub ttbr0: u64,
    /// TTBR1 - Translation Table Base Register 1
    pub ttbr1: u64,
    /// DACR - Domain Access Control Register
    pub dacr: u32,
}

impl Default for Cp15MmuRegs {
    fn default() -> Self {
        Self {
            ttbcr: 0x00000000,
            ttbr0: 0x00000000,
            ttbr1: 0x00000000,
            dacr: 0x00000000,
        }
    }
}

/// CP15 Fault Registers
#[derive(Debug, Clone)]
#[repr(C)]
pub struct Cp15FaultRegs {
    /// IFSR - Instruction Fault Status Register
    pub ifsr: u32,
    /// DFSR - Data Fault Status Register
    pub dfsr: u32,
    /// AIFSR - Auxiliary Instruction Fault Status Register
    pub aifsr: u32,
    /// ADFSR - Auxiliary Data Fault Status Register
    pub adfsr: u32,
    /// IFAR - Instruction Fault Address Register
    pub ifar: u32,
    /// DFAR - Data Fault Address Register
    pub dfar: u32,
}

impl Default for Cp15FaultRegs {
    fn default() -> Self {
        Self {
            ifsr: 0x00000000,
            dfsr: 0x00000000,
            aifsr: 0x00000000,
            adfsr: 0x00000000,
            ifar: 0x00000000,
            dfar: 0x00000000,
        }
    }
}

/// CP15 Address Translation Registers
#[derive(Debug, Clone)]
#[repr(C)]
pub struct Cp15TranslateRegs {
    /// PAR - Physical Address Register (32-bit)
    pub par: u32,
    /// PAR64 - Physical Address Register (64-bit)
    pub par64: u64,
}

impl Default for Cp15TranslateRegs {
    fn default() -> Self {
        Self {
            par: 0x00000000,
            par64: 0x00000000,
        }
    }
}

/// CP15 Performance Monitor Registers
#[derive(Debug, Clone)]
#[repr(C)]
pub struct Cp15PerfRegs {
    /// PMCR - Performance Monitor Control Register
    pub pmcr: u32,
    /// PMCNTEN - Count Enable Register
    pub pmcnten: u32,
    /// PMOVSR - Overflow Flag Status Register
    pub pmovsr: u32,
    /// PMXEVTYPER - Event Type Selection Register
    pub pmxevtyper: u32,
    /// PMUSERENR - User Enable Register
    pub pmuserenr: u32,
    /// PMINTEN - Interrupt Enable Register
    pub pminten: u32,
    /// Instruction cache lockdown
    pub insn_lock: u32,
    /// Data cache lockdown
    pub data_lock: u32,
}

impl Default for Cp15PerfRegs {
    fn default() -> Self {
        Self {
            pmcr: 0x00000000,
            pmcnten: 0x00000000,
            pmovsr: 0x00000000,
            pmxevtyper: 0x00000000,
            pmuserenr: 0x00000000,
            pminten: 0x00000000,
            insn_lock: 0x00000000,
            data_lock: 0x00000000,
        }
    }
}

/// CP15 Memory Attribute Registers
#[derive(Debug, Clone)]
#[repr(C)]
pub struct Cp15AttrRegs {
    /// PRRR - Primary Region Remap Register
    pub prrr: u32,
    /// NMRR - Normal Memory Remap Register
    pub nmrr: u32,
}

impl Default for Cp15AttrRegs {
    fn default() -> Self {
        Self {
            prrr: 0x00000000,
            nmrr: 0x00000000,
        }
    }
}

/// CP15 TLS and Other Registers
#[derive(Debug, Clone)]
#[repr(C)]
pub struct Cp15TlsRegs {
    /// VBAR - Vector Base Address Register
    pub vbar: u32,
    /// FCSEIDR - FCSE Process ID Register
    pub fcseidr: u32,
    /// CONTEXTIDR - Context ID Register
    pub contextidr: u32,
    /// TPIDRURO - Thread ID Register User RO
    pub tpidruro: u32,
    /// TPIDRURW - Thread ID Register User RW
    pub tpidrurw: u32,
    /// TPIDRPRW - Thread ID Register Privileged RW
    pub tpidrprw: u32,
}

impl Default for Cp15TlsRegs {
    fn default() -> Self {
        Self {
            vbar: 0x00000000,
            fcseidr: 0x00000000,
            contextidr: 0x00000000,
            tpidruro: 0x00000000,
            tpidrurw: 0x00000000,
            tpidrprw: 0x00000000,
        }
    }
}

/// CP15 Register State for a VCPU
///
/// This contains all CP15 coprocessor registers for an ARMv7/ARMv8-AArch32 VCPU.
/// Reference: xvisor/arch/arm/include/arch_regs.h:struct arm_priv_cp15
#[derive(Debug, Clone)]
pub struct Cp15Regs {
    /// Identification registers
    pub id: Cp15IdRegs,
    /// System control registers
    pub ctrl: Cp15CtrlRegs,
    /// MMU registers
    pub mmu: Cp15MmuRegs,
    /// Fault registers
    pub fault: Cp15FaultRegs,
    /// Address translation registers
    pub translate: Cp15TranslateRegs,
    /// Performance monitor registers
    pub perf: Cp15PerfRegs,
    /// Memory attribute registers
    pub attr: Cp15AttrRegs,
    /// TLS and other registers
    pub tls: Cp15TlsRegs,
}

impl Default for Cp15Regs {
    fn default() -> Self {
        Self {
            id: Cp15IdRegs::default(),
            ctrl: Cp15CtrlRegs::default(),
            mmu: Cp15MmuRegs::default(),
            fault: Cp15FaultRegs::default(),
            translate: Cp15TranslateRegs::default(),
            perf: Cp15PerfRegs::default(),
            attr: Cp15AttrRegs::default(),
            tls: Cp15TlsRegs::default(),
        }
    }
}

impl Cp15Regs {
    /// Create new CP15 registers for specific CPU
    pub fn for_cpu(cpuid: ArmCpuId, vcpu_id: u32) -> Self {
        let mut regs = Self::default();
        regs.id = Cp15IdRegs::for_cpu(cpuid);

        // Set MPIDR with VCPU ID
        regs.id.mpidr = (1 << 31) | vcpu_id;

        // Set PMCR with CPU ID
        regs.perf.pmcr = cpuid as u32 & 0xFF000000;

        regs
    }

    /// Read a CP15 register by encoding
    pub fn read(&self, encoding: Cp15Encoding) -> RegReadResult {
        match encoding.crn {
            // CRn=0: ID registers
            0 => self.read_id_reg(encoding),

            // CRn=1: System control registers
            1 => self.read_ctrl_reg(encoding),

            // CRn=2: MMU translation table base
            2 => self.read_ttb_reg(encoding),

            // CRn=3: Domain access control
            3 => RegReadResult::Ok { data: self.mmu.dacr },

            // CRn=5: Fault status registers
            5 => self.read_fault_status(encoding),

            // CRn=6: Fault address registers
            6 => self.read_fault_addr(encoding),

            // CRn=7: Address translation and cache operations
            7 => self.read_translate_reg(encoding),

            // CRn=9: Performance monitor and cache lockdown
            9 => self.read_perf_reg(encoding),

            // CRn=10: Memory attributes
            10 => self.read_attr_reg(encoding),

            // CRn=12: VBAR and security extensions
            12 => RegReadResult::Ok { data: self.tls.vbar },

            // CRn=13: Process ID and TLS registers
            13 => self.read_tls_reg(encoding),

            // CRn=15: Implementation-defined registers
            15 => self.read_impl_reg(encoding),

            _ => RegReadResult::Unimplemented,
        }
    }

    /// Write to a CP15 register by encoding
    pub fn write(&mut self, encoding: Cp15Encoding, value: u32) -> RegWriteResult {
        match encoding.crn {
            // CRn=0: ID registers (read-only)
            0 => RegWriteResult::ReadOnly,

            // CRn=1: System control registers
            1 => self.write_ctrl_reg(encoding, value),

            // CRn=2: MMU translation table base
            2 => self.write_ttb_reg(encoding, value),

            // CRn=3: Domain access control
            3 => {
                self.mmu.dacr = value;
                RegWriteResult::Ok
            }

            // CRn=5: Fault status registers
            5 => self.write_fault_status(encoding, value),

            // CRn=6: Fault address registers (read-only)
            6 => RegWriteResult::ReadOnly,

            // CRn=7: Cache operations (mostly write-only)
            7 => self.write_cache_op(encoding, value),

            // CRn=9: Performance monitor and cache lockdown
            9 => self.write_perf_reg(encoding, value),

            // CRn=10: Memory attributes
            10 => self.write_attr_reg(encoding, value),

            // CRn=12: VBAR
            12 => {
                self.tls.vbar = value;
                RegWriteResult::Ok
            }

            // CRn=13: TLS registers
            13 => self.write_tls_reg(encoding, value),

            // CRn=15: Implementation-defined registers
            15 => self.write_impl_reg(encoding, value),

            _ => RegWriteResult::Unimplemented,
        }
    }

    /// Read ID register (CRn=0)
    fn read_id_reg(&self, enc: Cp15Encoding) -> RegReadResult {
        match enc.opc1 {
            0 => match enc.crn {
                0 => match enc.crm {
                    0 => match enc.opc2 {
                        0 => RegReadResult::Ok { data: self.id.midr },
                        1 => RegReadResult::Ok { data: self.id.ctrr },
                        2 => RegReadResult::Ok { data: self.id.tcmtr },
                        3 => RegReadResult::Ok { data: self.id.tlbidr },
                        6 => RegReadResult::Ok { data: self.id.revidr },
                        _ => RegReadResult::Unimplemented,
                    },
                    1 => match enc.opc2 {
                        0 => RegReadResult::Ok { data: self.id.pfr0 },
                        1 => RegReadResult::Ok { data: self.id.pfr1 },
                        2 => RegReadResult::Ok { data: self.id.dfr0 },
                        3 => RegReadResult::Ok { data: self.id.afr0 },
                        4 => RegReadResult::Ok { data: self.id.mmfr0 },
                        5 => RegReadResult::Ok { data: self.id.mmfr1 },
                        6 => RegReadResult::Ok { data: self.id.mmfr2 },
                        7 => RegReadResult::Ok { data: self.id.mmfr3 },
                        _ => RegReadResult::Unimplemented,
                    },
                    2 => match enc.opc2 {
                        0 => RegReadResult::Ok { data: self.id.isar0 },
                        1 => RegReadResult::Ok { data: self.id.isar1 },
                        2 => RegReadResult::Ok { data: self.id.isar2 },
                        3 => RegReadResult::Ok { data: self.id.isar3 },
                        4 => RegReadResult::Ok { data: self.id.isar4 },
                        5 => RegReadResult::Ok { data: self.id.isar5 },
                        _ => RegReadResult::Unimplemented,
                    },
                    _ => RegReadResult::Unimplemented,
                },
                _ => RegReadResult::Unimplemented,
            },
            1 => match (enc.crm, enc.opc2) {
                (0, 1) => RegReadResult::Ok { data: self.id.clid },
                (0, 0) => RegReadResult::Ok { data: self.id.cssel },
                _ => RegReadResult::Unimplemented,
            },
            _ => RegReadResult::Unimplemented,
        }
    }

    /// Read control register (CRn=1)
    fn read_ctrl_reg(&self, enc: Cp15Encoding) -> RegReadResult {
        match enc.opc1 {
            0 => match enc.opc2 {
                0 => RegReadResult::Ok { data: self.ctrl.sctlr },
                1 => RegReadResult::Ok { data: self.ctrl.actlr },
                2 => RegReadResult::Ok { data: self.ctrl.cpacr },
                _ => RegReadResult::Unimplemented,
            },
            _ => RegReadResult::Unimplemented,
        }
    }

    /// Write control register (CRn=1)
    fn write_ctrl_reg(&mut self, enc: Cp15Encoding, value: u32) -> RegWriteResult {
        match enc.opc1 {
            0 => match enc.opc2 {
                0 => {
                    self.ctrl.sctlr = value;
                    RegWriteResult::Ok
                }
                1 => {
                    // ACTLR - implementation defined
                    self.ctrl.actlr = value;
                    RegWriteResult::Ok
                }
                2 => {
                    self.ctrl.cpacr = value;
                    RegWriteResult::Ok
                }
                _ => RegWriteResult::Unimplemented,
            },
            _ => RegWriteResult::Unimplemented,
        }
    }

    /// Read TTB register (CRn=2)
    fn read_ttb_reg(&self, enc: Cp15Encoding) -> RegReadResult {
        match enc.opc1 {
            0 => match enc.opc2 {
                0 => match enc.crm {
                    0 => RegReadResult::Ok { data: (self.mmu.ttbr0 & 0xFFFFFFFF) as u32 },
                    1 => RegReadResult::Ok { data: ((self.mmu.ttbr0 >> 32) & 0xF) as u32 },
                    2 => RegReadResult::Ok { data: self.mmu.ttbcr },
                    _ => RegReadResult::Unimplemented,
                },
                1 => match enc.crm {
                    0 => RegReadResult::Ok { data: (self.mmu.ttbr1 & 0xFFFFFFFF) as u32 },
                    1 => RegReadResult::Ok { data: ((self.mmu.ttbr1 >> 32) & 0xF) as u32 },
                    _ => RegReadResult::Unimplemented,
                },
                _ => RegReadResult::Unimplemented,
            },
            _ => RegReadResult::Unimplemented,
        }
    }

    /// Write TTB register (CRn=2)
    fn write_ttb_reg(&mut self, enc: Cp15Encoding, value: u32) -> RegWriteResult {
        match enc.opc1 {
            0 => match enc.opc2 {
                0 => match enc.crm {
                    0 => {
                        self.mmu.ttbr0 = (self.mmu.ttbr0 & 0xF00000000) | (value as u64 & 0xFFFFFFFF);
                        RegWriteResult::Ok
                    }
                    1 => {
                        self.mmu.ttbr0 = (self.mmu.ttbr0 & 0x0FFFFFFF) | ((value as u64 & 0xF) << 32);
                        RegWriteResult::Ok
                    }
                    2 => {
                        self.mmu.ttbcr = value;
                        RegWriteResult::Ok
                    }
                    _ => RegWriteResult::Unimplemented,
                },
                1 => match enc.crm {
                    0 => {
                        self.mmu.ttbr1 = (self.mmu.ttbr1 & 0xF00000000) | (value as u64 & 0xFFFFFFFF);
                        RegWriteResult::Ok
                    }
                    1 => {
                        self.mmu.ttbr1 = (self.mmu.ttbr1 & 0x0FFFFFFF) | ((value as u64 & 0xF) << 32);
                        RegWriteResult::Ok
                    }
                    _ => RegWriteResult::Unimplemented,
                },
                _ => RegWriteResult::Unimplemented,
            },
            _ => RegWriteResult::Unimplemented,
        }
    }

    /// Read fault status register (CRn=5)
    fn read_fault_status(&self, enc: Cp15Encoding) -> RegReadResult {
        match enc.opc1 {
            0 => match enc.opc2 {
                0 => RegReadResult::Ok { data: self.fault.dfsr },
                1 => RegReadResult::Ok { data: self.fault.ifsr },
                _ => RegReadResult::Unimplemented,
            },
            1 => match enc.opc2 {
                0 => RegReadResult::Ok { data: self.fault.adfsr },
                1 => RegReadResult::Ok { data: self.fault.aifsr },
                _ => RegReadResult::Unimplemented,
            },
            _ => RegReadResult::Unimplemented,
        }
    }

    /// Write fault status register (CRn=5)
    fn write_fault_status(&mut self, enc: Cp15Encoding, value: u32) -> RegWriteResult {
        match enc.opc1 {
            0 => match enc.opc2 {
                0 => {
                    self.fault.dfsr = value;
                    RegWriteResult::Ok
                }
                1 => {
                    self.fault.ifsr = value;
                    RegWriteResult::Ok
                }
                _ => RegWriteResult::Unimplemented,
            },
            1 => match enc.opc2 {
                0 => {
                    self.fault.adfsr = value;
                    RegWriteResult::Ok
                }
                1 => {
                    self.fault.aifsr = value;
                    RegWriteResult::Ok
                }
                _ => RegWriteResult::Unimplemented,
            },
            _ => RegWriteResult::Unimplemented,
        }
    }

    /// Read fault address register (CRn=6)
    fn read_fault_addr(&self, enc: Cp15Encoding) -> RegReadResult {
        match enc.opc1 {
            0 => match enc.opc2 {
                0 => RegReadResult::Ok { data: self.fault.dfar },
                1 => RegReadResult::Ok { data: self.fault.ifar },
                2 => RegReadResult::Ok { data: self.fault.dfar },
                _ => RegReadResult::Unimplemented,
            },
            _ => RegReadResult::Unimplemented,
        }
    }

    /// Read translation register (CRn=7)
    fn read_translate_reg(&self, enc: Cp15Encoding) -> RegReadResult {
        match enc.opc1 {
            0 => match enc.crm {
                4 => match enc.opc2 {
                    0 => RegReadResult::Ok { data: self.translate.par },
                    _ => RegReadResult::Unimplemented,
                },
                _ => RegReadResult::Unimplemented,
            },
            _ => RegReadResult::Unimplemented,
        }
    }

    /// Write cache operation (CRn=7)
    fn write_cache_op(&mut self, enc: Cp15Encoding, _value: u32) -> RegWriteResult {
        // Most cache operations are handled by hardware
        // We just track them for consistency
        match enc.crm {
            // DCCISW - Clean and invalidate data cache by set/way
            6 | 14 => RegWriteResult::Ok,
            // DCCSW - Clean data cache by set/way
            10 => RegWriteResult::Ok,
            // Other cache operations
            _ => RegWriteResult::Ok,
        }
    }

    /// Read performance monitor register (CRn=9)
    fn read_perf_reg(&self, enc: Cp15Encoding) -> RegReadResult {
        match enc.opc1 {
            0 => match enc.crm {
                12 => match enc.opc2 {
                    0 => RegReadResult::Ok { data: self.perf.pmcr },
                    1 => RegReadResult::Ok { data: self.perf.pmcnten },
                    2 => RegReadResult::Ok { data: self.perf.pmcnten },
                    3 => RegReadResult::Ok { data: self.perf.pmovsr },
                    5 => RegReadResult::Ok { data: self.perf.pmxevtyper },
                    6 => RegReadResult::Ok { data: self.perf.pmxevtyper },
                    _ => RegReadResult::Unimplemented,
                },
                13 => match enc.opc2 {
                    0 => RegReadResult::Ok { data: 0 }, // Cycle counter - not implemented
                    1 => RegReadResult::Ok { data: self.perf.pmxevtyper },
                    _ => RegReadResult::Unimplemented,
                },
                14 => match enc.opc2 {
                    0 => RegReadResult::Ok { data: self.perf.pmuserenr },
                    1 => RegReadResult::Ok { data: self.perf.pminten },
                    2 => RegReadResult::Ok { data: self.perf.pminten },
                    3 => RegReadResult::Ok { data: self.perf.pmxevtyper },
                    _ => RegReadResult::Unimplemented,
                },
                0 => match enc.opc2 {
                    0 => RegReadResult::Ok { data: self.perf.data_lock },
                    1 => RegReadResult::Ok { data: self.perf.insn_lock },
                    _ => RegReadResult::Unimplemented,
                },
                _ => RegReadResult::Unimplemented,
            },
            _ => RegReadResult::Unimplemented,
        }
    }

    /// Write performance monitor register (CRn=9)
    fn write_perf_reg(&mut self, enc: Cp15Encoding, value: u32) -> RegWriteResult {
        match enc.opc1 {
            0 => match enc.crm {
                12 => match enc.opc2 {
                    0 => {
                        // PMCR - only DP, X, D, E bits writable
                        self.perf.pmcr = (self.perf.pmcr & !0x39) | (value & 0x39);
                        RegWriteResult::Ok
                    }
                    1 => {
                        self.perf.pmcnten |= value & 0x80000000;
                        RegWriteResult::Ok
                    }
                    2 => {
                        self.perf.pmcnten &= !(value & 0x80000000);
                        RegWriteResult::Ok
                    }
                    3 => {
                        self.perf.pmovsr &= !value;
                        RegWriteResult::Ok
                    }
                    5 => {
                        self.perf.pmxevtyper = value & 0xFF;
                        RegWriteResult::Ok
                    }
                    _ => RegWriteResult::Unimplemented,
                },
                13 => match enc.opc2 {
                    0 => RegWriteResult::Ok, // Cycle counter - RAZ/WI
                    1 => {
                        self.perf.pmxevtyper = value & 0xFF;
                        RegWriteResult::Ok
                    }
                    _ => RegWriteResult::Unimplemented,
                },
                14 => match enc.opc2 {
                    0 => {
                        self.perf.pmuserenr = value & 1;
                        RegWriteResult::Ok
                    }
                    1 => {
                        self.perf.pminten |= value & 0x80000000;
                        RegWriteResult::Ok
                    }
                    2 => {
                        self.perf.pminten &= !(value & 0x80000000);
                        RegWriteResult::Ok
                    }
                    _ => RegWriteResult::Unimplemented,
                },
                0 => match enc.opc2 {
                    0 => {
                        self.perf.data_lock = value;
                        RegWriteResult::Ok
                    }
                    1 => {
                        self.perf.insn_lock = value;
                        RegWriteResult::Ok
                    }
                    _ => RegWriteResult::Unimplemented,
                },
                _ => RegWriteResult::Unimplemented,
            },
            _ => RegWriteResult::Unimplemented,
        }
    }

    /// Read attribute register (CRn=10)
    fn read_attr_reg(&self, enc: Cp15Encoding) -> RegReadResult {
        match enc.opc1 {
            0 => match enc.opc2 {
                0 => RegReadResult::Ok { data: self.attr.prrr },
                2 => RegReadResult::Ok { data: self.attr.nmrr },
                _ => RegReadResult::Unimplemented,
            },
            _ => RegReadResult::Unimplemented,
        }
    }

    /// Write attribute register (CRn=10)
    fn write_attr_reg(&mut self, enc: Cp15Encoding, value: u32) -> RegWriteResult {
        match enc.opc1 {
            0 => match enc.opc2 {
                0 => {
                    self.attr.prrr = value;
                    RegWriteResult::Ok
                }
                2 => {
                    self.attr.nmrr = value;
                    RegWriteResult::Ok
                }
                _ => RegWriteResult::Unimplemented,
            },
            _ => RegWriteResult::Unimplemented,
        }
    }

    /// Read TLS register (CRn=13)
    fn read_tls_reg(&self, enc: Cp15Encoding) -> RegReadResult {
        match enc.opc1 {
            0 => match enc.crm {
                0 => match enc.opc2 {
                    2 => RegReadResult::Ok { data: self.tls.fcseidr },
                    3 => RegReadResult::Ok { data: self.tls.contextidr },
                    4 => RegReadResult::Ok { data: self.tls.tpidrurw },
                    _ => RegReadResult::Unimplemented,
                },
                3 => match enc.opc2 {
                    2 => RegReadResult::Ok { data: self.tls.tpidruro },
                    3 => RegReadResult::Ok { data: self.tls.tpidrprw },
                    _ => RegReadResult::Unimplemented,
                },
                _ => RegReadResult::Unimplemented,
            },
            _ => RegReadResult::Unimplemented,
        }
    }

    /// Write TLS register (CRn=13)
    fn write_tls_reg(&mut self, enc: Cp15Encoding, value: u32) -> RegWriteResult {
        match enc.opc1 {
            0 => match enc.crm {
                0 => match enc.opc2 {
                    2 => {
                        self.tls.fcseidr = value;
                        RegWriteResult::Ok
                    }
                    3 => {
                        self.tls.contextidr = value;
                        RegWriteResult::Ok
                    }
                    4 => {
                        self.tls.tpidrurw = value;
                        RegWriteResult::Ok
                    }
                    _ => RegWriteResult::Unimplemented,
                },
                3 => match enc.opc2 {
                    2 => {
                        self.tls.tpidruro = value;
                        RegWriteResult::Ok
                    }
                    3 => {
                        self.tls.tpidrprw = value;
                        RegWriteResult::Ok
                    }
                    _ => RegWriteResult::Unimplemented,
                },
                _ => RegReadResult::Unimplemented,
            },
            _ => RegWriteResult::Unimplemented,
        }
    }

    /// Read implementation-defined register (CRn=15)
    fn read_impl_reg(&self, enc: Cp15Encoding) -> RegReadResult {
        match enc.opc1 {
            0 => RegReadResult::Ok { data: 0 }, // PCR - Power Control Register
            4 => {
                // CBAR - Configuration Base Address Register
                // Return different values based on CPU type
                let cbar = if (self.id.midr & 0xFF0FFFF0) == (ArmCpuId::CortexA9 as u32) {
                    0x1e000000
                } else {
                    0x2c000000 // Cortex-A7/A15
                };
                RegReadResult::Ok { data: cbar }
            }
            _ => RegReadResult::Unimplemented,
        }
    }

    /// Write implementation-defined register (CRn=15)
    fn write_impl_reg(&mut self, _enc: Cp15Encoding, _value: u32) -> RegWriteResult {
        // Most implementation-defined registers are write-ignore
        RegWriteResult::Ok
    }
}

/// Extension trait to add missing fields to Cp15IdRegs
trait Cp15IdRegsExt {
    fn ctrr(&self) -> u32;
    fn tcmtr(&self) -> u32;
    fn tlbidr(&self) -> u32;
    fn revidr(&self) -> u32;
    fn actlr(&self) -> u32;
}

impl Cp15IdRegsExt for Cp15IdRegs {
    fn ctrr(&self) -> u32 {
        self.cachetype
    }

    fn tcmtr(&self) -> u32 {
        0x00000000 // No TCM
    }

    fn tlbidr(&self) -> u32 {
        0x00000000
    }

    fn revidr(&self) -> u32 {
        0x00000000
    }
}

/// Extension trait to add ACTLR to control registers
trait Cp15CtrlRegsExt {
    fn actlr(&self) -> u32;
}

impl Cp15CtrlRegsExt for Cp15CtrlRegs {
    fn actlr(&self) -> u32 {
        0x00000000
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cp15_create_default() {
        let regs = Cp15Regs::default();
        assert_eq!(regs.ctrl.sctlr, 0x00C50078);
    }

    #[test]
    fn test_cp15_for_cortex_a9() {
        let regs = Cp15Regs::for_cpu(ArmCpuId::CortexA9, 0);
        assert_eq!(regs.id.midr & 0xFF0FFFF0, ArmCpuId::CortexA9 as u32 & 0xFF0FFFF0);
        assert_eq!(regs.id.mpidr, 0x80000000);
    }

    #[test]
    fn test_cp15_read_sctlr() {
        let regs = Cp15Regs::default();
        let enc = Cp15Encoding::new(0, 0, 1, 0);
        match regs.read(enc) {
            RegReadResult::Ok { data } => assert_eq!(data, regs.ctrl.sctlr),
            _ => panic!("Expected Ok result"),
        }
    }

    #[test]
    fn test_cp15_write_sctlr() {
        let mut regs = Cp15Regs::default();
        let enc = Cp15Encoding::new(0, 0, 1, 0);
        assert!(matches!(regs.write(enc, 0x12345678), RegWriteResult::Ok));
        assert_eq!(regs.ctrl.sctlr, 0x12345678);
    }

    #[test]
    fn test_cp15_read_ttbr0() {
        let regs = Cp15Regs::default();
        let enc = Cp15Encoding::new(0, 0, 2, 0);
        match regs.read(enc) {
            RegReadResult::Ok { data } => assert_eq!(data, regs.mmu.ttbr0 as u32),
            _ => panic!("Expected Ok result"),
        }
    }

    #[test]
    fn test_arm_cpu_id_from_u32() {
        let id = ArmCpuId::from_u32(0x410FC090);
        assert_eq!(id, ArmCpuId::CortexA9);
    }
}
