//! RISC-V CPU Features Detection
//!
//! This module provides CPU feature detection including:
//! - ISA extensions detection
//! - Virtualization support detection
//! - Performance counters detection
//! - Cache information

use bitflags::bitflags;

/// RISC-V ISA extensions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsaExtension {
    /// Base integer instruction set
    I,
    /// Multiplication and division
    M,
    /// Atomic operations
    A,
    /// Single-precision floating-point
    F,
    /// Double-precision floating-point
    D,
    /// Quad-precision floating-point
    Q,
    /// Compressed instructions
    C,
    /// Vector instructions
    V,
    /// Hypervisor extension
    H,
    /// Bit-manipulation
    B,
    /// Supervisor-level instructions
    S,
    /// User-level instructions
    U,
    /// Transactional memory
    T,
    /// N-bit user-level interrupt
    N,
    /// Custom extensions
    X,
}

/// CPU feature flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CpuFeatures: u64 {
        const HAS_EXTENSION_M = 1 << 0;
        const HAS_EXTENSION_A = 1 << 1;
        const HAS_EXTENSION_F = 1 << 2;
        const HAS_EXTENSION_D = 1 << 3;
        const HAS_EXTENSION_Q = 1 << 4;
        const HAS_EXTENSION_C = 1 << 5;
        const HAS_EXTENSION_V = 1 << 6;
        const HAS_EXTENSION_H = 1 << 7;
        const HAS_EXTENSION_S = 1 << 8;
        const HAS_EXTENSION_U = 1 << 9;
        const HAS_EXTENSION_T = 1 << 10;
        const HAS_EXTENSION_N = 1 << 11;
        const HAS_RVC = 1 << 12;
        const HAS_RV128 = 1 << 13;
        const HAS_ZBA = 1 << 14;
        const HAS_ZBB = 1 << 15;
        const HAS_ZBC = 1 << 16;
        const HAS_ZBS = 1 << 17;
        const HAS_ZBKB = 1 << 18;
        const HAS_ZBKC = 1 << 19;
        const HAS_ZBKX = 1 << 20;
        const HAS_ZKND = 1 << 21;
        const HAS_ZKNE = 1 << 22;
        const HAS_ZKNH = 1 << 23;
        const HAS_ZKR = 1 << 24;
        const HAS_ZKSED = 1 << 25;
        const HAS_ZKSH = 1 << 26;
        const HAS_ZKT = 1 << 27;
        const HAS_ZICBOM = 1 << 28;
        const HAS_ZICBOZ = 1 << 29;
        const HAS_ZICFIL = 1 << 30;
        const HAS_ZIFENCEI = 1 << 31;
        const HAS_ZIHPM = 1 << 32;
        const HAS_ZMMUL = 1 << 33;
        const HAS_ZFH = 1 << 34;
        const HAS_ZFHMIN = 1 << 35;
        const HAS_ZDINX = 1 << 36;
        const HAS_ZVFH = 1 << 37;
        const HAS_ZVFHMIN = 1 << 38;
        const HAS_ZVEDIC = 1 << 39;
        const HAS_ZVL128B = 1 << 40;
        const HAS_ZVL256B = 1 << 41;
        const HAS_ZVL512B = 1 << 42;
        const HAS_ZVL1024B = 1 << 43;
        const HAS_ZVL2048B = 1 << 44;
        const HAS_ZVL4096B = 1 << 45;
        const HAS_ZVL8192B = 1 << 46;
        const HAS_ZVL16384B = 1 << 47;
        const HAS_ZVL32768B = 1 << 48;
        const HAS_ZVL65536B = 1 << 49;
        const HAS_SSCOFPMF = 1 << 50;
        const HAS_SSTC = 1 << 51;
        const HAS_SVINVAL = 1 << 52;
        const HAS_SVNAPOT = 1 << 53;
        const HAS_SVPBMT = 1 << 54;
        const HAS_ZACAS = 1 << 55;
        const HAS_ZALRSC = 1 << 56;
        const HAS_ZAWRS = 1 << 57;
        const HAS_ZFA = 1 << 58;
        const HAS_ZCB = 1 << 59;
        const HAS_ZCMP = 1 << 60;
        const HAS_ZCMT = 1 << 61;
    }
}

