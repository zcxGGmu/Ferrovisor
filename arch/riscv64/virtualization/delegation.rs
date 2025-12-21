//! RISC-V Exception Delegation Module
//!
//! This module provides exception and interrupt delegation support for RISC-V virtualization:
//! - HEDELEG (Hypervisor Exception Delegation) management
//! - HIDELEG (Hypervisor Interrupt Delegation) management
//! - Exception delegation policy configuration
//! - Trap handling and routing logic

use crate::arch::riscv64::cpu::csr::*;
use crate::arch::riscv64::cpu::csr::{ExceptionCode, InterruptCause};
use bitflags::bitflags;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Exception delegation policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionDelegationPolicy {
    /// Do not delegate to guest supervisor
    None,
    /// Delegate all safe exceptions
    Safe,
    /// Delegate all exceptions including privileged ones
    All,
    /// Custom delegation mask
    Custom(Hedeleg),
}

/// Interrupt delegation policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptDelegationPolicy {
    /// Do not delegate to guest supervisor
    None,
    /// Delegate all supervisor interrupts
    All,
    /// Delegate only virtual interrupts
    Virtual,
    /// Custom delegation mask
    Custom(Hideleg),
}

/// Delegation configuration
#[derive(Debug, Clone)]
pub struct DelegationConfig {
    /// Exception delegation policy
    pub exception_policy: ExceptionDelegationPolicy,
    /// Interrupt delegation policy
    pub interrupt_policy: InterruptDelegationPolicy,
    /// Enable delegation for nested virtualization
    pub enable_nested_delegation: bool,
}

impl Default for DelegationConfig {
    fn default() -> Self {
        Self {
            exception_policy: ExceptionDelegationPolicy::Safe,
            interrupt_policy: InterruptDelegationPolicy::Virtual,
            enable_nested_delegation: false,
        }
    }
}

/// Exception delegation statistics
#[derive(Debug, Default)]
pub struct DelegationStats {
    /// Total exceptions handled
    pub total_exceptions: AtomicUsize,
    /// Exceptions delegated to guest
    pub delegated_exceptions: AtomicUsize,
    /// Exceptions handled by hypervisor
    pub hypervisor_exceptions: AtomicUsize,
    /// Total interrupts handled
    pub total_interrupts: AtomicUsize,
    /// Interrupts delegated to guest
    pub delegated_interrupts: AtomicUsize,
    /// Interrupts handled by hypervisor
    pub hypervisor_interrupts: AtomicUsize,
}

impl DelegationStats {
    /// Get snapshot of current statistics
    pub fn snapshot(&self) -> DelegationStatsSnapshot {
        DelegationStatsSnapshot {
            total_exceptions: self.total_exceptions.load(Ordering::Relaxed),
            delegated_exceptions: self.delegated_exceptions.load(Ordering::Relaxed),
            hypervisor_exceptions: self.hypervisor_exceptions.load(Ordering::Relaxed),
            total_interrupts: self.total_interrupts.load(Ordering::Relaxed),
            delegated_interrupts: self.delegated_interrupts.load(Ordering::Relaxed),
            hypervisor_interrupts: self.hypervisor_interrupts.load(Ordering::Relaxed),
        }
    }
}

/// Snapshot of delegation statistics
#[derive(Debug, Clone, Copy)]
pub struct DelegationStatsSnapshot {
    pub total_exceptions: usize,
    pub delegated_exceptions: usize,
    pub hypervisor_exceptions: usize,
    pub total_interrupts: usize,
    pub delegated_interrupts: usize,
    pub hypervisor_interrupts: usize,
}

/// Exception delegation manager
pub struct ExceptionDelegationManager {
    config: DelegationConfig,
    stats: DelegationStats,
}

impl ExceptionDelegationManager {
    /// Create a new exception delegation manager
    pub fn new(config: DelegationConfig) -> Self {
        Self {
            config,
            stats: DelegationStats::default(),
        }
    }

