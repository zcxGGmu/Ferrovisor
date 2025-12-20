//! Architecture-common code
//!
//! This module contains code that is common across all supported architectures
//! or provides architecture-agnostic abstractions.

use crate::core::mm::{PhysAddr, VirtAddr, PAGE_SIZE};

/// CPU feature flags
#[derive(Debug, Clone, Copy)]
pub struct CpuFeatures {
    /// Virtualization support
    pub virtualization: bool,
    /// Second Level Address Translation (SLAT)
    pub slat: bool,
    /// Virtualization Host Extensions (VHE) - ARM64 only
    pub vhe: bool,
    /// IOMMU support
    pub iommu: bool,
    /// Large pages (2MB/1GB)
    pub large_pages: bool,
    /// Nested virtualization
    pub nested_virt: bool,
    /// Hardware performance counters
    pub perf_counters: bool,
}

impl Default for CpuFeatures {
    fn default() -> Self {
        Self {
            virtualization: false,
            slat: false,
            vhe: false,
            iommu: false,
            large_pages: false,
            nested_virt: false,
            perf_counters: false,
        }
    }
}

/// CPU context structure for saving/restoring state
#[repr(C)]
pub struct CpuContext {
    /// General purpose registers
    pub gpr: [u64; 32],
    /// Program counter
    pub pc: u64,
    /// Stack pointer
    pub sp: u64,
    /// Processor state register
    pub psr: u64,
    /// Architecture-specific context
    pub arch_specific: ArchSpecificContext,
}

/// Architecture-specific context
#[repr(C)]
pub union ArchSpecificContext {
    /// ARM64 specific context
    pub arm64: Arm64Context,
    /// RISC-V specific context
    pub riscv64: Riscv64Context,
    /// x86_64 specific context
    pub x86_64: X86_64Context,
}

/// ARM64 specific context
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Arm64Context {
    /// TPIDR_EL0 (Thread ID Register)
    pub tpidr_el0: u64,
    /// TPIDR_EL1 (Thread ID Register)
    pub tpidr_el1: u64,
    /// VBAR_EL1 (Vector Base Address Register)
    pub vbar_el1: u64,
    /// SCTLR_EL1 (System Control Register)
    pub sctlr_el1: u64,
    /// TCR_EL1 (Translation Control Register)
    pub tcr_el1: u64,
    /// TTBR0_EL1 (Translation Table Base Register 0)
    pub ttbr0_el1: u64,
    /// TTBR1_EL1 (Translation Table Base Register 1)
    pub ttbr1_el1: u64,
}

/// RISC-V specific context
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Riscv64Context {
    /// Satp (Supervisor Address Translation and Protection)
    pub satp: u64,
    /// Sstatus (Supervisor Status Register)
    pub sstatus: u64,
    /// Sie (Supervisor Interrupt Enable Register)
    pub sie: u64,
    /// Stvec (Supervisor Trap Vector Base Address Register)
    pub stvec: u64,
    /// Sscratch (Supervisor Scratch Register)
    pub sscratch: u64,
    /// Sepc (Supervisor Exception Program Counter)
    pub sepc: u64,
    /// Scause (Supervisor Cause Register)
    pub scause: u64,
    /// Stval (Supervisor Trap Value Register)
    pub stval: u64,
}

/// x86_64 specific context
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct X86_64Context {
    /// CR0 (Control Register 0)
    pub cr0: u64,
    /// CR2 (Control Register 2)
    pub cr2: u64,
    /// CR3 (Control Register 3 - Page Table Base)
    pub cr3: u64,
    /// CR4 (Control Register 4)
    pub cr4: u64,
    /// RFLAGS
    pub rflags: u64,
    /// FS Base
    pub fs_base: u64,
    /// GS Base
    pub gs_base: u64,
    /// Kernel GS Base
    pub kernel_gs_base: u64,
}

/// I/O port access interface
pub trait IoPortAccess {
    /// Read 8-bit value from I/O port
    fn read_u8(&self, port: u16) -> u8;

