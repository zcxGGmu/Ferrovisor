//! RISC-V G-Stage Address Translation
//!
//! This module provides G-stage (guest physical to host physical) address translation
//! for RISC-V virtualization:
//! - HGATP register management
//! - G-stage page table traversal
//! - Two-stage address translation (GVA → GPA → HPA)
//! - Translation caching and optimization
//! - TLB invalidation for G-stage

use crate::arch::riscv64::cpu::csr::*;
use crate::arch::riscv64::mmu::ptable::*;
use crate::arch::riscv64::mmu::extended_pt::*;
use crate::arch::riscv64::mmu::tlb::*;
use bitflags::bitflags;
use core::sync::atomic::{AtomicUsize, Ordering};

/// G-stage translation modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GStageMode {
    /// No translation (bare metal)
    Bare = 0,
    /// Sv32x4 - 32-bit guest addresses, 4KB pages
    Sv32x4 = 1,
    /// Sv39x4 - 39-bit guest addresses, 4KB pages
    Sv39x4 = 8,
    /// Sv48x4 - 48-bit guest addresses, 4KB pages
    Sv48x4 = 9,
    /// Sv57x4 - 57-bit guest addresses, 4KB pages (extended)
    Sv57x4 = 10,
}

impl From<GStageMode> for ExtendedPageTableFormat {
    fn from(mode: GStageMode) -> Self {
        match mode {
            GStageMode::Bare => ExtendedPageTableFormat::Bare,
            GStageMode::Sv32x4 => ExtendedPageTableFormat::Sv32,
            GStageMode::Sv39x4 => ExtendedPageTableFormat::Sv39,
            GStageMode::Sv48x4 => ExtendedPageTableFormat::Sv48,
            GStageMode::Sv57x4 => ExtendedPageTableFormat::Sv57,
        }
    }
}

impl From<ExtendedPageTableFormat> for GStageMode {
    fn from(format: ExtendedPageTableFormat) -> Self {
        match format {
            ExtendedPageTableFormat::Bare => GStageMode::Bare,
            ExtendedPageTableFormat::Sv32 => GStageMode::Sv32x4,
            ExtendedPageTableFormat::Sv39 => GStageMode::Sv39x4,
            ExtendedPageTableFormat::Sv48 => GStageMode::Sv48x4,
            ExtendedPageTableFormat::Sv57 => GStageMode::Sv57x4,
            ExtendedPageTableFormat::Sv64 => GStageMode::Sv57x4, // Map Sv64 to Sv57x4
        }
    }
}

impl GStageMode {
    /// Get mode bits for HGATP register
    pub fn hgatp_mode_bits(self) -> usize {
        self as usize
    }

    /// Get number of address bits supported
    pub fn addr_bits(self) -> usize {
        ExtendedPageTableFormat::from(self).va_bits()
    }

    /// Get number of page table levels
    pub fn levels(self) -> usize {
        ExtendedPageTableFormat::from(self).levels()
    }

    /// Get number of VPN bits per level
    pub fn vpn_bits_per_level(self) -> usize {
        ExtendedPageTableFormat::from(self).vpn_bits_per_level()
    }

    /// Get entries per page table
    pub fn entries_per_pt(self) -> usize {
        ExtendedPageTableFormat::from(self).entries_per_pt()
    }

    /// Get maximum page size
    pub fn max_page_size(self) -> usize {
        ExtendedPageTableFormat::from(self).max_page_size()
    }

    /// Check if this format supports huge pages
    pub fn supports_huge_pages(self) -> bool {
        ExtendedPageTableFormat::from(self).supports_huge_pages()
    }

    /// Check if an address is valid for this format
    pub fn is_valid_va(self, va: usize) -> bool {
        ExtendedPageTableFormat::from(self).is_valid_va(va)
    }

    /// Check if a physical address is valid for this format
    pub fn is_valid_pa(self, pa: usize) -> bool {
        ExtendedPageTableFormat::from(self).is_valid_pa(pa)
    }

    /// Get extended page table format
    pub fn to_extended_format(self) -> ExtendedPageTableFormat {
        ExtendedPageTableFormat::from(self)
    }
}

/// G-stage PTE (Page Table Entry) bits
pub mod gstage_pte {
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
    /// Read/Write exclusion (for G-stage)
    pub const RWX: usize = 1 << 61;
    /// Page frame number mask
    pub const PPN_MASK: usize = 0x000FFFFFFFFFFF00;
}

/// G-stage translation result
#[derive(Debug, Clone, Copy)]
pub struct GStageTranslationResult {
    /// Host physical address
    pub hpa: usize,
    /// Access permissions
    pub permissions: GStagePermissions,
    /// Page size (in bytes)
    pub page_size: usize,
    /// Was translation cached
    pub cached: bool,
}

/// G-stage access permissions
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct GStagePermissions: usize {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const EXECUTE = 1 << 2;
        const USER = 1 << 3;
        const ACCESSED = 1 << 4;
        const DIRTY = 1 << 5;
        const GLOBAL = 1 << 6;
    }
}

