//! VMCS (Virtual Machine Control Structure) Management
//!
//! This module handles VMCS operations for x86_64 virtualization.

use crate::{Result, Error};
use crate::core::mm::{VirtAddr, PAGE_SIZE, align_up};

// VMCS field encodings (simplified)
pub const VMCS_VMCS_LINK_POINTER: u32 = 0x00004000;
pub const VMCS_GUEST_ES_SELECTOR: u32 = 0x00000800;
pub const VMCS_GUEST_CS_SELECTOR: u32 = 0x00000802;
pub const VMCS_GUEST_SS_SELECTOR: u32 = 0x00000804;
pub const VMCS_GUEST_DS_SELECTOR: u32 = 0x00000806;
pub const VMCS_GUEST_FS_SELECTOR: u32 = 0x00000808;
pub const VMCS_GUEST_GS_SELECTOR: u32 = 0x0000080A;
pub const VMCS_GUEST_LDTR_SELECTOR: u32 = 0x0000080C;
pub const VMCS_GUEST_TR_SELECTOR: u32 = 0x0000080E;
pub const VMCS_HOST_ES_SELECTOR: u32 = 0x00000C00;
pub const VMCS_HOST_CS_SELECTOR: u32 = 0x00000C02;
pub const VMCS_HOST_SS_SELECTOR: u32 = 0x00000C04;
pub const VMCS_HOST_DS_SELECTOR: u32 = 0x00000C06;
pub const VMCS_HOST_FS_SELECTOR: u32 = 0x00000C08;
pub const VMCS_HOST_GS_SELECTOR: u32 = 0x00000C0A;
pub const VMCS_HOST_TR_SELECTOR: u32 = 0x00000C0C;

pub const VMCS_GUEST_CR0: u32 = 0x00006800;
pub const VMCS_GUEST_CR3: u32 = 0x00006802;
pub const VMCS_GUEST_CR4: u32 = 0x00006804;
pub const VMCS_HOST_CR0: u32 = 0x00006C00;
pub const VMCS_HOST_CR3: u32 = 0x00006C02;
pub const VMCS_HOST_CR4: u32 = 0x00006C04;

pub const VMCS_GUEST_RSP: u32 = 0x00006814;
pub const VMCS_GUEST_RIP: u32 = 0x00006816;
pub const VMCS_HOST_RSP: u32 = 0x00006C14;
pub const VMCS_HOST_RIP: u32 = 0x00006C16;

/// VMCS region
pub struct VmcsRegion {
    /// Physical address of VMCS
    phys_addr: VirtAddr,
    /// Virtual address of VMCS
    virt_addr: VirtAddr,
    /// VMCS revision identifier
    revision_id: u32,
    /// Whether VMCS is current
    is_current: bool,
}

impl VmcsRegion {
    /// Create a new VMCS region
    pub fn new(revision_id: u32) -> Result<Self> {
        // Allocate a page for VMCS
        let page = crate::core::mm::frame::alloc_frame()
            .ok_or(Error::OutOfMemory)?;

        let vmcs = Self {
            phys_addr: page,
            virt_addr: page,
            revision_id,
            is_current: false,
        };

        // Clear the VMCS page
        unsafe {
            core::ptr::write_bytes(page as *mut u8, 0, PAGE_SIZE as usize);
        }

        // Write VMCS revision identifier at the start
        unsafe {
            let vmcs_ptr = vmcs.virt_addr as *mut u32;
            core::ptr::write_volatile(vmcs_ptr, revision_id);
        }

        Ok(vmcs)
    }

    /// Get physical address of VMCS
    pub fn phys_addr(&self) -> VirtAddr {
        self.phys_addr
    }

    /// Get virtual address of VMCS
    pub fn virt_addr(&self) -> VirtAddr {
        self.virt_addr
    }

    /// Get revision identifier
    pub fn revision_id(&self) -> u32 {
        self.revision_id
    }

    /// Clear the VMCS
    pub fn clear(&self) -> Result<()> {
        let result = unsafe { vmx_vmclear(self.phys_addr) };
        if result != 0 {
            return Err(Error::InvalidState);
        }
        Ok(())
    }

    /// Load the VMCS
    pub fn load(&mut self) -> Result<()> {
        let result = unsafe { vmx_vmptrld(self.phys_addr) };
        if result != 0 {
            return Err(Error::InvalidState);
        }
        self.is_current = true;
        Ok(())
    }

    /// Release the VMCS
    pub fn release(&mut self) {
        if self.is_current {
            unsafe { vmx_vmclear(self.phys_addr) };
            self.is_current = false;
        }
    }

    /// Read a VMCS field
    pub fn read(&self, field: u32) -> u64 {
        unsafe { vmx_read_vmcs(field) }
    }

    /// Write a VMCS field
    pub fn write(&self, field: u32, value: u64) -> Result<()> {
        let result = unsafe { vmx_write_vmcs(field, value) };
        if result != 0 {
            return Err(Error::InvalidState);
        }
        Ok(())
    }

