// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe, RAII-governed single-format compression and character encoding codecs.

pub mod branch;
pub mod brotli;
pub mod bzip2;
pub mod chardet;
pub mod deflate;
pub mod fast_blocks;
pub mod libdeflate;
pub mod lz4;
pub mod lzfse;
pub mod lzma;
pub mod lzma2;
pub mod ppmd;
pub use ppmd as ppmd_suballoc;
pub mod snappy;
pub mod zopfli;
pub mod zstd;
pub mod zstd_seekable;


// Safe RAII stream byte counting wrappers

use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Zero-overhead counting writer that wraps an underlying `Write` sink and tracks total bytes written.
pub struct CountingWriter<W: Write> {
    inner: W,
    count: Arc<AtomicU64>,
}

impl<W: Write> CountingWriter<W> {
    /// Creates a new counting writer wrapping `inner` with initial count 0.
    #[inline]
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Returns a shared handle to the atomic byte counter.
    #[inline]
    pub fn counter(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.count)
    }

    /// Returns total bytes written through this writer.
    #[inline]
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Unwraps and returns the underlying writer.
    #[inline]
    pub fn into_inner(self) -> W {
        self.inner
    }

    /// Returns an immutable reference to the underlying writer.
    #[inline]
    pub fn get_ref(&self) -> &W {
        &self.inner
    }

    /// Returns a mutable reference to the underlying writer.
    #[inline]
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }
}

impl<W: Write> Write for CountingWriter<W> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.count.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Zero-overhead counting reader that wraps an underlying `Read` source and tracks total bytes read.
pub struct CountingReader<R: Read> {
    inner: R,
    count: Arc<AtomicU64>,
}

impl<R: Read> CountingReader<R> {
    /// Creates a new counting reader wrapping `inner` with initial count 0.
    #[inline]
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Returns a shared handle to the atomic byte counter.
    #[inline]
    pub fn counter(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.count)
    }

    /// Returns total bytes read through this reader.
    #[inline]
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Unwraps and returns the underlying reader.
    #[inline]
    pub fn into_inner(self) -> R {
        self.inner
    }

    /// Returns an immutable reference to the underlying reader.
    #[inline]
    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    /// Returns a mutable reference to the underlying reader.
    #[inline]
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }
}

impl<R: Read> Read for CountingReader<R> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.count.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

pub use branch::*;
pub use brotli::{
    brotli_compress, brotli_compress_bound, brotli_compress_file, brotli_compress_stream_pipe,
    brotli_compress_to_vec, brotli_decompress, brotli_decompress_file,
    brotli_decompress_stream_pipe, brotli_decompress_to_vec, BrotliCompressorWriter, BrotliConfig,
    BrotliDecompressorReader, BROTLI_PIPE_BUFFER_SIZE,
};
pub use bzip2::{
    bzip2_compress, bzip2_compress_bound, bzip2_compress_stream_pipe, bzip2_compress_to_vec,
    bzip2_decompress, bzip2_decompress_stream_pipe, bzip2_decompress_to_vec,
    bzip2_inspect_header, Bzip2Compressor, Bzip2Decompressor, Bzip2HeaderInfo,
    BZIP2_PIPE_BUFFER_SIZE, BZ_EOS_BLOCK_MAGIC, BZ_MAGIC, BZ_PI_BLOCK_MAGIC,
};
pub use deflate::*;
pub use fast_blocks::*;
pub use libdeflate::{
    libdeflate_deflate_compress, libdeflate_deflate_decompress, libdeflate_gzip_compress,
    libdeflate_gzip_decompress, libdeflate_validate, libdeflate_zlib_compress,
    libdeflate_zlib_decompress, ContainerFormat,
};
pub use lz4::{
    copy_small_offset, copy_small_offset_ptr, header_checksum, is_lz4_frame_magic,
    is_lz4_legacy_magic, is_lz4_skippable_magic, lz4_compress, lz4_compress_bound,
    lz4_compress_fast, lz4_compress_fast_rust, lz4_compress_fast_rust_to_vec, lz4_compress_hc,
    lz4_compress_hc_to_vec, lz4_compress_to_vec, lz4_decompress, lz4_decompress_custom_to_vec,
    lz4_decompress_inplace_buffer_size, lz4_decompress_inplace_margin, lz4_decompress_safe_custom,
    lz4_decompress_safe_ext_dict, lz4_decompress_safe_ext_dict_partial,
    lz4_decompress_safe_ext_dict_to_vec, lz4_decompress_safe_partial, lz4_decompress_to_vec,
    lz4_hash4, lz4_hash4_bytes, lz4_hash5, lz4_hash5_slice, lz4_hash8, BlockIndependence,
    BlockMaxSize, FrameDescriptor, Lz4DictCompressor, Lz4FastCompressor, Lz4FrameDecoder,
    Lz4FrameEncoder, Lz4PreloadedDict, TableType, DEC64_TABLE, INC32_TABLE,
    KNUTH_GOLDEN_RATIO_32, LASTLITERALS, LZ4F_MAGICNUMBER, LZ4F_MAGIC_LEGACY,
    LZ4F_MAGIC_SKIPPABLE_END, LZ4F_MAGIC_SKIPPABLE_MASK, LZ4F_MAGIC_SKIPPABLE_START,
    LZ4F_VERSION_1, LZ4_64K_LIMIT, LZ4_DISTANCE_MAX, LZ4_FAST_LOOP_MARGIN, LZ4_HASH_LOG,
    LZ4_HASH_SIZE, LZ4_MAX_TOKEN_LITERAL_LEN, LZ4_MAX_TOKEN_MATCH_LEN, LZ4_MIN_MATCH, MFLIMIT,
    MINMATCH, PRIME_5BYTES_64, PRIME_8BYTES_64,
};
pub use lzfse::{
    lzfse_compress, lzfse_compress_bound, lzfse_compress_raw, lzfse_compress_stream,
    lzfse_compress_to_vec, lzfse_decompress, lzfse_decompress_raw, lzfse_decompress_stream,
    lzfse_decompress_to_vec, lzfse_validate, lzvn_compress, lzvn_compress_bound,
    lzvn_compress_raw, lzvn_compress_to_vec, lzvn_decompress, lzvn_decompress_raw,
    lzvn_decompress_to_vec, lzvn_validate, LzfseReader, LzfseWriter, LZFSE_BLOCK_CHUNK_SIZE,
};
pub use lzma::*;
pub use lzma2::*;

