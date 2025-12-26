//! ID Register Emulation for ARM64
//!
//! Provides ID register emulation for VCPU system register virtualization.
//! Reference: ARM DDI 0487I.a - Chapter D14 - ID Registers
//! Reference: xvisor/arch/arm/cpu/arm64/include/cpu_defines.h

use crate::arch::arm64::cpu::sysreg::{SysRegEncoding, RegReadResult, RegWriteResult};

/// ID_AA64PFR0_EL1 - Processor Feature Register 0
///
/// Describes the processor's execution state and supported instruction sets.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IdAa64Pfr0El1 {
    pub raw: u64,
}

impl IdAa64Pfr0El1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// EL0 is AArch64 only
    pub const fn el0_aarch64_only() -> Self {
        Self::new(0x1)
    }

    /// EL0 is AArch32 only
    pub const fn el0_aarch32_only() -> Self {
        Self::new(0x2)
    }

    /// EL0 supports both AArch64 and AArch32
    pub const fn el0_both() -> Self {
        Self::new(0x3)
    }

    /// Get EL0 value
    pub fn el0(&self) -> u64 {
        (self.raw >> 0) & 0xF
    }

    /// Get EL1 value
    pub fn el1(&self) -> u64 {
        (self.raw >> 4) & 0xF
    }

    /// Get EL2 value
    pub fn el2(&self) -> u64 {
        (self.raw >> 8) & 0xF
    }

    /// Get EL3 value
    pub fn el3(&self) -> u64 {
        (self.raw >> 12) & 0xF
    }

    /// Get FP value
    pub fn fp(&self) -> u64 {
        (self.raw >> 16) & 0xF
    }

    /// Get AdvSIMD value
    pub fn asimd(&self) -> u64 {
        (self.raw >> 20) & 0xF
    }

    /// Get GIC value
    pub fn gic(&self) -> u64 {
        (self.raw >> 24) & 0xF
    }

    /// Get RAS value
    pub fn ras(&self) -> u64 {
        (self.raw >> 28) & 0xF
    }

    /// Get SVE value
    pub fn sve(&self) -> u64 {
        (self.raw >> 32) & 0xF
    }

    /// Get SEL2 value
    pub fn sel2(&self) -> u64 {
        (self.raw >> 36) & 0xF
    }

    /// Get MPAM value
    pub fn mpam(&self) -> u64 {
        (self.raw >> 40) & 0xF
    }

    /// Get AMU value
    pub fn amu(&self) -> u64 {
        (self.raw >> 44) & 0xF
    }

    /// Create default PFR0 value for typical ARM64 CPU
    pub fn default() -> Self {
        // EL0=1 (AArch64), EL1=1 (AArch64), EL2=1 (AArch64), EL3=0 (not implemented)
        // FP=0x0 (implemented), ASIMD=0x0 (implemented)
        Self::new(0x00000011)
    }
}

/// ID_AA64PFR1_EL1 - Processor Feature Register 1
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IdAa64Pfr1El1 {
    pub raw: u64,
}

impl IdAa64Pfr1El1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get BTI value
    pub fn bti(&self) -> u64 {
        (self.raw >> 0) & 0xF
    }

    /// Get PAC value
    pub fn pac(&self) -> u64 {
        (self.raw >> 4) & 0xF
    }

    /// Create default PFR1 value
    pub fn default() -> Self {
        Self::new(0x00000000)
    }
}

/// ID_AA64DFR0_EL1 - Debug Feature Register 0
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IdAa64Dfr0El1 {
    pub raw: u64,
}

impl IdAa64Dfr0El1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get Debug architecture version
    pub fn debug_ver(&self) -> u64 {
        (self.raw >> 0) & 0xF
    }

    /// Get Trace architecture version
    pub fn trace_ver(&self) -> u64 {
        (self.raw >> 4) & 0xF
    }

    /// Get Performance Monitors architecture version
    pub fn pmu_ver(&self) -> u64 {
        (self.raw >> 8) & 0xF
    }

    /// Get Breakpoint fields
    pub fn brps(&self) -> u64 {
        (self.raw >> 12) & 0xF
    }

    /// Get Watchpoint fields
    pub fn wrps(&self) -> u64 {
        (self.raw >> 20) & 0xF
    }

    /// Create default DFR0 value
    pub fn default() -> Self {
        // DebugVer=0x6 (v8), PMUVer=0x1 (PMUv3)
        Self::new(0x00000006)
    }
}

