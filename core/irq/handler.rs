//! Interrupt handler implementations
//!
//! This module provides common interrupt handler implementations
//! for various types of interrupts and devices.

use crate::{Result, Error};
use crate::core::irq::{IrqNumber, IrqHandler, IrqDescriptor};
use crate::core::sync::SpinLock;
use crate::core::sched::{self, ThreadId};
use crate::core::vmm::{self, VmId, VcpuId};
use core::sync::atomic::{AtomicU64, Ordering};

/// Simple function-based interrupt handler
pub struct SimpleIrqHandler {
    /// Handler function
    handler_fn: fn(IrqNumber, *mut u8),
    /// Handler argument
    arg: *mut u8,
    /// Handler name for debugging
    name: &'static str,
    /// Call count
    call_count: AtomicU64,
}

impl SimpleIrqHandler {
    /// Create a new simple handler
    pub const fn new(handler_fn: fn(IrqNumber, *mut u8), arg: *mut u8, name: &'static str) -> Self {
        Self {
            handler_fn,
            arg,
            name,
            call_count: AtomicU64::new(0),
        }
    }

    /// Get the call count
    pub fn call_count(&self) -> u64 {
        self.call_count.load(Ordering::Relaxed)
    }
}

unsafe impl Send for SimpleIrqHandler {}
unsafe impl Sync for SimpleIrqHandler {}

impl IrqHandler for SimpleIrqHandler {
    fn handle(&mut self, irq: IrqNumber, arg: *mut u8) -> Result<()> {
        self.call_count.fetch_add(1, Ordering::Relaxed);
        crate::debug!("IRQ handler '{}' called for IRQ {}", self.name, irq);

        (self.handler_fn)(irq, arg);

        Ok(())
    }
}

/// Timer interrupt handler
pub struct TimerIrqHandler {
    /// Timer tick period in milliseconds
    tick_period_ms: u32,
    /// Total ticks elapsed
    total_ticks: AtomicU64,
    /// Last tick time
    last_tick: AtomicU64,
}

impl TimerIrqHandler {
    /// Create a new timer handler
    pub const fn new(tick_period_ms: u32) -> Self {
        Self {
            tick_period_ms,
            total_ticks: AtomicU64::new(0),
            last_tick: AtomicU64::new(0),
        }
    }

    /// Get total ticks
    pub fn total_ticks(&self) -> u64 {
        self.total_ticks.load(Ordering::Relaxed)
    }

    /// Get tick period
    pub fn tick_period_ms(&self) -> u32 {
        self.tick_period_ms
    }
}

unsafe impl Send for TimerIrqHandler {}
unsafe impl Sync for TimerIrqHandler {}

impl IrqHandler for TimerIrqHandler {
    fn handle(&mut self, irq: IrqNumber, _arg: *mut u8) -> Result<()> {
        let current_time = crate::utils::get_timestamp();
        let tick_count = self.total_ticks.fetch_add(1, Ordering::Relaxed);

        // Update last tick time
        self.last_tick.store(current_time, Ordering::Relaxed);

        // Handle scheduler tick
        if let Err(e) = sched::handle_tick() {
            crate::error!("Scheduler tick failed: {:?}", e);
        }

        // Trigger scheduling on current CPU
        let cpu_id = crate::core::cpu_id();
        if let Err(e) = sched::schedule(cpu_id) {
            crate::error!("Schedule failed on CPU {}: {:?}", cpu_id, e);
        }

        // Log timer tick periodically
        if tick_count % 100 == 0 {
            crate::debug!("Timer tick: {} (IRQ {})", tick_count, irq);
        }

        Ok(())
    }
}

/// Serial port interrupt handler
pub struct SerialIrqHandler {
    /// Base port address
    base_addr: u16,
    /// Receive buffer
    rx_buffer: SpinLock<Vec<u8>>,
    /// Transmit buffer
    tx_buffer: SpinLock<Vec<u8>>,
    /// Statistics
    stats: SpinLock<SerialStats>,
}

