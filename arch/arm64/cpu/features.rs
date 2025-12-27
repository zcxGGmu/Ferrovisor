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
        /// Virtualization Host Extensions (ARMv8.1)
        const VHE = 1 << 17;
        /// TME (Transactional Memory Extension)
        const TME = 1 << 18;
        /// RAS (Reliability, Availability, Serviceability)
        const RAS = 1 << 19;
        /// SVE2 (Scalable Vector Extension 2)
        const SVE2 = 1 << 20;
        /// Pointer Authentication (ARMv8.3)
        const PAUTH = 1 << 21;
        /// Memory Tagging Extension (ARMv8.5)
        const MTE = 1 << 22;
        /// Activity Monitors (ARMv8.4)
        const AMU = 1 << 23;
        /// SME (Scalable Matrix Extension)
        const SME = 1 << 24;
        /// EnhancedPAC
        const EPAC = 1 << 25;
        /// Fault handling precise timing
        const FPAC = 1 << 26;
        /// Enhanced virtualization traps (ARMv8.5)
        const E0PD = 1 << 27;
        /// BTI (Branch Target Identification, ARMv8.5)
        const BTI = 1 << 28;
        /// Constant-time key for PAC
        const CONSTPAC = 1 << 29;
        /// PAN (Privileged Access Never, ARMv8.1)
        const PAN = 1 << 30;
        /// UAO (User Access Override, ARMv8.2)
        const UAO = 1 << 31;
        /// SVB (SVE/FPR16 supports)
        const SVB = 1 << 32;
        /// CSV2 (Cache Speculation Variant 2)
        const CSV2 = 1 << 33;
        /// CSV3 (Cache Speculation Variant 3)
        const CSV3 = 1 << 34;
        /// DGH (Data Gathering Hint)
        const DGH = 1 << 35;
        /// ST (Full write in ST*W)
        const ST = 1 << 36;
        /// GTG (Guest Translation Granule)
        const GTG = 1 << 37;
        /// ECV (Enhanced Counter Virtualization)
        const ECV = 1 << 38;
        /// TTL (TLB Instruction Invalidate)
        const TTL = 1 << 39;
        /// LSB (Address Authenticates)
        const LSB = 1 << 40;
        /// AFP (Advanced Floating-point)
        const AFP = 1 << 41;
        /// DIT (Data Independent Timing)
        const DIT = 1 << 42;
        /// SPECRES (Speculation Restriction)
        const SPECRES = 1 << 43;
    }
}

/// ARMv8 architecture version
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArchVersion {
    /// ARMv8.0 - Base architecture
    Armv8_0 = 0x80,
    /// ARMv8.1 - VHE, PAN, PMU extensions
    Armv8_1 = 0x81,
    /// ARMv8.2 - UAO, SVE, PTM, DCPODP extensions
    Armv8_2 = 0x82,
    /// ARMv8.3 - PAUTH, SVE2, Nested Virtualization
    Armv8_3 = 0x83,
    /// ARMv8.4 - AMU, MPAM, SVE2, BTI extensions
    Armv8_4 = 0x84,
    /// ARMv8.5 - MTE, FR, Extentions
    Armv8_5 = 0x85,
    /// ARMv8.6 - BF16, I8MM extensions
    Armv8_6 = 0x86,
    /// ARMv8.7 - WFXT, HAFDBS extensions
    Armv8_7 = 0x87,
    /// ARMv8.8 - Overwrite, Permission overlay extensions
    Armv8_8 = 0x88,
    /// ARMv8.9 - GCS, TIDCP1 extensions
    Armv8_9 = 0x89,
    /// ARMv9.0 - SVE2, improved VArch, Pointer Auth
    Armv9_0 = 0x90,
    /// ARMv9.1 - Enhanced MTE, PAC
    Armv9_1 = 0x91,
    /// ARMv9.2 - Transparent HUK, RME
    Armv9_2 = 0x92,
    /// ARMv9.3 - LSE128, CX1
    Armv9_3 = 0x93,
    /// ARMv9.4 - GCS, THE
    Armv9_4 = 0x94,
    /// Unknown version
    Unknown = 0x00,
}