    /// Initialize delegation registers based on configuration
    pub fn init(&self) -> Result<(), &'static str> {
        log::debug!("Initializing exception delegation");

        // Configure HEDELEG
        self.configure_hedeleg()?;

        // Configure HIDELEG
        self.configure_hideleg()?;

        log::debug!("Exception delegation initialized successfully");
        Ok(())
    }

    /// Configure HEDELEG register
    fn configure_hedeleg(&self) -> Result<(), &'static str> {
        let hedeleg = match self.config.exception_policy {
            ExceptionDelegationPolicy::None => Hedeleg::empty(),
            ExceptionDelegationPolicy::Safe => {
                // Delegate safe exceptions that guest can handle
                Hedeleg::ILLEGAL_INSTRUCTION |
                Hedeleg::BREAKPOINT |
                Hedeleg::ECALL_FROM_UMODE |
                Hedeleg::ECALL_FROM_SMODE |
                Hedeleg::INSTRUCTION_PAGE_FAULT |
                Hedeleg::LOAD_PAGE_FAULT |
                Hedeleg::STORE_PAGE_FAULT
            }
            ExceptionDelegationPolicy::All => {
                // Delegate all standard exceptions
                Hedeleg::INSTRUCTION_MISALIGNED |
                Hedeleg::INSTRUCTION_ACCESS_FAULT |
                Hedeleg::ILLEGAL_INSTRUCTION |
                Hedeleg::BREAKPOINT |
                Hedeleg::LOAD_MISALIGNED |
                Hedeleg::LOAD_ACCESS_FAULT |
                Hedeleg::STORE_MISALIGNED |
                Hedeleg::STORE_ACCESS_FAULT |
                Hedeleg::ECALL_FROM_UMODE |
                Hedeleg::ECALL_FROM_SMODE |
                Hedeleg::INSTRUCTION_PAGE_FAULT |
                Hedeleg::LOAD_PAGE_FAULT |
                Hedeleg::STORE_PAGE_FAULT
            }
            ExceptionDelegationPolicy::Custom(mask) => mask,
        };

        HEDELEG::write(hedeleg);
        log::debug!("HEDELEG configured with: {:?}", hedeleg);

        Ok(())
    }

    /// Configure HIDELEG register
    fn configure_hideleg(&self) -> Result<(), &'static str> {
        let hideleg = match self.config.interrupt_policy {
            InterruptDelegationPolicy::None => Hideleg::empty(),
            InterruptDelegationPolicy::All => {
                // Delegate all supervisor interrupts
                Hideleg::SSIP |
                Hideleg::VSSIP |
                Hideleg::STIP |
                Hideleg::VSTIP |
                Hideleg::SEIP |
                Hideleg::VSEIP
            }
            InterruptDelegationPolicy::Virtual => {
                // Delegate only virtual interrupts
                Hideleg::VSSIP |
                Hideleg::VSTIP |
                Hideleg::VSEIP
            }
            InterruptDelegationPolicy::Custom(mask) => mask,
        };

        HIDELEG::write(hideleg);
        log::debug!("HIDELEG configured with: {:?}", hideleg);

        Ok(())
    }

    /// Handle exception and determine delegation
    pub fn handle_exception(&self, exception_code: ExceptionCode,
                           vcpu_id: Option<u16>) -> DelegationResult {
        self.stats.total_exceptions.fetch_add(1, Ordering::Relaxed);

        // Check if exception is delegated
        if HEDELEG::is_delegated(exception_code) {
            self.stats.delegated_exceptions.fetch_add(1, Ordering::Relaxed);

            let result = DelegationResult {
                should_delegate: true,
                to_guest: true,
                inject_virtual: false,
                delegated_code: exception_code,
                original_code: exception_code,
            };

            log::debug!("Exception {:?} delegated to guest vcpu={:?}",
                       exception_code, vcpu_id);
            result
        } else {
            self.stats.hypervisor_exceptions.fetch_add(1, Ordering::Relaxed);

            let result = DelegationResult {
                should_delegate: false,
                to_guest: false,
                inject_virtual: false,
                delegated_code: exception_code,
                original_code: exception_code,
            };

            log::debug!("Exception {:?} handled by hypervisor vcpu={:?}",
                       exception_code, vcpu_id);
            result
        }
    }

    /// Handle interrupt and determine delegation
    pub fn handle_interrupt(&self, interrupt: InterruptCause,
                           is_virtual: bool, vcpu_id: Option<u16>) -> DelegationResult {
        self.stats.total_interrupts.fetch_add(1, Ordering::Relaxed);

        // Check if interrupt is delegated
        if HIDELEG::is_delegated(interrupt) {
            self.stats.delegated_interrupts.fetch_add(1, Ordering::Relaxed);

            let result = DelegationResult {
                should_delegate: true,
                to_guest: true,
                inject_virtual: !is_virtual,
                delegated_code: ExceptionCode::ECallFromSMode, // Placeholder
                original_code: ExceptionCode::ECallFromSMode, // Placeholder
            };

            log::debug!("Interrupt {:?} delegated to guest vcpu={:?}",
                       interrupt, vcpu_id);
            result
        } else {
            self.stats.hypervisor_interrupts.fetch_add(1, Ordering::Relaxed);

            let result = DelegationResult {
                should_delegate: false,
                to_guest: false,
                inject_virtual: false,
                delegated_code: ExceptionCode::ECallFromSMode, // Placeholder
                original_code: ExceptionCode::ECallFromSMode, // Placeholder
            };

            log::debug!("Interrupt {:?} handled by hypervisor vcpu={:?}",
                       interrupt, vcpu_id);
            result
        }
    }

    /// Update delegation configuration
    pub fn update_config(&mut self, config: DelegationConfig) -> Result<(), &'static str> {
        self.config = config;
        self.init()
    }

    /// Get current configuration
    pub fn get_config(&self) -> &DelegationConfig {
        &self.config
    }

    /// Get delegation statistics
    pub fn get_stats(&self) -> DelegationStatsSnapshot {
        self.stats.snapshot()
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = DelegationStats::default();
    }

    /// Check if nested virtualization delegation is enabled
    pub fn is_nested_delegation_enabled(&self) -> bool {
        self.config.enable_nested_delegation
    }

    /// Enable/disable specific exception delegation
    pub fn set_exception_delegation(&self, exception: ExceptionCode,
                                   enable: bool) -> Result<(), &'static str> {
        let mut hedeleg = HEDELEG::read();
        let bit = match exception {
            ExceptionCode::InstructionMisaligned => Hedeleg::INSTRUCTION_MISALIGNED,
            ExceptionCode::InstructionAccessFault => Hedeleg::INSTRUCTION_ACCESS_FAULT,
            ExceptionCode::IllegalInstruction => Hedeleg::ILLEGAL_INSTRUCTION,
            ExceptionCode::Breakpoint => Hedeleg::BREAKPOINT,
            ExceptionCode::LoadMisaligned => Hedeleg::LOAD_MISALIGNED,
            ExceptionCode::LoadAccessFault => Hedeleg::LOAD_ACCESS_FAULT,
            ExceptionCode::StoreMisaligned => Hedeleg::STORE_MISALIGNED,
            ExceptionCode::StoreAccessFault => Hedeleg::STORE_ACCESS_FAULT,
            ExceptionCode::ECallFromUMode => Hedeleg::ECALL_FROM_UMODE,
            ExceptionCode::ECallFromSMode => Hedeleg::ECALL_FROM_SMODE,
            ExceptionCode::InstructionPageFault => Hedeleg::INSTRUCTION_PAGE_FAULT,
            ExceptionCode::LoadPageFault => Hedeleg::LOAD_PAGE_FAULT,
            ExceptionCode::StorePageFault => Hedeleg::STORE_PAGE_FAULT,
        };

        if enable {
            hedeleg |= bit;
        } else {
            hedeleg &= !bit;
        }

        HEDELEG::write(hedeleg);
        log::debug!("Exception {:?} delegation {}", exception,
                   if enable { "enabled" } else { "disabled" });

        Ok(())
    }

    /// Enable/disable specific interrupt delegation
    pub fn set_interrupt_delegation(&self, interrupt: InterruptCause,
                                   enable: bool) -> Result<(), &'static str> {
        let mut hideleg = HIDELEG::read();
        let bit = match interrupt {
            InterruptCause::SupervisorSoftware => Hideleg::SSIP,
            InterruptCause::SupervisorTimer => Hideleg::STIP,
            InterruptCause::SupervisorExternal => Hideleg::SEIP,
        };

        if enable {
            hideleg |= bit;
        } else {
            hideleg &= !bit;
        }

        HIDELEG::write(hideleg);
        log::debug!("Interrupt {:?} delegation {}", interrupt,
                   if enable { "enabled" } else { "disabled" });

        Ok(())
    }
}