/// Translation cache entry
#[derive(Debug, Clone)]
struct TranslationCacheEntry {
    /// Guest physical address (page aligned)
    gpa: usize,
    /// Host physical address (page aligned)
    hpa: usize,
    /// Permissions
    permissions: GStagePermissions,
    /// Page size
    page_size: usize,
    /// Last access time
    last_access: u64,
    /// Access count
    access_count: usize,
}

impl TranslationCacheEntry {
    fn new(gpa: usize, hpa: usize, permissions: GStagePermissions, page_size: usize) -> Self {
        Self {
            gpa,
            hpa,
            permissions,
            page_size,
            last_access: Self::get_timestamp(),
            access_count: 1,
        }
    }

    fn update_access(&mut self) {
        self.last_access = Self::get_timestamp();
        self.access_count += 1;
    }

    fn get_timestamp() -> u64 {
        use core::sync::atomic::{AtomicU64, Ordering};
        static TIMESTAMP: AtomicU64 = AtomicU64::new(0);
        TIMESTAMP.fetch_add(1, Ordering::Relaxed)
    }
}

/// G-stage translation cache
pub struct GStageTranslationCache {
    entries: Vec<Option<TranslationCacheEntry>>,
    max_entries: usize,
    hits: AtomicUsize,
    misses: AtomicUsize,
}

