//! SMCCC (SMC Calling Convention) for ARM64
//!
//! Provides SMC/HVC call handling for PSCI and other ARM standard calls.
//! Reference: ARM DEN 0028A - SMC Calling Convention
//!
//! SMCCC defines:
//! - SMC calling convention for calling secure firmware
//! - Function ID encoding (call type, calling convention, function number)
//! - Standard service calls (PSCI, SIP, SIP_SERVICE)
//! - Return value conventions

use super::{PsciReturn, PSCI_0_2_FN_PSCI_VERSION};

/// SMCCC function ID immediate mask
pub const SMCCC_FUNC_ID_MASK: u32 = 0xFFFF;

/// SMCCC function ID shift
pub const SMCCC_FUNC_ID_SHIFT: u32 = 0;

/// SMCCC fast call bit
pub const SMCCC_FAST_CALL: u32 = 0x80000000;

/// SMCCC standard call bit
pub const SMCCC_STD_CALL: u32 = 0x00000000;

/// SMCCC type mask
pub const SMCCC_CALL_TYPE_MASK: u32 = 0x80000000;

/// SMCCC 64-bit calling convention mask
pub const SMCCC_CALL_CONV_64: u32 = 0x40000000;

/// SMCCC 32-bit calling convention mask
pub const SMCCC_CALL_CONV_32: u32 = 0x00000000;

/// SMCCC service call type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SmcccCallType {
    /// Standard call (yielding)
    Standard = SMCCC_STD_CALL,
    /// Fast call (non-yielding, no preemption)
    Fast = SMCCC_FAST_CALL,
}

impl SmcccCallType {
    /// Create from raw value
    pub fn from_raw(value: u32) -> Self {
        match value & SMCCC_CALL_TYPE_MASK {
            SMCCC_FAST_CALL => Self::Fast,
            _ => Self::Standard,
        }
    }

    /// Get raw value
    pub fn raw(self) -> u32 {
        self as u32
    }

    /// Check if fast call
    pub fn is_fast(self) -> bool {
        matches!(self, Self::Fast)
    }

    /// Check if standard call
    pub fn is_standard(self) -> bool {
        !self.is_fast()
    }
}

/// SMCCC calling convention (register width)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SmcccCallConv {
    /// 32-bit calling convention (W0-W3)
    Bit32 = SMCCC_CALL_CONV_32,
    /// 64-bit calling convention (X0-X3)
    Bit64 = SMCCC_CALL_CONV_64,
}

impl SmcccCallConv {
    /// Create from raw value
    pub fn from_raw(value: u32) -> Self {
        match value & SMCCC_CALL_CONV_64 {
            SMCCC_CALL_CONV_64 => Self::Bit64,
            _ => Self::Bit32,
        }
    }

    /// Get raw value
    pub fn raw(self) -> u32 {
        self as u32
    }

    /// Check if 64-bit
    pub fn is_64bit(self) -> bool {
        matches!(self, Self::Bit64)
    }
}

/// SMCCC service type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SmcccService {
    /// Arm Architecture calls
    Arch = 0x00,
    /// CPU service calls
    CpuService = 0x01,
    /// SIP service calls
    SipService = 0x02,
    /// OEM service calls
    OemService = 0x03,
    /// Standard service calls
    StdService = 0x04,
    /// Trusted Hypervisor calls
    TrustedHypervisor = 0x05,
    /// Trusted OS calls
    TrustedOs = 0x06,
    /// Vendor specific calls
    VendorSpecific = 0x7F,
}

impl SmcccService {
    /// Create from raw value
    pub fn from_raw(value: u32) -> Self {
        let service = (value >> 24) & 0x3F;
        match service {
            0x00 => Self::Arch,
            0x01 => Self::CpuService,
            0x02 => Self::SipService,
            0x03 => Self::OemService,
            0x04 => Self::StdService,
            0x05 => Self::TrustedHypervisor,
            0x06 => Self::TrustedOs,
            0x7F => Self::VendorSpecific,
            _ => Self::VendorSpecific,
        }
    }

    /// Get raw value
    pub fn raw(self) -> u32 {
        self as u32
    }
}

/// SMCCC function ID decoder
#[derive(Debug, Clone, Copy)]
pub struct SmcccFunctionId {
    pub raw: u32,
}

impl SmcccFunctionId {
    /// Create from raw function ID
    pub const fn new(raw: u32) -> Self {
        Self { raw }
    }

    /// Get call type (fast/standard)
    pub fn call_type(&self) -> SmcccCallType {
        SmcccCallType::from_raw(self.raw)
    }

    /// Get calling convention (32/64-bit)
    pub fn call_conv(&self) -> SmcccCallConv {
        SmcccCallConv::from_raw(self.raw)
    }

    /// Get service type
    pub fn service(&self) -> SmcccService {
        SmcccService::from_raw(self.raw)
    }

    /// Get function number
    pub fn function_number(&self) -> u16 {
        (self.raw & 0xFFFF) as u16
    }

