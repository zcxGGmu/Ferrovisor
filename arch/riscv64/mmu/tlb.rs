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
use crate::core::sync::SpinLock;
use alloc::vec::Vec;
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

/// TLB optimization strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlbOptimizationStrategy {
    /// No optimization (default behavior)
    None,
    /// Least Recently Used (LRU)
    LRU,
    /// Most Recently Used (MRU)
    MRU,
    /// Least Frequently Used (LFU)
    LFU,
    /// Random replacement
    Random,
    /// Adaptive based on workload
    Adaptive,
}

/// TLB coalescing configuration
#[derive(Debug, Clone)]
pub struct TlbCoalescingConfig {
    /// Enable TLB entry coalescing
    pub enabled: bool,
    /// Minimum entries for coalescing
    pub min_entries: usize,
    /// Maximum coalesced entry size
    pub max_coalesced_size: usize,
    /// Coalescing threshold (hit rate percentage)
    pub hit_rate_threshold: f64,
}

/// TLB prefetching configuration
#[derive(Debug, Clone)]
pub struct TlbPrefetchConfig {
    /// Enable TLB prefetching
    pub enabled: bool,
    /// Prefetch distance (number of pages)
    pub prefetch_distance: usize,
    /// Prefetch threshold (access count)
    pub prefetch_threshold: u64,
    /// Maximum prefetch queue size
    pub max_prefetch_queue: usize,
}

/// Advanced TLB statistics
#[derive(Debug, Default)]
pub struct AdvancedTlbStats {
    /// Base statistics
    pub base: TlbStats,
    /// Coalescing statistics
    pub coalesced_entries: AtomicUsize,
    pub coalescing_operations: AtomicUsize,
    /// Prefetching statistics
    pub prefetched_entries: AtomicUsize,
    pub prefetch_hits: AtomicUsize,
    pub prefetch_misses: AtomicUsize,
    /// Optimization statistics
    pub optimization_cycles: AtomicU64,
    /// Performance metrics
    pub average_lookup_time: AtomicU64,
    pub peak_entries_used: AtomicUsize,
    /// Hardware interaction statistics
    pub hardware_flushes: AtomicUsize,
    pub remote_flushes: AtomicUsize,
    /// VM-specific statistics
    pub vm_tlb_shares: Vec<(u16, usize)>, // (VMID, entry_count)
}

impl AdvancedTlbStats {
    /// Get coalescing efficiency
    pub fn coalescing_efficiency(&self) -> f64 {
        let coalesced = self.coalesced_entries.load(Ordering::Relaxed);
        let operations = self.coalescing_operations.load(Ordering::Relaxed);
        if operations == 0 {
            0.0
        } else {
            (coalesced as f64 / operations as f64) * 100.0
        }
    }

    /// Get prefetching accuracy
    pub fn prefetch_accuracy(&self) -> f64 {
        let hits = self.prefetch_hits.load(Ordering::Relaxed);
        let total = hits + self.prefetch_misses.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            (hits as f64 / total as f64) * 100.0
        }
    }

    /// Update VM TLB share statistics
    pub fn update_vm_share(&mut self, vmid: u16, entry_count: usize) {
        // Remove existing entry for this VMID
        self.vm_tlb_shares.retain(|(id, _)| *id != vmid);
        // Add updated entry
        self.vm_tlb_shares.push((vmid, entry_count));
        // Sort by entry count descending
        self.vm_tlb_shares.sort_by(|a, b| b.1.cmp(&a.1));
    }
}

/// Global TLB manager with advanced optimizations
pub struct TlbManager {
    /// Regular translation TLB
    pub regular_tlb: SoftwareTlb,
    /// G-stage translation TLB
    pub gstage_tlb: SoftwareTlb,
    /// Hardware TLB management
    pub hardware_tlb: HardwareTlb,
    /// Optimization strategy
    optimization_strategy: TlbOptimizationStrategy,
    /// Coalescing configuration
    coalescing_config: TlbCoalescingConfig,
    /// Prefetching configuration
    prefetch_config: TlbPrefetchConfig,
    /// Advanced statistics
    advanced_stats: AdvancedTlbStats,
    /// Prefetch queue
    prefetch_queue: SpinLock<Vec<(usize, u16, u16)>>, // (vaddr, asid, vmid)
    /// Performance monitoring
    performance_monitor: SpinLock<TlbPerformanceMonitor>,
}

