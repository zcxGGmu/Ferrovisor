//! PSCI (Power State Coordination Interface) for ARM64
//!
//! Provides PSCI v0.2/v1.0 implementation for ARM64 systems.
//! Reference: ARM DEN 0022D - Power State Coordination Interface
//!
//! PSCI is a standard interface between OS and firmware for:
//! - CPU power management (suspend, on, off)
//! - System power management (system reset, system off)
//! - CPU affinity management
//! - Migration support

pub mod smccc;
pub mod cpu_state;

// Re-export commonly used types
pub use smccc::*;
pub use cpu_state::*;

/// PSCI v0.2 function base
pub const PSCI_0_2_FN_BASE: u32 = 0x84000000;

/// PSCI v0.2 64-bit function base
pub const PSCI_0_2_64BIT: u32 = 0x40000000;

/// PSCI v0.2 64-bit function base
pub const PSCI_0_2_FN64_BASE: u32 = PSCI_0_2_FN_BASE + PSCI_0_2_64BIT;

/// PSCI function ID constructor
pub const fn psci_0_2_fn(n: u32) -> u32 {
    PSCI_0_2_FN_BASE + n
}

/// PSCI 64-bit function ID constructor
pub const fn psci_0_2_fn64(n: u32) -> u32 {
    PSCI_0_2_FN64_BASE + n
}

// PSCI v0.2 function IDs
/// PSCI version
pub const PSCI_0_2_FN_PSCI_VERSION: u32 = psci_0_2_fn(0);
/// CPU suspend
pub const PSCI_0_2_FN_CPU_SUSPEND: u32 = psci_0_2_fn(1);
/// CPU off
pub const PSCI_0_2_FN_CPU_OFF: u32 = psci_0_2_fn(2);
/// CPU on
pub const PSCI_0_2_FN_CPU_ON: u32 = psci_0_2_fn(3);
/// Affinity info
pub const PSCI_0_2_FN_AFFINITY_INFO: u32 = psci_0_2_fn(4);
/// Migrate
pub const PSCI_0_2_FN_MIGRATE: u32 = psci_0_2_fn(5);
/// Migrate info type
pub const PSCI_0_2_FN_MIGRATE_INFO_TYPE: u32 = psci_0_2_fn(6);
/// Migrate info up CPU
pub const PSCI_0_2_FN_MIGRATE_INFO_UP_CPU: u32 = psci_0_2_fn(7);
/// System off
pub const PSCI_0_2_FN_SYSTEM_OFF: u32 = psci_0_2_fn(8);
/// System reset
pub const PSCI_0_2_FN_SYSTEM_RESET: u32 = psci_0_2_fn(9);

// PSCI v0.2 64-bit function IDs
/// CPU suspend (64-bit)
pub const PSCI_0_2_FN64_CPU_SUSPEND: u32 = psci_0_2_fn64(1);
/// CPU on (64-bit)
pub const PSCI_0_2_FN64_CPU_ON: u32 = psci_0_2_fn64(3);
/// Affinity info (64-bit)
pub const PSCI_0_2_FN64_AFFINITY_INFO: u32 = psci_0_2_fn64(4);
/// Migrate (64-bit)
pub const PSCI_0_2_FN64_MIGRATE: u32 = psci_0_2_fn64(5);
/// Migrate info up CPU (64-bit)
pub const PSCI_0_2_FN64_MIGRATE_INFO_UP_CPU: u32 = psci_0_2_fn64(7);

// PSCI v0.1 function IDs (implementation defined)
pub const PSCI_FN_BASE: u32 = 0x95c1ba5e;
pub const fn psci_fn(n: u32) -> u32 {
    PSCI_FN_BASE + n
}
pub const PSCI_FN_CPU_SUSPEND: u32 = psci_fn(0);
pub const PSCI_FN_CPU_OFF: u32 = psci_fn(1);
pub const PSCI_FN_CPU_ON: u32 = psci_fn(2);
pub const PSCI_FN_MIGRATE: u32 = psci_fn(3);

