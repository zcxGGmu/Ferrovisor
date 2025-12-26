//! NEON/ASIMD (Advanced SIMD) Extension Support for ARM64
//!
//! Provides NEON/ASIMD register management and operations.
//! Reference: ARM DDI 0487I.a - Chapter B4 - Advanced SIMD
//!
//! NEON/ASIMD provides:
//! - 128-bit SIMD operations on floating-point and integer data
//! - Vector operations across multiple data elements
//! - SVE (Scalable Vector Extension) support (optional)

use super::vfp::VfpRegs;

/// SIMD vector element types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SimdElementType {
    /// 8-bit signed byte
    S8 = 0,
    /// 8-bit unsigned byte
    U8 = 1,
    /// 16-bit signed half-word
    S16 = 2,
    /// 16-bit unsigned half-word
    U16 = 3,
    /// 32-bit signed word
    S32 = 4,
    /// 32-bit unsigned word
    U32 = 5,
    /// 64-bit signed double-word
    S64 = 6,
    /// 64-bit unsigned double-word
    U64 = 7,
    /// 16-bit half-precision floating-point
    F16 = 8,
    /// 32-bit single-precision floating-point
    F32 = 9,
    /// 64-bit double-precision floating-point
    F64 = 10,
}

impl SimdElementType {
    /// Get element size in bytes
    pub fn size(&self) -> usize {
        match self {
            Self::S8 | Self::U8 => 1,
            Self::S16 | Self::U16 | Self::F16 => 2,
            Self::S32 | Self::U32 | Self::F32 => 4,
            Self::S64 | Self::U64 | Self::F64 => 8,
        }
    }

    /// Get number of elements in a 128-bit vector
    pub fn count(&self) -> usize {
        16 / self.size()
    }

    /// Check if this is a floating-point type
    pub fn is_float(&self) -> bool {
        matches!(self, Self::F16 | Self::F32 | Self::F64)
    }

    /// Check if this is a signed type
    pub fn is_signed(&self) -> bool {
        matches!(self, Self::S8 | Self::S16 | Self::S32 | Self::S64)
    }
}

/// SIMD vector lane count
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SimdLaneCount {
    /// 128-bit vectors (standard NEON)
    V128 = 0,
    /// 256-bit vectors (SVE with vector length 256 bits)
    V256 = 1,
    /// 512-bit vectors (SVE with vector length 512 bits)
    V512 = 2,
    /// 1024-bit vectors (SVE with vector length 1024 bits)
    V1024 = 3,
    /// 2048-bit vectors (SVE with vector length 2048 bits)
    V2048 = 4,
}

impl SimdLaneCount {
    /// Get vector size in bits
    pub fn bits(&self) -> usize {
        match self {
            Self::V128 => 128,
            Self::V256 => 256,
            Self::V512 => 512,
            Self::V1024 => 1024,
            Self::V2048 => 2048,
        }
    }

    /// Get vector size in bytes
    pub fn bytes(&self) -> usize {
        self.bits() / 8
    }

    /// Get number of 64-bit chunks
    pub fn u64_count(&self) -> usize {
        self.bytes() / 8
    }
}

/// NEON/ASIMD vector register wrapper
///
/// Provides typed access to vector register contents.
#[derive(Debug, Clone)]
pub struct SimdVec128 {
    /// Low 64 bits
    pub low: u64,
    /// High 64 bits
    pub high: u64,
}

impl SimdVec128 {
    /// Create a new zero vector
    pub fn zero() -> Self {
        Self { low: 0, high: 0 }
    }

    /// Create from raw parts
    pub fn from_parts(low: u64, high: u64) -> Self {
        Self { low, high }
    }

