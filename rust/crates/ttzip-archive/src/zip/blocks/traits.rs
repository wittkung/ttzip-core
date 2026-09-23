// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Core Plain Old Data (POD) and Fixed-Size Block traits for zero-copy binary serialization.

/// Marker trait for Plain Old Data (POD) types that can safely undergo zero-copy
/// serialization and deserialization from raw byte slices.
///
/// # Safety
///
/// Implementors must ensure that:
/// 1. The type has a well-defined packed memory layout (`#[repr(C, packed)]`).
/// 2. All bit patterns within the type's byte footprint are valid representations (e.g., primitive integers).
/// 3. The type does not implement `Drop` and contains no pointers, references, or uninitialized padding.
pub unsafe trait Pod: Copy + 'static {
    /// Casts an immutable byte slice prefix to a reference of `Self` if the slice is large enough.
    ///
    /// Returns a tuple containing the reference to `Self` and the remaining unconsumed slice.
    #[inline(always)]
    fn ref_from_prefix(bytes: &[u8]) -> Option<(&Self, &[u8])> {
        let size = std::mem::size_of::<Self>();
        if bytes.len() < size {
            return None;
        }
        let (head, tail) = bytes.split_at(size);
        let ptr = head.as_ptr() as *const Self;
        unsafe { Some((&*ptr, tail)) }
    }

    /// Casts a mutable byte slice prefix to a mutable reference of `Self` if the slice is large enough.
    ///
    /// Returns a tuple containing the mutable reference to `Self` and the remaining unconsumed slice.
    #[inline(always)]
    fn mut_from_prefix(bytes: &mut [u8]) -> Option<(&mut Self, &mut [u8])> {
        let size = std::mem::size_of::<Self>();
        if bytes.len() < size {
            return None;
        }
        let (head, tail) = bytes.split_at_mut(size);
        let ptr = head.as_mut_ptr() as *mut Self;
        unsafe { Some((&mut *ptr, tail)) }
    }

    /// Returns a raw byte slice view of `self`.
    #[inline(always)]
    fn as_bytes(&self) -> &[u8] {
        let size = std::mem::size_of::<Self>();
        unsafe { std::slice::from_raw_parts(self as *const Self as *const u8, size) }
    }

    /// Returns a mutable raw byte slice view of `self`.
    #[inline(always)]
    fn as_mut_bytes(&mut self) -> &mut [u8] {
        let size = std::mem::size_of::<Self>();
        unsafe { std::slice::from_raw_parts_mut(self as *mut Self as *mut u8, size) }
    }
}

/// Common trait for all fixed-size ZIP binary structures.
pub trait FixedSizeBlock: Pod + Sized {
    /// 4-byte signature magic constant for this fixed-size block.
    const MAGIC: u32;

    /// Fixed size in bytes of this block in raw binary form.
    const SIZE: usize = std::mem::size_of::<Self>();

    /// Converts all numerical fields in `self` from little-endian representation to host-endian.
    #[allow(clippy::wrong_self_convention)]
    fn from_le(self) -> Self;

    /// Converts all numerical fields in `self` from host-endian representation to little-endian.
    fn to_le(self) -> Self;

    /// Parses a block from a raw byte slice, performing endian conversion from little-endian.
    ///
    /// Returns the parsed block and the number of bytes consumed (`Self::SIZE`).
    #[inline]
    fn parse(bytes: &[u8]) -> Option<(Self, usize)> {
        let size = std::mem::size_of::<Self>();
        if bytes.len() < size {
            return None;
        }
        let mut val = std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), val.as_mut_ptr() as *mut u8, size);
            Some((val.assume_init().from_le(), size))
        }
    }

    /// Parses a block from the prefix of a raw byte slice, returning the parsed block
    /// and the remaining unconsumed slice.
    #[inline]
    fn parse_from_prefix(bytes: &[u8]) -> Option<(Self, &[u8])> {
        let size = std::mem::size_of::<Self>();
        if bytes.len() < size {
            return None;
        }
        let (head, tail) = bytes.split_at(size);
        let mut val = std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            std::ptr::copy_nonoverlapping(head.as_ptr(), val.as_mut_ptr() as *mut u8, size);
            Some((val.assume_init().from_le(), tail))
        }
    }

    /// Serializes `self` into little-endian format and writes it into `dest`.
    ///
    /// Returns the number of bytes written (`Self::SIZE`) or `None` if `dest` is too small.
    #[inline]
    fn write(&self, dest: &mut [u8]) -> Option<usize> {
        let size = std::mem::size_of::<Self>();
        if dest.len() < size {
            return None;
        }
        let le_self = self.to_le();
        unsafe {
            std::ptr::copy_nonoverlapping(
                &le_self as *const Self as *const u8,
                dest.as_mut_ptr(),
                size,
            );
        }
        Some(size)
    }

    /// Serializes `self` into little-endian format and appends it to a `Vec<u8>`.
    #[inline]
    fn write_to_vec(&self, out: &mut Vec<u8>) {
        let size = std::mem::size_of::<Self>();
        let le_self = self.to_le();
        let start = out.len();
        out.reserve(size);
        unsafe {
            let p = out.as_mut_ptr().add(start);
            std::ptr::copy_nonoverlapping(&le_self as *const Self as *const u8, p, size);
            out.set_len(start + size);
        }
    }
}
