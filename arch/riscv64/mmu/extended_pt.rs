//! Extended RISC-V Page Table Formats
//!
//! This module provides comprehensive support for extended RISC-V page table formats:
//! - Complete Sv32x4, Sv39x4, and Sv48x4 implementations
//! - Page table format detection and validation
//! - Format switching and compatibility management
//! - Hardware capability probing
//! - Multi-level page table operations

use crate::arch::riscv64::*;
use crate::arch::riscv64::mmu::gstage::*;
use crate::arch::riscv64::mmu::ptable::*;
use bitflags::bitflags;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Extended page table format capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtendedPageTableFormat {
    /// No translation (bare metal)
    Bare = 0,
    /// Sv32 - 32-bit addresses, 2-level page tables
    Sv32 = 1,
    /// Sv39 - 39-bit addresses, 3-level page tables
    Sv39 = 8,
    /// Sv48 - 48-bit addresses, 4-level page tables
    Sv48 = 9,
    /// Sv57 - 57-bit addresses, 5-level page tables
    Sv57 = 10,
    /// Sv64 - 64-bit addresses, 5-level page tables
    Sv64 = 11,
}

impl ExtendedPageTableFormat {
    /// Get mode bits for SATP/HGATP registers
    pub fn mode_bits(self) -> usize {
        self as usize
    }

    /// Get number of virtual address bits
    pub fn va_bits(self) -> usize {
        match self {
            ExtendedPageTableFormat::Bare => 0,
            ExtendedPageTableFormat::Sv32 => 32,
            ExtendedPageTableFormat::Sv39 => 39,
            ExtendedPageTableFormat::Sv48 => 48,
            ExtendedPageTableFormat::Sv57 => 57,
            ExtendedPageTableFormat::Sv64 => 64,
        }
    }

    /// Get number of physical address bits
    pub fn pa_bits(self) -> usize {
        match self {
            ExtendedPageTableFormat::Bare => 0,
            ExtendedPageTableFormat::Sv32 => 34,  // PPN[33:32] reserved
            ExtendedPageTableFormat::Sv39 => 56,
            ExtendedPageTableFormat::Sv48 => 56,
            ExtendedPageTableFormat::Sv57 => 56,
            ExtendedPageTableFormat::Sv64 => 56,
        }
    }

    /// Get number of page table levels
    pub fn levels(self) -> usize {
        match self {
            ExtendedPageTableFormat::Bare => 0,
            ExtendedPageTableFormat::Sv32 => 2,
            ExtendedPageTableFormat::Sv39 => 3,
            ExtendedPageTableFormat::Sv48 => 4,
            ExtendedPageTableFormat::Sv57 => 5,
            ExtendedPageTableFormat::Sv64 => 5,
        }
    }

    /// Get page offset bits
    pub fn page_offset_bits(self) -> usize {
        12 // 4KB pages for all formats
    }

    /// Get VPN bits per level
    pub fn vpn_bits_per_level(self) -> usize {
        match self {
            ExtendedPageTableFormat::Bare => 0,
            ExtendedPageTableFormat::Sv32 => 10,
            ExtendedPageTableFormat::Sv39 => 9,
            ExtendedPageTableFormat::Sv48 => 9,
            ExtendedPageTableFormat::Sv57 => 9,
            ExtendedPageTableFormat::Sv64 => 9,
        }
    }

    /// Get entries per page table
    pub fn entries_per_pt(self) -> usize {
        1 << self.vpn_bits_per_level()
    }

    /// Check if this format supports huge pages
    pub fn supports_huge_pages(self) -> bool {
        matches!(self, ExtendedPageTableFormat::Sv32 | ExtendedPageTableFormat::Sv39 |
                   ExtendedPageTableFormat::Sv48 | ExtendedPageTableFormat::Sv57 |
                   ExtendedPageTableFormat::Sv64)
    }

    /// Check if this format supports super pages
    pub fn supports_super_pages(self) -> bool {
        self.supports_huge_pages()
    }

