//! Build script for Ferrovisor
//!
//! This script configures the build based on target architecture and features

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=TARGET");

    // Get target triple
    let target = env::var("TARGET").unwrap_or_else(|_| {
        "aarch64-unknown-none-softfloat".to_string()
    });

    // Configure based on target
    configure_architecture(&target);

    // Generate config file
    generate_config(&target);

    // Setup linker script
    setup_linker_script(&target);
}

fn configure_architecture(target: &str) {
    match target {
        t if t.contains("aarch64") => {
            println!("cargo:rustc-cfg=arch=\"aarch64\"");
            println!("cargo:rustc-cfg(target_arch=\"aarch64\")");

            // AArch64 specific flags
            println!("cargo:rustc-link-arg=-Tlink-aarch64.ld");
            println!("cargo:rustc-link-arg=--no-warn-mismatch");
        }
        t if t.contains("riscv64") => {
            println!("cargo:rustc-cfg=arch=\"riscv64\"");
            println!("cargo:rustc-cfg(target_arch=\"riscv64\")");

            // RISC-V specific flags
            println!("cargo:rustc-link-arg=-Tlink-riscv64.ld");
            println!("cargo:rustc-link-arg=-nostartfiles");
        }
        t if t.contains("x86_64") => {
            println!("cargo:rustc-cfg=arch=\"x86_64\"");
            println!("cargo:rustc-cfg(target_arch=\"x86_64\")");

            // x86_64 specific flags
            println!("cargo:rustc-link-arg=-Tlink-x86_64.ld");
            println!("cargo:rustc-link-arg=-nostartfiles");
        }
        _ => {
            panic!("Unsupported target architecture: {}", target);
        }
    }

    // Common flags
    println!("cargo:rustc-link-arg=-Wl,--gc-sections");

    // Feature-based configuration
    if cfg!(feature = "debug") {
        println!("cargo:rustc-cfg(debug)");
    }

    if cfg!(feature = "verbose") {
        println!("cargo:rustc-cfg(verbose)");
    }

    if cfg!(feature = "allocator") {
        println!("cargo:rustc-cfg(feature_allocator)");
    }
}

fn generate_config(target: &str) {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("config.rs");

    let mut config = String::new();
    config.push_str("// Auto-generated configuration file\n\n");

    // Architecture configuration
    if target.contains("aarch64") {
        config.push_str("#![cfg(target_arch = \"aarch64\")]\n");
        config.push_str("pub const ARCH: &str = \"aarch64\";\n");
        config.push_str("pub const PAGE_SIZE: usize = 4096;\n");
        config.push_str("pub const PAGE_SHIFT: usize = 12;\n");
        config.push_str("pub const VCPU_STACK_SIZE: usize = 8 * 1024; // 8KB\n");
    } else if target.contains("riscv64") {
        config.push_str("#![cfg(target_arch = \"riscv64\")]\n");
        config.push_str("pub const ARCH: &str = \"riscv64\";\n");
        config.push_str("pub const PAGE_SIZE: usize = 4096;\n");
        config.push_str("pub const PAGE_SHIFT: usize = 12;\n");
        config.push_str("pub const VCPU_STACK_SIZE: usize = 8 * 1024; // 8KB\n");
    } else if target.contains("x86_64") {
        config.push_str("#![cfg(target_arch = \"x86_64\")]\n");
        config.push_str("pub const ARCH: &str = \"x86_64\";\n");
        config.push_str("pub const PAGE_SIZE: usize = 4096;\n");
        config.push_str("pub const PAGE_SHIFT: usize = 12;\n");
        config.push_str("pub const VCPU_STACK_SIZE: usize = 16 * 1024; // 16KB\n");
    }

    // Feature flags
    config.push_str("\n// Feature configuration\n");
    config.push_str(&format!("pub const ENABLE_ALLOCATOR: bool = {};\n",
                           cfg!(feature = "allocator")));
    config.push_str(&format!("pub const DEBUG: bool = {};\n",
                           cfg!(feature = "debug")));
    config.push_str(&format!("pub const VERBOSE: bool = {};\n",
                           cfg!(feature = "verbose")));

    // Common constants
    config.push_str("\n// Common constants\n");
    config.push_str("pub const MAX_VCPUS: usize = 8;\n");
    config.push_str("pub const MAX_GUESTS: usize = 4;\n");
    config.push_str("pub const TIMER_FREQ: u64 = 1000; // 1ms\n");

    fs::write(&dest_path, config).unwrap();
    println!("cargo:rerun-if-changed={}", dest_path.display());
}

fn setup_linker_script(target: &str) {
    let out_dir = env::var("OUT_DIR").unwrap();
    let linker_script = match target {
        t if t.contains("aarch64") => {
            generate_aarch64_linker_script(&out_dir);
            "link-aarch64.ld"
        }
        t if t.contains("riscv64") => {
            generate_riscv64_linker_script(&out_dir);
            "link-riscv64.ld"
        }
        t if t.contains("x86_64") => {
            generate_x86_64_linker_script(&out_dir);
            "link-x86_64.ld"
        }
        _ => unreachable!(),
    };

    // Copy linker script to output directory
    let src = Path::new(&out_dir).join(linker_script);
    let dst = Path::new(&out_dir).join("linker.ld");
    fs::copy(src, dst).unwrap();
}

