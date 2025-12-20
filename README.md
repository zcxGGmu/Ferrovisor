# Ferrovisor

A Rust-based Type-1 Hypervisor inspired by [Xvisor](https://github.com/xvisor/xvisor).

## Overview

Ferrovisor is a project to rewrite the Xvisor hypervisor in Rust, providing memory safety and modern programming practices while maintaining the flexibility and performance of the original C implementation.

## Features

- **Type-1 Hypervisor**: Runs directly on hardware without requiring a host OS
- **Multi-Architecture Support**: ARM64, RISC-V 64-bit, and x86_64
- **Memory Safety**: Leverages Rust's ownership system for safe memory management
- **Modular Design**: Clean separation of concerns with modular architecture
- **Device Virtualization**: Support for VirtIO and hardware passthrough
- **Guest OS Support**: Run unmodified Linux and other operating systems

## Architecture

Ferrovisor is designed with a clear, modular architecture:

- **Core**: Virtual machine management, scheduling, memory management
- **Arch**: Architecture-specific code for different CPU types
- **Drivers**: Device drivers for hardware and virtual devices
- **Emulators**: Device emulators for virtual hardware
- **Utils**: Common utilities and helper functions

## Build Requirements

- Rust nightly toolchain
- GCC cross-compiler for target architectures
- QEMU for testing (optional)

### Setting up the Environment

1. Install Rust:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

2. Install cross-compilers:
```bash
# Ubuntu/Debian
sudo apt-get install gcc-aarch64-linux-gnu gcc-riscv64-linux-gnu gcc-x86-64-linux-gnu

# Or install from your distribution's package manager
```

3. Clone the repository:
```bash
git clone https://github.com/yourusername/ferrovisor.git
cd ferrovisor
```

## Building

Build for ARM64:
```bash
cargo build --target=aarch64-unknown-none-softfloat --features="arch_arm64"
```

Build for RISC-V 64-bit:
```bash
cargo build --target=riscv64gc-unknown-none-elf --features="arch_riscv64"
```

Build for x86_64:
```bash
cargo build --target=x86_64-unknown-none --features="arch_x86_64"
```

## Running

Using QEMU (ARM64 example):
```bash
qemu-system-aarch64 -M virt -cpu cortex-a57 -m 512M \
    -nographic -serial mon:stdio \
    -kernel target/aarch64-unknown-none-softfloat/debug/ferrovisor
```

## Project Status

This project is currently in early development. See the [REFACTOR_PLAN.md](REFACTOR_PLAN.md) for detailed progress and roadmap.

### Current Status

- [x] Project initialization and basic structure
- [ ] Core framework implementation
- [ ] Architecture-specific modules
- [ ] Virtual machine management
- [ ] Device emulation
- [ ] Memory management
- [ ] Interrupt handling
- [ ] Testing and documentation

## Contributing

We welcome contributions! Please see our contributing guidelines for more information.

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

This project is licensed under the GPL-2.0 License. See the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Inspired by the original [Xvisor](https://github.com/xvisor/xvisor) project
- Built with the Rust programming language
- Community feedback and contributions

## Contact

- Issues: Please use the GitHub issue tracker
- Discussions: Use GitHub discussions for questions and ideas

---

**Note**: This is a work in progress. Many features are not yet implemented.
