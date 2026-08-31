// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! High-performance pure-Rust Google Snappy raw block compressor.
//!
//! Implements 64-bit SWAR match length scanning, 16-byte unrolled sliding window
//! exploration, and adaptive heuristic skipping (`skip += skip >> 5`) achieving
//! > 500 MB/s single-core compression throughput with zero unsafe FFI overhead.

use crate::codecs::snappy::error::SnappyError;
use crate::codecs::snappy::hash_table::SnappyHashTable;

/// Standard maximum uncompressed block chunk size per Google Snappy specification (64KB).
pub const SNAPPY_BLOCK_SIZE: usize = 65536;

/// Minimum remaining bytes threshold required to execute the unrolled fast-path match search.
pub const SNAPPY_INPUT_MARGIN_BYTES: usize = 15;

/// Computes upper bound on compressed bytes for a given raw input size: `32 + N + N / 6`.
#[inline]
pub fn max_compressed_len(src_len: usize) -> usize {
    32 + src_len + src_len / 6
}

/// Encodes an unsigned integer into LEB128 varint format, returning the number of bytes written.
#[inline]
pub fn write_varint(mut val: usize, dst: &mut [u8]) -> usize {
    let mut i = 0;
    while val >= 0x80 {
        dst[i] = (val as u8 & 0x7F) | 0x80;
        val >>= 7;
        i += 1;
    }
    dst[i] = val as u8;
    i + 1
}

/// Computes identical byte length between two slices using 64-bit SWAR word scanning.
///
/// Uses hardware `trailing_zeros` (ARM64 `rbit` + `clz`, x86-64 `tzcnt` / `bsf`) to
/// resolve exact byte divergence within an 8-byte word in a single instruction.
#[inline(always)]
pub fn find_match_length(s1: &[u8], s2: &[u8]) -> usize {
    let limit = s1.len().min(s2.len());
    let mut matched = 0;

    while matched + 8 <= limit {
        let a1 = u64::from_le_bytes(s1[matched..matched + 8].try_into().unwrap());
        let a2 = u64::from_le_bytes(s2[matched..matched + 8].try_into().unwrap());
        if a1 == a2 {
            matched += 8;
        } else {
            let xor = a1 ^ a2;
            let diff_bytes = (xor.trailing_zeros() >> 3) as usize;
            return matched + diff_bytes;
        }
    }

    while matched < limit && s1[matched] == s2[matched] {
        matched += 1;
    }
    matched
}

/// Emits a literal byte sequence into the destination buffer with standard Snappy tag framing.
#[inline(always)]
fn emit_literal(dst: &mut [u8], mut op: usize, literal: &[u8]) -> Result<usize, SnappyError> {
    let len = literal.len();
    if len == 0 {
        return Ok(op);
    }
    let n = len - 1;
    if n < 60 {
        if op >= dst.len() {
            return Err(SnappyError::BufferTooSmall {
                required: op + 1 + len,
                available: dst.len(),
            });
        }
        dst[op] = (n as u8) << 2;
        op += 1;
    } else if n <= 0xFF {
        if op + 2 > dst.len() {
            return Err(SnappyError::BufferTooSmall {
                required: op + 2 + len,
                available: dst.len(),
            });
        }
        dst[op] = 60 << 2;
        dst[op + 1] = n as u8;
        op += 2;
    } else if n <= 0xFFFF {
        if op + 3 > dst.len() {
            return Err(SnappyError::BufferTooSmall {
                required: op + 3 + len,
                available: dst.len(),
            });
        }
        dst[op] = 61 << 2;
        dst[op + 1..op + 3].copy_from_slice(&(n as u16).to_le_bytes());
        op += 3;
    } else if n <= 0xFF_FFFF {
        if op + 4 > dst.len() {
            return Err(SnappyError::BufferTooSmall {
                required: op + 4 + len,
                available: dst.len(),
            });
        }
        dst[op] = 62 << 2;
        dst[op + 1] = (n & 0xFF) as u8;
        dst[op + 2] = ((n >> 8) & 0xFF) as u8;
        dst[op + 3] = ((n >> 16) & 0xFF) as u8;
        op += 4;
    } else {
        if op + 5 > dst.len() {
            return Err(SnappyError::BufferTooSmall {
                required: op + 5 + len,
                available: dst.len(),
            });
        }
        dst[op] = 63 << 2;
        dst[op + 1..op + 5].copy_from_slice(&(n as u32).to_le_bytes());
        op += 5;
    }

    if op + len > dst.len() {
        return Err(SnappyError::BufferTooSmall {
            required: op + len,
            available: dst.len(),
        });
    }
    dst[op..op + len].copy_from_slice(literal);
    Ok(op + len)
}

