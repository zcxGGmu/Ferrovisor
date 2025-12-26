//! CPU feature detection for ARM64
//!
//! Provides runtime detection of ARM64 CPU features and capabilities.

use bitflags::bitflags;

/// ARM CPU implementer/manufacturer IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CpuImplementer {
    /// ARM Limited
    Arm = 0x41,
    /// Broadcom Corporation
    Broadcom = 0x42,
    /// Cavium Inc.
    Cavium = 0x43,
    /// Digital Equipment Corporation
    Dec = 0x44,
    /// Fujitsu Ltd.
    Fujitsu = 0x46,
    /// Infineon Technologies AG
    Infineon = 0x49,
    /// Freescale Semiconductor Inc.
    Freescale = 0x4D,
    /// NVIDIA Corporation
    Nvidia = 0x4E,
    /// APM Semiconductor
    APM = 0x50,
    /// Qualcomm Inc.
    Qualcomm = 0x51,
    /// Marvell International Ltd.
    Marvell = 0x56,
    /// Intel Corporation
    Intel = 0x69,
    /// Unknown implementer
    Unknown = 0x00,
}

impl CpuImplementer {
    /// Create from implementer byte in MIDR_EL1
    pub fn from_midr(midr: u32) -> Self {
        let implementer = (midr >> 24) & 0xFF;
        match implementer {
            0x41 => CpuImplementer::Arm,
            0x42 => CpuImplementer::Broadcom,
            0x43 => CpuImplementer::Cavium,
            0x44 => CpuImplementer::Dec,
            0x46 => CpuImplementer::Fujitsu,
            0x49 => CpuImplementer::Infineon,
            0x4D => CpuImplementer::Freescale,
            0x4E => CpuImplementer::Nvidia,
            0x50 => CpuImplementer::APM,
            0x51 => CpuImplementer::Qualcomm,
            0x56 => CpuImplementer::Marvell,
            0x69 => CpuImplementer::Intel,
            _ => CpuImplementer::Unknown,
        }
    }

    /// Get implementer name as string
    pub fn as_str(&self) -> &'static str {
        match self {
            CpuImplementer::Arm => "ARM",
            CpuImplementer::Broadcom => "Broadcom",
            CpuImplementer::Cavium => "Cavium",
            CpuImplementer::Dec => "DEC",
            CpuImplementer::Fujitsu => "Fujitsu",
            CpuImplementer::Infineon => "Infineon",
            CpuImplementer::Freescale => "Freescale",
            CpuImplementer::Nvidia => "NVIDIA",
            CpuImplementer::APM => "APM",
            CpuImplementer::Qualcomm => "Qualcomm",
            CpuImplementer::Marvell => "Marvell",
            CpuImplementer::Intel => "Intel",
            CpuImplementer::Unknown => "Unknown",
        }
    }
}

/// ARM CPU part number (specific CPU model)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CpuPart {
    /// Cortex-A35
    CortexA35 = 0xD04,
    /// Cortex-A53
    CortexA53 = 0xD03,
    /// Cortex-A55
    CortexA55 = 0xD05,
    /// Cortex-A57
    CortexA57 = 0xD07,
    /// Cortex-A72
    CortexA72 = 0xD08,
    /// Cortex-A73
    CortexA73 = 0xD09,
    /// Cortex-A75
    CortexA75 = 0xD0A,
    /// Cortex-A76
    CortexA76 = 0xD0B,
    /// Cortex-A77
    CortexA77 = 0xD0D,
    /// Cortex-A78
    CortexA78 = 0xD41,
    /// Cortex-A510
    CortexA510 = 0xD46,
    /// Cortex-A710
    CortexA710 = 0xD47,
    /// Neoverse N1
    NeoverseN1 = 0xD0C,
    /// Neoverse N2
    NeoverseN2 = 0xD49,
    /// Unknown part
    Unknown = 0x000,
}

impl CpuPart {
    /// Create from part number in MIDR_EL1
    pub fn from_midr(midr: u32) -> Self {
        let part = midr & 0xFFF;
        match part {
            0xD04 => CpuPart::CortexA35,
            0xD03 => CpuPart::CortexA53,
            0xD05 => CpuPart::CortexA55,
            0xD07 => CpuPart::CortexA57,
            0xD08 => CpuPart::CortexA72,
            0xD09 => CpuPart::CortexA73,
            0xD0A => CpuPart::CortexA75,
            0xD0B => CpuPart::CortexA76,
            0xD0D => CpuPart::CortexA77,
            0xD41 => CpuPart::CortexA78,
            0xD46 => CpuPart::CortexA510,
            0xD47 => CpuPart::CortexA710,
            0xD0C => CpuPart::NeoverseN1,
            0xD49 => CpuPart::NeoverseN2,
            _ => CpuPart::Unknown,
        }
    }