    /// Read 16-bit value from I/O port
    fn read_u16(&self, port: u16) -> u16;

    /// Read 32-bit value from I/O port
    fn read_u32(&self, port: u16) -> u32;

    /// Read 64-bit value from I/O port
    fn read_u64(&self, port: u16) -> u64;

    /// Write 8-bit value to I/O port
    fn write_u8(&self, port: u16, value: u8);

    /// Write 16-bit value to I/O port
    fn write_u16(&self, port: u16, value: u16);

    /// Write 32-bit value to I/O port
    fn write_u32(&self, port: u16, value: u32);

    /// Write 64-bit value to I/O port
    fn write_u64(&self, port: u16, value: u64);
}

/// Memory-mapped I/O access
pub trait MmioAccess {
    /// Read 8-bit value from MMIO address
    fn read_u8(&self, addr: VirtAddr) -> u8;

    /// Read 16-bit value from MMIO address
    fn read_u16(&self, addr: VirtAddr) -> u16;

    /// Read 32-bit value from MMIO address
    fn read_u32(&self, addr: VirtAddr) -> u32;

    /// Read 64-bit value from MMIO address
    fn read_u64(&self, addr: VirtAddr) -> u64;

    /// Write 8-bit value to MMIO address
    fn write_u8(&self, addr: VirtAddr, value: u8);

    /// Write 16-bit value to MMIO address
    fn write_u16(&self, addr: VirtAddr, value: u16);

    /// Write 32-bit value to MMIO address
    fn write_u32(&self, addr: VirtAddr, value: u32);

    /// Write 64-bit value to MMIO address
    fn write_u64(&self, addr: VirtAddr, value: u64);
}

/// Generic MMIO access implementation
pub struct GenericMmioAccess;

impl MmioAccess for GenericMmioAccess {
    fn read_u8(&self, addr: VirtAddr) -> u8 {
        unsafe { core::ptr::read_volatile(addr as *const u8) }
    }

    fn read_u16(&self, addr: VirtAddr) -> u16 {
        unsafe { core::ptr::read_volatile(addr as *const u16) }
    }

    fn read_u32(&self, addr: VirtAddr) -> u32 {
        unsafe { core::ptr::read_volatile(addr as *const u32) }
    }

    fn read_u64(&self, addr: VirtAddr) -> u64 {
        unsafe { core::ptr::read_volatile(addr as *const u64) }
    }

    fn write_u8(&self, addr: VirtAddr, value: u8) {
        unsafe { core::ptr::write_volatile(addr as *mut u8, value) };
    }

    fn write_u16(&self, addr: VirtAddr, value: u16) {
        unsafe { core::ptr::write_volatile(addr as *mut u16, value) };
    }

    fn write_u32(&self, addr: VirtAddr, value: u32) {
        unsafe { core::ptr::write_volatile(addr as *mut u32, value) };
    }

    fn write_u64(&self, addr: VirtAddr, value: u64) {
        unsafe { core::ptr::write_volatile(addr as *mut u64, value) };
    }
}

/// Architecture-independent timer interface
pub trait Timer {
    /// Get the current timer value
    fn get_value(&self) -> u64;

    /// Set a timer to fire after a specified duration
    fn set_timer(&mut self, duration: u64);

    /// Cancel a timer
    fn cancel_timer(&mut self);

    /// Check if timer is pending
    fn is_pending(&self) -> bool;
}

/// Cache maintenance operations
pub mod cache {
    /// Invalidate the instruction cache
    pub fn invalidate_icache() {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            core::arch::asm!("ic iallu");
            core::arch::asm!("dsb ish");
            core::arch::asm!("isb");
        }

        #[cfg(target_arch = "riscv64")]
        unsafe {
            core::arch::asm!("fence.i");
        }

