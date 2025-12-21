//! RISC-V TLB Management Module
//!
//! This module provides comprehensive Translation Lookaside Buffer (TLB) management
//! for RISC-V virtualization including:
//! - Software TLB structure and management
//! - TLB lookup and update operations
//! - TLB invalidation mechanisms
//! - Performance optimizations for virtualization
//! - Support for both G-stage and regular translation

use crate::arch::riscv64::*;
use crate::arch::riscv64::cpu::csr::*;
use bitflags::bitflags;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// TLB entry types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlbEntryType {
    /// Regular translation (VA → PA)
    Regular,
    /// G-stage translation (GPA → HPA)
    GStage,
    /// Nested translation (GVA → HPA)
    Nested,
}

/// TLB access permissions
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TlbPermissions: u8 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const EXECUTE = 1 << 2;
        const USER = 1 << 3;
        const GLOBAL = 1 << 4;
        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
        const VALID = 1 << 7;
    }
}

/// TLB entry structure
#[derive(Debug, Clone)]
pub struct TlbEntry {
    /// Virtual address (or Guest Physical for G-stage)
    pub vaddr: usize,
    /// Physical address (or Host Physical for G-stage)
    pub paddr: usize,
    /// Address space identifier (ASID)
    pub asid: u16,
    /// Virtual machine identifier (VMID)
    pub vmid: u16,
    /// Page size in bytes
    pub page_size: usize,
    /// Entry permissions
    pub permissions: TlbPermissions,
    /// Entry type
    pub entry_type: TlbEntryType,
    /// Last access time (timestamp)
    pub last_access: u64,
    /// Access count for LRU tracking
    pub access_count: u64,
    /// Entry creation time
    pub creation_time: u64,
    /// Translation level (for multi-level page tables)
    pub level: u8,
}

impl TlbEntry {
    /// Create a new TLB entry
    pub fn new(
        vaddr: usize,
        paddr: usize,
        asid: u16,
        vmid: u16,
        page_size: usize,
        permissions: TlbPermissions,
        entry_type: TlbEntryType,
        level: u8,
    ) -> Self {
        let current_time = Self::get_timestamp();

        Self {
            vaddr,
            paddr,
            asid,
            vmid,
            page_size,
            permissions,
            entry_type,
            last_access: current_time,
            access_count: 0,
            creation_time: current_time,
            level,
        }
    }

    /// Get current timestamp (simplified implementation)
    fn get_timestamp() -> u64 {
        // In a real implementation, this would read from a hardware timer
        // For now, use a simple atomic counter
        static TIMESTAMP: AtomicU64 = AtomicU64::new(0);
        TIMESTAMP.fetch_add(1, Ordering::Relaxed)
    }

    /// Update access information
    pub fn update_access(&mut self) {
        self.last_access = Self::get_timestamp();
        self.access_count += 1;
        self.permissions.insert(TlbPermissions::ACCESSED);
    }

    /// Mark entry as dirty
    pub fn mark_dirty(&mut self) {
        self.permissions.insert(TlbPermissions::DIRTY);
    }

    /// Check if entry is valid for given address
    pub fn matches(&self, vaddr: usize, asid: u16, vmid: u16) -> bool {
        if !self.permissions.contains(TlbPermissions::VALID) {
            return false;
        }

        let page_mask = self.page_size - 1;
        let page_aligned_vaddr = vaddr & !page_mask;
        let page_aligned_entry = self.vaddr & !page_mask;

        page_aligned_vaddr == page_aligned_entry && self.asid == asid && self.vmid == vmid
    }

    /// Translate virtual address to physical address
    pub fn translate(&self, vaddr: usize) -> usize {
        let page_mask = self.page_size - 1;
        let page_offset = vaddr & page_mask;
        let page_aligned_paddr = self.paddr & !page_mask;
        page_aligned_paddr | page_offset
    }

    /// Check if address range overlaps with this entry
    pub fn overlaps(&self, addr: usize, size: usize) -> bool {
        let page_mask = self.page_size - 1;
        let entry_start = self.vaddr & !page_mask;
        let entry_end = entry_start + self.page_size;
        let query_end = addr + size;

        addr < entry_end && query_end > entry_start
    }
}

