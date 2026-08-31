// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Apple `LZFSE` block container parser, bitfield serializer, and Huffman frequency table codec.
//!
//! Conforms strictly to Apple LZFSE standard container specifications (`bvx-`, `bvx1`, `bvx2`, `bvxn`, `bvx$`).

use crate::types::TTZipStatus;

pub use super::freq_tables::*;
pub use super::fsm::{LzfseBlockFsm, LzfseFsmState, LzfseFsmStep};

// MARK: - BVX Magic Number Definition

/// Strong enumeration of 4-byte LZFSE container magic identifiers.
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum BvxMagic {
    /// Raw uncompressed payload block (`bvx-`).
    RawUncompressed = 0x2d78_7662,
    /// LZFSE compressed block with uncompressed frequency tables (`bvx1`).
    CompressedV1 = 0x3178_7662,
    /// LZFSE compressed block with compressed Huffman frequency tables (`bvx2`).
    CompressedV2 = 0x3278_7662,
    /// Apple LZVN compressed payload block (`bvxn`).
    CompressedLZVN = 0x6e78_7662,
    /// End-of-stream terminal marker block (`bvx$`).
    EndOfStream = 0x2478_7662,
}

impl BvxMagic {
    /// Parses a 32-bit unsigned integer (in host endianness) to `BvxMagic`.
    #[inline]
    #[must_use]
    pub const fn from_u32(val: u32) -> Option<Self> {
        match val {
            0x2d78_7662 => Some(Self::RawUncompressed),
            0x3178_7662 => Some(Self::CompressedV1),
            0x3278_7662 => Some(Self::CompressedV2),
            0x6e78_7662 => Some(Self::CompressedLZVN),
            0x2478_7662 => Some(Self::EndOfStream),
            _ => None,
        }
    }

    /// Converts the magic enumeration to its underlying 32-bit unsigned integer.
    #[inline]
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }

    /// Parses a 4-byte little-endian array to `BvxMagic`.
    #[inline]
    #[must_use]
    pub fn from_bytes(bytes: [u8; 4]) -> Option<Self> {
        Self::from_u32(u32::from_le_bytes(bytes))
    }

    /// Returns the 4-byte little-endian representation of this magic number.
    #[inline]
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 4] {
        self.as_u32().to_le_bytes()
    }

    /// Returns the 4-character ASCII representation (e.g. `"bvx-"`, `"bvx2"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RawUncompressed => "bvx-",
            Self::CompressedV1 => "bvx1",
            Self::CompressedV2 => "bvx2",
            Self::CompressedLZVN => "bvxn",
            Self::EndOfStream => "bvx$",
        }
    }
}

impl TryFrom<u32> for BvxMagic {
    type Error = TTZipStatus;

    #[inline]
    fn try_from(val: u32) -> Result<Self, Self::Error> {
        Self::from_u32(val).ok_or(TTZipStatus::ErrCorruptHeader)
    }
}

impl From<BvxMagic> for u32 {
    #[inline]
    fn from(magic: BvxMagic) -> Self {
        magic.as_u32()
    }
}

// MARK: - Bitfield Manipulation Utilities

#[inline]
fn get_field(v: u64, offset: usize, nbits: usize) -> u32 {
    debug_assert!(offset + nbits <= 64 && nbits <= 32);
    if nbits == 32 {
        (v >> offset) as u32
    } else {
        ((v >> offset) & ((1u64 << nbits) - 1)) as u32
    }
}

#[inline]
fn set_field(v: u32, offset: usize, nbits: usize) -> u64 {
    debug_assert!(offset + nbits <= 64 && nbits <= 32);
    let mask = if nbits == 32 {
        0xFFFF_FFFFu64
    } else {
        (1u64 << nbits) - 1
    };
    ((v as u64) & mask) << offset
}

// MARK: - LZFSE Block Header Structure

