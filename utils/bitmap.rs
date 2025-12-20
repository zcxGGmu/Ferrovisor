//! Simple bitmap implementation
//!
//! This module provides a bitmap implementation suitable for
//! tracking allocation of frames, IDs, and other resources.

/// Bitmap structure
pub struct Bitmap {
    /// Bitmap data
    data: *mut u64,
    /// Number of bits
    bits: usize,
    /// Number of u64 words
    words: usize,
}

impl Bitmap {
    /// Create a new bitmap
    ///
    /// # Safety
    /// The caller must ensure the memory at `data` is valid and
    /// large enough to hold `bits` bits.
    pub unsafe fn new(data: *mut u64, bits: usize) -> Self {
        let words = (bits + 63) / 64;
        // Clear the bitmap
        for i in 0..words {
            core::ptr::write_volatile(data.add(i), 0);
        }
        Self { data, bits, words }
    }

    /// Create a bitmap from a slice
    pub fn from_slice(slice: &mut [u64]) -> Self {
        let bits = slice.len() * 64;
        // Clear the slice
        for word in slice.iter_mut() {
            *word = 0;
        }
        Self {
            data: slice.as_mut_ptr(),
            bits,
            words: slice.len(),
        }
    }

    /// Get the number of bits
    pub fn bits(&self) -> usize {
        self.bits
    }

    /// Test if a bit is set
    pub fn test(&self, index: usize) -> bool {
        if index >= self.bits {
            return false;
        }
        let word = index / 64;
        let bit = index % 64;
        unsafe {
            let value = core::ptr::read_volatile(self.data.add(word));
            (value >> bit) & 1 == 1
        }
    }

    /// Set a bit
    pub fn set(&mut self, index: usize, value: bool) -> bool {
        if index >= self.bits {
            return false;
        }
        let word = index / 64;
        let bit = index % 64;
        unsafe {
            let current = core::ptr::read_volatile(self.data.add(word));
            let new_value = if value { current | (1 << bit) } else { current & !(1 << bit) };
            core::ptr::write_volatile(self.data.add(word), new_value);
        }
        true
    }

    /// Set a bit to 1
    pub fn set_bit(&mut self, index: usize) -> bool {
        self.set(index, true)
    }

    /// Clear a bit to 0
    pub fn clear_bit(&mut self, index: usize) -> bool {
        self.set(index, false)
    }

    /// Find the first zero bit
    pub fn find_first_zero(&self) -> Option<usize> {
        for word_idx in 0..self.words {
            unsafe {
                let value = core::ptr::read_volatile(self.data.add(word_idx));
                if value != u64::MAX {
                    // Found a word with a zero bit
                    let bit = (!value).trailing_zeros() as usize;
                    let index = word_idx * 64 + bit;
                    if index < self.bits {
                        return Some(index);
                    }
                }
            }
        }
        None
    }

    /// Find and set the first zero bit
    pub fn find_and_set(&mut self) -> Option<usize> {
        if let Some(index) = self.find_first_zero() {
            self.set_bit(index);
            Some(index)
        } else {
            None
        }
    }

    /// Find the first set bit
    pub fn find_first_set(&self) -> Option<usize> {
        for word_idx in 0..self.words {
            unsafe {
                let value = core::ptr::read_volatile(self.data.add(word_idx));
                if value != 0 {
                    // Found a word with a set bit
                    let bit = value.trailing_zeros() as usize;
                    let index = word_idx * 64 + bit;
                    if index < self.bits {
                        return Some(index);
                    }
                }
            }
        }
        None
    }

    /// Find and clear the first set bit
    pub fn find_and_clear(&mut self) -> Option<usize> {
        if let Some(index) = self.find_first_set() {
            self.clear_bit(index);
            Some(index)
        } else {
            None
        }
    }

    /// Count the number of set bits
    pub fn count_ones(&self) -> usize {
        let mut count = 0;
        for word_idx in 0..self.words {
            unsafe {
                let value = core::ptr::read_volatile(self.data.add(word_idx));
                count += value.count_ones() as usize;
            }
        }
        count
    }