/// Software TLB implementation
pub struct SoftwareTlb {
    /// TLB entries (associative cache)
    entries: Vec<Option<TlbEntry>>,
    /// Number of sets in the TLB
    num_sets: usize,
    /// Number of ways per set (associativity)
    ways: usize,
    /// Current timestamp for aging
    current_time: AtomicU64,
    /// Statistics
    stats: TlbStats,
    /// LRU tracking
    lru_tracker: Vec<Vec<usize>>,
}

/// TLB statistics
#[derive(Debug, Default)]
pub struct TlbStats {
    /// Total lookups
    pub lookups: AtomicUsize,
    /// Total hits
    pub hits: AtomicUsize,
    /// Total misses
    pub misses: AtomicUsize,
    /// Total invalidations
    pub invalidations: AtomicUsize,
    /// Total flushes
    pub flushes: AtomicUsize,
}

impl TlbStats {
    /// Get hit rate as percentage
    pub fn hit_rate(&self) -> f64 {
        let total_lookups = self.lookups.load(Ordering::Relaxed);
        if total_lookups == 0 {
            0.0
        } else {
            let hits = self.hits.load(Ordering::Relaxed);
            (hits as f64 / total_lookups as f64) * 100.0
        }
    }
}

impl SoftwareTlb {
    /// Create a new software TLB
    pub fn new(num_sets: usize, ways: usize) -> Self {
        let total_entries = num_sets * ways;
        let mut entries = Vec::with_capacity(total_entries);
        entries.resize(total_entries, None);

        let mut lru_tracker = Vec::with_capacity(num_sets);
        for _ in 0..num_sets {
            let mut set_lru = Vec::with_capacity(ways);
            for i in 0..ways {
                set_lru.push(i);
            }
            lru_tracker.push(set_lru);
        }

        Self {
            entries,
            num_sets,
            ways,
            current_time: AtomicU64::new(0),
            stats: TlbStats::default(),
            lru_tracker,
        }
    }

    /// Calculate set index for an address
    fn get_set_index(&self, vaddr: usize) -> usize {
        // Use hash of virtual address for better distribution
        let hash = ((vaddr >> 12) as u64).wrapping_mul(11400714819323198485u64);
        (hash as usize) % self.num_sets
    }

    /// Get entry index by set and way
    fn get_entry_index(&self, set: usize, way: usize) -> usize {
        set * self.ways + way
    }

    /// Find matching entry in a set
    fn find_matching_entry(&self, set: usize, vaddr: usize, asid: u16, vmid: u16) -> Option<usize> {
        for way in 0..self.ways {
            let entry_idx = self.get_entry_index(set, way);
            if let Some(ref entry) = self.entries[entry_idx] {
                if entry.matches(vaddr, asid, vmid) {
                    return Some(way);
                }
            }
        }
        None
    }

    /// Update LRU tracking
    fn update_lru(&mut self, set: usize, way: usize) {
        let set_lru = &mut self.lru_tracker[set];

        // Remove the accessed way from its current position
        set_lru.retain(|&w| w != way);

        // Move it to the end (most recently used)
        set_lru.push(way);
    }

    /// Get LRU way index for replacement
    fn get_lru_way(&self, set: usize) -> usize {
        self.lru_tracker[set][0] // First element is least recently used
    }

