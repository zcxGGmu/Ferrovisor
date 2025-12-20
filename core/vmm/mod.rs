//! Virtual Machine Manager (VMM)
//!
//! This module provides the core virtualization functionality,
//! including VM lifecycle management, VCPU creation, and
//! hypervisor entry/exit handling.

use crate::Result;
use crate::config::{VmConfig, DeviceConfig};
use crate::core::mm::{VirtAddr, PhysAddr, PAGE_SIZE};
use crate::core::sched::{Thread, ThreadId, Priority};

pub mod vm;
pub mod vcpu;
pub mod vmcs;

/// VM ID type
pub type VmId = u32;

/// VCPU ID type
pub type VcpuId = u32;

/// VM states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmState {
    /// VM is not created
    Uninitialized,
    /// VM is created but not started
    Created,
    /// VM is running
    Running,
    /// VM is paused
    Paused,
    /// VM is being reset
    Resetting,
    /// VM has terminated
    Terminated,
}

/// VM exit reasons
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmExitReason {
    /// Unknown/unhandled exit
    Unknown,
    /// Exception from guest
    Exception,
    /// External interrupt
    ExternalInterrupt,
    /// Timer interrupt
    TimerInterrupt,
    /// I/O instruction (x86) or MMIO access
    IoAccess,
    /// Memory mapped I/O access
    MmioAccess,
    /// Hypercall from guest
    Hypercall,
    /// HLT instruction
    Hlt,
    /// CPUID instruction (x86)
    Cpuid,
    /// MSR access (x86)
    MsrAccess,
    /// Debug breakpoint
    Debug,
    /// Shutdown
    Shutdown,
    /// Reset
    Reset,
    /// VM fail (invalid VMCS state)
    VmFail,
    /// VM exit valid
    VmExitValid,
}

/// VM exit information
#[derive(Debug, Clone)]
pub struct VmExitInfo {
    /// Exit reason
    pub reason: VmExitReason,
    /// Exit qualification (if applicable)
    pub qualification: u64,
    /// Guest RIP at exit
    pub guest_rip: u64,
    /// Exit instruction length
    pub instruction_length: u32,
    /// Exit instruction info
    pub instruction_info: u32,
    /// Architecture-specific data
    pub arch_data: VmExitArchData,
}

/// Architecture-specific VM exit data
#[repr(C)]
pub union VmExitArchData {
    /// ARM64 specific data
    pub arm64: VmExitArm64Data,
    /// RISC-V specific data
    pub riscv64: VmExitRiscv64Data,
    /// x86_64 specific data
    pub x86_64: VmExitX86_64Data,
}

/// ARM64-specific VM exit data
#[derive(Debug, Clone, Copy)]
pub struct VmExitArm64Data {
    /// ESR_EL2 value
    pub esr_el2: u64,
    /// FAR_EL2 value (fault address)
    pub far_el2: u64,
    /// HPFAR_EL2 value (IPA fault address)
    pub hpfar_el2: u64,
}

/// RISC-V-specific VM exit data
#[derive(Debug, Clone, Copy)]
pub struct VmExitRiscv64Data {
    /// Scause value
    pub scause: u64,
    /// Stval value
    pub stval: u64,
    /// HTVAL value (hypervisor trap value)
    pub htval: u64,
    /// HTINST value (hypervisor trap instruction)
    pub htinst: u64,
}

/// x86_64-specific VM exit data
#[derive(Debug, Clone, Copy)]
pub struct VmExitX86_64Data {
    /// Exit qualification field
    pub exit_qualification: u64,
    /// Guest linear address
    pub guest_linear_address: u64,
    /// Guest physical address
    pub guest_physical_address: u64,
}

/// Initialize the VMM subsystem
pub fn init() -> Result<()> {
    crate::info!("Initializing VMM subsystem");

    // Initialize VM management
    vm::init()?;

    // Initialize VCPU management
    vcpu::init()?;

    // Initialize VMCS management (if applicable)
    vmcs::init()?;

    crate::info!("VMM subsystem initialized");

    Ok(())
}

/// Create a new virtual machine
pub fn create_vm(config: &VmConfig) -> Result<VmId> {
    vm::create_vm(config)
}

/// Destroy a virtual machine
pub fn destroy_vm(vm_id: VmId) -> Result<()> {
    vm::destroy_vm(vm_id)
}

/// Start a virtual machine
pub fn start_vm(vm_id: VmId) -> Result<()> {
    vm::start_vm(vm_id)
}

/// Stop a virtual machine
pub fn stop_vm(vm_id: VmId) -> Result<()> {
    vm::stop_vm(vm_id)
}

/// Pause a virtual machine
pub fn pause_vm(vm_id: VmId) -> Result<()> {
    vm::pause_vm(vm_id)
}

/// Resume a virtual machine
pub fn resume_vm(vm_id: VmId) -> Result<()> {
    vm::resume_vm(vm_id)
}

/// Reset a virtual machine
pub fn reset_vm(vm_id: VmId) -> Result<()> {
    vm::reset_vm(vm_id)
}

/// Get VM state
pub fn get_vm_state(vm_id: VmId) -> Option<VmState> {
    vm::get_vm_state(vm_id)
}

/// Create a VCPU for a VM
pub fn create_vcpu(vm_id: VmId, vcpu_id: VcpuId) -> Result<()> {
    vcpu::create_vcpu(vm_id, vcpu_id)
}

/// Destroy a VCPU
pub fn destroy_vcpu(vm_id: VmId, vcpu_id: VcpuId) -> Result<()> {
    vcpu::destroy_vcpu(vm_id, vcpu_id)
}

