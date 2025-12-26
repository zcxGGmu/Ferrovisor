//! CPU State Management for PSCI
//!
//! Provides CPU power state tracking for PSCI operations.
//! Reference: ARM DEN 0022D - Power State Coordination Interface
//!
//! CPU state includes:
//! - Online/offline status
//! - Suspend states
//! - Affinity level states

use super::{PsciReturn, PSCI_0_2_AFFINITY_LEVEL_ON, PSCI_0_2_AFFINITY_LEVEL_OFF,
              PSCI_0_2_AFFINITY_LEVEL_ON_PENDING};

/// CPU power state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CpuPowerState {
    /// CPU is ON and running
    On = PSCI_0_2_AFFINITY_LEVEL_ON,
    /// CPU is OFF
    Off = PSCI_0_2_AFFINITY_LEVEL_OFF,
    /// CPU is ON but pending (being powered on)
    OnPending = PSCI_0_2_AFFINITY_LEVEL_ON_PENDING,
}

impl CpuPowerState {
    /// Create from raw value
    pub fn from_raw(raw: u32) -> Self {
        match raw {
            PSCI_0_2_AFFINITY_LEVEL_ON => Self::On,
            PSCI_0_2_AFFINITY_LEVEL_OFF => Self::Off,
            PSCI_0_2_AFFINITY_LEVEL_ON_PENDING => Self::OnPending,
            _ => Self::Off,
        }
    }

    /// Get raw value
    pub fn raw(self) -> u32 {
        self as u32
    }

    /// Check if CPU is on
    pub fn is_on(self) -> bool {
        matches!(self, Self::On | Self::OnPending)
    }

    /// Check if CPU is off
    pub fn is_off(self) -> bool {
        matches!(self, Self::Off)
    }

    /// Check if CPU is pending
    pub fn is_pending(self) -> bool {
        matches!(self, Self::OnPending)
    }
}

/// CPU affinity level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AffinityLevel {
    /// Level 0 - Thread/core
    Level0 = 0,
    /// Level 1 - Cluster (e.g., group of cores)
    Level1 = 1,
    /// Level 2 - SOC/package
    Level2 = 2,
    /// Level 3 - System/multichip
    Level3 = 3,
}

impl AffinityLevel {
    /// Create from raw value
    pub fn from_raw(raw: u8) -> Self {
        match raw {
            0 => Self::Level0,
            1 => Self::Level1,
            2 => Self::Level2,
            3 => Self::Level3,
            _ => Self::Level0,
        }
    }

    /// Get raw value
    pub fn raw(self) -> u8 {
        self as u8
    }

    /// Check if this is a valid affinity level
    pub fn is_valid(self) -> bool {
        matches!(self, Self::Level0 | Self::Level1 | Self::Level2 | Self::Level3)
    }

    /// Get bit shift for MPIDR affinity field
    pub fn mpidr_shift(self) -> u32 {
        self.raw() * 8
    }
}

/// CPU MPIDR (Multiprocessor Affinity Register)
#[derive(Debug, Clone, Copy)]
pub struct CpuMpidr {
    pub raw: u64,
}

impl CpuMpidr {
    /// Create from raw MPIDR value
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    /// Create from affinity levels
    pub fn from_affinity(aff3: u8, aff2: u8, aff1: u8, aff0: u8) -> Self {
        let raw = ((aff3 as u64) << 32) |
                  ((aff2 as u64) << 16) |
                  ((aff1 as u64) << 8) |
                  (aff0 as u64);
        Self { raw }
    }

    /// Get affinity level 0 (core)
    pub fn aff0(&self) -> u8 {
        (self.raw & 0xFF) as u8
    }

    /// Get affinity level 1 (cluster)
    pub fn aff1(&self) -> u8 {
        ((self.raw >> 8) & 0xFF) as u8
    }

    /// Get affinity level 2 (SOC)
    pub fn aff2(&self) -> u8 {
        ((self.raw >> 16) & 0xFF) as u8
    }

    /// Get affinity level 3 (system)
    pub fn aff3(&self) -> u8 {
        ((self.raw >> 32) & 0xFF) as u8
    }

    /// Get affinity at specific level
    pub fn affinity(&self, level: AffinityLevel) -> u8 {
        match level {
            AffinityLevel::Level0 => self.aff0(),
            AffinityLevel::Level1 => self.aff1(),
            AffinityLevel::Level2 => self.aff2(),
            AffinityLevel::Level3 => self.aff3(),
        }
    }

