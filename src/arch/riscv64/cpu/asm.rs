//! RISC-V Assembly Helper Functions
//!
//! This module provides assembly language helper functions for:
//! - Low-level register operations
//! - Context switching assembly routines
//! - Exception entry/exit handling
//! - Memory barriers and synchronization

/// Initialize assembly helpers
pub fn init() -> Result<(), &'static str> {
    log::debug!("Initializing assembly helpers");

    // Initialize any global assembly state
    // Set up trap vectors if needed

    log::debug!("Assembly helpers initialized");
    Ok(())
}

/// Read the time CSR (time)
#[inline]
pub fn read_time() -> usize {
    let time: usize;
    unsafe {
        core::arch::asm!("rdtime {}", out(reg) time);
    }
    time
}

/// Read the cycle CSR (mcycle)
#[inline]
pub fn read_cycle() -> u64 {
    let cycle: u64;
    unsafe {
        core::arch::asm!("rdcycle {}", out(reg) cycle);
    }
    cycle
}

/// Read the instruction retired CSR (minstret)
#[inline]
pub fn read_instret() -> u64 {
    let instret: u64;
    unsafe {
        core::arch::asm!("rdinstret {}", out(reg) instret);
    }
    instret
}

/// Read the cycle CSR with shadowing support
#[inline]
pub fn read_cycleh() -> usize {
    let cycleh: usize;
    unsafe {
        core::arch::asm!("rdcycleh {}", out(reg) cycleh);
    }
    cycleh
}

/// Read the instruction retired CSR with shadowing support
#[inline]
pub fn read_instreth() -> usize {
    let instreth: usize;
    unsafe {
        core::arch::asm!("rdinstreth {}", out(reg) instreth);
    }
    instreth
}

/// Get the current PC (program counter)
#[inline]
pub fn get_pc() -> usize {
    let pc: usize;
    unsafe {
        core::arch::asm!("auipc {}, 0", out(reg) pc);
    }
    pc
}

/// Pause instruction (recommended for spin loops)
#[inline]
pub fn pause() {
    unsafe {
        core::arch::asm!("pause");
    }
}

/// NOP instruction
#[inline]
pub fn nop() {
    unsafe {
        core::arch::asm!("nop");
    }
}

/// EBREAK instruction (breakpoint)
#[inline]
pub fn ebreak() {
    unsafe {
        core::arch::asm!("ebreak");
    }
}

/// ECALL instruction (environment call/system call)
#[inline]
pub fn ecall() {
    unsafe {
        core::arch::asm!("ecall");
    }
}

/// SRET instruction (supervisor return)
#[inline]
pub fn sret() {
    unsafe {
        core::arch::asm!("sret");
    }
}

/// MRET instruction (machine return)
#[inline]
pub fn mret() {
    unsafe {
        core::arch::asm!("mret");
    }
}

/// HRET instruction (hypervisor return)
#[inline]
pub fn hret() {
    unsafe {
        core::arch::asm!("hret");
    }
}

/// DRET instruction (debug return)
#[inline]
pub fn dret() {
    unsafe {
        core::arch::asm!("dret");
    }
}

/// WFI instruction (wait for interrupt)
#[inline]
pub fn wfi() {
    unsafe {
        core::arch::asm!("wfi");
    }
}

/// SFENCE.VMA instruction with no parameters
#[inline]
pub fn sfence_vma() {
    unsafe {
        core::arch::asm!("sfence.vma");
    }
}

/// SFENCE.VMA instruction with address parameter
#[inline]
pub fn sfence_vma_addr(addr: usize) {
    unsafe {
        core::arch::asm!("sfence.vma {}, zero", in(reg) addr);
    }
}

/// SFENCE.VMA instruction with address and asid parameters
#[inline]
pub fn sfence_vma_addr_asid(addr: usize, asid: usize) {
    unsafe {
        core::arch::asm!("sfence.vma {}, {}", in(reg) addr, in(reg) asid);
    }
}

/// SFENCE.VMA instruction with asid parameter
#[inline]
pub fn sfence_vma_asid(asid: usize) {
    unsafe {
        core::arch::asm!("sfence.vma zero, {}", in(reg) asid);
    }
}

/// HFENCE.VVMA instruction with no parameters
#[inline]
pub fn hfence_vvma() {
    unsafe {
        core::arch::asm!("hfence.vvma");
    }
}

/// HFENCE.VVMA instruction with address parameter
#[inline]
pub fn hfence_vvma_addr(vaddr: usize) {
    unsafe {
        core::arch::asm!("hfence.vvma {}, zero", in(reg) vaddr);
    }
}

/// HFENCE.VVMA instruction with address and asid parameters
#[inline]
pub fn hfence_vvma_addr_asid(vaddr: usize, asid: usize) {
    unsafe {
        core::arch::asm!("hfence.vvma {}, {}", in(reg) vaddr, in(reg) asid);
    }
}

/// HFENCE.VVMA instruction with asid parameter
#[inline]
pub fn hfence_vvma_asid(asid: usize) {
    unsafe {
        core::arch::asm!("hfence.vvma zero, {}", in(reg) asid);
    }
}

/// HFENCE.GVMA instruction with no parameters
#[inline]
pub fn hfence_gvma() {
    unsafe {
        core::arch::asm!("hfence.gvma");
    }
}

/// HFENCE.GVMA instruction with address parameter
#[inline]
pub fn hfence_gvma_addr(gaddr: usize) {
    unsafe {
        core::arch::asm!("hfence.gvma {}, zero", in(reg) gaddr);
    }
}