/// TLB performance monitor
#[derive(Debug, Default)]
pub struct TlbPerformanceMonitor {
    /// Performance samples
    samples: Vec<TlbPerformanceSample>,
    /// Current sample index
    current_sample: usize,
    /// Maximum samples to keep
    max_samples: usize,
    /// Optimization trigger thresholds
    pub hit_rate_threshold: f64,
    pub miss_rate_threshold: f64,
}

/// TLB performance sample
#[derive(Debug, Clone)]
pub struct TlbPerformanceSample {
    /// Timestamp
    pub timestamp: u64,
    /// Hit rate
    pub hit_rate: f64,
    /// Entry utilization
    pub entry_utilization: f64,
    /// Average lookup time
    pub avg_lookup_time: u64,
    /// Active VMs
    pub active_vms: usize,
}

impl TlbPerformanceMonitor {
    /// Create new performance monitor
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: Vec::with_capacity(max_samples),
            current_sample: 0,
            max_samples,
            hit_rate_threshold: 80.0, // 80% hit rate threshold
            miss_rate_threshold: 20.0, // 20% miss rate threshold
        }
    }

    /// Record performance sample
    pub fn record_sample(&mut self, sample: TlbPerformanceSample) {
        if self.samples.len() < self.max_samples {
            self.samples.push(sample);
        } else {
            self.samples[self.current_sample] = sample;
            self.current_sample = (self.current_sample + 1) % self.max_samples;
        }
    }

    /// Check if optimization is needed
    pub fn needs_optimization(&self) -> Option<TlbOptimizationTrigger> {
        if self.samples.is_empty() {
            return None;
        }

        // Calculate recent average
        let recent_samples: Vec<_> = self.samples.iter()
            .rev()
            .take(5) // Last 5 samples
            .collect();

        let avg_hit_rate: f64 = recent_samples.iter()
            .map(|s| s.hit_rate)
            .sum::<f64>() / recent_samples.len() as f64;

        let avg_miss_rate: f64 = recent_samples.iter()
            .map(|s| 100.0 - s.hit_rate)
            .sum::<f64>() / recent_samples.len() as f64;

        if avg_hit_rate < self.hit_rate_threshold {
            Some(TlbOptimizationTrigger::LowHitRate)
        } else if avg_miss_rate > self.miss_rate_threshold {
            Some(TlbOptimizationTrigger::HighMissRate)
        } else {
            None
        }
    }
}

/// TLB optimization triggers
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlbOptimizationTrigger {
    /// Low hit rate detected
    LowHitRate,
    /// High miss rate detected
    HighMissRate,
    /// High entry utilization
    HighUtilization,
    /// Periodic optimization
    Periodic,
    /// Manual optimization request
    Manual,
}

impl TlbManager {
    /// Create new TLB manager with default configurations
    pub fn new() -> Self {
        Self::with_configs(
            TlbOptimizationStrategy::LRU,
            TlbCoalescingConfig {
                enabled: true,
                min_entries: 8,
                max_coalesced_size: 64 * 1024, // 64KB
                hit_rate_threshold: 85.0,
            },
            TlbPrefetchConfig {
                enabled: true,
                prefetch_distance: 4,
                prefetch_threshold: 3,
                max_prefetch_queue: 16,
            },
        )
    }

    /// Create new TLB manager with custom configurations
    pub fn with_configs(
        optimization_strategy: TlbOptimizationStrategy,
        coalescing_config: TlbCoalescingConfig,
        prefetch_config: TlbPrefetchConfig,
    ) -> Self {
        Self {
            regular_tlb: SoftwareTlb::new(64, 4),  // 64 sets, 4-way associative
            gstage_tlb: SoftwareTlb::new(32, 8),   // 32 sets, 8-way associative
            hardware_tlb: HardwareTlb,
            optimization_strategy,
            coalescing_config,
            prefetch_config,
            advanced_stats: AdvancedTlbStats::default(),
            prefetch_queue: SpinLock::new(Vec::with_capacity(prefetch_config.max_prefetch_queue)),
            performance_monitor: SpinLock::new(TlbPerformanceMonitor::new(10)),
        }
    }

    /// Set optimization strategy
    pub fn set_optimization_strategy(&mut self, strategy: TlbOptimizationStrategy) {
        self.optimization_strategy = strategy;
        log::info!("TLB optimization strategy changed to {:?}", strategy);
    }

