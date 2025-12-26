//! Stage-2 Fault Handling for ARM64
//!
//! Provides fault decoding and handling for Stage-2 page faults.
//! Reference: ARM DDI 0487I.a - Chapter D13 - Exception Syndrome Register

use crate::{Result, Error};
use crate::arch::arm64::mm::{gstage, translate};

/// Stage-2 fault information
#[derive(Debug, Clone, Copy)]
pub struct FaultInfo {
    /// Fault type
    pub fault: Stage2Fault,
    /// Fault address (IPA that caused the fault)
    pub ipa: u64,
    /// Fault status code from ESR_EL2
    pub status_code: u64,
    /// Instruction Syndrome (for instruction aborts)
    pub iss: u64,
    /// Fault came from secure state
    pub s1ptw: bool,
    /// Fault on stage 2 translation
    pub is_stage2: bool,
    /// Write or not (0 = read, 1 = write)
    pub write: bool,
    /// Instruction fetch or not
    pub instruction: bool,
}

/// Stage-2 fault types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage2Fault {
    /// Translation fault (page not mapped)
    Translation {
        /// Level where fault occurred
        level: u32,
    },
    /// Access flag fault
    AccessFlag {
        /// Level where fault occurred
        level: u32,
    },
    /// Permission fault
    Permission {
        /// Level where fault occurred
        level: u32,
    },
    /// Address size fault (IPA too large)
    AddressSize,
    /// Alignment fault
    Alignment,
    /// TLB conflict abort (ARMv8.2+)
    TlbConflict,
    /// Unsupported atomic operation
    UnsupportedAtomic,
    /// Memory copy collision (ARMv8.5+)
    MemoryCopy,
    /// Hardware update to dirty state
    HardwareUpdateDirty,
    /// Hardware access flag update
    HardwareUpdateAccessFlag,
    /// Unknown fault
    Unknown,
}

impl FaultInfo {
    /// Decode fault from ESR_EL2
    ///
    /// ESR_EL2 format for Data Abort (ISS format):
    /// - [24] ISV: Instruction Syndrome Valid
    /// - [9] DFSC: Data Fault Status Code (0b100000 = Stage-2 Translation)
    /// - [8] FnV: Fault not valid
    /// - [7:6] S1PTW: Stage 1 permission walk fault
    /// - [5:0] FSC: Fault Status Code
    ///
    /// ESR_EL2 format for Instruction Abort (ISS format):
    /// - [9] IFSC: Instruction Fault Status Code
    /// - [8] FnV: Fault not valid
    /// - [7:6] S1PTW: Stage 1 permission walk fault
    /// - [5:0] FSC: Instruction Fault Status Code
    pub fn from_esr(el2_esr: u64, far: u64, is_instruction_abort: bool) -> Self {
        let iss = el2_esr & 0x1FFFFFF;
        let fsc = iss & 0x3F;
        let status_code = if is_instruction_abort {
            ((el2_esr >> 9) & 0x3F) << 6
        } else {
            ((el2_esr >> 9) & 0x3F) << 6
        };

        let write = if !is_instruction_abort {
            (iss >> 6) & 0x1 != 0
        } else {
            false
        };

        let s1ptw = (iss >> 7) & 0x1 != 0;

        // Check if it's a Stage-2 fault
        let is_stage2 = (fsc & 0x3C) == 0x20 || (fsc & 0x3C) == 0x24;

        // Decode fault type
        let fault = if is_stage2 {
            Self::decode_stage2_fault(fsc)
        } else {
            Stage2Fault::Unknown
        };

        FaultInfo {
            fault,
            ipa: far,
            status_code,
            iss,
            s1ptw,
            is_stage2,
            write,
            instruction: is_instruction_abort,
        }
    }

    /// Decode Stage-2 specific fault
    fn decode_stage2_fault(fsc: u64) -> Stage2Fault {
        let code = (fsc & 0x3C) >> 2;
        let level = (fsc & 0x3) as u32;

        match code {
            // Translation fault
            0b1000 => Stage2Fault::Translation { level },
            // Access flag fault
            0b1001 => Stage2Fault::AccessFlag { level },
            // Permission fault
            0b1011 => Stage2Fault::Permission { level },

            // Address size fault
            0b0101 | 0b0100 => Stage2Fault::AddressSize,

            // Alignment fault
            0b0001 => Stage2Fault::Alignment,

            // TLB conflict abort
            0b1100 | 0b1101 | 0b1110 | 0b1111 => Stage2Fault::TlbConflict,

            // Unsupported atomic operation
            0b10000 => Stage2Fault::UnsupportedAtomic,

            // Memory copy collision
            0b10001 => Stage2Fault::MemoryCopy,

            // Hardware update to dirty state (ARMv8.4+)
            0b0011 => Stage2Fault::HardwareUpdateDirty,

            // Hardware access flag update
            0b0010 => Stage2Fault::HardwareUpdateAccessFlag,

            _ => Stage2Fault::Unknown,
        }
    }