    /// Get as raw bytes
    pub fn as_bytes(&self) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        bytes[0..8].copy_from_slice(&self.low.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.high.to_le_bytes());
        bytes
    }

    /// Set from raw bytes
    pub fn set_bytes(&mut self, bytes: &[u8; 16]) {
        self.low = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        self.high = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    }

    /// Get element at index (type-specific)
    pub fn get_element(&self, elem_type: SimdElementType, index: usize) -> u64 {
        let size = elem_type.size();
        assert!(index < 16 / size, "Element index out of range");

        let bits = if index < 8 / size {
            self.low
        } else {
            self.high
        };

        let local_index = if index < 8 / size {
            index
        } else {
            index - 8 / size
        };

        let shift = local_index * size;
        let mask = ((1u64 << (size * 8)) - 1) << shift;
        (bits & mask) >> shift
    }

    /// Set element at index (type-specific)
    pub fn set_element(&mut self, elem_type: SimdElementType, index: usize, value: u64) {
        let size = elem_type.size();
        assert!(index < 16 / size, "Element index out of range");

        if index < 8 / size {
            let local_index = index;
            let shift = local_index * size;
            let mask = ((1u64 << (size * 8)) - 1) << shift;
            self.low = (self.low & !mask) | ((value & ((1u64 << (size * 8)) - 1)) << shift);
        } else {
            let local_index = index - 8 / size;
            let shift = local_index * size;
            let mask = ((1u64 << (size * 8)) - 1) << shift;
            self.high = (self.high & !mask) | ((value & ((1u64 << (size * 8)) - 1)) << shift);
        }
    }

    /// Vector logical AND
    pub fn and(&self, other: &Self) -> Self {
        Self {
            low: self.low & other.low,
            high: self.high & other.high,
        }
    }

    /// Vector logical OR
    pub fn or(&self, other: &Self) -> Self {
        Self {
            low: self.low | other.low,
            high: self.high | other.high,
        }
    }

    /// Vector logical XOR
    pub fn xor(&self, other: &Self) -> Self {
        Self {
            low: self.low ^ other.low,
            high: self.high ^ other.high,
        }
    }

    /// Vector logical AND with complement
    pub fn bic(&self, other: &Self) -> Self {
        Self {
            low: self.low & !other.low,
            high: self.high & !other.high,
        }
    }
}

impl Default for SimdVec128 {
    fn default() -> Self {
        Self::zero()
    }
}

/// SVE (Scalable Vector Extension) context
///
/// SVE provides variable-length vector registers (128-2048 bits).
/// This is a simplified implementation that tracks the vector length.
#[derive(Debug, Clone)]
pub struct SveContext {
    /// Vector length in bytes (16-256)
    pub vector_length: usize,
    /// Predication registers (P0-P15)
    pub predicates: [u8; 16],
    /// FFR (First Fault Register)
    pub ffr: u16,
}

impl Default for SveContext {
    fn default() -> Self {
        Self {
            vector_length: 16, // Minimum: 128 bits
            predicates: [0; 16],
            ffr: 0,
        }
    }
}

impl SveContext {
    /// Create new SVE context with specified vector length
    pub fn with_length(length: usize) -> Self {
        assert!(length >= 16 && length <= 256 && length.is_power_of_two(),
                "SVE vector length must be power of 2 between 16 and 256");
        Self {
            vector_length: length,
            ..Default::default()
        }
    }

    /// Get vector length in bits
    pub fn vector_length_bits(&self) -> usize {
        self.vector_length * 8
    }

    /// Get predicate register
    pub fn predicate(&self, index: usize) -> u8 {
        assert!(index < 16, "Predicate index out of range");
        self.predicates[index]
    }

    /// Set predicate register
    pub fn set_predicate(&mut self, index: usize, value: u8) {
        assert!(index < 16, "Predicate index out of range");
        self.predicates[index] = value;
    }

    /// Get FFR
    pub fn ffr(&self) -> u16 {
        self.ffr
    }

    /// Set FFR
    pub fn set_ffr(&mut self, value: u16) {
        self.ffr = value & ((1u16 << self.vector_length) - 1);
    }
}

/// NEON/ASIMD context for VCPU
///
/// Provides NEON/ASIMD state management and vector operations.
#[derive(Debug, Clone)]
pub struct NeonContext {
    /// VFP/NEON registers
    pub vfp: VfpRegs,
    /// SVE context (optional)
    pub sve: Option<SveContext>,
    /// ASIMD feature enabled
    pub asimd_enabled: bool,
    /// SVE feature enabled
    pub sve_enabled: bool,
}

impl Default for NeonContext {
    fn default() -> Self {
        Self {
            vfp: VfpRegs::new(),
            sve: None,
            asimd_enabled: true,
            sve_enabled: false,
        }
    }
}

