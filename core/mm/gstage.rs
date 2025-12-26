//! G-stage Address Translation
//!
//! This module provides G-stage (Stage 2) address translation support for RISC-V
//! virtualization, implementing Guest Physical Address (GPA) to Host Physical
//! Address (HPA) translation as defined in the RISC-V H-extension.

use crate::{Result, Error};
use crate::core::mm::{PhysAddr, VirtAddr, PageNr, PAGE_SIZE, PAGE_SHIFT, PageFlags};
use crate::core::sync::SpinLock;
use alloc::{vec::Vec, vec};
use core::sync::atomic::{AtomicU32, Ordering};

/// Guest Virtual Address type
pub type Gva = VirtAddr;

/// Guest Physical Address type
pub type Gpa = PhysAddr;

/// Host Physical Address type
pub type Hpa = PhysAddr;

/// Virtual Machine ID type
pub type Vmid = u16;

/// G-stage page table entry format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GStagePte {
    /// Raw PTE value
    pub bits: u64,
}

/// G-stage page table entry bit fields
pub mod gstage_pte {
    pub const V: u64 = 0x0000_0000_0000_0001;  // Valid bit
    pub const R: u64 = 0x0000_0000_0000_0002;  // Read bit
    pub const W: u64 = 0x0000_0000_0000_0004;  // Write bit
    pub const X: u64 = 0x0000_0000_0000_0008;  // Execute bit
    pub const U: u64 = 0x0000_0000_0000_0010;  // User mode bit (always 1 for G-stage)
    pub const G: u64 = 0x0000_0000_0000_0020;  // Global bit
    pub const A: u64 = 0x0000_0000_0000_0040;  // Accessed bit
    pub const D: u64 = 0x0000_0000_0000_0080;  // Dirty bit

    // RSW bits (Reserved for Software)
    pub const RSW0: u64 = 0x0000_0000_0000_0100;
    pub const RSW1: u64 = 0x0000_0000_0000_0200;

    // Physical address field (bits 53:10 for Sv39, 59:12 for Sv48)
    pub const PPN_SHIFT: u64 = 10;
    pub const PPN_MASK: u64 = 0x003F_FFFF_FFFF_FC00;

    // Leaf PTE bits (all permissions must be set for a leaf)
    pub const LEAF_BITS: u64 = R | W | X;
}

impl GStagePte {
    /// Create an invalid PTE
    pub const fn invalid() -> Self {
        Self { bits: 0 }
    }

    /// Create a leaf PTE with the given physical address and permissions
    pub fn leaf(ppn: u64, flags: u64) -> Self {
        let bits = (ppn << gstage_pte::PPN_SHIFT) | gstage_pte::V | flags;
        Self { bits }
    }

    /// Create a branch PTE (points to next level page table)
    pub fn branch(ppn: u64) -> Self {
        let bits = (ppn << gstage_pte::PPN_SHIFT) | gstage_pte::V;
        Self { bits }
    }

    /// Check if this PTE is valid
    pub const fn is_valid(&self) -> bool {
        (self.bits & gstage_pte::V) != 0
    }

    /// Check if this PTE is a leaf (has RWX permissions)
    pub const fn is_leaf(&self) -> bool {
        (self.bits & gstage_pte::LEAF_BITS) != 0
    }

    /// Check if this PTE is a branch (points to next level)
    pub const fn is_branch(&self) -> bool {
        self.is_valid() && !self.is_leaf()
    }

    /// Get the physical page number
    pub const fn ppn(&self) -> u64 {
        (self.bits & gstage_pte::PPN_MASK) >> gstage_pte::PPN_SHIFT
    }

    /// Get the physical address
    pub const fn pa(&self) -> PhysAddr {
        self.ppn() * PAGE_SIZE
    }

    /// Check read permission
    pub const fn can_read(&self) -> bool {
        (self.bits & gstage_pte::R) != 0
    }

    /// Check write permission
    pub const fn can_write(&self) -> bool {
        (self.bits & gstage_pte::W) != 0
    }

    /// Check execute permission
    pub const fn can_execute(&self) -> bool {
        (self.bits & gstage_pte::X) != 0
    }

    /// Check if the page is accessed
    pub const fn is_accessed(&self) -> bool {
        (self.bits & gstage_pte::A) != 0
    }

    /// Check if the page is dirty
    pub const fn is_dirty(&self) -> bool {
        (self.bits & gstage_pte::D) != 0
    }

    /// Set the accessed bit
    pub fn set_accessed(&mut self) {
        self.bits |= gstage_pte::A;
    }

    /// Set the dirty bit
    pub fn set_dirty(&mut self) {
        self.bits |= gstage_pte::D;
    }

    /// Clear the accessed bit
    pub fn clear_accessed(&mut self) {
        self.bits &= !gstage_pte::A;
    }

    /// Clear the dirty bit
    pub fn clear_dirty(&mut self) {
        self.bits &= !gstage_pte::D;
    }
}

/// G-stage address translation modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GStageMode {
    /// No translation (bypass)
    None,
    /// Sv32X4 - 32-bit address space with 4KB pages
    Sv32X4,
    /// Sv39X4 - 39-bit address space with 4KB pages
    Sv39X4,
    /// Sv48X4 - 48-bit address space with 4KB pages
    Sv48X4,
    /// Sv57X4 - 57-bit address space with 4KB pages
    Sv57X4,
}

/// Hardware capability information for G-stage translation
#[derive(Debug, Clone)]
pub struct GStageCapabilities {
    /// Supported page table formats
    pub supported_modes: Vec<GStageMode>,
    /// Maximum supported VMID bits
    pub max_vmid_bits: u32,
    /// Support for extended PTE format
    pub extended_pte: bool,
    /// Support for hardware page table walk
    pub hw_walk: bool,
    /// Support for virtualization extensions
    pub virtualization: bool,
    /// Support for huge pages at G-stage
    pub huge_pages: bool,
    /// Supported huge page sizes
    pub supported_huge_sizes: Vec<u64>,
}

impl GStageCapabilities {
    /// Detect hardware capabilities
    pub fn detect() -> Self {
        let mut supported_modes = Vec::new();

        // Base Sv39X4 is always supported on RISC-V 64-bit
        supported_modes.push(GStageMode::Sv39X4);

        // Check for extended modes based on hardware detection
        #[cfg(target_arch = "riscv64")]
        {
            // In a real implementation, this would read actual hardware registers
            // For now, assume we support the common modes
            supported_modes.push(GStageMode::Sv48X4);

            // Sv57X4 support detection (would check specific hardware bits)
            // supported_modes.push(GStageMode::Sv57X4);

            // Sv32X4 is for 32-bit systems, but detect anyway
            // supported_modes.push(GStageMode::Sv32X4);
        }

        let mut supported_huge_sizes = Vec::new();
        supported_huge_sizes.push(2 * 1024 * 1024); // 2MB
        supported_huge_sizes.push(1024 * 1024 * 1024); // 1GB

        Self {
            supported_modes,
            max_vmid_bits: 7, // Standard is 7 bits (0-127)
            extended_pte: true, // Modern RISC-V supports extended PTE
            hw_walk: true, // Hardware page table walk support
            virtualization: true, // H-extension present
            huge_pages: true, // Huge page support
            supported_huge_sizes,
        }
    }

