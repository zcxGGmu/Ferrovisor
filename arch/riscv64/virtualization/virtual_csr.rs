//! Virtual Supervisor CSR State Management
//!
//! This module provides comprehensive management of virtual supervisor CSRs (VS*):
//! - Complete VS* CSR state representation
//! - Efficient state save and restore operations
//! - State switching and validation logic
//! - Memory-safe CSR state management

use crate::arch::riscv64::cpu::csr::*;
use bitflags::bitflags;
use core::mem;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Virtual Supervisor CSR state
#[derive(Debug, Clone)]
pub struct VirtualCsrState {
    /// Virtual Supervisor Status Register
    pub vsstatus: VsstatusFlags,
    /// Virtual Supervisor Trap Vector Base Address Register
    pub vstvec: usize,
    /// Virtual Supervisor Scratch Register
    pub vsscratch: usize,
    /// Virtual Supervisor Exception Program Counter
    pub vsepc: usize,
    /// Virtual Supervisor Cause Register
    pub vscause: usize,
    /// Virtual Supervisor Trap Value Register
    pub vstval: usize,
    /// Virtual Supervisor Interrupt Enable Register
    pub vsie: VsieFlags,
    /// Virtual Supervisor Interrupt Pending Register
    pub vsip: VsipFlags,
    /// Virtual Supervisor Address Translation and Protection Register
    pub vsatp: usize,

    /// Guest physical address space ID
    pub vmid: u16,
    /// Last modified timestamp for tracking
    pub last_modified: u64,
    /// State validity flags
    pub validity: CsrValidityFlags,
}

/// CSR state validity flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CsrValidityFlags: u32 {
        const VSSTATUS_VALID = 1 << 0;
        const VSTVEC_VALID = 1 << 1;
        const VSIE_VALID = 1 << 2;
        const VSIP_VALID = 1 << 3;
        const VSATP_VALID = 1 << 4;
        const ALL_VALID = Self::VSSTATUS_VALID.bits() |
                        Self::VSTVEC_VALID.bits() |
                        Self::VSIE_VALID.bits() |
                        Self::VSIP_VALID.bits() |
                        Self::VSATP_VALID.bits();
    }
}

/// CSR state management statistics
#[derive(Debug, Default)]
pub struct CsrStats {
    /// Number of state saves
    pub saves: AtomicUsize,
    /// Number of state restores
    pub restores: AtomicUsize,
    /// Number of state switches
    pub switches: AtomicUsize,
    /// Number of validation failures
    pub validation_failures: AtomicUsize,
    /// Number of field updates
    pub field_updates: AtomicUsize,
}

impl VirtualCsrState {
    /// Create a new virtual CSR state with default values
    pub fn new(vmid: u16) -> Self {
        Self {
            vsstatus: VsstatusFlags::empty(),
            vstvec: 0,
            vsscratch: 0,
            vsepc: 0,
            vscause: 0,
            vstval: 0,
            vsie: VsieFlags::empty(),
            vsip: VsipFlags::empty(),
            vsatp: 0,
            vmid,
            last_modified: 0,
            validity: CsrValidityFlags::empty(),
        }
    }

    /// Save current VS* CSR state from hardware
    pub fn save_from_hw(vmid: u16) -> Result<Self, &'static str> {
        let state = Self {
            vsstatus: VSSTATUS::read(),
            vstvec: VSTVEC::read(),
            vsscratch: VSSCRATCH::read(),
            vsepc: VSEPC::read(),
            vscause: VSCAUSE::read(),
            vstval: VSTVAL::read(),
            vsie: VSIE::read(),
            vsip: VSIP::read(),
            vsatp: VSATP::read(),
            vmid,
            last_modified: Self::get_timestamp(),
            validity: CsrValidityFlags::ALL_VALID,
        };

