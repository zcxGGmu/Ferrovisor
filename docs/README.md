# Ferrovisor Documentation

This directory contains all documentation for the Ferrovisor hypervisor project.

## Documents

### Core Documentation

- **[REFACTOR_PLAN.md](./REFACTOR_PLAN.md)** - Detailed refactoring plan from xvisor to Rust
- **[STRUCTURE.md](./STRUCTURE.md)** - Directory structure and organization guide

### API Documentation

API documentation is generated automatically and can be built with:

```bash
make doc
# or
cargo doc --no-deps --features doc
```

The generated documentation will be available at `target/doc/ferrovisor/index.html`.

### Architecture Documentation

Architecture-specific documentation can be found in:
- `arch/arm64/docs/` - ARM64 architecture documentation
- `arch/riscv64/docs/` - RISC-V architecture documentation
- `arch/x86_64/docs/` - x86_64 architecture documentation

### Design Documents

Additional design documents and specifications will be added as the project evolves.

## Contributing to Documentation

When adding new documentation:
1. Use Markdown format (.md files)
2. Follow the existing style and structure
3. Include a table of contents for longer documents
4. Update this index file when adding new documents

## Document Categories

- **Planning** - Design plans and roadmaps
- **Architecture** - System architecture and design decisions
- **API** - API references and usage examples
- **Guides** - Tutorials and how-to guides
- **Examples** - Sample code and configurations