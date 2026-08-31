// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, high-performance LZMA2 Chunk Control Parser and 12-State HMM Stream Decoder.
//!
//! Implements strict chunk header decoding (EOS 0x00, Uncompressed 0x01/0x02, Compressed 0x80..=0xFF),
//! invalid control byte defense (0x03..=0x7F), 12-state finite Markov state machine tracking,
//! dynamic `lc`/`lp`/`pb` probability tables, and sliding dictionary window management.

pub mod dict;
pub mod header;

pub use dict::{Lzma2Dict, LZMA2_DEFAULT_DICT_SIZE};
pub use header::{
    Lzma2ChunkHeader, Lzma2DecodeError, LZMA2_MAX_PACK_CHUNK_SIZE, LZMA2_MAX_UNPACK_CHUNK_SIZE,
};

use crate::codecs::lzma::range_coder::{RangeCoderError, RangeDecoder, RangeEncoder};
use crate::codecs::lzma::state_machine::{
    LenCoderProbs, LiteralProperties, LzmaProbTable, LzmaState, NUM_ALIGN_BITS,
    NUM_LEN_TO_POS_STATES, NUM_POS_DECODERS,
};

/// 12-State Hidden Markov Model (HMM) Stream Decoder for LZMA2.
#[derive(Debug, Clone)]
pub struct Lzma2StreamDecoder {
    state: LzmaState,
    reps: [usize; 4],
    probs: LzmaProbTable,
    props: LiteralProperties,
    dict: Lzma2Dict,
    uncompressed_pos: usize,
    is_eos: bool,
    need_dict_reset: bool,
    need_props: bool,
}

impl Default for Lzma2StreamDecoder {
    fn default() -> Self {
        Self::new(LZMA2_DEFAULT_DICT_SIZE)
    }
}

impl Lzma2StreamDecoder {
    /// Creates a new `Lzma2StreamDecoder` with default properties (lc=3, lp=0, pb=2).
    #[must_use]
    pub fn new(dict_size: usize) -> Self {
        let props = LiteralProperties::default();
        let probs = LzmaProbTable::new(props);
        Self {
            state: LzmaState::default(),
            reps: [0; 4],
            probs,
            props,
            dict: Lzma2Dict::new(dict_size),
            uncompressed_pos: 0,
            is_eos: false,
            need_dict_reset: true,
            need_props: true,
        }
    }

    /// Resets the full decoder state machine, probabilities, and sliding dictionary.
    pub fn reset(&mut self) {
        self.state = LzmaState::default();
        self.reps = [0; 4];
        self.props = LiteralProperties::default();
        self.probs = LzmaProbTable::new(self.props);
        self.dict.reset();
        self.uncompressed_pos = 0;
        self.is_eos = false;
        self.need_dict_reset = true;
        self.need_props = true;
    }

    /// Returns `true` if the End-of-Stream (0x00) marker has been processed.
    #[inline(always)]
    pub const fn is_eos(&self) -> bool {
        self.is_eos
    }

    /// Returns the current 12-state HMM state.
    #[inline(always)]
    pub const fn current_state(&self) -> LzmaState {
        self.state
    }

    /// Returns the four repeat distances (`rep0..=rep3`).
    #[inline(always)]
    pub const fn repeat_distances(&self) -> [usize; 4] {
        self.reps
    }

    /// Returns the current literal context properties (`lc`, `lp`, `pb`).
    #[inline(always)]
    pub const fn literal_properties(&self) -> LiteralProperties {
        self.props
    }

    /// Returns total uncompressed bytes decoded by this stream.
    #[inline(always)]
    pub const fn total_uncompressed_bytes(&self) -> usize {
        self.uncompressed_pos
    }