/// PSCI v0.2 power state encoding for CPU_SUSPEND
pub const PSCI_0_2_POWER_STATE_ID_MASK: u32 = 0xffff;
pub const PSCI_0_2_POWER_STATE_ID_SHIFT: u32 = 0;
pub const PSCI_0_2_POWER_STATE_TYPE_SHIFT: u32 = 16;
pub const PSCI_0_2_POWER_STATE_TYPE_MASK: u32 = 0x1 << PSCI_0_2_POWER_STATE_TYPE_SHIFT;
pub const PSCI_0_2_POWER_STATE_AFFL_SHIFT: u32 = 24;
pub const PSCI_0_2_POWER_STATE_AFFL_MASK: u32 = 0x3 << PSCI_0_2_POWER_STATE_AFFL_SHIFT;

/// PSCI power state type
pub const PSCI_POWER_STATE_TYPE_POWER_DOWN: u32 = 0;
pub const PSCI_POWER_STATE_TYPE_STANDBY: u32 = 1;

/// PSCI power state affinity level
pub const PSCI_POWER_STATE_AFFL0: u32 = 0;
pub const PSCI_POWER_STATE_AFFL1: u32 = 1;
pub const PSCI_POWER_STATE_AFFL2: u32 = 2;
pub const PSCI_POWER_STATE_AFFL3: u32 = 3;

/// PSCI v0.2 affinity level states
pub const PSCI_0_2_AFFINITY_LEVEL_ON: u32 = 0;
pub const PSCI_0_2_AFFINITY_LEVEL_OFF: u32 = 1;
pub const PSCI_0_2_AFFINITY_LEVEL_ON_PENDING: u32 = 2;

/// PSCI v0.2 Trusted OS migration support
pub const PSCI_0_2_TOS_UP_MIGRATE: u32 = 0;
pub const PSCI_0_2_TOS_UP_NO_MIGRATE: u32 = 1;
pub const PSCI_0_2_TOS_MP: u32 = 2;

/// PSCI version decoding
pub const PSCI_VERSION_MAJOR_SHIFT: u32 = 16;
pub const PSCI_VERSION_MINOR_MASK: u32 = (1u32 << PSCI_VERSION_MAJOR_SHIFT) - 1;
pub const PSCI_VERSION_MAJOR_MASK: u32 = !PSCI_VERSION_MINOR_MASK;

pub const fn psci_version_major(ver: u32) -> u32 {
    (ver & PSCI_VERSION_MAJOR_MASK) >> PSCI_VERSION_MAJOR_SHIFT
}

pub const fn psci_version_minor(ver: u32) -> u32 {
    ver & PSCI_VERSION_MINOR_MASK
}

pub const fn psci_version(major: u32, minor: u32) -> u32 {
    (major << PSCI_VERSION_MAJOR_SHIFT) | minor
}

/// PSCI return values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum PsciReturn {
    Success = 0,
    NotSupported = -1,
    InvalidParams = -2,
    Denied = -3,
    AlreadyOn = -4,
    OnPending = -5,
    InternalFailure = -6,
    NotPresent = -7,
    Disabled = -8,
}

impl PsciReturn {
    /// Convert from i64
    pub fn from_i64(val: i64) -> Self {
        match val {
            0 => Self::Success,
            -1 => Self::NotSupported,
            -2 => Self::InvalidParams,
            -3 => Self::Denied,
            -4 => Self::AlreadyOn,
            -5 => Self::OnPending,
            -6 => Self::InternalFailure,
            -7 => Self::NotPresent,
            -8 => Self::Disabled,
            _ => Self::InternalFailure,
        }
    }

    /// Convert to i64
    pub fn to_i64(self) -> i64 {
        self as i64
    }

    /// Convert to u64 for x0 register
    pub fn to_u64(self) -> u64 {
        self.to_i64() as u64
    }

    /// Check if success
    pub fn is_success(self) -> bool {
        self == Self::Success
    }