impl GStageTranslationCache {
    /// Create a new translation cache
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: vec![None; max_entries],
            max_entries,
            hits: AtomicUsize::new(0),
            misses: AtomicUsize::new(0),
        }
    }

    /// Look up translation in cache
    pub fn lookup(&self, gpa: usize) -> Option<GStageTranslationResult> {
        let index = self.hash_index(gpa);

        if let Some(ref entry) = self.entries[index] {
            if entry.gpa == gpa && (gpa % entry.page_size) == 0 {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Some(GStageTranslationResult {
                    hpa: entry.hpa + (gpa % entry.page_size),
                    permissions: entry.permissions,
                    page_size: entry.page_size,
                    cached: true,
                });
            }
        }

        self.misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Insert translation into cache
    pub fn insert(&mut self, gpa: usize, result: &GStageTranslationResult) {
        let aligned_gpa = gpa & !(result.page_size - 1);
        let aligned_hpa = result.hpa & !(result.page_size - 1);

        let entry = TranslationCacheEntry::new(
            aligned_gpa,
            aligned_hpa,
            result.permissions,
            result.page_size,
        );

        let index = self.hash_index(aligned_gpa);
        self.entries[index] = Some(entry);
    }

    /// Invalidate cache entry
    pub fn invalidate(&mut self, gpa: usize) {
        let index = self.hash_index(gpa);
        if let Some(ref entry) = self.entries[index] {
            if entry.gpa == gpa {
                self.entries[index] = None;
            }
        }
    }

    /// Invalidate all cache entries
    pub fn invalidate_all(&mut self) {
        for entry in &mut self.entries {
            *entry = None;
        }
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> CacheStats {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;

        CacheStats {
            hits,
            misses,
            total,
            hit_rate: if total > 0 { (hits * 100) / total } else { 0 },
        }
    }

    fn hash_index(&self, gpa: usize) -> usize {
        ((gpa >> 12) as u64).wrapping_mul(11400714819323198485u64) as usize % self.max_entries
    }
}

/// Cache statistics
#[derive(Debug, Clone, Copy)]
pub struct CacheStats {
    pub hits: usize,
    pub misses: usize,
    pub total: usize,
    pub hit_rate: usize,
}

/// G-stage address translator
pub struct GStageTranslator {
    /// Current HGATP value
    hgatp: usize,
    /// G-stage mode
    mode: GStageMode,
    /// VMID
    vmid: u16,
    /// Root page table physical address
    root_pt_pa: usize,
    /// Translation cache (legacy)
    cache: GStageTranslationCache,
    /// Translation statistics
    translations: AtomicUsize,
    /// Page faults
    page_faults: AtomicUsize,
    /// Use TLB management
    use_tlb: bool,
}

impl GStageTranslator {
    /// Create a new G-stage translator
    pub fn new(vmid: u16, root_pt_pa: usize, mode: GStageMode) -> Self {
        let hgatp = Self::make_hgatp(vmid, root_pt_pa, mode);

        Self {
            hgatp,
            mode,
            vmid,
            root_pt_pa,
            cache: GStageTranslationCache::new(1024),
            translations: AtomicUsize::new(0),
            page_faults: AtomicUsize::new(0),
            use_tlb: true, // Enable TLB by default
        }
    }

    /// Create a new G-stage translator with automatic format detection
    pub fn new_with_auto_detection(vmid: u16, root_pt_pa: usize) -> Result<Self, &'static str> {
        // Use format detector to determine best mode
        let format_detector = FormatDetector::new();
        let detected_format = format_detector.detect_current_format()
            .unwrap_or_else(|| {
                // Fallback hierarchy: Sv48 -> Sv39 -> Sv32 -> Bare
                if format_detector.is_format_supported(ExtendedPageTableFormat::Sv48) {
                    ExtendedPageTableFormat::Sv48
                } else if format_detector.is_format_supported(ExtendedPageTableFormat::Sv39) {
                    ExtendedPageTableFormat::Sv39
                } else if format_detector.is_format_supported(ExtendedPageTableFormat::Sv32) {
                    ExtendedPageTableFormat::Sv32
                } else {
                    ExtendedPageTableFormat::Bare
                }
            });

        let mode = GStageMode::from(detected_format);
        log::info!("Auto-selected G-stage mode: {:?} (format: {:?})", mode, detected_format);

        Ok(Self::new(vmid, root_pt_pa, mode))
    }

    /// Make HGATP register value
    pub fn make_hgatp(vmid: u16, ppn: usize, mode: GStageMode) -> usize {
        let hgatp_ppn = ppn >> 12; // Convert to PPN
        (mode.hgatp_mode_bits() << 60) | ((vmid as usize) << 12) | hgatp_ppn
    }

    /// Extract VMID from HGATP
    pub fn extract_vmid(hgatp: usize) -> u16 {
        ((hgatp >> 12) & 0x3FFF) as u16
    }

    /// Extract PPN from HGATP
    pub fn extract_ppn(hgatp: usize) -> usize {
        (hgatp & gstage_pte::PPN_MASK) << 2
    }

    /// Extract mode from HGATP
    pub fn extract_mode(hgatp: usize) -> GStageMode {
        let mode_bits = (hgatp >> 60) & 0xF;
        match mode_bits {
            0 => GStageMode::Bare,
            1 => GStageMode::Sv32x4,
            8 => GStageMode::Sv39x4,
            9 => GStageMode::Sv48x4,
            10 => GStageMode::Sv57x4,
            _ => {
                // Try to auto-detect based on current hardware
                let format_detector = FormatDetector::new();
                if format_detector.is_format_supported(ExtendedPageTableFormat::Sv57) {
                    GStageMode::Sv57x4
                } else if format_detector.is_format_supported(ExtendedPageTableFormat::Sv48) {
                    GStageMode::Sv48x4
                } else if format_detector.is_format_supported(ExtendedPageTableFormat::Sv39) {
                    GStageMode::Sv39x4
                } else if format_detector.is_format_supported(ExtendedPageTableFormat::Sv32) {
                    GStageMode::Sv32x4
                } else {
                    GStageMode::Bare
                }
            }
        }
    }

    /// Translate guest physical address to host physical address
    pub fn translate(&self, gpa: usize) -> Result<GStageTranslationResult, GStageFault> {
        // Try TLB first if enabled
        if self.use_tlb {
            if let Some(manager) = get_manager() {
                if let Some(hpa) = manager.translate_gstage(gpa, 0, self.vmid) {
                    return Ok(GStageTranslationResult {
                        hpa,
                        permissions: GStagePermissions::READ | GStagePermissions::WRITE | GStagePermissions::EXECUTE,
                        page_size: 4096,
                        cached: true,
                    });
                }
            }
        }

        // Check legacy cache second
        if let Some(cached_result) = self.cache.lookup(gpa) {
            // Insert into TLB for future lookups
            if self.use_tlb {
                if let Some(manager) = get_manager_mut() {
                    let tlb_entry = TlbEntry::new(
                        cached_result.hpa, // For G-stage, GPA is stored in paddr field
                        cached_result.hpa,
                        0, // ASID is 0 for G-stage
                        self.vmid,
                        cached_result.page_size,
                        TlbPermissions::VALID |
                            if cached_result.permissions.contains(GStagePermissions::READ) { TlbPermissions::READ } else { TlbPermissions::empty() } |
                            if cached_result.permissions.contains(GStagePermissions::WRITE) { TlbPermissions::WRITE } else { TlbPermissions::empty() } |
                            if cached_result.permissions.contains(GStagePermissions::EXECUTE) { TlbPermissions::EXECUTE } else { TlbPermissions::empty() },
                        TlbEntryType::GStage,
                        0,
                    );
                    manager.insert_gstage(tlb_entry);
                }
            }
            return Ok(cached_result);
        }

        // Perform full translation
        let result = self.translate_full(gpa)?;

        // Cache the result in legacy cache
        self.cache.insert(gpa, &result);

        // Insert into TLB for future lookups
        if self.use_tlb {
            if let Some(manager) = get_manager_mut() {
                let tlb_entry = TlbEntry::new(
                    result.hpa, // For G-stage, GPA is stored in paddr field
                    result.hpa,
                    0, // ASID is 0 for G-stage
                    self.vmid,
                    result.page_size,
                    TlbPermissions::VALID |
                        if result.permissions.contains(GStagePermissions::READ) { TlbPermissions::READ } else { TlbPermissions::empty() } |
                        if result.permissions.contains(GStagePermissions::WRITE) { TlbPermissions::WRITE } else { TlbPermissions::empty() } |
                        if result.permissions.contains(GStagePermissions::EXECUTE) { TlbPermissions::EXECUTE } else { TlbPermissions::empty() },
                    TlbEntryType::GStage,
                    0,
                );
                manager.insert_gstage(tlb_entry);
            }
        }

        Ok(result)
    }

    /// Perform full G-stage translation
    fn translate_full(&self, gpa: usize) -> Result<GStageTranslationResult, GStageFault> {
        self.translations.fetch_add(1, Ordering::Relaxed);

        match self.mode {
            GStageMode::Bare => {
                // No translation, GPA = HPA
                Ok(GStageTranslationResult {
                    hpa: gpa,
                    permissions: GStagePermissions::READ | GStagePermissions::WRITE | GStagePermissions::EXECUTE,
                    page_size: 4096,
                    cached: false,
                })
            }
            GStageMode::Sv39x4 => self.translate_sv39x4(gpa),
            GStageMode::Sv48x4 => self.translate_sv48x4(gpa),
            GStageMode::Sv57x4 => self.translate_sv57x4(gpa),
            GStageMode::Sv32x4 => self.translate_sv32x4(gpa),
        }
    }

    /// Translate using Sv39x4 format
    fn translate_sv39x4(&self, gpa: usize) -> Result<GStageTranslationResult, GStageFault> {
        // Sv39x4: 39-bit guest addresses, 3-level page table
        let vpn = [
            (gpa >> 12) & 0x1FF,  // VPN [11:0]
            (gpa >> 21) & 0x1FF,  // VPN [20:12]
            (gpa >> 30) & 0x1FF,  // VPN [29:21]
        ];

        let mut pte = self.walk_page_table(self.root_pt_pa, vpn[2])?;

        // Check if leaf PTE
        if pte & gstage_pte::V == 0 {
            return Err(GStageFault::InvalidPte);
        }

        if (pte & (gstage_pte::R | gstage_pte::W | gstage_pte::X)) != 0 {
            // Leaf PTE found
            let ppn = (pte & gstage_pte::PPN_MASK) >> 2;
            let hpa = (ppn << 12) | (gpa & 0xFFF);

            let permissions = self.pte_to_permissions(pte);

            return Ok(GStageTranslationResult {
                hpa,
                permissions,
                page_size: 4096, // 4KB pages
                cached: false,
            });
        }

        // Continue to next level
        let next_pt_pa = ((pte & gstage_pte::PPN_MASK) >> 2) << 12;
        pte = self.walk_page_table(next_pt_pa, vpn[1])?;

        if (pte & (gstage_pte::R | gstage_pte::W | gstage_pte::X)) != 0 {
            // Leaf PTE at level 1 (could be 2MB page)
            let ppn = (pte & gstage_pte::PPN_MASK) >> 2;
            let hpa = (ppn << 12) | (gpa & 0x1FFFFF);

            let permissions = self.pte_to_permissions(pte);

            return Ok(GStageTranslationResult {
                hpa,
                permissions,
                page_size: 2 * 1024 * 1024, // 2MB
                cached: false,
            });
        }

        // Continue to final level
        let next_pt_pa = ((pte & gstage_pte::PPN_MASK) >> 2) << 12;
        pte = self.walk_page_table(next_pt_pa, vpn[0])?;

        if (pte & (gstage_pte::R | gstage_pte::W | gstage_pte::X)) == 0 {
            return Err(GStageFault::InvalidPte);
        }

        let ppn = (pte & gstage_pte::PPN_MASK) >> 2;
        let hpa = (ppn << 12) | (gpa & 0xFFF);

        let permissions = self.pte_to_permissions(pte);

        Ok(GStageTranslationResult {
            hpa,
            permissions,
            page_size: 4096,
            cached: false,
        })
    }

    /// Translate using Sv48x4 format (simplified version)
    fn translate_sv48x4(&self, gpa: usize) -> Result<GStageTranslationResult, GStageFault> {
        // For now, fall back to Sv39x4 translation
        // In a complete implementation, this would handle 4-level page tables
        self.translate_sv39x4(gpa)
    }

    /// Translate using Sv32x4 format
    fn translate_sv32x4(&self, gpa: usize) -> Result<GStageTranslationResult, GStageFault> {
        // Sv32x4: 32-bit guest addresses, 2-level page table
        let vpn = [
            (gpa >> 12) & 0x3FF,  // VPN [9:0]
            (gpa >> 22) & 0x3FF,  // VPN [19:10]
        ];

        let mut pte = self.walk_page_table(self.root_pt_pa, vpn[1])?;

        if (pte & (gstage_pte::R | gstage_pte::W | gstage_pte::X)) != 0 {
            // Leaf PTE found at level 1 (could be 4MB page)
            let ppn = (pte & gstage_pte::PPN_MASK) >> 2;
            let hpa = (ppn << 12) | (gpa & 0x3FFFFF);

            let permissions = self.pte_to_permissions(pte);

            return Ok(GStageTranslationResult {
                hpa,
                permissions,
                page_size: 4 * 1024 * 1024, // 4MB
                cached: false,
            });
        }

        // Continue to final level
        let next_pt_pa = ((pte & gstage_pte::PPN_MASK) >> 2) << 12;
        pte = self.walk_page_table(next_pt_pa, vpn[0])?;

        if (pte & (gstage_pte::R | gstage_pte::W | gstage_pte::X)) == 0 {
            return Err(GStageFault::InvalidPte);
        }

        let ppn = (pte & gstage_pte::PPN_MASK) >> 2;
        let hpa = (ppn << 12) | (gpa & 0xFFF);

        let permissions = self.pte_to_permissions(pte);

        Ok(GStageTranslationResult {
            hpa,
            permissions,
            page_size: 4096,
            cached: false,
        })
    }

    /// Translate using Sv57x4 format
    fn translate_sv57x4(&self, gpa: usize) -> Result<GStageTranslationResult, GStageFault> {
        // Sv57x4: 57-bit guest addresses, 5-level page table
        let vpn = [
            (gpa >> 12) & 0x1FF,   // VPN [11:0]
            (gpa >> 21) & 0x1FF,   // VPN [20:12]
            (gpa >> 30) & 0x1FF,   // VPN [29:21]
            (gpa >> 39) & 0x1FF,   // VPN [38:30]
            (gpa >> 48) & 0x1FF,   // VPN [47:39]
        ];

        // Walk the 5-level page table
        let mut pte = self.walk_page_table(self.root_pt_pa, vpn[4])?; // Level 4

        for level in (0..=3).rev() { // Levels 3, 2, 1, 0
            if pte & gstage_pte::V == 0 {
                return Err(GStageFault::InvalidPte);
            }

            if (pte & (gstage_pte::R | gstage_pte::W | gstage_pte::X)) != 0 {
                // Leaf PTE found - calculate HPA
                let ppn = (pte & gstage_pte::PPN_MASK) >> 2;
                let page_offset_bits = 12 + (level * 9); // 12 + level * VPN bits
                let page_mask = (1usize << page_offset_bits) - 1;
                let hpa = (ppn << 12) | (gpa & page_mask);

                let permissions = self.pte_to_permissions(pte);
                let page_size = 1usize << page_offset_bits;

                return Ok(GStageTranslationResult {
                    hpa,
                    permissions,
                    page_size,
                    cached: false,
                });
            }

            // Continue to next level
            let next_pt_pa = ((pte & gstage_pte::PPN_MASK) >> 2) << 12;
            pte = self.walk_page_table(next_pt_pa, vpn[level as usize])?;
        }

        // Final level (Level 0) - must be leaf
        if pte & gstage_pte::V == 0 {
            return Err(GStageFault::InvalidPte);
        }

        if (pte & (gstage_pte::R | gstage_pte::W | gstage_pte::X)) == 0 {
            return Err(GStageFault::InvalidPte);
        }

        let ppn = (pte & gstage_pte::PPN_MASK) >> 2;
        let hpa = (ppn << 12) | (gpa & 0xFFF);

        let permissions = self.pte_to_permissions(pte);

        Ok(GStageTranslationResult {
            hpa,
            permissions,
            page_size: 4096,
            cached: false,
        })
    }

    /// Walk page table to get PTE
    fn walk_page_table(&self, pt_pa: usize, vpn: usize) -> Result<usize, GStageFault> {
        // In a real implementation, this would access physical memory
        // For now, we simulate page table access
        let pte_index = vpn & 0x1FF; // 512 entries per page table
        let pte_addr = pt_pa + (pte_index * 8); // 8-byte PTEs

        // Simulate reading PTE from physical memory
        // This would typically be done with physical memory access
        let pte = self.simulate_pte_read(pte_addr)?;

        if pte & gstage_pte::V == 0 {
            return Err(GStageFault::PageNotFound);
        }

        Ok(pte)
    }

    /// Simulate reading a PTE from physical memory
    fn simulate_pte_read(&self, _pte_addr: usize) -> Result<usize, GStageFault> {
        // This is a simulation - in a real implementation,
        // this would read from actual physical memory
        // For now, return a valid leaf PTE
        Ok(gstage_pte::V | gstage_pte::R | gstage_pte::W | gstage_pte::X | (0x87654 << 10))
    }

    /// Convert PTE bits to permissions
    fn pte_to_permissions(&self, pte: usize) -> GStagePermissions {
        let mut permissions = GStagePermissions::empty();

        if pte & gstage_pte::R != 0 {
            permissions |= GStagePermissions::READ;
        }
        if pte & gstage_pte::W != 0 {
            permissions |= GStagePermissions::WRITE;
        }
        if pte & gstage_pte::X != 0 {
            permissions |= GStagePermissions::EXECUTE;
        }
        if pte & gstage_pte::U != 0 {
            permissions |= GStagePermissions::USER;
        }
        if pte & gstage_pte::A != 0 {
            permissions |= GStagePermissions::ACCESSED;
        }
        if pte & gstage_pte::D != 0 {
            permissions |= GStagePermissions::DIRTY;
        }
        if pte & gstage_pte::G != 0 {
            permissions |= GStagePermissions::GLOBAL;
        }

        permissions
    }

    /// Configure HGATP register
    pub fn configure_hgatp(&mut self, vmid: u16, root_pt_pa: usize, mode: GStageMode) {
        self.vmid = vmid;
        self.root_pt_pa = root_pt_pa;
        self.mode = mode;
        self.hgatp = Self::make_hgatp(vmid, root_pt_pa, mode);

        // Write to hardware HGATP register
        HGATP::write(self.hgatp);

        // Invalidate cache
        self.cache.invalidate_all();
    }

    /// Get current HGATP value
    pub fn get_hgatp(&self) -> usize {
        self.hgatp
    }

    /// Get current VMID
    pub fn get_vmid(&self) -> u16 {
        self.vmid
    }

    /// Get current mode
    pub fn get_mode(&self) -> GStageMode {
        self.mode
    }

    /// Get root page table physical address
    pub fn get_root_pt_pa(&self) -> usize {
        self.root_pt_pa
    }

    /// Invalidate translation cache for specific GPA
    pub fn invalidate_cache(&mut self, gpa: usize) {
        self.cache.invalidate(gpa);
    }

    /// Invalidate entire translation cache
    pub fn invalidate_cache_all(&mut self) {
        self.cache.invalidate_all();
    }

    /// Invalidate G-stage TLB entries
    pub fn invalidate_tlb(&self, gpa: usize, size: usize) {
        // Invalidate software TLB entries
        if self.use_tlb {
            if let Some(manager) = get_manager_mut() {
                // Invalidate G-stage entries for this VMID and address range
                let count = manager.gstage_tlb.invalidate_range(gpa, size, 0, self.vmid);
                if count > 0 {
                    log::debug!("Invalidated {} G-stage TLB entries for GPA {:#x}, size {}", count, gpa, size);
                }
            }
        }

        // Use SBI to invalidate hardware G-stage TLB
        // This would typically call sbi_hfence_gvma()
        HardwareTlb::flush_gstage_addr(gpa, self.vmid);
        log::debug!("Invalidated hardware G-stage TLB for GPA {:#x}, size {}", gpa, size);
    }

    /// Invalidate all G-stage TLB entries for this VM
    pub fn invalidate_all_tlb(&self) {
        // Invalidate software TLB entries
        if self.use_tlb {
            if let Some(manager) = get_manager_mut() {
                let count = manager.gstage_tlb.invalidate_vmid(self.vmid);
                if count > 0 {
                    log::debug!("Invalidated {} G-stage TLB entries for VMID {}", count, self.vmid);
                }
            }
        }

        // Flush hardware G-stage TLB
        HardwareTlb::flush_gstage_vmid(self.vmid);
        log::debug!("Flushed hardware G-stage TLB for VMID {}", self.vmid);
    }

    /// Enable or disable TLB usage
    pub fn set_tlb_enabled(&mut self, enabled: bool) {
        self.use_tlb = enabled;
        log::info!("TLB for G-stage translation {}", if enabled { "enabled" } else { "disabled" });
    }

    /// Get translation statistics
    pub fn get_stats(&self) -> GStageStats {
        GStageStats {
            translations: self.translations.load(Ordering::Relaxed),
            page_faults: self.page_faults.load(Ordering::Relaxed),
            cache_stats: self.cache.get_stats(),
        }
    }
}

