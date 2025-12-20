//! RISC-V Address Translation
//!
//! This module provides address translation functionality for RISC-V including:
//! - Virtual to physical address translation
//! - Two-stage translation for virtualization
//! - TLB management
//! - Address space management

use crate::arch::riscv64::*;
use crate::arch::riscv64::mmu::ptable::*;

/// Address space identifier management
pub struct AsidManager {
    next_asid: Asid,
    max_asid: Asid,
}

impl AsidManager {
    /// Create a new ASID manager
    pub fn new(max_asid: Asid) -> Self {
        Self {
            next_asid: 1, // ASID 0 is reserved
            max_asid,
        }
    }

    /// Allocate a new ASID
    pub fn allocate(&mut self) -> Result<Asid, &'static str> {
        if self.next_asid > self.max_asid {
            return Err("No available ASID");
        }

        let asid = self.next_asid;
        self.next_asid += 1;
        Ok(asid)
    }

    /// Free an ASID
    pub fn free(&mut self, asid: Asid) {
        // TODO: Implement ASID recycling
        log::debug!("Freed ASID {}", asid);
    }
}

/// Translation result
#[derive(Debug, Clone, Copy)]
pub struct TranslationResult {
    /// Physical address
    pub pa: usize,
    /// Access permissions
    pub flags: PteFlags,
    /// Was the translation successful?
    pub success: bool,
    /// Was it a superpage mapping?
    pub superpage: bool,
    /// Page size (4KB, 2MB, 1GB, or 512GB)
    pub page_size: usize,
}

impl TranslationResult {
    /// Create a failed translation
    pub fn failed() -> Self {
        Self {
            pa: 0,
            flags: PteFlags::empty(),
            success: false,
            superpage: false,
            page_size: 0,
        }
    }

    /// Create a successful translation
    pub fn success(pa: usize, flags: PteFlags, superpage: bool, page_size: usize) -> Self {
        Self {
            pa,
            flags,
            success: true,
            superpage,
            page_size,
        }
    }
}

/// Single-stage address translation (no virtualization)
pub fn translate_single_stage(
    root_ppn: usize,
    va: usize,
    mode: u8,
) -> TranslationResult {
    let levels = match mode {
        8 => 3, // Sv39
        9 => 4, // Sv48
        _ => return TranslationResult::failed(),
    };

    // Get the root page table
    let root_pa = root_ppn << PAGE_SHIFT;
    let root_table = unsafe { &*(root_pa as *const PageTable) };

    // Perform the walk
    let vpn_levels = [
        (va >> 12) & 0x1FF,  // Level 0 (4KB)
        (va >> 21) & 0x1FF,  // Level 1 (2MB)
        (va >> 30) & 0x1FF,  // Level 2 (1GB)
        (va >> 39) & 0x1FF,  // Level 3 (512GB)
    ];

    let mut current_table = root_table;

    for level in (0..levels).rev() {
        let vpn = vpn_levels[level];
        let entry = current_table.entry(vpn);

        if !entry.is_valid() {
            return TranslationResult::failed();
        }

        if level == 0 {
            // Leaf level - 4KB page
            let flags = entry.flags();
            let pa = entry.pa() | (va & (PAGE_SIZE - 1));
            return TranslationResult::success(pa, flags, false, PAGE_SIZE);
        } else if entry.is_leaf() {
            // Superpage mapping
            let page_size = PAGE_SIZE << (10 * level);
            let page_mask = page_size - 1;
            let flags = entry.flags();
            let pa = entry.pa() | (va & page_mask);
            return TranslationResult::success(pa, flags, true, page_size);
        } else {
            // Navigate to next level
            let next_pa = entry.pa();
            current_table = unsafe { &*(next_pa as *const PageTable) };
        }
    }

    TranslationResult::failed()
}

/// Two-stage address translation (with virtualization)
#[derive(Debug)]
pub struct TwoStageTranslation {
    /// Stage 1 translation (GVA -> GPA)
    pub gpa: TranslationResult,
    /// Stage 2 translation (GPA -> HPA)
    pub hpa: TranslationResult,
    /// Overall success status
    pub success: bool,
}