    /// Lookup address in TLB
    pub fn lookup(&mut self, vaddr: usize, asid: u16, vmid: u16) -> Option<TlbEntry> {
        self.stats.lookups.fetch_add(1, Ordering::Relaxed);

        let set = self.get_set_index(vaddr);

        if let Some(way) = self.find_matching_entry(set, vaddr, asid, vmid) {
            let entry_idx = self.get_entry_index(set, way);

            // Update access information
            if let Some(ref mut entry) = self.entries[entry_idx] {
                entry.update_access();
                self.update_lru(set, way);

                self.stats.hits.fetch_add(1, Ordering::Relaxed);
                return Some(entry.clone());
            }
        }

        self.stats.misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Insert new entry into TLB
    pub fn insert(&mut self, entry: TlbEntry) {
        let set = self.get_set_index(entry.vaddr);

        // Check if there's already a matching entry to update
        if let Some(way) = self.find_matching_entry(set, entry.vaddr, entry.asid, entry.vmid) {
            let entry_idx = self.get_entry_index(set, way);
            self.entries[entry_idx] = Some(entry);
            self.update_lru(set, way);
            return;
        }

        // Find LRU way to replace
        let lru_way = self.get_lru_way(set);
        let entry_idx = self.get_entry_index(set, lru_way);

        // Insert new entry
        self.entries[entry_idx] = Some(entry);
        self.update_lru(set, lru_way);
    }

    /// Invalidate specific entry
    pub fn invalidate_entry(&mut self, vaddr: usize, asid: u16, vmid: u16) -> bool {
        let set = self.get_set_index(vaddr);

        if let Some(way) = self.find_matching_entry(set, vaddr, asid, vmid) {
            let entry_idx = self.get_entry_index(set, way);
            self.entries[entry_idx] = None;
            self.stats.invalidations.fetch_add(1, Ordering::Relaxed);

            // Update LRU tracking
            let set_lru = &mut self.lru_tracker[set];
            set_lru.retain(|&w| w != way);
            set_lru.push(way);

            true
        } else {
            false
        }
    }

    /// Invalidate entries by ASID
    pub fn invalidate_asid(&mut self, asid: u16) -> usize {
        let mut count = 0;

        for set in 0..self.num_sets {
            for way in 0..self.ways {
                let entry_idx = self.get_entry_index(set, way);
                if let Some(ref entry) = self.entries[entry_idx] {
                    if entry.asid == asid {
                        self.entries[entry_idx] = None;
                        count += 1;

                        // Update LRU tracking
                        let set_lru = &mut self.lru_tracker[set];
                        set_lru.retain(|&w| w != way);
                        set_lru.push(way);
                    }
                }
            }
        }

        self.stats.invalidations.fetch_add(count, Ordering::Relaxed);
        count
    }

    /// Invalidate entries by VMID (for virtualization)
    pub fn invalidate_vmid(&mut self, vmid: u16) -> usize {
        let mut count = 0;

        for set in 0..self.num_sets {
            for way in 0..self.ways {
                let entry_idx = self.get_entry_index(set, way);
                if let Some(ref entry) = self.entries[entry_idx] {
                    if entry.vmid == vmid {
                        self.entries[entry_idx] = None;
                        count += 1;

                        // Update LRU tracking
                        let set_lru = &mut self.lru_tracker[set];
                        set_lru.retain(|&w| w != way);
                        set_lru.push(way);
                    }
                }
            }
        }

        self.stats.invalidations.fetch_add(count, Ordering::Relaxed);
        count
    }

    /// Invalidate entries in address range
    pub fn invalidate_range(&mut self, start_addr: usize, size: usize, asid: u16, vmid: u16) -> usize {
        let mut count = 0;
        let end_addr = start_addr + size;

        for set in 0..self.num_sets {
            for way in 0..self.ways {
                let entry_idx = self.get_entry_index(set, way);
                if let Some(ref entry) = self.entries[entry_idx] {
                    if entry.asid == asid && entry.vmid == vmid && entry.overlaps(start_addr, size) {
                        self.entries[entry_idx] = None;
                        count += 1;

                        // Update LRU tracking
                        let set_lru = &mut self.lru_tracker[set];
                        set_lru.retain(|&w| w != way);
                        set_lru.push(way);
                    }
                }
            }
        }

        self.stats.invalidations.fetch_add(count, Ordering::Relaxed);
        count
    }

    /// Flush entire TLB
    pub fn flush_all(&mut self) {
        for entry in &mut self.entries {
            *entry = None;
        }

        // Reset LRU trackers
        for set_lru in &mut self.lru_tracker {
            set_lru.clear();
            for i in 0..self.ways {
                set_lru.push(i);
            }
        }

        self.stats.flushes.fetch_add(1, Ordering::Relaxed);
        log::debug!("Flushed entire software TLB");
    }

    /// Flush entries by type
    pub fn flush_by_type(&mut self, entry_type: TlbEntryType) -> usize {
        let mut count = 0;

        for set in 0..self.num_sets {
            for way in 0..self.ways {
                let entry_idx = self.get_entry_index(set, way);
                if let Some(ref entry) = self.entries[entry_idx] {
                    if entry.entry_type == entry_type {
                        self.entries[entry_idx] = None;
                        count += 1;

                        // Update LRU tracking
                        let set_lru = &mut self.lru_tracker[set];
                        set_lru.retain(|&w| w != way);
                        set_lru.push(way);
                    }
                }
            }
        }

        self.stats.invalidations.fetch_add(count, Ordering::Relaxed);
        count
    }

    /// Get TLB statistics
    pub fn get_stats(&self) -> &TlbStats {
        &self.stats
    }

    /// Get number of valid entries
    pub fn valid_entries(&self) -> usize {
        self.entries.iter().filter(|e| e.is_some()).count()
    }

    /// Get entries by VMID for debugging
    pub fn get_entries_by_vmid(&self, vmid: u16) -> Vec<&TlbEntry> {
        self.entries.iter()
            .filter_map(|e| e.as_ref())
            .filter(|e| e.vmid == vmid)
            .collect()
    }

    /// Optimize TLB performance by aging out old entries
    pub fn optimize_performance(&mut self, age_threshold: u64) -> usize {
        let mut count = 0;
        let current_time = Self::get_timestamp();

        for set in 0..self.num_sets {
            for way in 0..self.ways {
                let entry_idx = self.get_entry_index(set, way);
                if let Some(ref entry) = self.entries[entry_idx] {
                    let age = current_time - entry.last_access;
                    if age > age_threshold {
                        self.entries[entry_idx] = None;
                        count += 1;

                        // Update LRU tracking
                        let set_lru = &mut self.lru_tracker[set];
                        set_lru.retain(|&w| w != way);
                        set_lru.push(way);
                    }
                }
            }
        }

        if count > 0 {
            log::debug!("Aged out {} TLB entries", count);
        }

        count
    }

    /// Print TLB state for debugging
    pub fn dump_state(&self) {
        log::info!("=== TLB State Dump ===");
        log::info!("Total entries: {}", self.valid_entries());
        log::info!("Hit rate: {:.2}%", self.stats.hit_rate());
        log::info!("Lookups: {}, Hits: {}, Misses: {}",
                  self.stats.lookups.load(Ordering::Relaxed),
                  self.stats.hits.load(Ordering::Relaxed),
                  self.stats.misses.load(Ordering::Relaxed));

        for set in 0..self.num_sets.min(4) { // Limit output to first 4 sets
            log::info!("Set {}:", set);
            for way in 0..self.ways {
                let entry_idx = self.get_entry_index(set, way);
                if let Some(ref entry) = self.entries[entry_idx] {
                    log::info!("  Way {}: VA={:#x}, PA={:#x}, ASID={}, VMID={}, Access={}",
                              way, entry.vaddr, entry.paddr, entry.asid, entry.vmid, entry.access_count);
                } else {
                    log::info!("  Way {}: <empty>", way);
                }
            }
        }
    }
}

/// Hardware TLB management utilities
pub struct HardwareTlb;

impl HardwareTlb {
    /// Flush hardware TLB entries
    pub fn flush_all() {
        unsafe {
            // Use SFENCE.VMA to flush all TLB entries
            core::arch::asm!("sfence.vma");
        }
        log::debug!("Flushed hardware TLB");
    }

