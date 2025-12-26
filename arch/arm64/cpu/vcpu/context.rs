//! VCPU context management for ARM64
//!
//! Provides high-level VCPU context switching functions.
//! Reference: xvisor/arch/arm/cpu/arm64/cpu_vcpu_helper.c

use crate::arch::arm64::cpu::sysreg::SysRegs;
use crate::arch::arm64::cpu::state::VcpuContext;
use crate::Result;

/// Offset definitions for SavedGprs structure
#[repr(C)]
pub struct SavedGprsOffsets {}
impl SavedGprsOffsets {
    pub const X1: usize = 0x00;
    pub const X2: usize = 0x08;
    pub const X3: usize = 0x10;
    pub const X4: usize = 0x18;
    pub const X5: usize = 0x20;
    pub const X6: usize = 0x28;
    pub const X7: usize = 0x30;
    pub const X8: usize = 0x38;
    pub const X9: usize = 0x40;
    pub const X10: usize = 0x48;
    pub const X11: usize = 0x50;
    pub const X12: usize = 0x58;
    pub const X13: usize = 0x60;
    pub const X14: usize = 0x68;
    pub const X15: usize = 0x70;
    pub const X16: usize = 0x78;
    pub const X17: usize = 0x80;
    pub const X18: usize = 0x88;
    pub const X19: usize = 0x90;
    pub const X20: usize = 0x98;
    pub const X21: usize = 0xA0;
    pub const X22: usize = 0xA8;
    pub const X23: usize = 0xB0;
    pub const X24: usize = 0xB8;
    pub const X25: usize = 0xC0;
    pub const X26: usize = 0xC8;
    pub const X27: usize = 0xD0;
    pub const X28: usize = 0xD8;
    pub const X29: usize = 0xE0;
    pub const X30: usize = 0xE8;
    pub const SP: usize = 0xF0;
}

/// Offset definitions for VcpuContext structure
#[repr(C)]
pub struct VcpuContextOffsets {}
impl VcpuContextOffsets {
    pub const HOST_SP: usize = 0x00;

    // Guest GPRs start at 0x08
    pub const GUEST_X1: usize = 0x08;
    pub const GUEST_X2: usize = 0x10;
    pub const GUEST_X3: usize = 0x18;
    pub const GUEST_X4: usize = 0x20;
    pub const GUEST_X5: usize = 0x28;
    pub const GUEST_X6: usize = 0x30;
    pub const GUEST_X7: usize = 0x38;
    pub const GUEST_X8: usize = 0x40;
    pub const GUEST_X9: usize = 0x48;
    pub const GUEST_X10: usize = 0x50;
    pub const GUEST_X11: usize = 0x58;
    pub const GUEST_X12: usize = 0x60;
    pub const GUEST_X13: usize = 0x68;
    pub const GUEST_X14: usize = 0x70;
    pub const GUEST_X15: usize = 0x78;
    pub const GUEST_X16: usize = 0x80;
    pub const GUEST_X17: usize = 0x88;
    pub const GUEST_X18: usize = 0x90;
    pub const GUEST_X19: usize = 0x98;
    pub const GUEST_X20: usize = 0xA0;
    pub const GUEST_X21: usize = 0xA8;
    pub const GUEST_X22: usize = 0xB0;
    pub const GUEST_X23: usize = 0xB8;
    pub const GUEST_X24: usize = 0xC0;
    pub const GUEST_X25: usize = 0xC8;
    pub const GUEST_X26: usize = 0xD0;
    pub const GUEST_X27: usize = 0xD8;
    pub const GUEST_X28: usize = 0xE0;
    pub const GUEST_X29: usize = 0xE8;
    pub const GUEST_X30: usize = 0xF0;
    pub const GUEST_SP: usize = 0xF8;

    pub const GUEST_ELR: usize = 0x200;   // elr_el1 (pc)
    pub const GUEST_SPSR: usize = 0x208,  // spsr_el1 (pstate)

    pub const SYSREGS: usize = 0x100,
    pub const VFPREGS: usize = 0x210,
}

/// VFP registers state
#[derive(Debug, Clone, Copy)]
#[repr(C, align(16))]
pub struct VfpRegs {
    /// Floating point control registers
    pub fpexc32_el2: u32,
    pub fpcr: u32,
    pub fpsr: u32,
    /// Reserved padding
    _reserved: u32,
    /// Floating point registers (q0-q31)
    /// Each is 128-bit (16 bytes), total 512 bytes
    pub fpregs: [u8; 512],
}

impl Default for VfpRegs {
    fn default() -> Self {
        Self {
            fpexc32_el2: 0,
            fpcr: 0,
            fpsr: 0,
            _reserved: 0,
            fpregs: [0; 512],
        }
    }
}

/// Saved general-purpose registers
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SavedGprs {
    /// x1-x30 and SP
    pub regs: [u64; 31],
}

impl Default for SavedGprs {
    fn default() -> Self {
        Self {
            regs: [0; 31],
        }
    }
}

impl SavedGprs {
    /// Create new zeroed GPR save area
    pub fn new() -> Self {
        Self::default()
    }

    /// Get register value
    pub fn get(&self, index: usize) -> u64 {
        if index == 0 || index > 30 {
            return 0; // x0 and SP are handled separately
        }
        self.regs[index - 1]
    }

    /// Set register value
    pub fn set(&mut self, index: usize, value: u64) {
        if index > 0 && index <= 30 {
            self.regs[index - 1] = value;
        }
    }

    /// Get SP
    pub fn get_sp(&self) -> u64 {
        self.regs[30] // Index 30 holds SP
    }

    /// Set SP
    pub fn set_sp(&mut self, value: u64) {
        self.regs[30] = value;
    }
}