        Ok(state)
    }

    /// Restore VS* CSR state to hardware
    pub fn restore_to_hw(&self) -> Result<(), &'static str> {
        // Validate state before restore
        self.validate()?;

        // Restore state to hardware CSRs
        VSSTATUS::write(self.vsstatus);
        VSTVEC::write(self.vstvec);
        VSSCRATCH::write(self.vsscratch);
        VSEPC::write(self.vsepc);
        VSCAUSE::write(self.vscause);
        VSTVAL::write(self.vstval);
        VSIE::write(self.vsie);
        VSIP::write(self.vsip);
        VSATP::write(self.vsatp);

        // Update statistics
        CsrStateManager::get_stats().restores.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Load CSR state from another VirtualCsrState
    pub fn load_from_state(&mut self, other: &VirtualCsrState) -> Result<(), &'static str> {
        other.validate()?;

        // Copy all fields
        self.vsstatus = other.vsstatus;
        self.vstvec = other.vstvec;
        self.vsscratch = other.vsscratch;
        self.vsepc = other.vsepc;
        self.vscause = other.vscause;
        self.vstval = other.vstval;
        self.vsie = other.vsie;
        self.vsip = other.vsip;
        self.vsatp = other.vsatp;
        self.vmid = other.vmid;
        self.validity = other.validity;
        self.last_modified = Self::get_timestamp();

        // Update statistics
        CsrStateManager::get_stats().switches.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Validate CSR state
    pub fn validate(&self) -> Result<(), &'static str> {
        // Check VSSTATUS validity
        if !self.validity.contains(CsrValidityFlags::VSSTATUS_VALID) {
            return Err("VSSTATUS is not valid");
        }

        // Check VSTVEC alignment (must be 4-byte aligned)
        if self.vstvec & 0x3 != 0 {
            return Err("VSTVEC is not properly aligned");
        }

        // Check VSATP validity if enabled
        if (self.vsstatus.contains(VsstatusFlags::TVM)) && self.vsatp != 0 {
            // VSATP should not be enabled if TVM is set
            return Err("VSATP invalid with TVM set");
        }

        // Check privilege level consistency
        let spp = if self.vsstatus.contains(VsstatusFlags::SPP) { 1 } else { 0 };
        if spp > 1 {
            return Err("Invalid SPP value in VSSTATUS");
        }

        Ok(())
    }

    /// Get VSSTATUS as raw value
    pub fn get_vsstatus_raw(&self) -> usize {
        self.vsstatus.bits()
    }

    /// Set VSSTATUS from raw value
    pub fn set_vsstatus_raw(&mut self, value: usize) {
        self.vsstatus = VsstatusFlags::from_bits_truncate(value);
        self.validity |= CsrValidityFlags::VSSTATUS_VALID;
        self.last_modified = Self::get_timestamp();
        CsrStateManager::get_stats().field_updates.fetch_add(1, Ordering::Relaxed);
    }

    /// Update specific VSSTATUS fields
    pub fn update_vsstatus<F>(&mut self, update_fn: F)
    where
        F: FnOnce(&mut VsstatusFlags)
    {
        update_fn(&mut self.vsstatus);
        self.validity |= CsrValidityFlags::VSSTATUS_VALID;
        self.last_modified = Self::get_timestamp();
        CsrStateManager::get_stats().field_updates.fetch_add(1, Ordering::Relaxed);
    }

    /// Get VSTVEC with mode bits
    pub fn get_vstvec_with_mode(&self) -> (usize, VstvecMode) {
        let mode = match self.vstvec & 0x3 {
            0 => VstvecMode::Direct,
            1 => VstvecMode::Vector,
            _ => VstvecMode::Reserved,
        };
        (self.vstvec & !0x3, mode)
    }

    /// Set VSTVEC with mode
    pub fn set_vstvec_with_mode(&mut self, base: usize, mode: VstvecMode) {
        let mode_bits = match mode {
            VstvecMode::Direct => 0,
            VstvecMode::Vector => 1,
            VstvecMode::Reserved => 3,
        };
        self.vstvec = (base & !0x3) | mode_bits;
        self.validity |= CsrValidityFlags::VSTVEC_VALID;
        self.last_modified = Self::get_timestamp();
        CsrStateManager::get_stats().field_updates.fetch_add(1, Ordering::Relaxed);
    }

    /// Check if interrupts are enabled
    pub fn are_interrupts_enabled(&self) -> bool {
        self.vsstatus.contains(VsstatusFlags::SIE)
    }

    /// Enable/disable interrupts
    pub fn set_interrupts_enabled(&mut self, enabled: bool) {
        if enabled {
            self.vsstatus |= VsstatusFlags::SIE;
        } else {
            self.vsstatus &= !VsstatusFlags::SIE;
        }
        self.last_modified = Self::get_timestamp();
    }

    /// Get current timestamp
    fn get_timestamp() -> u64 {
        // This would typically come from a time source
        // For now, use a simple counter
        use core::sync::atomic::{AtomicU64, Ordering};
        static TIMESTAMP_COUNTER: AtomicU64 = AtomicU64::new(0);
        TIMESTAMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    }

    /// Get age of this state (in arbitrary timestamp units)
    pub fn get_age(&self) -> u64 {
        Self::get_timestamp().saturating_sub(self.last_modified)
    }

    /// Check if state is stale
    pub fn is_stale(&self, max_age: u64) -> bool {
        self.get_age() > max_age
    }

    /// Reset all fields to default values
    pub fn reset(&mut self) {
        self.vsstatus = VsstatusFlags::empty();
        self.vstvec = 0;
        self.vsscratch = 0;
        self.vsepc = 0;
        self.vscause = 0;
        self.vstval = 0;
        self.vsie = VsieFlags::empty();
        self.vsip = VsipFlags::empty();
        self.vsatp = 0;
        self.validity = CsrValidityFlags::empty();
        self.last_modified = Self::get_timestamp();
    }

    /// Create a deep copy of the state
    pub fn deep_copy(&self) -> Self {
        Self {
            vsstatus: self.vsstatus,
            vstvec: self.vstvec,
            vsscratch: self.vsscratch,
            vsepc: self.vsepc,
            vscause: self.vscause,
            vstval: self.vstval,
            vsie: self.vsie,
            vsip: self.vsip,
            vsatp: self.vsatp,
            vmid: self.vmid,
            last_modified: self.last_modified,
            validity: self.validity,
        }
    }

    /// Get memory usage of this state
    pub fn memory_usage() -> usize {
        mem::size_of::<VirtualCsrState>()
    }
}