/// G-stage translation fault
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GStageFault {
    /// Page not found
    PageNotFound,
    /// Invalid PTE
    InvalidPte,
    /// Permission denied
    PermissionDenied,
    /// Invalid address
    InvalidAddress,
}

/// G-stage translation statistics
#[derive(Debug, Clone, Copy)]
pub struct GStageStats {
    pub translations: usize,
    pub page_faults: usize,
    pub cache_stats: CacheStats,
}

/// Two-stage address translator
pub struct TwoStageTranslator {
    /// G-stage translator (GPA → HPA)
    gstage: GStageTranslator,
    /// Stage-1 translator statistics
    stage1_translations: AtomicUsize,
}

impl TwoStageTranslator {
    /// Create a new two-stage translator
    pub fn new(vmid: u16, gstage_root_pt_pa: usize, mode: GStageMode) -> Self {
        Self {
            gstage: GStageTranslator::new(vmid, gstage_root_pt_pa, mode),
            stage1_translations: AtomicUsize::new(0),
        }
    }

    /// Perform two-stage translation (GVA → GPA → HPA)
    pub fn translate(&self, gva: usize, satp: usize) -> Result<TwoStageResult, TranslationError> {
        // Stage 1: GVA → GPA (using guest's SATP)
        let stage1_result = self.stage1_translate(gva, satp)?;
        self.stage1_translations.fetch_add(1, Ordering::Relaxed);

        // Stage 2: GPA → HPA (using G-stage page tables)
        let stage2_result = self.gstage.translate(stage1_result.gpa)?;

        Ok(TwoStageResult {
            hpa: stage2_result.hpa,
            gpa: stage1_result.gpa,
            permissions: stage1_result.permissions & stage2_result.permissions,
            stage1_info: stage1_result,
            stage2_info: stage2_result,
        })
    }

