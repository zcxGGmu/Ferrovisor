//! VGIC Interrupt Routing for ARM64
//!
//! Provides interrupt routing and distribution for virtual GIC.
//! Handles SGI, PPI, SPI, and LPI interrupt routing to VCPUs.
//!
//! Reference: xvisor/arch/arm/cpu/common/vgic.c

use crate::arch::arm64::interrupt::gic::{GicVersion, GicDevice};
use crate::arch::arm64::interrupt::vgic::{VgicGuestState, VgicVcpuState, VgicLr, VgicLrFlags, VGIC_MAX_NIRQ};
use crate::Result;

/// Maximum number of SGI interrupts (0-15)
pub const VGIC_MAX_SGI: u32 = 16;

/// Maximum number of PPI interrupts (16-31)
pub const VGIC_MAX_PPI: u32 = 16;

/// Maximum number of SPI interrupts (32-1019)
pub const VGIC_MAX_SPI: u32 = 988;

/// First SPI interrupt number
pub const VGIC_SPI_BASE: u32 = 32;

/// First LPI interrupt number
pub const VGIC_LPI_BASE: u32 = 4096;

/// Unknown LR value
pub const VGIC_LR_UNKNOWN: u8 = 0xFF;

/// Interrupt type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrqType {
    /// Software Generated Interrupt (0-15)
    Sgi,
    /// Private Peripheral Interrupt (16-31)
    Ppi,
    /// Shared Peripheral Interrupt (32-1019)
    Spi,
    /// Locality-specific Peripheral Interrupt (4096+)
    Lpi,
    /// Unknown interrupt type
    Unknown,
}

impl IrqType {
    /// Get interrupt type from IRQ number
    pub fn from_irq(irq: u32) -> Self {
        if irq < 16 {
            Self::Sgi
        } else if irq < 32 {
            Self::Ppi
        } else if irq >= VGIC_LPI_BASE {
            Self::Lpi
        } else if irq < 1020 {
            Self::Spi
        } else {
            Self::Unknown
        }
    }

    /// Check if IRQ is per-CPU (SGI or PPI)
    pub fn is_per_cpu(self) -> bool {
        matches!(self, Self::Sgi | Self::Ppi)
    }

    /// Check if IRQ is shared (SPI or LPI)
    pub fn is_shared(self) -> bool {
        matches!(self, Self::Spi | Self::Lpi)
    }
}

/// Interrupt state for a single IRQ
#[derive(Debug, Clone, Copy)]
pub struct IrqState {
    /// Active state per CPU (bitmask)
    pub active: u32,
    /// Level-sensitive state per CPU (bitmask)
    pub level: u32,
    /// Configuration model: 0 = N:N, 1 = 1:N
    pub model: bool,
    /// Trigger type: 0 = level, 1 = edge
    pub trigger: bool,
    /// Host IRQ mapping (UINT_MAX if not mapped)
    pub host_irq: u32,
}

impl Default for IrqState {
    fn default() -> Self {
        Self {
            active: 0,
            level: 0,
            model: false,
            trigger: false,
            host_irq: u32::MAX,
        }
    }
}

impl IrqState {
    /// Create new interrupt state
    pub fn new() -> Self {
        Self::default()
    }

    /// Test if interrupt is active for CPU mask
    pub fn is_active(&self, cpu_mask: u32) -> bool {
        (self.active & cpu_mask) != 0
    }

    /// Set active state for CPU mask
    pub fn set_active(&mut self, cpu_mask: u32) {
        self.active |= cpu_mask;
    }

    /// Clear active state for CPU mask
    pub fn clear_active(&mut self, cpu_mask: u32) {
        self.active &= !cpu_mask;
    }

    /// Test if interrupt level is set for CPU mask
    pub fn is_level(&self, cpu_mask: u32) -> bool {
        (self.level & cpu_mask) != 0
    }

    /// Set level state for CPU mask
    pub fn set_level(&mut self, cpu_mask: u32) {
        self.level |= cpu_mask;
    }

    /// Clear level state for CPU mask
    pub fn clear_level(&mut self, cpu_mask: u32) {
        self.level &= !cpu_mask;
    }

    /// Check if edge triggered
    pub fn is_edge_triggered(&self) -> bool {
        self.trigger
    }

    /// Set edge trigger
    pub fn set_edge_trigger(&mut self) {
        self.trigger = true;
    }

    /// Set level trigger
    pub fn set_level_trigger(&mut self) {
        self.trigger = false;
    }