fn generate_aarch64_linker_script(out_dir: &str) -> PathBuf {
    let script = r#"
/* AArch64 linker script for Ferrovisor */
ENTRY(_start)

/* Define memory regions */
MEMORY {
    RAM (rwx) : ORIGIN = 0x40080000, LENGTH = 512M  /* Start at 2MB + 512KB */
}

/* Section definitions */
SECTIONS {
    /* Code section */
    .text : {
        KEEP(*(.text.entry))   /* Entry point first */
        *(.text .text.*)
        *(.rodata .rodata.*)
    } > RAM

    /* Initialized data */
    .data : ALIGN(4096) {
        __data_start = .;
        *(.data .data.*)
        *(.sdata .sdata.*)
        __data_end = .;
    } > RAM

    /* Uninitialized data */
    .bss : ALIGN(4096) {
        __bss_start = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        *(COMMON)
        __bss_end = .;
    } > RAM

    /* Stack for each CPU */
    .stack : ALIGN(4096) {
        __stack_start = .;
        . = . + 64 * 1024;  /* 64KB stack per CPU */
        __stack_end = .;
    } > RAM

    /* Heap */
    .heap : ALIGN(4096) {
        __heap_start = .;
        __heap_end = ORIGIN(RAM) + LENGTH(RAM);
    } > RAM

    /* Discard unused sections */
    /DISCARD/ : {
        *(.eh_frame)
        *(.comment)
        *(.note*)
    }
}
"#;

    let path = Path::new(out_dir).join("link-aarch64.ld");
    fs::write(&path, script).unwrap();
    path
}

fn generate_riscv64_linker_script(out_dir: &str) -> PathBuf {
    let script = r#"
/* RISC-V 64-bit linker script for Ferrovisor */
ENTRY(_start)

/* Define memory regions */
MEMORY {
    RAM (rwx) : ORIGIN = 0x80200000, LENGTH = 512M  /* Start at 128MB */
}

/* Section definitions */
SECTIONS {
    /* Code section */
    .text : {
        KEEP(*(.text.entry))   /* Entry point first */
        *(.text .text.*)
        *(.rodata .rodata.*)
    } > RAM

    /* Initialized data */
    .data : ALIGN(4096) {
        __data_start = .;
        *(.data .data.*)
        *(.sdata .sdata.*)
        __data_end = .;
    } > RAM

    /* Uninitialized data */
    .bss : ALIGN(4096) {
        __bss_start = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        *(COMMON)
        __bss_end = .;
    } > RAM

    /* Stack for each CPU */
    .stack : ALIGN(4096) {
        __stack_start = .;
        . = . + 64 * 1024;  /* 64KB stack per CPU */
        __stack_end = .;
    } > RAM

    /* Heap */
    .heap : ALIGN(4096) {
        __heap_start = .;
        __heap_end = ORIGIN(RAM) + LENGTH(RAM);
    } > RAM

    /* Discard unused sections */
    /DISCARD/ : {
        *(.eh_frame)
        *(.comment)
        *(.note*)
    }
}
"#;

    let path = Path::new(out_dir).join("link-riscv64.ld");
    fs::write(&path, script).unwrap();
    path
}

fn generate_x86_64_linker_script(out_dir: &str) -> PathBuf {
    let script = r#"
/* x86_64 linker script for Ferrovisor */
ENTRY(_start)

/* Define memory regions */
MEMORY {
    RAM (rwx) : ORIGIN = 0x100000, LENGTH = 512M  /* Start at 1MB */
}

/* Section definitions */
SECTIONS {
    /* Code section */
    .text : {
        KEEP(*(.text.entry))   /* Entry point first */
        *(.text .text.*)
        *(.rodata .rodata.*)
    } > RAM

    /* Initialized data */
    .data : ALIGN(4096) {
        __data_start = .;
        *(.data .data.*)
        __data_end = .;
    } > RAM

    /* Uninitialized data */
    .bss : ALIGN(4096) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    } > RAM

    /* Stack for each CPU */
    .stack : ALIGN(4096) {
        __stack_start = .;
        . = . + 64 * 1024;  /* 64KB stack per CPU */
        __stack_end = .;
    } > RAM

    /* Heap */
    .heap : ALIGN(4096) {
        __heap_start = .;
        __heap_end = ORIGIN(RAM) + LENGTH(RAM);
    } > RAM

    /* Discard unused sections */
    /DISCARD/ : {
        *(.eh_frame)
        *(.comment)
        *(.note*)
    }
}
"#;

    let path = Path::new(out_dir).join("link-x86_64.ld");
    fs::write(&path, script).unwrap();
    path
}