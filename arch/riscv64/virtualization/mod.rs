//! RISC-V Virtualization Module
//!
//! This module provides virtualization support including:
//! - H extension implementation
//! - VCPU management
//! - Virtual memory handling
//! - Virtual device emulation

pub mod hextension;
pub mod vcpu;
pub mod vm;
pub mod devices;
pub mod delegation;

pub use hextension::*;
pub use vcpu::*;
pub use vm::*;
pub use delegation::*;

use crate::arch::riscv64::*;

/// Global H extension manager
static mut H_EXTENSION: Option<HExtensionManager> = None;

/// Virtual machine manager
static mut VM_MANAGER: Option<VmManager> = None;

/// Initialize virtualization subsystem
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing RISC-V virtualization subsystem");

    // Check if H extension is available
    if !HExtensionManager::is_available() {
        log::warn!("H extension not available, virtualization disabled");
        return Ok(());
    }

    // Initialize H extension
    let config = HExtensionConfig::default();
    let mut h_ext = HExtensionManager::new(config);
    h_ext.init()?;

    // Store global H extension manager
    unsafe {
        H_EXTENSION = Some(h_ext);
    }

    // Initialize exception delegation
    delegation::init()?;

    // Initialize VM manager
    let vm_manager = VmManager::new();
    unsafe {
        VM_MANAGER = Some(vm_manager);
    }

    log::info!("RISC-V virtualization subsystem initialized successfully");
    Ok(())
}

/// Get the global H extension manager
pub fn get_h_extension() -> Option<&'static HExtensionManager> {
    unsafe { H_EXTENSION.as_ref() }
}

/// Get mutable reference to global H extension manager
pub fn get_h_extension_mut() -> Option<&'static mut HExtensionManager> {
    unsafe { H_EXTENSION.as_mut() }
}

/// Get the global VM manager
pub fn get_vm_manager() -> Option<&'static VmManager> {
    unsafe { VM_MANAGER.as_ref() }
}

/// Get mutable reference to global VM manager
pub fn get_vm_manager_mut() -> Option<&'static mut VmManager> {
    unsafe { VM_MANAGER.as_mut() }
}

/// Check if H extension is supported
pub fn has_h_extension() -> bool {
    HExtensionManager::is_available()
}

/// Enter virtualization mode with a VCPU
pub fn enter_virtualization(vcpu: &Vcpu) -> Result<(), &'static str> {
    let h_ext = get_h_extension().ok_or("H extension not initialized")?;

    // Save current host state
    // This would be done in assembly

    // Configure stage-2 translation if enabled
    // This would be handled by the VM

    // Enter guest mode
    h_ext.enter_virtualization(&vcpu.guest_csr)?;

    // This would continue with assembly code to restore guest state and execute

    Ok(())
}

/// Exit virtualization mode
pub fn exit_virtualization() -> Result<HypervisorTrapInfo, &'static str> {
    let h_ext = get_h_extension().ok_or("H extension not initialized")?;

    // Save guest state
    // This would be done in assembly

    // Exit to hypervisor
    let trap_info = h_ext.exit_virtualization()?;

    // Handle the trap
    handle_hypervisor_trap(&trap_info)?;

    Ok(trap_info)
}

/// Handle hypervisor trap
fn handle_hypervisor_trap(trap_info: &HypervisorTrapInfo) -> Result<(), &'static str> {
    log::debug!("Handling hypervisor trap: cause={:#x}, tval={:#x}",
                trap_info.cause, trap_info.tval);

    // Determine if this is an interrupt or exception
    let is_interrupt = (trap_info.cause & 0x80000000) != 0;

    if is_interrupt {
        // Handle virtual interrupt with delegation
        let interrupt_cause = trap_info.cause & 0x7FFFFFFF;
        let interrupt = match interrupt_cause {
            1 => InterruptCause::SupervisorSoftware,
            5 => InterruptCause::SupervisorTimer,
            9 => InterruptCause::SupervisorExternal,
            _ => {
                log::warn!("Unknown interrupt cause: {}", interrupt_cause);
                return Err("Unknown interrupt cause");
            }
        };

        let delegation_result = delegation::handle_interrupt(
            interrupt,
            false, // This is a real interrupt, not virtual
            None   // VCPU ID would be available from context
        );

        if delegation_result.should_delegate && delegation_result.to_guest {
            return handle_virtual_interrupt(trap_info);
        } else {
            return handle_hypervisor_interrupt(trap_info, interrupt);
        }
    } else {
        // Handle guest exception with delegation
        let exception_code = ExceptionCode::try_from(trap_info.cause)
            .map_err(|_| "Invalid exception code")?;

        let delegation_result = delegation::handle_exception(
            exception_code,
            None // VCPU ID would be available from context
        );

        if delegation_result.should_delegate && delegation_result.to_guest {
            return handle_guest_exception(trap_info);
        } else {
            return handle_hypervisor_exception(trap_info, exception_code);
        }
    }
}