/// Extended VCPU context with all state
#[derive(Debug)]
pub struct ExtendedVcpuContext {
    /// Basic VCPU context
    pub context: VcpuContext,
    /// System registers
    pub sysregs: SysRegs,
    /// VFP registers
    pub vfpregs: VfpRegs,
    /// Saved GPRs (host or guest)
    pub saved_gprs: SavedGprs,
}

impl ExtendedVcpuContext {
    /// Create new extended VCPU context
    pub fn new() -> Self {
        Self {
            context: VcpuContext::default(),
            sysregs: SysRegs::init_default(),
            vfpregs: VfpRegs::default(),
            saved_gprs: SavedGprs::new(),
        }
    }

    /// Save current host context
    ///
    /// # Safety
    /// Must be called from EL2 with valid system register state
    pub unsafe fn save_host_context(&mut self) {
        // Save system registers
        self.sysregs.save_from_hw();

        // Save GPRs (x1-x30, sp)
        // x0 is used as parameter, saved separately
        // SP is saved at offset SavedGprsOffsets::SP
        self.saved_gprs.set_sp(crate::arch::arm64::cpu::regs::sp_el1_read());

        // Note: Full GPR save requires assembly or inline asm
        // This is a simplified version
    }

    /// Restore host context
    ///
    /// # Safety
    /// Must be called from EL2 to restore host state
    pub unsafe fn restore_host_context(&self) {
        // Restore system registers
        self.sysregs.restore_to_hw();

        // Restore SP
        crate::arch::arm64::cpu::regs::sp_el1_write(self.saved_gprs.get_sp());
    }
}

impl Default for ExtendedVcpuContext {
    fn default() -> Self {
        Self::new()
    }
}

// External assembly functions
extern "C" {
    /// Save system registers
    fn __vcpu_sysregs_save(sysregs: *mut SysRegs);

    /// Restore system registers
    fn __vcpu_sysregs_restore(sysregs: *const SysRegs);

    /// Save VFP registers
    fn __vcpu_vfp_save(vfpregs: *mut VfpRegs);

    /// Restore VFP registers
    fn __vcpu_vfp_restore(vfpregs: *const VfpRegs);

    /// Save GPRs
    fn __vcpu_gprs_save(gprs: *mut SavedGprs);

    /// Restore GPRs
    fn __vcpu_gprs_restore(gprs: *const SavedGprs);

    /// Switch to guest VCPU (does ERET)
    fn __vcpu_switch_to_guest(context: *const VcpuContext);
}

/// Save VCPU system registers
///
/// # Arguments
/// * `sysregs` - Mutable pointer to system registers state
///
/// # Safety
/// Must be called from EL2
#[inline]
pub unsafe fn sysregs_save(sysregs: *mut SysRegs) {
    __vcpu_sysregs_save(sysregs);
}

/// Restore VCPU system registers
///
/// # Arguments
/// * `sysregs` - Pointer to system registers state
///
/// # Safety
/// Must be called from EL2
#[inline]
pub unsafe fn sysregs_restore(sysregs: *const SysRegs) {
    __vcpu_sysregs_restore(sysregs);
}

/// Save VFP registers
///
/// # Arguments
/// * `vfpregs` - Mutable pointer to VFP state
///
/// # Safety
/// Must be called from EL2
#[inline]
pub unsafe fn vfp_save(vfpregs: *mut VfpRegs) {
    __vcpu_vfp_save(vfpregs);
}

/// Restore VFP registers
///
/// # Arguments
/// * `vfpregs` - Pointer to VFP state
///
/// # Safety
/// Must be called from EL2
#[inline]
pub unsafe fn vfp_restore(vfpregs: *const VfpRegs) {
    __vcpu_vfp_restore(vfpregs);
}

/// Save GPRs
///
/// # Arguments
/// * `gprs` - Mutable pointer to GPR save area
///
/// # Safety
/// Must be called from EL2
#[inline]
pub unsafe fn gprs_save(gprs: *mut SavedGprs) {
    __vcpu_gprs_save(gprs);
}

/// Restore GPRs
///
/// # Arguments
/// * `gprs` - Pointer to GPR save area
///
/// # Safety
/// Must be called from EL2
#[inline]
pub unsafe fn gprs_restore(gprs: *const SavedGprs) {
    __vcpu_gprs_restore(gprs);
}

/// Switch to guest VCPU
///
/// This function saves the host context and switches to the guest,
/// then performs ERET to enter the guest at EL1.
///
/// # Arguments
/// * `context` - Pointer to VCPU context
///
/// # Safety
/// Must be called from EL2, function never returns (it does ERET to guest)
#[inline]
pub unsafe fn switch_to_guest(context: *const VcpuContext) -> ! {
    __vcpu_switch_to_guest(context);
    unreachable!("ERET to guest should never return");
}

/// Initialize VCPU context switching
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing VCPU context switching");
    log::info!("VCPU context switching initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offsets() {
        assert_eq!(SavedGprsOffsets::X1, 0x00);
        assert_eq!(SavedGprsOffsets::X30, 0xE8);
        assert_eq!(SavedGprsOffsets::SP, 0xF0);
    }

    #[test]
    fn test_saved_gprs() {
        let mut gprs = SavedGprs::new();
        assert_eq!(gprs.get(1), 0);
        assert_eq!(gprs.get_sp(), 0);

        gprs.set(5, 0x123456789);
        assert_eq!(gprs.get(5), 0x123456789);

        gprs.set_sp(0xAABBCCDD);
        assert_eq!(gprs.get_sp(), 0xAABBCCDD);
    }

    #[test]
    fn test_vfp_regs_size() {
        assert_eq!(core::mem::size_of::<VfpRegs>(), 528);
    }
}