/// Result of delegation decision
#[derive(Debug, Clone, Copy)]
pub struct DelegationResult {
    /// Whether delegation should occur
    pub should_delegate: bool,
    /// Target is guest (true) or hypervisor (false)
    pub to_guest: bool,
    /// Whether to inject as virtual interrupt/exception
    pub inject_virtual: bool,
    /// Delegated exception code
    pub delegated_code: ExceptionCode,
    /// Original exception code
    pub original_code: ExceptionCode,
}

impl Default for ExceptionDelegationManager {
    fn default() -> Self {
        Self::new(DelegationConfig::default())
    }
}

/// Global exception delegation manager
static mut EXCEPTION_DELEGATION: Option<ExceptionDelegationManager> = None;

/// Initialize global exception delegation
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing RISC-V exception delegation");

    let config = DelegationConfig::default();
    let manager = ExceptionDelegationManager::new(config);
    manager.init()?;

    // Store global manager
    unsafe {
        EXCEPTION_DELEGATION = Some(manager);
    }

    log::info!("RISC-V exception delegation initialized successfully");
    Ok(())
}

/// Get the global exception delegation manager
pub fn get_manager() -> Option<&'static ExceptionDelegationManager> {
    unsafe { EXCEPTION_DELEGATION.as_ref() }
}