    /// Check if MT (Multi-threading) bit is set
    pub fn is_multithreaded(&self) -> bool {
        (self.raw & (1 << 24)) != 0
    }

    /// Get U (Uniprocessor) bit
    pub fn is_uniprocessor(&self) -> bool {
        (self.raw & (1 << 30)) != 0
    }

    /// Check if this is a valid MPIDR
    pub fn is_valid(&self) -> bool {
        !self.is_uniprocessor()
    }
}

/// CPU state for power management
#[derive(Debug, Clone)]
pub struct CpuState {
    /// MPIDR value
    pub mpidr: CpuMpidr,
    /// Current power state
    pub power_state: CpuPowerState,
    /// CPU is online (OS considers it available)
    pub online: bool,
    /// Entry point address for CPU_ON
    pub entry_point: Option<u64>,
    /// Context ID for CPU_ON
    pub context_id: Option<u64>,
}

impl CpuState {
    /// Create new CPU state
    pub fn new(mpidr: u64) -> Self {
        Self {
            mpidr: CpuMpidr::new(mpidr),
            power_state: CpuPowerState::Off,
            online: false,
            entry_point: None,
            context_id: None,
        }
    }

    /// Check if CPU is on
    pub fn is_on(&self) -> bool {
        self.power_state.is_on()
    }

    /// Check if CPU is offline
    pub fn is_off(&self) -> bool {
        self.power_state.is_off()
    }

    /// Get power state
    pub fn power_state(&self) -> CpuPowerState {
        self.power_state
    }

    /// Set power state to ON
    pub fn set_power_on(&mut self) {
        self.power_state = CpuPowerState::On;
    }

    /// Set power state to OFF
    pub fn set_power_off(&mut self) {
        self.power_state = CpuPowerState::Off;
    }

    /// Set power state to ON_PENDING
    pub fn set_power_on_pending(&mut self) {
        self.power_state = CpuPowerState::OnPending;
    }

    /// Mark CPU as online
    pub fn set_online(&mut self) {
        self.online = true;
    }

    /// Mark CPU as offline
    pub fn set_offline(&mut self) {
        self.online = false;
    }

    /// Check if CPU is online
    pub fn is_online(&self) -> bool {
        self.online
    }

    /// Set entry point (for CPU_ON)
    pub fn set_entry_point(&mut self, entry: u64, context_id: u64) {
        self.entry_point = Some(entry);
        self.context_id = Some(context_id);
    }

    /// Get entry point
    pub fn entry_point(&self) -> Option<(u64, u64)> {
        self.entry_point.zip(self.context_id)
    }

    /// Clear entry point
    pub fn clear_entry_point(&mut self) {
        self.entry_point = None;
        self.context_id = None;
    }
}

/// CPU state manager for all CPUs
#[derive(Debug)]
pub struct CpuStateManager {
    /// CPU states indexed by MPIDR affinity
    cpus: alloc::collections::BTreeMap<u64, CpuState>,
    /// Current CPU count
    cpu_count: usize,
    /// Maximum CPU count
    max_cpus: usize,
}

impl Default for CpuStateManager {
    fn default() -> Self {
        Self {
            cpus: alloc::collections::BTreeMap::new(),
            cpu_count: 0,
            max_cpus: 256, // Reasonable default
        }
    }
}

impl CpuStateManager {
    /// Create new CPU state manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with maximum CPU count
    pub fn with_max_cpus(max: usize) -> Self {
        Self {
            cpus: alloc::collections::BTreeMap::new(),
            cpu_count: 0,
            max_cpus: max,
        }
    }

    /// Register a CPU
    pub fn register_cpu(&mut self, mpidr: u64) -> Result<(), &'static str> {
        if self.cpu_count >= self.max_cpus {
            return Err("Maximum CPU count reached");
        }

        let cpu = CpuState::new(mpidr);
        let key = CpuMpidr::new(mpidr).raw & 0xFF00FFFFFF;

        if self.cpus.contains_key(&key) {
            return Err("CPU already registered");
        }

        self.cpus.insert(key, cpu);
        self.cpu_count += 1;

        log::info!("CPU Manager: Registered CPU with MPIDR=0x{:016x}", mpidr);

