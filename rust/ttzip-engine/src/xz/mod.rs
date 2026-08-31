// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Native high-throughput XZ container and streaming engine.

pub mod bcj;
pub mod block;
pub mod check;
pub mod checksum;
pub mod decoder;
pub mod filters;
pub mod header;
pub mod index;
pub mod payload;
pub mod seekable;
pub mod types;
pub mod vli;
pub mod writer;

pub use bcj::{
    arm, arm64, riscv, stream, x86, BcjArm, BcjArm64, BcjRiscv, BcjStreamFilter, BcjX86,
    BranchFilter,
};
pub use block::*;
pub use checksum::*;

pub use decoder::{
    xz_decompress, XzDecodeError, XzDecoderState, XzStreamDecoder, DEFAULT_XZ_MEMLIMIT,
};
pub use filters::{
    apply_filters_decode, arm_decode, arm_thumb_decode, delta_decode, ia64_decode, powerpc_decode,
    sparc_decode,
};
pub use header::*;
pub use index::*;
pub use payload::{decompress_block_payload, decompress_lzma2_payload, lzma2_dict_size_from_prop};
pub use seekable::{XzSeekableReader, DEFAULT_XZ_SEEK_MEMLIMIT};
pub use types::*;
pub use vli::*;
pub use writer::{
    xz_compress, XzBcjType, XzBlockEncoder, XzEncoderOptions, XzParallelStreamWriter,
    XzStreamWriter,
};