/// Serial port statistics
#[derive(Debug, Default, Clone, Copy)]
pub struct SerialStats {
    /// Bytes received
    pub bytes_received: u64,
    /// Bytes transmitted
    pub bytes_transmitted: u64,
    /// Receive overruns
    pub rx_overruns: u64,
    /// Transmit underruns
    pub tx_underruns: u64,
    /// Framing errors
    pub framing_errors: u64,
    /// Parity errors
    pub parity_errors: u64,
}

impl SerialIrqHandler {
    /// Create a new serial handler
    pub const fn new(base_addr: u16) -> Self {
        Self {
            base_addr,
            rx_buffer: SpinLock::new(Vec::new()),
            tx_buffer: SpinLock::new(Vec::new()),
            stats: SpinLock::new(SerialStats::default()),
        }
    }

    /// Read from receive buffer
    pub fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let mut rx = self.rx_buffer.lock();
        let count = core::cmp::min(buf.len(), rx.len());

        for i in 0..count {
            buf[i] = rx.remove(0);
        }

        Ok(count)
    }

    /// Write to transmit buffer
    pub fn write(&self, data: &[u8]) -> Result<usize> {
        let mut tx = self.tx_buffer.lock();
        let start_len = tx.len();
        tx.extend_from_slice(data);

        Ok(data.len())
    }

    /// Get statistics
    pub fn get_stats(&self) -> SerialStats {
        *self.stats.lock()
    }
}

unsafe impl Send for SerialIrqHandler {}
unsafe impl Sync for SerialIrqHandler {}

impl IrqHandler for SerialIrqHandler {
    fn handle(&mut self, irq: IrqNumber, _arg: *mut u8) -> Result<()> {
        // Read interrupt status
        let status = unsafe {
            // Read IIR (Interrupt Identification Register)
            let iir = ((self.base_addr + 2) as *mut u8).read_volatile();
            iir
        };

        let interrupt_id = status >> 1 & 0x07;

        match interrupt_id {
            0 => {
                // Modem status
                crate::debug!("Serial modem status interrupt (IRQ {})", irq);
            }
            1 => {
                // No interrupt pending
                // This shouldn't happen in an interrupt handler
            }
            2 => {
                // Transmit holding register empty
                let mut tx = self.tx_buffer.lock();
                if !tx.is_empty() {
                    // Transmit next byte
                    let byte = tx.remove(0);
                    unsafe {
                        // Write to THR (Transmit Holding Register)
                        ((self.base_addr + 0) as *mut u8).write_volatile(byte);
                    }

                    // Update statistics
                    let mut stats = self.stats.lock();
                    stats.bytes_transmitted += 1;
                }
            }
            3 => {
                // Received data available
                loop {
                    unsafe {
                        // Read LSR (Line Status Register)
                        let lsr = ((self.base_addr + 5) as *mut u8).read_volatile();

                        if (lsr & 0x01) == 0 {
                            break; // No more data
                        }

                        // Read RBR (Receiver Buffer Register)
                        let byte = ((self.base_addr + 0) as *mut u8).read_volatile();

                        // Store in buffer
                        let mut rx = self.rx_buffer.lock();
                        if rx.len() < 1024 { // Limit buffer size
                            rx.push(byte);

                            let mut stats = self.stats.lock();
                            stats.bytes_received += 1;
                        } else {
                            // Buffer overrun
                            let mut stats = self.stats.lock();
                            stats.rx_overruns += 1;
                        }
                    }
                }
            }
            4 => {
                // Line status error
                unsafe {
                    let lsr = ((self.base_addr + 5) as *mut u8).read_volatile();
                    let mut stats = self.stats.lock();

                    if (lsr & 0x80) != 0 {
                        stats.framing_errors += 1;
                    }
                    if (lsr & 0x40) != 0 {
                        stats.parity_errors += 1;
                    }
                }
                crate::warn!("Serial line status error (IRQ {})", irq);
            }
            5 => {
                // Data timeout
                // Similar to received data available
                crate::debug!("Serial data timeout (IRQ {})", irq);
            }
            _ => {
                crate::warn!("Unknown serial interrupt ID: {} (IRQ {})", interrupt_id, irq);
            }
        }

        Ok(())
    }
}

/// Network interrupt handler
pub struct NetworkIrqHandler {
    /// Network interface identifier
    interface_id: u32,
    /// Statistics
    stats: SpinLock<NetworkStats>,
}

