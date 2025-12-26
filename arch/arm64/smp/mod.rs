//! Symmetric Multiprocessing (SMP) for ARM64
//!
//! Provides SMP initialization and CPU management for ARM64 systems.
//!
//! ## SMP Overview
//!
//! SMP enables multiple CPUs to work together:
//! - Boot CPU (primary) starts first and initializes the system
//! - Secondary CPUs are started using enable-method (PSCI, spin-table, etc.)
//! - Each CPU has a unique MPIDR (Multiprocessor Affinity Register)
//!
//! ## CPU Enable Methods
//!
//! ARM64 supports several CPU enable methods:
//! - **PSCI**: Power State Coordination Interface (SMC/HVC calls)
//! - **Spin Table**: Device tree defined memory-mapped spin table
//! - **SCU**: Snoop Control Unit (ARMv7 only)
//!
//! ## References
//! - [ARM SMP Reference](https://developer.arm.com/documentation)
//! - [Xvisor SMP Implementation](https://github.com/xvisor/xvisor)

pub mod psci;
pub mod spin_table;

pub mod init;

// Re-export key types
pub use psci::*;
pub use spin_table::*;
pub use init::*;

/// Maximum number of CPUs supported
pub const MAX_CPUS: usize = 8;

/// Invalid MPIDR value
pub const MPIDR_INVALID: u64 = 0xFFFFFFFF;

/// CPU state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CpuState {
    /// CPU is offline (powered off)
    Offline = 0,
    /// CPU is booting
    Booting = 1,
    /// CPU is online (running)
    Online = 2,
    /// CPU is suspending
    Suspending = 3,
    /// CPU is suspended
    Suspended = 4,
}

impl CpuState {
    /// Create from raw value
    pub fn from_raw(raw: u8) -> Self {
        match raw {
            0 => Self::Offline,
            1 => Self::Booting,
            2 => Self::Online,
            3 => Self::Suspending,
            4 => Self::Suspended,
            _ => Self::Offline,
        }
    }

    /// Get raw value
    pub fn raw(&self) -> u8 {
        *self as u8
    }

    /// Check if CPU is online
    pub fn is_online(&self) -> bool {
        matches!(self, Self::Online)
    }

    /// Check if CPU is offline
    pub fn is_offline(&self) -> bool {
        matches!(self, Self::Offline)
    }

    /// Check if CPU is transitioning
    pub fn is_transitioning(&self) -> bool {
        matches!(self, Self::Booting | Self::Suspending)
    }
}

/// CPU information
#[derive(Debug, Clone)]
pub struct CpuInfo {
    /// Logical CPU ID (0-based)
    pub logical_id: u32,
    /// MPIDR value (hardware CPU ID)
    pub mpidr: u64,
    /// Current CPU state
    pub state: CpuState,
    /// Enable method name ("psci", "spin-table", etc.)
    pub enable_method: alloc::string::String,
    /// Entry point address (for boot)
    pub entry_point: Option<u64>,
    /// Context ID (passed to entry point)
    pub context_id: Option<u64>,
}

impl CpuInfo {
    /// Create new CPU info
    pub fn new(logical_id: u32, mpidr: u64) -> Self {
        Self {
            logical_id,
            mpidr,
            state: CpuState::Offline,
            enable_method: alloc::string::String::new(),
            entry_point: None,
            context_id: None,
        }
    }

    /// Set enable method
    pub fn set_enable_method(&mut self, method: &str) {
        self.enable_method = alloc::string::String::from(method);
    }

    /// Set entry point
    pub fn set_entry_point(&mut self, entry: u64, context: u64) {
        self.entry_point = Some(entry);
        self.context_id = Some(context);
    }

    /// Check if CPU has valid entry point
    pub fn has_entry_point(&self) -> bool {
        self.entry_point.is_some()
    }
}

/// SMP operations trait
///
/// This trait defines the interface for different CPU enable methods.
pub trait SmpOps {
    /// Get the name of this enable method
    fn name(&self) -> &str;

    /// Initialize the SMP operations
    fn ops_init(&mut self) -> Result<(), &'static str>;

    /// Initialize a specific CPU
    ///
    /// # Parameters
    /// - `logical_id`: Logical CPU ID (0-based)
    /// - `mpidr`: Hardware MPIDR value
    fn cpu_init(&mut self, logical_id: u32, mpidr: u64) -> Result<(), &'static str>;

