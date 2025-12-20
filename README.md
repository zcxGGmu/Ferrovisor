# Ferrovisor

A next-generation Type-1 Hypervisor written in Rust, designed for security, performance, and modularity.

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Key Features](#key-features)
- [Supported Architectures](#supported-architectures)
- [Quick Start](#quick-start)
- [Building](#building)
- [Configuration](#configuration)
- [Running](#running)
- [Project Structure](#project-structure)
- [Development Status](#development-status)
- [Contributing](#contributing)
- [License](#license)

## Overview

Ferrovisor is a modern Type-1 hypervisor implemented in Rust that runs directly on hardware, providing virtualization capabilities with a focus on memory safety, security, and performance. The project leverages Rust's ownership system and type safety to eliminate entire classes of vulnerabilities common in traditional hypervisors written in C.

## Architecture

```mermaid
graph TB
    subgraph "Hardware Layer"
        CPU[CPU Cores]
        MMU[Memory Management Unit]
        Devices[Physical Devices]
    end

    subgraph "Ferrovisor Hypervisor"
        subgraph "Core Components"
            VMM[Virtual Machine Manager]
            Scheduler[VCPU Scheduler]
            MemoryMgr[Memory Manager]
        end

        subgraph "Architecture Abstraction"
            Arch["Arch Layer<br/>ARM64 | RISC-V | x86_64"]
        end

        subgraph "Device Management"
            Drivers[Device Drivers]
            Emulators[Device Emulators]
            VirtIO[VirtIO Framework]
        end

        subgraph "Virtualization Support"
            HExt[H-Extension Support]
            TwoStage[Two-Stage Translation]
            TrapHandler[Trap Handler]
        end

        subgraph "Debug & Monitoring"
            Debug[Debug Support]
            Tracer[Event Tracer]
            Profiler[Performance Profiler]
        end
    end

    subgraph "Guest VMs"
        VM1["Guest VM 1<br/>Linux"]
        VM2["Guest VM 2<br/>RTOS"]
        VM3["Guest VM 3<br/>Bare-metal"]
    end

    CPU -.-> VMM
    MMU -.-> MemoryMgr
    Devices -.-> Drivers

    VMM --> VM1
    VMM --> VM2
    VMM --> VM3

    Arch --> VMM
    Arch --> HExt
    Arch --> TwoStage

    Drivers --> Emulators
    Drivers --> VirtIO

    MemoryMgr --> TwoStage
    VMM --> Scheduler
    Scheduler --> VM1
    Scheduler --> VM2
    Scheduler --> VM3

    TrapHandler --> HExt
    Debug --> VMM
    Tracer --> VMM
    Profiler --> Scheduler
```

## Key Features

### Core Hypervisor Capabilities
- **Type-1 Architecture**: Bare-metal hypervisor running directly on hardware
- **Multi-Guest Support**: Run multiple virtual machines simultaneously
- **Memory Safety**: Rust's ownership system prevents memory corruption vulnerabilities
- **High Performance**: Minimal overhead virtualization with hardware acceleration
- **Secure Isolation**: Strong isolation between guest VMs and the hypervisor

### Architecture Support
- **ARM64**: Full support for ARM virtualization extensions (ARMv8.1-A)
- **RISC-V 64-bit**: Complete H-extension implementation for hardware virtualization
- **x86_64**: Intel VT-x and AMD-V virtualization support

### Advanced Features
- **Hardware-Assisted Virtualization**: Leverages processor virtualization extensions
- **Nested Virtualization**: Support for running hypervisors within guest VMs
- **Live Migration**: Move running VMs between physical hosts
- **Device Passthrough**: Direct access to physical devices for guest VMs
- **VirtIO Support**: Standardized virtual I/O framework

### Developer Features
- **Comprehensive Debugging**: Hardware breakpoints, watchpoints, and tracing
- **Event Tracing**: Detailed system and VM event collection
- **Performance Profiling**: Built-in profiling and monitoring tools
- **Device Tree Support**: Dynamic hardware discovery and configuration

## Supported Architectures

### RISC-V 64-bit (Primary Focus)
- **H-Extension**: Complete hardware virtualization support
- **S-Mode**: Supervisor mode execution environment
- **M-Mode**: Machine mode hypervisor execution
- **SMP**: Multi-core virtualization support
- **Device Tree**: Hardware discovery and configuration
- **PLIC**: Platform-Level Interrupt Controller
- **CLINT**: Core-Local Interruptor for timers and IPIs

### ARM64
- **ARMv8.1-A Virtualization**: Hardware virtualization extensions
- **EL2**: Hypervisor Exception Level
- **VGIC**: Virtual Generic Interrupt Controller
- **GICv3**: Advanced interrupt controller support
- **SMMU**: System Memory Management Unit for I/O virtualization

### x86_64
- **Intel VT-x**: Hardware virtualization technology
- **AMD-V**: AMD virtualization extensions
- **EPT**: Extended Page Tables for memory virtualization
- **VMX**: Virtual Machine Extensions for CPU virtualization

## Quick Start

### Prerequisites

1. **Rust Toolchain** (nightly required):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default nightly
rustup component add rust-src
```

2. **Cross-compilers** for target architectures:
```bash
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install gcc-aarch64-linux-gnu gcc-riscv64-linux-gnu gcc-x86-64-linux-gnu

# macOS (with Homebrew)
brew install aarch64-elf-gcc riscv64-elf-gcc x86_64-elf-gcc
```

3. **Rust Targets**:
```bash
rustup target add aarch64-unknown-none-softfloat
rustup target add riscv64gc-unknown-none-elf
rustup target add x86_64-unknown-none
```

4. **QEMU** (for testing):
```bash
# Ubuntu/Debian
sudo apt-get install qemu-system-aarch64 qemu-system-riscv64 qemu-system-x86

# macOS
brew install qemu
```

### Building

```bash
# Clone the repository
git clone https://github.com/yourusername/ferrovisor.git
cd ferrovisor

# Build for RISC-V 64-bit (primary target)
make TARGET=riscv64gc-unknown-none-elf

# Build for ARM64
make TARGET=aarch64-unknown-none-softfloat

# Build for x86_64
make TARGET=x86_64-unknown-none

# Build with debug features
make FEATURES="--features debug,verbose"

# Build release version
make release
```

## Configuration

Ferrovisor supports extensive configuration through build-time features and runtime configuration files:

### Build-time Features
```bash
# Enable debugging support
--features debug

# Enable performance monitoring
--features pmu

# Enable tracing
--features trace

# Enable allocator support
--features allocator

# Verbose logging
--features verbose
```

### Runtime Configuration
The hypervisor can be configured through:
- Device Tree passed at boot time
- Configuration files in the firmware
- Command-line parameters via boot loader

## Running

### RISC-V in QEMU
```bash
qemu-system-riscv64 -M virt -cpu rv64 -smp 4 -m 2G \
    -nographic -serial mon:stdio \
    -bios none -kernel target/riscv64gc-unknown-none-elf/debug/ferrovisor \
    -device virtio-blk-device,drive=guest.img,if=none
```

### ARM64 in QEMU
```bash
qemu-system-aarch64 -M virt -cpu cortex-a57 -smp 4 -m 2G \
    -nographic -serial mon:stdio \
    -bios none -kernel target/aarch64-unknown-none-softfloat/debug/ferrovisor
```

### x86_64 in QEMU
```bash
qemu-system-x86_64 -M pc -cpu host -smp 4 -m 2G \
    -nographic -serial mon:stdio \
    -kernel target/x86_64-unknown-none/debug/ferrovisor
```

## Project Structure

```
ferrovisor/
├── src/
│   ├── arch/                  # Architecture-specific code
│   │   ├── riscv64/          # RISC-V 64-bit implementation
│   │   │   ├── cpu/         # CPU management
│   │   │   ├── mmu/         # Memory management unit
│   │   │   ├── interrupt/   # Interrupt handling
│   │   │   ├── virtualization/ # H-extension support
│   │   │   ├── smp/         # Symmetric multiprocessing
│   │   │   ├── devtree/     # Device tree support
│   │   │   ├── debug/       # Debug support
│   │   │   └── platform/    # Platform-specific code
│   │   ├── aarch64/          # ARM64 implementation
│   │   └── x86_64/           # x86_64 implementation
│   ├── core/                  # Core hypervisor components
│   │   ├── vm/              # Virtual machine management
│   │   ├── vcpu/            # Virtual CPU management
│   │   ├── memory/          # Memory management
│   │   ├── scheduler/       # VCPU scheduling
│   │   └── interrupt/       # Interrupt management
│   ├── drivers/              # Device drivers
│   │   ├── virtio/          # VirtIO framework
│   │   ├── block/           # Block device drivers
│   │   ├── network/         # Network device drivers
│   │   └── console/         # Console drivers
│   ├── emulator/             # Device emulators
│   ├── libs/                  # Common libraries
│   └── utils/                 # Utility functions
├── docs/                      # Documentation
├── scripts/                   # Build and utility scripts
├── tests/                     # Tests and benchmarks
└── tools/                     # Development tools
```

## Development Status

### Completed Components ✅

#### Core Hypervisor
- [x] Virtual machine lifecycle management
- [x] VCPU creation and destruction
- [x] Memory management and protection
- [x] Interrupt handling and distribution
- [x] VCPU scheduling algorithms
- [x] Resource allocation and isolation

#### RISC-V Architecture
- [x] CPU register and CSR management
- [x] MMU with Sv39/Sv48 paging
- [x] Interrupt and exception handling
- [x] H-extension virtualization support
- [x] SMP (Symmetric Multiprocessing)
- [x] Device tree parsing and manipulation
- [x] Debug support (breakpoints, tracing)
- [x] Platform configuration and drivers

#### Device Support
- [x] VirtIO framework implementation
- [x] Block device virtualization
- [x] Network device virtualization
- [x] Console and serial port support
- [x] Timer and clock management

### In Progress 🚧

- [ ] ARM64 architecture support
- [ ] x86_64 architecture support
- [ ] Live migration implementation
- [ ] Dynamic VM creation/destruction
- [ ] Comprehensive test suite
- [ ] Performance optimization

### Planned Features 📋

- [ ] Nested virtualization
- [ ] GPU virtualization
- [ ] NUMA awareness
- [ ] Security module integration
- [ ] Management API
- [ ] Web-based management interface

## Contributing

We welcome contributions from the community! Here's how you can help:

### Ways to Contribute
1. **Code Contributions**: Implement new features or fix bugs
2. **Documentation**: Improve documentation and write tutorials
3. **Testing**: Write tests and report bugs
4. **Performance**: Optimize critical paths
5. **Architecture**: Design new components and improvements

### Development Workflow
1. Fork the repository
2. Create a feature branch: `git checkout -b feature-name`
3. Make your changes with proper commits
4. Add tests for new functionality
5. Ensure all tests pass: `make test`
6. Submit a pull request with a clear description

### Code Style
- Follow Rust's standard formatting: `cargo fmt`
- Use clippy for linting: `cargo clippy -- -D warnings`
- Document public APIs with rustdoc comments
- Write unit tests for all modules

## License

This project is licensed under the GNU General Public License v2.0. See the [LICENSE](LICENSE) file for details.

## Contact

- **Issues**: Report bugs and feature requests on [GitHub Issues](https://github.com/yourusername/ferrovisor/issues)
- **Discussions**: Use [GitHub Discussions](https://github.com/yourusername/ferrovisor/discussions) for questions
- **Email**: ferrovisor@example.com

---

**Note**: Ferrovisor is actively being developed. While the RISC-V implementation is largely complete, some features are still in progress. Check the development status above for current capabilities.