    /// Check if a mode is supported
    pub fn supports_mode(&self, mode: GStageMode) -> bool {
        self.supported_modes.contains(&mode)
    }

    /// Get the best supported mode
    pub fn best_mode(&self) -> GStageMode {
        // Return the highest supported mode
        for &mode in &[GStageMode::Sv57X4, GStageMode::Sv48X4, GStageMode::Sv39X4, GStageMode::Sv32X4] {
            if self.supports_mode(mode) {
                return mode;
            }
        }
        GStageMode::None
    }

    /// Check if huge page size is supported
    pub fn supports_huge_size(&self, size: u64) -> bool {
        self.supported_huge_sizes.contains(&size)
    }
}

/// Address space layout for different G-stage modes
#[derive(Debug, Clone)]
pub struct GStageAddressSpace {
    /// Mode for this address space
    pub mode: GStageMode,
    /// Base address of the address space
    pub base: VirtAddr,
    /// Size of the address space
    pub size: u64,
    /// Number of levels
    pub levels: u32,
    /// Bits per level
    pub bits_per_level: u32,
    /// Physical address bits
    pub pa_bits: u32,
    /// Virtual address bits
    pub va_bits: u32,
}

impl GStageAddressSpace {
    /// Create address space layout for a mode
    pub fn for_mode(mode: GStageMode) -> Self {
        match mode {
            GStageMode::None => Self {
                mode,
                base: 0,
                size: 0,
                levels: 0,
                bits_per_level: 0,
                pa_bits: 0,
                va_bits: 0,
            },
            GStageMode::Sv32X4 => Self {
                mode,
                base: 0,
                size: 1u64 << 32,
                levels: 2,
                bits_per_level: 10,
                pa_bits: 34, // Standard for RV32
                va_bits: 32,
            },
            GStageMode::Sv39X4 => Self {
                mode,
                base: 0,
                size: 1u64 << 39,
                levels: 3,
                bits_per_level: 9,
                pa_bits: 56, // Standard for RV64
                va_bits: 39,
            },
            GStageMode::Sv48X4 => Self {
                mode,
                base: 0,
                size: 1u64 << 48,
                levels: 4,
                bits_per_level: 9,
                pa_bits: 56, // Standard for RV64
                va_bits: 48,
            },
            GStageMode::Sv57X4 => Self {
                mode,
                base: 0,
                size: 1u64 << 57,
                levels: 5,
                bits_per_level: 9,
                pa_bits: 56, // Standard for RV64
                va_bits: 57,
            },
        }
    }

    /// Check if an address is within this address space
    pub fn contains(&self, addr: VirtAddr) -> bool {
        addr >= self.base && addr < (self.base + self.size)
    }

    /// Get the maximum supported address
    pub fn max_address(&self) -> VirtAddr {
        self.base + self.size - 1
    }
}

impl GStageMode {
    /// Get the number of virtual address bits
    pub const fn va_bits(&self) -> u32 {
        match self {
            GStageMode::None => 0,
            GStageMode::Sv32X4 => 32,
            GStageMode::Sv39X4 => 39,
            GStageMode::Sv48X4 => 48,
            GStageMode::Sv57X4 => 57,
        }
    }

    /// Get the number of levels in the page table
    pub const fn levels(&self) -> u32 {
        match self {
            GStageMode::None => 0,
            GStageMode::Sv32X4 => 2,
            GStageMode::Sv39X4 => 3,
            GStageMode::Sv48X4 => 4,
            GStageMode::Sv57X4 => 5,
        }
    }

    /// Get the virtual address size
    pub const fn va_size(&self) -> u64 {
        1u64 << self.va_bits()
    }

    /// Get the HGATP mode value
    pub const fn hgatp_mode(&self) -> u64 {
        match self {
            GStageMode::None => 0,
            GStageMode::Sv32X4 => 8,
            GStageMode::Sv39X4 => 9,
            GStageMode::Sv48X4 => 10,
            GStageMode::Sv57X4 => 11,
        }
    }

    /// Get the number of PPN bits
    pub const fn ppn_bits(&self) -> u32 {
        match self {
            GStageMode::None => 0,
            GStageMode::Sv32X4 => 22,
            GStageMode::Sv39X4 => 44,
            GStageMode::Sv48X4 => 44,
            GStageMode::Sv57X4 => 44,
        }
    }
}

/// G-stage page table levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GStageLevel {
    /// Root level (level 0 for Sv39X4)
    Root = 0,
    /// Level 1
    Level1 = 1,
    /// Level 2
    Level2 = 2,
    /// Level 3
    Level3 = 3,
    /// Level 4
    Level4 = 4,
}

impl GStageLevel {
    /// Get the next level
    pub const fn next(&self) -> Option<Self> {
        match self {
            GStageLevel::Root => Some(GStageLevel::Level1),
            GStageLevel::Level1 => Some(GStageLevel::Level2),
            GStageLevel::Level2 => Some(GStageLevel::Level3),
            GStageLevel::Level3 => Some(GStageLevel::Level4),
            GStageLevel::Level4 => None,
        }
    }

    /// Get the level index
    pub const fn index(&self) -> usize {
        *self as usize
    }
}

/// G-stage page table
#[derive(Debug)]
pub struct GStagePageTable {
    /// Physical address of this page table
    pub pa: PhysAddr,
    /// Virtual address of this page table
    pub va: VirtAddr,
    /// Level of this page table
    pub level: GStageLevel,
    /// VMID associated with this page table
    pub vmid: Vmid,
    /// Translation mode
    pub mode: GStageMode,
    /// Page table entries (512 for Sv39X4)
    pub entries: SpinLock<Vec<GStagePte>>,
    /// Child page tables
    pub children: SpinLock<Vec<GStagePageTable>>,
    /// Reference count
    pub ref_count: AtomicU32,
}

impl GStagePageTable {
    /// Create a new G-stage page table
    pub fn new(
        level: GStageLevel,
        vmid: Vmid,
        mode: GStageMode,
        pa: PhysAddr,
        va: VirtAddr,
    ) -> Self {
        let entry_count = Self::entries_per_level_for_mode(mode);
        Self {
            pa,
            va,
            level,
            vmid,
            mode,
            entries: SpinLock::new(vec![GStagePte::invalid(); entry_count]),
            children: SpinLock::new(Vec::new()),
            ref_count: AtomicU32::new(1),
        }
    }

    /// Get the number of entries per level for a specific mode
    pub fn entries_per_level_for_mode(mode: GStageMode) -> usize {
        match mode {
            GStageMode::Sv32X4 => 1024, // 2^10 entries
            GStageMode::Sv39X4 | GStageMode::Sv48X4 | GStageMode::Sv57X4 => 512, // 2^9 entries
            GStageMode::None => 0,
        }
    }

    /// Get the number of entries per level
    pub fn entries_per_level(&self) -> usize {
        Self::entries_per_level_for_mode(self.mode)
    }

    /// Get the virtual bits per level for a specific mode
    pub fn bits_per_level_for_mode(mode: GStageMode) -> u32 {
        match mode {
            GStageMode::Sv32X4 => 10,
            GStageMode::Sv39X4 | GStageMode::Sv48X4 | GStageMode::Sv57X4 => 9,
            GStageMode::None => 0,
        }
    }