    /// Get host IRQ mapping
    pub fn host_irq(&self) -> Option<u32> {
        if self.host_irq != u32::MAX {
            Some(self.host_irq)
        } else {
            None
        }
    }

    /// Set host IRQ mapping
    pub fn set_host_irq(&mut self, host_irq: u32) {
        self.host_irq = host_irq;
    }
}

/// Distributor state for interrupt routing
#[derive(Debug)]
pub struct DistributorState {
    /// Maximum number of VCPUs
    pub max_vcpus: u32,

    /// Number of VCPUs
    pub num_vcpus: u32,

    /// Number of IRQs
    pub num_irqs: u32,

    /// Interrupt state per IRQ
    pub irq_state: [IrqState; VGIC_MAX_NIRQ as usize],

    /// SGI source tracking [cpu][sgi]
    pub sgi_source: [[u16; VGIC_MAX_SGI as usize]; 8],

    /// Interrupt target CPUs per IRQ
    pub irq_target: [u8; VGIC_MAX_NIRQ as usize],

    /// Priority for SGI/PPI (0-31) per CPU
    pub priority1: [[u8; 32]; 8],

    /// Priority for SPI (32-1019)
    pub priority2: [u8; (VGIC_MAX_NIRQ - 32) as usize],

    /// Interrupt enable state [cpu][irq_word]
    pub irq_enabled: [[u32; (VGIC_MAX_NIRQ / 32) as usize]; 8],

    /// Interrupt pending state [cpu][irq_word]
    pub irq_pending: [[u32; (VGIC_MAX_NIRQ / 32) as usize]; 8],

    /// Distributor enabled
    pub enabled: bool,
}

impl Default for DistributorState {
    fn default() -> Self {
        Self {
            max_vcpus: 8,
            num_vcpus: 1,
            num_irqs: VGIC_MAX_NIRQ,
            irq_state: [IrqState::default(); VGIC_MAX_NIRQ as usize],
            sgi_source: [[0; VGIC_MAX_SGI as usize]; 8],
            irq_target: [0; VGIC_MAX_NIRQ as usize],
            priority1: [[0; 32]; 8],
            priority2: [0; (VGIC_MAX_NIRQ - 32) as usize],
            irq_enabled: [[0; (VGIC_MAX_NIRQ / 32) as usize]; 8],
            irq_pending: [[0; (VGIC_MAX_NIRQ / 32) as usize]; 8],
            enabled: false,
        }
    }
}

impl DistributorState {
    /// Create new distributor state
    pub fn new(num_vcpus: u32, num_irqs: u32) -> Self {
        let num_vcpus = num_vcpus.min(8);
        let num_irqs = num_irqs.min(VGIC_MAX_NIRQ);

        Self {
            max_vcpus: 8,
            num_vcpus,
            num_irqs,
            ..Default::default()
        }
    }

    /// Get all CPUs mask
    pub fn all_cpus_mask(&self) -> u32 {
        (1u32 << self.num_vcpus) - 1
    }

    /// Test if interrupt is enabled
    pub fn test_enabled(&self, irq: u32, cpu_mask: u32) -> bool {
        for cpu in 0..self.num_vcpus {
            if (cpu_mask & (1 << cpu)) == 0 {
                continue;
            }
            let word = (irq >> 5) as usize;
            let bit = irq & 0x1f;
            if (self.irq_enabled[cpu as usize][word] & (1 << bit)) != 0 {
                return true;
            }
        }
        false
    }

    /// Set interrupt enabled
    pub fn set_enabled(&mut self, irq: u32, cpu_mask: u32) {
        for cpu in 0..self.num_vcpus {
            if (cpu_mask & (1 << cpu)) == 0 {
                continue;
            }
            let word = (irq >> 5) as usize;
            let bit = irq & 0x1f;
            self.irq_enabled[cpu as usize][word] |= 1 << bit;
        }
    }

    /// Clear interrupt enabled
    pub fn clear_enabled(&mut self, irq: u32, cpu_mask: u32) {
        for cpu in 0..self.num_vcpus {
            if (cpu_mask & (1 << cpu)) == 0 {
                continue;
            }
            let word = (irq >> 5) as usize;
            let bit = irq & 0x1f;
            self.irq_enabled[cpu as usize][word] &= !(1 << bit);
        }
    }