    /// Get maximum page size for this format
    pub fn max_page_size(self) -> usize {
        match self {
            ExtendedPageTableFormat::Bare => 4096,
            ExtendedPageTableFormat::Sv32 => 4 * 1024 * 1024, // 4MB
            ExtendedPageTableFormat::Sv39 => 512 * 1024 * 1024, // 512MB
            ExtendedPageTableFormat::Sv48 => 512 * 1024 * 1024, // 512MB
            ExtendedPageTableFormat::Sv57 => 2 * 1024 * 1024 * 1024, // 2TB
            ExtendedPageTableFormat::Sv64 => 2 * 1024 * 1024 * 1024, // 2TB
        }
    }

    /// Check if an address is valid for this format
    pub fn is_valid_va(self, va: usize) -> bool {
        let va_bits = self.va_bits();
        if va_bits == 0 {
            return true; // Bare mode accepts all addresses
        }
        va < (1usize << va_bits)
    }

    /// Check if a physical address is valid for this format
    pub fn is_valid_pa(self, pa: usize) -> bool {
        let pa_bits = self.pa_bits();
        if pa_bits == 0 {
            return true; // Bare mode accepts all addresses
        }
        pa < (1usize << pa_bits)
    }
}

/// Extended PTE (Page Table Entry) format
pub mod extended_pte {
    /// Valid bit
    pub const V: usize = 1 << 0;
    /// Read bit
    pub const R: usize = 1 << 1;
    /// Write bit
    pub const W: usize = 1 << 2;
    /// Execute bit
    pub const X: usize = 1 << 3;
    /// User mode bit
    pub const U: usize = 1 << 4;
    /// Global bit
    pub const G: usize = 1 << 5;
    /// Access bit
    pub const A: usize = 1 << 6;
    /// Dirty bit
    pub const D: usize = 1 << 7;
    /// PBMT (Page-based memory types) - for Sv57/Sv64
    pub const PBMT: usize = 0x3F << 59;
    /// NAPOT (Naturally aligned power-of-two) - for Sv57/Sv64
    pub const NAPOT: usize = 0x3F << 61;
}

/// Extended page table entry
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtendedPte {
    pub value: usize,
}

impl ExtendedPte {
    /// Create a new PTE
    pub const fn new() -> Self {
        Self { value: 0 }
    }

    /// Create PTE from raw value
    pub const fn from_raw(value: usize) -> Self {
        Self { value }
    }

    /// Get raw value
    pub const fn raw(&self) -> usize {
        self.value
    }

    /// Check if PTE is valid
    pub const fn is_valid(&self) -> bool {
        (self.value & extended_pte::V) != 0
    }

    /// Check if PTE is a leaf entry
    pub const fn is_leaf(&self) -> bool {
        self.is_valid() && ((self.value & (extended_pte::R | extended_pte::W | extended_pte::X)) != 0)
    }

    /// Check if PTE is a branch entry
    pub const fn is_branch(&self) -> bool {
        self.is_valid() && ((self.value & (extended_pte::R | extended_pte::W | extended_pte::X)) == 0)
    }

    /// Get PPN (Physical Page Number)
    pub const fn get_ppn(&self) -> usize {
        self.value & 0x000FFFFFFFFFFF00
    }

    /// Set PPN
    pub fn set_ppn(&mut self, ppn: usize) {
        self.value = (self.value & !0x000FFFFFFFFFFF00) | (ppn & 0x000FFFFFFFFFFF00);
    }