    /// Get part name as string
    pub fn as_str(&self) -> &'static str {
        match self {
            CpuPart::CortexA35 => "Cortex-A35",
            CpuPart::CortexA53 => "Cortex-A53",
            CpuPart::CortexA55 => "Cortex-A55",
            CpuPart::CortexA57 => "Cortex-A57",
            CpuPart::CortexA72 => "Cortex-A72",
            CpuPart::CortexA73 => "Cortex-A73",
            CpuPart::CortexA75 => "Cortex-A75",
            CpuPart::CortexA76 => "Cortex-A76",
            CpuPart::CortexA77 => "Cortex-A77",
            CpuPart::CortexA78 => "Cortex-A78",
            CpuPart::CortexA510 => "Cortex-A510",
            CpuPart::CortexA710 => "Cortex-A710",
            CpuPart::NeoverseN1 => "Neoverse N1",
            CpuPart::NeoverseN2 => "Neoverse N2",
            CpuPart::Unknown => "Unknown",
        }
    }
}

/// CPU feature flags
bitflags::bitflags! {
    /// ARM64 CPU features
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CpuFeatures: u64 {
        /// FP (Floating-point) supported
        const FP = 1 << 0;
        /// ASIMD (Advanced SIMD) supported
        const ASIMD = 1 << 1;
        /// EL3 (Secure monitor) implemented
        const EL3 = 1 << 2;
        /// 4KB granule supported
        const GRAN4K = 1 << 3;
        /// 64KB granule supported
        const GRAN64K = 1 << 4;
        /// 16KB granule supported
        const GRAN16K = 1 << 5;
        /// Mixed-endian support
        const MIXED_ENDIAN = 1 << 6;
        /// 32-bit EL0 supported
        const AARCH32 = 1 << 7;
        /// GIC system register interface
        const GIC_SYSREG = 1 << 8;
        /// Advanced SIMD in 32-bit EL0
        const ASIMD_32 = 1 << 9;
        /// SVE (Scalable Vector Extension)
        const SVE = 1 << 10;
        /// EL2 implemented
        const EL2 = 1 << 11;
        /// PA (Physical Address) size
        const PA_BITS = 0b11 << 12;
        /// VA (Virtual Address) size
        const VA_BITS = 0b11 << 14;
        /// PMU (Performance Monitors) v3
        const PMU_V3 = 1 << 16;
        /// Virtualization Host Extensions
        const VHE = 1 << 17;
        /// TME (Transactional Memory Extension)
        const TME = 1 << 18;
        /// RAS (Reliability, Availability, Serviceability)
        const RAS = 1 << 19;
        /// SVE2 (Scalable Vector Extension 2)
        const SVE2 = 1 << 20;
        /// Pointer Authentication
        const PAUTH = 1 << 21;
        /// Memory Tagging Extension
        const MTE = 1 << 22;
        /// Activity Monitors
        const AMU = 1 << 23;
        /// SME (Scalable Matrix Extension)
        const SME = 1 << 24;
        /// EnhancedPAC
        const EPAC = 1 << 25;
        /// Fault handling precise timing
        const FPAC = 1 << 26;
        /// Enhanced virtualization traps
        const E0PD = 1 << 27;
        /// BTI (Branch Target Identification)
        const BTI = 1 << 28;
        /// Constant-time key for PAC
        const CONSTPAC = 1 << 29;
    }
}

/// CPU information
pub struct CpuInfo {
    /// MIDR_EL1 value
    pub midr: u32,
    /// MPIDR_EL1 value
    pub mpidr: u64,
    /// CPU implementer
    pub implementer: CpuImplementer,
    /// CPU part number
    pub part: CpuPart,
    /// CPU variant
    pub variant: u8,
    /// CPU revision
    pub revision: u8,
    /// CPU features
    pub features: CpuFeatures,
    /// Cache line size
    pub cache_line_size: u32,
}

impl CpuInfo {
    /// Get CPU ID string
    pub fn id_string(&self) -> String {
        format!(
            "{} {} r{}p{}",
            self.implementer.as_str(),
            self.part.as_str(),
            self.variant,
            self.revision
        )
    }
}

/// Global CPU information (initialized at boot)
static mut CPU_INFO: Option<CpuInfo> = None;

/// Get CPU information
pub fn cpu_info() -> &'static CpuInfo {
    unsafe { CPU_INFO.as_ref().expect("CPU info not initialized") }
}

/// Get CPU ID string
pub fn cpu_id_string() -> String {
    cpu_info().id_string()
}

/// Check if CPU has a specific feature
pub fn has_feature(feature: CpuFeatures) -> bool {
    cpu_info().features.contains(feature)
}

/// Check if EL2 is implemented
pub fn has_el2() -> bool {
    has_feature(CpuFeatures::EL2)
}

/// Check if EL3 is implemented
pub fn has_el3() -> bool {
    has_feature(CpuFeatures::EL3)
}