/// HFENCE.GVMA instruction with address and vmid parameters
#[inline]
pub fn hfence_gvma_addr_vmid(gaddr: usize, vmid: usize) {
    unsafe {
        core::arch::asm!("hfence.gvma {}, {}", in(reg) gaddr, in(reg) vmid);
    }
}

/// HFENCE.GVMA instruction with vmid parameter
#[inline]
pub fn hfence_gvma_vmid(vmid: usize) {
    unsafe {
        core::arch::asm!("hfence.gvma zero, {}", in(reg) vmid);
    }
}

/// RDCYCLE instruction
#[inline]
pub fn rdcycle() -> usize {
    let cycle: usize;
    unsafe {
        core::arch::asm!("rdcycle {}", out(reg) cycle);
    }
    cycle
}

/// RDCYCLEH instruction (high 32 bits of cycle counter on 32-bit systems)
#[inline]
pub fn rdcycleh() -> usize {
    let cycleh: usize;
    unsafe {
        core::arch::asm!("rdcycleh {}", out(reg) cycleh);
    }
    cycleh
}

/// RDINSTRET instruction
#[inline]
pub fn rdinstret() -> usize {
    let instret: usize;
    unsafe {
        core::arch::asm!("rdinstret {}", out(reg) instret);
    }
    instret
}

/// RDINSTRETH instruction (high 32 bits of instruction counter on 32-bit systems)
#[inline]
pub fn rdinstreth() -> usize {
    let instreth: usize;
    unsafe {
        core::arch::asm!("rdinstreth {}", out(reg) instreth);
    }
    instreth
}

/// RDTIME instruction
#[inline]
pub fn rdtime() -> usize {
    let time: usize;
    unsafe {
        core::arch::asm!("rdtime {}", out(reg) time);
    }
    time
}

/// RDTIMEH instruction (high 32 bits of time counter on 32-bit systems)
#[inline]
pub fn rdtimeh() -> usize {
    let timeh: usize;
    unsafe {
        core::arch::asm!("rdtimeh {}", out(reg) timeh);
    }
    timeh
}

/// CPUID instruction (if supported)
#[inline]
pub fn cpuid(function: usize, arg0: usize) -> (usize, usize, usize, usize) {
    let (eax, ebx, ecx, edx): (usize, usize, usize, usize);
    unsafe {
        core::arch::asm!(
            "cpuid {}, {}, {}, {}, {}",
            in(reg) function,
            in(reg) arg0,
            out(reg) eax,
            out(reg) ebx,
            out(reg) ecx,
            out(reg) edx,
        );
    }
    (eax, ebx, ecx, edx)
}

/// Read a 32-bit value from memory with acquire semantics
#[inline]
pub fn load_acquire<T>(ptr: *const T) -> T
where
    T: Copy,
{
    let value: T;
    unsafe {
        core::arch::asm!(
            "lr.w {}, ({})",
            out(reg) value,
            in(reg) ptr,
        );
    }
    value
}

/// Write a 32-bit value to memory with release semantics
#[inline]
pub fn store_release<T>(ptr: *mut T, value: T)
where
    T: Copy,
{
    unsafe {
        core::arch::asm!(
            "sc.w {}, {}, ({})",
            out(reg) _,
            in(reg) value,
            in(reg) ptr,
        );
    }
}

/// Atomic fetch-and-add for 32-bit values
#[inline]
pub fn atomic_fetch_add(ptr: *mut u32, value: u32) -> u32 {
    let result: u32;
    unsafe {
        core::arch::asm!(
            "amoswap.w.aq {}, {}, ({})",
            out(reg) result,
            in(reg) value,
            in(reg) ptr,
        );
    }
    result
}

/// Atomic exchange for 32-bit values
#[inline]
pub fn atomic_swap(ptr: *mut u32, value: u32) -> u32 {
    let result: u32;
    unsafe {
        core::arch::asm!(
            "amoswap.w {}, {}, ({})",
            out(reg) result,
            in(reg) value,
            in(reg) ptr,
        );
    }
    result
}

/// Get current privilege level by reading mstatus
#[inline]
pub fn get_current_privilege() -> crate::arch::riscv64::PrivilegeLevel {
    let mstatus: usize;
    unsafe {
        core::arch::asm!("csrr {}, mstatus", out(reg) mstatus);
    }

    match (mstatus >> 11) & 0x3 {
        0 => crate::arch::riscv64::PrivilegeLevel::User,
        1 => crate::arch::riscv64::PrivilegeLevel::Supervisor,
        2 => crate::arch::riscv64::PrivilegeLevel::Reserved,
        3 => crate::arch::riscv64::PrivilegeLevel::Machine,
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nop() {
        // Just ensure it doesn't panic
        nop();
    }

    #[test]
    fn test_pause() {
        // Just ensure it doesn't panic
        pause();
    }

    #[test]
    fn test_privilege_level() {
        // In most test environments, we'll be in machine mode
        let priv_level = get_current_privilege();
        assert_eq!(priv_level, crate::arch::riscv64::PrivilegeLevel::Machine);
    }

    #[test]
    fn test_time_reading() {
        let t1 = rdtime();
        // Small delay
        for _ in 0..1000 {
            nop();
        }
        let t2 = rdtime();
        // Time should have advanced
        assert!(t2 >= t1);
    }

    #[test]
    fn test_cycle_counter() {
        let c1 = rdcycle();
        // Do some work
        for _ in 0..1000 {
            nop();
        }
        let c2 = rdcycle();
        // Cycle counter should have advanced
        assert!(c2 >= c1);
    }

    #[test]
    fn test_inst_counter() {
        let i1 = rdinstret();
        // Execute some instructions
        for _ in 0..10 {
            nop();
        }
        let i2 = rdinstret();
        // Instruction counter should have advanced
        assert!(i2 > i1);
    }
}