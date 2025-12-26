//! VTTBR_EL2 management for ARM64
//!
//! Provides VMID allocation and VTTBR_EL2 register management.
//! Reference: ARM DDI 0487I.a, D13.2.131 VTTBR_EL2

use core::sync::atomic::{AtomicU16, AtomicU64, Ordering};

/// Maximum number of VMIDs supported (8-bit VMID field in VTTBR_EL2)
pub const MAX_VMID: u16 = 255;

/// Invalid VMID (VMID=0 is reserved)
pub const INVALID_VMID: u16 = 0;

/// VMID allocator state
struct VmidAllocator {
    next_vmid: AtomicU16,
    bitmap: [AtomicU64; 4], // 256 bits = 4 * 64 bits
}

/// Global VMID allocator
static VMID_ALLOCATOR: VmidAllocator = VmidAllocator {
    next_vmid: AtomicU16::new(1),
    bitmap: [
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
    ],
};

/// Allocate a new VMID
///
/// Returns a unique VMID in range [1, 255]. VMID 0 is reserved.
pub fn allocate_vmid() -> Result<u16, &'static str> {
    // Try to use next_vmid first (fast path)
    let vmid = VMID_ALLOCATOR.next_vmid.load(Ordering::Relaxed);
    if vmid <= MAX_VMID {
        let idx = (vmid - 1) / 64;
        let bit = (vmid - 1) % 64;
        let mask = 1u64 << bit;

        let prev = VMID_ALLOCATOR.bitmap[idx].fetch_or(mask, Ordering::Acquire);
        if prev & mask == 0 {
            // Successfully allocated
            VMID_ALLOCATOR.next_vmid.store(vmid + 1, Ordering::Relaxed);
            return Ok(vmid);
        }
    }

    // Slow path: search for free VMID
    for i in 1..=MAX_VMID {
        let idx = (i - 1) / 64;
        let bit = (i - 1) % 64;
        let mask = 1u64 << bit;

        if VMID_ALLOCATOR.bitmap[idx].fetch_or(mask, Ordering::Acquire) & mask == 0 {
            return Ok(i);
        }
    }

    Err("No VMID available")
}

/// Free a VMID
///
/// # Safety
/// The VMID must have been previously allocated and is no longer in use.
pub fn free_vmid(vmid: u16) {
    if vmid == 0 || vmid > MAX_VMID {
        return;
    }

    let idx = (vmid - 1) / 64;
    let bit = (vmid - 1) % 64;
    let mask = 1u64 << bit;

    VMID_ALLOCATOR.bitmap[idx].fetch_and(!mask, Ordering::Release);
}

/// Check if a VMID is currently allocated
pub fn is_vmid_allocated(vmid: u16) -> bool {
    if vmid == 0 || vmid > MAX_VMID {
        return false;
    }

    let idx = (vmid - 1) / 64;
    let bit = (vmid - 1) % 64;
    let mask = 1u64 << bit;

    VMID_ALLOCATOR.bitmap[idx].load(Ordering::Acquire) & mask != 0
}

/// Create VTTBR_EL2 value from VMID and page table base address
///
/// # Arguments
/// * `vmid` - Virtual Machine ID (0-255)
/// * `baddr` - Page table base address (must be 4KB aligned, bits [47:12])
///
/// # Returns
/// VTTBR_EL2 register value
///
/// # Format
/// ```
/// VTTBR_EL2[63:56] - VMID (8-bit)
/// VTTBR_EL2[47:12] - BADDR (Page table base, 36 bits)
/// VTTBR_EL2[11:7]  - Reserved (SBZ)
/// VTTBR_EL2[6:5]   - VMID field width (for 48-bit IPA)
/// VTTBR_EL2[4:1]   - Reserved (SBZ)
/// VTTBR_EL2[0]     - Reserved (SBZ)
/// ```
pub fn make_vttbr(vmid: u16, baddr: u64) -> u64 {
    let vmid_bits = (vmid as u64) & 0xFF;
    let baddr_bits = baddr & 0x0000_FFFF_FFFF_F000;

    (vmid_bits << 56) | baddr_bits
}

/// Extract VMID from VTTBR_EL2 value
pub fn extract_vmid(vttbr: u64) -> u16 {
    ((vttbr >> 56) & 0xFF) as u16
}

/// Extract page table base address from VTTBR_EL2 value
pub fn extract_baddr(vttbr: u64) -> u64 {
    vttbr & 0x0000_FFFF_FFFF_F000
}

/// Get current VTTBR_EL2 value
///
/// # Safety
/// Must be called at EL2
pub unsafe fn read_vttbr_el2() -> u64 {
    let value: u64;
    core::arch::asm!("mrs {}, vttbr_el2", out(reg) value);
    value
}

/// Set VTTBR_EL2 value
///
/// # Safety
/// Must be called at EL2
pub unsafe fn write_vttbr_el2(vttbr: u64) {
    core::arch::asm!("msr vttbr_el2, {}", in(reg) vttbr);
}

/// Initialize VMID allocator
pub fn init() -> Result<(), &'static str> {
    // Reset allocator state (for testing or reinit)
    for i in 0..4 {
        VMID_ALLOCATOR.bitmap[i].store(0, Ordering::Release);
    }
    VMID_ALLOCATOR.next_vmid.store(1, Ordering::Release);

    // log::debug!("VTTBR_EL2 VMID allocator initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vttbr_operations() {
        let vmid = 42u16;
        let baddr = 0x4050_0000u64;
        let vttbr = make_vttbr(vmid, baddr);

        assert_eq!(extract_vmid(vttbr), 42);
        assert_eq!(extract_baddr(vttbr), 0x4050_0000);
    }

    #[test]
    fn test_vmid_allocation() {
        // Allocate VMIDs
        let vmid1 = allocate_vmid().unwrap();
        let vmid2 = allocate_vmid().unwrap();

        assert_ne!(vmid1, vmid2);
        assert_eq!(vmid1, 1); // First allocation
        assert_eq!(vmid2, 2); // Second allocation

        // Check allocation status
        assert!(is_vmid_allocated(vmid1));
        assert!(is_vmid_allocated(vmid2));

        // Free VMIDs
        free_vmid(vmid1);
        free_vmid(vmid2);

        // Verify deallocation
        assert!(!is_vmid_allocated(vmid1));
        assert!(!is_vmid_allocated(vmid2));
    }

    #[test]
    fn test_vmid_exhaustion() {
        // Try to allocate more than MAX_VMID
        let vmids: Vec<_> = (0..MAX_VMID)
            .filter_map(|_| allocate_vmid().ok())
            .collect();

        assert_eq!(vmids.len(), MAX_VMID as usize);

        // Should fail when all VMIDs are allocated
        assert!(allocate_vmid().is_err());
    }

    #[test]
    fn test_reserved_vmid_zero() {
        // VMID 0 should be reserved
        assert!(!is_vmid_allocated(0));
    }
}
