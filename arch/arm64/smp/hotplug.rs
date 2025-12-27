//! CPU Hotplug support for ARM64
//!
//! Provides CPU hotplug functionality for dynamically adding/removing CPUs:
//! - CPU online/offline operations
//! - CPU state management
//! - CPU notification mechanisms
//!
//! ## CPU Hotplug Overview
//!
//! CPU hotplug allows dynamic addition and removal of CPUs:
//! - **CPU Online**: Transition CPU from offline to running state
//! - **CPU Offline**: Transition CPU from running to offline state
//! - Uses PSCI CPU_ON/CPU_OFF for power management
//!
//! ## Hotplug Flow
//!
//! ### CPU Online (Hot-add)
//! 1. Check if CPU is available (in device tree)
//! 2. Check if CPU supports hotplug
//! 3. Call PSCI CPU_ON with entry point
//! 4. Wait for CPU to come online
//! 5. Notify subsystems of new CPU
//!
//! ### CPU Offline (Hot-remove)
//! 1. Check if CPU can go offline (no work, no IRQs)
//! 2. Migrate interrupts away from CPU
//! 3. Stop work on target CPU
//! 4. Call PSCI CPU_OFF
//! 5. Wait for CPU to go offline
//! 6. Notify subsystems of CPU removal
//!
//! ## References
//! - [ARM PSCI Specification](https://developer.arm.com/documentation/den0022/latest/)
//! - [Xvisor CPU Hotplug](https://github.com/xvisor/xvisor)

use super::{CpuState, MAX_CPUS};
use crate::arch::arm64::smp::psci::PsciSmpOps;

/// CPU hotplug state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotplugState {
    /// CPU is not hotplug capable
    NotCapable = 0,
    /// CPU is offline and can be onlined
    Offline = 1,
    /// CPU is coming online (in progress)
    Onlining = 2,
    /// CPU is online
    Online = 3,
    /// CPU is going offline (in progress)
    Offlining = 4,
    /// CPU offline failed
    OfflineFailed = 5,
}

/// CPU hotplug event
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotplugEvent {
    /// CPU came online
    CpuOnline(u32),
    /// CPU went offline
    CpuOffline(u32),
    /// CPU online failed
    OnlineFailed(u32),
    /// CPU offline failed
    OfflineFailed(u32),
}

/// CPU hotplug notification callback
pub trait HotplugCallback {
    /// Called when CPU comes online
    fn cpu_online(&mut self, cpu_id: u32);
    /// Called when CPU goes offline
    fn cpu_offline(&mut self, cpu_id: u32);
    /// Called when CPU online fails
    fn cpu_online_failed(&mut self, cpu_id: u32);
    /// Called when CPU offline fails
    fn cpu_offline_failed(&mut self, cpu_id: u32);
}

/// CPU hotplug manager
pub struct HotplugManager {
    /// CPU states
    cpu_states: [HotplugState; MAX_CPUS],
    /// PSCI operations
    psci_ops: PsciSmpOps,
    /// Hotplug callbacks
    callbacks: Vec<&'static mut dyn HotplugCallback>,
    /// Online CPU mask
    online_mask: u64,
    /// Available CPU mask
    available_mask: u64,
}

impl Default for HotplugManager {
    fn default() -> Self {
        Self {
            cpu_states: [HotplugState::NotCapable; MAX_CPUS],
            psci_ops: PsciSmpOps::new(),
            callbacks: Vec::new(),
            online_mask: 0,  // Initially no CPUs online
            available_mask: 0, // No CPUs available
        }
    }
}

impl HotplugManager {
    /// Create new hotplug manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize hotplug manager
    ///
    /// Sets up CPU availability based on system configuration.
    pub fn init(&mut self, num_cpus: usize) -> Result<(), &'static str> {
        log::info!("Hotplug: Initializing with {} CPUs", num_cpus);

        if num_cpus > MAX_CPUS {
            return Err("Too many CPUs");
        }

        // Mark CPUs as available
        for i in 0..num_cpus {
            self.cpu_states[i] = HotplugState::Offline;
            self.available_mask |= 1 << i;
        }

        // Boot CPU is already online
        self.cpu_states[0] = HotplugState::Online;
        self.online_mask |= 1;