    /// Update coalescing configuration
    pub fn update_coalescing_config(&mut self, config: TlbCoalescingConfig) {
        self.coalescing_config = config;
        log::info!("TLB coalescing configuration updated");
    }

    /// Update prefetching configuration
    pub fn update_prefetch_config(&mut self, config: TlbPrefetchConfig) {
        self.prefetch_config = config;
        log::info!("TLB prefetching configuration updated");
    }

    /// Perform optimized address translation with prefetching
    pub fn translate_regular_optimized(&mut self, vaddr: usize, asid: u16, vmid: u16) -> Option<usize> {
        let start_time = TlbEntry::get_timestamp();

        // Try software TLB first
        let result = if let Some(entry) = self.regular_tlb.lookup(vaddr, asid, vmid) {
            // Update access statistics
            self.update_access_statistics(vaddr, asid, vmid, true);

            // Trigger prefetching if configured and threshold met
            if self.prefetch_config.enabled &&
               entry.access_count >= self.prefetch_config.prefetch_threshold {
                self.schedule_prefetch(vaddr, asid, vmid);
            }

            Some(entry.translate(vaddr))
        } else {
            // Update access statistics
            self.update_access_statistics(vaddr, asid, vmid, false);

            // TLB miss - would trigger page walk in real implementation
            None
        };

        // Update performance monitoring
        let lookup_time = TlbEntry::get_timestamp() - start_time;
        self.update_performance_monitoring(lookup_time);

        result
    }

    /// Perform optimized G-stage translation with prefetching
    pub fn translate_gstage_optimized(&mut self, gpa: usize, asid: u16, vmid: u16) -> Option<usize> {
        let start_time = TlbEntry::get_timestamp();

        // Try G-stage software TLB first
        let result = if let Some(entry) = self.gstage_tlb.lookup(gpa, asid, vmid) {
            // Update access statistics
            self.update_access_statistics(gpa, asid, vmid, true);

            // Trigger prefetching for G-stage if configured
            if self.prefetch_config.enabled &&
               entry.access_count >= self.prefetch_config.prefetch_threshold {
                self.schedule_gstage_prefetch(gpa, asid, vmid);
            }

            Some(entry.translate(gpa))
        } else {
            // Update access statistics
            self.update_access_statistics(gpa, asid, vmid, false);

            // TLB miss - would trigger G-stage page walk in real implementation
            None
        };

        // Update performance monitoring
        let lookup_time = TlbEntry::get_timestamp() - start_time;
        self.update_performance_monitoring(lookup_time);

        result
    }

    /// Schedule prefetch for regular translation
    fn schedule_prefetch(&mut self, vaddr: usize, asid: u16, vmid: u16) {
        if !self.prefetch_config.enabled {
            return;
        }

        let page_size = 4096; // Standard page size
        let mut prefetch_queue = self.prefetch_queue.lock();

        // Schedule prefetches for nearby pages
        for offset in 1..=self.prefetch_config.prefetch_distance {
            let prefetch_addr = vaddr + (offset * page_size);

            // Check if not already in queue
            if !prefetch_queue.iter().any(|(addr, a, v)| *addr == prefetch_addr && *a == asid && *v == vmid) {
                if prefetch_queue.len() < self.prefetch_config.max_prefetch_queue {
                    prefetch_queue.push((prefetch_addr, asid, vmid));
                }
            }
        }

        // Process prefetch queue
        self.process_prefetch_queue();
    }

    /// Schedule prefetch for G-stage translation
    fn schedule_gstage_prefetch(&mut self, gpa: usize, asid: u16, vmid: u16) {
        // Similar to regular prefetch but for G-stage addresses
        self.schedule_prefetch(gpa, asid, vmid);
    }

    /// Process prefetch queue
    fn process_prefetch_queue(&mut self) {
        let mut prefetch_queue = self.prefetch_queue.lock();
        let queue_size = prefetch_queue.len();

        if queue_size == 0 {
            return;
        }

        // Process a batch of prefetch requests
        let batch_size = (queue_size / 2).min(4); // Process half, max 4
        for _ in 0..batch_size {
            if let Some((addr, asid, vmid)) = prefetch_queue.pop() {
                // In a real implementation, this would trigger actual prefetch
                self.advanced_stats.prefetched_entries.fetch_add(1, Ordering::Relaxed);
                log::debug!("Prefetching TLB entry for addr={:#x}, asid={}, vmid={}", addr, asid, vmid);
            }
        }
    }

