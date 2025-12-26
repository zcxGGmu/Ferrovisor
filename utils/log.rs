//! Simple logging implementation for Ferrovisor
//!
//! This module provides a minimal logging implementation suitable
//! for a no_std hypervisor environment.

use core::fmt;
use crate::utils::console;

/// Log levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Error level
    Error = 0,
    /// Warning level
    Warn = 1,
    /// Info level
    Info = 2,
    /// Debug level
    Debug = 3,
    /// Trace level
    Trace = 4,
}

impl Level {
    /// Convert level to string
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        }
    }
}

/// Get the current log level
pub fn level() -> Level {
    #[cfg(feature = "debug")]
    {
        #[cfg(feature = "verbose")]
        return Level::Trace;

        Level::Debug
    }

    #[cfg(not(feature = "debug"))]
    Level::Info
}

/// Set the log level
pub fn set_level(level: Level) {
    // TODO: Implement log level setting
    // For now, compile-time only
}

/// Log a message
pub fn log(level: Level, args: fmt::Arguments<'_>) {
    if level <= level() {
        let _timestamp = crate::utils::get_timestamp();

        // TODO: Implement console output
        // Format: [TIMESTAMP] [LEVEL] message
        // console::print!("[{:016x}] [{}] ", timestamp, level.as_str());
        // console::print_fmt(args);
        // console::print!("\n");
        let _ = args; // Suppress unused warning
    }
}

/// Log an error message
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::utils::log::log(
            $crate::utils::log::Level::Error,
            format_args!($($arg)*)
        );
    };
}

/// Log a warning message
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::utils::log::log(
            $crate::utils::log::Level::Warn,
            format_args!($($arg)*)
        );
    };
}

/// Log an info message
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::utils::log::log(
            $crate::utils::log::Level::Info,
            format_args!($($arg)*)
        );
    };
}

/// Log a debug message
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::utils::log::log(
            $crate::utils::log::Level::Debug,
            format_args!($($arg)*)
        );
    };
}

/// Log a trace message
#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        $crate::utils::log::log(
            $crate::utils::log::Level::Trace,
            format_args!($($arg)*)
        );
    };
}