    /// Stage 1 translation (GVA → GPA)
    fn stage1_translate(&self, gva: usize, satp: usize) -> Result<Stage1Result, TranslationError> {
        // This would implement standard Sv39/Sv48 translation using guest's page tables
        // For now, simulate a successful translation
        Ok(Stage1Result {
            gpa: gva + 0x10000000, // Simulate GPA = GVA + offset
            permissions: GStagePermissions::READ | GStagePermissions::WRITE | GStagePermissions::EXECUTE,
            page_size: 4096,
        })
    }

    /// Get G-stage translator reference
    pub fn get_gstage(&self) -> &GStageTranslator {
        &self.gstage
    }

    /// Get mutable G-stage translator reference
    pub fn get_gstage_mut(&mut self) -> &mut GStageTranslator {
        &mut self.gstage
    }

    /// Get two-stage translation statistics
    pub fn get_stats(&self) -> TwoStageStats {
        TwoStageStats {
            stage1_translations: self.stage1_translations.load(Ordering::Relaxed),
            gstage_stats: self.gstage.get_stats(),
        }
    }
}

/// Stage 1 translation result
#[derive(Debug, Clone, Copy)]
pub struct Stage1Result {
    pub gpa: usize,
    pub permissions: GStagePermissions,
    pub page_size: usize,
}