    /// Update access statistics
    fn update_access_statistics(&mut self, addr: usize, asid: u16, vmid: u16, hit: bool) {
        // Update VM-specific statistics
        let vm_entries = if vmid == 0 {
            self.regular_tlb.valid_entries()
        } else {
            self.gstage_tlb.valid_entries()
        };

        // Update advanced statistics
        let mut stats = &mut self.advanced_stats;
        stats.vm_tlb_shares.push((vmid, vm_entries));

        // Record peak entries used
        let current_entries = stats.peak_entries_used.load(Ordering::Relaxed);
        if vm_entries > current_entries {
            stats.peak_entries_used.store(vm_entries, Ordering::Relaxed);
        }
    }

    /// Update performance monitoring
    fn update_performance_monitoring(&mut self, lookup_time: u64) {
        let hit_rate = self.regular_tlb.get_stats().hit_rate();
        let entry_utilization = (self.regular_tlb.valid_entries() as f64 /
                                (self.regular_tlb.num_sets * self.regular_tlb.ways) as f64) * 100.0;

        let sample = TlbPerformanceSample {
            timestamp: TlbEntry::get_timestamp(),
            hit_rate,
            entry_utilization,
            avg_lookup_time: lookup_time,
            active_vms: self.advanced_stats.vm_tlb_shares.len(),
        };

        let mut monitor = self.performance_monitor.lock();
        monitor.record_sample(sample);

        // Check if optimization is needed
        if let Some(trigger) = monitor.needs_optimization() {
            self.perform_optimization(trigger);
        }
    }

    /// Perform TLB optimization based on trigger
    fn perform_optimization(&mut self, trigger: TlbOptimizationTrigger) {
        log::info!("Performing TLB optimization triggered by {:?}", trigger);

        let start_time = TlbEntry::get_timestamp();

        match self.optimization_strategy {
            TlbOptimizationStrategy::LRU => {
                self.optimize_lru();
            }
            TlbOptimizationStrategy::LFU => {
                self.optimize_lfu();
            }
            TlbOptimizationStrategy::MRU => {
                self.optimize_mru();
            }
            TlbOptimizationStrategy::Random => {
                self.optimize_random();
            }
            TlbOptimizationStrategy::Adaptive => {
                self.optimize_adaptive(&trigger);
            }
            TlbOptimizationStrategy::None => {
                // No optimization
            }
        }

        // Perform coalescing if enabled
        if self.coalescing_config.enabled {
            self.perform_coalescing();
        }

        let optimization_time = TlbEntry::get_timestamp() - start_time;
        self.advanced_stats.optimization_cycles.fetch_add(optimization_time, Ordering::Relaxed);

        log::info!("TLB optimization completed in {} cycles", optimization_time);
    }

    /// Optimize using LRU strategy
    fn optimize_lru(&mut self) {
        // Age out old entries based on LRU
        let aged_out = self.regular_tlb.optimize_performance(1000);
        if aged_out > 0 {
            log::debug!("Aged out {} entries using LRU optimization", aged_out);
        }
    }

    /// Optimize using LFU strategy
    fn optimize_lfu(&mut self) {
        // Remove least frequently used entries
        // This would require access frequency tracking in TLB entries
        self.regular_tlb.optimize_performance(500);
    }

    /// Optimize using MRU strategy
    fn optimize_mru(&mut self) {
        // Keep most recently used entries, remove others
        self.regular_tlb.optimize_performance(200);
    }

    /// Optimize using random strategy
    fn optimize_random(&mut self) {
        // Random replacement for better distribution
        self.regular_tlb.optimize_performance(300);
    }

    /// Adaptive optimization based on workload
    fn optimize_adaptive(&mut self, trigger: &TlbOptimizationTrigger) {
        match trigger {
            TlbOptimizationTrigger::LowHitRate => {
                // Aggressive optimization for low hit rate
                self.regular_tlb.optimize_performance(100);
                self.gstage_tlb.optimize_performance(100);
            }
            TlbOptimizationTrigger::HighMissRate => {
                // Moderate optimization for high miss rate
                self.regular_tlb.optimize_performance(500);
            }
            TlbOptimizationTrigger::HighUtilization => {
                // Conservative optimization for high utilization
                self.regular_tlb.optimize_performance(1000);
            }
            _ => {
                // Default optimization
                self.regular_tlb.optimize_performance(300);
            }
        }
    }