    /// Write a 16-bit field
    pub fn write16(&self, field: u32, value: u16) -> Result<()> {
        self.write(field, value as u64)
    }

    /// Write a 32-bit field
    pub fn write32(&self, field: u32, value: u32) -> Result<()> {
        self.write(field, value as u64)
    }

    /// Write a 64-bit field
    pub fn write64(&self, field: u32, value: u64) -> Result<()> {
        self.write(field, value)
    }

    /// Initialize the VMCS with basic values
    pub fn initialize(&self) -> Result<()> {
        // Set up guest state
        self.write64(VMCS_GUEST_CR0, 0x60000010)?; // PE, ET, PG
        self.write64(VMCS_GUEST_CR4, 0x2000)?;      // VMXE
        self.write64(VMCS_GUEST_RSP, 0)?;           // Initial stack pointer
        self.write64(VMCS_GUEST_RIP, 0)?;           // Initial instruction pointer

        // Set up host state
        let mut cr0 = x86_64::registers::control::Cr0::read();
        cr0.insert(x86_64::registers::control::Cr0Flags::PROTECTION_ENABLE);
        cr0.insert(x86_64::registers::control::Cr0Flags::PAGING);
        self.write64(VMCS_HOST_CR0, cr0.bits())?;

        let cr3 = x86_64::registers::control::Cr3::read();
        self.write64(VMCS_HOST_CR3, cr3)?;

        let mut cr4 = x86_64::registers::control::Cr4::read();
        cr4.insert(x86_64::registers::control::Cr4Flags::VIRTUAL_MACHINE_EXTENSIONS);
        self.write64(VMCS_HOST_CR4, cr4.bits())?;

        // Set up host segments
        self.write16(VMCS_HOST_CS_SELECTOR, 0x08)?;
        self.write16(VMCS_HOST_DS_SELECTOR, 0x10)?;
        self.write16(VMCS_HOST_ES_SELECTOR, 0x10)?;
        self.write16(VMCS_HOST_FS_SELECTOR, 0x10)?;
        self.write16(VMCS_HOST_GS_SELECTOR, 0x10)?;
        self.write16(VM_HOST_SS_SELECTOR, 0x10)?;
        self.write16(VMCS_HOST_TR_SELECTOR, 0x18)?;

        Ok(())
    }
}

impl Drop for VmcsRegion {
    fn drop(&mut self) {
        self.release();
        // Deallocate the page
        crate::core::mm::frame::dealloc_frame(self.phys_addr);
    }
}

/// VMCS field encoding and access types
#[allow(dead_code)]
pub mod fields {
    pub const VMX_VMCS_VM_EXIT_REASON: u32 = 0x4402;
    pub const VMX_VMCS_VM_EXIT_QUALIFICATION: u32 = 0x6400;
    pub const VMX_VMCS_VM_EXIT_INSTRUCTION_LENGTH: u32 = 0x440C;
    pub const VMX_VMCS_VM_EXIT_INSTRUCTION_INFO: u32 = 0x440A;

    pub const VMX_VMCS_GUEST_RIP: u32 = 0x6816;
    pub const VMX_VMCS_GUEST_RSP: u32 = 0x6814;
    pub const VMX_VMCS_GUEST_RFLAGS: u32 = 0x6820;
    pub const VMX_VMCS_GUEST_CR0: u32 = 0x6800;
    pub const VMX_VMCS_GUEST_CR3: u32 = 0x6802;
    pub const VMX_VMCS_GUEST_CR4: u32 = 0x6804;
    pub const VMX_VMCS_GUEST_CS_SELECTOR: u32 = 0x802;
    pub const VMX_VMCS_GUEST_CS_BASE: u32 = 0x4800;
    pub const VMX_VMCS_GUEST_CS_LIMIT: u32 = 0x4802;
    pub const VMX_VMCS_GUEST_CS_ACCESS_RIGHTS: u32 = 0x4816;
}

/// Initialize VMX operation
pub fn vmx_init() -> Result<()> {
    // Enable VMX in CR4
    let mut cr4 = x86_64::registers::control::Cr4::read();
    if !cr4.contains(x86_64::registers::control::Cr4Flags::VIRTUAL_MACHINE_EXTENSIONS) {
        cr4.insert(x86_64::registers::control::Cr4Flags::VIRTUAL_MACHINE_EXTENSIONS);
        x86_64::registers::control::Cr4::write(cr4);
    }

    // Check if VMX is already enabled
    let vmxon_result = unsafe { vmx_vmxon(0) };
    if vmxon_result == 0 {
        // VMX already enabled
        return Ok(());
    }

    // Allocate VMXON region
    let vmxon_region = VirtAddrRegion::new(1)?; // 1 page

    // Get VMCS revision identifier
    let vmx_basic = x86_64::registers::model_specific::Msr::new(0x480);
    let vmx_basic_msr = vmxon_region.read_msr(vmx_basic);
    let vmcs_revision_id = (vmx_basic_msr & 0x7FFFFFFF) as u32;

    // Write VMCS revision identifier to VMXON region
    unsafe {
        let vmxon_ptr = vmxon_region.virt_addr() as *mut u32;
        core::ptr::write_volatile(vmx_ptr, vmcs_revision_id);
    }

    // Enable VMX
    let vmxon_result = unsafe { vmx_vmxon(vmx_region.phys_addr()) };
    if vmxon_result != 0 {
        return Err(Error::Unsupported);
    }

    Ok(())
}