/// ID_AA64DFR1_EL1 - Debug Feature Register 1
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IdAa64Dfr1El1 {
    pub raw: u64,
}

impl IdAa64Dfr1El1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Create default DFR1 value
    pub fn default() -> Self {
        Self::new(0x00000000)
    }
}

/// ID_AA64ISAR0_EL1 - Instruction Set Attribute Register 0
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IdAa64Isar0El1 {
    pub raw: u64,
}

impl IdAa64Isar0El1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get AES value
    pub fn aes(&self) -> u64 {
        (self.raw >> 4) & 0xF
    }

    /// Get SHA1 value
    pub fn sha1(&self) -> u64 {
        (self.raw >> 8) & 0xF
    }

    /// Get SHA2 value
    pub fn sha2(&self) -> u64 {
        (self.raw >> 12) & 0xF
    }

    /// Get CRC32 value
    pub fn crc32(&self) -> u64 {
        (self.raw >> 16) & 0xF
    }

    /// Get Atomic instructions value
    pub fn atomic(&self) -> u64 {
        (self.raw >> 20) & 0xF
    }

    /// Get TME value
    pub fn tme(&self) -> u64 {
        (self.raw >> 24) & 0xF
    }

    /// Get RDM value
    pub fn rdm(&self) -> u64 {
        (self.raw >> 28) & 0xF
    }

    /// Get SHA3 value
    pub fn sha3(&self) -> u64 {
        (self.raw >> 32) & 0xF
    }

    /// Get SM3 value
    pub fn sm3(&self) -> u64 {
        (self.raw >> 36) & 0xF
    }

    /// Get SM4 value
    pub fn sm4(&self) -> u64 {
        (self.raw >> 40) & 0xF
    }

    /// Get Dot Product value
    pub fn dp(&self) -> u64 {
        (self.raw >> 44) & 0xF
    }

    /// Get FHM value
    pub fn fhm(&self) -> u64 {
        (self.raw >> 48) & 0xF
    }

    /// Get TS value
    pub fn ts(&self) -> u64 {
        (self.raw >> 52) & 0xF
    }

    /// Get RNDR value
    pub fn rndr(&self) -> u64 {
        (self.raw >> 60) & 0xF
    }

    /// Create default ISAR0 value with common crypto extensions
    pub fn default() -> Self {
        // AES=0x1, SHA1=0x1, SHA2=0x1, CRC32=0x1, Atomic=0x2
        Self::new(0x00001112)
    }
}

/// ID_AA64ISAR1_EL1 - Instruction Set Attribute Register 1
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IdAa64Isar1El1 {
    pub raw: u64,
}

impl IdAa64Isar1El1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get DPB value
    pub fn dpb(&self) -> u64 {
        (self.raw >> 0) & 0xF
    }

    /// Get APA value
    pub fn apa(&self) -> u64 {
        (self.raw >> 4) & 0xF
    }

    /// Get API value
    pub fn api(&self) -> u64 {
        (self.raw >> 8) & 0xF
    }

    /// Get JSCVT value
    pub fn jscvt(&self) -> u64 {
        (self.raw >> 12) & 0xF
    }

    /// Get FCMA value
    pub fn fcma(&self) -> u64 {
        (self.raw >> 16) & 0xF
    }

    /// Get LRCPC value
    pub fn lrcpc(&self) -> u64 {
        (self.raw >> 20) & 0xF
    }

    /// Get GPA value
    pub fn gpa(&self) -> u64 {
        (self.raw >> 24) & 0xF
    }

    /// Get GPI value
    pub fn gpi(&self) -> u64 {
        (self.raw >> 28) & 0xF
    }

    /// Get FRINTTS value
    pub fn frintts(&self) -> u64 {
        (self.raw >> 32) & 0xF
    }

    /// Get SB value
    pub fn sb(&self) -> u64 {
        (self.raw >> 36) & 0xF
    }

    /// Get SPECRES value
    pub fn specres(&self) -> u64 {
        (self.raw >> 40) & 0xF
    }

    /// Get BF16 value
    pub fn bf16(&self) -> u64 {
        (self.raw >> 44) & 0xF
    }

    /// Get DGH value
    pub fn dgh(&self) -> u64 {
        (self.raw >> 48) & 0xF
    }

    /// Get I8MM value
    pub fn i8mm(&self) -> u64 {
        (self.raw >> 52) & 0xF
    }

    /// Create default ISAR1 value
    pub fn default() -> Self {
        Self::new(0x00000000)
    }
}