    /// Prepare a CPU for booting
    ///
    /// This is called once before the first boot attempt.
    fn cpu_prepare(&mut self, logical_id: u32) -> Result<bool, &'static str> {
        let _ = logical_id;
        Ok(true) // Default: CPU can be booted
    }

    /// Boot a CPU
    ///
    /// # Parameters
    /// - `logical_id`: Logical CPU ID
    /// - `entry_point`: Physical address where CPU should start execution
    /// - `context_id`: Value to pass to the CPU (in x0 register)
    fn cpu_boot(&mut self, logical_id: u32, entry_point: u64, context_id: u64)
        -> Result<(), &'static str>;

    /// Post-boot cleanup
    ///
    /// Called from the CPU being booted, after it starts executing.
    fn cpu_postboot(&mut self, logical_id: u32) -> Result<(), &'static str> {
        let _ = logical_id;
        Ok(()) // Default: no post-boot work needed
    }
}

/// SMP manager
pub struct SmpManager {
    /// CPU information array
    cpus: alloc::vec::Vec<CpuInfo>,
    /// Number of online CPUs
    online_count: usize,
    /// Boot CPU (logical ID)
    boot_cpu: u32,
    /// Currently selected enable method
    enable_method: Option<&'static dyn SmpOps>,
}

impl Default for SmpManager {
    fn default() -> Self {
        Self {
            cpus: alloc::vec::Vec::new(),
            online_count: 0,
            boot_cpu: 0,
            enable_method: None,
        }
    }
}

impl SmpManager {
    /// Create new SMP manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize SMP manager
    pub fn init(&mut self) -> Result<(), &'static str> {
        log::info!("SMP Manager: Initializing");

        // Clear existing state
        self.cpus.clear();
        self.online_count = 0;
        self.boot_cpu = 0;

        // Register boot CPU (CPU 0)
        let mpidr = self.read_mpidr();
        let boot_cpu = CpuInfo::new(0, mpidr);
        boot_cpu.state = CpuState::Online;
        self.cpus.push(boot_cpu);
        self.online_count = 1;

        log::info!("SMP Manager: Boot CPU MPIDR=0x{:016x}", mpidr);

        Ok(())
    }

    /// Read current CPU's MPIDR
    fn read_mpidr(&self) -> u64 {
        let mpidr: u64;
        unsafe {
            core::arch::asm!("mrs {}, mpidr_el1", out(reg) mpidr);
        }
        // Mask out reserved bits
        mpidr & 0xFF00FFFFFF
    }

    /// Register a CPU
    pub fn register_cpu(&mut self, logical_id: u32, mpidr: u64) -> Result<(), &'static str> {
        if logical_id as usize >= MAX_CPUS {
            return Err("CPU ID exceeds maximum");
        }

        if self.cpus.iter().any(|c| c.logical_id == logical_id) {
            return Err("CPU already registered");
        }

        let cpu = CpuInfo::new(logical_id, mpidr);
        self.cpus.push(cpu);

        log::info!("SMP Manager: Registered CPU {} MPIDR=0x{:016x}",
                   logical_id, mpidr);

        Ok(())
    }

    /// Set enable method for a CPU
    pub fn set_enable_method(&mut self, logical_id: u32, method: &str) -> Result<(), &'static str> {
        let cpu = self.cpus.iter_mut()
            .find(|c| c.logical_id == logical_id)
            .ok_or("CPU not found")?;

        cpu.set_enable_method(method);

        log::debug!("SMP Manager: CPU {} enable-method={}", logical_id, method);

        Ok(())
    }

    /// Get CPU info
    pub fn cpu_info(&self, logical_id: u32) -> Option<&CpuInfo> {
        self.cpus.iter().find(|c| c.logical_id == logical_id)
    }

    /// Get mutable CPU info
    pub fn cpu_info_mut(&mut self, logical_id: u32) -> Option<&mut CpuInfo> {
        self.cpus.iter_mut().find(|c| c.logical_id == logical_id)
    }

    /// Find CPU by MPIDR
    pub fn find_cpu_by_mpidr(&self, mpidr: u64) -> Option<&CpuInfo> {
        self.cpus.iter().find(|c| c.mpidr == mpidr)
    }

    /// Get number of registered CPUs
    pub fn cpu_count(&self) -> usize {
        self.cpus.len()
    }

    /// Get number of online CPUs
    pub fn online_count(&self) -> usize {
        self.online_count
    }

    /// Check if CPU is online
    pub fn is_cpu_online(&self, logical_id: u32) -> bool {
        self.cpu_info(logical_id)
            .map(|c| c.state.is_online())
            .unwrap_or(false)
    }

    /// Boot a CPU
    pub fn cpu_boot(&mut self, ops: &mut dyn SmpOps, logical_id: u32,
                    entry_point: u64, context_id: u64) -> Result<(), &'static str> {
        let cpu = self.cpu_info_mut(logical_id)
            .ok_or("CPU not found")?;

        if cpu.state.is_online() {
            return Err("CPU already online");
        }

        log::info!("SMP Manager: Booting CPU {} (entry={:#x}, context={:#x})",
                   logical_id, entry_point, context_id);

        // Prepare CPU
        if !ops.cpu_prepare(logical_id)? {
            return Err("CPU prepare failed");
        }

        // Set entry point
        cpu.set_entry_point(entry_point, context_id);

        // Boot CPU
        ops.cpu_boot(logical_id, entry_point, context_id)?;

        // Update state
        cpu.state = CpuState::Booting;

        Ok(())
    }

    /// Mark CPU as online
    pub fn mark_cpu_online(&mut self, logical_id: u32) -> Result<(), &'static str> {
        let cpu = self.cpu_info_mut(logical_id)
            .ok_or("CPU not found")?;

        cpu.state = CpuState::Online;
        self.online_count += 1;

        log::info!("SMP Manager: CPU {} is now online ({}/{})",
                   logical_id, self.online_count, self.cpu_count());

        Ok(())
    }

    /// Get boot CPU ID
    pub fn boot_cpu(&self) -> u32 {
        self.boot_cpu
    }

    /// Check if current CPU is boot CPU
    pub fn is_boot_cpu(&self) -> bool {
        let current_mpidr = self.read_mpidr();
        self.cpus.get(0)
            .map(|boot| boot.mpidr == current_mpidr)
            .unwrap_or(true)
    }

    /// Dump CPU states for debugging
    pub fn dump(&self) {
        log::info!("SMP Manager State:");
        log::info!("  Total CPUs: {}", self.cpu_count());
        log::info!("  Online CPUs: {}", self.online_count());
        log::info!("  Boot CPU: {}", self.boot_cpu);

        for cpu in &self.cpus {
            log::info!("  CPU {}: MPIDR=0x{:016x} State={:?} Method={}",
                       cpu.logical_id, cpu.mpidr, cpu.state, cpu.enable_method);
        }
    }
}

