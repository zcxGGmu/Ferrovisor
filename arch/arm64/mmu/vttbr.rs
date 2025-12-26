//! VTTBR_EL2 management for ARM64
//!
//! Provides VMID allocation and VTTBR_EL2 register management.

use crate::mmu::vttbr;

/// Maximum number of VMIDs supported (8-bit VMID field)
pub const MAX_VMID: u16 = 255;

/// Invalid VMID
pub const INVALID_VMID: u16 = 0;

/// VMID allocator state
static mut NEXT_VMID: u16 = 1;
static mut VMID_BITMAP: [u64; 4] = [0; 4]; // 256 bits = 4 u64

/// Allocate a new VMID
pub fn allocate_vmid() -> Result<u16, &'static str> {
    unsafe {
        for i in 1..=MAX_VMID {
            let idx = (i - 1) / 64;
            let bit = (i - 1) % 64;
            if VMID_BITMAP[idx] & (1 << bit) == 0 {
                VMID_BITMAP[idx] |= 1 << bit;
                return Ok(i);
            }
        }
    }
    Err("No VMID available")
}

/// Free a VMID
pub fn free_vmid(vmid: u16) {
    if vmid == 0 {
        return;
    }
    unsafe {
        let idx = (vmid - 1) / 64;
        let bit = (vmid - 1) % 64;
        VMID_BITMAP[idx] &= !(1 << bit);
    }
}

/// Create VTTBR_EL2 value from VMID and page table base
pub fn make_vttbr(vmid: u16, baddr: u64) -> u64 {
    ((vmid as u64) << 48) | (baddr & 0x0000FFFFFFFFFFFF)
}

/// Extract VMID from VTTBR_EL2
pub fn extract_vmid(vttbr: u64) -> u16 {
    ((vttbr >> 48) & 0xFF) as u16
}

/// Extract page table base from VTTBR_EL2
pub fn extract_baddr(vttbr: u64) -> u64 {
    vttbr & 0x0000FFFFFFFFFFFF
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
        let vmid1 = allocate_vmid().unwrap();
        let vmid2 = allocate_vmid().unwrap();

        assert_ne!(vmid1, vmid2);
        assert_eq!(vmid1, 1); // First allocation
        assert_eq!(vmid2, 2); // Second allocation

        free_vmid(vmid1);
        free_vmid(vmid2);
    }
}
