//! Guest Address Space Management
//!
//! This module provides guest address space management for virtualization:
//! - Guest physical memory management
//! - G-stage page table allocation and management
//! - Memory mapping and protection for guests
//! - Integration with G-stage address translation

use crate::arch::riscv64::*;
use crate::arch::riscv64::mmu::ptable::*;
use crate::arch::riscv64::mmu::gstage::*;
use crate::arch::riscv64::mmu::memory::*;
use bitflags::bitflags;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Guest memory region
#[derive(Debug, Clone)]
pub struct GuestMemoryRegion {
    /// Guest physical address
    pub gpa: usize,
    /// Host physical address
    pub hpa: usize,
    /// Size in bytes
    pub size: usize,
    /// Memory flags
    pub flags: GuestMemoryFlags,
    /// Region name (for debugging)
    pub name: String,
}

/// Guest memory flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct GuestMemoryFlags: u32 {
        const READABLE = 1 << 0;
        const WRITABLE = 1 << 1;
        const EXECUTABLE = 1 << 2;
        const DEVICE = 1 << 3;
        const UNCACHED = 1 << 4;
        const SHARED = 1 << 5;
    }
}

/// Guest address space
pub struct GuestAddressSpace {
    /// VMID
    vmid: u16,
    /// G-stage mode
    mode: GStageMode,
    /// Root page table physical address
    root_pt_pa: usize,
    /// Guest memory regions
    regions: Vec<GuestMemoryRegion>,
    /// Next available GPA
    next_gpa: AtomicUsize,
    /// G-stage translator
    translator: GStageTranslator,
    /// Statistics
    stats: GuestSpaceStats,
}

/// Guest address space statistics
#[derive(Debug, Default)]
pub struct GuestSpaceStats {
    pub regions_mapped: AtomicUsize,
    pub total_mapped_bytes: AtomicUsize,
    pub page_table_allocations: AtomicUsize,
    pub translations: AtomicUsize,
    pub cache_hits: AtomicUsize,
    pub cache_misses: AtomicUsize,
}

impl GuestAddressSpace {
    /// Create a new guest address space
    pub fn new(vmid: u16, mode: GStageMode) -> Result<Self, &'static str> {
        // Allocate root page table for G-stage
        let root_pt_pa = Self::allocate_page_table(mode.levels())?;

        // Initialize root page table
        Self::initialize_page_table(root_pt_pa)?;

        // Create G-stage translator
        let translator = GStageTranslator::new(vmid, root_pt_pa, mode);

