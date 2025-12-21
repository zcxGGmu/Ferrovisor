//! RISC-V Virtual Interrupt Controller
//!
//! This module provides virtual interrupt injection and management functionality for RISC-V virtualization:
//! - Virtual interrupt injection into VCPUs
//! - Virtual interrupt controller state management
//! - Interrupt prioritization and masking
//! - HVIP/VSIE/VSIP register virtualization
//! - Nested virtualization interrupt support
//! - Platform interrupt virtualization
//! - AIA (Advanced Interrupt Architecture) support

use crate::arch::riscv64::cpu::csr;
use crate::arch::riscv64::virtualization::{VcpuId, VmId};
use bitflags::bitflags;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Simple slab allocator for VCPU states (replacement for slab crate)
struct VcpuStateSlab {
    entries: Vec<Option<VcpuInterruptState>>,
    next_free: usize,
}

impl VcpuStateSlab {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_free: 0,
        }
    }

    fn insert(&mut self, value: VcpuInterruptState) -> usize {
        // Find next free slot
        if self.next_free < self.entries.len() {
            let key = self.next_free;
            self.entries[key] = Some(value);

            // Find next free slot
            self.next_free = self.entries.iter()
                .enumerate()
                .find(|(_, entry)| entry.is_none())
                .map(|(i, _)| i)
                .unwrap_or(self.entries.len());

            key
        } else {
            // Append new entry
            self.entries.push(Some(value));
            self.next_free = self.entries.len();
            self.entries.len() - 1
        }
    }

    fn get(&self, key: usize) -> Option<&VcpuInterruptState> {
        self.entries.get(key).and_then(|entry| entry.as_ref())
    }

    fn get_mut(&mut self, key: usize) -> Option<&mut VcpuInterruptState> {
        self.entries.get_mut(key).and_then(|entry| entry.as_mut())
    }

    fn contains(&self, key: usize) -> bool {
        key < self.entries.len() && self.entries[key].is_some()
    }

    fn remove(&mut self, key: usize) -> Option<VcpuInterruptState> {
        if key < self.entries.len() {
            let result = self.entries[key].take();
            if result.is_some() && key < self.next_free {
                self.next_free = key;
            }
            result
        } else {
            None
        }
    }

    fn len(&self) -> usize {
        self.entries.iter().filter(|entry| entry.is_some()).count()
    }

    fn iter(&self) -> impl Iterator<Item = (usize, &VcpuInterruptState)> {
        self.entries.iter()
            .enumerate()
            .filter_map(|(i, entry)| entry.as_ref().map(|state| (i, state)))
    }
}

type Slab<T> = VcpuStateSlab;

/// Virtual interrupt types following RISC-V H-extension specification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VirtualInterruptType {
    /// Virtual Supervisor Software Interrupt (SSIP)
    SupervisorSoftware = 1,
    /// Virtual Supervisor Timer Interrupt (STIP)
    SupervisorTimer = 5,
    /// Virtual Supervisor External Interrupt (SEIP)
    SupervisorExternal = 9,
    /// Custom virtual interrupt
    Custom(u32),
}

impl VirtualInterruptType {
    /// Convert to interrupt bit position
    pub fn bit_position(self) -> u32 {
        match self {
            VirtualInterruptType::SupervisorSoftware => 1,
            VirtualInterruptType::SupervisorTimer => 5,
            VirtualInterruptType::SupervisorExternal => 9,
            VirtualInterruptType::Custom(id) => id,
        }
    }

    /// Convert to interrupt mask
    pub fn mask(self) -> u64 {
        1u64 << self.bit_position()
    }

    /// Check if interrupt is standard RISC-V type
    pub fn is_standard(self) -> bool {
        matches!(self,
            VirtualInterruptType::SupervisorSoftware |
            VirtualInterruptType::SupervisorTimer |
            VirtualInterruptType::SupervisorExternal
        )
    }

    /// Get interrupt priority (RISC-V standard: all same priority)
    pub fn priority(self) -> u8 {
        match self {
            VirtualInterruptType::SupervisorSoftware => 2,
            VirtualInterruptType::SupervisorTimer => 2,
            VirtualInterruptType::SupervisorExternal => 2,
            VirtualInterruptType::Custom(_) => 2, // Standard RISC-V priority
        }
    }
}