/// Strongly-typed model representing any valid LZFSE block header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LzfseBlockHeader {
    /// Magic identifier of the block container.
    pub magic: BvxMagic,
    /// Number of decoded (uncompressed raw output) bytes produced by this block.
    pub n_raw_bytes: u32,
    /// Total encoded payload bytes (literals + LMD bytes) for compressed blocks.
    pub n_payload_bytes: u32,
    /// Number of literal bytes emitted in this block.
    pub n_literals: u32,
    /// Number of matches in this block.
    pub n_matches: u32,
    /// Size in bytes of the literal stream payload.
    pub n_literal_payload_bytes: u32,
    /// Size in bytes of the L, M, D stream payload.
    pub n_lmd_payload_bytes: u32,
    /// Literal stream bit accumulator count offset in range `[-7, 0]`.
    pub literal_bits: i32,
    /// Initial FSE decoder states for the 4 interleaved literal streams (< 1024).
    pub literal_state: [u16; 4],
    /// LMD stream bit accumulator count offset in range `[-7, 0]`.
    pub lmd_bits: i32,
    /// Initial FSE decoder state for literal length `L` (< 64).
    pub l_state: u16,
    /// Initial FSE decoder state for match length `M` (< 64).
    pub m_state: u16,
    /// Initial FSE decoder state for match distance `D` (< 256).
    pub d_state: u16,
    /// Optional normalized frequency tables (for `bvx1` and `bvx2` blocks).
    pub freq_tables: Option<LzfseFreqTables>,
    /// Total header length in bytes (including magic, packed fields, and freq payload).
    pub header_size: u32,
}

impl Default for LzfseBlockHeader {
    fn default() -> Self {
        Self {
            magic: BvxMagic::EndOfStream,
            n_raw_bytes: 0,
            n_payload_bytes: 0,
            n_literals: 0,
            n_matches: 0,
            n_literal_payload_bytes: 0,
            n_lmd_payload_bytes: 0,
            literal_bits: 0,
            literal_state: [0u16; 4],
            lmd_bits: 0,
            l_state: 0,
            m_state: 0,
            d_state: 0,
            freq_tables: None,
            header_size: 4,
        }
    }
}

impl LzfseBlockHeader {
    /// Creates an uncompressed raw data block header (`bvx-`).
    #[must_use]
    pub fn new_uncompressed(n_raw_bytes: u32) -> Self {
        Self {
            magic: BvxMagic::RawUncompressed,
            n_raw_bytes,
            header_size: 8,
            ..Default::default()
        }
    }

    /// Creates an LZVN compressed block header (`bvxn`).
    #[must_use]
    pub fn new_lzvn(n_raw_bytes: u32, n_payload_bytes: u32) -> Self {
        Self {
            magic: BvxMagic::CompressedLZVN,
            n_raw_bytes,
            n_payload_bytes,
            header_size: 12,
            ..Default::default()
        }
    }

    /// Creates an end-of-stream terminal block header (`bvx$`).
    #[must_use]
    pub fn new_end_of_stream() -> Self {
        Self {
            magic: BvxMagic::EndOfStream,
            header_size: 4,
            ..Default::default()
        }
    }