/// ID_AA64ISAR2_EL1 - Instruction Set Attribute Register 2
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IdAa64Isar2El1 {
    pub raw: u64,
}

impl IdAa64Isar2El1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Create default ISAR2 value
    pub fn default() -> Self {
        Self::new(0x00000000)
    }
}

/// ID_AA64MMFR0_EL1 - Memory Model Feature Register 0
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IdAa64Mmfr0El1 {
    pub raw: u64,
}

impl IdAa64Mmfr0El1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get TGran4 value (4KB granule support)
    pub fn tgran4(&self) -> u64 {
        (self.raw >> 28) & 0xF
    }

    /// Get TGran64 value (64KB granule support)
    pub fn tgran64(&self) -> u64 {
        (self.raw >> 24) & 0xF
    }

    /// Get TGran16 value (16KB granule support)
    pub fn tgran16(&self) -> u64 {
        (self.raw >> 20) & 0xF
    }

    /// Get PARange value (physical address size)
    pub fn parange(&self) -> u64 {
        (self.raw >> 0) & 0xF
    }

    /// Get BigEndEL0 value
    pub fn bigend_el0(&self) -> u64 {
        (self.raw >> 16) & 0xF
    }

    /// Get SMemBarrier value
    pub fn smem_barrier(&self) -> u64 {
        (self.raw >> 36) & 0xF
    }

    /// Get CheckSum value
    pub fn checksum(&self) -> u64 {
        (self.raw >> 40) & 0xF
    }

    /// Create default MMFR0 value
    pub fn default() -> Self {
        // TGran4=0x0 (4KB supported), TGran64=0x0 (64KB supported)
        // PARange=0x6 (44 bits)
        Self::new(0x00000010)
    }
}

/// ID_AA64MMFR1_EL1 - Memory Model Feature Register 1
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IdAa64Mmfr1El1 {
    pub raw: u64,
}

impl IdAa64Mmfr1El1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get HAFDBS value
    pub fn hafdbs(&self) -> u64 {
        (self.raw >> 0) & 0xF
    }

    /// Get VMIDBits value
    pub fn vmid_bits(&self) -> u64 {
        (self.raw >> 4) & 0xF
    }

    /// Get VH value
    pub fn vh(&self) -> u64 {
        (self.raw >> 8) & 0xF
    }

    /// Get PAN value
    pub fn pan(&self) -> u64 {
        (self.raw >> 16) & 0xF
    }

    /// Get LO value
    pub fn lo(&self) -> u64 {
        (self.raw >> 20) & 0xF
    }

    /// Get HPDS value
    pub fn hpds(&self) -> u64 {
        (self.raw >> 24) & 0xF
    }

    /// Get VMID16 value
    pub fn vmid16(&self) -> u64 {
        (self.raw >> 28) & 0xF
    }

    /// Get TTL value
    pub fn ttl(&self) -> u64 {
        (self.raw >> 32) & 0xF
    }

    /// Get FWAT value
    pub fn fwat(&self) -> u64 {
        (self.raw >> 36) & 0xF
    }

    /// Create default MMFR1 value
    pub fn default() -> Self {
        // VMIDBits=0x2 (16 bits), PAN=0x1 (PAN supported)
        Self::new(0x00000021)
    }
}