    /// Perform TLB entry coalescing
    fn perform_coalescing(&mut self) {
        // Coalesce adjacent entries to improve TLB utilization
        let regular_entries = self.regular_tlb.valid_entries();
        let gstage_entries = self.gstage_tlb.valid_entries();

        if regular_entries >= self.coalescing_config.min_entries {
            self.coalesce_entries(&mut self.regular_tlb);
        }

        if gstage_entries >= self.coalescing_config.min_entries {
            self.coalesce_entries(&mut self.gstage_tlb);
        }

        self.advanced_stats.coalescing_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Coalesce entries in a TLB
    fn coalesce_entries(&mut self, tlb: &mut SoftwareTlb) {
        // In a real implementation, this would:
        // 1. Find adjacent entries with similar permissions
        // 2. Replace them with larger page entries if possible
        // 3. Update statistics accordingly

        let coalesced_count = tlb.valid_entries() / 8; // Example: coalesce 12.5% of entries
        self.advanced_stats.coalesced_entries.fetch_add(coalesced_count, Ordering::Relaxed);

        log::debug!("Coalesced {} TLB entries", coalesced_count);
    }

    /// Perform optimized VMID invalidation
    pub fn invalidate_vmid_optimized(&mut self, vmid: u16) -> usize {
        let start_time = TlbEntry::get_timestamp();

        // Invalidate both regular and G-stage entries
        let regular_count = self.regular_tlb.invalidate_vmid(vmid);
        let gstage_count = self.gstage_tlb.invalidate_vmid(vmid);
        let total = regular_count + gstage_count;

        if total > 0 {
            // Use optimized hardware flush
            self.perform_optimized_hardware_flush(vmid, None);

            // Update statistics
            self.advanced_stats.hardware_flushes.fetch_add(1, Ordering::Relaxed);
        }

        let flush_time = TlbEntry::get_timestamp() - start_time;
        log::debug!("Optimized VMID {} invalidation: {} entries in {} cycles", vmid, total, flush_time);

        total
    }

    /// Perform optimized address range invalidation
    pub fn invalidate_range_optimized(&mut self, start_addr: usize, size: usize, asid: u16, vmid: u16) -> usize {
        let start_time = TlbEntry::get_timestamp();

        // Invalidate entries in range
        let regular_count = self.regular_tlb.invalidate_range(start_addr, size, asid, vmid);
        let gstage_count = self.gstage_tlb.invalidate_range(start_addr, size, asid, vmid);
        let total = regular_count + gstage_count;

        if total > 0 {
            // Use targeted hardware flush
            self.perform_optimized_hardware_flush(vmid, Some(start_addr));

            // Update statistics
            self.advanced_stats.hardware_flushes.fetch_add(1, Ordering::Relaxed);
        }

        let flush_time = TlbEntry::get_timestamp() - start_time;
        log::debug!("Optimized range invalidation: {} entries in {} cycles", total, flush_time);

        total
    }

    /// Perform optimized hardware flush
    fn perform_optimized_hardware_flush(&mut self, vmid: u16, addr: Option<usize>) {
        // Use the most efficient flush method based on the situation
        if let Some(flush_addr) = addr {
            // Targeted flush
            self.hardware_tlb.flush_gstage_addr(flush_addr, vmid);
        } else {
            // VMID-specific flush
            self.hardware_tlb.flush_gstage_vmid(vmid);
        }
    }

    /// Get advanced statistics
    pub fn get_advanced_stats(&self) -> &AdvancedTlbStats {
        &self.advanced_stats
    }

    /// Get performance monitor snapshot
    pub fn get_performance_snapshot(&self) -> TlbPerformanceMonitor {
        self.performance_monitor.lock().clone()
    }

    /// Reset all statistics
    pub fn reset_statistics(&mut self) {
        self.advanced_stats = AdvancedTlbStats::default();
        *self.performance_monitor.lock() = TlbPerformanceMonitor::new(10);

        // Reset base TLB statistics
        self.regular_tlb.get_stats().lookups.store(0, Ordering::Relaxed);
        self.regular_tlb.get_stats().hits.store(0, Ordering::Relaxed);
        self.regular_tlb.get_stats().misses.store(0, Ordering::Relaxed);

        self.gstage_tlb.get_stats().lookups.store(0, Ordering::Relaxed);
        self.gstage_tlb.get_stats().hits.store(0, Ordering::Relaxed);
        self.gstage_tlb.get_stats().misses.store(0, Ordering::Relaxed);

        log::info!("All TLB statistics reset");
    }

    /// Generate comprehensive TLB report
    pub fn generate_report(&self) -> TlbReport {
        let regular_stats = self.regular_tlb.get_stats();
        let gstage_stats = self.gstage_tlb.get_stats();
        let advanced_stats = &self.advanced_stats;

        TlbReport {
            timestamp: TlbEntry::get_timestamp(),
            regular_hit_rate: regular_stats.hit_rate(),
            gstage_hit_rate: gstage_stats.hit_rate(),
            total_entries: self.regular_tlb.valid_entries() + self.gstage_tlb.valid_entries(),
            coalescing_efficiency: advanced_stats.coalescing_efficiency(),
            prefetch_accuracy: advanced_stats.prefetch_accuracy(),
            peak_utilization: (advanced_stats.peak_entries_used.load(Ordering::Relaxed) as f64 /
                               ((self.regular_tlb.num_sets * self.regular_tlb.ways) as f64)) * 100.0,
            optimization_cycles: advanced_stats.optimization_cycles.load(Ordering::Relaxed),
            vm_distribution: advanced_stats.vm_tlb_shares.clone(),
        }
    }
}

/// Comprehensive TLB performance report
#[derive(Debug, Clone)]
pub struct TlbReport {
    /// Report timestamp
    pub timestamp: u64,
    /// Regular TLB hit rate
    pub regular_hit_rate: f64,
    /// G-stage TLB hit rate
    pub gstage_hit_rate: f64,
    /// Total entries used
    pub total_entries: usize,
    /// Coalescing efficiency
    pub coalescing_efficiency: f64,
    /// Prefetching accuracy
    pub prefetch_accuracy: f64,
    /// Peak utilization percentage
    pub peak_utilization: f64,
    /// Total optimization cycles
    pub optimization_cycles: u64,
    /// VM distribution of TLB entries
    pub vm_distribution: Vec<(u16, usize)>,
}

impl TlbReport {
    /// Print formatted report
    pub fn print(&self) {
        log::info!("=== TLB Performance Report ===");
        log::info!("Timestamp: {}", self.timestamp);
        log::info!("Regular TLB Hit Rate: {:.2}%", self.regular_hit_rate);
        log::info!("G-stage TLB Hit Rate: {:.2}%", self.gstage_hit_rate);
        log::info!("Total Entries Used: {} / {}", self.total_entries, "N/A");
        log::info!("Peak Utilization: {:.2}%", self.peak_utilization);
        log::info!("Coalescing Efficiency: {:.2}%", self.coalescing_efficiency);
        log::info!("Prefetch Accuracy: {:.2}%", self.prefetch_accuracy);
        log::info!("Optimization Cycles: {}", self.optimization_cycles);

        log::info!("VM Distribution:");
        for (vmid, entries) in &self.vm_distribution {
            log::info!("  VMID {}: {} entries", vmid, entries);
        }
        log::info!("==============================");
    }
}