impl NeonContext {
    /// Create new NEON context
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with SVE enabled
    pub fn with_sve(vector_length: usize) -> Self {
        Self {
            vfp: VfpRegs::new(),
            sve: Some(SveContext::with_length(vector_length)),
            asimd_enabled: true,
            sve_enabled: true,
        }
    }

    /// Check if ASIMD is enabled
    pub fn has_asimd(&self) -> bool {
        self.asimd_enabled
    }

    /// Enable ASIMD
    pub fn enable_asimd(&mut self) {
        self.asimd_enabled = true;
    }

    /// Disable ASIMD
    pub fn disable_asimd(&mut self) {
        self.asimd_enabled = false;
    }

    /// Check if SVE is enabled
    pub fn has_sve(&self) -> bool {
        self.sve_enabled && self.sve.is_some()
    }

    /// Enable SVE
    pub fn enable_sve(&mut self, vector_length: usize) {
        self.sve = Some(SveContext::with_length(vector_length));
        self.sve_enabled = true;
    }

    /// Disable SVE
    pub fn disable_sve(&mut self) {
        self.sve = None;
        self.sve_enabled = false;
    }

    /// Get a 128-bit vector register
    pub fn vreg(&self, index: usize) -> SimdVec128 {
        assert!(index < 32, "V register index out of range");
        let (low, high) = self.vfp.vreg(index);
        SimdVec128::from_parts(low, high)
    }

    /// Set a 128-bit vector register
    pub fn set_vreg(&mut self, index: usize, vec: SimdVec128) {
        assert!(index < 32, "V register index out of range");
        self.vfp.set_vreg(index, vec.low, vec.high);
    }

    /// Perform vector operation (addition example)
    pub fn vec_add(&mut self, dest: usize, src1: usize, src2: usize, elem_type: SimdElementType) {
        let v1 = self.vreg(src1);
        let v2 = self.vreg(src2);
        let mut result = SimdVec128::zero();

        let count = elem_type.count();
        for i in 0..count {
            let e1 = v1.get_element(elem_type, i);
            let e2 = v2.get_element(elem_type, i);
            let sum = match elem_type {
                SimdElementType::S8 | SimdElementType::S16 | SimdElementType::S32 | SimdElementType::S64 => {
                    (e1 as i64).wrapping_add(e2 as i64) as u64
                }
                SimdElementType::U8 | SimdElementType::U16 | SimdElementType::U32 | SimdElementType::U64 => {
                    e1.wrapping_add(e2)
                }
                SimdElementType::F32 => {
                    let f1 = f32::from_bits(e1 as u32);
                    let f2 = f32::from_bits(e2 as u32);
                    f32::to_bits(f1 + f2) as u64
                }
                SimdElementType::F64 => {
                    let f1 = f64::from_bits(e1);
                    let f2 = f64::from_bits(e2);
                    f64::to_bits(f1 + f2)
                }
                SimdElementType::F16 => {
                    // Simplified: treat as integer
                    e1.wrapping_add(e2)
                }
            };
            result.set_element(elem_type, i, sum);
        }

        self.set_vreg(dest, result);
    }

    /// Save NEON context
    pub fn save(&mut self) {
        self.vfp.save();
        log::trace!("NEON: Context saved (ASIMD={}, SVE={})", self.asimd_enabled, self.sve_enabled);
    }

    /// Restore NEON context
    pub fn restore(&self) {
        self.vfp.restore();
        log::trace!("NEON: Context restored (ASIMD={}, SVE={})", self.asimd_enabled, self.sve_enabled);
    }