/// ID_AA64MMFR2_EL1 - Memory Model Feature Register 2
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IdAa64Mmfr2El1 {
    pub raw: u64,
}

impl IdAa64Mmfr2El1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get CNP value
    pub fn cnp(&self) -> u64 {
        (self.raw >> 0) & 0xF
    }

    /// Get AT value
    pub fn at(&self) -> u64 {
        (self.raw >> 4) & 0xF
    }

    /// Get ST value
    pub fn st(&self) -> u64 {
        (self.raw >> 8) & 0xF
    }

    /// Get VARange value
    pub fn varange(&self) -> u64 {
        (self.raw >> 12) & 0xF
    }

    /// Get IESB value
    pub fn iesb(&self) -> u64 {
        (self.raw >> 16) & 0xF
    }

    /// Get LSM value
    pub fn lsm(&self) -> u64 {
        (self.raw >> 20) & 0xF
    }

    /// Get UAO value
    pub fn uao(&self) -> u64 {
        (self.raw >> 24) & 0xF
    }

    /// Get WFXT value
    pub fn wfxt(&self) -> u64 {
        (self.raw >> 28) & 0xF
    }

    /// Create default MMFR2 value
    pub fn default() -> Self {
        // CNP=0x1 (CNP supported)
        Self::new(0x00000001)
    }
}

/// MIDR_EL1 - Main ID Register
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MidrEl1 {
    pub raw: u64,
}

impl MidrEl1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Create MIDR from fields
    pub fn from_fields(implementer: u8, variant: u8, architecture: u8,
                       part_num: u16, revision: u8) -> Self {
        let raw = ((implementer as u64) << 24) |
                  ((variant as u64) << 20) |
                  ((architecture as u64) << 16) |
                  ((part_num as u64) << 4) |
                  (revision as u64);
        Self { raw }
    }

    /// Get Implementer (e.g., ARM = 0x41)
    pub fn implementer(&self) -> u8 {
        ((self.raw >> 24) & 0xFF) as u8
    }

    /// Get Variant
    pub fn variant(&self) -> u8 {
        ((self.raw >> 20) & 0xF) as u8
    }

    /// Get Architecture
    pub fn architecture(&self) -> u8 {
        ((self.raw >> 16) & 0xF) as u8
    }

    /// Get Part Number
    pub fn part_number(&self) -> u16 {
        ((self.raw >> 4) & 0xFFF) as u16
    }

    /// Get Revision
    pub fn revision(&self) -> u8 {
        (self.raw & 0xF) as u8
    }

    /// ARM implementer ID
    pub const ARM_IMPLEMENTER: u8 = 0x41;

    /// Create default MIDR (generic ARM CPU)
    pub fn default() -> Self {
        // ARM implementer, Architecture=8 (AArch64)
        Self::from_fields(Self::ARM_IMPLEMENTER, 0, 8, 0xD00, 0)
    }
}

/// MPIDR_EL1 - Multiprocessor Affinity Register
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MpidrEl1 {
    pub raw: u64,
}

impl MpidrEl1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Get Affinity Level 0
    pub fn affinity0(&self) -> u64 {
        self.raw & 0xFF
    }

    /// Get Affinity Level 1
    pub fn affinity1(&self) -> u64 {
        (self.raw >> 8) & 0xFF
    }

    /// Get Affinity Level 2
    pub fn affinity2(&self) -> u64 {
        (self.raw >> 16) & 0xFF
    }

    /// Get Affinity Level 3
    pub fn affinity3(&self) -> u64 {
        (self.raw >> 32) & 0xFF
    }

    /// Get U bit (MT bit)
    pub fn is_multi_threaded(&self) -> bool {
        (self.raw & (1 << 30)) != 0
    }

    /// Get U bit
    pub fn is_uniprocessor(&self) -> bool {
        (self.raw & (1 << 31)) != 0
    }

    /// Create MPIDR for specific CPU
    pub fn for_cpu(cluster: u8, cpu: u8) -> Self {
        Self::new((cluster as u64) << 8 | (cpu as u64))
    }
}