    /// Count the number of zero bits
    pub fn count_zeros(&self) -> usize {
        self.bits - self.count_ones()
    }

    /// Check if all bits are set
    pub fn all(&self) -> bool {
        self.count_zeros() == 0
    }

    /// Check if any bit is set
    pub fn any(&self) -> bool {
        self.count_ones() > 0
    }

    /// Check if no bits are set
    pub fn none(&self) -> bool {
        self.count_ones() == 0
    }

    /// Set all bits
    pub fn set_all(&mut self) {
        for word_idx in 0..self.words {
            unsafe {
                core::ptr::write_volatile(self.data.add(word_idx), u64::MAX);
            }
        }
    }

    /// Clear all bits
    pub fn clear_all(&mut self) {
        for word_idx in 0..self.words {
            unsafe {
                core::ptr::write_volatile(self.data.add(word_idx), 0);
            }
        }
    }

    /// Get a slice of the bitmap data
    ///
    /// # Safety
    /// The returned slice is valid only as long as the bitmap exists.
    pub unsafe fn as_slice(&self) -> &[u64] {
        core::slice::from_raw_parts(self.data, self.words)
    }

    /// Get a mutable slice of the bitmap data
    ///
    /// # Safety
    /// The returned slice is valid only as long as the bitmap exists.
    pub unsafe fn as_mut_slice(&mut self) -> &mut [u64] {
        core::slice::from_raw_parts_mut(self.data, self.words)
    }
}

unsafe impl Send for Bitmap {}
unsafe impl Sync for Bitmap {}

/// Iterator over set bits in the bitmap
pub struct Iter<'a> {
    bitmap: &'a Bitmap,
    current_word: usize,
    current_bit: usize,
}

impl<'a> Bitmap {
    /// Create an iterator over set bits
    pub fn iter(&'a self) -> Iter<'a> {
        Iter {
            bitmap: self,
            current_word: 0,
            current_bit: 0,
        }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current_word < self.bitmap.words {
            unsafe {
                let value = core::ptr::read_volatile(self.bitmap.data.add(self.current_word));

                // Skip bits we've already processed in this word
                let masked = value >> self.current_bit;

                if masked != 0 {
                    let bit = masked.trailing_zeros() as usize;
                    let index = self.current_word * 64 + self.current_bit + bit;
                    if index < self.bitmap.bits {
                        self.current_bit += bit + 1;
                        return Some(index);
                    }
                }

                self.current_word += 1;
                self.current_bit = 0;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitmap_basic() {
        let mut data = [0u64; 2];
        let mut bitmap = Bitmap::from_slice(&mut data);

        // Test initial state
        assert_eq!(bitmap.count_ones(), 0);
        assert!(bitmap.none());
        assert!(!bitmap.any());

        // Set some bits
        bitmap.set_bit(0);
        bitmap.set_bit(1);
        bitmap.set_bit(64);

        assert_eq!(bitmap.count_ones(), 3);
        assert!(bitmap.test(0));
        assert!(bitmap.test(1));
        assert!(bitmap.test(64));
        assert!(!bitmap.test(2));

        // Find bits
        assert_eq!(bitmap.find_first_set(), Some(0));
        assert_eq!(bitmap.find_first_zero(), Some(2));

        // Clear bits
        bitmap.clear_bit(0);
        assert!(!bitmap.test(0));
        assert_eq!(bitmap.count_ones(), 2);
    }

    #[test]
    fn test_bitmap_find_and_set() {
        let mut data = [0u64; 1];
        let mut bitmap = Bitmap::from_slice(&mut data);

        // Find and set bits
        assert_eq!(bitmap.find_and_set(), Some(0));
        assert_eq!(bitmap.find_and_set(), Some(1));

        bitmap.clear_bit(0);
        assert_eq!(bitmap.find_and_set(), Some(0));

        // Fill the bitmap
        for _ in 0..64 {
            bitmap.find_and_set();
        }

        assert_eq!(bitmap.find_and_set(), None);
    }
}