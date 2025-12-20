# Makefile for Ferrovisor Hypervisor
#
# This makefile provides convenient targets for building,
# testing, and debugging the Ferrovisor hypervisor.

# Default target
.PHONY: all
all: build

# Build configuration
TARGET ?= aarch64-unknown-none-softfloat
PROFILE ?= debug
CARGO = cargo
TARGET_DIR = target/$(TARGET)/$(PROFILE)

# Rust flags
RUSTFLAGS = -C relocation-model=pie

# Feature flags
FEATURES = --features allocator,debug

# Build targets
.PHONY: build
build:
	$(CARGO) build $(FEATURES) --target $(TARGET) --profile $(PROFILE)

.PHONY: release
release:
	$(CARGO) build --release --features allocator $(FEATURES) --target $(TARGET)

# Clean targets
.PHONY: clean
clean:
	$(CARGO) clean

# Test targets
.PHONY: test
test:
	$(CARGO) test --lib --bins --features test

.PHONY: doc
doc:
	$(CARGO) doc --no-deps --features doc

# Check targets
.PHONY: check
check:
	$(CARGO) check --features allocator,debug --target $(TARGET)

.PHONY: clippy
clippy:
	$(CARGO) clippy --features allocator,debug --target $(TARGET)

# Format targets
.PHONY: fmt
fmt:
	$(CARGO) fmt

# Utility targets
.PHONY: size
size:
	$(CARGO) size --target $(TARGET) --bin ferrovisor

.PHONY: objdump
objdump: build
	llvm-objdump -d $(TARGET_DIR)/ferrovisor > ferrovisor.dump

.PHONY: nm
nm: build
	llvm-nm $(TARGET_DIR)/ferrovisor > ferrovisor.symbols

# Debug targets
.PHONY: gdb
gdb: build
	gdb $(TARGET_DIR)/ferrovisor

# QEMU targets
QEMU_ARGS ?= -M virt -cpu cortex-a57 -m 512M -nographic

.PHONY: run
run: build
	qemu-system-aarch64 $(QEMU_ARGS) -kernel $(TARGET_DIR)/ferrovisor

.PHONY: debug
debug: build
	qemu-system-aarch64 $(QEMU_ARGS) -kernel $(TARGET_DIR)/ferrovisor -s -S

.PHONY: run-riscv
run-riscv:
	$(MAKE) TARGET=riscv64-unknown-none-elf build
	qemu-system-riscv64 -M virt -m 512M -nographic -kernel target/riscv64-unknown-none-elf/debug/ferrovisor

# Help target
.PHONY: help
help:
	@echo "Ferrovisor Build System"
	@echo ""
	@echo "Targets:"
	@echo "  build      - Build the hypervisor (default)"
	@echo "  release    - Build optimized release version"
	@echo "  clean      - Clean build artifacts"
	@echo "  test       - Run tests"
	@echo "  doc        - Generate documentation"
	@echo "  check      - Check compilation without building"
	@echo "  clippy     - Run clippy lints"
	@echo "  fmt        - Format source code"
	@echo "  size       - Show binary size information"
	@echo "  objdump    - Disassemble the binary"
	@echo "  nm         - Show symbol table"
	@echo "  run        - Run in QEMU (ARM64)"
	@echo "  debug      - Debug in QEMU (ARM64)"
	@echo "  run-riscv  - Run in QEMU (RISC-V)"
	@echo "  help       - Show this help message"
	@echo ""
	@echo "Variables:"
	@echo "  TARGET     - Target triple (default: aarch64-unknown-none-softfloat)"
	@echo "  PROFILE    - Build profile (debug|release, default: debug)"
	@echo "  QEMU_ARGS  - Additional QEMU arguments"
	@echo ""
	@echo "Examples:"
	@echo "  make TARGET=riscv64-unknown-none-elf build"
	@echo "  make release"
	@echo "  make QEMU_ARGS=\"-M virt -smp 4\" run"

# CI target
.PHONY: ci
ci:
	$(MAKE) check
	$(MAKE) clippy
	$(MAKE) test