    /// Check if fault is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self.fault {
            Stage2Fault::Translation { .. } => true,
            Stage2Fault::AccessFlag { .. } => true,
            Stage2Fault::Permission { .. } => true,
            Stage2Fault::Alignment => true,
            Stage2Fault::TlbConflict => true,
            Stage2Fault::HardwareUpdateDirty => true,
            Stage2Fault::HardwareUpdateAccessFlag => true,
            Stage2Fault::AddressSize => false,
            Stage2Fault::UnsupportedAtomic => false,
            Stage2Fault::MemoryCopy => false,
            Stage2Fault::Unknown => false,
        }
    }

    /// Get human-readable fault description
    pub fn description(&self) -> &'static str {
        match self.fault {
            Stage2Fault::Translation { level } => {
                "Translation fault (page not mapped)"
            }
            Stage2Fault::AccessFlag { level } => {
                "Access flag fault (page not accessed)"
            }
            Stage2Fault::Permission { level } => {
                "Permission fault (access denied)"
            }
            Stage2Fault::AddressSize => {
                "Address size fault (IPA too large)"
            }
            Stage2Fault::Alignment => {
                "Alignment fault"
            }
            Stage2Fault::TlbConflict => {
                "TLB conflict abort"
            }
            Stage2Fault::UnsupportedAtomic => {
                "Unsupported atomic operation"
            }
            Stage2Fault::MemoryCopy => {
                "Memory copy collision"
            }
            Stage2Fault::HardwareUpdateDirty => {
                "Hardware update to dirty state"
            }
            Stage2Fault::HardwareUpdateAccessFlag => {
                "Hardware access flag update"
            }
            Stage2Fault::Unknown => {
                "Unknown fault"
            }
        }
    }

    /// Get fault level
    pub fn level(&self) -> Option<u32> {
        match self.fault {
            Stage2Fault::Translation { level } |
            Stage2Fault::AccessFlag { level } |
            Stage2Fault::Permission { level } => Some(level),
            _ => None,
        }
    }
}