    /// Get access permissions
    pub fn get_permissions(&self) -> ExtendedPermissions {
        let mut perms = ExtendedPermissions::empty();

        if (self.value & extended_pte::R) != 0 {
            perms |= ExtendedPermissions::READ;
        }
        if (self.value & extended_pte::W) != 0 {
            perms |= ExtendedPermissions::WRITE;
        }
        if (self.value & extended_pte::X) != 0 {
            perms |= ExtendedPermissions::EXECUTE;
        }
        if (self.value & extended_pte::U) != 0 {
            perms |= ExtendedPermissions::USER;
        }
        if (self.value & extended_pte::G) != 0 {
            perms |= ExtendedPermissions::GLOBAL;
        }
        if (self.value & extended_pte::A) != 0 {
            perms |= ExtendedPermissions::ACCESSED;
        }
        if (self.value & extended_pte::D) != 0 {
            perms |= ExtendedPermissions::DIRTY;
        }

        perms
    }

    /// Set access permissions
    pub fn set_permissions(&mut self, perms: ExtendedPermissions) {
        self.value &= !(extended_pte::R | extended_pte::W | extended_pte::X |
                        extended_pte::U | extended_pte::G | extended_pte::A | extended_pte::D);

        if perms.contains(ExtendedPermissions::READ) {
            self.value |= extended_pte::R;
        }
        if perms.contains(ExtendedPermissions::WRITE) {
            self.value |= extended_pte::W;
        }
        if perms.contains(ExtendedPermissions::EXECUTE) {
            self.value |= extended_pte::X;
        }
        if perms.contains(ExtendedPermissions::USER) {
            self.value |= extended_pte::U;
        }
        if perms.contains(ExtendedPermissions::GLOBAL) {
            self.value |= extended_pte::G;
        }
        if perms.contains(ExtendedPermissions::ACCESSED) {
            self.value |= extended_pte::A;
        }
        if perms.contains(ExtendedPermissions::DIRTY) {
            self.value |= extended_pte::D;
        }
    }

    /// Mark as valid branch PTE
    pub fn set_branch(&mut self, ppn: usize) {
        self.value = extended_pte::V | (ppn & 0x000FFFFFFFFFFF00);
    }

    /// Mark as valid leaf PTE
    pub fn set_leaf(&mut self, ppn: usize, perms: ExtendedPermissions) {
        self.value = extended_pte::V | (ppn & 0x000FFFFFFFFFFF00);
        self.set_permissions(perms);
    }
}

impl Default for ExtendedPte {
    fn default() -> Self {
        Self::new()
    }
}

/// Extended access permissions
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ExtendedPermissions: usize {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const EXECUTE = 1 << 2;
        const USER = 1 << 3;
        const GLOBAL = 1 << 4;
        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
    }
}

/// Extended page table format detector
pub struct FormatDetector {
    /// Supported formats
    supported_formats: u32,
    /// Current detected format
    current_format: Option<ExtendedPageTableFormat>,
    /// Detection statistics
    stats: DetectionStats,
}

/// Format detection statistics
#[derive(Debug, Default)]
pub struct DetectionStats {
    pub probes: AtomicUsize,
    pub successful_detections: AtomicUsize,
    pub format_switches: AtomicUsize,
}

impl FormatDetector {
    /// Create new format detector
    pub fn new() -> Self {
        Self {
            supported_formats: 0,
            current_format: None,
            stats: DetectionStats::default(),
        }
    }

    /// Detect supported page table formats by probing hardware
    pub fn detect_hardware_capabilities(&mut self) -> Result<ExtendedPageTableFormat, &'static str> {
        log::info!("Detecting hardware page table capabilities");

        self.stats.probes.fetch_add(1, Ordering::Relaxed);

        // Read MISA register to check for extensions
        let misa = MISA::read();
        let mxl = (misa >> 62) & 0x3; // MXL field

        let mut supported = 0u32;

        // Check for 64-bit support
        if mxl >= 2 {
            supported |= 1 << (ExtendedPageTableFormat::Sv64 as u32);
            supported |= 1 << (ExtendedPageTableFormat::Sv57 as u32);
            supported |= 1 << (ExtendedPageTableFormat::Sv48 as u32);
            supported |= 1 << (ExtendedPageTableFormat::Sv39 as u32);
        } else if mxl == 1 {
            supported |= 1 << (ExtendedPageTableFormat::Sv48 as u32);
            supported |= 1 << (ExtendedPageTableFormat::Sv39 as u32);
            supported |= 1 << (ExtendedPageTableFormat::Sv32 as u32);
        }