/// Get mutable reference to global exception delegation manager
pub fn get_manager_mut() -> Option<&'static mut ExceptionDelegationManager> {
    unsafe { EXCEPTION_DELEGATION.as_mut() }
}

/// Handle exception using global delegation manager
pub fn handle_exception(exception_code: ExceptionCode,
                       vcpu_id: Option<u16>) -> DelegationResult {
    if let Some(manager) = get_manager() {
        manager.handle_exception(exception_code, vcpu_id)
    } else {
        // Fallback: no delegation
        DelegationResult {
            should_delegate: false,
            to_guest: false,
            inject_virtual: false,
            delegated_code: exception_code,
            original_code: exception_code,
        }
    }
}

/// Handle interrupt using global delegation manager
pub fn handle_interrupt(interrupt: InterruptCause,
                       is_virtual: bool, vcpu_id: Option<u16>) -> DelegationResult {
    if let Some(manager) = get_manager() {
        manager.handle_interrupt(interrupt, is_virtual, vcpu_id)
    } else {
        // Fallback: no delegation
        DelegationResult {
            should_delegate: false,
            to_guest: false,
            inject_virtual: false,
            delegated_code: ExceptionCode::ECallFromSMode,
            original_code: ExceptionCode::ECallFromSMode,
        }
    }
}

/// Configure delegation policy
pub fn configure_policy(policy: DelegationConfig) -> Result<(), &'static str> {
    if let Some(manager) = get_manager_mut() {
        manager.update_config(policy)
    } else {
        Err("Delegation manager not initialized")
    }
}