        Ok(())
    }

    /// Get CPU state by MPIDR
    pub fn get_cpu(&self, mpidr: u64) -> Option<&CpuState> {
        let key = CpuMpidr::new(mpidr).raw & 0xFF00FFFFFF;
        self.cpus.get(&key)
    }

    /// Get mutable CPU state by MPIDR
    pub fn get_cpu_mut(&mut self, mpidr: u64) -> Option<&mut CpuState> {
        let key = CpuMpidr::new(mpidr).raw & 0xFF00FFFFFF;
        self.cpus.get_mut(&key)
    }

    /// Find CPU by target affinity (for CPU_ON)
    pub fn find_cpu(&self, target_mpidr: u64) -> Option<&CpuState> {
        let mask = 0xFF00FFFFFFu64; // Match without affinity 0
        let target = target_mpidr & mask;

        self.cpus.values()
            .find(|cpu| (cpu.mpidr.raw & mask) == target)
    }

    /// Find mutable CPU by target affinity
    pub fn find_cpu_mut(&mut self, target_mpidr: u64) -> Option<&mut CpuState> {
        let mask = 0xFF00FFFFFFu64;
        let target = target_mpidr & mask;

        self.cpus.values_mut()
            .find(|cpu| (cpu.mpidr.raw & mask) == target)
    }

    /// Get CPU count
    pub fn cpu_count(&self) -> usize {
        self.cpu_count
    }

    /// Get online CPU count
    pub fn online_cpu_count(&self) -> usize {
        self.cpus.values().filter(|cpu| cpu.is_online()).count()
    }

    /// Get affinity info for target affinity
    pub fn affinity_info(&self, target_mpidr: u64, lowest_level: u32) -> CpuPowerState {
        let mask = if lowest_level <= 3 {
            !((1u64 << (lowest_level * 8)) - 1)
        } else {
            0xFF00FFFFFFu64
        };

        let target = target_mpidr & mask;

        // Check if any CPU matching target affinity is online
        for cpu in self.cpus.values() {
            if (cpu.mpidr.raw & mask) == target && cpu.is_on() {
                return CpuPowerState::On;
            }
        }

        CpuPowerState::Off
    }

    /// Power on CPU
    pub fn cpu_on(&mut self, target_mpidr: u64, entry_point: u64,
                  context_id: u64) -> PsciReturn {
        // Find target CPU
        let cpu = if let Some(cpu) = self.find_cpu_mut(target_mpidr) {
            cpu
        } else {
            log::warn!("CPU Manager: CPU_ON - target CPU not found (MPIDR=0x{:016x})",
                      target_mpidr);
            return PsciReturn::InvalidParams;
        };

        // Check if already on
        if cpu.is_on() {
            log::warn!("CPU Manager: CPU_ON - CPU already on (MPIDR=0x{:016x})",
                      target_mpidr);
            return PsciReturn::AlreadyOn;
        }

        // Set entry point and mark as online
        cpu.set_entry_point(entry_point, context_id);
        cpu.set_power_on_pending();
        cpu.set_online();

        log::info!("CPU Manager: CPU_ON (MPIDR=0x{:016x}) entry=0x{:016x} context=0x{:016x}",
                  target_mpidr, entry_point, context_id);

        PsciReturn::Success
    }

    /// Power off CPU
    pub fn cpu_off(&mut self, mpidr: u64) -> PsciReturn {
        // Find CPU
        let cpu = if let Some(cpu) = self.get_cpu_mut(mpidr) {
            cpu
        } else {
            log::warn!("CPU Manager: CPU_OFF - CPU not found (MPIDR=0x{:016x})",
                      mpidr);
            return PsciReturn::InvalidParams;
        };

        // Check if already off
        if cpu.is_off() {
            log::warn!("CPU Manager: CPU_OFF - CPU already off (MPIDR=0x{:016x})",
                      mpidr);
            return PsciReturn::Denied;
        }

        // Mark as offline
        cpu.set_offline();
        cpu.set_power_off();

        log::info!("CPU Manager: CPU_OFF (MPIDR=0x{:016x})", mpidr);

        PsciReturn::Success
    }

    /// Suspend CPU
    pub fn cpu_suspend(&mut self, mpidr: u64, power_state: u32) -> PsciReturn {
        // Find CPU
        let cpu = if let Some(cpu) = self.get_cpu_mut(mpidr) {
            cpu
        } else {
            log::warn!("CPU Manager: CPU_SUSPEND - CPU not found (MPIDR=0x{:016x})",
                      mpidr);
            return PsciReturn::InvalidParams;
        };

        // For simplicity, treat suspend as WFI (just mark as pending)
        cpu.set_power_on_pending();

        log::debug!("CPU Manager: CPU_SUSPEND (MPIDR=0x{:016x}) state=0x{:08x}",
                     mpidr, power_state);

        PsciReturn::Success
    }

    /// Dump CPU state for debugging
    pub fn dump(&self) {
        log::info!("CPU State Manager:");
        log::info!("  Total CPUs: {}", self.cpu_count());
        log::info!("  Online CPUs: {}", self.online_cpu_count());

        for (i, (mpidr, cpu)) in self.cpus.iter().enumerate() {
            log::info!("  CPU {}: MPIDR=0x{:016x} State={:?} Online={}",
                       i, mpidr, cpu.power_state, cpu.online);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_power_state() {
        let state = CpuPowerState::On;
        assert!(state.is_on());
        assert!(!state.is_off());

        let state = CpuPowerState::Off;
        assert!(state.is_off());
        assert!(!state.is_on());
    }

    #[test]
    fn test_affinity_level() {
        let level = AffinityLevel::Level0;
        assert_eq!(level.raw(), 0);
        assert_eq!(level.mpidr_shift(), 0);

        let level = AffinityLevel::Level3;
        assert_eq!(level.raw(), 3);
        assert_eq!(level.mpidr_shift(), 24);
    }

    #[test]
    fn test_cpu_mpidr() {
        let mpidr = CpuMpidr::from_affinity(0, 0, 0, 1);
        assert_eq!(mpidr.aff0(), 1);
        assert_eq!(mpidr.aff1(), 0);
        assert_eq!(mpidr.aff2(), 0);
        assert_eq!(mpidr.aff3(), 0);

        let mpidr = CpuMpidr::new(0x80000010);
        assert_eq!(mpidr.aff0(), 0x10);
        assert!(mpidr.is_multithreaded());
    }

    #[test]
    fn test_cpu_state() {
        let mut cpu = CpuState::new(0x80000001);
        assert!(!cpu.is_on());

        cpu.set_power_on();
        cpu.set_online();
        assert!(cpu.is_on());
        assert!(cpu.is_online());

        cpu.set_entry_point(0x40000000, 0x12345678);
        assert_eq!(cpu.entry_point(), Some((0x40000000, 0x12345678)));
    }

    #[test]
    fn test_cpu_state_manager() {
        let mut manager = CpuStateManager::with_max_cpus(4);

        assert_eq!(manager.register_cpu(0x80000000), Ok(()));
        assert_eq!(manager.register_cpu(0x80000001), Ok(()));
        assert_eq!(manager.cpu_count(), 2);

        let cpu = manager.get_cpu(0x80000000);
        assert!(cpu.is_some());
        assert!(cpu.unwrap().is_off());

        let ret = manager.cpu_on(0x80000001, 0x40000000, 0);
        assert_eq!(ret, PsciReturn::Success);

        let cpu = manager.get_cpu(0x80000001);
        assert!(cpu.unwrap().is_on());
    }

    #[test]
    fn test_cpu_on_already_on() {
        let mut manager = CpuStateManager::new();
        assert_eq!(manager.register_cpu(0x80000000), Ok(()));

        // First CPU_ON
        let ret = manager.cpu_on(0x80000000, 0x40000000, 0);
        assert_eq!(ret, PsciReturn::Success);

        // Second CPU_ON - should fail
        let ret = manager.cpu_on(0x80000000, 0x40000000, 0);
        assert_eq!(ret, PsciReturn::AlreadyOn);
    }

    #[test]
    fn test_cpu_off() {
        let mut manager = CpuStateManager::new();
        assert_eq!(manager.register_cpu(0x80000000), Ok(()));

        // Power on first
        let ret = manager.cpu_on(0x80000000, 0x40000000, 0);
        assert_eq!(ret, PsciReturn::Success);

        // Power off
        let ret = manager.cpu_off(0x80000000);
        assert_eq!(ret, PsciReturn::Success);

        let cpu = manager.get_cpu(0x80000000);
        assert!(cpu.unwrap().is_off());
    }

    #[test]
    fn test_affinity_info() {
        let mut manager = CpuStateManager::new();
        assert_eq!(manager.register_cpu(0x80000000), Ok(()));
        assert_eq!(manager.register_cpu(0x80000001), Ok(()));

        // Power on CPU 1
        let _ = manager.cpu_on(0x80000001, 0x40000000, 0);

        // Affinity info - should be ON since CPU 1 is on
        let state = manager.affinity_info(0x80000000, 0);
        assert_eq!(state, CpuPowerState::On);
    }
}