/// VSTVEC mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VstvecMode {
    Direct = 0,
    Vector = 1,
    Reserved = 3,
}

impl Default for VirtualCsrState {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Convert from legacy GuestCsrState
impl From<crate::arch::riscv64::virtualization::GuestCsrState> for VirtualCsrState {
    fn from(guest_csr: crate::arch::riscv64::virtualization::GuestCsrState) -> Self {
        Self {
            vsstatus: VsstatusFlags::from_bits_truncate(guest_csr.vsstatus),
            vstvec: guest_csr.vstvec,
            vsscratch: guest_csr.vsscratch,
            vsepc: guest_csr.vsepc,
            vscause: guest_csr.vscause,
            vstval: guest_csr.vstval,
            vsie: VsieFlags::from_bits_truncate(guest_csr.vsie),
            vsip: VsipFlags::from_bits_truncate(guest_csr.vsip),
            vsatp: guest_csr.vsatp,
            vmid: 0, // Default VMID
            last_modified: VirtualCsrState::get_timestamp(),
            validity: CsrValidityFlags::ALL_VALID,
        }
    }
}

/// CSR State Manager
pub struct CsrStateManager {
    /// Statistics
    stats: CsrStats,
    /// Maximum state age before considered stale
    max_state_age: u64,
}

impl CsrStateManager {
    /// Create new CSR state manager
    pub fn new(max_state_age: u64) -> Self {
        Self {
            stats: CsrStats::default(),
            max_state_age,
        }
    }

    /// Get global statistics
    pub fn get_stats(&self) -> &CsrStats {
        &self.stats
    }

    /// Save current hardware state
    pub fn save_hw_state(vmid: u16) -> Result<VirtualCsrState, &'static str> {
        let state = VirtualCsrState::save_from_hw(vmid)?;
        Self::get_stats().saves.fetch_add(1, Ordering::Relaxed);
        Ok(state)
    }