/// REVIDR_EL1 - Revision ID Register
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct RevidrEl1 {
    pub raw: u64,
}

impl RevidrEl1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Create default REVIDR
    pub fn default() -> Self {
        Self::new(0x00000000)
    }
}

/// ID register state for a VCPU
#[derive(Debug, Clone)]
pub struct IdRegisters {
    /// ID_AA64PFR0_EL1
    pub pfr0: IdAa64Pfr0El1,
    /// ID_AA64PFR1_EL1
    pub pfr1: IdAa64Pfr1El1,
    /// ID_AA64DFR0_EL1
    pub dfr0: IdAa64Dfr0El1,
    /// ID_AA64DFR1_EL1
    pub dfr1: IdAa64Dfr1El1,
    /// ID_AA64ISAR0_EL1
    pub isar0: IdAa64Isar0El1,
    /// ID_AA64ISAR1_EL1
    pub isar1: IdAa64Isar1El1,
    /// ID_AA64ISAR2_EL1
    pub isar2: IdAa64Isar2El1,
    /// ID_AA64MMFR0_EL1
    pub mmfr0: IdAa64Mmfr0El1,
    /// ID_AA64MMFR1_EL1
    pub mmfr1: IdAa64Mmfr1El1,
    /// ID_AA64MMFR2_EL1
    pub mmfr2: IdAa64Mmfr2El1,
    /// MIDR_EL1
    pub midr: MidrEl1,
    /// MPIDR_EL1
    pub mpidr: MpidrEl1,
    /// REVIDR_EL1
    pub revidr: RevidrEl1,
}

impl Default for IdRegisters {
    fn default() -> Self {
        Self {
            pfr0: IdAa64Pfr0El1::default(),
            pfr1: IdAa64Pfr1El1::default(),
            dfr0: IdAa64Dfr0El1::default(),
            dfr1: IdAa64Dfr1El1::default(),
            isar0: IdAa64Isar0El1::default(),
            isar1: IdAa64Isar1El1::default(),
            isar2: IdAa64Isar2El1::default(),
            mmfr0: IdAa64Mmfr0El1::default(),
            mmfr1: IdAa64Mmfr1El1::default(),
            mmfr2: IdAa64Mmfr2El1::default(),
            midr: MidrEl1::default(),
            mpidr: MpidrEl1::for_cpu(0, 0),
            revidr: RevidrEl1::default(),
        }
    }
}

impl IdRegisters {
    /// Create new ID registers with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize from hardware ID registers
    pub fn from_hw() -> Self {
        let mut regs = Self::new();

        #[cfg(target_arch = "aarch64")]
        unsafe {
            // Read ID registers from hardware
            let mut pfr0: u64;
            let mut dfr0: u64;
            let mut isar0: u64;
            let mut mmfr0: u64;
            let mut midr: u64;
            let mut mpidr: u64;

            core::arch::asm!("mrs {}, id_aa64pfr0_el1", out(reg) pfr0);
            core::arch::asm!("mrs {}, id_aa64dfr0_el1", out(reg) dfr0);
            core::arch::asm!("mrs {}, id_aa64isar0_el1", out(reg) isar0);
            core::arch::asm!("mrs {}, id_aa64mmfr0_el1", out(reg) mmfr0);
            core::arch::asm!("mrs {}, midr_el1", out(reg) midr);
            core::arch::asm!("mrs {}, mpidr_el1", out(reg) mpidr);

            regs.pfr0 = IdAa64Pfr0El1::new(pfr0);
            regs.dfr0 = IdAa64Dfr0El1::new(dfr0);
            regs.isar0 = IdAa64Isar0El1::new(isar0);
            regs.mmfr0 = IdAa64Mmfr0El1::new(mmfr0);
            regs.midr = MidrEl1::new(midr);
            regs.mpidr = MpidrEl1::new(mpidr);
        }

        regs
    }