    /// Check if this is a PSCI call
    pub fn is_psci(&self) -> bool {
        // PSCI uses service range 0x00 (arch calls) with specific function IDs
        // PSCI v0.2 function IDs are 0x84000000-0x84000009
        (self.raw & 0xFF000000) == 0x84000000 || (self.raw & 0xFF000000) == 0xC4000000
    }

    /// Create SMCCC function ID
    pub fn build(call_type: SmcccCallType, call_conv: SmcccCallConv,
                 service: SmcccService, fn_num: u16) -> Self {
        let raw = call_type.raw() | call_conv.raw() |
                  ((service.raw() as u32) << 24) | (fn_num as u32);
        Self { raw }
    }

    /// Create fast call function ID
    pub fn fast_64(service: SmcccService, fn_num: u16) -> Self {
        Self::build(SmcccCallType::Fast, SmcccCallConv::Bit64, service, fn_num)
    }

    /// Create standard call function ID
    pub fn standard_32(service: SmcccService, fn_num: u16) -> Self {
        Self::build(SmcccCallType::Standard, SmcccCallConv::Bit32, service, fn_num)
    }
}

/// SMCCC call result
#[derive(Debug, Clone, Copy)]
pub struct SmcccResult {
    /// Return value (x0/r0)
    pub x0: u64,
    /// Additional return values (optional)
    pub x1: Option<u64>,
    pub x2: Option<u64>,
    pub x3: Option<u64>,
}

impl SmcccResult {
    /// Create result with only x0
    pub const fn new(x0: u64) -> Self {
        Self {
            x0,
            x1: None,
            x2: None,
            x3: None,
        }
    }

    /// Create result with x0 and x1
    pub const fn with_x1(x0: u64, x1: u64) -> Self {
        Self {
            x0,
            x1: Some(x1),
            x2: None,
            x3: None,
        }
    }

    /// Create result with x0, x1, x2
    pub const fn with_x2(x0: u64, x1: u64, x2: u64) -> Self {
        Self {
            x0,
            x1: Some(x1),
            x2: Some(x2),
            x3: None,
        }
    }

    /// Create result with x0, x1, x2, x3
    pub const fn with_x3(x0: u64, x1: u64, x2: u64, x3: u64) -> Self {
        Self {
            x0,
            x1: Some(x1),
            x2: Some(x2),
            x3: Some(x3),
        }
    }

    /// Create from PSCI return value
    pub fn from_psci(return_value: u64, _ret: PsciReturn) -> Self {
        Self::new(return_value)
    }

    /// Check if success (x0 == 0)
    pub fn is_success(&self) -> bool {
        self.x0 == 0
    }

    /// Get error code
    pub fn error_code(&self) -> Option<i64> {
        if self.x0 > 0x80000000 {
            Some(self.x0 as i64)
        } else {
            None
        }
    }
}

/// SMCCC register values for SMC call
#[derive(Debug, Clone)]
pub struct SmcccRegs {
    /// Function ID (x0)
    pub x0: u64,
    /// Argument 0 (x1)
    pub x1: u64,
    /// Argument 1 (x2)
    pub x2: u64,
    /// Argument 2 (x3)
    pub x3: u64,
    /// Argument 3 (x4)
    pub x4: u64,
    /// Argument 4 (x5)
    pub x5: u64,
    /// Argument 5 (x6)
    pub x6: u64,
    /// Client ID argument (x7)
    pub x7: u64,
}

impl Default for SmcccRegs {
    fn default() -> Self {
        Self {
            x0: 0,
            x1: 0,
            x2: 0,
            x3: 0,
            x4: 0,
            x5: 0,
            x6: 0,
            x7: 0,
        }
    }
}

impl SmcccRegs {
    /// Create new SMCCC registers
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with function ID
    pub fn with_function_id(function_id: u32) -> Self {
        Self {
            x0: function_id as u64,
            ..Self::default()
        }
    }

    /// Create with function ID and one argument
    pub fn with_args(function_id: u32, args: &[u64]) -> Self {
        let mut regs = Self::with_function_id(function_id);
        if args.len() > 0 {
            regs.x1 = args[0];
        }
        if args.len() > 1 {
            regs.x2 = args[1];
        }
        if args.len() > 2 {
            regs.x3 = args[2];
        }
        if args.len() > 3 {
            regs.x4 = args[3];
        }
        if args.len() > 4 {
            regs.x5 = args[4];
        }
        if args.len() > 5 {
            regs.x6 = args[5];
        }
        regs
    }

    /// Get function ID
    pub fn function_id(&self) -> u32 {
        self.x0 as u32
    }

    /// Get decoded function ID
    pub fn decoded_function_id(&self) -> SmcccFunctionId {
        SmcccFunctionId::new(self.function_id())
    }

