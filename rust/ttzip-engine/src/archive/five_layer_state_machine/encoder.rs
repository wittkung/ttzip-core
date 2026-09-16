// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Pure functional companion frame encoder creating 5-layer TTZ1 frames.

use super::{
    BlockHeader, BlockType, FrameHeader, Lz77Sequence, MAX_BLOCK_SIZE_128KB,
};

/// Pure functional companion encoder creating 5-layer TTZ1 frames.
pub struct FiveLayerFrameEncoder;

impl FiveLayerFrameEncoder {
    /// Encodes a raw byte slice into a 5-layer TTZ1 frame.
    pub fn encode_frame(raw_data: &[u8], use_checksum: bool) -> Vec<u8> {
        let mut out = Vec::with_capacity(raw_data.len() + 64);
        let header = FrameHeader {
            version: 1,
            has_checksum: use_checksum,
            dictionary_id: None,
            expected_uncompressed_size: Some(raw_data.len() as u64),
        };
        let mut hdr_buf = [0u8; 32];
        let hdr_len = header.write_to_slice(&mut hdr_buf).unwrap();
        out.extend_from_slice(&hdr_buf[..hdr_len]);

        if raw_data.is_empty() {
            let blk = BlockHeader {
                block_type: BlockType::RawUncompressed,
                is_last_block: true,
                uncompressed_size: 0,
                compressed_size: 0,
            };
            let mut blk_buf = [0u8; 9];
            blk.write_to_slice(&mut blk_buf).unwrap();
            out.extend_from_slice(&blk_buf);
        } else {
            let chunks: Vec<&[u8]> = raw_data.chunks(MAX_BLOCK_SIZE_128KB).collect();
            for (idx, chunk) in chunks.iter().enumerate() {
                let is_last = idx == chunks.len() - 1;
                if chunk.len() > 16 && chunk.iter().all(|&b| b == chunk[0]) {
                    let blk = BlockHeader {
                        block_type: BlockType::RleRepeatedByte,
                        is_last_block: is_last,
                        uncompressed_size: chunk.len() as u32,
                        compressed_size: 1,
                    };
                    let mut blk_buf = [0u8; 9];
                    blk.write_to_slice(&mut blk_buf).unwrap();
                    out.extend_from_slice(&blk_buf);
                    out.push(chunk[0]);
                } else {
                    let mut compressed = false;
                    if chunk.len() > 32 {
                        let (seqs, lits) = Self::simple_lz77_compress(chunk);
                        let payload_len = 4 + seqs.len() * 6 + lits.len();
                        if payload_len < chunk.len() {
                            let blk = BlockHeader {
                                block_type: BlockType::CompressedLz77Entropy,
                                is_last_block: is_last,
                                uncompressed_size: chunk.len() as u32,
                                compressed_size: payload_len as u32,
                            };
                            let mut blk_buf = [0u8; 9];
                            blk.write_to_slice(&mut blk_buf).unwrap();
                            out.extend_from_slice(&blk_buf);
                            out.extend_from_slice(&(seqs.len() as u16).to_le_bytes());
                            out.extend_from_slice(&(lits.len() as u16).to_le_bytes());
                            for s in &seqs {
                                out.extend_from_slice(&(s.literal_length as u16).to_le_bytes());
                                out.extend_from_slice(&(s.match_offset as u16).to_le_bytes());
                                out.extend_from_slice(&(s.match_length as u16).to_le_bytes());
                            }
                            out.extend_from_slice(&lits);
                            compressed = true;
                        }
                    }
                    if !compressed {
                        let blk = BlockHeader {
                            block_type: BlockType::RawUncompressed,
                            is_last_block: is_last,
                            uncompressed_size: chunk.len() as u32,
                            compressed_size: chunk.len() as u32,
                        };
                        let mut blk_buf = [0u8; 9];
                        blk.write_to_slice(&mut blk_buf).unwrap();
                        out.extend_from_slice(&blk_buf);
                        out.extend_from_slice(chunk);
                    }
                }
            }
        }

        if use_checksum {
            let crc = crc32fast::hash(raw_data);
            out.extend_from_slice(&crc.to_le_bytes());
        }
        out
    }

    fn simple_lz77_compress(input: &[u8]) -> (Vec<Lz77Sequence>, Vec<u8>) {
        let (mut sequences, mut literals) = (Vec::new(), Vec::new());
        let (mut pos, mut lit_start) = (0, 0);

        while pos < input.len() {
            let (mut best_len, mut best_off) = (0, 0);
            let window_start = pos.saturating_sub(32768);

            if pos + 4 <= input.len() {
                for candidate in (window_start..pos).rev() {
                    let mut match_len = 0;
                    while pos + match_len < input.len()
                        && input[candidate + match_len] == input[pos + match_len]
                        && match_len < 255
                    {
                        match_len += 1;
                    }
                    if match_len > best_len {
                        best_len = match_len;
                        best_off = pos - candidate;
                        if match_len >= 32 {
                            break;
                        }
                    }
                }
            }

            if best_len >= 4 {
                literals.extend_from_slice(&input[lit_start..pos]);
                sequences.push(Lz77Sequence::new(
                    (pos - lit_start) as u32,
                    best_off as u32,
                    best_len as u32,
                ));
                pos += best_len;
                lit_start = pos;
            } else {
                pos += 1;
            }
        }

        if lit_start < input.len() {
            literals.extend_from_slice(&input[lit_start..]);
        }
        (sequences, literals)
    }
}
