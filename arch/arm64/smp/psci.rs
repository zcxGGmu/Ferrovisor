//! PSCI-based SMP initialization for ARM64
//!
//! Provides CPU initialization using PSCI (Power State Coordination Interface).
//!
//! ## PSCI SMP Overview
//!
//! PSCI is a standard interface between OS and firmware for:
//! - CPU power management (CPU_ON, CPU_OFF, CPU_SUSPEND)
//! - System power management (SYSTEM_OFF, SYSTEM_RESET)
//!
//! For SMP, PSCI_CPU_ON is used to start secondary CPUs.
//!
//! ## PSCI Calling Convention
//!
//! PSCI uses SMC (Secure Monitor Call) or HVC (Hypervisor Call):
//! - SMC: Calls to EL3 firmware (secure world)
//! - HVC: Calls to EL2 hypervisor
//!
//! ## References
//! - ARM DEN 0022D (PSCI specification)
//! - [Xvisor PSCI Implementation](https://github.com/xvisor/xvisor)

use super::{SmpOps, CpuState, MAX_CPUS};
use crate::arch::arm64::psci as psci_module;

/// Secondary CPU entry point placeholder
///
/// In a real implementation, this would be set to the actual secondary
/// startup code physical address.
static mut SECONDARY_ENTRY_POINT: u64 = 0;

/// PSCI SMP operations
pub struct PsciSmpOps {
    /// PSCI version
    version: (u32, u32),
    /// Use HVC instead of SMC
    use_hvc: bool,
}

impl Default for PsciSmpOps {
    fn default() -> Self {
        Self {
            version: (0, 2),
            use_hvc: false,
        }
    }
}

impl PsciSmpOps {
    /// Create new PSCI SMP operations
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with specific version
    pub fn with_version(major: u32, minor: u32) -> Self {
        Self {
            version: (major, minor),
            ..Self::default()
        }
    }

    /// Create with HVC conduit
    pub fn with_hvc(use_hvc: bool) -> Self {
        Self {
            use_hvc,
            ..Self::default()
        }
    }

    /// Get PSCI version
    pub fn version(&self) -> (u32, u32) {
        self.version
    }

    /// Check if PSCI is available
    pub fn is_available(&self) -> bool {
        psci_module::is_available()
    }

    /// Check if using HVC conduit
    pub fn uses_hvc(&self) -> bool {
        self.use_hvc
    }

    /// Call PSCI CPU_ON
    ///
    /// # Parameters
    /// - `target_mpidr`: MPIDR of target CPU
    /// - `entry_point`: Physical address where CPU should start
    /// - `context_id`: Value passed in x0/x1 registers
    fn psci_cpu_on(&self, target_mpidr: u64, entry_point: u64,
                   context_id: u64) -> Result<(), &'static str> {
        log::debug!("PSCI SMP: CPU_ON MPIDR=0x{:016x} entry={:#x} context={:#x}",
                    target_mpidr, entry_point, context_id);

        // Use the PSCI module's handle_smc function
        let fn_id = if self.use_hvc {
            psci_module::PSCI_0_2_FN64_CPU_ON
        } else {
            psci_module::PSCI_0_2_FN64_CPU_ON
        };

        let args = [target_mpidr, entry_point, context_id];
        let (ret_val, ret) = psci_module::handle_smc(fn_id, &args);

        match ret {
            psci_module::PsciReturn::Success => {
                log::info!("PSCI SMP: CPU_ON success for MPIDR=0x{:016x}", target_mpidr);
                Ok(())
            }
            psci_module::PsciReturn::AlreadyOn => {
                log::warn!("PSCI SMP: CPU already on (MPIDR=0x{:016x})", target_mpidr);
                Err("CPU already on")
            }
            _ => {
                log::error!("PSCI SMP: CPU_ON failed (ret={:?})", ret);
                Err(ret.as_str())
            }
        }
    }

    /// Call PSCI CPU_OFF
    fn psci_cpu_off(&self) -> Result<(), &'static str> {
        log::debug!("PSCI SMP: CPU_OFF");

        let fn_id = if self.use_hvc {
            psci_module::PSCI_0_2_FN_CPU_OFF
        } else {
            psci_module::PSCI_0_2_FN_CPU_OFF
        };

        let (_, ret) = psci_module::handle_smc(fn_id, &[]);

        match ret {
            psci_module::PsciReturn::Success => Ok(()),
            _ => Err(ret.as_str()),
        }
    }

    /// Query PSCI version
    fn psci_version(&self) -> (u32, u32) {
        let fn_id = psci_module::PSCI_0_2_FN_PSCI_VERSION;
        let (version, _) = psci_module::handle_smc(fn_id, &[]);

        let major = (version >> 16) & 0xFFFF;
        let minor = version & 0xFFFF;

        (major as u32, minor as u32)
    }

    /// Query CPU affinity info
    fn psci_affinity_info(&self, target_mpidr: u64,
                          lowest_level: u32) -> psci_module::CpuPowerState {
        let fn_id = if self.use_hvc {
            psci_module::PSCI_0_2_FN64_AFFINITY_INFO
        } else {
            psci_module::PSCI_0_2_FN_AFFINITY_INFO
        };

        let args = [target_mpidr, lowest_level as u64];
        let (_, ret) = psci_module::handle_smc(fn_id, &args);

        // Convert return to power state
        match ret {
            psci_module::PsciReturn::Success => psci_module::CpuPowerState::On,
            psci_module::PsciReturn::NotPresent => psci_module::CpuPowerState::Off,
            _ => psci_module::CpuPowerState::Off,
        }
    }
}

impl SmpOps for PsciSmpOps {
    fn name(&self) -> &str {
        "psci"
    }

