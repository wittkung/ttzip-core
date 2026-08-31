// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Apple `LZFSE` streaming block container finite state machine (FSM).

use super::block::{parse_block_header, BvxMagic, LzfseBlockHeader};
use crate::types::TTZipStatus;

// MARK: - Streaming Block Container FSM

/// Lifecycle state for the LZFSE block container streaming finite state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LzfseFsmState {
    /// Awaiting next block magic and header.
    NeedHeader,
    /// Header parsed, actively streaming payload bytes for current block.
    InBlock {
        /// Active block header.
        header: Box<LzfseBlockHeader>,
        /// Number of payload bytes consumed so far for this block.
        payload_consumed: usize,
    },
    /// Stream cleanly terminated with `bvx$` marker.
    EndOfStream,
    /// Terminal error encountered during parsing.
    Error(TTZipStatus),
}

/// Result of extracting a complete LZFSE block from a continuous buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LzfseParsedBlock<'a> {
    /// Parsed block header.
    pub header: LzfseBlockHeader,
    /// Direct slice reference to block payload.
    pub payload: &'a [u8],
    /// Total bytes consumed from source buffer (header + payload).
    pub total_consumed: usize,
}

/// Single step result from driving `LzfseBlockFsm`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LzfseFsmStep<'a> {
    /// A new block header was successfully parsed and validated.
    HeaderParsed(&'a LzfseBlockHeader),
    /// A chunk of block payload bytes is ready for processing.
    PayloadChunk {
        /// Associated block header.
        header: &'a LzfseBlockHeader,
        /// Slice of payload bytes from current input chunk.
        data: &'a [u8],
    },
    /// The current block payload has completed.
    BlockComplete(&'a LzfseBlockHeader),
    /// The end of stream marker (`bvx$`) was parsed.
    EndOfStream,
    /// Additional input bytes are required to proceed.
    NeedMoreInput {
        /// Minimum number of bytes needed to parse current construct.
        required_bytes: usize,
    },
}

/// Streaming finite state machine (FSM) for parsing LZFSE multi-block container streams.
#[derive(Debug, Clone)]
pub struct LzfseBlockFsm {
    state: LzfseFsmState,
    current_header: Option<LzfseBlockHeader>,
    blocks_processed: usize,
    total_raw_bytes: u64,
    total_payload_bytes: u64,
}

impl Default for LzfseBlockFsm {
    fn default() -> Self {
        Self::new()
    }
}

impl LzfseBlockFsm {
    /// Creates a fresh LZFSE streaming finite state machine.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: LzfseFsmState::NeedHeader,
            current_header: None,
            blocks_processed: 0,
            total_raw_bytes: 0,
            total_payload_bytes: 0,
        }
    }

    /// Returns the current state of the state machine.
    #[must_use]
    pub fn state(&self) -> &LzfseFsmState {
        &self.state
    }

    /// Resets the finite state machine to initial pristine state.
    pub fn reset(&mut self) {
        self.state = LzfseFsmState::NeedHeader;
        self.current_header = None;
        self.blocks_processed = 0;
        self.total_raw_bytes = 0;
        self.total_payload_bytes = 0;
    }

    /// Returns `true` if the end of stream marker (`bvx$`) has been reached.
    #[must_use]
    pub fn is_end_of_stream(&self) -> bool {
        matches!(self.state, LzfseFsmState::EndOfStream)
    }

    /// Cumulative decoded raw uncompressed bytes accounted across all parsed blocks.
    #[must_use]
    pub fn total_raw_bytes(&self) -> u64 {
        self.total_raw_bytes
    }

    /// Cumulative encoded payload bytes accounted across all parsed blocks.
    #[must_use]
    pub fn total_payload_bytes(&self) -> u64 {
        self.total_payload_bytes
    }

    /// Total count of successfully parsed blocks.
    #[must_use]
    pub fn blocks_processed(&self) -> usize {
        self.blocks_processed
    }

    /// Advances the state machine with source buffer `src`.
    ///
    /// Returns `(step, bytes_consumed_from_src)` on success.
    pub fn feed<'a>(&'a mut self, src: &'a [u8]) -> Result<(LzfseFsmStep<'a>, usize), TTZipStatus> {
        match &mut self.state {
            LzfseFsmState::EndOfStream => Ok((LzfseFsmStep::EndOfStream, 0)),
            LzfseFsmState::Error(status) => Err(*status),
            LzfseFsmState::NeedHeader => {
                if src.len() < 4 {
                    return Ok((LzfseFsmStep::NeedMoreInput { required_bytes: 4 }, 0));
                }

                let (header, header_len) = match parse_block_header(src) {
                    Ok(res) => res,
                    Err(err) => {
                        self.state = LzfseFsmState::Error(err);
                        return Err(err);
                    }
                };

                if header.magic == BvxMagic::EndOfStream {
                    self.state = LzfseFsmState::EndOfStream;
                    self.blocks_processed += 1;
                    return Ok((LzfseFsmStep::EndOfStream, header_len));
                }

                self.total_raw_bytes += header.n_raw_bytes as u64;
                let payload_len = match header.magic {
                    BvxMagic::RawUncompressed => header.n_raw_bytes as usize,
                    BvxMagic::CompressedLZVN => header.n_payload_bytes as usize,
                    BvxMagic::CompressedV1 | BvxMagic::CompressedV2 => {
                        (header.n_literal_payload_bytes + header.n_lmd_payload_bytes) as usize
                    }
                    BvxMagic::EndOfStream => 0,
                };
                self.total_payload_bytes += payload_len as u64;

                self.current_header = Some(header);
                let hdr_ref = self.current_header.as_ref().unwrap();

                if payload_len == 0 {
                    self.blocks_processed += 1;
                    let step = LzfseFsmStep::BlockComplete(hdr_ref);
                    self.state = LzfseFsmState::NeedHeader;
                    Ok((step, header_len))
                } else {
                    self.state = LzfseFsmState::InBlock {
                        header: Box::new(hdr_ref.clone()),
                        payload_consumed: 0,
                    };
                    Ok((
                        LzfseFsmStep::HeaderParsed(self.current_header.as_ref().unwrap()),
                        header_len,
                    ))
                }
            }
            LzfseFsmState::InBlock {
                header,
                payload_consumed,
            } => {
                let total_payload = match header.magic {
                    BvxMagic::RawUncompressed => header.n_raw_bytes as usize,
                    BvxMagic::CompressedLZVN => header.n_payload_bytes as usize,
                    BvxMagic::CompressedV1 | BvxMagic::CompressedV2 => {
                        (header.n_literal_payload_bytes + header.n_lmd_payload_bytes) as usize
                    }
                    BvxMagic::EndOfStream => 0,
                };

                let remaining = total_payload.saturating_sub(*payload_consumed);
                if remaining == 0 {
                    self.blocks_processed += 1;
                    self.current_header = Some((**header).clone());
                    let hdr_ref = self.current_header.as_ref().unwrap();
                    let step = LzfseFsmStep::BlockComplete(hdr_ref);
                    self.state = LzfseFsmState::NeedHeader;
                    return Ok((step, 0));
                }

                if src.is_empty() {
                    return Ok((
                        LzfseFsmStep::NeedMoreInput {
                            required_bytes: remaining,
                        },
                        0,
                    ));
                }

                let chunk_size = src.len().min(remaining);
                *payload_consumed += chunk_size;

                self.current_header = Some((**header).clone());
                let hdr_ref = self.current_header.as_ref().unwrap();
                let step = LzfseFsmStep::PayloadChunk {
                    header: hdr_ref,
                    data: &src[..chunk_size],
                };

                if *payload_consumed >= total_payload {
                    self.blocks_processed += 1;
                    self.state = LzfseFsmState::NeedHeader;
                }

                Ok((step, chunk_size))
            }
        }
    }

    /// Convenience helper to extract the next complete block from `src`.
    pub fn parse_complete_block(
        src: &[u8],
    ) -> Result<Option<LzfseParsedBlock<'_>>, TTZipStatus> {
        if src.len() < 4 {
            return Ok(None);
        }

        let magic_u32 = u32::from_le_bytes(
            src[0..4]
                .try_into()
                .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
        );
        let magic = match BvxMagic::from_u32(magic_u32) {
            Some(m) => m,
            None => return Err(TTZipStatus::ErrCorruptHeader),
        };

        let min_header_size = match magic {
            BvxMagic::EndOfStream => 4,
            BvxMagic::RawUncompressed => 8,
            BvxMagic::CompressedLZVN => 12,
            BvxMagic::CompressedV2 => 32,
            BvxMagic::CompressedV1 => 770,
        };

        if src.len() < min_header_size {
            return Ok(None);
        }

        let (header, header_len) = parse_block_header(src)?;
        let payload_len = match header.magic {
            BvxMagic::RawUncompressed => header.n_raw_bytes as usize,
            BvxMagic::CompressedLZVN => header.n_payload_bytes as usize,
            BvxMagic::CompressedV1 | BvxMagic::CompressedV2 => {
                (header.n_literal_payload_bytes + header.n_lmd_payload_bytes) as usize
            }
            BvxMagic::EndOfStream => 0,
        };

        let total_consumed = header_len + payload_len;
        if src.len() < total_consumed {
            return Ok(None);
        }

        let payload = &src[header_len..total_consumed];
        Ok(Some(LzfseParsedBlock {
            header,
            payload,
            total_consumed,
        }))
    }
}