        log::info!("Hotplug: Initialized {} CPUs (CPU 0 online)", num_cpus);
        Ok(())
    }

    /// Get CPU hotplug state
    pub fn cpu_state(&self, cpu_id: u32) -> HotplugState {
        if (cpu_id as usize) < MAX_CPUS {
            self.cpu_states[cpu_id as usize]
        } else {
            HotplugState::NotCapable
        }
    }

    /// Check if CPU is online
    pub fn is_online(&self, cpu_id: u32) -> bool {
        (self.online_mask & (1 << cpu_id)) != 0
    }

    /// Check if CPU is available for hotplug
    pub fn is_available(&self, cpu_id: u32) -> bool {
        (self.available_mask & (1 << cpu_id)) != 0
    }

    /// Get number of online CPUs
    pub fn online_count(&self) -> u32 {
        self.online_mask.count_ones() as u32
    }

    /// Get list of online CPU IDs
    pub fn online_cpus(&self) -> Vec<u32> {
        let mut cpus = Vec::new();
        for i in 0..MAX_CPUS {
            if self.is_online(i as u32) {
                cpus.push(i as u32);
            }
        }
        cpus
    }

    /// Add hotplug callback
    pub fn add_callback(&mut self, callback: &'static mut dyn HotplugCallback) {
        self.callbacks.push(callback);
    }

    /// Bring CPU online
    ///
    /// # Parameters
    /// - `cpu_id`: CPU to bring online
    /// - `entry_point`: Physical address of CPU entry point
    /// - `mpidr`: MPIDR of target CPU
    ///
    /// Returns true if CPU was successfully brought online
    pub fn cpu_online(&mut self, cpu_id: u32, entry_point: u64, mpidr: u64)
        -> Result<bool, &'static str> {
        log::info!("Hotplug: Bringing CPU {} online (MPIDR=0x{:x})",
                   cpu_id, mpidr);

        // Check if CPU is available
        if !self.is_available(cpu_id) {
            return Err("CPU not available for hotplug");
        }

        // Check if CPU is already online
        if self.is_online(cpu_id) {
            log::warn!("Hotplug: CPU {} already online", cpu_id);
            return Ok(true);
        }

        // Mark CPU as onlining
        self.cpu_states[cpu_id as usize] = HotplugState::Onlining;

        // Call PSCI CPU_ON
        match self.psci_ops.psci_cpu_on(mpidr, entry_point, 0) {
            Ok(()) => {
                // Mark CPU as online
                self.cpu_states[cpu_id as usize] = HotplugState::Online;
                self.online_mask |= 1 << cpu_id;

                log::info!("Hotplug: CPU {} is now online", cpu_id);

                // Notify callbacks
                for callback in &mut self.callbacks {
                    callback.cpu_online(cpu_id);
                }

                // Send notification event
                self.notify(HotplugEvent::CpuOnline(cpu_id));

                Ok(true)
            }
            Err(e) => {
                // Mark CPU as offline failed
                self.cpu_states[cpu_id as usize] = HotplugState::OfflineFailed;

                log::error!("Hotplug: CPU {} online failed: {}", cpu_id, e);

                // Notify callbacks
                for callback in &mut self.callbacks {
                    callback.cpu_online_failed(cpu_id);
                }

                // Send notification event
                self.notify(HotplugEvent::OnlineFailed(cpu_id));

                Err(e)
            }
        }
    }

    /// Take CPU offline
    ///
    /// # Parameters
    /// - `cpu_id`: CPU to take offline
    ///
    /// Returns true if CPU was successfully taken offline
    pub fn cpu_offline(&mut self, cpu_id: u32) -> Result<bool, &'static str> {
        log::info!("Hotplug: Taking CPU {} offline", cpu_id);

        // Cannot take boot CPU offline
        if cpu_id == 0 {
            return Err("Cannot take boot CPU offline");
        }

        // Check if CPU is online
        if !self.is_online(cpu_id) {
            log::warn!("Hotplug: CPU {} not online", cpu_id);
            return Ok(false);
        }

        // Check if CPU can go offline
        if !self.can_offline(cpu_id) {
            return Err("CPU cannot go offline (busy or has pinned IRQs)");
        }

        // Mark CPU as offlining
        self.cpu_states[cpu_id as usize] = HotplugState::Offlining;

        // Call PSCI CPU_OFF
        match self.psci_ops.psci_cpu_off() {
            Ok(()) => {
                // Mark CPU as offline
                self.cpu_states[cpu_id as usize] = HotplugState::Offline;
                self.online_mask &= !(1 << cpu_id);

                log::info!("Hotplug: CPU {} is now offline", cpu_id);

                // Notify callbacks
                for callback in &mut self.callbacks {
                    callback.cpu_offline(cpu_id);
                }

                // Send notification event
                self.notify(HotplugEvent::CpuOffline(cpu_id));

                Ok(true)
            }
            Err(e) => {
                // Mark CPU as online (since offline failed)
                self.cpu_states[cpu_id as usize] = HotplugState::Online;

                log::error!("Hotplug: CPU {} offline failed: {}", cpu_id, e);

                // Notify callbacks
                for callback in &mut self.callbacks {
                    callback.cpu_offline_failed(cpu_id);
                }

                // Send notification event
                self.notify(HotplugEvent::OfflineFailed(cpu_id));

                Err(e)
            }
        }
    }

    /// Check if CPU can go offline
    ///
    /// A CPU can go offline if:
    /// - It's not the boot CPU
    /// - It has no pinned IRQs
    /// - It has no work scheduled
    fn can_offline(&self, cpu_id: u32) -> bool {
        // Boot CPU cannot go offline
        if cpu_id == 0 {
            return false;
        }

        // TODO: Check for pinned IRQs
        // TODO: Check for work scheduled

        true
    }

    /// Send hotplug notification
    fn notify(&self, event: HotplugEvent) {
        log::debug!("Hotplug: Event {:?}", event);
        // TODO: Send notification to subsystems
        // This could use an event queue or callback mechanism
    }

    /// Get CPU statistics
    pub fn stats(&self) -> HotplugStats {
        let total = self.available_mask.count_ones();
        let online = self.online_mask.count_ones();
        let offline = total - online;

        HotplugStats {
            total_cpus: total as u32,
            online_cpus: online as u32,
            offline_cpus: offline as u32,
        }
    }
}

