//! Virtual Machine (VM) Management
//!
//! This module handles the lifecycle and management of virtual machines.

use crate::{Result, Error};
use crate::config::{VmConfig, DeviceConfig, validate_vm_config};
use crate::core::vmm::{VmId, VmState, VcpuId};
use crate::core::mm::{VirtAddr, PhysAddr, AddressSpace, PAGE_SIZE, align_up};
use crate::core::sync::SpinLock;
use crate::utils::bitmap::Bitmap;
use alloc::vec::Vec;
use alloc::boxed::Box;
use core::ptr::NonNull;

/// Maximum number of VMs
pub const MAX_VMS: usize = 64;

/// VM structure
pub struct VirtualMachine {
    /// Unique VM ID
    id: VmId,
    /// VM configuration
    config: VmConfig,
    /// Current VM state
    state: VmState,
    /// Guest address space
    address_space: AddressSpace,
    /// Physical memory allocation bitmap
    memory_bitmap: Bitmap,
    /// Physical memory base address
    phys_memory_base: PhysAddr,
    /// Physical memory size
    phys_memory_size: u64,
    /// List of VCPUs
    vcpus: SpinLock<[Option<VcpuId>; 16]>, // Max 16 VCPUs per VM
    /// Number of active VCPUs
    vcpu_count: SpinLock<usize>,
    /// Mapped devices
    devices: SpinLock<Vec<DeviceConfig>>,
}

/// VM Manager
struct VmManager {
    /// Bitmap tracking allocated VM IDs
    vm_id_bitmap: Bitmap,
    /// Array of VM references
    vms: [Option<NonNull<VirtualMachine>>; MAX_VMS],
    /// Number of active VMs
    active_vms: usize,
}

impl VirtualMachine {
    /// Create a new virtual machine
    pub fn new(id: VmId, config: VmConfig) -> Result<Self> {
        // Validate configuration
        validate_vm_config(&config)?;

        // Create guest address space
        let address_space = AddressSpace::new(crate::core::mm::AddressSpaceType::User)
            .ok_or(Error::OutOfMemory)?;

        // Calculate required physical memory
        let aligned_memory_size = align_up(config.memory_size);
        let memory_bitmap_size = (aligned_memory_size / PAGE_SIZE + 63) / 64;

        // Allocate VM structure
        let vm = Self {
            id,
            config,
            state: VmState::Created,
            address_space,
            memory_bitmap: unsafe {
                // TODO: Allocate memory for bitmap
                Bitmap::new(core::ptr::null_mut(), memory_bitmap_size as usize)
            },
            phys_memory_base: 0, // TODO: Allocate physical memory
            phys_memory_size: aligned_memory_size,
            vcpus: SpinLock::new([None; 16]),
            vcpu_count: SpinLock::new(0),
            devices: SpinLock::new(Vec::new()),
        };

        // TODO: Initialize guest memory
        // TODO: Load kernel image
        // TODO: Setup initial state

        Ok(vm)
    }

    /// Get VM ID
    pub fn id(&self) -> VmId {
        self.id
    }

    /// Get VM state
    pub fn state(&self) -> VmState {
        self.state
    }

    /// Set VM state
    pub fn set_state(&mut self, state: VmState) {
        self.state = state;
    }

    /// Get VM configuration
    pub fn config(&self) -> &VmConfig {
        &self.config
    }

    /// Get guest address space
    pub fn address_space(&self) -> &AddressSpace {
        &self.address_space
    }

    /// Add a VCPU to this VM
    pub fn add_vcpu(&self, vcpu_id: VcpuId) -> Result<()> {
        let mut vcpus = self.vcpus.lock();
        let mut count = self.vcpu_count.lock();

        // Find a free slot
        for slot in vcpus.iter_mut() {
            if slot.is_none() {
                *slot = Some(vcpu_id);
                *count += 1;
                return Ok(());
            }
        }

        Err(Error::ResourceUnavailable) // No more VCPU slots
    }

    /// Remove a VCPU from this VM
    pub fn remove_vcpu(&self, vcpu_id: VcpuId) -> Result<()> {
        let mut vcpus = self.vcpus.lock();
        let mut count = self.vcpu_count.lock();

        for slot in vcpus.iter_mut() {
            if *slot == Some(vcpu_id) {
                *slot = None;
                *count = count.saturating_sub(1);
                return Ok(());
            }
        }

        Err(Error::NotFound)
    }

    /// Get number of VCPUs
    pub fn vcpu_count(&self) -> usize {
        *self.vcpu_count.lock()
    }

    /// Map a device into VM's address space
    pub fn map_device(&self, device: &DeviceConfig) -> Result<()> {
        let base_addr = device.base_address.ok_or(Error::InvalidArgument)?;
        let size = device.size.ok_or(Error::InvalidArgument)?;

        // Map device as MMIO
        self.address_space.map_range(
            base_addr,
            base_addr,
            size,
            crate::core::mm::PageFlags {
                present: true,
                writable: true,
                executable: false,
                user: true,
                write_through: false,
                cache_disable: true,
                accessed: false,
                dirty: false,
                global: false,
            },
        )?;

        // Add to device list
        self.devices.lock().push(device.clone());

        Ok(())
    }