    /// Test if interrupt is pending
    pub fn test_pending(&self, irq: u32, cpu_mask: u32) -> bool {
        for cpu in 0..self.num_vcpus {
            if (cpu_mask & (1 << cpu)) == 0 {
                continue;
            }
            let word = (irq >> 5) as usize;
            let bit = irq & 0x1f;
            if (self.irq_pending[cpu as usize][word] & (1 << bit)) != 0 {
                return true;
            }
        }
        false
    }

    /// Set interrupt pending
    pub fn set_pending(&mut self, irq: u32, cpu_mask: u32) {
        for cpu in 0..self.num_vcpus {
            if (cpu_mask & (1 << cpu)) == 0 {
                continue;
            }
            let word = (irq >> 5) as usize;
            let bit = irq & 0x1f;
            self.irq_pending[cpu as usize][word] |= 1 << bit;
        }
    }

    /// Clear interrupt pending
    pub fn clear_pending(&mut self, irq: u32, cpu_mask: u32) {
        for cpu in 0..self.num_vcpus {
            if (cpu_mask & (1 << cpu)) == 0 {
                continue;
            }
            let word = (irq >> 5) as usize;
            let bit = irq & 0x1f;
            self.irq_pending[cpu as usize][word] &= !(1 << bit);
        }
    }

    /// Get interrupt priority
    pub fn get_priority(&self, irq: u32, cpu: u32) -> u8 {
        if irq < 32 {
            self.priority1[cpu as usize][irq as usize]
        } else {
            self.priority2[(irq - 32) as usize]
        }
    }

    /// Set interrupt priority
    pub fn set_priority(&mut self, irq: u32, cpu: u32, priority: u8) {
        if irq < 32 {
            self.priority1[cpu as usize][irq as usize] = priority;
        } else {
            self.priority2[(irq - 32) as usize] = priority;
        }
    }

    /// Get interrupt target CPUs
    pub fn get_target(&self, irq: u32) -> u32 {
        self.irq_target[irq as usize] as u32 & self.all_cpus_mask()
    }

    /// Set interrupt target CPUs
    pub fn set_target(&mut self, irq: u32, target: u32) {
        self.irq_target[irq as usize] = (target & self.all_cpus_mask()) as u8;
    }

    /// Get interrupt state
    pub fn get_irq_state(&self, irq: u32) -> IrqState {
        self.irq_state[irq as usize]
    }

    /// Set interrupt state
    pub fn set_irq_state(&mut self, irq: u32, state: IrqState) {
        self.irq_state[irq as usize] = state;
    }
}

/// VGIC routing context
pub struct VgicRouting {
    /// Distributor state
    pub dist: DistributorState,

    /// GIC version
    pub version: GicVersion,
}

impl VgicRouting {
    /// Create new VGIC routing context
    pub fn new(num_vcpus: u32, num_irqs: u32, version: GicVersion) -> Self {
        Self {
            dist: DistributorState::new(num_vcpus, num_irqs),
            version,
        }
    }

    /// Enable distributor
    pub fn enable(&mut self) {
        self.dist.enabled = true;
        log::debug!("VGIC distributor enabled");
    }

    /// Disable distributor
    pub fn disable(&mut self) {
        self.dist.enabled = false;
        log::debug!("VGIC distributor disabled");
    }

    /// Check if distributor is enabled
    pub fn is_enabled(&self) -> bool {
        self.dist.enabled
    }

    /// Get target CPUs for an interrupt
    pub fn get_irq_target(&self, irq: u32, source_cpu: u32) -> u32 {
        let irq_type = IrqType::from_irq(irq);

        match irq_type {
            IrqType::Sgi | IrqType::Ppi => {
                // Per-CPU interrupts target only the source CPU
                1 << source_cpu
            }
            IrqType::Spi | IrqType::Lpi => {
                // Shared interrupts use target register
                self.dist.get_target(irq)
            }
            IrqType::Unknown => 0,
        }
    }

    /// Route SGI to target CPUs
    pub fn route_sgi(&mut self, sgi: u32, source_cpu: u32, target_mask: u32) -> Result<(), &'static str> {
        if sgi >= 16 {
            return Err("Invalid SGI number");
        }
        if source_cpu >= self.dist.num_vcpus {
            return Err("Invalid source CPU");
        }

        // Track SGI sources
        let target_mask = target_mask & self.dist.all_cpus_mask();
        self.dist.sgi_source[source_cpu as usize][sgi as usize] = target_mask as u16;

        // Set pending for target CPUs
        self.dist.set_pending(sgi, target_mask);