/// Network statistics
#[derive(Debug, Default, Clone, Copy)]
pub struct NetworkStats {
    /// Packets received
    pub packets_received: u64,
    /// Packets transmitted
    pub packets_transmitted: u64,
    /// Bytes received
    pub bytes_received: u64,
    /// Bytes transmitted
    pub bytes_transmitted: u64,
    /// Receive errors
    pub rx_errors: u64,
    /// Transmit errors
    pub tx_errors: u64,
    /// Collisions
    pub collisions: u64,
}

impl NetworkIrqHandler {
    /// Create a new network handler
    pub const fn new(interface_id: u32) -> Self {
        Self {
            interface_id,
            stats: SpinLock::new(NetworkStats::default()),
        }
    }

    /// Get statistics
    pub fn get_stats(&self) -> NetworkStats {
        *self.stats.lock()
    }
}

unsafe impl Send for NetworkIrqHandler {}
unsafe impl Sync for NetworkIrqHandler {}

impl IrqHandler for NetworkIrqHandler {
    fn handle(&mut self, irq: IrqNumber, _arg: *mut u8) -> Result<()> {
        crate::debug!("Network interrupt on interface {} (IRQ {})", self.interface_id, irq);

        // Read interrupt status
        // This would be implemented based on the specific network hardware

        // Update statistics
        {
            let mut stats = self.stats.lock();
            // Update based on actual interrupt cause
        }

        Ok(())
    }
}

/// IPI (Inter-Processor Interrupt) handler
pub struct IpiIrqHandler {
    /// CPU ID that this handler runs on
    cpu_id: usize,
    /// Statistics
    stats: SpinLock<IpiStats>,
}

/// IPI statistics
#[derive(Debug, Default, Clone, Copy)]
pub struct IpiStats {
    /// Reschedule IPIs received
    pub reschedule_count: u64,
    /// Function call IPIs received
    pub function_call_count: u64,
    /// TLB flush IPIs received
    pub tlb_flush_count: u64,
    /// Stop CPU IPIs received
    pub stop_cpu_count: u64,
}

impl IpiIrqHandler {
    /// Create a new IPI handler
    pub const fn new(cpu_id: usize) -> Self {
        Self {
            cpu_id,
            stats: SpinLock::new(IpiStats::default()),
        }
    }

    /// Get statistics
    pub fn get_stats(&self) -> IpiStats {
        *self.stats.lock()
    }
}

unsafe impl Send for IpiIrqHandler {}
unsafe impl Sync for IpiIrqHandler {}

impl IrqHandler for IpiIrqHandler {
    fn handle(&mut self, irq: IrqNumber, _arg: *mut u8) -> Result<()> {
        // Read IPI type from IPI register
        let ipi_type = unsafe {
            // This would read from the architecture-specific IPI register
            // For now, assume it's a reschedule
            0u32
        };

        match ipi_type {
            0 => {
                // Reschedule IPI
                crate::debug!("Reschedule IPI on CPU {} (IRQ {})", self.cpu_id, irq);

                // Trigger scheduler
                if let Err(e) = sched::schedule(self.cpu_id) {
                    crate::error!("Schedule failed on CPU {}: {:?}", self.cpu_id, e);
                }

                let mut stats = self.stats.lock();
                stats.reschedule_count += 1;
            }
            1 => {
                // Function call IPI
                crate::debug!("Function call IPI on CPU {} (IRQ {})", self.cpu_id, irq);

                // Handle function call
                // Implementation depends on architecture

                let mut stats = self.stats.lock();
                stats.function_call_count += 1;
            }
            2 => {
                // TLB flush IPI
                crate::debug!("TLB flush IPI on CPU {} (IRQ {})", self.cpu_id, irq);

                // Invalidate TLB
                crate::arch::invalidate_tlb();

                let mut stats = self.stats.lock();
                stats.tlb_flush_count += 1;
            }
            3 => {
                // Stop CPU IPI
                crate::warn!("Stop CPU IPI on CPU {} (IRQ {})", self.cpu_id, irq);

                // Stop this CPU
                // Implementation depends on architecture

                let mut stats = self.stats.lock();
                stats.stop_cpu_count += 1;
            }
            _ => {
                crate::warn!("Unknown IPI type {} on CPU {} (IRQ {})", ipi_type, self.cpu_id, irq);
            }
        }

        Ok(())
    }
}