        Ok(Self {
            vmid,
            mode,
            root_pt_pa,
            regions: Vec::new(),
            next_gpa: AtomicUsize::new(0x10000000), // Start from 256MB
            translator,
            stats: GuestSpaceStats::default(),
        })
    }

    /// Allocate a page table
    fn allocate_page_table(levels: usize) -> Result<usize, &'static str> {
        // In a real implementation, this would allocate physical pages
        // For now, simulate allocation
        let pt_pa = 0x80000000 + (levels * 0x1000);
        log::debug!("Allocated page table at PA {:#x} with {} levels", pt_pa, levels);
        Ok(pt_pa)
    }

    /// Initialize a page table (clear all entries)
    fn initialize_page_table(pt_pa: usize) -> Result<(), &'static str> {
        // In a real implementation, this would clear the page table memory
        log::debug!("Initialized page table at PA {:#x}", pt_pa);
        Ok(())
    }

    /// Map a memory region for the guest
    pub fn map_region(&mut self, gpa: usize, hpa: usize, size: usize,
                     flags: GuestMemoryFlags, name: &str) -> Result<(), &'static str> {
        log::debug!("Mapping guest region: GPA {:#x} -> HPA {:#x}, size {:#x}, flags {:?}",
                   gpa, hpa, size, flags);

        // Validate alignment
        if gpa & 0xFFF != 0 || hpa & 0xFFF != 0 || size & 0xFFF != 0 {
            return Err("Addresses and size must be 4KB aligned");
        }

        // Create guest memory region
        let region = GuestMemoryRegion {
            gpa,
            hpa,
            size,
            flags,
            name: name.to_string(),
        };

        // Map in G-stage page tables
        self.map_in_gstage(&region)?;

        // Add to regions list
        self.regions.push(region);

        // Update statistics
        self.stats.regions_mapped.fetch_add(1, Ordering::Relaxed);
        self.stats.total_mapped_bytes.fetch_add(size, Ordering::Relaxed);

        Ok(())
    }

    /// Map region in G-stage page tables
    fn map_in_gstage(&mut self, region: &GuestMemoryRegion) -> Result<(), &'static str> {
        let pages = region.size / 4096;
        let mut gpa = region.gpa;
        let mut hpa = region.hpa;

        for _ in 0..pages {
            // Create PTE with appropriate permissions
            let mut pte = gstage_pte::V;

            if region.flags.contains(GuestMemoryFlags::READABLE) {
                pte |= gstage_pte::R;
            }
            if region.flags.contains(GuestMemoryFlags::WRITABLE) {
                pte |= gstage_pte::W;
            }
            if region.flags.contains(GuestMemoryFlags::EXECUTABLE) {
                pte |= gstage_pte::X;
            }
            if region.flags.contains(GuestMemoryFlags::DEVICE) {
                // Mark as device memory
                pte |= gstage_pte::RWX; // Typically devices have all permissions
            }

            // Set PPN (physical page number)
            let ppn = hpa >> 12;
            pte |= ppn << 10;

            // Map the page in G-stage page tables
            self.map_page_in_gstage(gpa, pte)?;

            gpa += 4096;
            hpa += 4096;
        }

        Ok(())
    }

    /// Map a single page in G-stage page tables
    fn map_page_in_gstage(&mut self, gpa: usize, pte: usize) -> Result<(), &'static str> {
        match self.mode {
            GStageMode::Sv39x4 => self.map_page_sv39x4(gpa, pte),
            GStageMode::Sv48x4 => self.map_page_sv48x4(gpa, pte),
            GStageMode::Sv32x4 => self.map_page_sv32x4(gpa, pte),
            GStageMode::Bare => {
                // No mapping needed for bare metal mode
                Ok(())
            }
        }
    }

    /// Map page using Sv39x4 format
    fn map_page_sv39x4(&mut self, gpa: usize, pte: usize) -> Result<(), &'static str> {
        let vpn = [
            (gpa >> 12) & 0x1FF,  // VPN [11:0]
            (gpa >> 21) & 0x1FF,  // VPN [20:12]
            (gpa >> 30) & 0x1FF,  // VPN [29:21]
        ];

        // Allocate and populate page table levels as needed
        let level0_pt = self.root_pt_pa;
        let level1_pt = self.ensure_page_table_level(level0_pt, vpn[2])?;
        let level2_pt = self.ensure_page_table_level(level1_pt, vpn[1])?;

        // Map final PTE in level 2
        let pte_addr = level2_pt + (vpn[0] * 8);
        self.write_pte(pte_addr, pte)?;

        Ok(())
    }

    /// Map page using Sv48x4 format (simplified)
    fn map_page_sv48x4(&mut self, gpa: usize, pte: usize) -> Result<(), &'static str> {
        // For now, fall back to Sv39x4 mapping
        self.map_page_sv39x4(gpa, pte)
    }

    /// Map page using Sv32x4 format
    fn map_page_sv32x4(&mut self, gpa: usize, pte: usize) -> Result<(), &'static str> {
        let vpn = [
            (gpa >> 12) & 0x3FF,  // VPN [9:0]
            (gpa >> 22) & 0x3FF,  // VPN [19:10]
        ];

        // Allocate and populate page table levels
        let level0_pt = self.root_pt_pa;
        let level1_pt = self.ensure_page_table_level(level0_pt, vpn[1])?;

        // Map final PTE in level 1
        let pte_addr = level1_pt + (vpn[0] * 8);
        self.write_pte(pte_addr, pte)?;

        Ok(())
    }

    /// Ensure a page table level exists
    fn ensure_page_table_level(&mut self, parent_pt_pa: usize, vpn: usize) -> Result<usize, &'static str> {
        let pte_addr = parent_pt_pa + (vpn * 8);
        let existing_pte = self.read_pte(pte_addr)?;

        if (existing_pte & gstage_pte::V) == 0 {
            // Allocate new page table
            let new_pt_pa = Self::allocate_page_table(1)?;
            Self::initialize_page_table(new_pt_pa)?;

            // Create PTE pointing to new page table
            let new_pte = gstage_pte::V | ((new_pt_pa >> 12) << 10);
            self.write_pte(pte_addr, new_pte)?;

            self.stats.page_table_allocations.fetch_add(1, Ordering::Relaxed);

            Ok(new_pt_pa)
        } else {
            // Use existing page table
            let ppn = (existing_pte >> 10) & 0xFFFFFFFFFFF;
            Ok(ppn << 12)
        }
    }

    /// Read a PTE from physical memory
    fn read_pte(&self, pte_addr: usize) -> Result<usize, &'static str> {
        // In a real implementation, this would read from physical memory
        // For simulation, return a non-valid PTE
        Ok(0)
    }

    /// Write a PTE to physical memory
    fn write_pte(&self, pte_addr: usize, pte: usize) -> Result<(), &'static str> {
        // In a real implementation, this would write to physical memory
        log::debug!("Writing PTE {:#x} to address {:#x}", pte, pte_addr);
        Ok(())
    }

    /// Unmap a memory region
    pub fn unmap_region(&mut self, gpa: usize, size: usize) -> Result<(), &'static str> {
        log::debug!("Unmapping guest region: GPA {:#x}, size {:#x}", gpa, size);

        // Find and remove the region
        let index = self.regions.iter().position(|r| r.gpa == gpa && r.size == size)
            .ok_or("Region not found")?;

        let region = self.regions.remove(index);

        // Unmap from G-stage page tables
        self.unmap_in_gstage(&region)?;

        // Update statistics
        self.stats.regions_mapped.fetch_sub(1, Ordering::Relaxed);
        self.stats.total_mapped_bytes.fetch_sub(size, Ordering::Relaxed);

        Ok(())
    }

    /// Unmap region from G-stage page tables
    fn unmap_in_gstage(&mut self, region: &GuestMemoryRegion) -> Result<(), &'static str> {
        let pages = region.size / 4096;
        let mut gpa = region.gpa;

        for _ in 0..pages {
            self.unmap_page_in_gstage(gpa)?;
            gpa += 4096;
        }

        Ok(())
    }

    /// Unmap a single page from G-stage
    fn unmap_page_in_gstage(&mut self, gpa: usize) -> Result<(), &'static str> {
        // Clear the PTE to make it invalid
        match self.mode {
            GStageMode::Sv39x4 => self.unmap_page_sv39x4(gpa),
            GStageMode::Sv48x4 => self.unmap_page_sv48x4(gpa),
            GStageMode::Sv32x4 => self.unmap_page_sv32x4(gpa),
            GStageMode::Bare => Ok(()),
        }
    }

    /// Unmap page using Sv39x4 format
    fn unmap_page_sv39x4(&mut self, gpa: usize) -> Result<(), &'static str> {
        let vpn = [
            (gpa >> 12) & 0x1FF,
            (gpa >> 21) & 0x1FF,
            (gpa >> 30) & 0x1FF,
        ];

        // Navigate to the final PTE and clear it
        let level0_pt = self.root_pt_pa;
        let level1_pt = self.get_page_table_level(level0_pt, vpn[2])?;
        let level2_pt = self.get_page_table_level(level1_pt, vpn[1])?;

        let pte_addr = level2_pt + (vpn[0] * 8);
        self.write_pte(pte_addr, 0)?;

        Ok(())
    }

    /// Unmap page using Sv48x4 format (simplified)
    fn unmap_page_sv48x4(&mut self, gpa: usize) -> Result<(), &'static str> {
        self.unmap_page_sv39x4(gpa)
    }

    /// Unmap page using Sv32x4 format
    fn unmap_page_sv32x4(&mut self, gpa: usize) -> Result<(), &'static str> {
        let vpn = [
            (gpa >> 12) & 0x3FF,
            (gpa >> 22) & 0x3FF,
        ];

        let level0_pt = self.root_pt_pa;
        let level1_pt = self.get_page_table_level(level0_pt, vpn[1])?;

        let pte_addr = level1_pt + (vpn[0] * 8);
        self.write_pte(pte_addr, 0)?;

        Ok(())
    }

    /// Get existing page table level
    fn get_page_table_level(&self, parent_pt_pa: usize, vpn: usize) -> Result<usize, &'static str> {
        let pte_addr = parent_pt_pa + (vpn * 8);
        let pte = self.read_pte(pte_addr)?;

        if (pte & gstage_pte::V) == 0 {
            return Err("Page table level not allocated");
        }

        let ppn = (pte >> 10) & 0xFFFFFFFFFFF;
        Ok(ppn << 12)
    }

    /// Allocate guest physical address space
    pub fn allocate_gpa(&mut self, size: usize) -> Result<usize, &'static str> {
        // Align to page boundary
        let aligned_size = (size + 4095) & !4095;

        let gpa = self.next_gpa.fetch_add(aligned_size, Ordering::Relaxed);

        log::debug!("Allocated GPA space: {:#x}, size {:#x}", gpa, aligned_size);

        Ok(gpa)
    }

    /// Translate guest physical address to host physical address
    pub fn translate(&self, gpa: usize) -> Result<GStageTranslationResult, GStageFault> {
        self.stats.translations.fetch_add(1, Ordering::Relaxed);
        self.translator.translate(gpa)
    }

    /// Get G-stage translator
    pub fn get_translator(&self) -> &GStageTranslator {
        &self.translator
    }

    /// Get VMID
    pub fn get_vmid(&self) -> u16 {
        self.vmid
    }

    /// Get G-stage mode
    pub fn get_mode(&self) -> GStageMode {
        self.mode
    }

    /// Get root page table address
    pub fn get_root_pt_pa(&self) -> usize {
        self.root_pt_pa
    }

    /// Get guest memory regions
    pub fn get_regions(&self) -> &[GuestMemoryRegion] {
        &self.regions
    }

    /// Get statistics
    pub fn get_stats(&self) -> GuestSpaceStatsSnapshot {
        GuestSpaceStatsSnapshot {
            regions_mapped: self.stats.regions_mapped.load(Ordering::Relaxed),
            total_mapped_bytes: self.stats.total_mapped_bytes.load(Ordering::Relaxed),
            page_table_allocations: self.stats.page_table_allocations.load(Ordering::Relaxed),
            translations: self.stats.translations.load(Ordering::Relaxed),
            gstage_stats: self.translator.get_stats(),
        }
    }

    /// Activate this guest address space (configure HGATP)
    pub fn activate(&self) -> Result<(), &'static str> {
        log::info!("Activating guest address space for VMID {}", self.vmid);

        // Configure G-stage translator
        self.translator.configure_hgatp(self.vmid, self.root_pt_pa, self.mode);

        // Invalidate G-stage TLB for this VM
        self.translator.invalidate_tlb(0, usize::MAX);

        log::info!("Guest address space activated successfully");
        Ok(())
    }

    /// Create a simple memory mapping (convenience method)
    pub fn map_memory(&mut self, gpa: usize, hpa: usize, size: usize,
                     readable: bool, writable: bool, executable: bool,
                     name: &str) -> Result<(), &'static str> {
        let mut flags = GuestMemoryFlags::empty();
        if readable { flags |= GuestMemoryFlags::READABLE; }
        if writable { flags |= GuestMemoryFlags::WRITABLE; }
        if executable { flags |= GuestMemoryFlags::EXECUTABLE; }

        self.map_region(gpa, hpa, size, flags, name)
    }

    /// Create a device memory mapping
    pub fn map_device(&mut self, gpa: usize, hpa: usize, size: usize, name: &str) -> Result<(), &'static str> {
        let flags = GuestMemoryFlags::READABLE | GuestMemoryFlags::WRITABLE | GuestMemoryFlags::DEVICE;
        self.map_region(gpa, hpa, size, flags, name)
    }
}