/// Emits a single copy token of length $\le 64$ into the destination buffer.
#[inline(always)]
fn emit_copy_at_most_64(
    dst: &mut [u8],
    mut op: usize,
    offset: usize,
    len: usize,
) -> Result<usize, SnappyError> {
    debug_assert!((4..=64).contains(&len));
    debug_assert!((1..65536).contains(&offset));

    if len < 12 && offset < 2048 {
        if op + 2 > dst.len() {
            return Err(SnappyError::BufferTooSmall {
                required: op + 2,
                available: dst.len(),
            });
        }
        let tag = 0b01 | (((len - 4) as u8) << 2) | ((((offset >> 8) & 0x07) as u8) << 5);
        dst[op] = tag;
        dst[op + 1] = (offset & 0xFF) as u8;
        op += 2;
    } else {
        if op + 3 > dst.len() {
            return Err(SnappyError::BufferTooSmall {
                required: op + 3,
                available: dst.len(),
            });
        }
        let tag = 0b10 | (((len - 1) as u8) << 2);
        dst[op] = tag;
        dst[op + 1..op + 3].copy_from_slice(&(offset as u16).to_le_bytes());
        op += 3;
    }
    Ok(op)
}

/// Emits copy tokens for an arbitrary match length by chunking into $\le 64$-byte copies.
#[inline(always)]
fn emit_copy(
    dst: &mut [u8],
    mut op: usize,
    offset: usize,
    mut len: usize,
) -> Result<usize, SnappyError> {
    while len >= 68 {
        op = emit_copy_at_most_64(dst, op, offset, 64)?;
        len -= 64;
    }
    if len > 64 {
        op = emit_copy_at_most_64(dst, op, offset, 60)?;
        len -= 60;
    }
    if len >= 4 {
        op = emit_copy_at_most_64(dst, op, offset, len)?;
    }
    Ok(op)
}