/// Check if VHE is available
pub fn has_vhe() -> bool {
    has_feature(CpuFeatures::VHE)
}

/// Check if SVE is available
pub fn has_sve() -> bool {
    has_feature(CpuFeatures::SVE)
}

/// Check if pointer authentication is available
pub fn has_pauth() -> bool {
    has_feature(CpuFeatures::PAUTH)
}

/// Detect CPU features
pub fn detect() {
    use super::regs::info;

    let midr = unsafe { info::read_midr_el1() } as u32;
    let mpidr = unsafe { info::read_mpidr_el1() };

    let implementer = CpuImplementer::from_midr(midr);
    let part = CpuPart::from_midr(midr);
    let variant = ((midr >> 20) & 0xF) as u8;
    let revision = (midr & 0xF) as u8;

    let mut features = CpuFeatures::empty();

    // Detect features from ID_AA64PFR0_EL1
    let pfr0 = unsafe { info::read_id_aa64pfr0_el1() };

    // EL0, EL1, EL2, EL3 fields (4 bits each)
    let el0 = (pfr0 >> 0) & 0xF;
    let el1 = (pfr0 >> 4) & 0xF;
    let el2 = (pfr0 >> 8) & 0xF;
    let el3 = (pfr0 >> 12) & 0xF;

    if el2 != 0xF {
        features |= CpuFeatures::EL2;
    }
    if el3 != 0xF {
        features |= CpuFeatures::EL3;
    }

    // FP and ASIMD fields
    let fp = (pfr0 >> 16) & 0xF;
    let asimd = (pfr0 >> 20) & 0xF;
    if fp != 0xF {
        features |= CpuFeatures::FP;
    }
    if asimd != 0xF {
        features |= CpuFeatures::ASIMD;
    }

    // AArch32 EL0
    let aarch32 = (pfr0 >> 28) & 0xF;
    if aarch32 != 0xF {
        features |= CpuFeatures::AARCH32;
    }

    // Detect more features from ID_AA64MMFR0_EL1
    let mmfr0 = unsafe { info::read_id_aa64mmfr0_el1() };

    // PARange (Physical Address Size)
    let pa_range = (mmfr0 >> 0) & 0xF;
    let pa_bits = match pa_range {
        0b0000 => 32,
        0b0001 => 36,
        0b0010 => 40,
        0b0011 => 42,
        0b0100 => 44,
        0b0101 => 48,
        0b0110 => 52,
        _ => 48, // Default to 48-bit
    };
    features |= CpuFeatures::from_bits_truncate(((pa_bits - 32) as u64) << 12);

    // TGran4 (4KB granule support)
    let tgran4 = (mmfr0 >> 28) & 0xF;
    if tgran4 != 0xF {
        features |= CpuFeatures::GRAN4K;
    }

    // TGran16 (16KB granule support)
    let tgran16 = (mmfr0 >> 20) & 0xF;
    if tgran16 != 0xF {
        features |= CpuFeatures::GRAN16K;
    }

    // TGran64 (64KB granule support)
    let tgran64 = (mmfr0 >> 24) & 0xF;
    if tgran64 != 0xF {
        features |= CpuFeatures::GRAN64K;
    }

    // Detect more features from ID_AA64ISAR0_EL1
    let isar0 = unsafe { info::read_id_aa64isar0_el1() };

    // SVE
    let sve = (isar0 >> 0) & 0xF;
    if sve != 0 {
        features |= CpuFeatures::SVE;
    }

    // PAUTH
    let pauth = (isar0 >> 4) & 0xF;
    if pauth != 0 {
        features |= CpuFeatures::PAUTH;
    }

    // Detect more features from ID_AA64DFR0_EL1
    let dfr0 = unsafe { info::read_id_aa64dfr0_el1() };

    // PMUVer
    let pmuver = (dfr0 >> 8) & 0xF;
    if pmuver != 0 {
        features |= CpuFeatures::PMU_V3;
    }

    let info = CpuInfo {
        midr,
        mpidr,
        implementer,
        part,
        variant,
        revision,
        features,
        cache_line_size: 64, // Default to 64 bytes for ARM64
    };

    unsafe {
        CPU_INFO = Some(info);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_implementer() {
        assert_eq!(CpuImplementer::Arm as u32, 0x41);
        assert_eq!(CpuImplementer::Arm.as_str(), "ARM");
    }

    #[test]
    fn test_cpu_part() {
        assert_eq!(CpuPart::CortexA53 as u32, 0xD03);
        assert_eq!(CpuPart::CortexA53.as_str(), "Cortex-A53");
    }

    #[test]
    fn test_cpu_features() {
        let features = CpuFeatures::FP | CpuFeatures::ASIMD | CpuFeatures::EL2;
        assert!(features.contains(CpuFeatures::FP));
        assert!(features.contains(CpuFeatures::ASIMD));
        assert!(features.contains(CpuFeatures::EL2));
    }
}