/// Global SMP manager instance
static mut SMP_MANAGER: Option<SmpManager> = None;

/// Initialize SMP subsystem
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing ARM64 SMP");

    unsafe {
        SMP_MANAGER = Some(SmpManager::new());
        SMP_MANAGER.as_mut().unwrap().init()?;
    }

    // Initialize sub-modules
    psci::init()?;
    spin_table::init()?;

    log::info!("ARM64 SMP initialized");

    Ok(())
}

/// Get global SMP manager
pub fn manager() -> Option<&'static SmpManager> {
    unsafe { SMP_MANAGER.as_ref() }
}

/// Get mutable global SMP manager
pub fn manager_mut() -> Option<&'static mut SmpManager> {
    unsafe { SMP_MANAGER.as_mut() }
}

/// Get current CPU's logical ID
pub fn current_cpu_id() -> u32 {
    if let Some(mgr) = manager() {
        let mpidr: u64;
        unsafe {
            core::arch::asm!("mrs {}, mpidr_el1", out(reg) mpidr);
        }
        let mpidr = mpidr & 0xFF00FFFFFF;

        for cpu in &mgr.cpus {
            if cpu.mpidr == mpidr {
                return cpu.logical_id;
            }
        }
    }

    // Default to boot CPU
    0
}

/// Check if we're in SMP mode
pub fn is_smp() -> bool {
    manager()
        .map(|mgr| mgr.cpu_count() > 1)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_state() {
        let state = CpuState::Offline;
        assert!(state.is_offline());
        assert!(!state.is_online());

        let state = CpuState::Online;
        assert!(state.is_online());
        assert!(!state.is_offline());
    }

    #[test]
    fn test_cpu_info() {
        let mut cpu = CpuInfo::new(1, 0x80000001);
        assert_eq!(cpu.logical_id, 1);
        assert_eq!(cpu.mpidr, 0x80000001);

        cpu.set_enable_method("psci");
        assert_eq!(cpu.enable_method, "psci");

        cpu.set_entry_point(0x40000000, 0x1234);
        assert!(cpu.has_entry_point());
    }

    #[test]
    fn test_smp_manager() {
        let mut mgr = SmpManager::new();
        mgr.init().unwrap();

        assert_eq!(mgr.cpu_count(), 1);
        assert_eq!(mgr.online_count(), 1);
        assert!(mgr.is_boot_cpu());

        mgr.register_cpu(1, 0x80000001).unwrap();
        assert_eq!(mgr.cpu_count(), 2);

        mgr.set_enable_method(1, "psci").unwrap();
        let cpu = mgr.cpu_info(1).unwrap();
        assert_eq!(cpu.enable_method, "psci");
    }

    #[test]
    fn test_find_cpu_by_mpidr() {
        let mut mgr = SmpManager::new();
        mgr.init().unwrap();

        mgr.register_cpu(1, 0x80000001).unwrap();
        mgr.register_cpu(2, 0x80000002).unwrap();

        let cpu = mgr.find_cpu_by_mpidr(0x80000002);
        assert!(cpu.is_some());
        assert_eq!(cpu.unwrap().logical_id, 2);
    }
}