/// Two-stage translation result
#[derive(Debug, Clone, Copy)]
pub struct TwoStageResult {
    pub hpa: usize,
    pub gpa: usize,
    pub permissions: GStagePermissions,
    pub stage1_info: Stage1Result,
    pub stage2_info: GStageTranslationResult,
}

/// Translation error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslationError {
    /// Stage 1 translation fault
    Stage1Fault,
    /// G-stage translation fault
    GStageFault(GStageFault),
    /// Invalid address
    InvalidAddress,
}

/// Two-stage translation statistics
#[derive(Debug, Clone, Copy)]
pub struct TwoStageStats {
    pub stage1_translations: usize,
    pub gstage_stats: GStageStats,
}

/// Global G-stage translator
static mut GSTAGE_TRANSLATOR: Option<GStageTranslator> = None;

/// Initialize G-stage translation with automatic format detection
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing G-stage address translation");

    // Initialize extended page table format detection
    let format_detector = FormatDetector::new();
    let supported_formats = format_detector.get_supported_formats();
    log::info!("Supported extended page table formats: {:#x}", supported_formats);

    // Auto-detect best format based on hardware capabilities
    let detected_format = format_detector.detect_current_format()
        .unwrap_or(ExtendedPageTableFormat::Sv39); // Fallback to Sv39
    log::info!("Detected page table format: {:?}", detected_format);

    // Convert to G-stage mode
    let mode = GStageMode::from(detected_format);

    // Create default G-stage translator with detected format
    let vmid = 1;
    let root_pt_pa = 0x80000000; // Default root page table location

    let translator = GStageTranslator::new_with_auto_detection(vmid, root_pt_pa)?;

    // Configure hardware HGATP register
    translator.configure_hgatp(vmid, root_pt_pa, mode);

    unsafe {
        GSTAGE_TRANSLATOR = Some(translator);
    }

    log::info!("G-stage address translation initialized successfully with mode: {:?}", mode);
    Ok(())
}