    /// Unmap a device from VM's address space
    pub fn unmap_device(&self, device_name: &str) -> Result<()> {
        let mut devices = self.devices.lock();

        // Find the device
        let device_index = devices.iter().position(|d| d.name == device_name)
            .ok_or(Error::NotFound)?;

        let device = &devices[device_index];
        let base_addr = device.base_address.ok_or(Error::InvalidArgument)?;
        let size = device.size.ok_or(Error::InvalidArgument)?;

        // Unmap from address space
        self.address_space.unmap_page(base_addr)
            .map_err(|_| Error::InvalidState)?;

        // Remove from device list
        devices.remove(device_index);

        Ok(())
    }

    /// Get list of mapped devices
    pub fn get_devices(&self) -> Vec<DeviceConfig> {
        self.devices.lock().clone()
    }

    /// Allocate physical memory for guest
    pub fn allocate_guest_memory(&self, size: u64) -> Option<PhysAddr> {
        // TODO: Implement guest physical memory allocation
        None
    }

    /// Deallocate guest physical memory
    pub fn deallocate_guest_memory(&self, addr: PhysAddr, size: u64) -> bool {
        // TODO: Implement guest physical memory deallocation
        false
    }

    /// Translate guest physical to host physical address
    pub fn translate_guest_phys(&self, guest_phys: PhysAddr) -> Option<PhysAddr> {
        // Check if within guest physical memory range
        if guest_phys >= self.phys_memory_base &&
           guest_phys < self.phys_memory_base + self.phys_memory_size {
            Some(guest_phys - self.phys_memory_base + self.phys_memory_base)
        } else {
            // Check mapped devices
            let devices = self.devices.lock();
            for device in devices.iter() {
                if let Some(base) = device.base_address {
                    if let Some(size) = device.size {
                        if guest_phys >= base && guest_phys < base + size {
                            return Some(guest_phys); // Pass-through
                        }
                    }
                }
            }
            None
        }
    }
}

// VM Manager implementation
static mut VM_MANAGER: Option<VmManager> = None;
static VM_MANAGER_INIT: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

impl VmManager {
    /// Create a new VM manager
    const fn new() -> Self {
        Self {
            vm_id_bitmap: Bitmap::new(core::ptr::null_mut(), MAX_VMS),
            vms: [None; MAX_VMS],
            active_vms: 0,
        }
    }

    /// Initialize the VM manager
    fn init() -> Result<()> {
        unsafe {
            if VM_MANAGER.is_none() {
                // TODO: Allocate memory for VM ID bitmap
                let bitmap_data = [0u64; (MAX_VMS + 63) / 64];
                VM_MANAGER = Some(VmManager {
                    vm_id_bitmap: Bitmap::new(bitmap_data.as_ptr() as *mut u64, MAX_VMS),
                    vms: [None; MAX_VMS],
                    active_vms: 0,
                });
                VM_MANAGER_INIT.store(true, core::sync::atomic::Ordering::Release);
            }
        }
        Ok(())
    }

    /// Get the VM manager instance
    fn get() -> &'static mut VmManager {
        unsafe {
            VM_MANAGER.as_mut().unwrap()
        }
    }

    /// Allocate a VM ID
    fn allocate_vm_id(&mut self) -> Result<VmId> {
        if let Some(index) = self.vm_id_bitmap.find_and_set() {
            Ok(index as VmId)
        } else {
            Err(Error::ResourceUnavailable)
        }
    }

    /// Free a VM ID
    fn free_vm_id(&mut self, vm_id: VmId) -> Result<()> {
        if vm_id as usize >= MAX_VMS {
            return Err(Error::InvalidArgument);
        }

        if self.vm_id_bitmap.clear_bit(vm_id as usize) {
            Ok(())
        } else {
            Err(Error::InvalidArgument)
        }
    }
}

/// Initialize VM management
pub fn init() -> Result<()> {
    VmManager::init()
}

/// Create a new virtual machine
pub fn create_vm(config: &VmConfig) -> Result<VmId> {
    let manager = VmManager::get();
    let vm_id = manager.allocate_vm_id()?;

    // Create VM
    let vm = VirtualMachine::new(vm_id, config.clone())
        .map_err(|e| {
            manager.free_vm_id(vm_id).ok();
            e
        })?;

    // Store VM in manager
    let vm_ptr = NonNull::new(Box::into_raw(Box::new(vm)) as *mut VirtualMachine)
        .ok_or(Error::OutOfMemory)?;

    manager.vms[vm_id as usize] = Some(vm_ptr);
    manager.active_vms += 1;

    crate::info!("Created VM {} with name '{}'", vm_id, config.name);

    Ok(vm_id)
}