    /// Flush TLB entries for specific ASID
    pub fn flush_asid(asid: u16) {
        unsafe {
            core::arch::asm!(
                "sfence.vma x0, {}",
                in(reg) asid,
            );
        }
        log::debug!("Flushed hardware TLB for ASID {}", asid);
    }

    /// Flush TLB entries for specific address and ASID
    pub fn flush_addr(vaddr: usize, asid: u16) {
        unsafe {
            core::arch::asm!(
                "sfence.vma {}, {}",
                in(reg) vaddr,
                in(reg) asid,
            );
        }
        log::debug!("Flushed hardware TLB for VA {:#x}, ASID {}", vaddr, asid);
    }

    /// Flush G-stage TLB entries
    pub fn flush_gstage_all() {
        unsafe {
            // Use HFENCE.GVMA to flush all G-stage TLB entries
            core::arch::asm!("hfence.gvma");
        }
        log::debug!("Flushed G-stage hardware TLB");
    }

    /// Flush G-stage TLB entries for specific VMID
    pub fn flush_gstage_vmid(vmid: u16) {
        unsafe {
            core::arch::asm!(
                "hfence.gvma x0, {}",
                in(reg) vmid,
            );
        }
        log::debug!("Flushed G-stage hardware TLB for VMID {}", vmid);
    }