    /// Get error message
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "Success",
            Self::NotSupported => "Not supported",
            Self::InvalidParams => "Invalid parameters",
            Self::Denied => "Denied",
            Self::AlreadyOn => "Already on",
            Self::OnPending => "On pending",
            Self::InternalFailure => "Internal failure",
            Self::NotPresent => "Not present",
            Self::Disabled => "Disabled",
        }
    }
}

/// PSCI context for handling PSCI calls
#[derive(Debug, Clone)]
pub struct PsciContext {
    /// PSCI version (0x10000 for v0.1, 0x20000 for v0.2, 0x10000 for v1.0)
    pub version: u32,
    /// PSCI available
    pub available: bool,
}

impl Default for PsciContext {
    fn default() -> Self {
        Self {
            version: psci_version(0, 2), // Default to PSCI v0.2
            available: true,
        }
    }
}

impl PsciContext {
    /// Create new PSCI context
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with specific version
    pub fn with_version(major: u32, minor: u32) -> Self {
        Self {
            version: psci_version(major, minor),
            available: true,
        }
    }

    /// Get PSCI version as (major, minor)
    pub fn version_tuple(&self) -> (u32, u32) {
        (psci_version_major(self.version), psci_version_minor(self.version))
    }

    /// Check if PSCI v0.1
    pub fn is_0_1(&self) -> bool {
        self.version_tuple() == (0, 1)
    }

    /// Check if PSCI v0.2 or later
    pub fn is_0_2_or_later(&self) -> bool {
        let (major, _) = self.version_tuple();
        major >= 0
    }

    /// Get version string
    pub fn version_string(&self) -> String {
        let (major, minor) = self.version_tuple();
        format!("v{}.{}", major, minor)
    }

    /// Handle PSCI call
    ///
    /// Returns the return value to be placed in x0
    pub fn handle_call(&self, function_id: u32, args: &[u64]) -> PsciReturn {
        log::debug!("PSCI: Handling call 0x{:08x} (version: {})",
                    function_id, self.version_string());

        let fn_id = function_id & 0xFF;

        match self.version_tuple() {
            (0, 1) => self.handle_0_1_call(fn_id, args),
            (0, 2) | (1, 0) => self.handle_0_2_call(function_id, args),
            _ => PsciReturn::NotSupported,
        }
    }

    /// Handle PSCI v0.1 call
    fn handle_0_1_call(&self, fn_id: u32, _args: &[u64]) -> PsciReturn {
        match fn_id {
            0 => PsciReturn::NotSupported, // CPU_SUSPEND
            1 => PsciReturn::Success,     // CPU_OFF - simplified
            2 => PsciReturn::NotSupported, // CPU_ON - requires VCPU management
            3 => PsciReturn::NotSupported, // MIGRATE
            _ => PsciReturn::NotSupported,
        }
    }

    /// Handle PSCI v0.2/v1.0 call
    fn handle_0_2_call(&self, function_id: u32, _args: &[u64]) -> PsciReturn {
        match function_id {
            PSCI_0_2_FN_PSCI_VERSION => PsciReturn::Success,
            PSCI_0_2_FN_CPU_SUSPEND | PSCI_0_2_FN64_CPU_SUSPEND => {
                // Simplified: treat as WFI
                PsciReturn::Success
            }
            PSCI_0_2_FN_CPU_OFF => PsciReturn::Success,
            PSCI_0_2_FN_CPU_ON | PSCI_0_2_FN64_CPU_ON => {
                // Requires VCPU management - return not supported for now
                PsciReturn::NotSupported
            }
            PSCI_0_2_FN_AFFINITY_INFO | PSCI_0_2_FN64_AFFINITY_INFO => {
                // Return OFF for simplicity
                PsciReturn::NotPresent
            }
            PSCI_0_2_FN_MIGRATE | PSCI_0_2_FN64_MIGRATE => PsciReturn::NotSupported,
            PSCI_0_2_FN_MIGRATE_INFO_TYPE => PsciReturn::Success,
            PSCI_0_2_FN_MIGRATE_INFO_UP_CPU | PSCI_0_2_FN64_MIGRATE_INFO_UP_CPU => {
                PsciReturn::NotSupported
            }
            PSCI_0_2_FN_SYSTEM_OFF => PsciReturn::Success,
            PSCI_0_2_FN_SYSTEM_RESET => PsciReturn::Success,
            _ => PsciReturn::NotSupported,
        }
    }