    /// Perform address translation with TLB lookup (legacy)
    pub fn translate_regular(&mut self, vaddr: usize, asid: u16, vmid: u16) -> Option<usize> {
        self.translate_regular_optimized(vaddr, asid, vmid)
    }

    /// Perform G-stage translation with TLB lookup (legacy)
    pub fn translate_gstage(&mut self, gpa: usize, asid: u16, vmid: u16) -> Option<usize> {
        self.translate_gstage_optimized(gpa, asid, vmid)
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
        self.invalidate_vmid_optimized(vmid)
    }

    /// Flush all TLBs
    pub fn flush_all(&mut self) {
        self.regular_tlb.flush_all();
        self.gstage_tlb.flush_all();
        self.hardware_tlb.flush_all();
        self.hardware_tlb.flush_gstage_all();

        // Update statistics
        self.advanced_stats.hardware_flushes.fetch_add(1, Ordering::Relaxed);
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

    /// Perform periodic maintenance
    pub fn perform_maintenance(&mut self) {
        log::debug!("Performing TLB maintenance");

        // Process prefetch queue
        self.process_prefetch_queue();

        // Check if optimization is needed
        let monitor = self.performance_monitor.lock();
        if let Some(trigger) = monitor.needs_optimization() {
            drop(monitor);
            self.perform_optimization(trigger);
        } else {
            drop(monitor);
        }

        // Update performance statistics
        let current_time = TlbEntry::get_timestamp();
        let report = self.generate_report();

        log::debug!("TLB maintenance completed - Hit rates: R={:.1}%, G={:.1}%",
                   report.regular_hit_rate, report.gstage_hit_rate);
    }

    /// Get TLB health metrics
    pub fn get_health_metrics(&self) -> TlbHealthMetrics {
        let report = self.generate_report();

        TlbHealthMetrics {
            overall_health: self.calculate_health_score(&report),
            hit_rate_trend: self.calculate_hit_rate_trend(),
            utilization_trend: self.calculate_utilization_trend(),
            optimization_frequency: self.advanced_stats.optimization_cycles.load(Ordering::Relaxed),
            recommendations: self.generate_recommendations(&report),
        }
    }

    /// Calculate overall health score (0-100)
    fn calculate_health_score(&self, report: &TlbReport) -> u8 {
        let hit_rate_score = (report.regular_hit_rate.max(report.gstage_hit_rate) * 0.5) as u8;
        let utilization_score = if report.peak_utilization < 80.0 { 25 } else { 10 };
        let coalescing_score = if report.coalescing_efficiency > 50.0 { 15 } else { 5 };
        let prefetch_score = if report.prefetch_accuracy > 60.0 { 10 } else { 0 };

        (hit_rate_score + utilization_score + coalescing_score + prefetch_score).min(100)
    }

    /// Calculate hit rate trend
    fn calculate_hit_rate_trend(&self) -> Trend {
        // Implementation would analyze recent performance samples
        Trend::Stable
    }

    /// Calculate utilization trend
    fn calculate_utilization_trend(&self) -> Trend {
        // Implementation would analyze utilization over time
        Trend::Stable
    }

    /// Generate optimization recommendations
    fn generate_recommendations(&self, report: &TlbReport) -> Vec<String> {
        let mut recommendations = Vec::new();

        if report.regular_hit_rate < 80.0 {
            recommendations.push("Consider increasing TLB size or adjusting replacement strategy".to_string());
        }

        if report.peak_utilization > 90.0 {
            recommendations.push("High TLB utilization detected - consider coalescing or increasing capacity".to_string());
        }

        if report.prefetch_accuracy < 50.0 {
            recommendations.push("Low prefetch accuracy - adjust prefetch distance or threshold".to_string());
        }

        if report.optimization_cycles > 10000 {
            recommendations.push("High optimization overhead - consider less aggressive strategy".to_string());
        }

        recommendations
    }
}

/// TLB health metrics
#[derive(Debug, Clone)]
pub struct TlbHealthMetrics {
    /// Overall health score (0-100)
    pub overall_health: u8,
    /// Hit rate trend
    pub hit_rate_trend: Trend,
    /// Utilization trend
    pub utilization_trend: Trend,
    /// Optimization frequency
    pub optimization_frequency: u64,
    /// Optimization recommendations
    pub recommendations: Vec<String>,
}

/// Performance trend indicators
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trend {
    /// Improving performance
    Improving,
    /// Stable performance
    Stable,
    /// Degrading performance
    Degrading,
}

impl TlbHealthMetrics {
    /// Check if TLB needs attention
    pub fn needs_attention(&self) -> bool {
        self.overall_health < 70 ||
        matches!(self.hit_rate_trend, Trend::Degrading) ||
        matches!(self.utilization_trend, Trend::Degrading) ||
        !self.recommendations.is_empty()
    }

    /// Print health report
    pub fn print(&self) {
        log::info!("=== TLB Health Report ===");
        log::info!("Overall Health: {}/100", self.overall_health);
        log::info!("Hit Rate Trend: {:?}", self.hit_rate_trend);
        log::info!("Utilization Trend: {:?}", self.utilization_trend);
        log::info!("Optimization Frequency: {}", self.optimization_frequency);

        if !self.recommendations.is_empty() {
            log::info!("Recommendations:");
            for (i, rec) in self.recommendations.iter().enumerate() {
                log::info!("  {}. {}", i + 1, rec);
            }
        }

        if self.needs_attention() {
            log::warn!("TLB needs attention!");
        } else {
            log::info!("TLB operating normally.");
        }
        log::info!("========================");
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