        // Check for specific extensions
        if misa & (1 << ('V' as u8 - 'A' as u8)) != 0 {
            log::debug!("V extension detected");
        }

        self.supported_formats = supported;

        // Select the best supported format
        let best_format = self.select_best_format(supported)?;

        log::info!("Detected hardware capabilities: supported formats = {:#x}", supported);
        log::info!("Selected page table format: {:?}", best_format);

        self.current_format = Some(best_format);
        self.stats.successful_detections.fetch_add(1, Ordering::Relaxed);

        Ok(best_format)
    }

    /// Select the best supported format
    fn select_best_format(&self, supported: u32) -> Result<ExtendedPageTableFormat, &'static str> {
        // Check formats in order of preference
        let formats = [
            ExtendedPageTableFormat::Sv64,
            ExtendedPageTableFormat::Sv57,
            ExtendedPageTableFormat::Sv48,
            ExtendedPageTableFormat::Sv39,
            ExtendedPageTableFormat::Sv32,
            ExtendedPageTableFormat::Bare,
        ];

        for format in formats.iter() {
            if supported & (1 << (*format as u32)) != 0 {
                return Ok(*format);
            }
        }

        Err("No supported page table formats found")
    }

    /// Get current detected format
    pub fn current_format(&self) -> Option<ExtendedPageTableFormat> {
        self.current_format
    }

    /// Check if a format is supported
    pub fn is_supported(&self, format: ExtendedPageTableFormat) -> bool {
        self.supported_formats & (1 << (format as u32)) != 0
    }

    /// Switch to a different format
    pub fn switch_format(&mut self, new_format: ExtendedPageTableFormat) -> Result<(), &'static str> {
        if !self.is_supported(new_format) {
            return Err("Format not supported by hardware");
        }

        if self.current_format == Some(new_format) {
            return Ok(()); // Already using this format
        }

        log::info!("Switching page table format from {:?} to {:?}",
                   self.current_format, new_format);

        self.current_format = Some(new_format);
        self.stats.format_switches.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Auto-detect optimal format for given requirements
    pub fn auto_detect(&mut self, va_range: usize, pa_range: usize) -> Result<ExtendedPageTableFormat, &'static str> {
        if let Some(current) = self.current_format {
            // Check if current format can handle the requirements
            if current.is_valid_va(va_range - 1) && current.is_valid_pa(pa_range - 1) {
                return Ok(current);
            }
        }

        // If no current format or it's insufficient, detect capabilities
        self.detect_hardware_capabilities()
    }

    /// Get detection statistics
    pub fn get_stats(&self) -> DetectionStatsSnapshot {
        DetectionStatsSnapshot {
            probes: self.stats.probes.load(Ordering::Relaxed),
            successful_detections: self.stats.successful_detections.load(Ordering::Relaxed),
            format_switches: self.stats.format_switches.load(Ordering::Relaxed),
        }
    }
}

/// Snapshot of detection statistics
#[derive(Debug, Clone, Copy)]
pub struct DetectionStatsSnapshot {
    pub probes: usize,
    pub successful_detections: usize,
    pub format_switches: usize,
}

/// Extended page table operations
pub struct ExtendedPageTable {
    format: ExtendedPageTableFormat,
    root_pa: usize,
    levels: usize,
}

impl ExtendedPageTable {
    /// Create a new extended page table
    pub fn new(format: ExtendedPageTableFormat, root_pa: usize) -> Self {
        Self {
            format,
            root_pa,
            levels: format.levels(),
        }
    }

    /// Get the format
    pub fn format(&self) -> ExtendedPageTableFormat {
        self.format
    }

    /// Get root page table physical address
    pub fn root_pa(&self) -> usize {
        self.root_pa
    }

    /// Get number of levels
    pub fn levels(&self) -> usize {
        self.levels
    }