    /// Restore state to hardware
    pub fn restore_hw_state(state: &VirtualCsrState) -> Result<(), &'static str> {
        state.restore_to_hw()
    }

    /// Validate state freshness
    pub fn validate_state_freshness(state: &VirtualCsrState) -> Result<(), &'static str> {
        if state.is_stale(MAX_STATE_AGE) {
            return Err("CSR state is stale");
        }
        Ok(())
    }

    /// Optimize state switching
    pub fn optimize_state_switch(from: &VirtualCsrState, to: &VirtualCsrState) -> Vec<CsrUpdate> {
        let mut updates = Vec::new();

        // Compare fields and create minimal update list
        if from.vsstatus != to.vsstatus {
            updates.push(CsrUpdate::Vsstatus(to.vsstatus));
        }
        if from.vstvec != to.vstvec {
            updates.push(CsrUpdate::Vstvec(to.vstvec));
        }
        if from.vsie != to.vsie {
            updates.push(CsrUpdate::Vsie(to.vsie));
        }
        if from.vsip != to.vsip {
            updates.push(CsrUpdate::Vsip(to.vsip));
        }
        if from.vsatp != to.vsatp {
            updates.push(CsrUpdate::Vsatp(to.vsatp));
        }

        updates
    }
}

/// CSR update operation
#[derive(Debug, Clone)]
pub enum CsrUpdate {
    Vsstatus(VsstatusFlags),
    Vstvec(usize),
    Vsie(VsieFlags),
    Vsip(VsipFlags),
    Vsatp(usize),
}

impl CsrUpdate {
    /// Apply this update to hardware
    pub fn apply(&self) {
        match self {
            CsrUpdate::Vsstatus(value) => VSSTATUS::write(*value),
            CsrUpdate::Vstvec(value) => VSTVEC::write(*value),
            CsrUpdate::Vsie(value) => VSIE::write(*value),
            CsrUpdate::Vsip(value) => VSIP::write(*value),
            CsrUpdate::Vsatp(value) => VSATP::write(*value),
        }
    }
}

/// Maximum state age before considered stale
const MAX_STATE_AGE: u64 = 1000;

/// Global CSR state manager
static mut CSR_STATE_MANAGER: Option<CsrStateManager> = None;

/// Initialize CSR state manager
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing virtual CSR state manager");

    let manager = CsrStateManager::new(MAX_STATE_AGE);

    unsafe {
        CSR_STATE_MANAGER = Some(manager);
    }

    log::info!("Virtual CSR state manager initialized successfully");
    Ok(())
}

/// Get the global CSR state manager
pub fn get_manager() -> Option<&'static CsrStateManager> {
    unsafe { CSR_STATE_MANAGER.as_ref() }
}

/// Save current hardware CSR state
pub fn save_state(vmid: u16) -> Result<VirtualCsrState, &'static str> {
    if let Some(manager) = get_manager() {
        manager.save_hw_state(vmid)
    } else {
        VirtualCsrState::save_from_hw(vmid)
    }
}

/// Restore CSR state to hardware
pub fn restore_state(state: &VirtualCsrState) -> Result<(), &'static str> {
    if let Some(manager) = get_manager() {
        manager.restore_hw_state(state)
    } else {
        state.restore_to_hw()
    }
}

/// Perform optimized state switch
pub fn switch_state(from: &VirtualCsrState, to: &VirtualCsrState) -> Result<(), &'static str> {
    if let Some(manager) = get_manager() {
        let updates = manager.optimize_state_switch(from, to);

        // Apply updates in optimal order
        for update in updates {
            update.apply();
        }

        // Update statistics
        manager.get_stats().switches.fetch_add(1, Ordering::Relaxed);

        Ok(())
    } else {
        // Fallback to full restore
        to.restore_to_hw()
    }
}