    /// Dump PSCI state for debugging
    pub fn dump(&self) {
        log::info!("PSCI Context:");
        log::info!("  Version: {}", self.version_string());
        log::info!("  Available: {}", self.available);
    }
}

/// Global PSCI context
static mut PSCI_CTX: Option<PsciContext> = None;

/// Initialize PSCI
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing PSCI");

    let ctx = PsciContext::new();
    unsafe {
        PSCI_CTX = Some(ctx);
    }

    log::info!("PSCI initialized: version {}",
                unsafe { PSCI_CTX.as_ref().unwrap().version_string() });

    Ok(())
}

/// Get PSCI context
pub fn context() -> Option<&'static PsciContext> {
    unsafe { PSCI_CTX.as_ref() }
}

/// Get mutable PSCI context
pub fn context_mut() -> Option<&'static mut PsciContext> {
    unsafe { PSCI_CTX.as_mut() }
}

/// Check if PSCI is available
pub fn is_available() -> bool {
    context().map(|ctx| ctx.available).unwrap_or(false)
}

/// Handle PSCI SMC call
pub fn handle_smc(function_id: u32, args: &[u64]) -> (u64, PsciReturn) {
    let ret = if let Some(ctx) = context() {
        ctx.handle_call(function_id, args)
    } else {
        PsciReturn::InternalFailure
    };

    // Return value in x0
    // For PSCI_VERSION, return the version number
    let x0 = if function_id == PSCI_0_2_FN_PSCI_VERSION {
        if let Some(ctx) = context() {
            ctx.version as u64
        } else {
            ret.to_u64()
        }
    } else {
        ret.to_u64()
    };

    (x0, ret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_psci_version() {
        assert_eq!(psci_version_major(0x20000), 0);
        assert_eq!(psci_version_minor(0x20000), 2);
        assert_eq!(psci_version(0, 2), 0x20000);
    }

    #[test]
    fn test_psci_fn_macros() {
        assert_eq!(PSCI_0_2_FN_PSCI_VERSION, 0x84000000);
        assert_eq!(PSCI_0_2_FN_CPU_ON, 0x84000003);
        assert_eq!(PSCI_0_2_FN64_CPU_ON, 0xC4000003);
    }

    #[test]
    fn test_psci_return() {
        let ret = PsciReturn::Success;
        assert!(ret.is_success());
        assert_eq!(ret.to_i64(), 0);
        assert_eq!(ret.to_u64(), 0);

        let ret = PsciReturn::InvalidParams;
        assert!(!ret.is_success());
        assert_eq!(ret.to_i64(), -2);
    }

    #[test]
    fn test_psci_context() {
        let ctx = PsciContext::new();
        assert_eq!(ctx.version_tuple(), (0, 2));
        assert!(ctx.is_0_2_or_later());
    }

    #[test]
    fn test_psci_context_version_string() {
        let ctx = PsciContext::with_version(1, 0);
        assert_eq!(ctx.version_string(), "v1.0");
    }

    #[test]
    fn test_psci_handle_psci_version() {
        let ctx = PsciContext::new();
        let ret = ctx.handle_call(PSCI_0_2_FN_PSCI_VERSION, &[]);
        assert_eq!(ret, PsciReturn::Success);
    }

    #[test]
    fn test_psci_handle_cpu_off() {
        let ctx = PsciContext::new();
        let ret = ctx.handle_call(PSCI_0_2_FN_CPU_OFF, &[]);
        assert_eq!(ret, PsciReturn::Success);
    }

    #[test]
    fn test_psci_handle_unknown() {
        let ctx = PsciContext::new();
        let ret = ctx.handle_call(0x840000FF, &[]);
        assert_eq!(ret, PsciReturn::NotSupported);
    }
}