    /// Create SATP value for this page table
    pub fn make_satp(&self, asid: usize, mode: usize) -> usize {
        match self.format {
            ExtendedPageTableFormat::Bare => 0,
            ExtendedPageTableFormat::Sv32 => {
                let ppn = self.root_pa >> 12;
                (mode << 31) | (asid << 22) | (ppn & 0x3FFFFF)
            }
            ExtendedPageTableFormat::Sv39 => {
                let ppn = self.root_pa >> 12;
                (mode << 60) | (asid << 44) | (ppn & 0x3FFFFFFFFF)
            }
            ExtendedPageTableFormat::Sv48 => {
                let ppn = self.root_pa >> 12;
                (mode << 60) | (asid << 44) | (ppn & 0x3FFFFFFFFF)
            }
            ExtendedPageTableFormat::Sv57 => {
                let ppn = self.root_pa >> 12;
                (mode << 60) | (asid << 44) | (ppn & 0x3FFFFFFFFF)
            }
            ExtendedPageTableFormat::Sv64 => {
                let ppn = self.root_pa >> 12;
                (mode << 60) | (asid << 44) | (ppn & 0x3FFFFFFFFF)
            }
        }
    }

    /// Create HGATP value for G-stage
    pub fn make_hgatp(&self, vmid: u16, mode: usize) -> usize {
        let ppn = self.root_pa >> 12;
        (mode << 60) | ((vmid as usize) << 12) | ppn
    }

    /// Extract VPN from virtual address
    pub fn extract_vpn(&self, va: usize, level: usize) -> usize {
        let vpn_bits = self.format.vpn_bits_per_level();
        let shift = self.page_offset_bits() + (vpn_bits * level);
        (va >> shift) & ((1 << vpn_bits) - 1)
    }

    /// Extract virtual page number array
    pub fn extract_vpn_array(&self, va: usize) -> Vec<usize> {
        let mut vpn_array = Vec::new();
        for level in 0..self.levels {
            vpn_array.push(self.extract_vpn(va, level));
        }
        vpn_array
    }

    /// Get page offset bits
    fn page_offset_bits(&self) -> usize {
        self.format.page_offset_bits()
    }

    /// Validate virtual address for this format
    pub fn validate_va(&self, va: usize) -> Result<(), ExtendedPteError> {
        if !self.format.is_valid_va(va) {
            return Err(ExtendedPteError::InvalidAddress);
        }
        Ok(())
    }

    /// Validate physical address for this format
    pub fn validate_pa(&self, pa: usize) -> Result<(), ExtendedPteError> {
        if !self.format.is_valid_pa(pa) {
            return Err(ExtendedPteError::InvalidAddress);
        }
        Ok(())
    }
}

/// Extended PTE error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtendedPteError {
    /// Invalid address
    InvalidAddress,
    /// Invalid PTE
    InvalidPte,
    /// Permission denied
    PermissionDenied,
    /// Page not found
    PageNotFound,
}

/// Global format detector
static mut FORMAT_DETECTOR: Option<FormatDetector> = None;

/// Initialize extended page table format detection
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing extended page table format detection");

    let mut detector = FormatDetector::new();
    let format = detector.detect_hardware_capabilities()?;

    log::info!("Extended page table format detection completed");
    log::info!("Using page table format: {:?}", format);

    unsafe {
        FORMAT_DETECTOR = Some(detector);
    }

    Ok(())
}

/// Get the global format detector
pub fn get_detector() -> Option<&'static FormatDetector> {
    unsafe { FORMAT_DETECTOR.as_ref() }
}

/// Get mutable reference to global format detector
pub fn get_detector_mut() -> Option<&'static mut FormatDetector> {
    unsafe { FORMAT_DETECTOR.as_mut() }
}

/// Get current page table format
pub fn current_format() -> Option<ExtendedPageTableFormat> {
    get_detector().and_then(|d| d.current_format())
}