    /// Get the virtual bits per level
    pub fn bits_per_level(&self) -> u32 {
        Self::bits_per_level_for_mode(self.mode)
    }

    /// Get the address space layout for this page table's mode
    pub fn address_space(&self) -> GStageAddressSpace {
        GStageAddressSpace::for_mode(self.mode)
    }

    /// Check if this level can use huge pages
    pub fn can_use_huge_pages(&self, level: GStageLevel) -> bool {
        match self.mode {
            GStageMode::Sv32X4 => level.index() >= 1, // Superpage (4MB) at level 1
            GStageMode::Sv39X4 => level.index() >= 2, // Superpage (2MB) at level 2
            GStageMode::Sv48X4 => level.index() >= 2, // Superpage (2MB) at level 2
            GStageMode::Sv57X4 => level.index() >= 2, // Superpage (2MB) at level 2
            GStageMode::None => false,
        }
    }

    /// Get huge page size for this level
    pub fn huge_page_size(&self, level: GStageLevel) -> Option<u64> {
        if !self.can_use_huge_pages(level) {
            return None;
        }

        let bits_per_level = self.bits_per_level();
        let remaining_levels = self.mode.levels() - level.index() as u32 - 1;
        let page_bits = PAGE_SHIFT + (bits_per_level * remaining_levels);

        Some(1u64 << page_bits)
    }

    /// Check if an address is aligned to huge page boundary at this level
    pub fn is_huge_aligned(&self, gpa: Gpa, level: GStageLevel) -> bool {
        if let Some(huge_size) = self.huge_page_size(level) {
            (gpa & (huge_size - 1)) == 0
        } else {
            false
        }
    }

    /// Extract VPN for a given level
    pub fn extract_vpn(&self, gpa: Gpa, level: GStageLevel) -> usize {
        let address_space = self.address_space();

        // Check if GPA is within address space
        if !address_space.contains(gpa) {
            return 0; // Invalid address
        }

        let vpn_shift = PAGE_SHIFT + (address_space.bits_per_level * (address_space.levels - level.index() as u32 - 1));
        let vpn_mask = (1u64 << address_space.bits_per_level) - 1;
        let vpn = (gpa >> vpn_shift) & vpn_mask;

        vpn as usize
    }

    /// Extract multiple VPNs for different levels (optimized extraction)
    pub fn extract_vpns(&self, gpa: Gpa) -> Vec<usize> {
        let address_space = self.address_space();
        let mut vpns = Vec::with_capacity(address_space.levels as usize);
        let bits_per_level = address_space.bits_per_level;

        for level in 0..address_space.levels {
            let level_idx = address_space.levels - level - 1;
            let vpn_shift = PAGE_SHIFT + (bits_per_level * level);
            let vpn_mask = (1u64 << bits_per_level) - 1;
            let vpn = ((gpa >> vpn_shift) & vpn_mask) as usize;
            vpns.push(vpn);
        }

        vpns
    }

    /// Create next level page table if needed
    pub fn create_child_table(&self, level: GStageLevel, index: usize) -> Result<Box<GStagePageTable>> {
        if level.index() >= (self.mode.levels() as usize - 1) {
            return Err(Error::InvalidArgument); // Can't create child at leaf level
        }

        // Allocate physical frame for child page table
        let child_pa = crate::core::mm::frame::alloc_frame()
            .ok_or(Error::OutOfMemory)?;
        let child_va = crate::core::mm::frame::phys_to_virt(child_pa);

        let next_level = level.next().ok_or(Error::InvalidArgument)?;
        let child = Box::new(GStagePageTable::new(
            next_level,
            self.vmid,
            self.mode,
            child_pa,
            child_va,
        ));

        // Update the current PTE to point to child table
        let child_ppn = child_pa / PAGE_SIZE;
        let branch_pte = GStagePte::branch(child_ppn);
        self.set_pte(index, branch_pte)?;

        // Add to children list
        self.children.lock().push(*child);

        Ok(child)
    }

    /// Get PTE at a specific index
    pub fn get_pte(&self, index: usize) -> Result<GStagePte> {
        let entries = self.entries.lock();
        if index < entries.len() {
            Ok(entries[index])
        } else {
            Err(Error::InvalidArgument)
        }
    }

    /// Set PTE at a specific index
    pub fn set_pte(&self, index: usize, pte: GStagePte) -> Result<()> {
        let mut entries = self.entries.lock();
        if index < entries.len() {
            entries[index] = pte;
            Ok(())
        } else {
            Err(Error::InvalidArgument)
        }
    }

    /// Clear PTE at a specific index
    pub fn clear_pte(&self, index: usize) -> Result<()> {
        self.set_pte(index, GStagePte::invalid())
    }