impl ArchVersion {
    /// Get architecture version as string
    pub fn as_str(&self) -> &'static str {
        match self {
            ArchVersion::Armv8_0 => "ARMv8.0",
            ArchVersion::Armv8_1 => "ARMv8.1",
            ArchVersion::Armv8_2 => "ARMv8.2",
            ArchVersion::Armv8_3 => "ARMv8.3",
            ArchVersion::Armv8_4 => "ARMv8.4",
            ArchVersion::Armv8_5 => "ARMv8.5",
            ArchVersion::Armv8_6 => "ARMv8.6",
            ArchVersion::Armv8_7 => "ARMv8.7",
            ArchVersion::Armv8_8 => "ARMv8.8",
            ArchVersion::Armv8_9 => "ARMv8.9",
            ArchVersion::Armv9_0 => "ARMv9.0",
            ArchVersion::Armv9_1 => "ARMv9.1",
            ArchVersion::Armv9_2 => "ARMv9.2",
            ArchVersion::Armv9_3 => "ARMv9.3",
            ArchVersion::Armv9_4 => "ARMv9.4",
            ArchVersion::Unknown => "Unknown",
        }
    }

    /// Check if ARMv9 or later
    pub fn is_armv9(&self) -> bool {
        matches!(self,
            ArchVersion::Armv9_0 | ArchVersion::Armv9_1 |
            ArchVersion::Armv9_2 | ArchVersion::Armv9_3 | ArchVersion::Armv9_4
        )
    }
}

/// SVE (Scalable Vector Extension) information
#[derive(Debug, Clone, Copy)]
pub struct SveInfo {
    /// SVE version (0 = not supported, 1 = SVE, 2 = SVE2)
    pub version: u8,
    /// Vector length in bits (128-2048, multiple of 128)
    pub vl: u16,
    /// Maximum supported vector length
    pub max_vl: u16,
}

impl Default for SveInfo {
    fn default() -> Self {
        Self {
            version: 0,
            vl: 0,
            max_vl: 0,
        }
    }
}

/// Pointer Authentication information
#[derive(Debug, Clone, Copy)]
pub struct PauthInfo {
    /// APIA (Instruction authentication A) supported
    pub apia: bool,
    /// APIB (Instruction authentication B) supported
    pub apib: bool,
    /// APDA (Data authentication A) supported
    pub apda: bool,
    /// APDB (Data authentication B) supported
    pub apdb: bool,
    /// APGA (Generic authentication) supported
    pub apga: bool,
    /// Enhanced PAC (EPAC) supported
    pub epac: bool,
    /// PAC 2.0 (PAC_QARMA5) supported
    pub pac2_0: bool,
    /// PAC combined (impDef) supported
    pub pac_combined: bool,
}

impl Default for PauthInfo {
    fn default() -> Self {
        Self {
            apia: false,
            apib: false,
            apda: false,
            apdb: false,
            apga: false,
            epac: false,
            pac2_0: false,
            pac_combined: false,
        }
    }
}

/// Virtualization features
#[derive(Debug, Clone, Copy)]
pub struct VirtualizationFeatures {
    /// VHE (Virtualization Host Extension) supported
    pub vhe: bool,
    /// Stage-2 Page table walk dirty tracking
    pub st2_dirty: bool,
    /// Hardware update of dirty flag
    pub hw_dirty: bool,
    /// Hardware AF (Access Flag) update
    pub hw_af: bool,
    /// TTL (TLB Instruction/VMID Invalidate) supported
    pub ttl: bool,
    /// VMID16 (16-bit VMID) supported
    pub vmid16: bool,
    /// BBML (Block-based Break-Before-Make) supported
    pub bbml: bool,
    /// ECV (Enhanced Counter Virtualization) supported
    pub ecv: bool,
    /// FGT (Fine-Grained Traps) supported
    pub fgt: bool,
    /// FGT2 (Extended FGT) supported
    pub fgt2: bool,
    /// HAFDBS (Hardware Access Flag Dirty Bit State) supported
    pub hafdbs: bool,
}

impl Default for VirtualizationFeatures {
    fn default() -> Self {
        Self {
            vhe: false,
            st2_dirty: false,
            hw_dirty: false,
            hw_af: false,
            ttl: false,
            vmid16: false,
            bbml: false,
            ecv: false,
            fgt: false,
            fgt2: false,
            hafdbs: false,
        }
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
    /// Architecture version
    pub arch_version: ArchVersion,
    /// SVE information
    pub sve_info: SveInfo,
    /// Pointer authentication information
    pub pauth_info: PauthInfo,
    /// Virtualization features
    pub virt_features: VirtualizationFeatures,
    /// Physical address size in bits
    pub pa_bits: u8,
    /// Virtual address size in bits
    pub va_bits: u8,
}

impl CpuInfo {
    /// Get CPU ID string
    pub fn id_string(&self) -> String {
        format!(
            "{} {} r{}p{} ({})",
            self.implementer.as_str(),
            self.part.as_str(),
            self.variant,
            self.revision,
            self.arch_version.as_str()
        )
    }