    /// Flush G-stage TLB entries for specific GPA and VMID
    pub fn flush_gstage_addr(gpa: usize, vmid: u16) {
        unsafe {
            core::arch::asm!(
                "hfence.gvma {}, {}",
                in(reg) gpa,
                in(reg) vmid,
            );
        }
        log::debug!("Flushed G-stage hardware TLB for GPA {:#x}, VMID {}", gpa, vmid);
    }
}

/// Global TLB manager
pub struct TlbManager {
    /// Regular translation TLB
    pub regular_tlb: SoftwareTlb,
    /// G-stage translation TLB
    pub gstage_tlb: SoftwareTlb,
    /// Hardware TLB management
    pub hardware_tlb: HardwareTlb,
}

impl TlbManager {
    /// Create new TLB manager
    pub fn new() -> Self {
        Self {
            regular_tlb: SoftwareTlb::new(64, 4),  // 64 sets, 4-way associative
            gstage_tlb: SoftwareTlb::new(32, 8),   // 32 sets, 8-way associative
            hardware_tlb: HardwareTlb,
        }
    }

    /// Perform address translation with TLB lookup
    pub fn translate_regular(&mut self, vaddr: usize, asid: u16, vmid: u16) -> Option<usize> {
        // Try software TLB first
        if let Some(entry) = self.regular_tlb.lookup(vaddr, asid, vmid) {
            return Some(entry.translate(vaddr));
        }

        // TLB miss - would trigger page walk in real implementation
        None
    }

    /// Perform G-stage translation with TLB lookup
    pub fn translate_gstage(&mut self, gpa: usize, asid: u16, vmid: u16) -> Option<usize> {
        // Try G-stage software TLB first
        if let Some(entry) = self.gstage_tlb.lookup(gpa, asid, vmid) {
            return Some(entry.translate(gpa));
        }

        // TLB miss - would trigger G-stage page walk in real implementation
        None
    }

    /// Insert regular translation entry
    pub fn insert_regular(&mut self, entry: TlbEntry) {
        self.regular_tlb.insert(entry);
    }

    /// Insert G-stage translation entry
    pub fn insert_gstage(&mut self, entry: TlbEntry) {
        self.gstage_tlb.insert(entry);
    }

    /// Invalidate by VMID (flushes both regular and G-stage)
    pub fn invalidate_vmid(&mut self, vmid: u16) -> usize {
        let regular_count = self.regular_tlb.invalidate_vmid(vmid);
        let gstage_count = self.gstage_tlb.invalidate_vmid(vmid);
        let total = regular_count + gstage_count;

        if total > 0 {
            // Also flush hardware TLBs
            self.hardware_tlb.flush_asid(vmid);
            self.hardware_tlb.flush_gstage_vmid(vmid);
        }

        total
    }

    /// Flush all TLBs
    pub fn flush_all(&mut self) {
        self.regular_tlb.flush_all();
        self.gstage_tlb.flush_all();
        self.hardware_tlb.flush_all();
        self.hardware_tlb.flush_gstage_all();
    }