    /// Increment reference count
    pub fn inc_ref(&self) {
        self.ref_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement reference count
    pub fn dec_ref(&self) -> u32 {
        self.ref_count.fetch_sub(1, Ordering::Relaxed) - 1
    }

    /// Get current reference count
    pub fn get_ref_count(&self) -> u32 {
        self.ref_count.load(Ordering::Relaxed)
    }

    /// Walk the page table to find or create an entry (multi-format support)
    pub fn walk(&self, gpa: Gpa, create: bool) -> Result<(GStagePte, Option<GStageLevel>)> {
        let address_space = self.address_space();

        // Check if GPA is within address space
        if !address_space.contains(gpa) {
            return Err(Error::InvalidArgument);
        }

        let vpn = self.extract_vpn(gpa, self.level);
        let pte = self.get_pte(vpn)?;

        if !pte.is_valid() {
            if create {
                // Need to create next level page table
                if self.level.index() < (self.mode.levels() as usize - 1) {
                    let child_table = self.create_child_table(self.level, vpn)?;
                    // Continue walking in child table
                    return child_table.walk(gpa, create);
                } else {
                    // This is the leaf level, return invalid PTE for mapping
                    return Ok((GStagePte::invalid(), Some(self.level)));
                }
            } else {
                return Ok((pte, None));
            }
        }

        if pte.is_leaf() {
            Ok((pte, Some(self.level)))
        } else {
            // This is a branch, continue walking
            // In a full implementation, we would load the child table and continue
            // For now, return the branch PTE
            Ok((pte, Some(self.level)))
        }
    }

    /// Multi-format page table walk with optimized path
    pub fn walk_multi_format(&self, gpa: Gpa, create: bool) -> Result<(GStagePte, GStageLevel, u64)> {
        let address_space = self.address_space();
        let mut current_level = self.level;
        let mut current_table = self;
        let mut final_offset = gpa & (PAGE_SIZE - 1);

        // Check if GPA is within address space
        if !address_space.contains(gpa) {
            return Err(Error::InvalidArgument);
        }

        // Walk through each level
        while current_level.index() < address_space.levels as usize {
            let vpn = current_table.extract_vpn(gpa, current_level);
            let pte = current_table.get_pte(vpn)?;

            if !pte.is_valid() {
                if create && current_level.index() < (address_space.levels as usize - 1) {
                    // Create child table
                    let child_table = current_table.create_child_table(current_level, vpn)?;
                    current_table = &*child_table;
                    current_level = current_level.next().unwrap();
                    continue;
                } else {
                    // Return invalid PTE at current level for mapping
                    return Ok((GStagePte::invalid(), current_level, final_offset));
                }
            }

            if pte.is_leaf() {
                // Calculate final offset based on the level where we found the leaf
                let remaining_levels = address_space.levels - current_level.index() as u32 - 1;
                let level_size = PAGE_SIZE << (address_space.bits_per_level * remaining_levels);
                final_offset = gpa & (level_size - 1);
                return Ok((pte, current_level, final_offset));
            }

            // Continue to next level
            if current_level.index() >= (address_space.levels as usize - 1) {
                return Err(Error::InvalidState); // Branch at leaf level
            }

            // In a full implementation, load child table here
            // For now, we can't continue without child table loading
            return Ok((pte, current_level, final_offset));
        }

        Err(Error::NotFound)
    }

    /// Map a GPA to HPA with specified permissions (multi-format with huge page support)
    pub fn map(&self, gpa: Gpa, hpa: Hpa, size: u64, flags: u64) -> Result<()> {
        let address_space = self.address_space();

        // Check if GPA and HPA are within valid ranges
        if !address_space.contains(gpa) {
            return Err(Error::InvalidArgument);
        }

        if !self.is_aligned(gpa, size) || !self.is_aligned(hpa, size) {
            return Err(Error::InvalidArgument);
        }

        // Try to use huge pages if possible
        if size >= PAGE_SIZE {
            if let Ok(_) = self.try_map_huge_pages(gpa, hpa, size, flags) {
                return Ok(());
            }
        }

        // Fall back to regular page mapping
        let pages = (size / PAGE_SIZE) as usize;
        for i in 0..pages {
            let current_gpa = gpa + (i as u64 * PAGE_SIZE);
            let current_hpa = hpa + (i as u64 * PAGE_SIZE);
            self.map_page(current_gpa, current_hpa, flags)?;
        }

        Ok(())
    }

    /// Try to map using huge pages (multi-format support)
    fn try_map_huge_pages(&self, gpa: Gpa, hpa: Hpa, size: u64, flags: u64) -> Result<()> {
        let address_space = self.address_space();
        let mut remaining_size = size;
        let mut current_gpa = gpa;
        let mut current_hpa = hpa;

        // Try from largest to smallest huge page size
        let huge_sizes = [
            1024 * 1024 * 1024, // 1GB
            2 * 1024 * 1024,    // 2MB
        ];

        for huge_size in huge_sizes.iter() {
            if remaining_size >= *huge_size {
                // Find the appropriate level for this huge page size
                if let Some(huge_level) = self.find_level_for_huge_size(*huge_size) {
                    while remaining_size >= *huge_size {
                        if self.is_huge_aligned(current_gpa, huge_level) &&
                           self.is_huge_aligned(current_hpa, huge_level) {
                            self.map_huge_page(current_gpa, current_hpa, *huge_size, flags, huge_level)?;
                            current_gpa += *huge_size;
                            current_hpa += *huge_size;
                            remaining_size -= *huge_size;
                        } else {
                            break; // Can't use this huge page size anymore
                        }
                    }
                }
            }
        }

        if remaining_size == 0 {
            Ok(())
        } else {
            Err(Error::InvalidArgument) // Couldn't map everything with huge pages
        }
    }

    /// Find the appropriate level for a given huge page size
    fn find_level_for_huge_size(&self, size: u64) -> Option<GStageLevel> {
        let address_space = self.address_space();
        let bits_per_level = address_space.bits_per_level;

        for level_idx in 0..address_space.levels {
            let level = match level_idx {
                0 => GStageLevel::Root,
                1 => GStageLevel::Level1,
                2 => GStageLevel::Level2,
                3 => GStageLevel::Level3,
                4 => GStageLevel::Level4,
                _ => return None,
            };

            if let Some(huge_page_size) = self.huge_page_size(level) {
                if huge_page_size == size {
                    return Some(level);
                }
            }
        }

        None
    }

    /// Map a huge page at the specified level
    fn map_huge_page(&self, gpa: Gpa, hpa: Hpa, size: u64, flags: u64, level: GStageLevel) -> Result<()> {
        // Create page tables down to the huge page level
        let mut current_table = self;
        let mut current_level = self.level;

        while current_level.index() < level.index() {
            let vpn = current_table.extract_vpn(gpa, current_level);
            let pte = current_table.get_pte(vpn)?;

            if !pte.is_valid() {
                // Create child table
                current_table = &*current_table.create_child_table(current_level, vpn)?;
            } else if pte.is_leaf() {
                return Err(Error::InvalidState); // Found leaf where we need branch
            } else {
                // In a full implementation, load child table here
                return Err(Error::NotImplemented);
            }

            current_level = current_level.next().unwrap();
        }

        // At the huge page level, create the huge page mapping
        let vpn = current_table.extract_vpn(gpa, level);
        let ppn = hpa / PAGE_SIZE;
        let pte = GStagePte::leaf(ppn, flags);
        current_table.set_pte(vpn, pte)?;

        Ok(())
    }

    /// Map a single page (multi-format)
    fn map_page(&self, gpa: Gpa, hpa: Hpa, flags: u64) -> Result<()> {
        // Use the multi-format walk to find the appropriate location
        let (pte, level, _) = self.walk_multi_format(gpa, true)?;

        if !pte.is_valid() {
            // Create the leaf mapping
            let ppn = hpa / PAGE_SIZE;
            let leaf_pte = GStagePte::leaf(ppn, flags);

            // Set the PTE at the appropriate level
            match level {
                GStageLevel::Root => {
                    if self.level.index() == level.index() {
                        let vpn = self.extract_vpn(gpa, level);
                        self.set_pte(vpn, leaf_pte)?;
                    }
                }
                GStageLevel::Level1 | GStageLevel::Level2 | GStageLevel::Level3 | GStageLevel::Level4 => {
                    // In a full implementation, we would navigate to the correct child table
                    // For now, just try to set at current level
                    let vpn = self.extract_vpn(gpa, level);
                    self.set_pte(vpn, leaf_pte)?;
                }
            }
        }

        Ok(())
    }

    /// Check if an address is properly aligned
    fn is_aligned(&self, addr: u64, size: u64) -> bool {
        (addr & (size - 1)) == 0
    }

    /// Unmap a GPA range
    pub fn unmap(&self, gpa: Gpa, size: u64) -> Result<()> {
        if !self.is_aligned(gpa, size) {
            return Err(Error::InvalidArgument);
        }

        let pages = (size / PAGE_SIZE) as usize;
        for i in 0..pages {
            let current_gpa = gpa + (i as u64 * PAGE_SIZE);
            self.unmap_page(current_gpa)?;
        }

        Ok(())
    }

    /// Unmap a single page
    fn unmap_page(&self, gpa: Gpa) -> Result<()> {
        let vpn = self.extract_vpn(gpa, self.level);
        self.clear_pte(vpn)?;
        Ok(())
    }

    /// Translate GPA to HPA
    pub fn translate(&self, gpa: Gpa) -> Result<Hpa> {
        let (pte, _) = self.walk(gpa, false)?;

        if pte.is_valid() && pte.is_leaf() {
            let offset = gpa & (PAGE_SIZE - 1);
            Ok(pte.pa() + offset)
        } else {
            Err(Error::NotFound)
        }
    }

    /// Check permissions for a GPA
    pub fn check_permissions(&self, gpa: Gpa, read: bool, write: bool, execute: bool) -> Result<bool> {
        let (pte, _) = self.walk(gpa, false)?;

        if !pte.is_valid() || !pte.is_leaf() {
            return Ok(false);
        }

        if read && !pte.can_read() {
            return Ok(false);
        }

        if write && !pte.can_write() {
            return Ok(false);
        }

        if execute && !pte.can_execute() {
            return Ok(false);
        }

        // Update accessed bit
        if !pte.is_accessed() {
            let vpn = self.extract_vpn(gpa, self.level);
            let mut modified_pte = pte;
            modified_pte.set_accessed();
            self.set_pte(vpn, modified_pte)?;
        }

        Ok(true)
    }

    /// Flush TLB entries for this page table
    pub fn flush_tlb(&self, gpa: Option<Gpa>, size: Option<u64>) {
        // In a real implementation, this would:
        // 1. Use sfence.vma for local TLB flush
        // 2. Use SBI calls for remote TLB flush
        // 3. Use VMID-specific flush if supported

        crate::debug!("Flushing TLB for VMID {}, GPA {:?}", self.vmid, gpa);

        // For now, just flush all
        #[cfg(target_arch = "riscv64")]
        unsafe {
            core::arch::asm!("sfence.vma");
        }
    }
}

/// G-stage translation context
pub struct GStageContext {
    /// VMID for this context
    pub vmid: Vmid,
    /// Translation mode
    pub mode: GStageMode,
    /// Hardware capabilities
    pub capabilities: GStageCapabilities,
    /// Address space layout
    pub address_space: GStageAddressSpace,
    /// Root page table
    pub root: SpinLock<Option<Box<GStagePageTable>>>,
    /// HGATP register value
    pub hgatp: SpinLock<u64>,
    /// Physical address of root page table
    pub root_pa: SpinLock<Option<PhysAddr>>,
    /// Context statistics
    pub stats: SpinLock<GStageStats>,
}

/// G-stage context statistics
#[derive(Debug, Clone, Default)]
pub struct GStageStats {
    /// Number of page tables allocated
    pub page_tables: u32,
    /// Number of leaf mappings
    pub leaf_mappings: u32,
    /// Number of huge page mappings
    pub huge_mappings: u32,
    /// Number of translation walks
    pub translations: u64,
    /// Number of translation misses
    pub translation_misses: u64,
    /// TLB flush count
    pub tlb_flushes: u32,
}

impl GStageContext {
    /// Create a new G-stage context with hardware capability detection
    pub fn new(vmid: Vmid) -> Self {
        let capabilities = GStageCapabilities::detect();
        let mode = capabilities.best_mode();
        let address_space = GStageAddressSpace::for_mode(mode);

        Self {
            vmid,
            mode,
            capabilities,
            address_space,
            root: SpinLock::new(None),
            hgatp: SpinLock::new(0),
            root_pa: SpinLock::new(None),
            stats: SpinLock::new(GStageStats::default()),
        }
    }