    fn ops_init(&mut self) -> Result<(), &'static str> {
        log::info!("PSCI SMP: Initializing");

        // Check PSCI availability
        if !self.is_available() {
            return Err("PSCI not available");
        }

        // Query PSCI version
        self.version = self.psci_version();
        log::info!("PSCI SMP: PSCI version {}.{}", self.version.0, self.version.1);

        Ok(())
    }

    fn cpu_init(&mut self, _logical_id: u32, mpidr: u64) -> Result<(), &'static str> {
        log::debug!("PSCI SMP: CPU init MPIDR=0x{:016x}", mpidr);

        // Query CPU state
        let state = self.psci_affinity_info(mpidr, 0);
        log::debug!("PSCI SMP: CPU state: {:?}", state);

        Ok(())
    }

    fn cpu_prepare(&mut self, logical_id: u32) -> Result<bool, &'static str> {
        log::debug!("PSCI SMP: CPU prepare {}", logical_id);

        // Check if we have a valid entry point
        let entry = unsafe { SECONDARY_ENTRY_POINT };
        if entry == 0 {
            return Ok(false); // Cannot boot without entry point
        }

        Ok(true)
    }

    fn cpu_boot(&mut self, logical_id: u32, entry_point: u64,
                context_id: u64) -> Result<(), &'static str> {
        // Get target MPIDR from SMP manager
        let mpidr = if let Some(mgr) = super::manager() {
            mgr.cpu_info(logical_id)
                .map(|cpu| cpu.mpidr)
                .ok_or("CPU not found")?
        } else {
            return Err("SMP manager not initialized");
        };

        log::info!("PSCI SMP: Booting CPU {} (MPIDR=0x{:016x})",
                   logical_id, mpidr);

        // Store entry point for potential use
        unsafe {
            SECONDARY_ENTRY_POINT = entry_point;
        }

        // Call PSCI CPU_ON
        self.psci_cpu_on(mpidr, entry_point, context_id)?;

        Ok(())
    }

    fn cpu_postboot(&mut self, logical_id: u32) -> Result<(), &'static str> {
        log::info!("PSCI SMP: CPU {} post-boot", logical_id);

        // Mark CPU as online in SMP manager
        if let Some(mgr) = super::manager_mut() {
            mgr.mark_cpu_online(logical_id)?;
        }

        Ok(())
    }
}

/// Global PSCI SMP operations instance
static mut PSCI_OPS: Option<PsciSmpOps> = None;

/// Initialize PSCI SMP
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing PSCI SMP");

    // Create and initialize PSCI ops
    let mut ops = PsciSmpOps::new();
    ops.ops_init()?;

    unsafe {
        PSCI_OPS = Some(ops);
    }

    log::info!("PSCI SMP initialized");
    Ok(())
}

/// Get PSCI SMP operations
pub fn ops() -> Option<&'static PsciSmpOps> {
    unsafe { PSCI_OPS.as_ref() }
}

/// Get mutable PSCI SMP operations
pub fn ops_mut() -> Option<&'static mut PsciSmpOps> {
    unsafe { PSCI_OPS.as_mut() }
}

/// Boot a CPU using PSCI
///
/// This is a convenience function that can be called directly
/// without going through the SmpOps trait.
pub fn cpu_on(logical_id: u32, entry_addr: u64, context_id: u64) -> Result<(), &'static str> {
    if let Some(ops) = ops_mut() {
        ops.cpu_boot(logical_id, entry_addr, context_id)
    } else {
        Err("PSCI ops not initialized")
    }
}

/// Query CPU status using PSCI
pub fn cpu_status(mpidr: u64) -> CpuState {
    if let Some(ops) = ops() {
        let power_state = ops.psci_affinity_info(mpidr, 0);
        match power_state {
            psci_module::CpuPowerState::On => CpuState::Online,
            psci_module::CpuPowerState::Off => CpuState::Offline,
            _ => CpuState::Offline,
        }
    } else {
        CpuState::Offline
    }
}

/// Set secondary entry point
///
/// This is typically called during SMP initialization to set
/// the common entry point for all secondary CPUs.
pub fn set_secondary_entry_point(entry: u64) {
    unsafe {
        SECONDARY_ENTRY_POINT = entry;
    }
    log::info!("PSCI SMP: Secondary entry point set to {:#x}", entry);
}

/// Get secondary entry point
pub fn secondary_entry_point() -> u64 {
    unsafe { SECONDARY_ENTRY_POINT }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_psci_smp_ops_new() {
        let ops = PsciSmpOps::new();
        assert_eq!(ops.name(), "psci");
        assert!(!ops.uses_hvc());
        assert_eq!(ops.version(), (0, 2));
    }

    #[test]
    fn test_psci_smp_ops_with_version() {
        let ops = PsciSmpOps::with_version(1, 0);
        assert_eq!(ops.version(), (1, 0));
    }

    #[test]
    fn test_psci_smp_ops_with_hvc() {
        let ops = PsciSmpOps::with_hvc(true);
        assert!(ops.uses_hvc());
    }

    #[test]
    fn test_secondary_entry_point() {
        assert_eq!(secondary_entry_point(), 0);

        set_secondary_entry_point(0x40000000);
        assert_eq!(secondary_entry_point(), 0x40000000);

        set_secondary_entry_point(0);
        assert_eq!(secondary_entry_point(), 0);
    }

    #[test]
    fn test_cpu_status() {
        let mpidr = 0x80000001u64;
        let state = cpu_status(mpidr);
        // Should return Offline since PSCI not initialized in test
        assert!(matches!(state, CpuState::Offline));
    }
}