/// Run a VCPU
pub fn run_vcpu(vm_id: VmId, vcpu_id: VcpuId) -> Result<VmExitInfo> {
    vcpu::run_vcpu(vm_id, vcpu_id)
}

/// Inject an interrupt into a VCPU
pub fn inject_interrupt(vm_id: VmId, vcpu_id: VcpuId, vector: u32) -> Result<()> {
    vcpu::inject_interrupt(vm_id, vcpu_id, vector)
}

/// Inject an exception into a VCPU
pub fn inject_exception(vm_id: VmId, vcpu_id: VcpuId, exception: u32, error_code: u32) -> Result<()> {
    vcpu::inject_exception(vm_id, vcpu_id, exception, error_code)
}

/// Get VCPU registers
pub fn get_vcpu_regs(vm_id: VmId, vcpu_id: VcpuId) -> Option<VcpuRegisters> {
    vcpu::get_vcpu_regs(vm_id, vcpu_id)
}

/// Set VCPU registers
pub fn set_vcpu_regs(vm_id: VmId, vcpu_id: VcpuId, regs: &VcpuRegisters) -> Result<()> {
    vcpu::set_vcpu_regs(vm_id, vcpu_id, regs)
}

/// Map a device into a VM's address space
pub fn map_device(vm_id: VmId, config: &DeviceConfig) -> Result<()> {
    vm::map_device(vm_id, config)
}

/// Unmap a device from a VM's address space
pub fn unmap_device(vm_id: VmId, device_name: &str) -> Result<()> {
    vm::unmap_device(vm_id, device_name)
}

/// Get VMM statistics
pub fn get_vmm_stats() -> VmmStats {
    VmmStats {
        total_vms: vm::get_vm_count(),
        running_vms: vm::get_running_vm_count(),
        total_vcpus: vcpu::get_vcpu_count(),
        running_vcpus: vcpu::get_running_vcpu_count(),
    }
}

/// VMM statistics
#[derive(Debug, Clone, Copy)]
pub struct VmmStats {
    /// Total number of VMs
    pub total_vms: usize,
    /// Number of running VMs
    pub running_vms: usize,
    /// Total number of VCPUs
    pub total_vcpus: usize,
    /// Number of running VCPUs
    pub running_vcpus: usize,
}

/// VCPU register state
#[derive(Debug, Clone)]
pub struct VcpuRegisters {
    /// General purpose registers
    pub gpr: [u64; 32],
    /// Program counter
    pub pc: u64,
    /// Stack pointer
    pub sp: u64,
    /// Processor state register
    pub psr: u64,
    /// Architecture-specific registers
    pub arch_regs: VcpuArchRegisters,
}

/// Architecture-specific VCPU registers
#[repr(C)]
pub union VcpuArchRegisters {
    /// ARM64 specific registers
    pub arm64: VcpuArm64Registers,
    /// RISC-V specific registers
    pub riscv64: VcpuRiscv64Registers,
    /// x86_64 specific registers
    pub x86_64: VcpuX86_64Registers,
}

/// ARM64-specific VCPU registers
#[derive(Debug, Clone)]
pub struct VcpuArm64Registers {
    /// System registers
    pub sctlr_el1: u64,
    pub tcr_el1: u64,
    pub ttbr0_el1: u64,
    pub ttbr1_el1: u64,
    pub mair_el1: u64,
    pub amair_el1: u64,
    pub vbar_el1: u64,
    pub cntvoff_el2: u64,
    pub cntkctl_el1: u64,
    /// Floating point registers (optional)
    pub fp_simd: Option<[u128; 32]>,
}

/// RISC-V-specific VCPU registers
#[derive(Debug, Clone)]
pub struct VcpuRiscv64Registers {
    /// System registers
    pub satp: u64,
    pub sstatus: u64,
    pub sie: u64,
    pub stvec: u64,
    pub sscratch: u64,
    pub sepc: u64,
    pub scause: u64,
    pub stval: u64,
    pub sip: u64,
    /// Floating point registers (optional)
    pub fp_regs: Option<[u64; 32]>,
}

/// x86_64-specific VCPU registers
#[derive(Debug, Clone)]
pub struct VcpuX86_64Registers {
    /// Control registers
    pub cr0: u64,
    pub cr2: u64,
    pub cr3: u64,
    pub cr4: u64,
    pub cr8: u64,
    /// Debug registers
    pub dr0: u64,
    pub dr1: u64,
    pub dr2: u64,
    pub dr3: u64,
    pub dr6: u64,
    pub dr7: u64,
    /// MSRs
    pub efer: u64,
    pub star: u64,
    pub lstar: u64,
    pub cstar: u64,
    pub sfmask: u64,
    pub fs_base: u64,
    pub gs_base: u64,
    pub kernel_gs_base: u64,
    /// Segment registers
    pub es: SegmentRegister,
    pub cs: SegmentRegister,
    pub ss: SegmentRegister,
    pub ds: SegmentRegister,
    pub fs: SegmentRegister,
    pub gs: SegmentRegister,
    pub tr: SegmentRegister,
    pub ldtr: SegmentRegister,
}

/// Segment register
#[derive(Debug, Clone, Copy)]
pub struct SegmentRegister {
    /// Selector
    pub selector: u16,
    /// Base address
    pub base: u64,
    /// Limit
    pub limit: u32,
    /// Access rights
    pub access: u32,
    /// Flags
    pub flags: u32,
}