/// Get CSR management statistics
pub fn get_stats() -> Option<CsrStatsSnapshot> {
    get_manager().map(|m| CsrStatsSnapshot {
        saves: m.stats.saves.load(Ordering::Relaxed),
        restores: m.stats.restores.load(Ordering::Relaxed),
        switches: m.stats.switches.load(Ordering::Relaxed),
        validation_failures: m.stats.validation_failures.load(Ordering::Relaxed),
        field_updates: m.stats.field_updates.load(Ordering::Relaxed),
    })
}

/// Snapshot of CSR statistics
#[derive(Debug, Clone, Copy)]
pub struct CsrStatsSnapshot {
    pub saves: usize,
    pub restores: usize,
    pub switches: usize,
    pub validation_failures: usize,
    pub field_updates: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_csr_state_creation() {
        let state = VirtualCsrState::new(1);
        assert_eq!(state.vmid, 1);
        assert_eq!(state.vsstatus, VsstatusFlags::empty());
        assert!(!state.validity.contains(CsrValidityFlags::VSSTATUS_VALID));
    }

    #[test]
    fn test_vsstatus_operations() {
        let mut state = VirtualCsrState::new(0);

        // Set VSSTATUS
        state.set_vsstatus_raw(0x80000001);
        assert_eq!(state.get_vsstatus_raw(), 0x80000001);
        assert!(state.validity.contains(CsrValidityFlags::VSSTATUS_VALID));

        // Update with function
        state.update_vsstatus(|vsstatus| {
            *vsstatus |= VsstatusFlags::SIE;
        });

        assert!(state.vsstatus.contains(VsstatusFlags::SIE));
    }

    #[test]
    fn test_vstvec_mode() {
        let mut state = VirtualCsrState::new(0);

        state.set_vstvec_with_mode(0x80000000, VstvecMode::Vector);
        let (base, mode) = state.get_vstvec_with_mode();

        assert_eq!(base, 0x80000000);
        assert_eq!(mode, VstvecMode::Vector);
        assert_eq!(state.vstvec, 0x80000001);
    }

    #[test]
    fn test_interrupt_management() {
        let mut state = VirtualCsrState::new(0);

        assert!(!state.are_interrupts_enabled());

        state.set_interrupts_enabled(true);
        assert!(state.are_interrupts_enabled());
        assert!(state.vsstatus.contains(VsstatusFlags::SIE));
    }

    #[test]
    fn test_state_validation() {
        let state = VirtualCsrState::new(0);

        // Invalid state should fail validation
        assert!(state.validate().is_err());

        // Valid VSTVEC alignment
        let mut valid_state = VirtualCsrState::new(0);
        valid_state.vstvec = 0x80000000; // 4-byte aligned
        valid_state.validity = CsrValidityFlags::ALL_VALID;

        assert!(valid_state.validate().is_ok());
    }

    #[test]
    fn test_state_copy() {
        let mut original = VirtualCsrState::new(1);
        original.vsstatus = VsstatusFlags::SIE;
        original.vstvec = 0x80000000;
        original.validity = CsrValidityFlags::VSSTATUS_VALID;

        let copy = original.deep_copy();

        assert_eq!(copy.vmid, original.vmid);
        assert_eq!(copy.vsstatus, original.vsstatus);
        assert_eq!(copy.vstvec, original.vstvec);
        assert_eq!(copy.validity, original.validity);
    }

    #[test]
    fn test_csr_update() {
        let vsstatus = VsstatusFlags::SIE | VsstatusFlags::SUM;
        let update = CsrUpdate::Vsstatus(vsstatus);

        // Just test that the enum can be created
        match update {
            CsrUpdate::Vsstatus(flags) => {
                assert!(flags.contains(VsstatusFlags::SIE));
            }
            _ => panic!("Wrong update type"),
        }
    }

    #[test]
    fn test_state_age() {
        let state = VirtualCsrState::new(0);

        // New state should have age 0
        assert_eq!(state.get_age(), 0);

        // Should not be stale immediately
        assert!(!state.is_stale(100));
    }

    #[test]
    fn test_memory_usage() {
        let usage = VirtualCsrState::memory_usage();
        assert!(usage > 0);
        assert!(usage == core::mem::size_of::<VirtualCsrState>());
    }
}