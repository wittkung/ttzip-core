// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Types, parameters, buffer definitions and FFI bindings for Facebook `Zstandard` (zstd),
//! Pre-trained Dictionaries, Long Distance Matching (LDM), and FSE/Huff0 entropy micro-kernels.

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ZstdCParameter {
    CompressionLevel = 100,
    WindowLog = 101,
    HashLog = 102,
    ChainLog = 103,
    SearchLog = 104,
    MinMatch = 105,
    TargetLength = 106,
    Strategy = 107,
    EnableLongDistanceMatching = 160,
    LdmHashLog = 161,
    LdmMinMatch = 162,
    LdmBucketSizeLog = 163,
    LdmHashRateLog = 164,
    ContentSizeFlag = 200,
    ChecksumFlag = 201,
    DictIdFlag = 202,
    NbWorkers = 400,
    JobSize = 401,
    OverlapLog = 402,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ZstdDParameter {
    WindowLogMax = 100,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ZstdEndDirective {
    Continue = 0,
    Flush = 1,
    End = 2,
}

#[repr(C)]
pub struct ZstdInBuffer {
    pub src: *const libc::c_void,
    pub size: libc::size_t,
    pub pos: libc::size_t,
}

#[repr(C)]
pub struct ZstdOutBuffer {
    pub dst: *mut libc::c_void,
    pub capacity: libc::size_t,
    pub pos: libc::size_t,
}

pub enum ZstdCCtxOpaque {}
pub enum ZstdDCtxOpaque {}
pub enum ZstdCDictOpaque {}
pub enum ZstdDDictOpaque {}

#[allow(dead_code)]
extern "C" {
    pub fn ZSTD_createCCtx() -> *mut ZstdCCtxOpaque;
    pub fn ZSTD_freeCCtx(cctx: *mut ZstdCCtxOpaque) -> libc::size_t;
    pub fn ZSTD_createDCtx() -> *mut ZstdDCtxOpaque;
    pub fn ZSTD_freeDCtx(dctx: *mut ZstdDCtxOpaque) -> libc::size_t;

    pub fn ZSTD_CCtx_setParameter(
        cctx: *mut ZstdCCtxOpaque,
        param: ZstdCParameter,
        value: libc::c_int,
    ) -> libc::size_t;
    pub fn ZSTD_DCtx_setParameter(
        dctx: *mut ZstdDCtxOpaque,
        param: ZstdDParameter,
        value: libc::c_int,
    ) -> libc::size_t;
    pub fn ZSTD_CCtx_reset(cctx: *mut ZstdCCtxOpaque, reset: libc::c_int) -> libc::size_t;
    pub fn ZSTD_DCtx_reset(dctx: *mut ZstdDCtxOpaque, reset: libc::c_int) -> libc::size_t;

    pub fn ZSTD_compressBound(src_size: libc::size_t) -> libc::size_t;
    pub fn ZSTD_isError(code: libc::size_t) -> libc::c_uint;
    pub fn ZSTD_getErrorName(code: libc::size_t) -> *const libc::c_char;

    pub fn ZSTD_compressCCtx(
        cctx: *mut ZstdCCtxOpaque,
        dst: *mut libc::c_void,
        dst_capacity: libc::size_t,
        src: *const libc::c_void,
        src_size: libc::size_t,
        compression_level: libc::c_int,
    ) -> libc::size_t;

    pub fn ZSTD_decompressDCtx(
        dctx: *mut ZstdDCtxOpaque,
        dst: *mut libc::c_void,
        dst_capacity: libc::size_t,
        src: *const libc::c_void,
        src_size: libc::size_t,
    ) -> libc::size_t;

    pub fn ZSTD_compress2(
        cctx: *mut ZstdCCtxOpaque,
        dst: *mut libc::c_void,
        dst_capacity: libc::size_t,
        src: *const libc::c_void,
        src_size: libc::size_t,
    ) -> libc::size_t;

    pub fn ZSTD_compressStream2(
        cctx: *mut ZstdCCtxOpaque,
        output: *mut ZstdOutBuffer,
        input: *mut ZstdInBuffer,
        end_op: ZstdEndDirective,
    ) -> libc::size_t;

    pub fn ZSTD_decompressStream(
        dctx: *mut ZstdDCtxOpaque,
        output: *mut ZstdOutBuffer,
        input: *mut ZstdInBuffer,
    ) -> libc::size_t;

    pub fn ZSTD_compress(
        dst: *mut libc::c_void,
        dst_capacity: libc::size_t,
        src: *const libc::c_void,
        src_size: libc::size_t,
        compression_level: libc::c_int,
    ) -> libc::size_t;

    pub fn ZSTD_decompress(
        dst: *mut libc::c_void,
        dst_capacity: libc::size_t,
        src: *const libc::c_void,
        src_size: libc::size_t,
    ) -> libc::size_t;

    pub fn ZSTD_getFrameContentSize(
        src: *const libc::c_void,
        src_size: libc::size_t,
    ) -> libc::c_ulonglong;

    // MARK: - Dictionary FFI Bindings

    pub fn ZSTD_createCDict(
        dict_buffer: *const libc::c_void,
        dict_size: libc::size_t,
        compression_level: libc::c_int,
    ) -> *mut ZstdCDictOpaque;
    pub fn ZSTD_freeCDict(cdict: *mut ZstdCDictOpaque) -> libc::size_t;
    pub fn ZSTD_sizeof_CDict(cdict: *const ZstdCDictOpaque) -> libc::size_t;
    pub fn ZSTD_getDictID_fromCDict(cdict: *const ZstdCDictOpaque) -> libc::c_uint;

    pub fn ZSTD_compress_usingCDict(
        cctx: *mut ZstdCCtxOpaque,
        dst: *mut libc::c_void,
        dst_capacity: libc::size_t,
        src: *const libc::c_void,
        src_size: libc::size_t,
        cdict: *const ZstdCDictOpaque,
    ) -> libc::size_t;

    pub fn ZSTD_createDDict(
        dict_buffer: *const libc::c_void,
        dict_size: libc::size_t,
    ) -> *mut ZstdDDictOpaque;
    pub fn ZSTD_freeDDict(ddict: *mut ZstdDDictOpaque) -> libc::size_t;
    pub fn ZSTD_sizeof_DDict(ddict: *const ZstdDDictOpaque) -> libc::size_t;
    pub fn ZSTD_getDictID_fromDDict(ddict: *const ZstdDDictOpaque) -> libc::c_uint;

    pub fn ZSTD_decompress_usingDDict(
        dctx: *mut ZstdDCtxOpaque,
        dst: *mut libc::c_void,
        dst_capacity: libc::size_t,
        src: *const libc::c_void,
        src_size: libc::size_t,
        ddict: *const ZstdDDictOpaque,
    ) -> libc::size_t;

    pub fn ZSTD_CCtx_refCDict(
        cctx: *mut ZstdCCtxOpaque,
        cdict: *const ZstdCDictOpaque,
    ) -> libc::size_t;
    pub fn ZSTD_DCtx_refDDict(
        dctx: *mut ZstdDCtxOpaque,
        ddict: *const ZstdDDictOpaque,
    ) -> libc::size_t;

    pub fn ZSTD_CCtx_loadDictionary(
        cctx: *mut ZstdCCtxOpaque,
        dict: *const libc::c_void,
        dict_size: libc::size_t,
    ) -> libc::size_t;
    pub fn ZSTD_DCtx_loadDictionary(
        dctx: *mut ZstdDCtxOpaque,
        dict: *const libc::c_void,
        dict_size: libc::size_t,
    ) -> libc::size_t;

    // MARK: - Dictionary Builder FFI Bindings

    pub fn ZDICT_trainFromBuffer(
        dict_buffer: *mut libc::c_void,
        dict_buffer_capacity: libc::size_t,
        samples_buffer: *const libc::c_void,
        samples_sizes: *const libc::size_t,
        nb_samples: libc::c_uint,
    ) -> libc::size_t;
    pub fn ZDICT_isError(code: libc::size_t) -> libc::c_uint;
    pub fn ZDICT_getErrorName(code: libc::size_t) -> *const libc::c_char;

    // MARK: - FSE (Finite State Entropy / tANS) FFI Bindings

    pub fn FSE_compressBound(size: libc::size_t) -> libc::size_t;
    pub fn FSE_isError(code: libc::size_t) -> libc::c_uint;
    pub fn FSE_getErrorName(code: libc::size_t) -> *const libc::c_char;
    pub fn FSE_optimalTableLog(
        max_table_log: libc::c_uint,
        src_size: libc::size_t,
        max_symbol_value: libc::c_uint,
    ) -> libc::c_uint;
    pub fn FSE_normalizeCount(
        normalized_counter: *mut i16,
        table_log: libc::c_uint,
        count: *const libc::c_uint,
        src_size: libc::size_t,
        max_symbol_value: libc::c_uint,
        use_low_prob_count: libc::c_uint,
    ) -> libc::size_t;
    pub fn FSE_writeNCount(
        buffer: *mut libc::c_void,
        buffer_size: libc::size_t,
        normalized_counter: *const i16,
        max_symbol_value: libc::c_uint,
        table_log: libc::c_uint,
    ) -> libc::size_t;
    pub fn FSE_readNCount(
        normalized_counter: *mut i16,
        max_sv_ptr: *mut libc::c_uint,
        table_log_ptr: *mut libc::c_uint,
        header_buffer: *const libc::c_void,
        hb_size: libc::size_t,
    ) -> libc::size_t;
    pub fn FSE_buildCTable_wksp(
        ct: *mut libc::c_uint,
        normalized_counter: *const i16,
        max_symbol_value: libc::c_uint,
        table_log: libc::c_uint,
        work_space: *mut libc::c_void,
        wksp_size: libc::size_t,
    ) -> libc::size_t;
    pub fn FSE_buildDTable_wksp(
        dt: *mut libc::c_uint,
        normalized_counter: *const i16,
        max_symbol_value: libc::c_uint,
        table_log: libc::c_uint,
        work_space: *mut libc::c_void,
        wksp_size: libc::size_t,
    ) -> libc::size_t;
    pub fn FSE_compress_usingCTable(
        dst: *mut libc::c_void,
        dst_capacity: libc::size_t,
        src: *const libc::c_void,
        src_size: libc::size_t,
        ct: *const libc::c_uint,
    ) -> libc::size_t;
    pub fn FSE_decompress_usingDTable(
        dst: *mut libc::c_void,
        max_dst_size: libc::size_t,
        c_src: *const libc::c_void,
        c_src_size: libc::size_t,
        dt: *const libc::c_uint,
    ) -> libc::size_t;

    pub fn HIST_count_wksp(
        count: *mut libc::c_uint,
        max_symbol_value_ptr: *mut libc::c_uint,
        src: *const libc::c_void,
        src_size: libc::size_t,
        work_space: *mut libc::c_void,
        wksp_size: libc::size_t,
    ) -> libc::size_t;

    // MARK: - Huff0 (4-Stream / 1-Stream Huffman) FFI Bindings

    pub fn HUF_compressBound(size: libc::size_t) -> libc::size_t;
    pub fn HUF_isError(code: libc::size_t) -> libc::c_uint;
    pub fn HUF_getErrorName(code: libc::size_t) -> *const libc::c_char;
    pub fn HUF_buildCTable_wksp(
        tree: *mut libc::c_void,
        count: *const libc::c_uint,
        max_symbol_value: libc::c_uint,
        max_nb_bits: libc::c_uint,
        work_space: *mut libc::c_void,
        wksp_size: libc::size_t,
    ) -> libc::size_t;
    pub fn HUF_writeCTable_wksp(
        dst: *mut libc::c_void,
        max_dst_size: libc::size_t,
        ctable: *const libc::c_void,
        max_symbol_value: libc::c_uint,
        huff_log: libc::c_uint,
        work_space: *mut libc::c_void,
        wksp_size: libc::size_t,
    ) -> libc::size_t;
    pub fn HUF_compress4X_usingCTable(
        dst: *mut libc::c_void,
        max_dst_size: libc::size_t,
        src: *const libc::c_void,
        src_size: libc::size_t,
        ctable: *const libc::c_void,
        flags: libc::c_int,
    ) -> libc::size_t;
    pub fn HUF_compress1X_usingCTable(
        dst: *mut libc::c_void,
        max_dst_size: libc::size_t,
        src: *const libc::c_void,
        src_size: libc::size_t,
        ctable: *const libc::c_void,
        flags: libc::c_int,
    ) -> libc::size_t;
    pub fn HUF_compress4X_repeat(
        dst: *mut libc::c_void,
        dst_size: libc::size_t,
        src: *const libc::c_void,
        src_size: libc::size_t,
        max_symbol_value: libc::c_uint,
        table_log: libc::c_uint,
        work_space: *mut libc::c_void,
        wksp_size: libc::size_t,
        huf_table: *mut libc::c_void,
        repeat: *mut libc::c_int,
        flags: libc::c_int,
    ) -> libc::size_t;
    pub fn HUF_compress1X_repeat(
        dst: *mut libc::c_void,
        dst_size: libc::size_t,
        src: *const libc::c_void,
        src_size: libc::size_t,
        max_symbol_value: libc::c_uint,
        table_log: libc::c_uint,
        work_space: *mut libc::c_void,
        wksp_size: libc::size_t,
        huf_table: *mut libc::c_void,
        repeat: *mut libc::c_int,
        flags: libc::c_int,
    ) -> libc::size_t;
    pub fn HUF_decompress4X_usingDTable(
        dst: *mut libc::c_void,
        max_dst_size: libc::size_t,
        c_src: *const libc::c_void,
        c_src_size: libc::size_t,
        dtable: *const libc::c_uint,
        flags: libc::c_int,
    ) -> libc::size_t;
    pub fn HUF_decompress1X_usingDTable(
        dst: *mut libc::c_void,
        max_dst_size: libc::size_t,
        c_src: *const libc::c_void,
        c_src_size: libc::size_t,
        dtable: *const libc::c_uint,
        flags: libc::c_int,
    ) -> libc::size_t;
    pub fn FSE_decompress_wksp_bmi2(
        dst: *mut libc::c_void,
        dst_capacity: libc::size_t,
        c_src: *const libc::c_void,
        c_src_size: libc::size_t,
        max_log: libc::c_uint,
        work_space: *mut libc::c_void,
        wksp_size: libc::size_t,
        bmi2: libc::c_int,
    ) -> libc::size_t;
    pub fn HUF_readDTableX1_wksp(
        dtable: *mut libc::c_uint,
        src: *const libc::c_void,
        src_size: libc::size_t,
        work_space: *mut libc::c_void,
        wksp_size: libc::size_t,
        flags: libc::c_int,
    ) -> libc::size_t;
    pub fn HUF_readDTableX2_wksp(
        dtable: *mut libc::c_uint,
        src: *const libc::c_void,
        src_size: libc::size_t,
        work_space: *mut libc::c_void,
        wksp_size: libc::size_t,
        flags: libc::c_int,
    ) -> libc::size_t;
}

/// Advanced configuration parameters for Zstandard compression and LDM pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZstdConfig {
    pub level: i32,
    pub nb_workers: u32,
    pub job_size_mb: u32,
    pub overlap_log: u32,
    pub window_log: u32,
    pub enable_ldm: bool,
    pub enable_checksum: bool,
    pub ldm_hash_log: u32,
    pub ldm_min_match: u32,
    pub ldm_bucket_size_log: u32,
    pub ldm_hash_rate_log: u32,
}

impl Default for ZstdConfig {
    fn default() -> Self {
        Self {
            level: 3,
            nb_workers: 0,
            job_size_mb: 0,
            overlap_log: 0,
            window_log: 0,
            enable_ldm: false,
            enable_checksum: true,
            ldm_hash_log: 0,
            ldm_min_match: 0,
            ldm_bucket_size_log: 0,
            ldm_hash_rate_log: 0,
        }
    }
}

impl ZstdConfig {
    /// Constructs an LDM configuration with default level 19 and custom windowLog (26 = 64MB .. 31 = 2GB).
    pub fn ldm(level: i32, window_log: u32) -> Self {
        Self {
            level: level.clamp(1, 22),
            enable_ldm: true,
            window_log: window_log.clamp(10, 31),
            enable_checksum: true,
            ..Default::default()
        }
    }

    /// Configures Long Distance Matching (LDM) with target window size in Megabytes.
    pub fn with_ldm_window_mb(mut self, window_mb: usize) -> Self {
        self.enable_ldm = true;
        // Compute window_log from megabytes: 64MB -> 26, 128MB -> 27, 256MB -> 28, 512MB -> 29, 1024MB -> 30, 2048MB -> 31
        let bytes = (window_mb.max(1) as u64).saturating_mul(1024 * 1024);
        let log = (64 - bytes.leading_zeros()).saturating_sub(1);
        self.window_log = log.clamp(10, 31);
        self
    }

    /// Sets explicit LDM matching parameters.
    pub fn with_ldm_tuning(
        mut self,
        ldm_hash_log: u32,
        ldm_min_match: u32,
        ldm_bucket_size_log: u32,
        ldm_hash_rate_log: u32,
    ) -> Self {
        self.enable_ldm = true;
        self.ldm_hash_log = ldm_hash_log;
        self.ldm_min_match = ldm_min_match;
        self.ldm_bucket_size_log = ldm_bucket_size_log;
        self.ldm_hash_rate_log = ldm_hash_rate_log;
        self
    }
}