    /// Create a new G-stage context with specific mode
    pub fn new_with_mode(vmid: Vmid, mode: GStageMode) -> Result<Self> {
        let capabilities = GStageCapabilities::detect();

        if !capabilities.supports_mode(mode) {
            return Err(Error::InvalidArgument);
        }

        let address_space = GStageAddressSpace::for_mode(mode);

        Ok(Self {
            vmid,
            mode,
            capabilities,
            address_space,
            root: SpinLock::new(None),
            hgatp: SpinLock::new(0),
            root_pa: SpinLock::new(None),
            stats: SpinLock::new(GStageStats::default()),
        })
    }

    /// Initialize the G-stage context
    pub fn init(&mut self) -> Result<()> {
        // Allocate root page table
        let root_pa = crate::core::mm::frame::alloc_frame()
            .ok_or(Error::OutOfMemory)?;
        let root_va = crate::core::mm::frame::phys_to_virt(root_pa);

        let root = Box::new(GStagePageTable::new(
            GStageLevel::Root,
            self.vmid,
            self.mode,
            root_pa,
            root_va,
        ));

        // Store root page table
        *self.root.lock() = Some(root);
        *self.root_pa.lock() = Some(root_pa);

        // Update statistics
        {
            let mut stats = self.stats.lock();
            stats.page_tables = 1;
        }

        // Configure HGATP register
        self.configure_hgatp()?;

        crate::info!(
            "G-stage context initialized for VMID {}, mode {:?}, address space: {}GB",
            self.vmid,
            self.mode,
            self.address_space.size / (1024 * 1024 * 1024)
        );
        Ok(())
    }

    /// Change the translation mode
    pub fn change_mode(&mut self, new_mode: GStageMode) -> Result<()> {
        if !self.capabilities.supports_mode(new_mode) {
            return Err(Error::InvalidArgument);
        }

        if self.mode == new_mode {
            return Ok(()); // No change needed
        }

        // Save current mappings if needed (complex migration)
        // For now, just reinitialize with new mode
        self.mode = new_mode;
        self.address_space = GStageAddressSpace::for_mode(new_mode);

        // Reinitialize with new mode
        self.init()?;

        crate::info!("G-stage context mode changed to {:?}", new_mode);
        Ok(())
    }

    /// Get context statistics
    pub fn get_stats(&self) -> GStageStats {
        *self.stats.lock()
    }

    /// Reset context statistics
    pub fn reset_stats(&self) {
        *self.stats.lock() = GStageStats::default();
    }

    /// Configure HGATP register for different modes
    fn configure_hgatp(&self) -> Result<()> {
        let root_pa = *self.root_pa.lock();
        if let Some(pa) = root_pa {
            let ppn = pa / PAGE_SIZE;

            // HGATP format depends on the mode
            let hgatp = match self.mode {
                GStageMode::Sv32X4 => {
                    // Sv32X4: [MODE=8] [VMID=10:7] [PPN=31:12]
                    let vmid_field = ((self.vmid as u64) & 0xF) << 7; // 4 bits at position 7
                    let ppn_field = (ppn & 0x000FFFFF) << 12; // 20 bits at position 12
                    let mode_field = 8u64 << 60; // MODE at position 60
                    mode_field | vmid_field | ppn_field
                }
                GStageMode::Sv39X4 | GStageMode::Sv48X4 | GStageMode::Sv57X4 => {
                    // Sv39X4/Sv48X4/Sv57X4: [MODE] [VMID=25:24] [PPN=43:12]
                    let vmid_field = ((self.vmid as u64) & 0x3FF) << 24; // 10 bits at position 24
                    let ppn_field = (ppn & 0x00000FFFFFFFFFFF) << 12; // 32 bits at position 12
                    let mode_field = (self.mode.hgatp_mode() as u64) << 60; // MODE at position 60
                    mode_field | vmid_field | ppn_field
                }
                GStageMode::None => 0, // Bypass mode
            };

            *self.hgatp.lock() = hgatp;

            // Write to HGATP register (would be done in context switch)
            #[cfg(target_arch = "riscv64")]
            unsafe {
                // In a real implementation, this would write to HGATP CSR
                // core::arch::asm!("csrw hgatp, {}", in(reg) hgatp);

                // Also need to update memory management configuration
                // For example, setting the appropriate bits in menvcfg
                // let mut menvcfg: u64;
                // core::arch::asm!("csrr {}, menvcfg", out(reg) menvcfg);
                // menvcfg |= (1 << 62) // Enable Sv57X4 support if needed
                // core::arch::asm!("csrw menvcfg, {}", in(reg) menvcfg);
            }

            crate::debug!("HGATP configured for VMID {}, mode {:?}, value: 0x{:x}",
                         self.vmid, self.mode, hgatp);
            Ok(())
        } else {
            Err(Error::InvalidState)
        }
    }