/// Handle virtual interrupt
fn handle_virtual_interrupt(trap_info: &HypervisorTrapInfo) -> Result<(), &'static str> {
    let interrupt_cause = trap_info.cause & 0x7FFFFFFF;

    match interrupt_cause {
        5 => {
            // Supervisor timer interrupt
            log::debug!("Virtual supervisor timer interrupt");
            // Inject virtual timer to current VCPU
        }
        9 => {
            // Supervisor external interrupt
            log::debug!("Virtual supervisor external interrupt");
            // Handle virtual external interrupt
        }
        _ => {
            log::warn!("Unknown virtual interrupt: {}", interrupt_cause);
        }
    }

    Ok(())
}

/// Handle guest exception
fn handle_guest_exception(trap_info: &HypervisorTrapInfo) -> Result<(), &'static str> {
    let exception_code = trap_info.cause;

    match exception_code {
        0 => {
            // Instruction address misaligned
            log::debug!("Guest instruction address misaligned");
            return handle_instruction_misaligned(trap_info);
        }
        2 => {
            // Illegal instruction
            log::debug!("Guest illegal instruction");
            return handle_illegal_instruction(trap_info);
        }
        8 | 9 => {
            // Environment call
            log::debug!("Guest environment call");
            return handle_ecall(trap_info);
        }
        12 | 13 | 15 => {
            // Page fault
            log::debug!("Guest page fault");
            return handle_page_fault(trap_info);
        }
        _ => {
            log::warn!("Unhandled guest exception: {}", exception_code);
        }
    }

    Ok(())
}

/// Handle hypervisor exception (when delegation is disabled)
fn handle_hypervisor_exception(trap_info: &HypervisorTrapInfo,
                               exception_code: ExceptionCode) -> Result<(), &'static str> {
    log::debug!("Handling hypervisor exception: {:?}", exception_code);

    match exception_code {
        ExceptionCode::IllegalInstruction => {
            log::debug!("Hypervisor illegal instruction at {:#x}", trap_info.sepc);
            // Handle hypervisor-specific illegal instructions
            match trap_info.htinst & 0xFFFF {
                0x102 => {
                    // HFENCE.VVMA
                    log::debug!("HFENCE.VVMA instruction");
                    // Handle hypervisor fence
                    Ok(())
                }
                _ => {
                    log::error!("Unknown hypervisor illegal instruction: {:#x}", trap_info.htinst);
                    Err("Unknown hypervisor illegal instruction")
                }
            }
        }
        ExceptionCode::InstructionPageFault |
        ExceptionCode::LoadPageFault |
        ExceptionCode::StorePageFault => {
            log::debug!("Hypervisor page fault at {:#x}", trap_info.tval);
            // Handle hypervisor page faults (e.g., accessing guest memory)
            Ok(())
        }
        _ => {
            log::warn!("Unhandled hypervisor exception: {:?}", exception_code);
            Err("Unhandled hypervisor exception")
        }
    }
}