/// VMXON instruction
///
/// Returns 0 on success, non-zero on error
#[inline]
#[target_feature(enable = "vmx")]
unsafe fn vmx_vmxon(region: VirtAddr) -> u32 {
    let mut result: u32;
    core::arch::asm!(
        "vmxon {region}",
        "setc {result}",
        region = in(reg) region,
        result = out(reg_byte) result,
        options(att_syntax, nomem, nostack, preserves_flags)
    );
    result
}

/// VMPTRLD instruction
///
/// Returns 0 on success, non-zero on error
#[inline]
#[target_feature(enable = "vmx")]
unsafe fn vmx_vmptrld(region: VirtAddr) -> u32 {
    let mut result: u32;
    core::arch::asm!(
        "vmptrld {region}",
        "setc {result}",
        region = in(reg) region,
        result = out(reg_byte) result,
        options(att_syntax, nomem, nostack, preserves_flags)
    );
    result
}

/// VMCLEAR instruction
///
/// Returns 0 on success, non-zero on error
#[inline]
#[target_feature(enable = "vmx")]
unsafe fn vmx_vmclear(region: VirtAddr) -> u32 {
    let mut result: u32;
    core::arch::asm!(
        "vmclear {region}",
        "setc {result}",
        region = in(reg) region,
        result = out(reg_byte) result,
        options(att_syntax, nomem, nostack, preserves_flags)
    );
    result
}

/// VMLAUNCH instruction
///
/// Returns 0 on success, non-zero on error
#[inline]
#[target_feature(enable = "vmx")]
unsafe fn vmx_vmlaunch() -> u32 {
    let mut result: u32;
    core::arch::asm!(
        "vmlaunch",
        "setc {result}",
        result = out(reg_byte) result,
        options(att_syntax, nomem, nostack, preserves_flags)
    );
    result
}

/// VMRESUME instruction
///
/// Returns 0 on success, non-zero on error
#[inline]
#[target_feature(enable = "vmx")]
unsafe fn vmx_vmresume() -> u32 {
    let mut result: u32;
    core::arch::asm!(
        "vmresume",
        "setc {result}",
        result = out(reg_byte) result,
        options(att_syntax, nomem, nostack, preserves_flags)
    );
    result
}

/// VMREAD instruction
///
/// Returns the value read from VMCS field
#[inline]
#[target_feature(enable = "vmx")]
unsafe fn vmx_read_vmcs(field: u32) -> u64 {
    let mut value: u64;
    core::arch::asm!(
        "vmread {field}, {value}",
        field = in(reg) field,
        value = out(reg) value,
        options(att_syntax, nomem, nostack, preserves_flags)
    );
    value
}

/// VMWRITE instruction
///
/// Returns 0 on success, non-zero on error
#[inline]
#[target_feature(enable = "vmx")]
unsafe fn vmx_write_vmcs(field: u32, value: u64) -> u32 {
    let mut result: u32;
    core::arch::asm!(
        "vmwrite {field}, {value}",
        "setc {result}",
        field = in(reg) field,
        value = in(reg) value,
        result = out(reg_byte) result,
        options(att_syntax, nomem, nostack, preserves_flags)
    );
    result
}

/// Simple virtual memory region allocator
struct VirtAddrRegion {
    virt_addr: VirtAddr,
    phys_addr: VirtAddr,
    pages: usize,
}

impl VirtAddrRegion {
    /// Allocate a new virtual memory region
    fn new(pages: usize) -> Result<Self> {
        // Allocate physical pages
        let mut phys_addr = crate::core::mm::frame::alloc_frame()
            .ok_or(Error::OutOfMemory)?;

        // For simplicity, assume identity mapping
        let virt_addr = phys_addr;

        // Allocate additional pages if needed
        for _ in 1..pages {
            let page = crate::core::mm::frame::alloc_frame()
                .ok_or(Error::OutOfMemory)?;
            // TODO: Set up page table mapping
        }

        Ok(Self {
            virt_addr,
            phys_addr,
            pages,
        })
    }

    /// Get virtual address
    fn virt_addr(&self) -> VirtAddr {
        self.virt_addr
    }

    /// Get physical address
    fn phys_addr(&self) -> VirtAddr {
        self.phys_addr
    }

    /// Read MSR value
    fn read_msr(&self, msr: x86_64::registers::model_specific::Msr) -> u64 {
        msr.read()
    }
}

/// Initialize VMCS management
pub fn init() -> Result<()> {
    #[cfg(target_arch = "x86_64")]
    {
        vmx_init()?;
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        // VMCS is x86_64 specific
    }

    Ok(())
}

/// Create a new VMCS region
pub fn create_vmcs(revision_id: u32) -> Result<VmcsRegion> {
    VmcsRegion::new(revision_id)
}