    /// Get the current HGATP value
    pub fn get_hgatp(&self) -> u64 {
        *self.hgatp.lock()
    }

    /// Map a GPA range to HPA
    pub fn map(&self, gpa: Gpa, hpa: Hpa, size: u64, flags: u64) -> Result<()> {
        let root = self.root.lock();
        if let Some(ref root_table) = *root {
            root_table.map(gpa, hpa, size, flags)
        } else {
            Err(Error::InvalidState)
        }
    }

    /// Unmap a GPA range
    pub fn unmap(&self, gpa: Gpa, size: u64) -> Result<()> {
        let root = self.root.lock();
        if let Some(ref root_table) = *root {
            root_table.unmap(gpa, size)
        } else {
            Err(Error::InvalidState)
        }
    }

    /// Translate GPA to HPA with multi-format support and statistics
    pub fn translate(&self, gpa: Gpa) -> Result<Hpa> {
        // Update statistics
        {
            let mut stats = self.stats.lock();
            stats.translations += 1;
        }

        // Check if GPA is within address space
        if !self.address_space.contains(gpa) {
            let mut stats = self.stats.lock();
            stats.translation_misses += 1;
            return Err(Error::InvalidArgument);
        }

        let root = self.root.lock();
        if let Some(ref root_table) = *root {
            match root_table.translate(gpa) {
                Ok(hpa) => Ok(hpa),
                Err(e) => {
                    // Update miss statistics
                    let mut stats = self.stats.lock();
                    stats.translation_misses += 1;
                    Err(e)
                }
            }
        } else {
            let mut stats = self.stats.lock();
            stats.translation_misses += 1;
            Err(Error::InvalidState)
        }
    }

    /// Translate with multi-format optimized walk
    pub fn translate_optimized(&self, gpa: Gpa) -> Result<Hpa> {
        // Update statistics
        {
            let mut stats = self.stats.lock();
            stats.translations += 1;
        }

        // Check if GPA is within address space
        if !self.address_space.contains(gpa) {
            let mut stats = self.stats.lock();
            stats.translation_misses += 1;
            return Err(Error::InvalidArgument);
        }

        let root = self.root.lock();
        if let Some(ref root_table) = *root {
            match root_table.walk_multi_format(gpa, false) {
                Ok((pte, level, offset)) => {
                    if pte.is_valid() && pte.is_leaf() {
                        let hpa = pte.pa() + offset;
                        Ok(hpa)
                    } else {
                        let mut stats = self.stats.lock();
                        stats.translation_misses += 1;
                        Err(Error::NotFound)
                    }
                }
                Err(e) => {
                    let mut stats = self.stats.lock();
                    stats.translation_misses += 1;
                    Err(e)
                }
            }
        } else {
            let mut stats = self.stats.lock();
            stats.translation_misses += 1;
            Err(Error::InvalidState)
        }
    }

    /// Check permissions for GPA access
    pub fn check_permissions(&self, gpa: Gpa, read: bool, write: bool, execute: bool) -> Result<bool> {
        // Check if GPA is within address space
        if !self.address_space.contains(gpa) {
            return Ok(false);
        }

        let root = self.root.lock();
        if let Some(ref root_table) = *root {
            root_table.check_permissions(gpa, read, write, execute)
        } else {
            Ok(false)
        }
    }

    /// Flush TLB entries with statistics tracking
    pub fn flush_tlb(&self, gpa: Option<Gpa>, size: Option<u64>) {
        // Update statistics
        {
            let mut stats = self.stats.lock();
            stats.tlb_flushes += 1;
        }

        let root = self.root.lock();
        if let Some(ref root_table) = *root {
            // Use VMID-specific flush if supported by hardware
            if self.capabilities.hw_walk {
                crate::debug!("TLB flush for VMID {}, GPA: {:?}, size: {:?}",
                             self.vmid, gpa, size);
            }

            root_table.flush_tlb(gpa, size);
        }

        // Perform actual hardware TLB flush
        #[cfg(target_arch = "riscv64")]
        unsafe {
            if let Some(flush_gpa) = gpa {
                if let Some(flush_size) = size {
                    // Flush specific range
                    let pages = flush_size / PAGE_SIZE;
                    for i in 0..pages {
                        let addr = flush_gpa + (i * PAGE_SIZE);
                        core::arch::asm!("sfence.vma {}, {}", in(reg) addr, in(reg) self.vmid);
                    }
                } else {
                    // Flush single address
                    core::arch::asm!("sfence.vma {}, {}", in(reg) flush_gpa, in(reg) self.vmid);
                }
            } else {
                // Flush all for this VMID
                core::arch::asm!("sfence.vma {}, {}", in(reg) 0, in(reg) self.vmid);
            }
        }
    }

    /// Flush all TLB entries for this VM
    pub fn flush_tlb_all(&self) {
        self.flush_tlb(None, None);
    }
}

/// G-stage manager for managing multiple VM contexts
pub struct GStageManager {
    /// VMID allocation bitmap
    vmid_bitmap: SpinLock<Vec<u32>>,
    /// Maximum VMID value
    max_vmid: Vmid,
    /// G-stage contexts (indexed by VMID)
    contexts: SpinLock<Vec<Option<GStageContext>>>,
    /// Current active VMID
    active_vmid: SpinLock<Option<Vmid>>,
}

impl GStageManager {
    /// Create a new G-stage manager
    pub fn new(max_vmid: Vmid) -> Self {
        let bitmap_size = ((max_vmid + 31) / 32) as usize;
        Self {
            vmid_bitmap: SpinLock::new(vec![0; bitmap_size]),
            max_vmid,
            contexts: SpinLock::new(vec![None; (max_vmid + 1) as usize]),
            active_vmid: SpinLock::new(None),
        }
    }

    /// Allocate a VMID
    pub fn allocate_vmid(&self) -> Result<Vmid> {
        let mut bitmap = self.vmid_bitmap.lock();

        for vmid in 0..=self.max_vmid {
            let word_idx = (vmid / 32) as usize;
            let bit_idx = vmid % 32;

            if word_idx < bitmap.len() && (bitmap[word_idx] & (1 << bit_idx)) == 0 {
                bitmap[word_idx] |= 1 << bit_idx;
                return Ok(vmid);
            }
        }

        Err(Error::ResourceBusy)
    }

    /// Free a VMID
    pub fn free_vmid(&self, vmid: Vmid) -> Result<()> {
        if vmid > self.max_vmid {
            return Err(Error::InvalidArgument);
        }

        let mut bitmap = self.vmid_bitmap.lock();
        let word_idx = (vmid / 32) as usize;
        let bit_idx = vmid % 32;

        if word_idx < bitmap.len() {
            bitmap[word_idx] &= !(1 << bit_idx);

            // Clean up context
            let mut contexts = self.contexts.lock();
            if (vmid as usize) < contexts.len() {
                contexts[vmid as usize] = None;
            }

            Ok(())
        } else {
            Err(Error::InvalidArgument)
        }
    }