        log::debug!("SGI {} routed from CPU {} to CPUs {:#b}", sgi, source_cpu, target_mask);
        Ok(())
    }

    /// Route PPI to target CPU
    pub fn route_ppi(&mut self, pp: u32, cpu: u32) -> Result<(), &'static str> {
        if pp < 16 || pp >= 32 {
            return Err("Invalid PPI number");
        }
        if cpu >= self.dist.num_vcpus {
            return Err("Invalid CPU");
        }

        // PPIs are per-CPU, only target the specified CPU
        let cpu_mask = 1 << cpu;
        self.dist.set_pending(pp, cpu_mask);

        log::debug!("PPI {} routed to CPU {}", pp, cpu);
        Ok(())
    }

    /// Route SPI to target CPUs
    pub fn route_spi(&mut self, spi: u32, target_mask: u32) -> Result<(), &'static str> {
        if spi < 32 || spi >= 1020 {
            return Err("Invalid SPI number");
        }

        let target_mask = target_mask & self.dist.all_cpus_mask();
        if target_mask == 0 {
            return Err("No target CPUs specified");
        }

        // Set target and pending
        self.dist.set_target(spi, target_mask);
        self.dist.set_pending(spi, target_mask);

        log::debug!("SPI {} routed to CPUs {:#b}", spi, target_mask);
        Ok(())
    }

    /// Route LPI to target CPUs
    pub fn route_lpi(&mut self, lpi: u32, target_mask: u32) -> Result<(), &'static str> {
        if lpi < VGIC_LPI_BASE {
            return Err("Invalid LPI number");
        }

        let target_mask = target_mask & self.dist.all_cpus_mask();
        if target_mask == 0 {
            return Err("No target CPUs specified");
        }

        // LPIs use a different routing mechanism (via Redistributor)
        // For now, just set pending state
        let lpi_index = (lpi - VGIC_LPI_BASE) as usize;
        if lpi_index < self.dist.irq_pending[0].len() * 32 {
            let word = lpi_index / 32;
            let bit = lpi_index % 32;
            for cpu in 0..self.dist.num_vcpus {
                if (target_mask & (1 << cpu)) != 0 {
                    self.dist.irq_pending[cpu as usize][word] |= 1 << bit;
                }
            }
        }

        log::debug!("LPI {} routed to CPUs {:#b}", lpi, target_mask);
        Ok(())
    }

    /// Configure interrupt routing
    pub fn configure_irq(&mut self, irq: u32, target_mask: u32, enable: bool) -> Result<(), &'static str> {
        let target_mask = target_mask & self.dist.all_cpus_mask();

        if enable {
            self.dist.set_enabled(irq, target_mask);
        } else {
            self.dist.clear_enabled(irq, target_mask);
        }

        let irq_type = IrqType::from_irq(irq);
        match irq_type {
            IrqType::Sgi => {
                // SGI targets are configured at send time
            }
            IrqType::Ppi => {
                // PPI is per-CPU, get first set bit
                let cpu = target_mask.trailing_zeros() as u32;
                if cpu < self.dist.num_vcpus {
                    self.dist.set_target(irq, 1 << cpu);
                }
            }
            IrqType::Spi => {
                self.dist.set_target(irq, target_mask);
            }
            IrqType::Lpi => {
                // LPI routing handled by Redistributor
            }
            IrqType::Unknown => {
                return Err("Unknown interrupt type");
            }
        }

        log::debug!("IRQ {} configured with target {:#b}, enable={}", irq, target_mask, enable);
        Ok(())
    }

    /// Set interrupt priority
    pub fn set_irq_priority(&mut self, irq: u32, cpu: u32, priority: u8) -> Result<(), &'static str> {
        if irq >= self.dist.num_irqs {
            return Err("Invalid IRQ number");
        }
        if cpu >= self.dist.num_vcpus {
            return Err("Invalid CPU");
        }

        self.dist.set_priority(irq, cpu, priority);
        log::debug!("IRQ {} priority set to {} for CPU {}", irq, priority, cpu);
        Ok(())
    }

    /// Set interrupt trigger type
    pub fn set_irq_trigger(&mut self, irq: u32, edge_triggered: bool) -> Result<(), &'static str> {
        if irq >= self.dist.num_irqs {
            return Err("Invalid IRQ number");
        }

        let mut state = self.dist.get_irq_state(irq);
        if edge_triggered {
            state.set_edge_trigger();
        } else {
            state.set_level_trigger();
        }
        self.dist.set_irq_state(irq, state);

        log::debug!("IRQ {} trigger set to {:?}", irq, if edge_triggered { "edge" } else { "level" });
        Ok(())
    }

    /// Map host IRQ to guest IRQ
    pub fn map_host_irq(&mut self, guest_irq: u32, host_irq: u32) -> Result<(), &'static str> {
        if guest_irq >= self.dist.num_irqs {
            return Err("Invalid guest IRQ number");
        }

        let mut state = self.dist.get_irq_state(guest_irq);
        state.set_host_irq(host_irq);
        self.dist.set_irq_state(guest_irq, state);

        log::debug!("Guest IRQ {} mapped to host IRQ {}", guest_irq, host_irq);
        Ok(())
    }

    /// Get host IRQ mapping
    pub fn get_host_irq(&self, guest_irq: u32) -> Option<u32> {
        if guest_irq >= self.dist.num_irqs {
            return None;
        }
        self.dist.get_irq_state(guest_irq).host_irq()
    }

    /// Clear interrupt pending
    pub fn clear_irq_pending(&mut self, irq: u32, cpu_mask: u32) {
        let cpu_mask = cpu_mask & self.dist.all_cpus_mask();
        self.dist.clear_pending(irq, cpu_mask);

        let irq_type = IrqType::from_irq(irq);
        if irq_type == IrqType::Sgi {
            // Clear SGI source tracking
            for cpu in 0..self.dist.num_vcpus {
                if (cpu_mask & (1 << cpu)) != 0 {
                    let sgi = irq as usize;
                    self.dist.sgi_source[cpu as usize][sgi] &= !(cpu_mask as u16);
                }
            }
        }

        log::debug!("IRQ {} pending cleared for CPUs {:#b}", irq, cpu_mask);
    }

    /// Check if VCPU has pending interrupts
    pub fn vcpu_has_pending(&self, vcpu: &VgicVcpuState) -> bool {
        let vcpu_id = vcpu.parent_irq as usize;
        if vcpu_id >= self.dist.num_vcpus as usize {
            return false;
        }

        // Check if any enabled interrupt is pending
        for word in 0..(self.dist.num_irqs / 32) as usize {
            let pending = self.dist.irq_pending[vcpu_id][word];
            let enabled = self.dist.irq_enabled[vcpu_id][word];
            if (pending & enabled) != 0 {
                return true;
            }
        }

        false
    }

    /// Get pending interrupts for VCPU
    pub fn get_pending_irqs(&self, vcpu: &VgicVcpuState) -> u32 {
        let vcpu_id = vcpu.parent_irq as usize;
        if vcpu_id >= self.dist.num_vcpus as usize {
            return 0;
        }

        let mut pending_irqs = 0u32;
        for word in 0..(self.dist.num_irqs / 32).min(32) as usize {
            let pending = self.dist.irq_pending[vcpu_id][word];
            let enabled = self.dist.irq_enabled[vcpu_id][word];
            if (pending & enabled) != 0 {
                pending_irqs |= pending & enabled;
            }
        }

        pending_irqs
    }
}