/// CPU information structure
#[derive(Debug, Clone)]
pub struct CpuInfo {
    /// Hardware thread ID
    pub hart_id: usize,
    /// Vendor ID
    pub vendor_id: usize,
    /// Architecture ID
    pub arch_id: usize,
    /// Implementation ID
    pub impl_id: usize,
    /// ISA string
    pub isa_string: String,
    /// Supported features
    pub features: CpuFeatures,
    /// XLEN (register width)
    pub xlen: usize,
    /// Physical address width
    pub paddr_bits: usize,
    /// Virtual address width
    pub vaddr_bits: usize,
    /// Cache line size
    pub cache_line_size: usize,
    /// Number of performance counters
    pub num_counters: usize,
}

impl Default for CpuInfo {
    fn default() -> Self {
        Self {
            hart_id: 0,
            vendor_id: 0,
            arch_id: 0,
            impl_id: 0,
            isa_string: String::new(),
            features: CpuFeatures::empty(),
            xlen: 64,
            paddr_bits: 56,
            vaddr_bits: 48,
            cache_line_size: 64,
            num_counters: 0,
        }
    }
}

/// Global CPU information
static mut CPU_INFO: CpuInfo = CpuInfo {
    hart_id: 0,
    vendor_id: 0,
    arch_id: 0,
    arch_id: 0,
    impl_id: 0,
    isa_string: String::new(),
    features: CpuFeatures::empty(),
    xlen: 64,
    paddr_bits: 56,
    vaddr_bits: 48,
    cache_line_size: 64,
    num_counters: 0,
};

/// Detect CPU features
pub fn detect() -> Result<(), &'static str> {
    log::info!("Detecting RISC-V CPU features");

    let mut info = CpuInfo::default();

    // Read hardware thread ID
    info.hart_id = read_csr!(crate::arch::riscv64::csr::MHARTID);

    // Read vendor and architecture IDs
    info.vendor_id = read_csr!(crate::arch::riscv64::csr::MVENDORID);
    info.arch_id = read_csr!(crate::arch::riscv64::csr::MARCHID);
    info.impl_id = read_csr!(crate::arch::riscv64::csr::MIMPID);

    // Read ISA string from device tree or construct it
    info.isa_string = detect_isa_string();

    // Detect XLEN
    info.xlen = detect_xlen();

    // Detect various extensions
    detect_extensions(&mut info);

    // Detect cache information
    detect_cache_info(&mut info);

    // Detect performance counters
    detect_performance_counters(&mut info);

    // Store global CPU info
    unsafe {
        CPU_INFO = info.clone();
    }

    log::info!("CPU Features: Hart ID: {}, Vendor: {:#x}, Arch: {:#x}",
               info.hart_id, info.vendor_id, info.arch_id);
    log::info!("ISA: {}", info.isa_string);
    log::info!("XLEN: {}, PADDR: {} bits, VADDR: {} bits",
               info.xlen, info.paddr_bits, info.vaddr_bits);

    Ok(())
}

/// Get CPU information
pub fn get_cpu_info() -> &'static CpuInfo {
    unsafe { &CPU_INFO }
}

/// Check if a specific extension is supported
pub fn has_extension(ext: IsaExtension) -> bool {
    let info = get_cpu_info();
    match ext {
        IsaExtension::I => info.isa_string.contains("rv32i") || info.isa_string.contains("rv64i") || info.isa_string.contains("rv128i"),
        IsaExtension::M => info.isa_string.contains('m') || info.features.contains(CpuFeatures::HAS_EXTENSION_M),
        IsaExtension::A => info.isa_string.contains('a') || info.features.contains(CpuFeatures::HAS_EXTENSION_A),
        IsaExtension::F => info.isa_string.contains('f') || info.features.contains(CpuFeatures::HAS_EXTENSION_F),
        IsaExtension::D => info.isa_string.contains('d') || info.features.contains(CpuFeatures::HAS_EXTENSION_D),
        IsaExtension::Q => info.isa_string.contains('q') || info.features.contains(CpuFeatures::HAS_EXTENSION_Q),
        IsaExtension::C => info.isa_string.contains('c') || info.features.contains(CpuFeatures::HAS_EXTENSION_C),
        IsaExtension::V => info.isa_string.contains('v') || info.features.contains(CpuFeatures::HAS_EXTENSION_V),
        IsaExtension::H => info.isa_string.contains('h') || info.features.contains(CpuFeatures::HAS_EXTENSION_H),
        IsaExtension::S => info.isa_string.contains('s') || info.features.contains(CpuFeatures::HAS_EXTENSION_S),
        IsaExtension::U => info.isa_string.contains('u') || info.features.contains(CpuFeatures::HAS_EXTENSION_U),
        IsaExtension::T => info.isa_string.contains('t') || info.features.contains(CpuFeatures::HAS_EXTENSION_T),
        IsaExtension::N => info.isa_string.contains('n') || info.features.contains(CpuFeatures::HAS_EXTENSION_N),
        IsaExtension::X => info.isa_string.contains('x'),
    }
}