/// Two-stage address translation (GVA -> GPA -> HPA)
pub fn translate_two_stage(
    g_root_ppn: usize,
    g_mode: u8,
    h_root_ppn: usize,
    h_mode: u8,
    gva: usize,
) -> TwoStageTranslation {
    // Stage 1: GVA -> GPA
    let gpa = translate_single_stage(g_root_ppn, gva, g_mode);

    if !gpa.success {
        return TwoStageTranslation {
            gpa,
            hpa: TranslationResult::failed(),
            success: false,
        };
    }

    // Stage 2: GPA -> HPA
    let hpa = translate_single_stage(h_root_ppn, gpa.pa, h_mode);

    TwoStageTranslation {
        gpa,
        hpa,
        success: hpa.success,
    }
}

/// Check if a virtual address is aligned to page boundary
pub fn is_page_aligned(va: usize, page_size: usize) -> bool {
    (va & (page_size - 1)) == 0
}

/// Align virtual address down to page boundary
pub fn page_align_down(va: usize, page_size: usize) -> usize {
    va & !(page_size - 1)
}

/// Align virtual address up to page boundary
pub fn page_align_up(va: usize, page_size: usize) -> usize {
    ((va + page_size - 1) & !(page_size - 1))
}

/// Get the page number containing this virtual address
pub fn vpn_from_va(va: usize, page_shift: usize) -> usize {
    va >> page_shift
}

/// Get the page offset within the page
pub fn page_offset(va: usize, page_size: usize) -> usize {
    va & (page_size - 1)
}

/// Check if virtual addresses are in the same page
pub fn same_page(va1: usize, va2: usize, page_size: usize) -> bool {
    page_align_down(va1, page_size) == page_align_down(va2, page_size)
}

/// Range of virtual addresses
#[derive(Debug, Clone, Copy)]
pub struct VaRange {
    /// Start virtual address (inclusive)
    pub start: usize,
    /// End virtual address (exclusive)
    pub end: usize,
}

impl VaRange {
    /// Create a new range
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Create a range from start and size
    pub fn from_size(start: usize, size: usize) -> Self {
        Self {
            start,
            end: start + size,
        }
    }

    /// Get the size of the range
    pub fn size(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Check if the range is empty
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    /// Check if an address is in this range
    pub fn contains(&self, va: usize) -> bool {
        va >= self.start && va < self.end
    }

    /// Split range at page boundaries
    pub fn split_at_page(&self, page_size: usize) -> Option<(VaRange, VaRange)> {
        if self.is_empty() || is_page_aligned(self.start, page_size) {
            return None;
        }

        let page_end = page_align_up(self.start, page_size);
        if page_end >= self.end {
            return None;
        }

        Some((
            VaRange::new(self.start, page_end),
            VaRange::new(page_end, self.end),
        ))
    }

    /// Iterate over page-aligned sub-ranges
    pub fn page_iter(&self, page_size: usize) -> PageIter {
        PageIter {
            current: page_align_down(self.start, page_size),
            end: page_align_up(self.end, page_size),
            page_size,
        }
    }
}

/// Iterator over page-aligned ranges
pub struct PageIter {
    current: usize,
    end: usize,
    page_size: usize,
}

impl Iterator for PageIter {
    type Item = VaRange;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.end {
            return None;
        }

        let start = self.current;
        self.current += self.page_size;

        Some(VaRange::from_size(start, self.page_size))
    }
}

/// TLB management functions
pub mod tlb {
    use super::*;

    /// Invalidate all TLB entries
    #[inline]
    pub fn invalidate_all() {
        crate::arch::riscv64::cpu::asm::sfence_vma();
    }

    /// Invalidate TLB entries for a specific virtual address
    #[inline]
    pub fn invalidate_va(va: usize) {
        crate::arch::riscv64::cpu::asm::sfence_vma_addr(va);
    }

    /// Invalidate TLB entries for a specific ASID
    #[inline]
    pub fn invalidate_asid(asid: usize) {
        crate::arch::riscv64::cpu::asm::sfence_vma_asid(asid);
    }

    /// Invalidate TLB entries for a specific virtual address and ASID
    #[inline]
    pub fn invalidate_va_asid(va: usize, asid: usize) {
        crate::arch::riscv64::cpu::asm::sfence_vma_addr_asid(va, asid);
    }

    /// Invalidate guest TLB entries (for virtualization)
    #[inline]
    pub fn invalidate_guest_all() {
        crate::arch::riscv64::cpu::asm::hfence_vvma();
    }

    /// Invalidate guest TLB entries for a specific guest virtual address
    #[inline]
    pub fn invalidate_guest_va(gva: usize) {
        crate::arch::riscv64::cpu::asm::hfence_vvma_addr(gva);
    }

