//! RISC-V SBI (Supervisor Binary Interface) for SMP
//!
//! This module provides SBI calls for SMP operations including:
//! - Hart (CPU) management
/// - IPI sending
/// - Remote fence operations
/// - CPU suspend/resume

use crate::arch::riscv64::*;

/// SBI error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(isize)]
pub enum SbiError {
    Success = 0,
    Failed = -1,
    NotSupported = -2,
    InvalidParam = -3,
    Denied = -4,
    InvalidAddress = -5,
    AlreadyAvailable = -6,
    AlreadyStarted = -7,
    AlreadyStopped = -8,
    NoShmem = -9,
    InvalidShmemSize = -10,
    NoResume = -11,
}

impl SbiError {
    /// Convert from raw error code
    pub fn from_raw(code: isize) -> Self {
        match code {
            0 => SbiError::Success,
            -1 => SbiError::Failed,
            -2 => SbiError::NotSupported,
            -3 => SbiError::InvalidParam,
            -4 => SbiError::Denied,
            -5 => SbiError::InvalidAddress,
            -6 => SbiError::AlreadyAvailable,
            -7 => SbiError::AlreadyStarted,
            -8 => SbiError::AlreadyStopped,
            -9 => SbiError::NoShmem,
            -10 => SbiError::InvalidShmemSize,
            -11 => SbiError::NoResume,
            _ => SbiError::Failed,
        }
    }

    /// Convert to result
    pub fn into_result(self) -> Result<(), &'static str> {
        match self {
            SbiError::Success => Ok(()),
            _ => Err("SBI call failed"),
        }
    }
}

/// SBI extensions
pub mod sbi_ext {
    pub const HSM_START: usize = 0x48534D;
    pub const HSM_STOP: usize = 0x48534D;
    pub const HSM_GET_STATUS: usize = 0x48534M;
    pub const HSM_SUSPEND: usize = 0x48534D;
    pub const HSM_RESUME: usize = 0x48534D;

    pub const IPI_SEND_IPI: usize = 0x735049;

    pub const RFENCE_REMOTE_FENCE_I: usize = 0x52464E;
    pub const RFENCE_REMOTE_SFENCE_VMA: usize = 0x52464E;
    pub const RFENCE_REMOTE_SFENCE_VMA_ASID: usize = 0x52464E;
    pub const RFENCE_REMOTE_HFENCE_GVMA: usize = 0x52464E;
    pub const RFENCE_REMOTE_HFENCE_GVMA_VMID: usize = 0x52464E;
}

/// SBI HSM (Hart State Management) states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum HartState {
    Started = 0,
    Stopped = 1,
    StartPending = 2,
    StopPending = 3,
    Suspended = 4,
    SuspendPending = 5,
    ResumePending = 6,
    Unknown = 0xFFFFFFFF,
}

impl HartState {
    /// Convert from raw state value
    pub fn from_raw(state: u32) -> Self {
        match state {
            0 => HartState::Started,
            1 => HartState::Stopped,
            2 => HartState::StartPending,
            3 => HartState::StopPending,
            4 => HartState::Suspended,
            5 => HartState::SuspendPending,
            6 => HartState::ResumePending,
            _ => HartState::Unknown,
        }
    }
}

/// SBI suspend types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuspendType {
    /// Retentive suspend (keeps state)
    Retentive = 0,
    /// Non-retentive suspend (loses state)
    NonRetentive = 1,
}

/// Perform SBI call
fn sbi_call(
    ext_id: usize,
    func_id: usize,
    arg0: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
) -> (usize, usize) {
    let mut error: usize;
    let mut value: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            lateout("a0") error,
            lateout("a1") value,
            in("a6") ext_id,
            in("a7") func_id,
            in("a0") arg0,
            in("a1") arg1,
            in("a2") arg2,
            in("a3") arg3,
            in("a4") arg4,
            in("a5") arg5,
        );
    }

    (error, value)
}

/// Start a hart
pub fn sbi_hart_start(hart_id: usize, start_addr: usize, priv: usize) -> Result<(), SbiError> {
    let (error, _) = sbi_call(
        sbi_ext::HSM_START,
        0, // HART_START function ID
        hart_id,
        start_addr,
        priv,
        0, 0, 0,
    );

    match SbiError::from_raw(error as isize) {
        SbiError::Success => Ok(()),
        e => Err(e),
    }
}

/// Stop a hart
pub fn sbi_hart_stop() -> Result<(), SbiError> {
    let (error, _) = sbi_call(
        sbi_ext::HSM_STOP,
        0, // HART_STOP function ID
        0, 0, 0, 0, 0, 0, 0,
    );

    match SbiError::from_raw(error as isize) {
        SbiError::Success => Ok(()),
        e => Err(e),
    }
}