        #[cfg(target_arch = "x86_64")]
        {
            // x86_64 has coherent instruction cache
        }
    }

    /// Clean data cache range
    pub fn clean_dcache_range(addr: VirtAddr, size: usize) {
        // TODO: Implement architecture-specific cache clean
    }

    /// Invalidate data cache range
    pub fn invalidate_dcache_range(addr: VirtAddr, size: usize) {
        // TODO: Implement architecture-specific cache invalidate
    }

    /// Clean and invalidate data cache range
    pub fn clean_invalidate_dcache_range(addr: VirtAddr, size: usize) {
        // TODO: Implement architecture-specific cache clean/invalidate
    }
}

/// TLB maintenance operations
pub mod tlb {
    use crate::core::mm::VirtAddr;

    /// Invalidate all TLB entries
    pub fn invalidate_all() {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            core::arch::asm!("tlbi vmalle1is");
            core::arch::asm!("dsb ish");
            core::arch::asm!("isb");
        }

        #[cfg(target_arch = "riscv64")]
        unsafe {
            core::arch::asm!("sfence.vma");
        }

        #[cfg(target_arch = "x86_64")]
        {
            x86_64::instructions::tlb::flush_all();
        }
    }

    /// Invalidate TLB entry for specific address
    pub fn invalidate_addr(addr: VirtAddr) {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            core::arch::asm!("tlbi vae1is, {}", in(reg) addr);
            core::arch::asm!("dsb ish");
            core::arch::asm!("isb");
        }

        #[cfg(target_arch = "riscv64")]
        unsafe {
            core::arch::asm!("sfence.vma, {}", in(reg) addr);
        }

        #[cfg(target_arch = "x86_64")]
        {
            x86_64::instructions::tlb::flush(addr);
        }
    }

    /// Invalidate TLB entries for address space
    pub fn invalidate_asid(asid: u16) {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            core::arch::asm!("tlbi aside1is, {}", in(reg) asid);
            core::arch::asm!("dsb ish");
            core::arch::asm!("isb");
        }

        #[cfg(target_arch = "riscv64")]
        {
            // RISC-V doesn't have ASID-based invalidation
            invalidate_all();
        }

        #[cfg(target_arch = "x86_64")]
        {
            // x86_64 doesn't have ASID in hardware
            invalidate_all();
        }
    }
}

/// Memory barrier operations
pub mod barrier {
    /// Full system barrier
    pub fn full() {
        #[cfg(target_arch = "aarch64")]
        cortex_a::asm::dmb(cortex_a::asm::SY);

        #[cfg(target_arch = "riscv64")]
        riscv::asm::fence(riscv::asm::Ordering::SeqCst, riscv::asm::Ordering::SeqCst);

        #[cfg(target_arch = "x86_64")]
        x86_64::instructions::mfence();
    }

    /// Memory acquire barrier
    pub fn acquire() {
        #[cfg(target_arch = "aarch64")]
        cortex_a::asm::dmb(cortex_a::asm::LD);

        #[cfg(target_arch = "riscv64")]
        riscv::asm::fence(riscv::asm::Ordering::Acquire, riscv::asm::Ordering::Relaxed);

        #[cfg(target_arch = "x86_64")]
        x86_64::instructions::lfence();
    }

    /// Memory release barrier
    pub fn release() {
        #[cfg(target_arch = "aarch64")]
        cortex_a::asm::dmb(cortex_a::asm::ST);

        #[cfg(target_arch = "riscv64")]
        riscv::asm::fence(riscv::asm::Ordering::Relaxed, riscv::asm::Ordering::Release);

        #[cfg(target_arch = "x86_64")]
        x86_64::instructions::sfence();
    }
}

/// Get CPU features
pub fn get_cpu_features() -> CpuFeatures {
    #[cfg(target_arch = "aarch64")]
    {
        crate::arch::arm64::get_cpu_features()
    }

    #[cfg(target_arch = "riscv64")]
    {
        crate::arch::riscv64::get_cpu_features()
    }

    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x86_64::get_cpu_features()
    }
}

/// Initialize common architecture components
pub fn init() -> Result<(), crate::Error> {
    // Initialize CPU feature detection
    let features = get_cpu_features();
    crate::info!("CPU features: {:?}", features);

    // Initialize memory management
    crate::core::mm::init()?;

    Ok(())
}