    /// Decodes a single parsed LZMA2 chunk given its header and payload slice.
    ///
    /// # Errors
    /// Returns `Lzma2DecodeError` on corrupted payload, truncated data, or invalid distances.
    pub fn decode_chunk(
        &mut self,
        header: &Lzma2ChunkHeader,
        payload: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<usize, Lzma2DecodeError> {
        match *header {
            Lzma2ChunkHeader::Eos => {
                self.is_eos = true;
                Ok(0)
            }
            Lzma2ChunkHeader::UncompressedResetDict { unpack_size } => {
                if payload.len() < unpack_size {
                    return Err(Lzma2DecodeError::TruncatedPayload {
                        expected: unpack_size,
                        available: payload.len(),
                    });
                }
                self.dict.reset();
                self.reps = [0; 4];
                self.state = LzmaState::default();
                self.need_dict_reset = false;
                self.need_props = true;
                let chunk_data = &payload[..unpack_size];
                self.dict.put_slice(chunk_data);
                out.extend_from_slice(chunk_data);
                self.uncompressed_pos += unpack_size;
                Ok(unpack_size)
            }
            Lzma2ChunkHeader::UncompressedNoReset { unpack_size } => {
                if self.need_dict_reset {
                    return Err(Lzma2DecodeError::CorruptData(
                        "First LZMA2 chunk must reset dictionary".to_string(),
                    ));
                }
                if payload.len() < unpack_size {
                    return Err(Lzma2DecodeError::TruncatedPayload {
                        expected: unpack_size,
                        available: payload.len(),
                    });
                }
                let chunk_data = &payload[..unpack_size];
                self.dict.put_slice(chunk_data);
                out.extend_from_slice(chunk_data);
                self.uncompressed_pos += unpack_size;
                Ok(unpack_size)
            }
            Lzma2ChunkHeader::Compressed {
                mode,
                unpack_size,
                pack_size,
                props,
            } => {
                if payload.len() < pack_size {
                    return Err(Lzma2DecodeError::TruncatedPayload {
                        expected: pack_size,
                        available: payload.len(),
                    });
                }
                if self.need_dict_reset && mode != 3 {
                    return Err(Lzma2DecodeError::CorruptData(
                        "First LZMA2 chunk must reset dictionary".to_string(),
                    ));
                }
                if self.need_props && mode < 2 {
                    return Err(Lzma2DecodeError::CorruptData(
                        "LZMA2 chunk requires properties to be defined".to_string(),
                    ));
                }
                self.handle_compressed_chunk(mode, unpack_size, &payload[..pack_size], props, out)
            }
        }
    }

    fn handle_compressed_chunk(
        &mut self,
        mode: u8,
        unpack_size: usize,
        compressed_payload: &[u8],
        props_byte: Option<u8>,
        out: &mut Vec<u8>,
    ) -> Result<usize, Lzma2DecodeError> {
        if mode == 3 {
            self.dict.reset();
            self.need_dict_reset = false;
        }

        if mode >= 2 {
            let prop_val = props_byte.ok_or_else(|| {
                Lzma2DecodeError::CorruptData("Missing property byte for LZMA2 mode >= 2".to_string())
            })?;
            let parsed_props = LiteralProperties::from_byte(prop_val).map_err(|_| {
                Lzma2DecodeError::CorruptData("Invalid LZMA2 literal property byte".to_string())
            })?;
            if parsed_props.lc + parsed_props.lp > 4 {
                return Err(Lzma2DecodeError::CorruptData(format!(
                    "LZMA2 literal properties lc({}) + lp({}) > 4",
                    parsed_props.lc, parsed_props.lp
                )));
            }
            self.props = parsed_props;
            self.need_props = false;
        }

        if mode >= 1 {
            self.state = LzmaState::default();
            self.reps = [0; 4];
            self.probs = LzmaProbTable::new(self.props);
        }

        let mut rd = RangeDecoder::new(compressed_payload)?;
        let mut decoded_in_chunk = 0usize;

        while decoded_in_chunk < unpack_size {
            let pos_state = self.props.pos_state(self.uncompressed_pos);
            let is_match =
                rd.decode_bit(&mut self.probs.is_match[self.state.as_usize()][pos_state])?;

            if is_match == 0 {
                // Literal symbol decoding
                let prev_byte = self.dict.last_byte();
                let byte = if self.state.is_literal() {
                    let sub = self
                        .probs
                        .literal_sub_table_mut(self.uncompressed_pos, prev_byte);
                    rd.decode_literal_byte(sub)?
                } else {
                    let match_byte = self.dict.get_byte_at_distance(self.reps[0])?;
                    let sub = self
                        .probs
                        .literal_sub_table_mut(self.uncompressed_pos, prev_byte);
                    rd.decode_matched_byte(sub, match_byte)?
                };

                self.dict.put_byte(byte);
                out.push(byte);
                self.uncompressed_pos += 1;
                decoded_in_chunk += 1;
                self.state = self.state.update_literal();
            } else {
                // Match or Repetition match
                let is_rep = rd.decode_bit(&mut self.probs.is_rep[self.state.as_usize()])?;
                if is_rep == 0 {
                    // Simple Match with new distance
                    self.reps[3] = self.reps[2];
                    self.reps[2] = self.reps[1];
                    self.reps[1] = self.reps[0];

                    let len = decode_length(&mut rd, &mut self.probs.len_coder, pos_state)?;
                    let len_to_pos_state = (len.min(NUM_LEN_TO_POS_STATES + 1) - 2).min(3);
                    let pos_slot = rd.decode_bit_tree(
                        &mut self.probs.pos_slot[len_to_pos_state],
                        6,
                    )? as usize;

                    if pos_slot < 4 {
                        self.reps[0] = pos_slot;
                    } else {
                        let num_direct_bits = ((pos_slot >> 1) - 1) as u32;
                        let base_dist = (2 | (pos_slot & 1)) << num_direct_bits;

                        if pos_slot < 14 {
                            let dist_tree = decode_pos_slot_reverse_tree(
                                &mut rd,
                                &mut self.probs.pos_decoders,
                                pos_slot,
                                num_direct_bits,
                            )?;
                            self.reps[0] = base_dist + (dist_tree as usize);
                        } else {
                            let direct_bits = rd.decode_direct_bits(
                                num_direct_bits - (NUM_ALIGN_BITS as u32),
                            )?;
                            let align_bits = rd.decode_reverse_bit_tree(
                                &mut self.probs.pos_align,
                                NUM_ALIGN_BITS as u32,
                            )?;
                            self.reps[0] = base_dist
                                + ((direct_bits as usize) << NUM_ALIGN_BITS)
                                + (align_bits as usize);
                        }
                    }

                    if self.reps[0] == 0xFFFF_FFFF || self.reps[0] == u32::MAX as usize {
                        return Err(Lzma2DecodeError::CorruptData(
                            "LZMA-level EOPM marker is forbidden in LZMA2".to_string(),
                        ));
                    }

                    self.state = self.state.update_match();
                    if decoded_in_chunk + len > unpack_size {
                        return Err(Lzma2DecodeError::CorruptData(
                            "Decoded match length exceeds chunk unpack size".to_string(),
                        ));
                    }
                    for _ in 0..len {
                        let b = self.dict.get_byte_at_distance(self.reps[0])?;
                        self.dict.put_byte(b);
                        out.push(b);
                    }
                    self.uncompressed_pos += len;
                    decoded_in_chunk += len;
                } else {
                    // Repetition match (Rep0, Rep1, Rep2, Rep3)
                    let is_rep_g0 =
                        rd.decode_bit(&mut self.probs.is_rep_g0[self.state.as_usize()])?;
                    let len = if is_rep_g0 == 0 {
                        let is_short_rep = rd.decode_bit(
                            &mut self.probs.is_rep0_long[self.state.as_usize()][pos_state],
                        )?;
                        if is_short_rep == 0 {
                            self.state = self.state.update_short_rep();
                            1
                        } else {
                            let len =
                                decode_length(&mut rd, &mut self.probs.rep_len_coder, pos_state)?;
                            self.state = self.state.update_rep();
                            len
                        }
                    } else {
                        let is_rep_g1 =
                            rd.decode_bit(&mut self.probs.is_rep_g1[self.state.as_usize()])?;
                        let dist = if is_rep_g1 == 0 {
                            let d = self.reps[1];
                            self.reps[1] = self.reps[0];
                            d
                        } else {
                            let is_rep_g2 =
                                rd.decode_bit(&mut self.probs.is_rep_g2[self.state.as_usize()])?;
                            if is_rep_g2 == 0 {
                                let d = self.reps[2];
                                self.reps[2] = self.reps[1];
                                self.reps[1] = self.reps[0];
                                d
                            } else {
                                let d = self.reps[3];
                                self.reps[3] = self.reps[2];
                                self.reps[2] = self.reps[1];
                                self.reps[1] = self.reps[0];
                                d
                            }
                        };
                        self.reps[0] = dist;
                        let len =
                            decode_length(&mut rd, &mut self.probs.rep_len_coder, pos_state)?;
                        self.state = self.state.update_rep();
                        len
                    };

                    if decoded_in_chunk + len > unpack_size {
                        return Err(Lzma2DecodeError::CorruptData(
                            "Decoded rep match length exceeds chunk unpack size".to_string(),
                        ));
                    }
                    for _ in 0..len {
                        let b = self.dict.get_byte_at_distance(self.reps[0])?;
                        self.dict.put_byte(b);
                        out.push(b);
                    }
                    self.uncompressed_pos += len;
                    decoded_in_chunk += len;
                }
            }
        }

        if rd.pos() < compressed_payload.len() || rd.code() != 0 {
            return Err(Lzma2DecodeError::CorruptData(
                "LZMA2 compressed chunk has unconsumed data or non-zero range coder residue".to_string(),
            ));
        }

        Ok(unpack_size)
    }

    /// Decompresses an entire LZMA2 stream from `src` into `dst` vector until EOS.
    ///
    /// # Errors
    /// Returns `Lzma2DecodeError` if invalid control bytes or corrupted chunks are encountered.
    pub fn decode_all(&mut self, src: &[u8], dst: &mut Vec<u8>) -> Result<usize, Lzma2DecodeError> {
        let mut cursor = 0usize;
        let mut total_unpacked = 0usize;

        while cursor < src.len() && !self.is_eos {
            let Some((header, header_len)) = Lzma2ChunkHeader::parse(&src[cursor..])? else {
                return Err(Lzma2DecodeError::TruncatedHeader);
            };
            cursor += header_len;

            if header.is_eos() {
                self.is_eos = true;
                break;
            }

            let pack_size = header.pack_size();
            if cursor + pack_size > src.len() {
                return Err(Lzma2DecodeError::TruncatedPayload {
                    expected: pack_size,
                    available: src.len() - cursor,
                });
            }

            let payload = &src[cursor..cursor + pack_size];
            cursor += pack_size;

            let unpacked = self.decode_chunk(&header, payload, dst)?;
            total_unpacked += unpacked;
        }

        Ok(total_unpacked)
    }

    /// Incremental streaming decompression consuming from `src` and writing to `dst` slice.
    ///
    /// Returns `(bytes_in_consumed, bytes_out_produced, is_eos)`.
    pub fn decompress_stream(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
    ) -> Result<(usize, usize, bool), Lzma2DecodeError> {
        let mut temp_out = Vec::with_capacity(dst.len());
        let mut cursor = 0usize;

        while cursor < src.len() && !self.is_eos && temp_out.len() < dst.len() {
            let Some((header, header_len)) = Lzma2ChunkHeader::parse(&src[cursor..])? else {
                break;
            };

            if header.is_eos() {
                cursor += header_len;
                self.is_eos = true;
                break;
            }

            let pack_size = header.pack_size();
            if cursor + header_len + pack_size > src.len() {
                break; // Need more input bytes
            }

            cursor += header_len;
            let payload = &src[cursor..cursor + pack_size];
            cursor += pack_size;

            self.decode_chunk(&header, payload, &mut temp_out)?;
        }

        let bytes_out = temp_out.len().min(dst.len());
        dst[..bytes_out].copy_from_slice(&temp_out[..bytes_out]);
        Ok((cursor, bytes_out, self.is_eos))
    }
}

/// Decodes length symbol (2..=273) from choice trees and pos_state.
#[inline]
fn decode_length(
    rd: &mut RangeDecoder<'_>,
    len_coder: &mut LenCoderProbs,
    pos_state: usize,
) -> Result<usize, RangeCoderError> {
    let choice1 = rd.decode_bit(&mut len_coder.choice1)?;
    if choice1 == 0 {
        let sym = rd.decode_bit_tree(&mut len_coder.low[pos_state], 3)?;
        Ok(2 + (sym as usize))
    } else {
        let choice2 = rd.decode_bit(&mut len_coder.choice2)?;
        if choice2 == 0 {
            let sym = rd.decode_bit_tree(&mut len_coder.mid[pos_state], 3)?;
            Ok(2 + 8 + (sym as usize))
        } else {
            let sym = rd.decode_bit_tree(&mut len_coder.high, 8)?;
            Ok(2 + 8 + 8 + (sym as usize))
        }
    }
}

/// Decodes reverse bit tree for distance slots 4..13 from `pos_decoders` array.
#[inline]
fn decode_pos_slot_reverse_tree(
    rd: &mut RangeDecoder<'_>,
    pos_decoders: &mut [u16; NUM_POS_DECODERS],
    pos_slot: usize,
    num_direct_bits: u32,
) -> Result<u32, RangeCoderError> {
    let base = (2 | (pos_slot & 1)) << num_direct_bits;
    let start_offset = base - pos_slot;
    let mut symbol = 1usize;
    let mut dist = 0u32;
    for i in 0..num_direct_bits {
        let prob_idx = start_offset + symbol - 1;
        if prob_idx >= pos_decoders.len() {
            return Err(RangeCoderError::InvalidBitTreeSymbol);
        }
        let bit = rd.decode_bit(&mut pos_decoders[prob_idx])?;
        symbol = (symbol << 1) | (bit as usize);
        dist |= bit << i;
    }
    Ok(dist)
}

/// Encodes raw uncompressed data into an LZMA2 compressed chunk payload (for testing and synthetic payloads).
pub fn encode_lzma2_literal_chunk(
    raw_data: &[u8],
    props: LiteralProperties,
) -> (Lzma2ChunkHeader, Vec<u8>) {
    let mut enc = RangeEncoder::new();
    let mut probs = LzmaProbTable::new(props);
    let mut state = LzmaState::default();
    let mut dict_prev_byte = 0u8;
    let mut payload = Vec::new();

    for (i, &byte) in raw_data.iter().enumerate() {
        let pos_state = props.pos_state(i);
        enc.encode_bit(&mut probs.is_match[state.as_usize()][pos_state], 0, &mut payload);
        let sub = probs.literal_sub_table_mut(i, dict_prev_byte);
        enc.encode_literal_byte(sub, byte, &mut payload);
        dict_prev_byte = byte;
        state = state.update_literal();
    }
    enc.finish(&mut payload);

    let header = Lzma2ChunkHeader::Compressed {
        mode: 3, // reset dict + state + probs
        unpack_size: raw_data.len(),
        pack_size: payload.len(),
        props: Some(props.to_byte()),
    };

    (header, payload)
}
