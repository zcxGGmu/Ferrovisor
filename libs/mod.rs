//! Common libraries module
//!
//! Provides common utility libraries used throughout the hypervisor.

use crate::{Error, Result};

/// Initialize common libraries
pub fn init() -> Result<()> {
    log::info!("Initializing common libraries");

    // Initialize string library
    init_string_lib()?;

    // Initialize math library
    init_math_lib()?;

    // Initialize data structures library
    init_data_structures_lib()?;

    log::info!("Common libraries initialized successfully");
    Ok(())
}

/// Initialize string library
fn init_string_lib() -> Result<()> {
    log::debug!("Initializing string library");
    // TODO: Initialize string manipulation functions
    Ok(())
}

/// Initialize math library
fn init_math_lib() -> Result<()> {
    log::debug!("Initializing math library");
    // TODO: Initialize mathematical functions
    Ok(())
}

/// Initialize data structures library
fn init_data_structures_lib() -> Result<()> {
    log::debug!("Initializing data structures library");
    // TODO: Initialize common data structures
    Ok(())
}

/// Common string manipulation utilities
pub mod string {
    /// Safe string length calculation
    pub fn strlen(s: &str) -> usize {
        s.len()
    }

    /// Safe string copy
    pub fn strcpy(dst: &mut [u8], src: &str) -> Result<usize, ()> {
        let src_bytes = src.as_bytes();
        if src_bytes.len() > dst.len() {
            return Err(());
        }

        dst[..src_bytes.len()].copy_from_slice(src_bytes);
        Ok(src_bytes.len())
    }

    /// Safe string concatenation
    pub fn strcat(dst: &mut [u8], src: &str) -> Result<usize, ()> {
        // Find null terminator or end of buffer
        let mut len = 0;
        while len < dst.len() && dst[len] != 0 {
            len += 1;
        }

        let src_bytes = src.as_bytes();
        if len + src_bytes.len() > dst.len() {
            return Err(());
        }

        dst[len..len + src_bytes.len()].copy_from_slice(src_bytes);
        Ok(len + src_bytes.len())
    }
}

/// Common mathematical utilities
pub mod math {
    /// Calculate the greatest common divisor
    pub fn gcd(a: u64, b: u64) -> u64 {
        if b == 0 {
            a
        } else {
            gcd(b, a % b)
        }
    }

    /// Calculate the least common multiple
    pub fn lcm(a: u64, b: u64) -> u64 {
        if a == 0 || b == 0 {
            0
        } else {
            (a / gcd(a, b)) * b
        }
    }

    /// Round up to the nearest power of 2
    pub fn round_up_pow2(n: u64) -> u64 {
        if n <= 1 {
            1
        } else {
            1 << (64 - n.leading_zeros())
        }
    }

    /// Check if a number is a power of 2
    pub fn is_power_of_two(n: u64) -> bool {
        n != 0 && (n & (n - 1)) == 0
    }
}

/// Common data structures
pub mod data_structures {
    use core::ptr::NonNull;

    /// Simple linked list node
    #[derive(Debug)]
    pub struct ListNode<T> {
        pub data: T,
        pub next: Option<NonNull<ListNode<T>>>,
    }

    impl<T> ListNode<T> {
        pub fn new(data: T) -> Self {
            Self {
                data,
                next: None,
            }
        }

        pub fn append(&mut self, node: NonNull<ListNode<T>>) {
            unsafe {
                (*node.as_ptr()).next = self.next;
                self.next = Some(node);
            }
        }
    }

    /// Simple ring buffer implementation
    #[derive(Debug)]
    pub struct RingBuffer<T> {
        buffer: *mut T,
        capacity: usize,
        head: usize,
        tail: usize,
        count: usize,
    }

    impl<T> RingBuffer<T> {
        pub fn new(buffer: *mut T, capacity: usize) -> Self {
            Self {
                buffer,
                capacity,
                head: 0,
                tail: 0,
                count: 0,
            }
        }

        pub fn push(&mut self, item: T) -> Result<(), ()> {
            if self.count >= self.capacity {
                return Err(());
            }

            unsafe {
                *self.buffer.add(self.tail) = item;
                self.tail = (self.tail + 1) % self.capacity;
                self.count += 1;
            }

            Ok(())
        }

        pub fn pop(&mut self) -> Result<T, ()> {
            if self.count == 0 {
                return Err(());
            }

            unsafe {
                let item = core::ptr::read(self.buffer.add(self.head));
                self.head = (self.head + 1) % self.capacity;
                self.count -= 1;
                Ok(item)
            }
        }

        pub fn is_empty(&self) -> bool {
            self.count == 0
        }

        pub fn is_full(&self) -> bool {
            self.count >= self.capacity
        }

        pub fn len(&self) -> usize {
            self.count
        }

        pub fn capacity(&self) -> usize {
            self.capacity
        }
    }

    impl<T> Drop for RingBuffer<T> {
        fn drop(&mut self) {
            // Note: We don't deallocate the buffer here as it's managed externally
            while let Ok(_) = self.pop() {
                // Drop all remaining items
            }
        }
    }
}

/// Library error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibError {
    /// Invalid argument
    InvalidArgument,
    /// Buffer overflow
    BufferOverflow,
    /// Out of memory
    OutOfMemory,
    /// Division by zero
    DivisionByZero,
}

impl From<LibError> for Error {
    fn from(err: LibError) -> Self {
        Error::CoreError(crate::core::Error::LibError(err))
    }
}