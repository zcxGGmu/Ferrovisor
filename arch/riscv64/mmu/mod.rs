//! RISC-V MMU Module
//!
//! This module provides memory management unit functionality including:
//! - Page table management (Sv39/Sv48)
//! - Address translation
//! - Memory protection
//! - Two-stage translation for virtualization
//! - Physical and virtual memory management

pub mod ptable;
pub mod translation;
pub mod memory;
pub mod gstage;
pub mod guest_space;
pub mod extended_pt;

pub use ptable::*;
pub use translation::*;
pub use memory::*;
pub use gstage::*;
pub use guest_space::*;
pub use extended_pt::*;

use crate::arch::riscv64::*;

/// Global MMU instance
static mut MMU: Option<Mmu> = None;

/// Initialize MMU subsystem
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing RISC-V MMU");

    // Create a simple frame allocator for testing
    // In a real implementation, this would be initialized from the memory map
    let mut allocator = SimpleFrameAllocator::new(0x80000000, 0x10000000); // 256MB

    // Initialize the allocator with reserved regions
    let reserved_regions = vec![
        // Reserve lower memory (usually contains device mappings)
        MemRegion::new(
            0x00000000,
            0x00000000,
            0x10000000,
            MemFlags::READABLE | MemFlags::WRITABLE | MemFlags::DEVICE,
            "reserved_low",
        ),
        // Reserve memory for the kernel
        MemRegion::new(
            0x80000000,
            0x80000000,
            0x00800000,
            MemFlags::READABLE | MemFlags::WRITABLE | MemFlags::EXECUTABLE,
            "kernel",
        ),
    ];

    allocator.init(&reserved_regions);

    // Create global MMU instance
    let mmu = Mmu::new(allocator);

    unsafe {
        MMU = Some(mmu);
    }

    log::info!("RISC-V MMU initialized successfully");

    // Initialize G-stage translation
    crate::arch::riscv64::mmu::gstage::init()?;

    // Initialize guest address space manager
    crate::arch::riscv64::mmu::guest_space::init()?;

    // Initialize extended page table format detection
    crate::arch::riscv64::mmu::extended_pt::init()?;

    Ok(())
}

/// Get the global MMU instance
pub fn get_mmu() -> Option<&'static Mmu> {
    unsafe { MMU.as_ref() }
}

/// Get mutable reference to global MMU instance
pub fn get_mmu_mut() -> Option<&'static mut Mmu> {
    unsafe { MMU.as_mut() }
}

/// Enable virtual memory
pub fn enable_vm() -> Result<(), &'static str> {
    log::debug!("Enabling virtual memory");

    // Create initial address space for the kernel
    let mut kernel_space = create_kernel_address_space()?;

    // Activate the kernel address space
    kernel_space.activate();

    log::info!("Virtual memory enabled");
    Ok(())
}

/// Create initial kernel address space
pub fn create_kernel_address_space() -> Result<AddressSpace, &'static str> {
    let mmu = get_mmu_mut().ok_or("MMU not initialized")?;

    // Create kernel address space
    let mut kernel_space = mmu.create_address_space(8)?; // Sv39 mode

    // Map kernel code and data
    let kernel_regions = vec![
        // Map kernel text segment (executable)
        MemRegion::new(
            0x80000000,
            0x80000000,
            0x00200000,
            MemFlags::READABLE | MemFlags::EXECUTABLE,
            "kernel_text",
        ),
        // Map kernel data segment (readable/writable)
        MemRegion::new(
            0x80200000,
            0x80200000,
            0x00200000,
            MemFlags::READABLE | MemFlags::WRITABLE,
            "kernel_data",
        ),
        // Map kernel BSS segment
        MemRegion::new(
            0x80400000,
            0x80400000,
            0x00100000,
            MemFlags::READABLE | MemFlags::WRITABLE,
            "kernel_bss",
        ),
    ];

    for region in kernel_regions {
        kernel_space.map_region(region)?;
    }

    // Map device memory
    let device_regions = vec![
        // UART
        MemRegion::new(
            0x10000000,
            0x10000000,
            0x1000,
            MemFlags::READABLE | MemFlags::WRITABLE | MemFlags::DEVICE,
            "uart",
        ),
        // PLIC
        MemRegion::new(
            0x0C000000,
            0x0C000000,
            0x200000,
            MemFlags::READABLE | MemFlags::WRITABLE | MemFlags::DEVICE,
            "plic",
        ),
    ];

    for region in device_regions {
        kernel_space.map_region(region)?;
    }

    Ok(kernel_space)
}