    /// Read an ID register by encoding
    pub fn read_id_reg(&self, encoding: SysRegEncoding) -> RegReadResult {
        // ID registers are encoded as: op0=3, op1=0, crn=0, crm=4-7, op2=0-7
        match (encoding.op0, encoding.op1, encoding.crn, encoding.crm, encoding.op2) {
            // ID_AA64PFR0_EL1: op0=3, op1=0, crn=0, crm=4, op2=0
            (3, 0, 0, 4, 0) => RegReadResult::Ok { data: self.pfr0.raw },
            // ID_AA64PFR1_EL1: op0=3, op1=0, crn=0, crm=4, op2=1
            (3, 0, 0, 4, 1) => RegReadResult::Ok { data: self.pfr1.raw },
            // ID_AA64DFR0_EL1: op0=3, op1=0, crn=0, crm=5, op2=0
            (3, 0, 0, 5, 0) => RegReadResult::Ok { data: self.dfr0.raw },
            // ID_AA64DFR1_EL1: op0=3, op1=0, crn=0, crm=5, op2=1
            (3, 0, 0, 5, 1) => RegReadResult::Ok { data: self.dfr1.raw },
            // ID_AA64ISAR0_EL1: op0=3, op1=0, crn=0, crm=6, op2=0
            (3, 0, 0, 6, 0) => RegReadResult::Ok { data: self.isar0.raw },
            // ID_AA64ISAR1_EL1: op0=3, op1=0, crn=0, crm=6, op2=1
            (3, 0, 0, 6, 1) => RegReadResult::Ok { data: self.isar1.raw },
            // ID_AA64MMFR0_EL1: op0=3, op1=0, crn=0, crm=7, op2=0
            (3, 0, 0, 7, 0) => RegReadResult::Ok { data: self.mmfr0.raw },
            // ID_AA64MMFR1_EL1: op0=3, op1=0, crn=0, crm=7, op2=1
            (3, 0, 0, 7, 1) => RegReadResult::Ok { data: self.mmfr1.raw },
            // MIDR_EL1: op0=3, op1=0, crn=0, crm=0, op2=0
            (3, 0, 0, 0, 0) => RegReadResult::Ok { data: self.midr.raw },
            // MPIDR_EL1: op0=3, op1=0, crn=0, crm=0, op2=5
            (3, 0, 0, 0, 5) => RegReadResult::Ok { data: self.mpidr.raw },
            // REVIDR_EL1: op0=3, op1=0, crn=0, crm=0, op2=6
            (3, 0, 0, 0, 6) => RegReadResult::Ok { data: self.revidr.raw },
            _ => RegReadResult::Unimplemented,
        }
    }

    /// Write to an ID register (usually ignored, but can be emulated)
    pub fn write_id_reg(&mut self, _encoding: SysRegEncoding, _value: u64) -> RegWriteResult {
        // ID registers are read-only, writes are ignored
        RegWriteResult::Ignored
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pfr0_creation() {
        let pfr0 = IdAa64Pfr0El1::default();
        assert_eq!(pfr0.el0(), 1); // AArch64
        assert_eq!(pfr0.el1(), 1); // AArch64
        assert_eq!(pfr0.el2(), 1); // AArch64
    }

    #[test]
    fn test_midr_creation() {
        let midr = MidrEl1::default();
        assert_eq!(midr.implementer(), MidrEl1::ARM_IMPLEMENTER);
        assert_eq!(midr.architecture(), 8); // AArch64
    }

    #[test]
    fn test_id_registers_read() {
        let regs = IdRegisters::new();
        let encoding = SysRegEncoding {
            op0: 3, op1: 0, crn: 0, crm: 4, op2: 0
        };

        match regs.read_id_reg(encoding) {
            RegReadResult::Ok { data } => assert_eq!(data, regs.pfr0.raw),
            _ => panic!("Expected Ok result"),
        }
    }

    #[test]
    fn test_id_registers_write_ignored() {
        let mut regs = IdRegisters::new();
        let encoding = SysRegEncoding {
            op0: 3, op1: 0, crn: 0, crm: 4, op2: 0
        };

        assert!(matches!(regs.write_id_reg(encoding, 0xDEADBEEF),
                         RegWriteResult::Ignored));
    }
}