    /// Performs validation against standard LZFSE specification constraints.
    pub fn validate(&self) -> Result<(), TTZipStatus> {
        match self.magic {
            BvxMagic::RawUncompressed => Ok(()),
            BvxMagic::CompressedLZVN => Ok(()),
            BvxMagic::EndOfStream => Ok(()),
            BvxMagic::CompressedV1 | BvxMagic::CompressedV2 => {
                if (self.n_literals as usize) > LZFSE_LITERALS_PER_BLOCK {
                    eprintln!("validate err: n_literals={}", self.n_literals);
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
                if (self.n_matches as usize) > LZFSE_MATCHES_PER_BLOCK {
                    eprintln!("validate err: n_matches={}", self.n_matches);
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
                for (i, &state) in self.literal_state.iter().enumerate() {
                    if state as usize >= LZFSE_ENCODE_LITERAL_STATES {
                        eprintln!("validate err: literal_state[{i}]={state}");
                        return Err(TTZipStatus::ErrCorruptHeader);
                    }
                }
                if self.l_state as usize >= LZFSE_ENCODE_L_STATES
                    || self.m_state as usize >= LZFSE_ENCODE_M_STATES
                    || self.d_state as usize >= LZFSE_ENCODE_D_STATES
                {
                    eprintln!(
                        "validate err: l_state={}, m_state={}, d_state={}",
                        self.l_state, self.m_state, self.d_state
                    );
                    return Err(TTZipStatus::ErrCorruptHeader);
                }
                if let Some(tables) = &self.freq_tables {
                    if let Err(e) = tables.validate() {
                        eprintln!("validate err: tables.validate() returned {e:?}");
                        return Err(e);
                    }
                }
                Ok(())
            }
        }
    }

    /// Unpacks 3x 64-bit V2 bitfields into a structured `LzfseBlockHeader`.
    pub fn unpack_v2_fields(
        n_raw_bytes: u32,
        packed: [u64; 3],
        freq_tables: Option<LzfseFreqTables>,
    ) -> Result<Self, TTZipStatus> {
        let v0 = packed[0];
        let v1 = packed[1];
        let v2 = packed[2];

        let n_literals = get_field(v0, 0, 20);
        let n_literal_payload_bytes = get_field(v0, 20, 20);
        let n_matches = get_field(v0, 40, 20);
        let literal_bits = (get_field(v0, 60, 3) as i32) - 7;

        let literal_state = [
            get_field(v1, 0, 10) as u16,
            get_field(v1, 10, 10) as u16,
            get_field(v1, 20, 10) as u16,
            get_field(v1, 30, 10) as u16,
        ];
        let n_lmd_payload_bytes = get_field(v1, 40, 20);
        let lmd_bits = (get_field(v1, 60, 3) as i32) - 7;

        let header_size = get_field(v2, 0, 32);
        let l_state = get_field(v2, 32, 10) as u16;
        let m_state = get_field(v2, 42, 10) as u16;
        let d_state = get_field(v2, 52, 10) as u16;

        let header = Self {
            magic: BvxMagic::CompressedV2,
            n_raw_bytes,
            n_payload_bytes: n_literal_payload_bytes + n_lmd_payload_bytes,
            n_literals,
            n_matches,
            n_literal_payload_bytes,
            n_lmd_payload_bytes,
            literal_bits,
            literal_state,
            lmd_bits,
            l_state,
            m_state,
            d_state,
            freq_tables,
            header_size,
        };

        header.validate()?;
        Ok(header)
    }

    /// Packs V2 header fields into 3x 64-bit bitfields for wire serialization.
    #[must_use]
    pub fn pack_v2_fields(&self) -> [u64; 3] {
        let v0 = set_field(self.n_literals, 0, 20)
            | set_field(self.n_literal_payload_bytes, 20, 20)
            | set_field(self.n_matches, 40, 20)
            | set_field((7 + self.literal_bits).clamp(0, 7) as u32, 60, 3);

        let v1 = set_field(self.literal_state[0] as u32, 0, 10)
            | set_field(self.literal_state[1] as u32, 10, 10)
            | set_field(self.literal_state[2] as u32, 20, 10)
            | set_field(self.literal_state[3] as u32, 30, 10)
            | set_field(self.n_lmd_payload_bytes, 40, 20)
            | set_field((7 + self.lmd_bits).clamp(0, 7) as u32, 60, 3);

        let v2 = set_field(self.header_size, 0, 32)
            | set_field(self.l_state as u32, 32, 10)
            | set_field(self.m_state as u32, 42, 10)
            | set_field(self.d_state as u32, 52, 10);

        [v0, v1, v2]
    }
}

// MARK: - Header Parsing and Emission

/// Parses any valid LZFSE block header from the source byte slice `src`.
///
/// Returns `(header, header_size_in_bytes)` on success.
pub fn parse_block_header(src: &[u8]) -> Result<(LzfseBlockHeader, usize), TTZipStatus> {
    if src.len() < 4 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    let magic_u32 = u32::from_le_bytes(
        src[0..4]
            .try_into()
            .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
    );
    let magic = BvxMagic::from_u32(magic_u32).ok_or(TTZipStatus::ErrCorruptHeader)?;

    match magic {
        BvxMagic::EndOfStream => Ok((LzfseBlockHeader::new_end_of_stream(), 4)),
        BvxMagic::RawUncompressed => {
            if src.len() < 8 {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            let n_raw_bytes = u32::from_le_bytes(
                src[4..8]
                    .try_into()
                    .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
            );
            Ok((LzfseBlockHeader::new_uncompressed(n_raw_bytes), 8))
        }
        BvxMagic::CompressedLZVN => {
            if src.len() < 12 {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            let n_raw_bytes = u32::from_le_bytes(
                src[4..8]
                    .try_into()
                    .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
            );
            let n_payload_bytes = u32::from_le_bytes(
                src[8..12]
                    .try_into()
                    .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
            );
            Ok((
                LzfseBlockHeader::new_lzvn(n_raw_bytes, n_payload_bytes),
                12,
            ))
        }
        BvxMagic::CompressedV2 => {
            if src.len() < LZFSE_V2_HEADER_FIXED_SIZE {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            let n_raw_bytes = u32::from_le_bytes(
                src[4..8]
                    .try_into()
                    .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
            );
            let packed0 = u64::from_le_bytes(
                src[8..16]
                    .try_into()
                    .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
            );
            let packed1 = u64::from_le_bytes(
                src[16..24]
                    .try_into()
                    .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
            );
            let packed2 = u64::from_le_bytes(
                src[24..32]
                    .try_into()
                    .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
            );

            let header_size = get_field(packed2, 0, 32);
            if (header_size as usize) < LZFSE_V2_HEADER_FIXED_SIZE || (header_size as usize) > src.len()
            {
                return Err(TTZipStatus::ErrCorruptHeader);
            }

            let freq_tables = if (header_size as usize) > LZFSE_V2_HEADER_FIXED_SIZE {
                let freq_slice = &src[LZFSE_V2_HEADER_FIXED_SIZE..header_size as usize];
                let (tables, _) = decode_v2_freq_tables(freq_slice)?;
                Some(tables)
            } else {
                None
            };

            let header = LzfseBlockHeader::unpack_v2_fields(
                n_raw_bytes,
                [packed0, packed1, packed2],
                freq_tables,
            )?;

            Ok((header, header_size as usize))
        }
        BvxMagic::CompressedV1 => {
            // Uncompressed tables V1 block header is 770 bytes fixed layout
            const V1_HEADER_SIZE: usize = 770;
            if src.len() < V1_HEADER_SIZE {
                return Err(TTZipStatus::ErrCorruptHeader);
            }
            let n_raw_bytes = u32::from_le_bytes(
                src[4..8]
                    .try_into()
                    .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
            );
            let n_payload_bytes = u32::from_le_bytes(
                src[8..12]
                    .try_into()
                    .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
            );
            let n_literals = u32::from_le_bytes(
                src[12..16]
                    .try_into()
                    .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
            );
            let n_matches = u32::from_le_bytes(
                src[16..20]
                    .try_into()
                    .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
            );
            let n_literal_payload_bytes = u32::from_le_bytes(
                src[20..24]
                    .try_into()
                    .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
            );
            let n_lmd_payload_bytes = u32::from_le_bytes(
                src[24..28]
                    .try_into()
                    .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
            );
            let literal_bits = i32::from_le_bytes(
                src[28..32]
                    .try_into()
                    .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
            );
            let literal_state = [
                u16::from_le_bytes(
                    src[32..34]
                        .try_into()
                        .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
                ),
                u16::from_le_bytes(
                    src[34..36]
                        .try_into()
                        .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
                ),
                u16::from_le_bytes(
                    src[36..38]
                        .try_into()
                        .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
                ),
                u16::from_le_bytes(
                    src[38..40]
                        .try_into()
                        .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
                ),
            ];
            let lmd_bits = i32::from_le_bytes(
                src[40..44]
                    .try_into()
                    .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
            );
            let l_state = u16::from_le_bytes(
                src[44..46]
                    .try_into()
                    .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
            );
            let m_state = u16::from_le_bytes(
                src[46..48]
                    .try_into()
                    .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
            );
            let d_state = u16::from_le_bytes(
                src[48..50]
                    .try_into()
                    .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
            );

            let mut symbols = [0u16; LZFSE_FREQ_TOTAL_SYMBOLS];
            for i in 0..LZFSE_FREQ_TOTAL_SYMBOLS {
                let off = 50 + i * 2;
                symbols[i] = u16::from_le_bytes(
                    src[off..off + 2]
                        .try_into()
                        .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
                );
            }
            let freq_tables = Some(LzfseFreqTables::from_symbols(&symbols)?);

            let header = LzfseBlockHeader {
                magic: BvxMagic::CompressedV1,
                n_raw_bytes,
                n_payload_bytes,
                n_literals,
                n_matches,
                n_literal_payload_bytes,
                n_lmd_payload_bytes,
                literal_bits,
                literal_state,
                lmd_bits,
                l_state,
                m_state,
                d_state,
                freq_tables,
                header_size: V1_HEADER_SIZE as u32,
            };

            header.validate()?;
            Ok((header, V1_HEADER_SIZE))
        }
    }
}

/// Serializes and emits a `bvx2` compressed block header into the target byte vector `dst`.
pub fn emit_block_header_v2(header: &LzfseBlockHeader, dst: &mut Vec<u8>) {
    let mut freq_bytes = Vec::new();
    if let Some(tables) = &header.freq_tables {
        encode_v2_freq_tables(tables, &mut freq_bytes);
    }

    let header_size = (LZFSE_V2_HEADER_FIXED_SIZE + freq_bytes.len()) as u32;
    let mut header_copy = header.clone();
    header_copy.magic = BvxMagic::CompressedV2;
    header_copy.header_size = header_size;

    let packed = header_copy.pack_v2_fields();

    dst.extend_from_slice(&BvxMagic::CompressedV2.as_bytes());
    dst.extend_from_slice(&header.n_raw_bytes.to_le_bytes());
    dst.extend_from_slice(&packed[0].to_le_bytes());
    dst.extend_from_slice(&packed[1].to_le_bytes());
    dst.extend_from_slice(&packed[2].to_le_bytes());
    dst.extend_from_slice(&freq_bytes);
}