/// CPU hotplug statistics
#[derive(Debug, Clone, Copy)]
pub struct HotplugStats {
    /// Total available CPUs
    pub total_cpus: u32,
    /// Number of online CPUs
    pub online_cpus: u32,
    /// Number of offline CPUs
    pub offline_cpus: u32,
}

/// Initialize CPU hotplug
pub fn init() -> Result<(), &'static str> {
    log::info!("Hotplug: Initializing CPU hotplug");

    let mut manager = HotplugManager::new();
    manager.init(4)?; // Default to 4 CPUs

    log::info!("Hotplug: Initialized");
    Ok(())
}

/// Get global hotplug manager
pub fn manager() -> Option<&'static mut HotplugManager> {
    static mut MANAGER: Option<HotplugManager> = None;

    unsafe {
        if MANAGER.is_none() {
            MANAGER = Some(HotplugManager::new());
        }
        MANAGER.as_mut()
    }
}

/// Simple hotplug callback implementation
pub struct SimpleHotplugCallback {
    name: &'static str,
}

impl SimpleHotplugCallback {
    /// Create new simple callback
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }
}

impl HotplugCallback for SimpleHotplugCallback {
    fn cpu_online(&mut self, cpu_id: u32) {
        log::info!("{}: CPU {} online", self.name, cpu_id);
    }

    fn cpu_offline(&mut self, cpu_id: u32) {
        log::info!("{}: CPU {} offline", self.name, cpu_id);
    }

    fn cpu_online_failed(&mut self, cpu_id: u32) {
        log::warn!("{}: CPU {} online failed", self.name, cpu_id);
    }

    fn cpu_offline_failed(&mut self, cpu_id: u32) {
        log::warn!("{}: CPU {} offline failed", self.name, cpu_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hotplug_manager() {
        let mut manager = HotplugManager::new();
        manager.init(4).unwrap();

        assert_eq!(manager.is_online(0), true);
        assert_eq!(manager.is_online(1), false);
        assert_eq!(manager.online_count(), 1);
    }

    #[test]
    fn test_hotplug_states() {
        assert_eq!(HotplugState::Offline as u8, 1);
        assert_eq!(HotplugState::Online as u8, 3);
    }

    #[test]
    fn test_hotplug_stats() {
        let mut manager = HotplugManager::new();
        manager.init(4).unwrap();

        let stats = manager.stats();
        assert_eq!(stats.total_cpus, 4);
        assert_eq!(stats.online_cpus, 1);
        assert_eq!(stats.offline_cpus, 3);
    }

    #[test]
    fn test_simple_callback() {
        let callback = SimpleHotplugCallback::new("test");
        callback.cpu_online(1);
        callback.cpu_offline(1);
    }
}