/// Snapshot of guest address space statistics
#[derive(Debug, Clone, Copy)]
pub struct GuestSpaceStatsSnapshot {
    pub regions_mapped: usize,
    pub total_mapped_bytes: usize,
    pub page_table_allocations: usize,
    pub translations: usize,
    pub gstage_stats: GStageStats,
}

impl Default for GuestAddressSpace {
    fn default() -> Self {
        Self::new(0, GStageMode::Sv39x4).unwrap()
    }
}

/// Guest address space manager
pub struct GuestSpaceManager {
    /// Guest address spaces
    spaces: Vec<GuestAddressSpace>,
    /// Next VMID to allocate
    next_vmid: u16,
}

impl GuestSpaceManager {
    /// Create a new guest space manager
    pub fn new() -> Self {
        Self {
            spaces: Vec::new(),
            next_vmid: 1,
        }
    }

    /// Create a new guest address space
    pub fn create_space(&mut self, mode: GStageMode) -> Result<&mut GuestAddressSpace, &'static str> {
        if self.next_vmid == 0 {
            return Err("No more VMIDs available");
        }

        let vmid = self.next_vmid;
        self.next_vmid += 1;

        let space = GuestAddressSpace::new(vmid, mode)?;
        self.spaces.push(space);

        Ok(&mut self.spaces[self.spaces.len() - 1])
    }

    /// Get guest address space by VMID
    pub fn get_space(&mut self, vmid: u16) -> Option<&mut GuestAddressSpace> {
        self.spaces.iter_mut().find(|s| s.get_vmid() == vmid)
    }

    /// Remove a guest address space
    pub fn remove_space(&mut self, vmid: u16) -> Result<(), &'static str> {
        let index = self.spaces.iter().position(|s| s.get_vmid() == vmid)
            .ok_or("Guest space not found")?;

        self.spaces.remove(index);
        log::info!("Removed guest address space for VMID {}", vmid);
        Ok(())
    }

    /// Get all guest spaces
    pub fn get_spaces(&self) -> &[GuestAddressSpace] {
        &self.spaces
    }

    /// Get number of guest spaces
    pub fn count(&self) -> usize {
        self.spaces.len()
    }
}