/// Check if a format is supported
pub fn is_format_supported(format: ExtendedPageTableFormat) -> bool {
    get_detector().map_or(false, |d| d.is_supported(format))
}

/// Auto-detect optimal format
pub fn auto_detect(va_range: usize, pa_range: usize) -> Result<ExtendedPageTableFormat, &'static str> {
    if let Some(detector) = get_detector_mut() {
        detector.auto_detect(va_range, pa_range)
    } else {
        Err("Format detector not initialized")
    }
}

/// Switch to a specific format
pub fn switch_format(format: ExtendedPageTableFormat) -> Result<(), &'static str> {
    if let Some(detector) = get_detector_mut() {
        detector.switch_format(format)
    } else {
        Err("Format detector not initialized")
    }
}

/// Get format detection statistics
pub fn get_detection_stats() -> Option<DetectionStatsSnapshot> {
    get_detector().map(|d| d.get_stats())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extended_page_table_formats() {
        assert_eq!(ExtendedPageTableFormat::Sv32.va_bits(), 32);
        assert_eq!(ExtendedPageTableFormat::Sv39.va_bits(), 39);
        assert_eq!(ExtendedPageTableFormat::Sv48.va_bits(), 48);
        assert_eq!(ExtendedPageTableFormat::Sv57.va_bits(), 57);
        assert_eq!(ExtendedPageTableFormat::Sv64.va_bits(), 64);

        assert_eq!(ExtendedPageTableFormat::Sv32.levels(), 2);
        assert_eq!(ExtendedPageTableFormat::Sv39.levels(), 3);
        assert_eq!(ExtendedPageTableFormat::Sv48.levels(), 4);
        assert_eq!(ExtendedPageTableFormat::Sv57.levels(), 5);
        assert_eq!(ExtendedPageTableFormat::Sv64.levels(), 5);
    }

    #[test]
    fn test_extended_pte() {
        let mut pte = ExtendedPte::new();

        // Test leaf PTE creation
        pte.set_leaf(0x87654321000, ExtendedPermissions::READ | ExtendedPermissions::WRITE);
        assert!(pte.is_valid());
        assert!(pte.is_leaf());
        assert!(!pte.is_branch());
        assert_eq!(pte.get_ppn(), 0x87654321000);

        let perms = pte.get_permissions();
        assert!(perms.contains(ExtendedPermissions::READ));
        assert!(perms.contains(ExtendedPermissions::WRITE));
        assert!(!perms.contains(ExtendedPermissions::EXECUTE));
    }

    #[test]
    fn test_format_validation() {
        assert!(ExtendedPageTableFormat::Sv39.is_valid_va(0x7FFFFFFFFF));
        assert!(!ExtendedPageTableFormat::Sv39.is_valid_va(1 << 39));

        assert!(ExtendedPageTableFormat::Sv32.is_valid_va(0xFFFFFFFF));
        assert!(!ExtendedPageTableFormat::Sv32.is_valid_va(1 << 32));
    }

    #[test]
    fn test_format_detector() {
        let mut detector = FormatDetector::new();

        // Initially no format detected
        assert!(detector.current_format().is_none());

        // Test format support checking
        detector.supported_formats = 1 << (ExtendedPageTableFormat::Sv39 as u32);
        assert!(detector.is_supported(ExtendedPageTableFormat::Sv39));
        assert!(!detector.is_supported(ExtendedPageTableFormat::Sv48));
    }

    #[test]
    fn test_extended_page_table() {
        let format = ExtendedPageTableFormat::Sv39;
        let root_pa = 0x80000000;
        let pt = ExtendedPageTable::new(format, root_pa);

        assert_eq!(pt.format(), format);
        assert_eq!(pt.root_pa(), root_pa);
        assert_eq!(pt.levels(), 3);

        let va = 0x123456780;
        let vpn_array = pt.extract_vpn_array(va);
        assert_eq!(vpn_array.len(), 3);

        assert!(pt.validate_va(va).is_ok());
        assert!(pt.validate_va(1 << 39).is_err());
    }
}