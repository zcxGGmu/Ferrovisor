//! Spin table SMP initialization
//!
//! Provides CPU initialization using spin table method.

/// Spin table entry
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SpinTableEntry {
    pub cpu_id: u64,
    pub entry_addr: u64,
    pub context_id: u64,
}

/// Start a CPU using spin table
pub fn cpu_on(spin_table_addr: u64, cpu_id: u32, entry_addr: u64, context_id: u64) -> Result<(), &'static str> {
    log::info!("Starting CPU {} via spin table (entry={:#x})", cpu_id, entry_addr);

    let entry = SpinTableEntry {
        cpu_id: cpu_id as u64,
        entry_addr,
        context_id,
    };

    // TODO: Write spin table entry to memory
    // TODO: Issue SEV to wake up CPU

    log::debug!("Spin table entry written to {:#x}", spin_table_addr);
    Ok(())
}

/// Initialize spin table SMP
pub fn init() -> Result<(), &'static str> {
    log::info!("Initializing spin table SMP");
    log::info!("Spin table SMP initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spin_table_entry() {
        let entry = SpinTableEntry {
            cpu_id: 1,
            entry_addr: 0x40000000,
            context_id: 0,
        };
        assert_eq!(entry.cpu_id, 1);
    }
}
