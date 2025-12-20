# Ferrovisor Directory Structure

This document describes the directory structure of Ferrovisor, which follows the xvisor project layout.

## Top-level Directories

```
ferrovisor/
├── arch/          # Architecture-specific code
├── core/          # Core hypervisor modules
├── drivers/       # Device drivers
├── emulators/     # Device emulators
├── libs/          # Common libraries
├── commands/      # Command-line tools
├── daemons/       # Background services
├── tests/         # Test suites
├── tools/         # Build and utility tools
├── docs/          # Documentation
├── src/           # Rust source code (main entry point)
├── Makefile       # Build system
├── openconf.cfg   # Build configuration
├── Cargo.toml     # Rust project configuration
└── README.md      # Project documentation
```

## Architecture (`arch/`)

Contains architecture-specific implementations:

```
arch/
├── arm64/         # ARM64 architecture
│   ├── board/     # Board-specific code
│   ├── cpu/       # CPU-specific implementations
│   ├── configs/   # Board configuration files
│   └── dts/       # Device tree files
├── riscv64/       # RISC-V 64-bit architecture
│   ├── cpu/       # CPU-specific implementations
│   └── configs/   # Platform configurations
├── x86_64/        # x86_64 architecture
│   ├── board/     # Board-specific code
│   ├── cpu/       # CPU-specific implementations
│   ├── configs/   # Platform configurations
│   └── guests/    # Guest OS support
└── common/        # Architecture-independent code
    └── include/   # Common headers
```

## Core (`core/`)

Contains the core hypervisor modules:

```
core/
├── ferro_main.rs      # Main hypervisor entry point
├── ferro_manager.rs   # VM management
├── ferro_vcpu.rs      # Virtual CPU management
├── ferro_scheduler.rs # Process/thread scheduling
├── mm/                # Memory management
├── irq/               # Interrupt handling
├── sync/              # Synchronization primitives
├── include/           # Core headers
├── vio/               # Virtual I/O
├── block/             # Block device support
├── net/               # Network stack
└── schedalgo/         # Scheduling algorithms
```

## Drivers (`drivers/`)

Device drivers organized by category:

```
drivers/
├── base/         # Driver framework
├── block/        # Block device drivers
├── net/          # Network device drivers
├── serial/       # Serial port drivers
├── virtio/       # VirtIO virtual device drivers
├── clk/          # Clock drivers
├── gpio/         # GPIO drivers
├── i2c/          # I2C bus drivers
├── input/        # Input device drivers
├── mmc/          # MMC/SD card drivers
├── mtd/          # Memory Technology Device drivers
├── pci/          # PCI bus drivers
├── rtc/          # Real-time clock drivers
├── spi/          # SPI bus drivers
├── usb/          # USB drivers
└── video/        # Display drivers
```

## Emulators (`emulators/`)

Device emulators for virtual hardware:

```
emulators/
├── uart/         # UART emulator
├── rtc/          # Real-time clock emulator
└── gpio/         # GPIO emulator
```

## Libraries (`libs/`)

Common libraries and utilities:

```
libs/
├── include/      # Library headers
├── crypto/       # Cryptographic functions
├── netstack/     # Network stack
└── vfs/          # Virtual File System
```

## Naming Conventions

To maintain consistency with xvisor:
- Core modules use `ferro_` prefix (e.g., `ferro_manager.rs`)
- Architecture files follow their respective conventions
- Configuration files use `.cfg` extension
- Build files follow Make conventions

## Module Dependencies

```
      src/
        |
    ┌───┴───┐
    arch   core
    │       │
    │     drivers
    │       │
    │   emulators
    │       │
    └─────libs/
```

## Build System

The build system follows xvisor's approach:
- `openconf.cfg` for build configuration
- Architecture-specific makefiles
- Modular compilation rules
- Cross-compilation support