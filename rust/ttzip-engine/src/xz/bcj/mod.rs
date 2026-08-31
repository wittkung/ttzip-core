// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Hardware-accelerated Branch/Call/Jump (BCJ) architecture instruction filters for XZ streams.
//!
//! Complies strictly with the .xz File Format Specification Section 5.3 (Filter IDs 0x04..=0x0B).
//! BCJ filters convert relative branch target offsets in machine instructions into absolute
//! addresses during encoding (to enhance LZMA2 dictionary redundancy) and restore the original
//! relative offsets during decoding with 100% mathematical bijectivity.

pub mod arm;
pub mod arm64;
pub mod riscv;
pub mod stream;
pub mod x86;

pub use arm::BcjArm;
pub use arm64::BcjArm64;
pub use riscv::BcjRiscv;
pub use stream::BcjStreamFilter;
pub use x86::BcjX86;

use std::fmt::Debug;

/// Standardized XZ BCJ filter identifier constants.
pub const FILTER_ID_X86: u64 = 0x04;
pub const FILTER_ID_POWERPC: u64 = 0x05;
pub const FILTER_ID_IA64: u64 = 0x06;
pub const FILTER_ID_ARM: u64 = 0x07;
pub const FILTER_ID_ARMTHUMB: u64 = 0x08;
pub const FILTER_ID_SPARC: u64 = 0x09;
pub const FILTER_ID_ARM64: u64 = 0x0A;
pub const FILTER_ID_RISCV: u64 = 0x0B;

/// Common interface for bi-directional architecture-specific Branch/Call/Jump filters.
pub trait BranchFilter: Send + Sync + Debug {
    /// Numerical filter identifier compliant with the .xz specification.
    fn filter_id(&self) -> u64;

    /// Required instruction memory alignment quantum in bytes (1, 2, 4, or 16).
    fn alignment(&self) -> usize;

    /// Maximum number of unfiltered lookahead bytes that can remain at the end of a buffer chunk.
    fn unfiltered_max(&self) -> usize;

    /// Normalizes relative branch targets to absolute addresses in-place (compression phase).
    ///
    /// # Arguments
    /// * `buf` - Mutable byte slice containing instruction stream.
    /// * `now_pos` - Global uncompressed stream offset (program counter base).
    ///
    /// Returns the number of bytes filtered.
    fn encode(&mut self, buf: &mut [u8], now_pos: u32) -> usize;

    /// Restores relative branch targets from absolute addresses in-place (decompression phase).
    ///
    /// # Arguments
    /// * `buf` - Mutable byte slice containing normalized instruction stream.
    /// * `now_pos` - Global uncompressed stream offset (program counter base).
    ///
    /// Returns the number of bytes filtered.
    fn decode(&mut self, buf: &mut [u8], now_pos: u32) -> usize;

    /// Resets internal filter state (masks, sliding positions) for a new block or stream.
    fn reset(&mut self);
}