    /// Create a new G-stage context with automatic mode detection
    pub fn create_context(&self) -> Result<Vmid> {
        let vmid = self.allocate_vmid()?;
        let mut context = GStageContext::new(vmid);
        context.init()?;

        let mut contexts = self.contexts.lock();
        if (vmid as usize) < contexts.len() {
            contexts[vmid as usize] = Some(context);
            Ok(vmid)
        } else {
            self.free_vmid(vmid)?;
            Err(Error::InvalidState)
        }
    }

    /// Create a new G-stage context with specific mode
    pub fn create_context_with_mode(&self, mode: GStageMode) -> Result<Vmid> {
        let vmid = self.allocate_vmid()?;
        let mut context = GStageContext::new_with_mode(vmid, mode)?;
        context.init()?;

        let mut contexts = self.contexts.lock();
        if (vmid as usize) < contexts.len() {
            contexts[vmid as usize] = Some(context);
            Ok(vmid)
        } else {
            self.free_vmid(vmid)?;
            Err(Error::InvalidState)
        }
    }

    /// Get hardware capabilities
    pub fn get_capabilities(&self) -> GStageCapabilities {
        GStageCapabilities::detect()
    }

    /// Destroy a G-stage context
    pub fn destroy_context(&self, vmid: Vmid) -> Result<()> {
        let mut contexts = self.contexts.lock();
        if (vmid as usize) < contexts.len() {
            contexts[vmid as usize] = None;
            self.free_vmid(vmid)
        } else {
            Err(Error::InvalidArgument)
        }
    }

    /// Get a G-stage context
    pub fn get_context(&self, vmid: Vmid) -> Option<&GStageContext> {
        // This is a simplified version - in practice, you'd need proper reference handling
        let contexts = self.contexts.lock();
        if (vmid as usize) < contexts.len() {
            // Return a reference with extended lifetime - this is unsafe and just for demonstration
            unsafe { core::mem::transmute(contexts[vmid as usize].as_ref()) }
        } else {
            None
        }
    }

    /// Set active VMID
    pub fn set_active_vmid(&self, vmid: Vmid) -> Result<()> {
        if let Some(context) = self.get_context(vmid) {
            // Configure HGATP for active VM
            #[cfg(target_arch = "riscv64")]
            unsafe {
                // core::arch::asm!("csrw hgatp, {}", in(reg) context.get_hgatp());
            }

            *self.active_vmid.lock() = Some(vmid);
            Ok(())
        } else {
            Err(Error::NotFound)
        }
    }

    /// Get active VMID
    pub fn get_active_vmid(&self) -> Option<Vmid> {
        *self.active_vmid.lock()
    }

    /// Translate GPA for active VM
    pub fn translate_active(&self, gpa: Gpa) -> Result<Hpa> {
        if let Some(vmid) = self.get_active_vmid() {
            if let Some(context) = self.get_context(vmid) {
                context.translate(gpa)
            } else {
                Err(Error::NotFound)
            }
        } else {
            Err(Error::InvalidState)
        }
    }
}

/// Global G-stage manager
static mut G_STAGE_MANAGER: Option<GStageManager> = None;
static G_STAGE_MANAGER_INIT: SpinLock<bool> = SpinLock::new(false);

/// Initialize the global G-stage manager
pub fn init(max_vmid: Vmid) -> Result<()> {
    let mut init_guard = G_STAGE_MANAGER_INIT.lock();

    if *init_guard {
        return Ok(());
    }

    let manager = GStageManager::new(max_vmid);
    unsafe {
        G_STAGE_MANAGER = Some(manager);
    }

    *init_guard = true;
    crate::info!("Global G-stage manager initialized with max VMID {}", max_vmid);
    Ok(())
}

/// Get the global G-stage manager
pub fn get() -> Option<&'static GStageManager> {
    unsafe { G_STAGE_MANAGER.as_ref() }
}

/// Get the global G-stage manager (panic if not initialized)
pub fn get_expect() -> &'static GStageManager {
    get().expect("G-stage manager not initialized")
}

/// Get global hardware capabilities
pub fn get_capabilities() -> GStageCapabilities {
    GStageCapabilities::detect()
}

/// Check if a specific page table format is supported
pub fn supports_mode(mode: GStageMode) -> bool {
    get_capabilities().supports_mode(mode)
}

/// Get the best supported page table format
pub fn get_best_mode() -> GStageMode {
    get_capabilities().best_mode()
}

/// Create a G-stage context with automatic mode selection
pub fn create_context_auto() -> Result<Vmid> {
    if let Some(manager) = get() {
        manager.create_context()
    } else {
        Err(Error::InvalidState)
    }
}

/// Create a G-stage context with specific mode
pub fn create_context_with_mode(mode: GStageMode) -> Result<Vmid> {
    if let Some(manager) = get() {
        manager.create_context_with_mode(mode)
    } else {
        Err(Error::InvalidState)
    }
}

/// Translate GPA for active VM with optimized walk
pub fn translate_active_optimized(gpa: Gpa) -> Result<Hpa> {
    if let Some(manager) = get() {
        if let Some(vmid) = manager.get_active_vmid() {
            if let Some(context) = manager.get_context(vmid) {
                context.translate_optimized(gpa)
            } else {
                Err(Error::NotFound)
            }
        } else {
            Err(Error::InvalidState)
        }
    } else {
        Err(Error::InvalidState)
    }
}

/// Flush TLB for all VMs with optimized management
pub fn flush_all_tlbs() {
    if let Some(manager) = get() {
        // Use optimized flush for all VMs
        #[cfg(target_arch = "riscv64")]
        crate::arch::riscv64::mmu::tlb::get_manager_mut()
            .map(|tlb_mgr| tlb_mgr.flush_all());

        #[cfg(not(target_arch = "riscv64"))]
        let _ = manager; // Suppress unused warning
    }
}

/// Perform optimized GPA to HPA translation with TLB integration
pub fn translate_with_tlb_optimization(gpa: Gpa) -> Result<Hpa> {
    if let Some(manager) = get() {
        if let Some(vmid) = manager.get_active_vmid() {
            if let Some(context) = manager.get_context(vmid) {
                // Try optimized translation first
                if let Some(hpa) = translate_active_optimized(gpa) {
                    return Ok(hpa);
                }

                // Fallback to regular translation
                context.translate(gpa)
            } else {
                Err(Error::NotFound)
            }
        } else {
            Err(Error::InvalidState)
        }
    } else {
        Err(Error::InvalidState)
    }
}