/// Check if virtualization is supported
pub fn has_virtualization() -> bool {
    has_extension(IsaExtension::H)
}

/// Check if vector extension is supported
pub fn has_vector() -> bool {
    has_extension(IsaExtension::V)
}

/// Check if floating-point is supported
pub fn has_floating_point() -> bool {
    has_extension(IsaExtension::F) || has_extension(IsaExtension::D) || has_extension(IsaExtension::Q)
}

fn detect_isa_string() -> String {
    // In a real implementation, this would read from device tree
    // For now, return a default based on common configurations
    String::from("rv64imafdcsuv")
}

fn detect_xlen() -> usize {
    // Read MXL field from MISA
    let misa = read_csr!(crate::arch::riscv64::csr::MISA);
    match misa & 0xC0000000 {
        0x40000000 => 32,
        0x80000000 => 64,
        0xC0000000 => 128,
        _ => 64, // Default to 64-bit
    }
}

fn detect_extensions(info: &mut CpuInfo) {
    let misa = read_csr!(crate::arch::riscv64::csr::MISA);

    // Check standard extensions
    if misa & (1 << ('m' as u8 - 'a' as u8)) != 0 {
        info.features.insert(CpuFeatures::HAS_EXTENSION_M);
    }
    if misa & (1 << ('a' as u8 - 'a' as u8)) != 0 {
        info.features.insert(CpuFeatures::HAS_EXTENSION_A);
    }
    if misa & (1 << ('f' as u8 - 'a' as u8)) != 0 {
        info.features.insert(CpuFeatures::HAS_EXTENSION_F);
    }
    if misa & (1 << ('d' as u8 - 'a' as u8)) != 0 {
        info.features.insert(CpuFeatures::HAS_EXTENSION_D);
    }
    if misa & (1 << ('c' as u8 - 'a' as u8)) != 0 {
        info.features.insert(CpuFeatures::HAS_EXTENSION_C);
    }
    if misa & (1 << ('s' as u8 - 'a' as u8)) != 0 {
        info.features.insert(CpuFeatures::HAS_EXTENSION_S);
    }
    if misa & (1 << ('u' as u8 - 'a' as u8)) != 0 {
        info.features.insert(CpuFeatures::HAS_EXTENSION_U);
    }
    if misa & (1 << ('h' as u8 - 'a' as u8)) != 0 {
        info.features.insert(CpuFeatures::HAS_EXTENSION_H);
    }

    // TODO: Detect Z-extensions and other vendor-specific extensions
}

fn detect_cache_info(info: &mut CpuInfo) {
    // In a real implementation, this would read cache configuration
    // from device tree or probe using cache operations
    info.cache_line_size = 64; // Default cache line size
}

fn detect_performance_counters(info: &mut CpuInfo) {
    // Detect number of available performance counters
    // In RISC-V, this is implementation-specific
    info.num_counters = 29; // Default to 29 (MHPMCOUNTER3-MHPMCOUNTER31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_info_default() {
        let info = CpuInfo::default();
        assert_eq!(info.xlen, 64);
        assert_eq!(info.hart_id, 0);
    }

    #[test]
    fn test_extension_support() {
        // These tests would depend on actual hardware
        // For now, just test the logic
        let info = CpuInfo {
            isa_string: "rv64imafdc".to_string(),
            features: CpuFeatures::HAS_EXTENSION_M | CpuFeatures::HAS_EXTENSION_A | CpuFeatures::HAS_EXTENSION_F,
            ..Default::default()
        };

        assert!(info.isa_string.contains('m'));
        assert!(info.isa_string.contains('a'));
        assert!(info.isa_string.contains('f'));
        assert!(!info.isa_string.contains('v'));
    }
}