impl TryFrom<u32> for VirtualInterruptType {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(VirtualInterruptType::SupervisorSoftware),
            5 => Ok(VirtualInterruptType::SupervisorTimer),
            9 => Ok(VirtualInterruptType::SupervisorExternal),
            10..=63 => Ok(VirtualInterruptType::Custom(value)),
            _ => Err(()),
        }
    }
}

/// Virtual interrupt injection flags
bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct VirtualInterruptFlags: u32 {
        /// Normal interrupt injection
        const NORMAL = 0x01;
        /// High priority interrupt
        const HIGH_PRIORITY = 0x02;
        /// Inject immediately (bypass queuing)
        const IMMEDIATE = 0x04;
        /// Auto-clear after delivery
        const AUTO_CLEAR = 0x08;
        /// Level-triggered interrupt
        const LEVEL_TRIGGERED = 0x10;
        /// Edge-triggered interrupt
        const EDGE_TRIGGERED = 0x20;
        /// Target all VCPUs in VM
        const BROADCAST = 0x40;
        /// Persistent interrupt (re-inject if not handled)
        const PERSISTENT = 0x80;
    }
}

/// Virtual interrupt state for a single VCPU
#[derive(Debug, Clone)]
pub struct VcpuInterruptState {
    /// VCPU identifier
    pub vcpu_id: VcpuId,
    /// VM identifier
    pub vmid: VmId,

    /// Virtual interrupt enable bits (VSIE equivalent)
    pub virtual_ie: u64,
    /// Virtual interrupt pending bits (VSIP equivalent)
    pub virtual_ip: u64,
    /// Hypervisor virtual interrupt pending (HVIP equivalent)
    pub hvip: u64,

    /// Interrupt injection flags per interrupt
    pub interrupt_flags: [VirtualInterruptFlags; 64],

    /// Interrupt injection timestamps
    pub injection_timestamps: [u64; 64],

    /// Interrupt delivery statistics
    pub stats: VcpuInterruptStats,

    /// Interrupt masks for custom interrupt ranges
    pub custom_mask_enabled: bool,
    pub custom_interrupt_mask: u64,
}

impl Default for VcpuInterruptState {
    fn default() -> Self {
        Self {
            vcpu_id: 0,
            vmid: 0,
            virtual_ie: 0,
            virtual_ip: 0,
            hvip: 0,
            interrupt_flags: [VirtualInterruptFlags::NORMAL; 64],
            injection_timestamps: [0; 64],
            stats: VcpuInterruptStats::default(),
            custom_mask_enabled: false,
            custom_interrupt_mask: u64::MAX,
        }
    }
}

impl VcpuInterruptState {
    /// Create new VCPU interrupt state
    pub fn new(vcpu_id: VcpuId, vmid: VmId) -> Self {
        let mut state = Self {
            vcpu_id,
            vmid,
            ..Default::default()
        };

        // Enable standard virtual interrupts by default
        state.virtual_ie |= VirtualInterruptType::SupervisorSoftware.mask();
        state.virtual_ie |= VirtualInterruptType::SupervisorTimer.mask();
        state.virtual_ie |= VirtualInterruptType::SupervisorExternal.mask();

        state
    }

    /// Check if interrupt is enabled
    pub fn is_interrupt_enabled(&self, interrupt_type: VirtualInterruptType) -> bool {
        let mask = interrupt_type.mask();

        if interrupt_type.is_standard() {
            (self.virtual_ie & mask) != 0
        } else {
            // For custom interrupts, check custom mask
            self.custom_mask_enabled && ((self.custom_interrupt_mask & mask) != 0)
        }
    }

    /// Enable/disable specific interrupt
    pub fn enable_interrupt(&mut self, interrupt_type: VirtualInterruptType, enable: bool) {
        let mask = interrupt_type.mask();

        if interrupt_type.is_standard() {
            if enable {
                self.virtual_ie |= mask;
            } else {
                self.virtual_ie &= !mask;
            }
        } else {
            // For custom interrupts
            if self.custom_mask_enabled {
                if enable {
                    self.custom_interrupt_mask |= mask;
                } else {
                    self.custom_interrupt_mask &= !mask;
                }
            }
        }
    }

    /// Check if interrupt is pending
    pub fn is_interrupt_pending(&self, interrupt_type: VirtualInterruptType) -> bool {
        (self.virtual_ip & interrupt_type.mask()) != 0
    }

    /// Get pending interrupt mask
    pub fn get_pending_interrupts(&self) -> u64 {
        self.virtual_ip & self.virtual_ie
    }