/// Virtualization interrupt handler
pub struct VirtIrqHandler {
    /// VM ID
    vm_id: VmId,
    /// VCPU ID
    vcpu_id: VcpuId,
    /// Statistics
    stats: SpinLock<VirtIrqStats>,
}

/// Virtualization interrupt statistics
#[derive(Debug, Default, Clone, Copy)]
pub struct VirtIrqStats {
    /// Virtual interrupts injected
    pub vint_injected: u64,
    /// Physical interrupts for VM
    pub pint_for_vm: u64,
    /// VM exits due to interrupts
    pub vm_exits_irq: u64,
    /// VM exits due to exceptions
    pub vm_exits_exception: u64,
}

impl VirtIrqHandler {
    /// Create a new virtualization handler
    pub const fn new(vm_id: VmId, vcpu_id: VcpuId) -> Self {
        Self {
            vm_id,
            vcpu_id,
            stats: SpinLock::new(VirtIrqStats::default()),
        }
    }

    /// Get statistics
    pub fn get_stats(&self) -> VirtIrqStats {
        *self.stats.lock()
    }
}

unsafe impl Send for VirtIrqHandler {}
unsafe impl Sync for VirtIrqHandler {}

impl IrqHandler for VirtIrqHandler {
    fn handle(&mut self, irq: IrqNumber, _arg: *mut u8) -> Result<()> {
        crate::debug!("Virtualization interrupt for VM:{} VCPU:{} (IRQ {})",
                     self.vm_id, self.vcpu_id, irq);

        // Handle virtual interrupt
        // This could involve injecting an interrupt into the guest VM

        // Update statistics
        {
            let mut stats = self.stats.lock();
            stats.pint_for_vm += 1;
        }

        Ok(())
    }
}

/// Error interrupt handler
pub struct ErrorIrqHandler {
    /// Error type
    error_type: &'static str,
    /// Error count
    error_count: AtomicU64,
}

impl ErrorIrqHandler {
    /// Create a new error handler
    pub const fn new(error_type: &'static str) -> Self {
        Self {
            error_type,
            error_count: AtomicU64::new(0),
        }
    }

    /// Get error count
    pub fn error_count(&self) -> u64 {
        self.error_count.load(Ordering::Relaxed)
    }
}

unsafe impl Send for ErrorIrqHandler {}
unsafe impl Sync for ErrorIrqHandler {}

impl IrqHandler for ErrorIrqHandler {
    fn handle(&mut self, irq: IrqNumber, _arg: *mut u8) -> Result<()> {
        let count = self.error_count.fetch_add(1, Ordering::Relaxed) + 1;

        crate::error!("{} interrupt #{} (IRQ {})", self.error_type, count, irq);

        // For error interrupts, we might want to:
        // 1. Log detailed error information
        // 2. Try to recover if possible
        // 3. Or trigger a panic if it's critical

        // For now, just log and continue
        Ok(())
    }
}

/// Create a timer interrupt handler
pub fn create_timer_handler(tick_period_ms: u32) -> TimerIrqHandler {
    TimerIrqHandler::new(tick_period_ms)
}

/// Create a serial port interrupt handler
pub fn create_serial_handler(base_addr: u16) -> SerialIrqHandler {
    SerialIrqHandler::new(base_addr)
}

/// Create a network interrupt handler
pub fn create_network_handler(interface_id: u32) -> NetworkIrqHandler {
    NetworkIrqHandler::new(interface_id)
}

/// Create an IPI handler
pub fn create_ipi_handler(cpu_id: usize) -> IpiIrqHandler {
    IpiIrqHandler::new(cpu_id)
}

/// Create a virtualization interrupt handler
pub fn create_virt_handler(vm_id: VmId, vcpu_id: VcpuId) -> VirtIrqHandler {
    VirtIrqHandler::new(vm_id, vcpu_id)
}

/// Create an error interrupt handler
pub fn create_error_handler(error_type: &'static str) -> ErrorIrqHandler {
    ErrorIrqHandler::new(error_type)
}