/// Get hart status
pub fn sbi_hart_get_status(hart_id: usize) -> Result<HartState, SbiError> {
    let (error, value) = sbi_call(
        sbi_ext::HSM_GET_STATUS,
        0, // HART_GET_STATUS function ID
        hart_id,
        0, 0, 0, 0, 0, 0,
    );

    match SbiError::from_raw(error as isize) {
        SbiError::Success => Ok(HartState::from_raw(value as u32)),
        e => Err(e),
    }
}

/// Suspend a hart
pub fn sbi_hart_suspend(
    suspend_type: SuspendType,
    resume_addr: usize,
    opaque: usize,
) -> Result<(), SbiError> {
    let (error, _) = sbi_call(
        sbi_ext::HSM_SUSPEND,
        0, // HART_SUSPEND function ID
        suspend_type as usize,
        resume_addr,
        opaque,
        0, 0, 0, 0,
    );

    match SbiError::from_raw(error as isize) {
        SbiError::Success => Ok(()),
        e => Err(e),
    }
}

/// Resume a suspended hart
pub fn sbi_hart_resume(hart_id: usize, resume_addr: usize, opaque: usize) -> Result<(), SbiError> {
    let (error, _) = sbi_call(
        sbi_ext::HSM_RESUME,
        0, // HART_RESUME function ID
        hart_id,
        resume_addr,
        opaque,
        0, 0, 0, 0,
    );

    match SbiError::from_raw(error as isize) {
        SbiError::Success => Ok(()),
        e => Err(e),
    }
}

/// Send IPI to harts
pub fn sbi_send_ipi(hart_mask: usize, hart_mask_base: usize) -> Result<(), SbiError> {
    let (error, _) = sbi_call(
        sbi_ext::IPI_SEND_IPI,
        0, // SEND_IPI function ID
        hart_mask,
        hart_mask_base,
        0, 0, 0, 0, 0,
    );

    match SbiError::from_raw(error as isize) {
        SbiError::Success => Ok(()),
        e => Err(e),
    }
}

/// Remote fence I
pub fn sbi_remote_fence_i(hart_mask: usize, hart_mask_base: usize) -> Result<(), SbiError> {
    let (error, _) = sbi_call(
        sbi_ext::RFENCE_REMOTE_FENCE_I,
        0, // REMOTE_FENCE_I function ID
        hart_mask,
        hart_mask_base,
        0, 0, 0, 0, 0,
    );

    match SbiError::from_raw(error as isize) {
        SbiError::Success => Ok(()),
        e => Err(e),
    }
}

/// Remote SFENCE.VMA
pub fn sbi_remote_sfence_vma(
    hart_mask: usize,
    hart_mask_base: usize,
    start_addr: usize,
    size: usize,
) -> Result<(), SbiError> {
    let (error, _) = sbi_call(
        sbi_ext::RFENCE_REMOTE_SFENCE_VMA,
        0, // REMOTE_SFENCE_VMA function ID
        hart_mask,
        hart_mask_base,
        start_addr,
        size,
        0, 0, 0,
    );

    match SbiError::from_raw(error as isize) {
        SbiError::Success => Ok(()),
        e => Err(e),
    }
}

/// Remote SFENCE.VMA with ASID
pub fn sbi_remote_sfence_vma_asid(
    hart_mask: usize,
    hart_mask_base: usize,
    start_addr: usize,
    size: usize,
    asid: usize,
) -> Result<(), SbiError> {
    let (error, _) = sbi_call(
        sbi_ext::RFENCE_REMOTE_SFENCE_VMA_ASID,
        0, // REMOTE_SFENCE_VMA_ASID function ID
        hart_mask,
        hart_mask_base,
        start_addr,
        size,
        asid,
        0, 0,
    );

    match SbiError::from_raw(error as isize) {
        SbiError::Success => Ok(()),
        e => Err(e),
    }
}

/// Remote HFENCE.GVMA
pub fn sbi_remote_hfence_gvma(
    hart_mask: usize,
    hart_mask_base: usize,
    start_addr: usize,
    size: usize,
) -> Result<(), SbiError> {
    let (error, _) = sbi_call(
        sbi_ext::RFENCE_REMOTE_HFENCE_GVMA,
        0, // REMOTE_HFENCE_GVMA function ID
        hart_mask,
        hart_mask_base,
        start_addr,
        size,
        0, 0, 0,
    );

    match SbiError::from_raw(error as isize) {
        SbiError::Success => Ok(()),
        e => Err(e),
    }
}