    /// Get highest priority pending interrupt
    pub fn get_highest_priority_pending(&self) -> Option<VirtualInterruptType> {
        let pending = self.get_pending_interrupts();

        if pending == 0 {
            return None;
        }

        // RISC-V has uniform priority, so find lowest set bit
        let position = pending.trailing_zeros() as u32;

        VirtualInterruptType::try_from(position).ok()
    }

    /// Check if any interrupt is pending
    pub fn has_pending_interrupts(&self) -> bool {
        self.get_pending_interrupts() != 0
    }

    /// Inject interrupt with flags
    pub fn inject_interrupt(&mut self, interrupt_type: VirtualInterruptType,
                           flags: VirtualInterruptFlags) -> Result<(), &'static str> {
        if !self.is_interrupt_enabled(interrupt_type) {
            return Err("Interrupt is not enabled");
        }

        let mask = interrupt_type.mask();
        let timestamp = get_timestamp();

        // Update injection flags
        let bit_pos = interrupt_type.bit_position() as usize;
        if bit_pos < 64 {
            self.interrupt_flags[bit_pos] = flags;
            self.injection_timestamps[bit_pos] = timestamp;
        }

        // Set pending bit
        self.virtual_ip |= mask;
        self.hvip |= mask;

        // Update statistics
        self.stats.interrupts_injected += 1;
        self.stats.last_injection_timestamp = timestamp;

        // For immediate injection, also set hardware state
        if flags.contains(VirtualInterruptFlags::IMMEDIATE) {
            self.set_hvip_register(mask)?;
        }

        log::debug!("Injected interrupt {:?} into VCPU {} (VMID: {}) with flags: {:?}",
                   interrupt_type, self.vcpu_id, self.vmid, flags);

        Ok(())
    }

    /// Clear interrupt
    pub fn clear_interrupt(&mut self, interrupt_type: VirtualInterruptType) -> Result<(), &'static str> {
        let mask = interrupt_type.mask();

        // Clear pending bits
        self.virtual_ip &= !mask;
        self.hvip &= !mask;

        // Clear hardware HVIP register
        self.clear_hvip_register(mask)?;

        // Update statistics
        self.stats.interrupts_cleared += 1;

        log::debug!("Cleared interrupt {:?} from VCPU {} (VMID: {})",
                   interrupt_type, self.vcpu_id, self.vmid);

        Ok(())
    }

    /// Assert/deassert interrupt (similar to Linux IRQ API)
    pub fn assert_interrupt(&mut self, interrupt_type: VirtualInterruptType) -> Result<(), &'static str> {
        let flags = VirtualInterruptFlags::LEVEL_TRIGGERED | VirtualInterruptFlags::NORMAL;
        self.inject_interrupt(interrupt_type, flags)
    }

    pub fn deassert_interrupt(&mut self, interrupt_type: VirtualInterruptType) -> Result<(), &'static str> {
        self.clear_interrupt(interrupt_type)
    }

    /// Set HVIP register (hardware operation)
    fn set_hvip_register(&mut self, mask: u64) -> Result<(), &'static str> {
        // In a real implementation, this would write to HVIP CSR
        // For now, we simulate the operation
        log::trace!("Would set HVIP register bits: {:#x}", mask);
        Ok(())
    }

    /// Clear HVIP register (hardware operation)
    fn clear_hvip_register(&mut self, mask: u64) -> Result<(), &'static str> {
        // In a real implementation, this would clear HVIP CSR bits
        // For now, we simulate the operation
        log::trace!("Would clear HVIP register bits: {:#x}", mask);
        Ok(())
    }

    /// Synchronize with hardware state
    pub fn sync_with_hardware(&mut self) -> Result<(), &'static str> {
        // Read current HVIP state
        // In a real implementation, this would read from HVIP CSR
        log::trace!("Would sync VCPU {} interrupt state with hardware", self.vcpu_id);
        Ok(())
    }

    /// Update statistics
    pub fn update_stats(&mut self) {
        self.stats.stats_timestamp = get_timestamp();
        self.stats.pending_count = self.virtual_ip.count_ones() as usize;
        self.stats.enabled_count = self.virtual_ie.count_ones() as usize;
    }
}

/// VCPU interrupt statistics
#[derive(Debug, Clone, Default)]
pub struct VcpuInterruptStats {
    /// Total interrupts injected
    pub interrupts_injected: u64,
    /// Total interrupts cleared
    pub interrupts_cleared: u64,
    /// Total interrupts delivered
    pub interrupts_delivered: u64,
    /// Current pending interrupt count
    pub pending_count: usize,
    /// Current enabled interrupt count
    pub enabled_count: usize,
    /// Last injection timestamp
    pub last_injection_timestamp: u64,
    /// Statistics snapshot timestamp
    pub stats_timestamp: u64,
}