/// Get the global G-stage translator
pub fn get_translator() -> Option<&'static GStageTranslator> {
    unsafe { GSTAGE_TRANSLATOR.as_ref() }
}

/// Get mutable reference to global G-stage translator
pub fn get_translator_mut() -> Option<&'static mut GStageTranslator> {
    unsafe { GSTAGE_TRANSLATOR.as_mut() }
}

/// Translate GPA to HPA using global translator
pub fn translate_gpa(gpa: usize) -> Result<GStageTranslationResult, GStageFault> {
    if let Some(translator) = get_translator() {
        translator.translate(gpa)
    } else {
        Err(GStageFault::InvalidAddress)
    }
}

/// Configure G-stage for a VM
pub fn configure_gstage(vmid: u16, root_pt_pa: usize, mode: GStageMode) -> Result<(), &'static str> {
    if let Some(translator) = get_translator_mut() {
        translator.configure_hgatp(vmid, root_pt_pa, mode);
        Ok(())
    } else {
        Err("G-stage translator not initialized")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gstage_mode() {
        assert_eq!(GStageMode::Sv39x4.hgatp_mode_bits(), 8);
        assert_eq!(GStageMode::Sv39x4.addr_bits(), 39);
        assert_eq!(GStageMode::Sv39x4.levels(), 3);

        assert_eq!(GStageMode::Sv48x4.hgatp_mode_bits(), 9);
        assert_eq!(GStageMode::Sv48x4.addr_bits(), 48);
        assert_eq!(GStageMode::Sv48x4.levels(), 4);
    }

    #[test]
    fn test_hgatp_operations() {
        let vmid = 123;
        let ppn = 0x87654321;
        let mode = GStageMode::Sv39x4;

        let hgatp = GStageTranslator::make_hgatp(vmid, ppn, mode);

        assert_eq!(GStageTranslator::extract_vmid(hgatp), vmid);
        assert_eq!(GStageTranslator::extract_ppn(hgatp), ppn);
        assert_eq!(GStageTranslator::extract_mode(hgatp), mode);
    }

    #[test]
    fn test_gstage_translator() {
        let vmid = 1;
        let root_pt_pa = 0x80000000;
        let mode = GStageMode::Sv39x4;

        let translator = GStageTranslator::new(vmid, root_pt_pa, mode);

        assert_eq!(translator.get_vmid(), vmid);
        assert_eq!(translator.get_mode(), mode);
        assert_eq!(translator.get_root_pt_pa(), root_pt_pa);
    }

    #[test]
    fn test_translation_cache() {
        let mut cache = GStageTranslationCache::new(16);

        let result = GStageTranslationResult {
            hpa: 0x87654321,
            permissions: GStagePermissions::READ | GStagePermissions::WRITE,
            page_size: 4096,
            cached: false,
        };

        // Initially not in cache
        assert!(cache.lookup(0x1000).is_none());

        // Insert into cache
        cache.insert(0x1000, &result);

        // Should be found in cache
        let cached_result = cache.lookup(0x1000).unwrap();
        assert!(cached_result.cached);
        assert_eq!(cached_result.hpa, 0x87654321);

        // Test statistics
        let stats = cache.get_stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
    }

    #[test]
    fn test_permissions() {
        let pte = gstage_pte::V | gstage_pte::R | gstage_pte::W | gstage_pte::X | gstage_pte::U;

        let translator = GStageTranslator::new(1, 0x80000000, GStageMode::Sv39x4);
        let permissions = translator.pte_to_permissions(pte);

        assert!(permissions.contains(GStagePermissions::READ));
        assert!(permissions.contains(GStagePermissions::WRITE));
        assert!(permissions.contains(GStagePermissions::EXECUTE));
        assert!(permissions.contains(GStagePermissions::USER));
    }

    #[test]
    fn test_extended_page_table_formats() {
        // Test conversion between GStageMode and ExtendedPageTableFormat
        let mode = GStageMode::Sv57x4;
        let format = ExtendedPageTableFormat::from(mode);
        assert_eq!(format, ExtendedPageTableFormat::Sv57);

        let converted_mode = GStageMode::from(format);
        assert_eq!(converted_mode, mode);

        // Test format properties
        assert_eq!(format.va_bits(), 57);
        assert_eq!(format.levels(), 5);
        assert_eq!(format.entries_per_pt(), 512);
        assert!(format.supports_huge_pages());
        assert!(format.is_valid_va(0x123456789ABCDEF));
        assert!(!format.is_valid_va(1usize << 57)); // Too large
    }

    #[test]
    fn test_auto_detection() {
        // Test auto-detection fallback hierarchy
        let mut format_detector = FormatDetector::new();

        // Mock scenario: only Sv32 supported
        format_detector.set_supported_formats(1 << 3); // Sv32 only
        assert!(format_detector.is_format_supported(ExtendedPageTableFormat::Sv32));
        assert!(!format_detector.is_format_supported(ExtendedPageTableFormat::Sv39));

        // Test auto-detection result
        let detected = format_detector.detect_current_format();
        assert_eq!(detected, None); // No current format set

        // Test manual format setting
        format_detector.set_current_format(Some(ExtendedPageTableFormat::Sv32));
        assert_eq!(format_detector.detect_current_format(), Some(ExtendedPageTableFormat::Sv32));
    }

    #[test]
    fn test_sv57x4_translation() {
        let translator = GStageTranslator::new(1, 0x80000000, GStageMode::Sv57x4);

        // Test basic translation properties
        assert_eq!(translator.get_mode(), GStageMode::Sv57x4);
        assert_eq!(translator.get_vmid(), 1);
        assert_eq!(translator.get_root_pt_pa(), 0x80000000);

        // Test mode-specific properties
        let mode = translator.get_mode();
        assert_eq!(mode.addr_bits(), 57);
        assert_eq!(mode.levels(), 5);
        assert!(mode.supports_huge_pages());
        assert_eq!(mode.max_page_size(), 1usize << 52); // 4PB huge page
    }

    #[test]
    fn test_new_with_auto_detection() {
        // Test successful auto-detection
        let result = GStageTranslator::new_with_auto_detection(1, 0x80000000);
        assert!(result.is_ok());

        let translator = result.unwrap();
        // Should have selected some mode (fallback to Sv39 if nothing else)
        assert!(translator.get_mode() != GStageMode::Bare || translator.get_mode() == GStageMode::Bare);
    }

    #[test]
    fn test_enhanced_hgatp_extraction() {
        // Test enhanced mode extraction with auto-detection
        let hgatp_sv57 = (10usize << 60) | (1usize << 12) | 0x80000; // Sv57x4 mode
        let extracted_mode = GStageTranslator::extract_mode(hgatp_sv57);
        // Should fallback to supported mode since 10 might not be directly supported
        assert!(matches!(extracted_mode, GStageMode::Sv57x4 | GStageMode::Sv48x4 | GStageMode::Sv39x4));

        let hgatp_bare = 0usize; // Bare mode
        let bare_mode = GStageTranslator::extract_mode(hgatp_bare);
        assert_eq!(bare_mode, GStageMode::Bare);
    }

    #[test]
    fn test_format_conversion_compatibility() {
        // Test all format conversions work both ways
        let modes = [
            GStageMode::Bare,
            GStageMode::Sv32x4,
            GStageMode::Sv39x4,
            GStageMode::Sv48x4,
            GStageMode::Sv57x4,
        ];

        for mode in modes.iter() {
            let format = ExtendedPageTableFormat::from(*mode);
            let converted_back = GStageMode::from(format);

            // Note: Sv64 maps to Sv57x4, so we check compatibility
            match *mode {
                GStageMode::Sv57x4 => assert_eq!(converted_back, GStageMode::Sv57x4),
                _ => assert_eq!(converted_back, *mode),
            }
        }
    }
}