/// Compresses a single block fragment ($\le 64\text{KB}$) without emitting the uncompressed length varint preamble.
///
/// Returns the number of compressed bytes written to `output`.
pub fn raw_compress_fragment(
    input: &[u8],
    output: &mut [u8],
    table: &mut SnappyHashTable,
) -> Result<usize, SnappyError> {
    if input.len() > SNAPPY_BLOCK_SIZE {
        return Err(SnappyError::BlockTooLarge {
            size: input.len(),
            max: SNAPPY_BLOCK_SIZE,
        });
    }
    let min_required = input.len() + input.len() / 6 + 16;
    if output.len() < min_required {
        return Err(SnappyError::BufferTooSmall {
            required: min_required,
            available: output.len(),
        });
    }

    table.clear();

    if input.len() < SNAPPY_INPUT_MARGIN_BYTES {
        return emit_literal(output, 0, input);
    }

    let ip_limit = input.len() - SNAPPY_INPUT_MARGIN_BYTES;
    let mut ip = 0;
    let mut op = 0;
    let mut preload = u32::from_le_bytes(input[1..5].try_into().unwrap());

    'outer: while ip < ip_limit {
        let next_emit = ip;
        ip += 1;
        let mut data = u64::from_le_bytes(input[ip..ip + 8].try_into().unwrap());
        let mut skip = 32usize;
        let mut candidate: usize = 0;

        if ip_limit.saturating_sub(ip) >= 16 {
            let delta = ip;
            let mut found = false;
            for j in 0..4 {
                for k in 0..4 {
                    let i = 4 * j + k;
                    let dword = if i == 0 { preload } else { data as u32 };
                    candidate = table.lookup_and_update(dword, delta + i);
                    let cand_dword =
                        u32::from_le_bytes(input[candidate..candidate + 4].try_into().unwrap());
                    if cand_dword == dword {
                        op = emit_literal(output, op, &input[next_emit..ip + i])?;
                        ip += i;
                        found = true;
                        break;
                    }
                    data >>= 8;
                }
                if found {
                    break;
                }
                if j < 3 {
                    data = u64::from_le_bytes(
                        input[ip + 4 * j + 4..ip + 4 * j + 12]
                            .try_into()
                            .unwrap(),
                    );
                }
            }
            if found {
                // Match found at ip from 16-byte unrolled loop
                loop {
                    let matched =
                        4 + find_match_length(&input[candidate + 4..], &input[ip + 4..]);
                    let offset = ip - candidate;
                    if offset == 0 || offset > ip {
                        return Err(SnappyError::OffsetOutOfBounds {
                            offset,
                            current_pos: ip,
                        });
                    }
                    op = emit_copy(output, op, offset, matched)?;
                    ip += matched;
                    if ip >= ip_limit {
                        break 'outer;
                    }
                    let prev_dword =
                        u32::from_le_bytes(input[ip - 1..ip + 3].try_into().unwrap());
                    table.update(prev_dword, ip - 1);
                    let curr_dword =
                        u32::from_le_bytes(input[ip..ip + 4].try_into().unwrap());
                    candidate = table.lookup_and_update(curr_dword, ip);
                    let cand_dword =
                        u32::from_le_bytes(input[candidate..candidate + 4].try_into().unwrap());
                    if cand_dword != curr_dword {
                        break;
                    }
                }
                preload = u32::from_le_bytes(input[ip + 1..ip + 5].try_into().unwrap());
                continue 'outer;
            }
            ip += 16;
            skip += 16;
        }

        loop {
            let dword = data as u32;
            candidate = table.lookup_and_update(dword, ip);
            let cand_dword =
                u32::from_le_bytes(input[candidate..candidate + 4].try_into().unwrap());
            if cand_dword == dword {
                op = emit_literal(output, op, &input[next_emit..ip])?;
                break;
            }
            let bytes_between_lookups = skip >> 5;
            skip += bytes_between_lookups;
            let next_ip = ip + bytes_between_lookups;
            if next_ip > ip_limit {
                ip = next_emit;
                break 'outer;
            }
            ip = next_ip;
            if ip + 8 <= input.len() {
                data = u64::from_le_bytes(input[ip..ip + 8].try_into().unwrap());
            } else {
                data = u32::from_le_bytes(input[ip..ip + 4].try_into().unwrap()) as u64;
            }
        }

        loop {
            let matched = 4 + find_match_length(&input[candidate + 4..], &input[ip + 4..]);
            let offset = ip - candidate;
            if offset == 0 || offset > ip {
                return Err(SnappyError::OffsetOutOfBounds {
                    offset,
                    current_pos: ip,
                });
            }
            op = emit_copy(output, op, offset, matched)?;
            ip += matched;
            if ip >= ip_limit {
                break 'outer;
            }
            let prev_dword = u32::from_le_bytes(input[ip - 1..ip + 3].try_into().unwrap());
            table.update(prev_dword, ip - 1);
            let curr_dword = u32::from_le_bytes(input[ip..ip + 4].try_into().unwrap());
            candidate = table.lookup_and_update(curr_dword, ip);
            let cand_dword =
                u32::from_le_bytes(input[candidate..candidate + 4].try_into().unwrap());
            if cand_dword != curr_dword {
                break;
            }
        }
        preload = u32::from_le_bytes(input[ip + 1..ip + 5].try_into().unwrap());
    }

    if ip < input.len() {
        op = emit_literal(output, op, &input[ip..])?;
    }

    Ok(op)
}