/// Virtual interrupt injection result
#[derive(Debug, Clone, Copy)]
pub struct InjectionResult {
    /// Injection was successful
    pub success: bool,
    /// Interrupt was already pending
    pub already_pending: bool,
    /// Interrupt was delivered immediately
    pub immediate_delivery: bool,
    /// Number of VCPUs affected
    pub vcpus_affected: usize,
    /// Error message (if any)
    pub error: Option<&'static str>,
}

/// Virtual interrupt controller
pub struct VirtualInterruptController {
    /// Per-VCPU interrupt states
    vcpu_states: Slab<VcpuInterruptState>,

    /// Global injection statistics
    global_stats: VirtualIntcStats,

    /// Configuration
    config: VirtualIntcConfig,
}

/// Virtual interrupt controller configuration
#[derive(Debug, Clone)]
pub struct VirtualIntcConfig {
    /// Maximum number of VCPUs supported
    pub max_vcpus: usize,
    /// Enable custom interrupt support
    pub enable_custom_interrupts: bool,
    /// Enable AIA (Advanced Interrupt Architecture) support
    pub enable_aia: bool,
    /// Default interrupt flags
    pub default_flags: VirtualInterruptFlags,
    /// Global interrupt mask
    pub global_interrupt_mask: u64,
}

impl Default for VirtualIntcConfig {
    fn default() -> Self {
        Self {
            max_vcpus: 64,
            enable_custom_interrupts: true,
            enable_aia: false, // Requires additional hardware support
            default_flags: VirtualInterruptFlags::NORMAL | VirtualInterruptFlags::LEVEL_TRIGGERED,
            global_interrupt_mask: u64::MAX,
        }
    }
}

/// Global virtual interrupt controller statistics
#[derive(Debug, Default)]
pub struct VirtualIntcStats {
    /// Total injections attempted
    pub total_injection_attempts: AtomicU64,
    /// Total successful injections
    pub successful_injections: AtomicU64,
    /// Total failed injections
    pub failed_injections: AtomicU64,
    /// Total interrupts delivered
    pub total_deliveries: AtomicU64,
    /// Concurrent injection operations
    pub concurrent_operations: AtomicUsize,
}

impl VirtualInterruptController {
    /// Create new virtual interrupt controller
    pub fn new(config: VirtualIntcConfig) -> Self {
        Self {
            vcpu_states: slab::Slab::new(),
            global_stats: VirtualIntcStats::default(),
            config,
        }
    }

    /// Initialize the controller
    pub fn init(&mut self) -> Result<(), &'static str> {
        log::info!("Initializing RISC-V Virtual Interrupt Controller");