/// Get current delegation policy
pub fn get_current_policy() -> Option<DelegationConfig> {
    get_manager().map(|m| m.get_config().clone())
}

/// Enable/disable specific exception delegation
pub fn configure_exception_delegation(exception: ExceptionCode,
                                     enable: bool) -> Result<(), &'static str> {
    if let Some(manager) = get_manager() {
        manager.set_exception_delegation(exception, enable)
    } else {
        Err("Delegation manager not initialized")
    }
}

/// Enable/disable specific interrupt delegation
pub fn configure_interrupt_delegation(interrupt: InterruptCause,
                                    enable: bool) -> Result<(), &'static str> {
    if let Some(manager) = get_manager() {
        manager.set_interrupt_delegation(interrupt, enable)
    } else {
        Err("Delegation manager not initialized")
    }
}

/// Get delegation statistics
pub fn get_delegation_stats() -> Option<DelegationStatsSnapshot> {
    get_manager().map(|m| m.get_stats())
}

/// Reset delegation statistics
pub fn reset_delegation_stats() -> Result<(), &'static str> {
    if let Some(manager) = get_manager_mut() {
        manager.reset_stats();
        Ok(())
    } else {
        Err("Delegation manager not initialized")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delegation_config_default() {
        let config = DelegationConfig::default();
        assert!(matches!(config.exception_policy, ExceptionDelegationPolicy::Safe));
        assert!(matches!(config.interrupt_policy, InterruptDelegationPolicy::Virtual));
        assert!(!config.enable_nested_delegation);
    }

    #[test]
    fn test_exception_delegation_manager() {
        let config = DelegationConfig {
            exception_policy: ExceptionDelegationPolicy::Safe,
            interrupt_policy: InterruptDelegationPolicy::Virtual,
            enable_nested_delegation: false,
        };

        let manager = ExceptionDelegationManager::new(config);

        // Test initialization
        assert!(manager.init().is_ok());

        // Test exception handling
        let result = manager.handle_exception(
            ExceptionCode::IllegalInstruction,
            Some(1)
        );
        assert!(result.should_delegate);
        assert!(result.to_guest);

        // Test interrupt handling
        let int_result = manager.handle_interrupt(
            InterruptCause::SupervisorTimer,
            false,
            Some(1)
        );
        assert!(int_result.should_delegate);
    }

    #[test]
    fn test_delegation_stats() {
        let manager = ExceptionDelegationManager::new(DelegationConfig::default());

        // Handle some exceptions
        manager.handle_exception(ExceptionCode::IllegalInstruction, None);
        manager.handle_exception(ExceptionCode::Breakpoint, None);

        // Check stats
        let stats = manager.get_stats();
        assert_eq!(stats.total_exceptions, 2);
        assert_eq!(stats.delegated_exceptions, 2);
        assert_eq!(stats.hypervisor_exceptions, 0);
    }

    #[test]
    fn test_exception_delegation_policies() {
        // Test None policy
        let config = DelegationConfig {
            exception_policy: ExceptionDelegationPolicy::None,
            interrupt_policy: InterruptDelegationPolicy::None,
            enable_nested_delegation: false,
        };

        let manager = ExceptionDelegationManager::new(config);
        manager.init().ok();

        // Should not delegate exceptions
        let result = manager.handle_exception(ExceptionCode::IllegalInstruction, None);
        assert!(!result.should_delegate);

        // Test All policy
        let config_all = DelegationConfig {
            exception_policy: ExceptionDelegationPolicy::All,
            interrupt_policy: InterruptDelegationPolicy::All,
            enable_nested_delegation: false,
        };

        let manager_all = ExceptionDelegationManager::new(config_all);
        manager_all.init().ok();

        // Should delegate exceptions
        let result_all = manager_all.handle_exception(ExceptionCode::IllegalInstruction, None);
        assert!(result_all.should_delegate);
    }
}