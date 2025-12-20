# RISC-V Hypervisor Extension Documentation Summary

This document provides a comprehensive overview of the RISC-V Hypervisor Extension (H-extension) documentation.

## Documentation Structure

```
docs/riscv/
├── README.md           # Overview of RISC-V virtualization
├── csrs.md             # Control and Status Registers
├── address-translation.md # Two-stage address translation
├── instructions.md     # Virtualization instructions
├── exceptions.md       # Traps and exception handling
└── SUMMARY.md          # This summary
```

## Quick Reference

### Core Concepts

| Concept | Description | Key Registers/Instructions |
|----------|-------------|----------------------------|
| Virtualization Mode | V=0 (non-virtualized) or V=1 (virtualized) | `hstatus`, `mstatus` |
| Privilege Modes | M/HS/U (V=0), VS/VU (V=1) | CSR access rules |
| Two-Stage Translation | GVA→GPA→SPA | `vsatp`, `hgatp` |
| VMID | Virtual Machine Identifier | `hgatp.VMID` |

### Essential CSRs

| CSR | Mode | Purpose |
|-----|------|---------|
| `hstatus` | HS | Hypervisor status and control |
| `hedeleg`/`hideleg` | HS | Exception/interrupt delegation |
| `hgatp` | HS | G-stage address translation |
| `vsstatus` | VS | Virtual supervisor status |
| `hvip`/`hip`/`hie` | HS | Virtual interrupt management |

### Key Instructions

| Instruction | Purpose | Valid Modes |
|-------------|---------|-------------|
| `HLV.*`/`HSV.*` | Virtual memory access | M/HS/U (if HU=1) |
| `HFENCE.VVMA` | VS-stage TLB fence | M/HS |
| `HFENCE.GVMA` | G-stage TLB fence | M/HS |
| `SRET` | Return from supervisor trap | All (behavior varies) |

## Implementation Checklist

### Minimum Requirements
- [ ] Base ISA RV32I or RV64I
- [ ] Page-based virtual memory (Sv32 for RV32, Sv39+ for RV64)
- [ ] Non-read-only `mtval` CSR
- [ ] Set bit 7 in `misa` to enable H-extension

### HS-mode Support
- [ ] Implement all HS-mode CSRs
- [ ] Support two-stage address translation
- [ ] Handle VS-level interrupts
- [ ] Implement virtual memory access instructions

### VS-mode Support
- [ ] Implement VS CSR replicas
- [ ] Support guest physical address translation
- [ ] Handle virtual-instruction exceptions
- [ ] Support nested virtualization if required

## Performance Considerations

### TLB Management
- Implement VMID tagging for isolation
- Support selective invalidation
- Consider TLB size for multi-VM workloads

### Page Table Structure
- 16 KiB root page tables for G-stage
- 2-bit address extension for guest physical addresses
- Efficient page table walking hardware

### Interrupt Handling
- Fast interrupt injection via `hvip`
- Guest external interrupt routing
- Minimal interrupt latency

## Security Features

### Isolation
- Hardware-enforced memory isolation
- VMID-based address space isolation
- Privilege level separation

### Protection
- Two-stage permission checking
- Physical memory protection integration
- Secure boot support compatibility

## Compatibility

### Software Compatibility
- Regular S-mode OS runs unmodified
- HS-mode OS with H-extension features
- VS-mode guests transparent to applications

### Hardware Compatibility
- Can be emulated on non-H hardware
- Graceful degradation without H-extension
- Nested virtualization support

## Related Extensions

### Recommended
- **Svadu**: Automatic A/D bit management
- **Svpbmt**: Memory type attributes
- **Zicfilp**: Landing pad prediction
- **Ssdbltrp**: Double trap detection

### Optional
- **Zicfiss**: Shadow stack support
- **Sstc**: Timer counter delegation
- **Sscofpmf**: Performance monitoring

## Debugging Support

### Debug CSRs
- Transformed instruction encoding
- Guest physical address reporting
- Virtualization mode tracking

### Exception Information
- Detailed fault information in `htval`
- Instruction transformation in `htinst`
- Virtualization mode in status bits

## Migration from Classic Virtualization

### Benefits
- Reduced VM exits
- Hardware-assisted memory management
- Standardized virtualization interface
- Better performance isolation

### Changes Needed
- Update hypervisor to use H-extension CSRs
- Modify guest exit handling
- Implement new trap delegation scheme
- Update memory management for two-stage translation

## Reference Implementation

### Open Source Projects
- [QEMU](https://www.qemu.org/) - Emulation support
- [KVM](https://www.kernel.org/doc/html/latest/virt/kvm/) - Kernel-based VM
- [FireMarshal](https://github.com/firemarshal/firemarshal) - RISC-V hypervisor framework

### Documentation Links
- [RISC-V ISA Manual](https://github.com/riscv/riscv-isa-manual)
- [RISC-V Privileged Spec](https://github.com/riscv/riscv-isa-manual/blob/main/src/privileged.adoc)
- [RISC-V Hypervisor Specification](https://github.com/riscv/riscv-isa-manual/blob/main/src/hypervisor.adoc)