# Ferrovisor Documentation Index

Welcome to the Ferrovisor documentation. This index helps you navigate through all available documentation.

## Quick Start

- [Getting Started](../README.md) - Main project README with build instructions
- [Directory Structure](./STRUCTURE.md) - Understanding the code organization
- [Refactoring Plan](./REFACTOR_PLAN.md) - Project development roadmap

## Core Documentation

### Design and Architecture
- [Architecture Overview](./STRUCTURE.md#architecture) - System architecture details
- [Memory Management](../core/mm/) - Memory management design
- [Scheduling System](../core/sched/) - Process and VCPU scheduling
- [Interrupt Handling](../core/irq/) - Interrupt and exception management

### Development Guides
- [Building the Project](../README.md#building) - Compilation instructions
- [Contributing](../README.md#contributing) - How to contribute to Ferrovisor
- [Coding Standards](./CODING_STANDARDS.md) - Code style guidelines (TODO)
- [Testing Guide](./TESTING.md) - Testing framework and procedures (TODO)

## API Reference

Generated API documentation is available by running:
```bash
make doc
```

## Architecture-Specific Documentation

- [ARM64](../arch/arm64/) - ARM64-specific documentation
- [RISC-V](../arch/riscv64/) - RISC-V-specific documentation
- [x86_64](../arch/x86_64/) - x86_64-specific documentation

## Modules

### Core Modules
- [Virtual Machine Manager](../core/vmm/) - VM and VCPU management
- [Device Drivers](../drivers/) - Driver framework and implementations
- [Device Emulators](../emulators/) - Virtual device emulations
- [Utilities](../utils/) - Common utilities and helpers

### Driver Categories
- [Base Drivers](../drivers/base/) - Core driver framework
- [Platform Drivers](../drivers/platform/) - Platform-specific drivers
- [VirtIO Drivers](../drivers/virtio/) - Virtual I/O device drivers

## External Resources

- [Xvisor Project](https://github.com/xvisor/xvisor) - Original C implementation
- [Rust Documentation](https://doc.rust-lang.org/) - Rust language documentation
- [ARM Architecture Reference Manual](https://developer.arm.com/documentation)
- [RISC-V International](https://riscv.org/) - RISC-V specifications

## Feedback and Contributions

For questions, issues, or contributions:
- Use GitHub Issues for bug reports
- Use GitHub Discussions for questions
- Submit Pull Requests for contributions