/// Destroy a virtual machine
pub fn destroy_vm(vm_id: VmId) -> Result<()> {
    let manager = VmManager::get();

    if vm_id as usize >= MAX_VMS {
        return Err(Error::InvalidArgument);
    }

    // Get VM reference
    let vm_ptr = manager.vms[vm_id as usize]
        .ok_or(Error::NotFound)?;

    let vm = unsafe { vm_ptr.as_ref() };

    // Check if VM can be destroyed
    match vm.state() {
        VmState::Running => return Err(Error::ResourceBusy),
        VmState::Paused => {}, // OK to destroy
        _ => {},
    }

    // Cleanup VCPUs
    // TODO: Destroy all VCPUs

    // Cleanup memory
    // TODO: Deallocate all guest memory

    // Free VM
    let _ = unsafe { Box::from_raw(vm_ptr.as_ptr()) };
    manager.vms[vm_id as usize] = None;
    manager.active_vms -= 1;

    // Free VM ID
    manager.free_vm_id(vm_id)?;

    crate::info!("Destroyed VM {}", vm_id);

    Ok(())
}

/// Start a virtual machine
pub fn start_vm(vm_id: VmId) -> Result<()> {
    let manager = VmManager::get();

    if vm_id as usize >= MAX_VMS {
        return Err(Error::InvalidArgument);
    }

    let vm_ptr = manager.vms[vm_id as usize]
        .ok_or(Error::NotFound)?;

    let vm = unsafe { vm_ptr.as_mut() };

    match vm.state() {
        VmState::Created | VmState::Paused => {
            vm.set_state(VmState::Running);
            // TODO: Start all VCPUs
            crate::info!("Started VM {}", vm_id);
            Ok(())
        }
        VmState::Running => Err(Error::ResourceBusy),
        _ => Err(Error::InvalidState),
    }
}

/// Stop a virtual machine
pub fn stop_vm(vm_id: VmId) -> Result<()> {
    let manager = VmManager::get();

    if vm_id as usize >= MAX_VMS {
        return Err(Error::InvalidArgument);
    }

    let vm_ptr = manager.vms[vm_id as usize]
        .ok_or(Error::NotFound)?;

    let vm = unsafe { vm_ptr.as_mut() };

    match vm.state() {
        VmState::Running => {
            vm.set_state(VmState::Paused);
            // TODO: Pause all VCPUs
            crate::info!("Stopped VM {}", vm_id);
            Ok(())
        }
        _ => Err(Error::InvalidState),
    }
}

/// Pause a virtual machine
pub fn pause_vm(vm_id: VmId) -> Result<()> {
    stop_vm(vm_id) // Same implementation for now
}

/// Resume a virtual machine
pub fn resume_vm(vm_id: VmId) -> Result<()> {
    start_vm(vm_id) // Same implementation for now
}

/// Reset a virtual machine
pub fn reset_vm(vm_id: VmId) -> Result<()> {
    let manager = VmManager::get();

    if vm_id as usize >= MAX_VMS {
        return Err(Error::InvalidArgument);
    }

    let vm_ptr = manager.vms[vm_id as usize]
        .ok_or(Error::NotFound)?;

    let vm = unsafe { vm_ptr.as_mut() };

    match vm.state() {
        VmState::Running | VmState::Paused => {
            vm.set_state(VmState::Resetting);
            // TODO: Reset all VCPUs
            // TODO: Reset device state
            vm.set_state(VmState::Created);
            crate::info!("Reset VM {}", vm_id);
            Ok(())
        }
        _ => Err(Error::InvalidState),
    }
}

/// Get VM state
pub fn get_vm_state(vm_id: VmId) -> Option<VmState> {
    let manager = VmManager::get();

    if vm_id as usize >= MAX_VMS {
        return None;
    }

    let vm_ptr = manager.vms[vm_id as usize]?;
    Some(unsafe { vm_ptr.as_ref().state() })
}

/// Map a device into a VM's address space
pub fn map_device(vm_id: VmId, config: &DeviceConfig) -> Result<()> {
    let manager = VmManager::get();

    if vm_id as usize >= MAX_VMS {
        return Err(Error::InvalidArgument);
    }

    let vm_ptr = manager.vms[vm_id as usize]
        .ok_or(Error::NotFound)?;

    let vm = unsafe { vm_ptr.as_ref() };
    vm.map_device(config)
}

/// Unmap a device from a VM's address space
pub fn unmap_device(vm_id: VmId, device_name: &str) -> Result<()> {
    let manager = VmManager::get();

    if vm_id as usize >= MAX_VMS {
        return Err(Error::InvalidArgument);
    }

    let vm_ptr = manager.vms[vm_id as usize]
        .ok_or(Error::NotFound)?;

    let vm = unsafe { vm_ptr.as_ref() };
    vm.unmap_device(device_name)
}

/// Get number of VMs
pub fn get_vm_count() -> usize {
    let manager = VmManager::get();
    manager.active_vms
}

/// Get number of running VMs
pub fn get_running_vm_count() -> usize {
    let manager = VmManager::get();
    let mut count = 0;

    for vm_ptr in manager.vms.iter().flatten() {
        let vm = unsafe { vm_ptr.as_ref() };
        if vm.state() == VmState::Running {
            count += 1;
        }
    }

    count
}