/// Create default routing context
pub fn create_routing(num_vcpus: u32, num_irqs: u32, version: GicVersion) -> VgicRouting {
    VgicRouting::new(num_vcpus, num_irqs, version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_irq_type() {
        assert_eq!(IrqType::from_irq(0), IrqType::Sgi);
        assert_eq!(IrqType::from_irq(15), IrqType::Sgi);
        assert_eq!(IrqType::from_irq(16), IrqType::Ppi);
        assert_eq!(IrqType::from_irq(31), IrqType::Ppi);
        assert_eq!(IrqType::from_irq(32), IrqType::Spi);
        assert_eq!(IrqType::from_irq(1000), IrqType::Spi);
        assert_eq!(IrqType::from_irq(4096), IrqType::Lpi);
        assert_eq!(IrqType::from_irq(1020), IrqType::Unknown);
    }

    #[test]
    fn test_irq_type_properties() {
        assert!(IrqType::Sgi.is_per_cpu());
        assert!(IrqType::Ppi.is_per_cpu());
        assert!(IrqType::Spi.is_shared());
        assert!(IrqType::Lpi.is_shared());
    }

    #[test]
    fn test_distributor_state() {
        let dist = DistributorState::new(4, 256);
        assert_eq!(dist.num_vcpus, 4);
        assert_eq!(dist.num_irqs, 256);
        assert_eq!(dist.all_cpus_mask(), 0b1111);
    }

    #[test]
    fn test_set_enable_irq() {
        let mut dist = DistributorState::new(2, 256);

        // Test set enabled
        dist.set_enabled(32, 0b11);
        assert!(dist.test_enabled(32, 0b01));
        assert!(dist.test_enabled(32, 0b10));

        // Test clear enabled
        dist.clear_enabled(32, 0b01);
        assert!(!dist.test_enabled(32, 0b01));
        assert!(dist.test_enabled(32, 0b10));
    }

    #[test]
    fn test_set_pending_irq() {
        let mut dist = DistributorState::new(2, 256);

        // Test set pending
        dist.set_pending(32, 0b11);
        assert!(dist.test_pending(32, 0b01));
        assert!(dist.test_pending(32, 0b10));

        // Test clear pending
        dist.clear_pending(32, 0b01);
        assert!(!dist.test_pending(32, 0b01));
        assert!(dist.test_pending(32, 0b10));
    }

    #[test]
    fn test_irq_target() {
        let mut dist = DistributorState::new(4, 256);

        dist.set_target(32, 0b0101);
        assert_eq!(dist.get_target(32), 0b0101);
    }

    #[test]
    fn test_irq_priority() {
        let mut dist = DistributorState::new(2, 256);

        // SGI/PPI priority (per-CPU)
        dist.set_priority(0, 0, 10);
        assert_eq!(dist.get_priority(0, 0), 10);

        // SPI priority (shared)
        dist.set_priority(32, 0, 20);
        assert_eq!(dist.get_priority(32, 0), 20);
        assert_eq!(dist.get_priority(32, 1), 20);
    }

    #[test]
    fn test_irq_state() {
        let mut state = IrqState::new();

        assert!(!state.is_active(0b11));
        state.set_active(0b11);
        assert!(state.is_active(0b01));
        assert!(state.is_active(0b10));
        state.clear_active(0b01);
        assert!(!state.is_active(0b01));
        assert!(state.is_active(0b10));

        assert!(!state.is_edge_triggered());
        state.set_edge_trigger();
        assert!(state.is_edge_triggered());

        assert!(state.host_irq().is_none());
        state.set_host_irq(42);
        assert_eq!(state.host_irq(), Some(42));
    }

    #[test]
    fn test_routing_sgi() {
        let mut routing = VgicRouting::new(4, 256, GicVersion::V3);
        routing.enable();

        routing.route_sgi(0, 0, 0b0101).unwrap();
        assert!(routing.dist.test_pending(0, 0b0101));
        assert_eq!(routing.dist.sgi_source[0][0], 0b0101);
    }

    #[test]
    fn test_routing_spi() {
        let mut routing = VgicRouting::new(4, 256, GicVersion::V3);
        routing.enable();

        routing.route_spi(100, 0b1111).unwrap();
        assert!(routing.dist.test_pending(100, 0b1111));
        assert_eq!(routing.dist.get_target(100), 0b1111);
    }

    #[test]
    fn test_configure_irq() {
        let mut routing = VgicRouting::new(4, 256, GicVersion::V3);

        routing.configure_irq(100, 0b0101, true).unwrap();
        assert!(routing.dist.test_enabled(100, 0b0101));
        assert_eq!(routing.dist.get_target(100), 0b0101);
    }

    #[test]
    fn test_set_irq_priority() {
        let mut routing = VgicRouting::new(4, 256, GicVersion::V3);

        routing.set_irq_priority(100, 0, 15).unwrap();
        assert_eq!(routing.dist.get_priority(100, 0), 15);
    }

    #[test]
    fn test_set_irq_trigger() {
        let mut routing = VgicRouting::new(4, 256, GicVersion::V3);

        routing.set_irq_trigger(100, true).unwrap();
        assert!(routing.dist.get_irq_state(100).is_edge_triggered());

        routing.set_irq_trigger(100, false).unwrap();
        assert!(!routing.dist.get_irq_state(100).is_edge_triggered());
    }

    #[test]
    fn test_map_host_irq() {
        let mut routing = VgicRouting::new(4, 256, GicVersion::V3);

        routing.map_host_irq(100, 50).unwrap();
        assert_eq!(routing.get_host_irq(100), Some(50));
    }

    #[test]
    fn test_clear_irq_pending() {
        let mut routing = VgicRouting::new(4, 256, GicVersion::V3);
        routing.enable();

        routing.route_sgi(5, 0, 0b1111).unwrap();
        routing.clear_irq_pending(5, 0b0101);
        assert!(!routing.dist.test_pending(5, 0b0101));
        assert!(routing.dist.test_pending(5, 0b1010));
    }
}