/// Handle hypervisor interrupt (when delegation is disabled)
fn handle_hypervisor_interrupt(trap_info: &HypervisorTrapInfo,
                                interrupt: InterruptCause) -> Result<(), &'static str> {
    log::debug!("Handling hypervisor interrupt: {:?}", interrupt);

    match interrupt {
        InterruptCause::SupervisorTimer => {
            log::debug!("Hypervisor timer interrupt");
            // Handle hypervisor timer (e.g., scheduling)
            Ok(())
        }
        InterruptCause::SupervisorExternal => {
            log::debug!("Hypervisor external interrupt");
            // Handle hypervisor external interrupts (e.g., IPI)
            Ok(())
        }
        InterruptCause::SupervisorSoftware => {
            log::debug!("Hypervisor software interrupt");
            // Handle hypervisor software interrupts (e.g., IPI)
            Ok(())
        }
    }
}

/// Handle instruction address misaligned
fn handle_instruction_misaligned(_trap_info: &HypervisorTrapInfo) -> Result<(), &'static str> {
    // For now, just inject exception to guest
    // In a real implementation, we might handle this differently
    Err("Instruction address misaligned not handled")
}

/// Handle illegal instruction
fn handle_illegal_instruction(trap_info: &HypervisorTrapInfo) -> Result<(), &'static str> {
    // Check if this is a hypervisor instruction that should be trapped
    match trap_info.htinst & 0xFFFF {
        0x102 => {
            // HFENCE.VVMA
            log::debug!("Guest executed HFENCE.VVMA");
            // Handle virtual fence
            Ok(())
        }
        0x120 => {
            // HLVX.WU
            log::debug!("Guest executed HLVX.WU");
            // Handle virtual load
            Ok(())
        }
        _ => {
            // Unknown illegal instruction
            log::warn!("Guest illegal instruction: {:#x}", trap_info.htinst);
            Err("Illegal instruction")
        }
    }
}

/// Handle environment call (ecall)
fn handle_ecall(trap_info: &HypervisorTrapInfo) -> Result<(), &'static str> {
    // Check privilege level from guest status
    let guest_privilege = (trap_info.guest_csr.vsstatus >> 8) & 0x3;

    match guest_privilege {
        0 => {
            // User-mode ecall - forward to guest OS
            log::debug!("Guest user-mode ecall");
            inject_guest_ecall(8)?; // User ecall
        }
        1 => {
            // Supervisor-mode ecall - hypervisor call
            log::debug!("Guest supervisor ecall (hypercall)");
            handle_hypercall(trap_info)?;
        }
        _ => {
            log::warn!("Unexpected ecall privilege level: {}", guest_privilege);
            return Err("Unexpected ecall privilege level");
        }
    }

    Ok(())
}

/// Handle page fault
fn handle_page_fault(trap_info: &HypervisorTrapInfo) -> Result<(), &'static str> {
    log::debug!("Guest page fault at {:#x}", trap_info.tval);

    // Check if this is a valid guest physical address
    // and handle stage-2 translation if needed

    // For now, just forward to guest
    match trap_info.cause {
        12 => inject_guest_ecall(12)?, // Instruction page fault
        13 => inject_guest_ecall(13)?, // Load page fault
        15 => inject_guest_ecall(15)?, // Store page fault
        _ => return Err("Invalid page fault type"),
    }

    Ok(())
}

/// Inject ecall to guest
fn inject_guest_ecall(exception_code: usize) -> Result<(), &'static str> {
    // Set guest cause and tval
    crate::arch::riscv64::cpu::csr::write_csr!(hcsr::VSCAUSE, exception_code);
    crate::arch::riscv64::cpu::csr::write_csr!(hcsr::VSTVAL, 0);

    // Set guest PC to trap handler
    let vstvec = crate::arch::riscv64::cpu::csr::read_csr!(hcsr::VSTVEC);
    crate::arch::riscv64::cpu::csr::write_csr!(hcsr::VSEPC, vstvec);

    Ok(())
}

/// Handle hypercall
fn handle_hypercall(trap_info: &HypervisorTrapInfo) -> Result<(), &'static str> {
    // Get hypercall number from a register (e.g., a7)
    let hypercall_num = 0; // This would be read from VCPU state

    match hypercall_num {
        0 => {
            // SBI call - forward to SBI implementation
            log::debug!("Guest SBI call");
            handle_sbi_call(trap_info)
        }
        1 => {
            // Hypervisor shutdown
            log::info!("Guest requested shutdown");
            Ok(())
        }
        _ => {
            log::warn!("Unknown hypercall: {}", hypercall_num);
            Err("Unknown hypercall")
        }
    }
}