impl Default for GuestSpaceManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global guest space manager
static mut GUEST_SPACE_MANAGER: Option<GuestSpaceManager> = None;

/// Initialize guest space manager
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing guest address space manager");

    let manager = GuestSpaceManager::new();

    unsafe {
        GUEST_SPACE_MANAGER = Some(manager);
    }

    log::info!("Guest address space manager initialized successfully");
    Ok(())
}

/// Get the global guest space manager
pub fn get_manager() -> Option<&'static GuestSpaceManager> {
    unsafe { GUEST_SPACE_MANAGER.as_ref() }
}

/// Get mutable reference to global guest space manager
pub fn get_manager_mut() -> Option<&'static mut GuestSpaceManager> {
    unsafe { GUEST_SPACE_MANAGER.as_mut() }
}

/// Create a new guest address space
pub fn create_guest_space(mode: GStageMode) -> Result<u16, &'static str> {
    if let Some(manager) = get_manager_mut() {
        let space = manager.create_space(mode)?;
        Ok(space.get_vmid())
    } else {
        Err("Guest space manager not initialized")
    }
}

/// Get guest address space by VMID
pub fn get_guest_space(vmid: u16) -> Option<&'static GuestAddressSpace> {
    // Note: This returns a reference with lifetime issues in real code
    // In practice, you'd need more sophisticated lifetime management
    get_manager().and_then(|m| m.get_spaces().iter().find(|s| s.get_vmid() == vmid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guest_address_space_creation() {
        let space = GuestAddressSpace::new(1, GStageMode::Sv39x4).unwrap();

        assert_eq!(space.get_vmid(), 1);
        assert_eq!(space.get_mode(), GStageMode::Sv39x4);
        assert_eq!(space.get_regions().len(), 0);
    }

    #[test]
    fn test_guest_memory_mapping() {
        let mut space = GuestAddressSpace::new(1, GStageMode::Sv39x4).unwrap();

        let gpa = 0x10000000;
        let hpa = 0x80000000;
        let size = 0x1000; // 4KB

        let result = space.map_memory(
            gpa, hpa, size,
            true, false, false, // readable only
            "test_memory"
        );

        assert!(result.is_ok());
        assert_eq!(space.get_regions().len(), 1);

        let region = &space.get_regions()[0];
        assert_eq!(region.gpa, gpa);
        assert_eq!(region.hpa, hpa);
        assert_eq!(region.size, size);
        assert!(region.flags.contains(GuestMemoryFlags::READABLE));
        assert!(!region.flags.contains(GuestMemoryFlags::WRITABLE));
    }

    #[test]
    fn test_device_mapping() {
        let mut space = GuestAddressSpace::new(1, GStageMode::Sv39x4).unwrap();

        let result = space.map_device(0x10000000, 0x10010000, 0x1000, "uart");
        assert!(result.is_ok());

        let region = &space.get_regions()[0];
        assert!(region.flags.contains(GuestMemoryFlags::DEVICE));
        assert!(region.flags.contains(GuestMemoryFlags::READABLE));
        assert!(region.flags.contains(GuestMemoryFlags::WRITABLE));
    }

    #[test]
    fn test_guest_space_manager() {
        let mut manager = GuestSpaceManager::new();

        let vmid1 = manager.create_space(GStageMode::Sv39x4).unwrap().get_vmid();
        let vmid2 = manager.create_space(GStageMode::Sv39x4).unwrap().get_vmid();

        assert_eq!(vmid1, 1);
        assert_eq!(vmid2, 2);
        assert_eq!(manager.count(), 2);

        // Test retrieval
        let space = manager.get_space(vmid1);
        assert!(space.is_some());
        assert_eq!(space.unwrap().get_vmid(), vmid1);

        // Test removal
        manager.remove_space(vmid1).unwrap();
        assert_eq!(manager.count(), 1);
        assert!(manager.get_space(vmid1).is_none());
    }

    #[test]
    fn test_memory_flags() {
        let flags = GuestMemoryFlags::READABLE | GuestMemoryFlags::WRITABLE | GuestMemoryFlags::EXECUTABLE;

        assert!(flags.contains(GuestMemoryFlags::READABLE));
        assert!(flags.contains(GuestMemoryFlags::WRITABLE));
        assert!(flags.contains(GuestMemoryFlags::EXECUTABLE));
        assert!(!flags.contains(GuestMemoryFlags::DEVICE));
    }
}