    /// Get PA size in bits
    pub fn pa_size_bits(&self) -> u8 {
        self.pa_bits
    }

    /// Get VA size in bits
    pub fn va_size_bits(&self) -> u8 {
        self.va_bits
    }

    /// Check if VHE is available
    pub fn has_vhe(&self) -> bool {
        self.virt_features.vhe
    }

    /// Get SVE vector length
    pub fn sve_vl(&self) -> u16 {
        self.sve_info.vl
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
    let mut arch_version = ArchVersion::Armv8_0;
    let mut sve_info = SveInfo::default();
    let mut pauth_info = PauthInfo::default();
    let mut virt_features = VirtualizationFeatures::default();
    let mut pa_bits: u8 = 48;
    let mut va_bits: u8 = 48;

    // ============================================================
    // Detect features from ID_AA64PFR0_EL1 and ID_AA64PFR1_EL1
    // ============================================================
    let pfr0 = unsafe { info::read_id_aa64pfr0_el1() };
    let pfr1 = unsafe { info::read_id_aa64pfr1_el1() };

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

    // Check from PFR1 for BTI, PMU, etc.
    let bt = (pfr1 >> 4) & 0xF; // Branch Target Identification
    if bt != 0 {
        features |= CpuFeatures::BTI;
        if bt >= 2 {
            arch_version = arch_version.max(ArchVersion::Armv8_5);
        }
    }

    let ssbs = (pfr1 >> 8) & 0xF; // Speculation Store Bypass Safe
    if ssbs != 0 {
        features |= CpuFeatures::SPECRES;
        if ssbs >= 2 {
            arch_version = arch_version.max(ArchVersion::Armv8_5);
        }
    }

    // ============================================================
    // Detect features from ID_AA64MMFR0_EL1, MMFR1, MMFR2
    // ============================================================
    let mmfr0 = unsafe { info::read_id_aa64mmfr0_el1() };
    let mmfr1 = unsafe { info::read_id_aa64mmfr1_el1() };
    let mmfr2 = unsafe { info::read_id_aa64mmfr2_el1() };

    // PARange (Physical Address Size)
    let pa_range = (mmfr0 >> 0) & 0xF;
    pa_bits = match pa_range {
        0b0000 => 32,
        0b0001 => 36,
        0b0010 => 40,
        0b0011 => 42,
        0b0100 => 44,
        0b0101 => 48,
        0b0110 => 52,
        _ => 48,
    };

    // VARange (Virtual Address Size)
    let va_range = (mmfr0 >> 4) & 0xF;
    va_bits = match va_range {
        0b0000 => 32,
        0b0001 => 36,
        0b0010 => 40,
        0b0011 => 42,
        0b0100 => 44,
        0b0101 => 48,
        0b0110 => 52,
        _ => 48,
    };

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

    // Check for ECV (Enhanced Counter Virtualization) - MMFR2
    let ecv = (mmfr2 >> 0) & 0xF;
    if ecv != 0 {
        virt_features.ecv = true;
        features |= CpuFeatures::ECV;
        if ecv >= 2 {
            arch_version = arch_version.max(ArchVersion::Armv8_6);
        }
    }

    // Check for GTG (Guest Translation Granule)
    let gtg = (mmfr2 >> 28) & 0xF;
    if gtg != 0 {
        virt_features.vmid16 = true;
        features |= CpuFeatures::GTG;
        arch_version = arch_version.max(ArchVersion::Armv8_6);
    }

    // ============================================================
    // Detect features from ID_AA64ISAR0_EL1 and ISAR1, ISAR2
    // ============================================================
    let isar0 = unsafe { info::read_id_aa64isar0_el1() };
    let isar1 = unsafe { info::read_id_aa64isar1_el1() };
    let isar2 = unsafe { info::read_id_aa64isar2_el1() };

    // SVE
    let sve = (isar0 >> 0) & 0xF;
    if sve != 0 {
        features |= CpuFeatures::SVE;
        sve_info.version = 1;
        arch_version = arch_version.max(ArchVersion::Armv8_2);

        // Read SVE features from ZFR0
        let zfr0 = unsafe { info::read_id_aa64zfr0_el1() };
        sve_info.vl = 128 << ((zfr0 >> 0) & 0xF); // ZLen field
        sve_info.max_vl = 128 << ((zfr0 >> 4) & 0xF); // ZLen_max field

        // SVE2 from ISAR1
        let sve2 = (isar1 >> 0) & 0xF;
        if sve2 != 0 {
            sve_info.version = 2;
            features |= CpuFeatures::SVE2;
            arch_version = arch_version.max(ArchVersion::Armv8_3);
        }
    }

    // PAUTH
    let pauth = (isar0 >> 4) & 0xF;
    if pauth != 0 {
        features |= CpuFeatures::PAUTH;
        pauth_info.apia = true;
        pauth_info.apda = true;
        arch_version = arch_version.max(ArchVersion::Armv8_3);

        // Check for enhanced PAC features from ISAR1/ISAR2
        let apda = (isar1 >> 4) & 0xF;
        let apib = (isar1 >> 8) & 0xF;
        let apdb = (isar1 >> 12) & 0xF;
        let apga = (isar1 >> 16) & 0xF;

        pauth_info.apib = apib != 0;
        pauth_info.apdb = apdb != 0;
        pauth_info.apga = apga != 0;

        // Check for EPAC (Enhanced PAC)
        let epac = (isar1 >> 20) & 0xF;
        if epac != 0 {
            pauth_info.epac = true;
            features |= CpuFeatures::EPAC;
            arch_version = arch_version.max(ArchVersion::Armv8_3);
        }

        // Check for PAC 2.0 from ISAR2
        let pac_frac = (isar2 >> 0) & 0xF;
        if pac_frac >= 2 {
            pauth_info.pac2_0 = true;
            features |= CpuFeatures::CONSTPAC;
            arch_version = arch_version.max(ArchVersion::Armv8_3);
        }
    }

    // Check for DPB (Data Barrier) - ARMv8.1
    let dpb = (isar0 >> 8) & 0xF;
    if dpb != 0 {
        arch_version = arch_version.max(ArchVersion::Armv8_1);
    }

    // Check for CRC32
    let crc32 = (isar0 >> 12) & 0xF;
    if crc32 != 0 {
        arch_version = arch_version.max(ArchVersion::Armv8_1);
    }

    // Check for LSE (Large System Extension) - ARMv8.1
    let lse = (isar0 >> 16) & 0xF;
    if lse != 0 {
        arch_version = arch_version.max(ArchVersion::Armv8_1);
    }

    // Check for LSE2 - ARMv8.5
    let lse2 = (isar1 >> 28) & 0xF;
    if lse2 != 0 {
        arch_version = arch_version.max(ArchVersion::Armv8_5);
    }

    // Check for FP16 - ARMv8.2
    let fp16 = (isar0 >> 20) & 0xF;
    if fp16 != 0 {
        arch_version = arch_version.max(ArchVersion::Armv8_2);
    }

    // Check for RDM (Read Multiple) - ARMv8.1
    let rdm = (isar0 >> 24) & 0xF;
    if rdm != 0 {
        arch_version = arch_version.max(ArchVersion::Armv8_1);
    }

    // Check for SVE-BIT (SVE/FPR16 supports) - ARMv8.5
    let svebit = (isar1 >> 24) & 0xF;
    if svebit != 0 {
        features |= CpuFeatures::SVB;
        arch_version = arch_version.max(ArchVersion::Armv8_5);
    }

    // Check for MTE (Memory Tagging Extension) - ARMv8.5
    let mte = (isar1 >> 20) & 0xF;
    if mte != 0 {
        features |= CpuFeatures::MTE;
        arch_version = arch_version.max(ArchVersion::Armv8_5);
    }

    // Check for SME (Scalable Matrix Extension) - ARMv9
    let sme = (isar1 >> 12) & 0xF;
    if sme != 0 {
        features |= CpuFeatures::SME;
        arch_version = arch_version.max(ArchVersion::Armv9_0);
    }

    // ============================================================
    // Detect features from ID_AA64DFR0_EL1 and DFR1
    // ============================================================
    let dfr0 = unsafe { info::read_id_aa64dfr0_el1() };
    let dfr1 = unsafe { info::read_id_aa64dfr1_el1() };

    // PMUVer
    let pmuver = (dfr0 >> 8) & 0xF;
    if pmuver != 0 {
        features |= CpuFeatures::PMU_V3;
    }

    // Check for HPMN0 (Hierarchical PMU) - ARMv8.4
    let hpmn0 = (dfr0 >> 12) & 0xF;
    if hpmn0 != 0 {
        arch_version = arch_version.max(ArchVersion::Armv8_4);
    }

    // Check for MTPMU and PMUv3 from DFR1
    let debug_ver = (dfr1 >> 0) & 0xF;
    if debug_ver >= 9 {
        arch_version = arch_version.max(ArchVersion::Armv8_4);
    }

    // ============================================================
    // Detect virtualization features
    // ============================================================

    // Check for VHE from ID_AA64MMFR1_EL1
    let vh = (mmfr1 >> 28) & 0xF;
    if vh != 0 {
        virt_features.vhe = true;
        features |= CpuFeatures::VHE;
        arch_version = arch_version.max(ArchVersion::Armv8_1);
    }

    // Check for VMID16 from MMFR1
    let vmid_bits = (mmfr1 >> 4) & 0xF;
    if vmid_bits == 2 {
        virt_features.vmid16 = true;
        arch_version = arch_version.max(ArchVersion::Armv8_5);
    }

    // Check for Hardware AF (Access Flag) update
    let hafs = (mmfr1 >> 8) & 0xF;
    if hafs != 0 {
        virt_features.hw_af = true;
        arch_version = arch_version.max(ArchVersion::Armv8_2);
    }

    // Check for Hardware dirty flag update
    let hafdbs = (mmfr1 >> 12) & 0xF;
    if hafdbs != 0 {
        virt_features.hw_dirty = true;
        virt_features.hafdbs = true;
        features |= CpuFeatures::RAS;
        arch_version = arch_version.max(ArchVersion::Armv8_4);
    }

    // Check for XNX (Execute-never) - ARMv8.2
    let xnx = (mmfr1 >> 16) & 0xF;
    if xnx != 0 {
        arch_version = arch_version.max(ArchVersion::Armv8_2);
    }

    // ============================================================
    // Detect more features from ID_AA64FR0_EL1 (Floating-point)
    // ============================================================
    let fr0 = unsafe { info::read_id_aa64fr0_el1() };

    // Check for CSV2, CSV3 - Speculation variant
    let csv2 = (fr0 >> 20) & 0xF;
    if csv2 != 0 {
        features |= CpuFeatures::CSV2;
        if csv2 >= 2 {
            features |= CpuFeatures::CSV3;
            arch_version = arch_version.max(ArchVersion::Armv8_5);
        }
    }

    // Check for DIT (Data Independent Timing) - ARMv8.4
    let dit = (fr0 >> 24) & 0xF;
    if dit != 0 {
        features |= CpuFeatures::DIT;
        arch_version = arch_version.max(ArchVersion::Armv8_4);
    }

    // ============================================================
    // Determine architecture version from detected features
    // ============================================================
    // Base version is 8.0, update based on detected features above
    if features.contains(CpuFeatures::VHE) ||
       features.contains(CpuFeatures::PAN) {
        arch_version = arch_version.max(ArchVersion::Armv8_1);
    }

    // ARMv9 detection (from FR0 or SVE/SVE2)
    if arch_version >= ArchVersion::Armv9_0 ||
       features.contains(CpuFeatures::SVE2) {
        arch_version = ArchVersion::Armv9_0;
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
        arch_version,
        sve_info,
        pauth_info,
        virt_features,
        pa_bits,
        va_bits,
    };

    unsafe {
        CPU_INFO = Some(info);
    }

    log::info!("CPU: {}", info.id_string());
    log::info!("  PA bits: {}, VA bits: {}", pa_bits, va_bits);
    log::info!("  Features: {:?}", info.features);
    if info.sve_info.version > 0 {
        log::info!("  SVE: v{}, VL={}", info.sve_info.version, info.sve_info.vl);
    }
    if info.pauth_info.apia {
        log::info!("  PAUTH: APA");
    }
    if info.virt_features.vhe {
        log::info!("  Virtualization: VHE");
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