    /// Get combined statistics
    pub fn get_combined_stats(&self) -> TlbCombinedStats {
        TlbCombinedStats {
            regular_hits: self.regular_tlb.get_stats().hits.load(Ordering::Relaxed),
            regular_misses: self.regular_tlb.get_stats().misses.load(Ordering::Relaxed),
            gstage_hits: self.gstage_tlb.get_stats().hits.load(Ordering::Relaxed),
            gstage_misses: self.gstage_tlb.get_stats().misses.load(Ordering::Relaxed),
            regular_hit_rate: self.regular_tlb.get_stats().hit_rate(),
            gstage_hit_rate: self.gstage_tlb.get_stats().hit_rate(),
        }
    }
}

/// Combined TLB statistics
#[derive(Debug)]
pub struct TlbCombinedStats {
    pub regular_hits: usize,
    pub regular_misses: usize,
    pub gstage_hits: usize,
    pub gstage_misses: usize,
    pub regular_hit_rate: f64,
    pub gstage_hit_rate: f64,
}

/// Global TLB manager instance
static mut TLB_MANAGER: Option<TlbManager> = None;

/// Initialize TLB manager
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing RISC-V TLB management");

    let manager = TlbManager::new();

    unsafe {
        TLB_MANAGER = Some(manager);
    }

    log::info!("RISC-V TLB management initialized successfully");
    Ok(())
}

/// Get global TLB manager
pub fn get_manager() -> Option<&'static TlbManager> {
    unsafe { TLB_MANAGER.as_ref() }
}