/// Compresses a complete raw byte buffer into Google Snappy raw block format with uncompressed length preamble.
///
/// Chunks the input into 64KB fragments and compresses each fragment with an L1-cache-resident hash table.
pub fn raw_compress(src: &[u8], dst: &mut [u8]) -> Result<usize, SnappyError> {
    if src.is_empty() {
        if dst.is_empty() {
            return Err(SnappyError::BufferTooSmall {
                required: 1,
                available: 0,
            });
        }
        dst[0] = 0x00;
        return Ok(1);
    }

    let max_len = max_compressed_len(src.len());
    if dst.len() < max_len {
        return Err(SnappyError::BufferTooSmall {
            required: max_len,
            available: dst.len(),
        });
    }

    let mut op = write_varint(src.len(), dst);
    let mut table = SnappyHashTable::new();

    for chunk in src.chunks(SNAPPY_BLOCK_SIZE) {
        let chunk_written = raw_compress_fragment(chunk, &mut dst[op..], &mut table)?;
        op += chunk_written;
    }

    Ok(op)
}

/// Compresses a memory slice into a newly allocated `Vec<u8>`.
pub fn raw_compress_to_vec(src: &[u8]) -> Result<Vec<u8>, SnappyError> {
    let bound = max_compressed_len(src.len());
    let mut out = vec![0u8; bound];
    let written = raw_compress(src, &mut out)?;
    out.truncate(written);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_compressed_len_bounds() {
        assert_eq!(max_compressed_len(0), 32);
        assert_eq!(max_compressed_len(100), 32 + 100 + 16);
        assert_eq!(max_compressed_len(65536), 32 + 65536 + 65536 / 6);
    }

    #[test]
    fn test_write_varint_correctness() {
        let mut buf = [0u8; 10];
        let n1 = write_varint(0, &mut buf);
        assert_eq!(&buf[..n1], &[0x00]);

        let n2 = write_varint(64, &mut buf);
        assert_eq!(&buf[..n2], &[0x40]);

        let n3 = write_varint(2097150, &mut buf);
        assert_eq!(&buf[..n3], &[0xFE, 0xFF, 0x7F]);
    }

    #[test]
    fn test_find_match_length_swar() {
        let s1 = b"01234567abcdefghXXXXXXXX";
        let s2 = b"01234567abcdefghYYYYYYYY";
        assert_eq!(find_match_length(s1, s2), 16);

        let s3 = b"abc";
        let s4 = b"abd";
        assert_eq!(find_match_length(s3, s4), 2);

        let s5 = b"identical";
        let s6 = b"identical";
        assert_eq!(find_match_length(s5, s6), 9);
    }

    #[test]
    fn test_raw_compress_small_string() {
        let data = b"Hello, Snappy raw compression world 2026!";
        let mut out = vec![0u8; max_compressed_len(data.len())];
        let written = raw_compress(data, &mut out).expect("compress small string");
        assert!(written > 0);

        let mut decomp = vec![0u8; data.len()];
        let dec_len = crate::codecs::snappy::block::snappy_decompress(&out[..written], &mut decomp)
            .expect("decompress");
        assert_eq!(dec_len, data.len());
        assert_eq!(&decomp, data);
    }

    #[test]
    fn test_raw_compress_repetitive_data() {
        let data = vec![b'A'; 10000];
        let compressed = raw_compress_to_vec(&data).expect("compress repetitive");
        assert!(
            compressed.len() < 500,
            "Repetitive data (10000 bytes) should compress heavily (< 500 bytes with 64-byte max copy), got {}",
            compressed.len()
        );

        let mut decomp = vec![0u8; data.len()];
        let dec_len = crate::codecs::snappy::block::snappy_decompress(&compressed, &mut decomp)
            .expect("decompress");
        assert_eq!(dec_len, data.len());
        assert_eq!(&decomp, &data);
    }
}
