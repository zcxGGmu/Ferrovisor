//! CPU utilities
//!
//! This module provides CPU-related utility functions used throughout the hypervisor.

/// Get the current CPU ID
pub fn get_current_cpu_id() -> Option<u32> {
    #[cfg(target_arch = "riscv64")]
    {
        // On RISC-V, we can use the hart ID from mhartid CSR
        use riscv::register::mhartid;
        Some(mhartid::read() as u32)
    }

    #[cfg(target_arch = "aarch64")]
    {
        // On ARM64, we can use MPIDR_EL1 to get CPU ID
        let mut mpidr: u64;
        unsafe {
            core::arch::asm!("mrs {}, mpidr_el1", out(reg) mpidr);
        }
        Some((mpidr & 0xFF) as u32)
    }

    #[cfg(target_arch = "x86_64")]
    {
        // On x86_64, we can use APIC ID (simplified)
        Some(0) // TODO: Implement proper APIC ID reading
    }

    #[cfg(not(any(target_arch = "riscv64", target_arch = "aarch64", target_arch = "x86_64")))]
    {
        Some(0) // Default fallback
    }
}

/// Get the total number of CPUs
pub fn get_cpu_count() -> Option<u32> {
    // For now, return a reasonable default
    // In a real implementation, this would query the hardware/DTB/ACPI
    #[cfg(target_arch = "riscv64")]
    {
        // RISC-V systems typically have up to 8 harts in simple configurations
        Some(8)
    }

    #[cfg(target_arch = "aarch64")]
    {
        // ARM64 systems can have varying core counts
        Some(4)
    }

    #[cfg(target_arch = "x86_64")]
    {
        // x86_64 systems typically have multiple cores
        Some(4)
    }

    #[cfg(not(any(target_arch = "riscv64", target_arch = "aarch64", target_arch = "x86_64")))]
    {
        Some(1) // Single core fallback
    }
}

/// Check if we're in the context of a specific CPU
pub fn is_cpu(cpu_id: u32) -> bool {
    get_current_cpu_id() == Some(cpu_id)
}

/// Get the current CPU's frequency in Hz
pub fn get_cpu_frequency() -> Option<u64> {
    // This would typically come from device tree or ACPI
    // For now, return reasonable defaults
    #[cfg(target_arch = "riscv64")]
    {
        Some(1_000_000_000) // 1 GHz default for RISC-V
    }

    #[cfg(target_arch = "aarch64")]
    {
        Some(2_000_000_000) // 2 GHz default for ARM64
    }

    #[cfg(target_arch = "x86_64")]
    {
        Some(3_000_000_000) // 3 GHz default for x86_64
    }

    #[cfg(not(any(target_arch = "riscv64", target_arch = "aarch64", target_arch = "x86_64")))]
    {
        Some(1_000_000_000) // 1 GHz fallback
    }
}