/// Handle a Stage-2 fault
///
/// This function handles Stage-2 page faults by either:
/// 1. Resolving the fault by mapping the page (if possible)
/// 2. Returning an error to inject an exception into the guest
///
/// # Arguments
/// * `fault_info` - Fault information decoded from ESR_EL2
/// * `vmid` - VM ID for the faulting guest
///
/// # Returns
/// Ok(()) if fault was handled, Err otherwise
pub fn handle_stage2_fault(fault_info: FaultInfo, vmid: u16) -> Result<bool> {
    log::warn!(
        "Stage-2 fault for VMID {}: {} at IPA {:#x}, write={}, instruction={}",
        vmid,
        fault_info.description(),
        fault_info.ipa,
        fault_info.write,
        fault_info.instruction
    );

    // Check if fault is recoverable
    if !fault_info.is_recoverable() {
        log::error!("Non-recoverable Stage-2 fault: {}", fault_info.description());
        return Err(Error::InvalidState);
    }

    // Try to handle the fault based on its type
    match fault_info.fault {
        Stage2Fault::Translation { level } => {
            handle_translation_fault(fault_info, vmid, level)
        }
        Stage2Fault::Permission { level } => {
            handle_permission_fault(fault_info, vmid, level)
        }
        Stage2Fault::AccessFlag { level } => {
            handle_access_flag_fault(fault_info, vmid, level)
        }
        Stage2Fault::Alignment => {
            handle_alignment_fault(fault_info, vmid)
        }
        Stage2Fault::HardwareUpdateDirty | Stage2Fault::HardwareUpdateAccessFlag => {
            // These are hardware-managed, just clear and continue
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// Handle translation fault (page not mapped)
fn handle_translation_fault(fault_info: FaultInfo, vmid: u16, level: u32) -> Result<bool> {
    // Try to resolve the fault by mapping the page
    // In a real implementation, this would:
    // 1. Check if the IPA should be mapped
    // 2. Allocate a physical page
    // 3. Create the mapping
    // 4. Flush TLB

    // For now, return false to inject exception to guest
    log::debug!("Translation fault at level {} for IPA {:#x}", level, fault_info.ipa);
    Ok(false)
}

/// Handle permission fault (access denied)
fn handle_permission_fault(fault_info: FaultInfo, vmid: u16, level: u32) -> Result<bool> {
    // Check if we can fix the permission issue
    // In a real implementation, this would:
    // 1. Check if the access should be allowed
    // 2. Update PTE permissions if needed
    // 3. Flush TLB

    log::debug!(
        "Permission fault at level {} for IPA {:#x}, write={}",
        level,
        fault_info.ipa,
        fault_info.write
    );
    Ok(false)
}

/// Handle access flag fault
fn handle_access_flag_fault(fault_info: FaultInfo, vmid: u16, level: u32) -> Result<bool> {
    // Set the access flag in the PTE
    // In a real implementation, this would:
    // 1. Find the PTE
    // 2. Set the AF bit
    // 3. Flush TLB

    log::debug!("Access flag fault at level {} for IPA {:#x}", level, fault_info.ipa);
    Ok(false)
}

/// Handle alignment fault
fn handle_alignment_fault(fault_info: FaultInfo, vmid: u16) -> Result<bool> {
    // Alignment faults are typically not recoverable
    log::debug!("Alignment fault for IPA {:#x}", fault_info.ipa);
    Ok(false)
}

/// Read ESR_EL2 and decode fault
///
/// This is a convenience function that reads ESR_EL2 and returns
/// the decoded fault information.
///
/// # Safety
/// Must be called from EL2
pub unsafe fn decode_fault_from_el2(is_instruction_abort: bool) -> FaultInfo {
    let mut el2_esr: u64;
    let mut far: u64;

    core::arch::asm!(
        "mrs {}, esr_el2",
        out(reg) el2_esr,
    );

    if is_instruction_abort {
        core::arch::asm!(
            "mrs {}, far_el2",
            out(reg) far,
        );
    } else {
        core::arch::asm!(
            "mrs {}, far_el2",
            out(reg) far,
        );
    }

    FaultInfo::from_esr(el2_esr, far, is_instruction_abort)
}

/// Fault handling result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultResolution {
    /// Fault was resolved, execution can continue
    Resolved,
    /// Fault could not be resolved, inject exception to guest
    InjectException,
    /// Fault is fatal, terminate VM
    Fatal,
}

/// Resolve a Stage-2 fault
///
/// This function attempts to resolve a Stage-2 fault by updating
/// page tables or taking other corrective action.
///
/// # Arguments
/// * `fault_info` - Fault information
/// * `vmid` - VM ID
///
/// # Returns
/// Fault resolution result
pub fn resolve_fault(fault_info: FaultInfo, vmid: u16) -> FaultResolution {
    match handle_stage2_fault(fault_info, vmid) {
        Ok(true) => FaultResolution::Resolved,
        Ok(false) => FaultResolution::InjectException,
        Err(_) => FaultResolution::Fatal,
    }
}

/// Inject Stage-2 fault into guest
///
/// This function prepares exception information to inject a Stage-2
/// fault exception into the guest.
///
/// # Arguments
/// * `fault_info` - Fault information
/// * `vmid` - VM ID
/// * `vcpu_id` - VCPU ID
///
/// # Returns
/// Exception information to inject
pub fn inject_stage2_fault(fault_info: FaultInfo, vmid: u16, vcpu_id: u32) -> Result<ExceptionInfo> {
    let exception_class = if fault_info.instruction {
        0x20 // Instruction abort from lower EL
    } else {
        0x24 // Data abort from lower EL
    };

    let iss = fault_info.iss;

    Ok(ExceptionInfo {
        exception_class,
        iss,
        fault_address: fault_info.ipa,
        vcpu_id,
    })
}

/// Exception information for injection
#[derive(Debug, Clone, Copy)]
pub struct ExceptionInfo {
    /// Exception class (for ESR_EL2)
    pub exception_class: u64,
    /// Instruction Specific Syndrome
    pub iss: u64,
    /// Fault address
    pub fault_address: u64,
    /// Target VCPU ID
    pub vcpu_id: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fault_decoding() {
        // Test translation fault at level 0
        let fsc = 0b100000; // Translation fault, level 0
        let fault = FaultInfo::decode_stage2_fault(fsc);
        assert!(matches!(fault, Stage2Fault::Translation { level: 0 }));

        // Test permission fault at level 3
        let fsc = 0b101111; // Permission fault, level 3
        let fault = FaultInfo::decode_stage2_fault(fsc);
        assert!(matches!(fault, Stage2Fault::Permission { level: 3 }));
    }

    #[test]
    fn test_fault_recoverability() {
        let info = FaultInfo {
            fault: Stage2Fault::Translation { level: 0 },
            ipa: 0x1000,
            status_code: 0,
            iss: 0,
            s1ptw: false,
            is_stage2: true,
            write: false,
            instruction: false,
        };
        assert!(info.is_recoverable());
    }
}