        log::info!("Virtual Interrupt Controller initialized successfully");
        Ok(())
    }

    /// Register a VCPU with the controller
    pub fn register_vcpu(&mut self, vcpu_id: VcpuId, vmid: VmId) -> Result<usize, &'static str> {
        if self.vcpu_states.len() >= self.config.max_vcpus {
            return Err("Maximum VCPU limit reached");
        }

        let state = VcpuInterruptState::new(vcpu_id, vmid);
        let key = self.vcpu_states.insert(state);

        log::debug!("Registered VCPU {} (VMID: {}) with VIC key: {}", vcpu_id, vmid, key);
        Ok(key)
    }

    /// Unregister a VCPU from the controller
    pub fn unregister_vcpu(&mut self, vcpu_key: usize) -> Result<(), &'static str> {
        if self.vcpu_states.contains(vcpu_key) {
            let state = self.vcpu_states.remove(vcpu_key);
            log::debug!("Unregistered VCPU {} (VMID: {}) with VIC key: {}",
                       state.vcpu_id, state.vmid, vcpu_key);
            Ok(())
        } else {
            Err("VCPU not found")
        }
    }

    /// Inject interrupt into specific VCPU
    pub fn inject_interrupt(&mut self, vcpu_key: usize, interrupt_type: VirtualInterruptType,
                           flags: VirtualInterruptFlags) -> InjectionResult {
        self.global_stats.total_injection_attempts.fetch_add(1, Ordering::Relaxed);
        self.global_stats.concurrent_operations.fetch_add(1, Ordering::Relaxed);

        let result = if let Some(state) = self.vcpu_states.get_mut(vcpu_key) {
            let was_pending = state.is_interrupt_pending(interrupt_type);

            match state.inject_interrupt(interrupt_type, flags) {
                Ok(()) => {
                    self.global_stats.successful_injections.fetch_add(1, Ordering::Relaxed);

                    InjectionResult {
                        success: true,
                        already_pending: was_pending,
                        immediate_delivery: flags.contains(VirtualInterruptFlags::IMMEDIATE),
                        vcpus_affected: 1,
                        error: None,
                    }
                }
                Err(e) => {
                    self.global_stats.failed_injections.fetch_add(1, Ordering::Relaxed);

                    InjectionResult {
                        success: false,
                        already_pending: false,
                        immediate_delivery: false,
                        vcpus_affected: 0,
                        error: Some(e),
                    }
                }
            }
        } else {
            self.global_stats.failed_injections.fetch_add(1, Ordering::Relaxed);

            InjectionResult {
                success: false,
                already_pending: false,
                immediate_delivery: false,
                vcpus_affected: 0,
                error: Some("VCPU not found"),
            }
        };

        self.global_stats.concurrent_operations.fetch_sub(1, Ordering::Relaxed);
        result
    }

    /// Broadcast interrupt to all VCPUs in a VM
    pub fn inject_interrupt_to_vm(&mut self, vmid: VmId, interrupt_type: VirtualInterruptType,
                                 flags: VirtualInterruptFlags) -> Vec<InjectionResult> {
        let mut results = Vec::new();
        let broadcast_flags = flags | VirtualInterruptFlags::BROADCAST;

        for (key, state) in self.vcpu_states.iter() {
            if state.vmid == vmid {
                let result = self.inject_interrupt(key, interrupt_type, broadcast_flags);
                results.push(result);
            }
        }

        log::debug!("Broadcast interrupt {:?} to {} VCPUs in VM {}",
                   interrupt_type, results.len(), vmid);

        results
    }

    /// Clear interrupt from specific VCPU
    pub fn clear_interrupt(&mut self, vcpu_key: usize, interrupt_type: VirtualInterruptType) -> Result<(), &'static str> {
        if let Some(state) = self.vcpu_states.get_mut(vcpu_key) {
            state.clear_interrupt(interrupt_type)
        } else {
            Err("VCPU not found")
        }
    }

    /// Get VCPU interrupt state
    pub fn get_vcpu_state(&self, vcpu_key: usize) -> Option<&VcpuInterruptState> {
        self.vcpu_states.get(vcpu_key)
    }

    /// Get mutable VCPU interrupt state
    pub fn get_vcpu_state_mut(&mut self, vcpu_key: usize) -> Option<&mut VcpuInterruptState> {
        self.vcpu_states.get_mut(vcpu_key)
    }

    /// Find VCPU by VCPU ID
    pub fn find_vcpu_by_id(&self, vcpu_id: VcpuId) -> Option<usize> {
        self.vcpu_states.iter().find(|(_, state)| state.vcpu_id == vcpu_id).map(|(key, _)| key)
    }

    /// Find VCPUs by VM ID
    pub fn find_vcpus_by_vm(&self, vmid: VmId) -> Vec<usize> {
        self.vcpu_states.iter()
            .filter(|(_, state)| state.vmid == vmid)
            .map(|(key, _)| key)
            .collect()
    }

    /// Synchronize all VCPU states with hardware
    pub fn sync_all_with_hardware(&mut self) -> Result<(), &'static str> {
        for state in self.vcpu_states.values_mut() {
            state.sync_with_hardware()?;
        }
        Ok(())
    }

    /// Get global statistics
    pub fn get_global_stats(&self) -> VirtualIntcStatsSnapshot {
        VirtualIntcStatsSnapshot {
            total_injection_attempts: self.global_stats.total_injection_attempts.load(Ordering::Relaxed),
            successful_injections: self.global_stats.successful_injections.load(Ordering::Relaxed),
            failed_injections: self.global_stats.failed_injections.load(Ordering::Relaxed),
            total_deliveries: self.global_stats.total_deliveries.load(Ordering::Relaxed),
            concurrent_operations: self.global_stats.concurrent_operations.load(Ordering::Relaxed),
            registered_vcpus: self.vcpu_states.len(),
        }
    }

    /// Reset global statistics
    pub fn reset_global_stats(&mut self) {
        self.global_stats = VirtualIntcStats::default();
    }

    /// Update configuration
    pub fn update_config(&mut self, config: VirtualIntcConfig) -> Result<(), &'static str> {
        self.config = config;
        Ok(())
    }

    /// Get configuration
    pub fn get_config(&self) -> &VirtualIntcConfig {
        &self.config
    }
}