    /// Dump NEON state for debugging
    pub fn dump(&self) {
        log::info!("NEON/ASIMD Context:");
        log::info!("  ASIMD enabled: {}", self.asimd_enabled);
        log::info!("  SVE enabled: {}", self.sve_enabled);
        if let Some(sve) = &self.sve {
            log::info!("  SVE vector length: {} bits", sve.vector_length_bits());
        }
        log::info!("NEON Registers (first 4):");
        for i in 0..4.min(32) {
            let v = self.vreg(i);
            log::info!("  V{:02} = 0x{:016x}{:016x}", i, v.high, v.low);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_element_size() {
        assert_eq!(SimdElementType::S8.size(), 1);
        assert_eq!(SimdElementType::S16.size(), 2);
        assert_eq!(SimdElementType::S32.size(), 4);
        assert_eq!(SimdElementType::S64.size(), 8);
    }

    #[test]
    fn test_simd_element_count() {
        assert_eq!(SimdElementType::U8.count(), 16);
        assert_eq!(SimdElementType::U16.count(), 8);
        assert_eq!(SimdElementType::U32.count(), 4);
        assert_eq!(SimdElementType::U64.count(), 2);
    }

    #[test]
    fn test_simd_vec128() {
        let vec = SimdVec128::from_parts(0x1111111111111111, 0x2222222222222222);
        assert_eq!(vec.low, 0x1111111111111111);
        assert_eq!(vec.high, 0x2222222222222222);
    }

    #[test]
    fn test_simd_vec128_and() {
        let v1 = SimdVec128::from_parts(0xFFFFFFFFFFFFFFFF, 0xFFFFFFFFFFFFFFFF);
        let v2 = SimdVec128::from_parts(0x1111111111111111, 0x2222222222222222);
        let result = v1.and(&v2);
        assert_eq!(result.low, 0x1111111111111111);
        assert_eq!(result.high, 0x2222222222222222);
    }

    #[test]
    fn test_simd_vec128_element_access() {
        let mut vec = SimdVec128::zero();
        vec.set_element(SimdElementType::U8, 0, 0xAB);
        assert_eq!(vec.get_element(SimdElementType::U8, 0), 0xAB);
    }

    #[test]
    fn test_simd_vec128_element_access_32() {
        let mut vec = SimdVec128::zero();
        vec.set_element(SimdElementType::U32, 0, 0x12345678);
        assert_eq!(vec.get_element(SimdElementType::U32, 0), 0x12345678);
    }

    #[test]
    fn test_neon_context_create() {
        let ctx = NeonContext::new();
        assert!(ctx.has_asimd());
        assert!(!ctx.has_sve());
    }

    #[test]
    fn test_neon_context_sve() {
        let ctx = NeonContext::with_sve(32);
        assert!(ctx.has_sve());
        assert_eq!(ctx.sve.as_ref().unwrap().vector_length, 32);
    }

    #[test]
    fn test_neon_context_vreg_access() {
        let mut ctx = NeonContext::new();
        let vec = SimdVec128::from_parts(0x1111111111111111, 0x2222222222222222);
        ctx.set_vreg(0, vec);
        assert_eq!(ctx.vreg(0).low, 0x1111111111111111);
    }

    #[test]
    fn test_neon_vec_add_u8() {
        let mut ctx = NeonContext::new();

        // Set V0 = [1, 2, 3, 4, ...]
        let mut v0 = SimdVec128::zero();
        for i in 0..16 {
            v0.set_element(SimdElementType::U8, i, i as u64 + 1);
        }
        ctx.set_vreg(0, v0);

        // Set V1 = [1, 1, 1, 1, ...]
        let mut v1 = SimdVec128::zero();
        for i in 0..16 {
            v1.set_element(SimdElementType::U8, i, 1);
        }
        ctx.set_vreg(1, v1);

        // V2 = V0 + V1
        ctx.vec_add(2, 0, 1, SimdElementType::U8);

        // Check result
        let result = ctx.vreg(2);
        for i in 0..16 {
            assert_eq!(result.get_element(SimdElementType::U8, i), (i + 2) as u64);
        }
    }

    #[test]
    fn test_sve_context_length() {
        let ctx = SveContext::with_length(32);
        assert_eq!(ctx.vector_length, 32);
        assert_eq!(ctx.vector_length_bits(), 256);
    }

    #[test]
    fn test_sve_predicate() {
        let mut ctx = SveContext::default();
        ctx.set_predicate(0, 0xFF);
        assert_eq!(ctx.predicate(0), 0xFF);
    }

    #[test]
    #[should_panic(expected = "SVE vector length must be power of 2")]
    fn test_sve_invalid_length() {
        let _ = SveContext::with_length(15);
    }

    #[test]
    fn test_simd_lane_count() {
        assert_eq!(SimdLaneCount::V128.bits(), 128);
        assert_eq!(SimdLaneCount::V256.bits(), 256);
        assert_eq!(SimdLaneCount::V512.bits(), 512);
    }
}