    /// Get arguments as slice
    pub fn args(&self) -> &[u64] {
        // Count actual arguments (x1-x6, not including x7 which is client ID)
        let count = if self.x6 != 0 {
            6
        } else if self.x5 != 0 {
            5
        } else if self.x4 != 0 {
            4
        } else if self.x3 != 0 {
            3
        } else if self.x2 != 0 {
            2
        } else if self.x1 != 0 {
            1
        } else {
            0
        };

        unsafe {
            let ptr = &self.x1 as *const u64;
            std::slice::from_raw_parts(ptr, count)
        }
    }
}

/// SMCCC caller state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmcccCallerState {
    /// Calling from AArch64
    AArch64,
    /// Calling from AArch32
    AArch32,
}

/// SMCCC conduit method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SmcccConduit {
    /// SMC instruction
    Smc = 0,
    /// HVC instruction
    Hvc = 1,
}

/// SMCCC client ID
#[derive(Debug, Clone, Copy)]
pub struct SmcccClientId {
    pub raw: u32,
}

impl SmcccClientId {
    /// Create from raw value
    pub const fn new(raw: u32) -> Self {
        Self { raw }
    }

    /// Create client ID for current implementation
    pub const fn current() -> Self {
        // Bits [31:24] = implementation defined
        // Bits [23:16] = implementer defined
        // Bits [15:0] = architecture defined
        Self::new(0)
    }

    /// Get raw value
    pub const fn raw(&self) -> u32 {
        self.raw
    }
}

/// Execute SMC call (hypervisor-to-firmware)
///
/// # Safety
///
/// This function executes an SMC instruction which traps to EL3.
/// It should only be called with valid firmware-provided function IDs.
#[inline]
pub unsafe fn smc_call(regs: &SmcccRegs) -> SmcccResult {
    let x0: u64;
    let x1: u64;
    let x2: u64;
    let x3: u64;

    core::arch::asm!(
        "smc #0",
        inlateout("x0") regs.x0 as u64 => x0,
        inlateout("x1") regs.x1 as u64 => x1,
        inlateout("x2") regs.x2 as u64 => x2,
        inlateout("x3") regs.x3 as u64 => x3,
        in("x4") regs.x4 as u64,
        in("x5") regs.x5 as u64,
        in("x6") regs.x6 as u64,
        in("x7") regs.x7 as u64,
        clobber_abi("system")
    );

    // For simplicity, only return x0
    SmcccResult::new(x0)
}

/// Execute HVC call (hypervisor-to-hypervisor)
///
/// # Safety
///
/// This function executes an HVC instruction which traps to EL2.
#[inline]
pub unsafe fn hvc_call(regs: &SmcccRegs) -> SmcccResult {
    let x0: u64;
    let x1: u64;
    let x2: u64;
    let x3: u64;

    core::arch::asm!(
        "hvc #0",
        inlateout("x0") regs.x0 as u64 => x0,
        inlateout("x1") regs.x1 as u64 => x1,
        inlateout("x2") regs.x2 as u64 => x2,
        inlateout("x3") regs.x3 as u64 => x3,
        in("x4") regs.x4 as u64,
        in("x5") regs.x5 as u64,
        in("x6") regs.x6 as u64,
        in("x7") regs.x7 as u64,
        clobber_abi("system")
    );

    SmcccResult::new(x0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smccc_function_id() {
        let fid = SmcccFunctionId::new(0x84000003); // PSCI_CPU_ON
        assert!(fid.is_psci());
        assert_eq!(fid.function_number(), 3);
        assert_eq!(fid.call_type(), SmcccCallType::Standard);
        assert_eq!(fid.call_conv(), SmcccCallConv::Bit32);
    }

    #[test]
    fn test_smccc_call_type() {
        assert!(SmcccCallType::Standard.is_standard());
        assert!(SmcccCallType::Fast.is_fast());
        assert!(!SmcccCallType::Fast.is_standard());
    }

    #[test]
    fn test_smccc_call_conv() {
        assert!(SmcccCallConv::Bit32.is_64bit() == false);
        assert!(SmcccCallConv::Bit64.is_64bit());
    }

    #[test]
    fn test_smccc_result() {
        let result = SmcccResult::new(0);
        assert!(result.is_success());

        let result = SmcccResult::new(0xFFFFFFFF);
        assert!(!result.is_success());
        assert!(result.error_code().is_some());
    }

    #[test]
    fn test_smccc_regs() {
        let regs = SmcccRegs::with_args(0x84000000, &[1, 2, 3]);
        assert_eq!(regs.function_id(), 0x84000000);
        assert_eq!(regs.x1, 1);
        assert_eq!(regs.x2, 2);
        assert_eq!(regs.x3, 3);
    }

    #[test]
    fn test_smccc_build_function_id() {
        let fid = SmcccFunctionId::fast_64(SmcccService::Arch, 0x03);
        assert_eq!(fid.raw & 0xFF000000, 0x83000000);
        assert!(fid.call_type().is_fast());
        assert!(fid.call_conv().is_64bit());
    }
}