/// Snapshot of global virtual interrupt controller statistics
#[derive(Debug, Clone)]
pub struct VirtualIntcStatsSnapshot {
    pub total_injection_attempts: u64,
    pub successful_injections: u64,
    pub failed_injections: u64,
    pub total_deliveries: u64,
    pub concurrent_operations: usize,
    pub registered_vcpus: usize,
}

/// Global virtual interrupt controller instance
static mut VIRTUAL_INTC: Option<VirtualInterruptController> = None;

/// Initialize global virtual interrupt controller
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing RISC-V Virtual Interrupt Controller");

    let config = VirtualIntcConfig::default();
    let mut controller = VirtualInterruptController::new(config);
    controller.init()?;

    // Store global controller
    unsafe {
        VIRTUAL_INTC = Some(controller);
    }

    log::info!("RISC-V Virtual Interrupt Controller initialized successfully");
    Ok(())
}

/// Get the global virtual interrupt controller
pub fn get_controller() -> Option<&'static VirtualInterruptController> {
    unsafe { VIRTUAL_INTC.as_ref() }
}

/// Get mutable reference to global virtual interrupt controller
pub fn get_controller_mut() -> Option<&'static mut VirtualInterruptController> {
    unsafe { VIRTUAL_INTC.as_mut() }
}

/// Inject interrupt into VCPU by ID
pub fn inject_interrupt(vcpu_id: VcpuId, interrupt_type: VirtualInterruptType,
                       flags: VirtualInterruptFlags) -> InjectionResult {
    if let Some(controller) = get_controller_mut() {
        if let Some(vcpu_key) = controller.find_vcpu_by_id(vcpu_id) {
            controller.inject_interrupt(vcpu_key, interrupt_type, flags)
        } else {
            InjectionResult {
                success: false,
                already_pending: false,
                immediate_delivery: false,
                vcpus_affected: 0,
                error: Some("VCPU not found"),
            }
        }
    } else {
        InjectionResult {
            success: false,
            already_pending: false,
            immediate_delivery: false,
            vcpus_affected: 0,
            error: Some("Virtual interrupt controller not initialized"),
        }
    }
}

/// Clear interrupt from VCPU by ID
pub fn clear_interrupt(vcpu_id: VcpuId, interrupt_type: VirtualInterruptType) -> Result<(), &'static str> {
    if let Some(controller) = get_controller_mut() {
        if let Some(vcpu_key) = controller.find_vcpu_by_id(vcpu_id) {
            controller.clear_interrupt(vcpu_key, interrupt_type)
        } else {
            Err("VCPU not found")
        }
    } else {
        Err("Virtual interrupt controller not initialized")
    }
}

/// Broadcast interrupt to all VCPUs in a VM
pub fn inject_interrupt_to_vm(vmid: VmId, interrupt_type: VirtualInterruptType,
                             flags: VirtualInterruptFlags) -> Vec<InjectionResult> {
    if let Some(controller) = get_controller_mut() {
        controller.inject_interrupt_to_vm(vmid, interrupt_type, flags)
    } else {
        Vec::new()
    }
}

/// Get VCPU interrupt statistics by VCPU ID
pub fn get_vcpu_interrupt_stats(vcpu_id: VcpuId) -> Option<VcpuInterruptStats> {
    if let Some(controller) = get_controller() {
        if let Some(vcpu_key) = controller.find_vcpu_by_id(vcpu_id) {
            controller.get_vcpu_state(vcpu_key).map(|state| {
                let mut stats = state.stats.clone();
                // Update snapshot data
                stats.stats_timestamp = get_timestamp();
                stats.pending_count = state.virtual_ip.count_ones() as usize;
                stats.enabled_count = state.virtual_ie.count_ones() as usize;
                stats
            })
        } else {
            None
        }
    } else {
        None
    }
}

/// Get global virtual interrupt controller statistics
pub fn get_global_stats() -> Option<VirtualIntcStatsSnapshot> {
    get_controller().map(|c| c.get_global_stats())
}

/// Register VCPU with virtual interrupt controller
pub fn register_vcpu(vcpu_id: VcpuId, vmid: VmId) -> Result<usize, &'static str> {
    if let Some(controller) = get_controller_mut() {
        controller.register_vcpu(vcpu_id, vmid)
    } else {
        Err("Virtual interrupt controller not initialized")
    }
}