pub use ppmd::*;
pub use snappy::{
    crc32c, crc32c_update, decode_varint32, emit_copy1_tag, emit_copy2_tag, emit_copy4_tag,
    emit_literal_tag, encode_varint32, is_framed_snappy, mask_crc32c, parse_element,
    parse_tag_header, parse_varint, raw_compress, raw_compress_to_vec, raw_decompress,
    raw_decompress_to_vec, raw_uncompressed_length, raw_validate, snappy_compress,
    snappy_compress_bound, snappy_compress_file, snappy_compress_framed, snappy_compress_raw,
    snappy_compress_stream_pipe, snappy_compress_to_vec, snappy_decompress,
    snappy_decompress_file, snappy_decompress_framed, snappy_decompress_raw,
    snappy_decompress_stream_pipe, snappy_decompress_to_vec, snappy_frame_decode,
    snappy_frame_decode_to_vec, snappy_frame_encode, snappy_frame_encode_to_vec,
    snappy_frame_max_encoded_length, snappy_frame_validate, snappy_uncompressed_length,
    snappy_validate, snappy_validate_bounded, snappy_validate_framed, snappy_validate_raw,
    unmask_crc32c, varint32_len, CASTAGNOLI_POLYNOMIAL, LENGTH_MINUS_OFFSET_TABLE,
    MAX_VARINT32_BYTES, SNAPPY_CRC_MASK_DELTA, SNAPPY_MAX_CHUNK_SIZE, SNAPPY_PIPE_BUFFER_SIZE,
    SNAPPY_STREAM_IDENTIFIER, SnappyCrc32cHasher, SnappyElement, SnappyError,
    SnappyFramedReader, SnappyFramedWriter, SnappyHashTable, SnappyTagHeader, SnappyTagType,
};
pub use zstd::{
    fse_compress, fse_compress_bound, fse_decompress, huf0_compress1x, huf0_compress4x,
    huf0_compress_bound, huf0_decompress1x, huf0_decompress4x, with_thread_local_zstd_cctx,
    with_thread_local_zstd_dctx, zstd_compress, zstd_compress_advanced, zstd_compress_bound,
    zstd_compress_ldm, zstd_compress_stream_pipe, zstd_compress_with_dict, zstd_decompress,
    zstd_decompress_stream_pipe, zstd_decompress_with_dict, zstd_get_decompressed_size,
    zstd_train_dictionary, CDict, DDict, SeekFrameInfo, SeekTableDecoder, SeekTableEncoder,
    SeekTableEntry, SeekableError, ZstdCCtx, ZstdCParameter, ZstdConfig, ZstdDCtx, ZstdDParameter,
    ZstdDictionary, ZstdDictionaryManager, ZstdEndDirective, ZstdInBuffer, ZstdOutBuffer,
    ZstdSeekableReader, ZstdSeekableWriter, ZstdStreamReader, ZstdStreamWriter,
    DEFAULT_SEEKABLE_FRAME_SIZE, SEEKABLE_FOOTER_SIZE, SEEKABLE_MAGIC_NUMBER,
    SEEK_TABLE_FLAG_CHECKSUM, SKIPPABLE_HEADER_SIZE, SKIPPABLE_MAGIC_NUMBER,
    ZSTD_PIPE_BUFFER_SIZE, ZSTD_STANDARD_DICTIONARY_SIZE_BYTES, ZSTD_STREAM_BUFFER_SIZE,
};
pub use zopfli::{
    estimate_entropy_cost, zopfli_compress, zopfli_compress_deflate, zopfli_compress_gzip,
    zopfli_compress_zlib, BlockStats, CumulativeHistogram, ZopfliBlockSplitter, ZopfliCostModel,
    ZopfliEncoder, ZopfliFormat, ZopfliHash, ZopfliMatchCache, ZopfliOptions,
    ZopfliShortestPathMatcher, ZopfliSqueeze, ZopfliToken, DYNAMIC_HEADER_COST_BITS,
    END_OF_BLOCK_SYM, MIN_BLOCK_SIZE, NUM_DIST_SYMS, NUM_LITLEN_SYMS, SPLIT_GAIN_THRESHOLD_BITS,
};