/// Handle SBI call from guest
fn handle_sbi_call(_trap_info: &HypervisorTrapInfo) -> Result<(), &'static str> {
    // Implement virtual SBI interface
    // This would handle various SBI extensions
    log::debug!("Handling virtual SBI call");
    Ok(())
}

/// Virtual Machine Manager
pub struct VmManager {
    /// List of VMs
    vms: Vec<VirtualMachine>,
    /// Next VM ID to allocate
    next_vm_id: u16,
}

impl VmManager {
    /// Create a new VM manager
    pub fn new() -> Self {
        Self {
            vms: Vec::new(),
            next_vm_id: 1,
        }
    }

    /// Create a new VM
    pub fn create_vm(
        &mut self,
        name: String,
        config: VmConfig,
        flags: VmFlags,
    ) -> Result<&mut VirtualMachine, &'static str> {
        if self.next_vm_id >= 1024 {
            return Err("Maximum VMs reached");
        }

        let vm_id = self.next_vm_id;
        self.next_vm_id += 1;

        let mut vm = VirtualMachine::new(vm_id, name, config, flags)?;
        vm.init()?;

        self.vms.push(vm);
        Ok(&mut self.vms[self.vms.len() - 1])
    }

    /// Get a VM by ID
    pub fn get_vm(&mut self, vm_id: u16) -> Option<&mut VirtualMachine> {
        self.vms.iter_mut().find(|vm| vm.id == vm_id)
    }

    /// Get all VMs
    pub fn get_vms(&self) -> &[VirtualMachine] {
        &self.vms
    }

    /// Get mutable list of all VMs
    pub fn get_vms_mut(&mut self) -> &mut [VirtualMachine] {
        &mut self.vms
    }

    /// Destroy a VM
    pub fn destroy_vm(&mut self, vm_id: u16) -> Result<(), &'static str> {
        let index = self.vms.iter().position(|vm| vm.id == vm_id)
            .ok_or("VM not found")?;

        let vm = &mut self.vms[index];

        // Stop the VM if it's running
        if vm.state == VmState::Running {
            vm.stop()?;
        }

        // Free VMID
        if let Some(h_ext) = get_h_extension_mut() {
            h_ext.free_vmid(vm.vmid);
        }

        self.vms.remove(index);
        log::info!("VM {} destroyed", vm_id);
        Ok(())
    }

    /// Get total number of VMs
    pub fn vm_count(&self) -> usize {
        self.vms.len()
    }

    /// Get running VMs
    pub fn get_running_vms(&self) -> Vec<&VirtualMachine> {
        self.vms.iter().filter(|vm| vm.state == VmState::Running).collect()
    }
}

impl Default for VmManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_h_extension_detection() {
        let has_h = has_h_extension();
        println!("H extension supported: {}", has_h);
    }

    #[test]
    fn test_h_extension_config() {
        let config = HExtensionConfig::default();
        assert!(config.enable_two_stage_translation);
        assert!(config.enable_virtual_interrupts);
        assert_eq!(config.max_vmid, 4095);
    }

    #[test]
    fn test_vmid_allocator() {
        let mut allocator = VmidAllocator::new(10);

        let vmid1 = allocator.allocate().unwrap();
        let vmid2 = allocator.allocate().unwrap();

        assert_ne!(vmid1, vmid2);
        assert_eq!(vmid1, 1);
        assert_eq!(vmid2, 2);

        allocator.free(vmid1);
        let vmid3 = allocator.allocate().unwrap();
        assert_eq!(vmid3, vmid1);
    }

    #[test]
    fn test_vm_manager() {
        let mut manager = VmManager::new();

        let config = VmConfig::default();
        let vm = manager.create_vm(
            "test_vm".to_string(),
            config,
            VmFlags::VIRTUAL_INTERRUPTS,
        ).unwrap();

        assert_eq!(vm.name, "test_vm");
        assert_eq!(manager.vm_count(), 1);

        let retrieved = manager.get_vm(vm.id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, vm.id);
    }
}