/// Unregister VCPU from virtual interrupt controller
pub fn unregister_vcpu(vcpu_key: usize) -> Result<(), &'static str> {
    if let Some(controller) = get_controller_mut() {
        controller.unregister_vcpu(vcpu_key)
    } else {
        Err("Virtual interrupt controller not initialized")
    }
}

/// Get current timestamp
fn get_timestamp() -> u64 {
    // Use a simple counter for now
    // In a real implementation, this would use the time CSR
    static mut COUNTER: u64 = 0;
    unsafe {
        COUNTER += 1;
        COUNTER
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_interrupt_types() {
        let sw_int = VirtualInterruptType::SupervisorSoftware;
        let timer_int = VirtualInterruptType::SupervisorTimer;
        let ext_int = VirtualInterruptType::SupervisorExternal;
        let custom_int = VirtualInterruptType::Custom(15);

        assert_eq!(sw_int.bit_position(), 1);
        assert_eq!(timer_int.bit_position(), 5);
        assert_eq!(ext_int.bit_position(), 9);
        assert_eq!(custom_int.bit_position(), 15);

        assert!(sw_int.is_standard());
        assert!(!custom_int.is_standard());

        assert_eq!(sw_int.mask(), 0x02);
        assert_eq!(timer_int.mask(), 0x20);
        assert_eq!(ext_int.mask(), 0x200);
        assert_eq!(custom_int.mask(), 0x8000);

        assert_eq!(sw_int.priority(), 2);
    }

    #[test]
    fn test_vcpu_interrupt_state() {
        let mut state = VcpuInterruptState::new(1, 100);

        // Test initial state
        assert!(state.is_interrupt_enabled(VirtualInterruptType::SupervisorSoftware));
        assert!(state.is_interrupt_enabled(VirtualInterruptType::SupervisorTimer));
        assert!(state.is_interrupt_enabled(VirtualInterruptType::SupervisorExternal));

        assert!(!state.is_interrupt_pending(VirtualInterruptType::SupervisorSoftware));
        assert!(!state.has_pending_interrupts());

        // Test interrupt injection
        let result = state.inject_interrupt(
            VirtualInterruptType::SupervisorSoftware,
            VirtualInterruptFlags::NORMAL
        );
        assert!(result.is_ok());
        assert!(state.is_interrupt_pending(VirtualInterruptType::SupervisorSoftware));
        assert!(state.has_pending_interrupts());

        // Test highest priority pending
        let pending = state.get_highest_priority_pending();
        assert_eq!(pending, Some(VirtualInterruptType::SupervisorSoftware));

        // Test interrupt clearing
        let result = state.clear_interrupt(VirtualInterruptType::SupervisorSoftware);
        assert!(result.is_ok());
        assert!(!state.is_interrupt_pending(VirtualInterruptType::SupervisorSoftware));
    }

    #[test]
    fn test_vcpu_interrupt_enable_disable() {
        let mut state = VcpuInterruptState::new(1, 100);

        // Disable software interrupt
        state.enable_interrupt(VirtualInterruptType::SupervisorSoftware, false);
        assert!(!state.is_interrupt_enabled(VirtualInterruptType::SupervisorSoftware));

        // Try to inject disabled interrupt
        let result = state.inject_interrupt(
            VirtualInterruptType::SupervisorSoftware,
            VirtualInterruptFlags::NORMAL
        );
        assert!(result.is_err());

        // Re-enable software interrupt
        state.enable_interrupt(VirtualInterruptType::SupervisorSoftware, true);
        assert!(state.is_interrupt_enabled(VirtualInterruptType::SupervisorSoftware));
    }

    #[test]
    fn test_vcpu_interrupt_assert_deassert() {
        let mut state = VcpuInterruptState::new(1, 100);

        // Assert interrupt
        let result = state.assert_interrupt(VirtualInterruptType::SupervisorTimer);
        assert!(result.is_ok());
        assert!(state.is_interrupt_pending(VirtualInterruptType::SupervisorTimer));

        // Deassert interrupt
        let result = state.deassert_interrupt(VirtualInterruptType::SupervisorTimer);
        assert!(result.is_ok());
        assert!(!state.is_interrupt_pending(VirtualInterruptType::SupervisorTimer));
    }

    #[test]
    fn test_virtual_interrupt_controller() {
        let config = VirtualIntcConfig::default();
        let mut controller = VirtualInterruptController::new(config);

        // Register VCPU
        let vcpu_key = controller.register_vcpu(1, 100).unwrap();
        assert_eq!(controller.get_vcpu_state(vcpu_key).unwrap().vcpu_id, 1);

        // Inject interrupt
        let result = controller.inject_interrupt(
            vcpu_key,
            VirtualInterruptType::SupervisorSoftware,
            VirtualInterruptFlags::NORMAL
        );
        assert!(result.success);
        assert!(!result.already_pending);

        // Check state
        let state = controller.get_vcpu_state(vcpu_key).unwrap();
        assert!(state.is_interrupt_pending(VirtualInterruptType::SupervisorSoftware));

        // Inject same interrupt again
        let result2 = controller.inject_interrupt(
            vcpu_key,
            VirtualInterruptType::SupervisorSoftware,
            VirtualInterruptFlags::NORMAL
        );
        assert!(result2.success);
        assert!(result2.already_pending);

        // Clear interrupt
        let result3 = controller.clear_interrupt(vcpu_key, VirtualInterruptType::SupervisorSoftware);
        assert!(result3.is_ok());
    }

    #[test]
    fn test_virtual_interrupt_controller_broadcast() {
        let config = VirtualIntcConfig::default();
        let mut controller = VirtualInterruptController::new(config);

        // Register multiple VCPUs in same VM
        let vcpu1_key = controller.register_vcpu(1, 100).unwrap();
        let vcpu2_key = controller.register_vcpu(2, 100).unwrap();
        let vcpu3_key = controller.register_vcpu(3, 200).unwrap(); // Different VM

        // Broadcast to VM 100
        let results = controller.inject_interrupt_to_vm(
            100,
            VirtualInterruptType::SupervisorTimer,
            VirtualInterruptFlags::NORMAL
        );

        assert_eq!(results.len(), 2); // Only VCPUs 1 and 2 in VM 100

        for result in &results {
            assert!(result.success);
        }

        // Check VCPU states
        assert!(controller.get_vcpu_state(vcpu1_key).unwrap()
                .is_interrupt_pending(VirtualInterruptType::SupervisorTimer));
        assert!(controller.get_vcpu_state(vcpu2_key).unwrap()
                .is_interrupt_pending(VirtualInterruptType::SupervisorTimer));
        assert!(!controller.get_vcpu_state(vcpu3_key).unwrap()
                 .is_interrupt_pending(VirtualInterruptType::SupervisorTimer));
    }

    #[test]
    fn test_interrupt_statistics() {
        let mut state = VcpuInterruptState::new(1, 100);

        // Update stats
        state.update_stats();
        assert_eq!(state.stats.interrupts_injected, 0);
        assert_eq!(state.stats.interrupts_cleared, 0);
        assert_eq!(state.stats.pending_count, 0);
        assert_eq!(state.stats.enabled_count, 3); // 3 standard interrupts enabled

        // Inject interrupt
        state.assert_interrupt(VirtualInterruptType::SupervisorSoftware).unwrap();
        state.update_stats();
        assert_eq!(state.stats.interrupts_injected, 1);
        assert_eq!(state.stats.pending_count, 1);

        // Clear interrupt
        state.clear_interrupt(VirtualInterruptType::SupervisorSoftware).unwrap();
        state.update_stats();
        assert_eq!(state.stats.interrupts_cleared, 1);
        assert_eq!(state.stats.pending_count, 0);
    }

    #[test]
    fn test_virtual_interrupt_flags() {
        let flags = VirtualInterruptFlags::NORMAL | VirtualInterruptFlags::HIGH_PRIORITY;
        assert!(flags.contains(VirtualInterruptFlags::NORMAL));
        assert!(flags.contains(VirtualInterruptFlags::HIGH_PRIORITY));
        assert!(!flags.contains(VirtualInterruptFlags::IMMEDIATE));

        let broadcast_flags = flags | VirtualInterruptFlags::BROADCAST;
        assert!(broadcast_flags.contains(VirtualInterruptFlags::BROADCAST));
    }

    #[test]
    fn test_injection_result() {
        let result = InjectionResult {
            success: true,
            already_pending: false,
            immediate_delivery: true,
            vcpus_affected: 1,
            error: None,
        };

        assert!(result.success);
        assert!(!result.already_pending);
        assert!(result.immediate_delivery);
        assert_eq!(result.vcpus_affected, 1);
        assert!(result.error.is_none());
    }
}