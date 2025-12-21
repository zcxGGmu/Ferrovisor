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
        let entry_count = 512; // RISC-V page tables have 512 entries
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

    /// Get the number of entries per level
    pub fn entries_per_level(&self) -> usize {
        512 // RISC-V standard: 512 entries per level
    }

    /// Get the virtual bits per level
    pub fn bits_per_level(&self) -> u32 {
        9 // log2(512) = 9
    }

    /// Extract VPN for a given level
    pub fn extract_vpn(&self, gpa: Gpa, level: GStageLevel) -> usize {
        let vpn_shift = PAGE_SHIFT + (self.bits_per_level() * (self.mode.levels() as u32 - level.index() as u32 - 1));
        let vpn = (gpa >> vpn_shift) & ((1u64 << self.bits_per_level()) - 1);
        vpn as usize
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

    /// Walk the page table to find or create an entry
    pub fn walk(&self, gpa: Gpa, create: bool) -> Result<(GStagePte, Option<GStageLevel>)> {
        let vpn = self.extract_vpn(gpa, self.level);
        let pte = self.get_pte(vpn)?;

        if !pte.is_valid() {
            if create {
                // Need to create next level page table
                if self.level.index() < (self.mode.levels() as usize - 1) {
                    return Err(Error::NotImplemented);
                } else {
                    // This is the leaf level, return invalid PTE
                    return Ok((pte, None));
                }
            } else {
                return Ok((pte, None));
            }
        }

        if pte.is_leaf() {
            Ok((pte, Some(self.level)))
        } else {
            // This is a branch, continue walking
            Ok((pte, Some(self.level)))
        }
    }

    /// Map a GPA to HPA with specified permissions
    pub fn map(&self, gpa: Gpa, hpa: Hpa, size: u64, flags: u64) -> Result<()> {
        if !self.is_aligned(gpa, size) || !self.is_aligned(hpa, size) {
            return Err(Error::InvalidArgument);
        }

        let pages = (size / PAGE_SIZE) as usize;
        for i in 0..pages {
            let current_gpa = gpa + (i as u64 * PAGE_SIZE);
            let current_hpa = hpa + (i as u64 * PAGE_SIZE);
            self.map_page(current_gpa, current_hpa, flags)?;
        }

        Ok(())
    }

    /// Map a single page
    fn map_page(&self, gpa: Gpa, hpa: Hpa, flags: u64) -> Result<()> {
        // For now, implement simple mapping at leaf level
        // In a full implementation, this would handle multi-level page tables
        let ppn = hpa / PAGE_SIZE;
        let pte = GStagePte::leaf(ppn, flags);

        let vpn = self.extract_vpn(gpa, self.level);
        self.set_pte(vpn, pte)?;

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
    /// Root page table
    pub root: SpinLock<Option<Box<GStagePageTable>>>,
    /// HGATP register value
    pub hgatp: SpinLock<u64>,
    /// Physical address of root page table
    pub root_pa: SpinLock<Option<PhysAddr>>,
}

impl GStageContext {
    /// Create a new G-stage context
    pub fn new(vmid: Vmid, mode: GStageMode) -> Self {
        Self {
            vmid,
            mode,
            root: SpinLock::new(None),
            hgatp: SpinLock::new(0),
            root_pa: SpinLock::new(None),
        }
    }

    /// Initialize the G-stage context
    pub fn init(&mut self) -> Result<()> {
        // Allocate root page table
        let root_pa = crate::core::mm::frame::alloc_frame()?;
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

        // Configure HGATP register
        self.configure_hgatp()?;

        crate::info!("G-stage context initialized for VMID {}, mode {:?}", self.vmid, self.mode);
        Ok(())
    }

    /// Configure HGATP register
    fn configure_hgatp(&self) -> Result<()> {
        let root_pa = *self.root_pa.lock();
        if let Some(pa) = root_pa {
            let ppn = pa / PAGE_SIZE;
            let hgatp = (ppn << 44) | ((self.vmid as u64) << 44) | ((self.mode.hgatp_mode() as u64) << 60);

            *self.hgatp.lock() = hgatp;

            // Write to HGATP register (would be done in context switch)
            #[cfg(target_arch = "riscv64")]
            unsafe {
                // In a real implementation, this would write to HGATP CSR
                // core::arch::asm!("csrw hgatp, {}", in(reg) hgatp);
            }

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

    /// Translate GPA to HPA
    pub fn translate(&self, gpa: Gpa) -> Result<Hpa> {
        let root = self.root.lock();
        if let Some(ref root_table) = *root {
            root_table.translate(gpa)
        } else {
            Err(Error::InvalidState)
        }
    }

    /// Check permissions for GPA access
    pub fn check_permissions(&self, gpa: Gpa, read: bool, write: bool, execute: bool) -> Result<bool> {
        let root = self.root.lock();
        if let Some(ref root_table) = *root {
            root_table.check_permissions(gpa, read, write, execute)
        } else {
            Ok(false)
        }
    }

    /// Flush TLB entries
    pub fn flush_tlb(&self, gpa: Option<Gpa>, size: Option<u64>) {
        let root = self.root.lock();
        if let Some(ref root_table) = *root {
            root_table.flush_tlb(gpa, size);
        }
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

    /// Create a new G-stage context
    pub fn create_context(&self, mode: GStageMode) -> Result<Vmid> {
        let vmid = self.allocate_vmid()?;
        let mut context = GStageContext::new(vmid, mode);
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