    /// Invalidate guest TLB entries for a specific ASID
    #[inline]
    pub fn invalidate_guest_asid(asid: usize) {
        crate::arch::riscv64::cpu::asm::hfence_vvma_asid(asid);
    }

    /// Invalidate guest TLB entries for a specific guest virtual address and ASID
    #[inline]
    pub fn invalidate_guest_va_asid(gva: usize, asid: usize) {
        crate::arch::riscv64::cpu::asm::hfence_vvma_addr_asid(gva, asid);
    }

    /// Invalidate stage-2 TLB entries (guest physical to host physical)
    #[inline]
    pub fn invalidate_stage2_all() {
        crate::arch::riscv64::cpu::asm::hfence_gvma();
    }

    /// Invalidate stage-2 TLB entries for a specific guest physical address
    #[inline]
    pub fn invalidate_stage2_gpa(gpa: usize) {
        crate::arch::riscv64::cpu::asm::hfence_gvma_addr(gpa);
    }

    /// Invalidate stage-2 TLB entries for a specific VMID
    #[inline]
    pub fn invalidate_stage2_vmid(vmid: usize) {
        crate::arch::riscv64::cpu::asm::hfence_gvma_vmid(vmid);
    }

    /// Invalidate stage-2 TLB entries for a specific guest physical address and VMID
    #[inline]
    pub fn invalidate_stage2_gpa_vmid(gpa: usize, vmid: usize) {
        crate::arch::riscv64::cpu::asm::hfence_gvma_addr_vmid(gpa, vmid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_alignment() {
        assert!(is_page_aligned(0x1000, 4096));
        assert!(!is_page_aligned(0x1001, 4096));
        assert!(is_page_aligned(0x200000, 0x200000));

        assert_eq!(page_align_down(0x12345, 4096), 0x12000);
        assert_eq!(page_align_up(0x12345, 4096), 0x13000);
        assert_eq!(page_offset(0x12345, 4096), 0x345);
    }

    #[test]
    fn test_vpn_from_va() {
        let va = 0x123456789ABC;
        assert_eq!(vpn_from_va(va, 12), va >> 12);
        assert_eq!(vpn_from_va(va, 21), va >> 21);
    }

    #[test]
    fn test_va_range() {
        let range = VaRange::new(0x1000, 0x3000);
        assert_eq!(range.size(), 0x2000);
        assert!(range.contains(0x1000));
        assert!(range.contains(0x2000));
        assert!(!range.contains(0x3000));
        assert!(!range.contains(0x0FFF));

        let from_size = VaRange::from_size(0x1000, 0x2000);
        assert_eq!(from_size.start, 0x1000);
        assert_eq!(from_size.end, 0x3000);
    }

    #[test]
    fn test_va_range_split() {
        let range = VaRange::new(0x1234, 0x2000);
        let split = range.split_at_page(4096);
        assert!(split.is_some());

        let (unaligned, aligned) = split.unwrap();
        assert_eq!(unaligned.start, 0x1234);
        assert_eq!(unaligned.end, 0x2000);
        assert_eq!(aligned.start, 0x2000);
        assert_eq!(aligned.end, 0x2000); // Empty range
    }

    #[test]
    fn test_page_iter() {
        let range = VaRange::new(0x1000, 0x3000);
        let pages: Vec<_> = range.page_iter(4096).collect();
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].start, 0x1000);
        assert_eq!(pages[0].size(), 4096);
        assert_eq!(pages[1].start, 0x2000);
        assert_eq!(pages[1].size(), 4096);
    }

    #[test]
    fn test_asid_manager() {
        let mut manager = AsidManager::new(10);
        let asid1 = manager.allocate().unwrap();
        let asid2 = manager.allocate().unwrap();

        assert_ne!(asid1, asid2);
        assert_eq!(asid1, 1);
        assert_eq!(asid2, 2);

        manager.free(asid1);
    }

    #[test]
    fn test_translation_result() {
        let failed = TranslationResult::failed();
        assert!(!failed.success);
        assert_eq!(failed.pa, 0);

        let success = TranslationResult::success(
            0x12345000,
            PteFlags::R | PteFlags::W | PteFlags::V,
            false,
            4096,
        );
        assert!(success.success);
        assert_eq!(success.pa, 0x12345000);
        assert!(!success.superpage);
        assert_eq!(success.page_size, 4096);
    }
}