/// Get mutable global TLB manager
pub fn get_manager_mut() -> Option<&'static mut TlbManager> {
    unsafe { TLB_MANAGER.as_mut() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tlb_entry_creation() {
        let permissions = TlbPermissions::READ | TlbPermissions::WRITE | TlbPermissions::VALID;
        let entry = TlbEntry::new(
            0x80000000,
            0x90000000,
            1,
            100,
            4096,
            permissions,
            TlbEntryType::Regular,
            2,
        );

        assert_eq!(entry.vaddr, 0x80000000);
        assert_eq!(entry.paddr, 0x90000000);
        assert_eq!(entry.asid, 1);
        assert_eq!(entry.vmid, 100);
        assert_eq!(entry.page_size, 4096);
        assert!(entry.permissions.contains(TlbPermissions::VALID));
    }

    #[test]
    fn test_tlb_entry_translation() {
        let entry = TlbEntry::new(
            0x80001000,
            0x90002000,
            1,
            100,
            4096,
            TlbPermissions::VALID,
            TlbEntryType::Regular,
            0,
        );

        // Test page-aligned translation
        let translated = entry.translate(0x80001000);
        assert_eq!(translated, 0x90002000);

        // Test translation with offset
        let translated = entry.translate(0x80001234);
        assert_eq!(translated, 0x90002234);
    }

    #[test]
    fn test_tlb_entry_matching() {
        let entry = TlbEntry::new(
            0x80000000,
            0x90000000,
            1,
            100,
            4096,
            TlbPermissions::VALID,
            TlbEntryType::Regular,
            0,
        );

        // Exact match
        assert!(entry.matches(0x80000000, 1, 100));

        // Page-aligned match
        assert!(entry.matches(0x80001000, 1, 100));

        // Different ASID
        assert!(!entry.matches(0x80000000, 2, 100));

        // Different VMID
        assert!(!entry.matches(0x80000000, 1, 101));
    }

    #[test]
    fn test_software_tlb() {
        let mut tlb = SoftwareTlb::new(4, 2); // 4 sets, 2-way

        // Insert entry
        let permissions = TlbPermissions::READ | TlbPermissions::WRITE | TlbPermissions::VALID;
        let entry = TlbEntry::new(
            0x80000000,
            0x90000000,
            1,
            100,
            4096,
            permissions,
            TlbEntryType::Regular,
            0,
        );
        tlb.insert(entry);

        // Lookup should succeed
        let found = tlb.lookup(0x80000000, 1, 100);
        assert!(found.is_some());
        assert_eq!(found.unwrap().paddr, 0x90000000);

        // Check statistics
        let stats = tlb.get_stats();
        assert_eq!(stats.lookups.load(Ordering::Relaxed), 1);
        assert_eq!(stats.hits.load(Ordering::Relaxed), 1);
        assert_eq!(stats.misses.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_tlb_invalidation() {
        let mut tlb = SoftwareTlb::new(4, 2);

        // Insert multiple entries with different ASIDs
        let entry1 = TlbEntry::new(
            0x80000000, 0x90000000, 1, 100, 4096,
            TlbPermissions::VALID, TlbEntryType::Regular, 0,
        );
        let entry2 = TlbEntry::new(
            0x80100000, 0x90100000, 2, 100, 4096,
            TlbPermissions::VALID, TlbEntryType::Regular, 0,
        );

        tlb.insert(entry1);
        tlb.insert(entry2);

        // Both should be found initially
        assert!(tlb.lookup(0x80000000, 1, 100).is_some());
        assert!(tlb.lookup(0x80100000, 2, 100).is_some());

        // Invalidate ASID 1
        let count = tlb.invalidate_asid(1);
        assert_eq!(count, 1);

        // Only entry2 should remain
        assert!(tlb.lookup(0x80000000, 1, 100).is_none());
        assert!(tlb.lookup(0x80100000, 2, 100).is_some());
    }

    #[test]
    fn test_tlb_manager() {
        let mut manager = TlbManager::new();

        // Insert regular translation
        let entry = TlbEntry::new(
            0x80000000, 0x90000000, 1, 100, 4096,
            TlbPermissions::READ | TlbPermissions::WRITE | TlbPermissions::VALID,
            TlbEntryType::Regular, 0,
        );
        manager.insert_regular(entry);

        // Lookup should succeed
        let translated = manager.translate_regular(0x80000000, 1, 100);
        assert!(translated.is_some());
        assert_eq!(translated.unwrap(), 0x90000000);

        // Get combined stats
        let stats = manager.get_combined_stats();
        assert_eq!(stats.regular_hits, 1);
        assert_eq!(stats.regular_misses, 0);
        assert!(stats.regular_hit_rate > 0.0);
    }

    #[test]
    fn test_tlb_lru_replacement() {
        let mut tlb = SoftwareTlb::new(2, 2); // 2 sets, 2-way

        // Fill first set with 2 entries
        let entry1 = TlbEntry::new(
            0x1000, 0x2000, 1, 100, 4096,
            TlbPermissions::VALID, TlbEntryType::Regular, 0,
        );
        let entry2 = TlbEntry::new(
            0x2000, 0x3000, 1, 100, 4096,
            TlbPermissions::VALID, TlbEntryType::Regular, 0,
        );

        tlb.insert(entry1);
        tlb.insert(entry2);

        // Both should be found
        assert!(tlb.lookup(0x1000, 1, 100).is_some());
        assert!(tlb.lookup(0x2000, 1, 100).is_some());

        // Insert third entry that should replace LRU
        let entry3 = TlbEntry::new(
            0x3000, 0x4000, 1, 100, 4096,
            TlbPermissions::VALID, TlbEntryType::Regular, 0,
        );
        tlb.insert(entry3);

        // entry1 should be replaced, entry2 should remain
        assert!(tlb.lookup(0x1000, 1, 100).is_none());
        assert!(tlb.lookup(0x2000, 1, 100).is_some());
        assert!(tlb.lookup(0x3000, 1, 100).is_some());
    }

    #[test]
    fn test_tlb_performance_optimization() {
        let mut tlb = SoftwareTlb::new(4, 2);

        // Insert old entries
        let old_entry = TlbEntry::new(
            0x1000, 0x2000, 1, 100, 4096,
            TlbPermissions::VALID, TlbEntryType::Regular, 0,
        );

        // Manually set old timestamp
        let mut entry_with_old_time = old_entry.clone();
        entry_with_old_time.last_access = 0; // Very old
        tlb.insert(entry_with_old_time);

        // Optimize with age threshold
        let aged_out = tlb.optimize_performance(100); // Age out entries older than 100
        assert!(aged_out > 0);

        // Verify old entry was removed
        assert!(tlb.lookup(0x1000, 1, 100).is_none());
    }

    #[test]
    fn test_tlb_range_invalidation() {
        let mut tlb = SoftwareTlb::new(4, 2);

        // Insert multiple entries
        let entry1 = TlbEntry::new(
            0x1000, 0x2000, 1, 100, 4096,
            TlbPermissions::VALID, TlbEntryType::Regular, 0,
        );
        let entry2 = TlbEntry::new(
            0x2000, 0x3000, 1, 100, 4096,
            TlbPermissions::VALID, TlbEntryType::Regular, 0,
        );
        let entry3 = TlbEntry::new(
            0x5000, 0x6000, 1, 100, 4096,
            TlbPermissions::VALID, TlbEntryType::Regular, 0,
        );

        tlb.insert(entry1);
        tlb.insert(entry2);
        tlb.insert(entry3);

        // Invalidate range covering first two entries
        let invalidated = tlb.invalidate_range(0x1000, 0x2000, 1, 100);
        assert_eq!(invalidated, 2);

        // Verify entries in range are gone
        assert!(tlb.lookup(0x1000, 1, 100).is_none());
        assert!(tlb.lookup(0x2000, 1, 100).is_none());

        // Verify entry outside range remains
        assert!(tlb.lookup(0x5000, 1, 100).is_some());
    }

    #[test]
    fn test_tlb_entry_access_tracking() {
        let mut entry = TlbEntry::new(
            0x80000000, 0x90000000, 1, 100, 4096,
            TlbPermissions::READ | TlbPermissions::WRITE | TlbPermissions::VALID,
            TlbEntryType::Regular, 0,
        );

        // Initial state
        assert_eq!(entry.access_count, 0);
        assert!(entry.permissions.contains(TlbPermissions::VALID));
        assert!(!entry.permissions.contains(TlbPermissions::ACCESSED));

        // Update access
        entry.update_access();
        assert_eq!(entry.access_count, 1);
        assert!(entry.permissions.contains(TlbPermissions::ACCESSED));

        // Mark dirty
        entry.mark_dirty();
        assert!(entry.permissions.contains(TlbPermissions::DIRTY));
    }

    #[test]
    fn test_hardware_tlb_flushes() {
        // These tests would require actual hardware or emulator
        // For now, we just verify the functions compile
        HardwareTlb::flush_all();
        HardwareTlb::flush_asid(1);
        HardwareTlb::flush_addr(0x1000, 1);
        HardwareTlb::flush_gstage_all();
        HardwareTlb::flush_gstage_vmid(1);
        HardwareTlb::flush_gstage_addr(0x2000, 1);
    }

    #[test]
    fn test_tlb_statistics() {
        let mut stats = TlbStats::default();

        // Update statistics
        stats.lookups.store(100, Ordering::Relaxed);
        stats.hits.store(75, Ordering::Relaxed);
        stats.misses.store(25, Ordering::Relaxed);

        // Check hit rate
        assert_eq!(stats.hit_rate(), 75.0);

        // Edge case: zero lookups
        stats.lookups.store(0, Ordering::Relaxed);
        assert_eq!(stats.hit_rate(), 0.0);
    }

    #[test]
    fn test_gstage_tlb_integration() {
        let mut manager = TlbManager::new();

        // Insert G-stage translation
        let gstage_entry = TlbEntry::new(
            0x80000000, 0x90000000, 0, 100, 4096,
            TlbPermissions::READ | TlbPermissions::WRITE | TlbPermissions::VALID,
            TlbEntryType::GStage, 0,
        );
        manager.insert_gstage(gstage_entry);

        // Lookup should succeed
        let translated = manager.translate_gstage(0x80000000, 0, 100);
        assert!(translated.is_some());
        assert_eq!(translated.unwrap(), 0x90000000);

        // Invalidate by VMID
        let count = manager.invalidate_vmid(100);
        assert!(count > 0);

        // Lookup should now fail
        let translated = manager.translate_gstage(0x80000000, 0, 100);
        assert!(translated.is_none());
    }
}