/// Perform bulk translation with TLB preloading
#[cfg(target_arch = "riscv64")]
pub fn translate_bulk_with_preloading(gpas: &[Gpa]) -> Vec<Result<Hpa>> {
    let mut results = Vec::with_capacity(gpas.len());

    if let Some(tlb_manager) = crate::arch::riscv64::mmu::tlb::get_manager_mut() {
        if let Some(gstage_manager) = get() {
            if let Some(vmid) = gstage_manager.get_active_vmid() {
                // Preload TLB with entries that are likely to be accessed
                let asid = 0; // G-stage typically uses ASID 0

                for &gpa in gpas {
                    // Try optimized G-stage translation
                    if let Some(hpa) = tlb_manager.translate_gstage_optimized(gpa, asid, vmid) {
                        results.push(Ok(hpa));
                    } else {
                        // Fallback to regular translation and cache the result
                        if let Some(context) = gstage_manager.get_context(vmid) {
                            match context.translate(gpa) {
                                Ok(hpa) => {
                                    // Insert into TLB for future access
                                    let entry = crate::arch::riscv64::mmu::tlb::TlbEntry::new(
                                        gpa as usize,
                                        hpa as usize,
                                        asid,
                                        vmid,
                                        4096, // Page size
                                        crate::arch::riscv64::mmu::tlb::TlbPermissions::READ |
                                        crate::arch::riscv64::mmu::tlb::TlbPermissions::WRITE |
                                        crate::arch::riscv64::mmu::tlb::TlbPermissions::VALID,
                                        crate::arch::riscv64::mmu::tlb::TlbEntryType::GStage,
                                        0,
                                    );
                                    tlb_manager.insert_gstage(entry);
                                    results.push(Ok(hpa));
                                }
                                Err(e) => results.push(Err(e)),
                            }
                        } else {
                            results.push(Err(Error::InvalidState));
                        }
                    }
                }
            } else {
                // No active VM, fill with errors
                results.resize(gpas.len(), Err(Error::InvalidState));
            }
        } else {
            results.resize(gpas.len(), Err(Error::InvalidState));
        }
    } else {
        results.resize(gpas.len(), Err(Error::InvalidState));
    }

    results
}

/// Invalidate GPA range with TLB optimization
#[cfg(target_arch = "riscv64")]
pub fn invalidate_range_optimized(gpa: Gpa, size: u64) -> Result<()> {
    if let Some(manager) = get() {
        if let Some(vmid) = manager.get_active_vmid() {
            // Use optimized range invalidation
            if let Some(tlb_manager) = crate::arch::riscv64::mmu::tlb::get_manager_mut() {
                let invalidated = tlb_manager.invalidate_range_optimized(
                    gpa as usize,
                    size as usize,
                    0, // ASID
                    vmid,
                );

                crate::debug!("Invalidated {} TLB entries for GPA range {:#x}-{:#x}",
                             invalidated, gpa, gpa + size);
            }

            // Also flush G-stage hardware TLB
            manager.flush_tlb(Some(gpa), Some(size));
        }
        Ok(())
    } else {
        Err(Error::InvalidState)
    }
}

/// Get TLB performance report for G-stage
#[cfg(target_arch = "riscv64")]
pub fn get_tlb_performance_report() -> Option<crate::arch::riscv64::mmu::tlb::TlbReport> {
    if let Some(tlb_manager) = crate::arch::riscv64::mmu::tlb::get_manager() {
        let mut report = tlb_manager.generate_report();

        // Filter for G-stage specific metrics
        if let Some(gstage_manager) = get() {
            if let Some(vmid) = gstage_manager.get_active_vmid() {
                // Add VM-specific information
                report.vm_distribution.retain(|(id, _)| *id == vmid);
            }
        }

        Some(report)
    } else {
        None
    }
}

/// Perform periodic TLB maintenance for G-stage
#[cfg(target_arch = "riscv64")]
pub fn perform_tlb_maintenance() {
    if let Some(tlb_manager) = crate::arch::riscv64::mmu::tlb::get_manager_mut() {
        tlb_manager.perform_maintenance();

        // Get health metrics and log if needed
        let health = tlb_manager.get_health_metrics();
        if health.needs_attention() {
            health.print();
        }
    }
}

/// Configure TLB optimization strategy for G-stage workloads
#[cfg(target_arch = "riscv64")]
pub fn configure_tlb_optimization() -> Result<()> {
    if let Some(tlb_manager) = crate::arch::riscv64::mmu::tlb::get_manager_mut() {
        // Configure adaptive strategy for virtualization workloads
        tlb_manager.set_optimization_strategy(
            crate::arch::riscv64::mmu::tlb::TlbOptimizationStrategy::Adaptive
        );

        // Configure coalescing for better VM isolation
        let coalescing_config = crate::arch::riscv64::mmu::tlb::TlbCoalescingConfig {
            enabled: true,
            min_entries: 16, // Higher threshold for virtualization
            max_coalesced_size: 128 * 1024, // 128KB for VM workloads
            hit_rate_threshold: 90.0, // Higher threshold for VM workloads
        };
        tlb_manager.update_coalescing_config(coalescing_config);

        // Configure prefetching for VM access patterns
        let prefetch_config = crate::arch::riscv64::mmu::tlb::TlbPrefetchConfig {
            enabled: true,
            prefetch_distance: 8, // Aggressive prefetch for VM memory access
            prefetch_threshold: 2, // Lower threshold for VM workloads
            max_prefetch_queue: 32, // Larger queue for VM patterns
        };
        tlb_manager.update_prefetch_config(prefetch_config);

        crate::info!("TLB optimization configured for virtualization workloads");
        Ok(())
    } else {
        Err(Error::InvalidState)
    }
}

/// Helper functions for page flag conversion
pub mod flags {
    use super::*;

    /// Convert PageFlags to G-stage flags
    pub fn page_flags_to_gstage(flags: &PageFlags) -> u64 {
        let mut gstage_flags = 0;

        if flags.readable {
            gstage_flags |= gstage_pte::R;
        }

        if flags.writable {
            gstage_flags |= gstage_pte::W;
        }

        if flags.executable {
            gstage_flags |= gstage_pte::X;
        }

        // G-stage always sets U bit for VS-mode access
        gstage_flags |= gstage_pte::U;

        if flags.global {
            gstage_flags |= gstage_pte::G;
        }

        // Set accessed bit initially
        gstage_flags |= gstage_pte::A;

        gstage_flags
    }

    /// Convert G-stage flags to PageFlags
    pub fn gstage_flags_to_page(gstage_flags: u64) -> PageFlags {
        PageFlags {
            present: (gstage_flags & gstage_pte::V) != 0,
            writable: (gstage_flags & gstage_pte::W) != 0,
            executable: (gstage_flags & gstage_pte::X) != 0,
            user: (gstage_flags & gstage_pte::U) != 0,
            write_through: false,
            cache_disable: false,
            accessed: (gstage_flags & gstage_pte::A) != 0,
            dirty: (gstage_flags & gstage_pte::D) != 0,
            global: (gstage_flags & gstage_pte::G) != 0,
            cow: false,
            write_protected: false,
        }
    }

    /// Default G-stage flags for normal pages
    pub const fn default_gstage_flags() -> u64 {
        gstage_pte::R | gstage_pte::W | gstage_pte::X | gstage_pte::U | gstage_pte::A
    }

    /// G-stage flags for read-only pages
    pub const fn readonly_gstage_flags() -> u64 {
        gstage_pte::R | gstage_pte::X | gstage_pte::U | gstage_pte::A
    }

    /// G-stage flags for executable-only pages
    pub const fn exec_only_gstage_flags() -> u64 {
        gstage_pte::X | gstage_pte::U | gstage_pte::A
    }
}