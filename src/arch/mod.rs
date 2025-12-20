//! Architecture support

#[cfg(target_arch = "aarch64")]
pub mod arm64;

#[cfg(target_arch = "riscv64")]
pub mod riscv64;

#[cfg(target_arch = "x86_64")]
pub mod x86_64;

pub mod common;

#[cfg(target_arch = "aarch64")]
pub use arm64::*;

#[cfg(target_arch = "riscv64")]
pub use riscv64::*;

#[cfg(target_arch = "x86_64")]
pub use x86_64::*;