/// Remote HFENCE.GVMA with VMID
pub fn sbi_remote_hfence_gvma_vmid(
    hart_mask: usize,
    hart_mask_base: usize,
    start_addr: usize,
    size: usize,
    vmid: usize,
) -> Result<(), SbiError> {
    let (error, _) = sbi_call(
        sbi_ext::RFENCE_REMOTE_HFENCE_GVMA_VMID,
        0, // REMOTE_HFENCE_GVMA_VMID function ID
        hart_mask,
        hart_mask_base,
        start_addr,
        size,
        vmid,
        0, 0,
    );

    match SbiError::from_raw(error as isize) {
        SbiError::Success => Ok(()),
        e => Err(e),
    }
}

/// Check if SBI HSM extension is available
pub fn is_hsm_available() -> bool {
    let (error, _) = sbi_call(
        sbi_ext::HSM_START,
        0,
        0, 0, 0, 0, 0, 0, 0,
    );

    // If error is not "NotSupported", extension is available
    error != (SbiError::NotSupported as usize)
}

/// Check if SBI IPI extension is available
pub fn is_ipi_available() -> bool {
    let (error, _) = sbi_call(
        sbi_ext::IPI_SEND_IPI,
        0,
        0, 0, 0, 0, 0, 0, 0,
    );

    error != (SbiError::NotSupported as usize)
}

/// Check if SBI RFENCE extension is available
pub fn is_rfence_available() -> bool {
    let (error, _) = sbi_call(
        sbi_ext::RFENCE_REMOTE_FENCE_I,
        0,
        0, 0, 0, 0, 0, 0, 0,
    );

    error != (SbiError::NotSupported as usize)
}

/// Initialize SBI for SMP
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing SBI for SMP");

    // Check required extensions
    if !is_hsm_available() {
        log::warn!("SBI HSM extension not available");
        return Err("SBI HSM extension not available");
    }

    if !is_ipi_available() {
        log::warn!("SBI IPI extension not available");
        return Err("SBI IPI extension not available");
    }

    if !is_rfence_available() {
        log::warn!("SBI RFENCE extension not available");
        return Err("SBI RFENCE extension not available");
    }

    log::info!("SBI extensions available: HSM={}, IPI={}, RFENCE={}",
             is_hsm_available(),
             is_ipi_available(),
             is_rfence_available());

    log::info!("SBI initialized for SMP");
    Ok(())
}

/// Get SBI implementation version
pub fn get_sbi_version() -> (u32, u32) {
    let (error, value) = sbi_call(
        0x10, // Base extension
        0,    // SBI_GET_SBI_VERSION
        0, 0, 0, 0, 0, 0, 0,
    );

    if error == 0 {
        let major = (value >> 24) as u32;
        let minor = (value & 0xFFFFFF) as u32;
        (major, minor)
    } else {
        (0, 0)
    }
}

/// Get SBI implementation ID
pub fn get_sbi_impl_id() -> u32 {
    let (error, value) = sbi_call(
        0x10, // Base extension
        1,    // SBI_GET_IMPL_ID
        0, 0, 0, 0, 0, 0, 0,
    );

    if error == 0 {
        value as u32
    } else {
        0
    }
}

/// Get SBI vendor ID
pub fn get_sbi_vendor_id() -> u32 {
    let (error, value) = sbi_call(
        0x10, // Base extension
        2,    // SBI_GET_VENDOR_ID
        0, 0, 0, 0, 0, 0, 0,
    );

    if error == 0 {
        value as u32
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sbi_error() {
        assert_eq!(SbiError::Success as isize, 0);
        assert_eq!(SbiError::Failed as isize, -1);
        assert_eq!(SbiError::NotSupported as isize, -2);

        let error = SbiError::from_raw(-3);
        assert_eq!(error, SbiError::InvalidParam);

        assert!(SbiError::Success.into_result().is_ok());
        assert!(SbiError::Failed.into_result().is_err());
    }

    #[test]
    fn test_hart_state() {
        assert_eq!(HartState::Started, HartState::from_raw(0));
        assert_eq!(HartState::Stopped, HartState::from_raw(1));
        assert_eq!(HartState::Suspended, HartState::from_raw(4));
        assert_eq!(HartState::Unknown, HartState::from_raw(999));
    }

    #[test]
    fn test_suspend_type() {
        assert_eq!(SuspendType::Retentive as u32, 0);
        assert_eq!(SuspendType::NonRetentive as u32, 1);
    }

    #[test]
    fn test_sbi_version() {
        let (major, minor) = get_sbi_version();
        println!("SBI version: {}.{}", major, minor);
    }

    #[test]
    fn test_sbi_info() {
        let impl_id = get_sbi_impl_id();
        let vendor_id = get_sbi_vendor_id();
        println!("SBI impl ID: {:#x}, vendor ID: {:#x}", impl_id, vendor_id);
    }

    #[test]
    fn test_sbi_extensions() {
        println!("HSM available: {}", is_hsm_available());
        println!("IPI available: {}", is_ipi_available());
        println!("RFENCE available: {}", is_rfence_available());
    }
}