/// Translation modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslationMode {
    /// No translation (bare metal)
    Bare = 0,
    /// Sv39 - 39-bit virtual addresses, 3-level page tables
    Sv39 = 8,
    /// Sv48 - 48-bit virtual addresses, 4-level page tables
    Sv48 = 9,
}

/// Get supported translation mode
pub fn get_supported_mode() -> TranslationMode {
    // Check if Sv48 is supported
    // This would typically be determined by probing the hardware
    // For now, default to Sv39
    TranslationMode::Sv39
}

/// Check if virtualization is available
pub fn has_virtualization() -> bool {
    // Check if H extension is present
    crate::arch::riscv64::cpu::features::has_extension(
        crate::arch::riscv64::cpu::features::IsaExtension::H,
    )
}

/// Memory barrier instructions for MMU operations
pub mod barrier {
    /// Full memory barrier
    #[inline]
    pub fn full() {
        crate::arch::riscv64::cpu::asm::memory_fence();
    }

    /// Acquire barrier
    #[inline]
    pub fn acquire() {
        crate::arch::riscv64::cpu::asm::memory_fence();
    }

    /// Release barrier
    #[inline]
    pub fn release() {
        crate::arch::riscv64::cpu::asm::memory_fence();
    }

    /// I/O barrier
    #[inline]
    pub fn io() {
        crate::arch::riscv64::cpu::asm::memory_fence_io();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translation_mode() {
        let mode = get_supported_mode();
        assert!(mode == TranslationMode::Sv39 || mode == TranslationMode::Sv48);
    }

    #[test]
    fn test_kernel_address_space() {
        // This test requires MMU initialization
        init().unwrap();

        let kernel_space = create_kernel_address_space().unwrap();
        assert_eq!(kernel_space.mode(), 8); // Sv39
        assert!(kernel_space.translate(0x80000000).is_ok());
    }

    #[test]
    fn test_memory_barriers() {
        barrier::full();
        barrier::acquire();
        barrier::release();
        barrier::io();
    }

    #[test]
    fn test_gstage_mode() {
        use crate::arch::riscv64::mmu::gstage::*;

        assert_eq!(GStageMode::Sv39x4.addr_bits(), 39);
        assert_eq!(GStageMode::Sv48x4.addr_bits(), 48);
        assert_eq!(GStageMode::Sv32x4.levels(), 2);
    }

    #[test]
    fn test_gstage_hgatp() {
        use crate::arch::riscv64::mmu::gstage::*;

        let vmid = 100;
        let ppn = 0x87654321;
        let mode = GStageMode::Sv39x4;

        let hgatp = GStageTranslator::make_hgatp(vmid, ppn, mode);

        assert_eq!(GStageTranslator::extract_vmid(hgatp), vmid);
        assert_eq!(GStageTranslator::extract_mode(hgatp), mode);
    }

    #[test]
    fn test_guest_address_space() {
        use crate::arch::riscv64::mmu::guest_space::*;

        let mut space = GuestAddressSpace::new(1, GStageMode::Sv39x4).unwrap();

        // Test memory mapping
        let result = space.map_memory(0x10000000, 0x80000000, 0x1000,
                                        true, true, false, "test");
        assert!(result.is_ok());
        assert_eq!(space.get_regions().len(), 1);
    }
}