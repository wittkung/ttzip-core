// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Safe RAII wrappers for Zstandard Digested Dictionaries (`CDict` / `DDict`),
//! Pre-trained Dictionary Builder (`ZDICT`), and High-Throughput Small File Manager.

use super::cctx::with_thread_local_zstd_cctx;
use super::dctx::with_thread_local_zstd_dctx;
use super::types::*;
use crate::types::TTZipStatus;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, LazyLock};

/// Standard shared pre-trained dictionary buffer size (112 KB = 114,688 bytes).
pub const ZSTD_STANDARD_DICTIONARY_SIZE_BYTES: usize = 112 * 1024;

// MARK: - Safe RAII CDict (Compression Dictionary)

/// Safe RAII wrapper for a pre-digested Zstandard Compression Dictionary (`ZSTD_CDict`).
///
/// `CDict` contains pre-computed parsing tables, match tables, and entropy headers.
/// It is completely thread-safe (`Send + Sync`) and can be shared across multiple threads
/// concurrently to eliminate dictionary initialization overhead.
pub struct CDict {
    handle: NonNull<ZstdCDictOpaque>,
    dict_id: u32,
    size: usize,
}

unsafe impl Send for CDict {}
unsafe impl Sync for CDict {}

impl CDict {
    /// Creates a digested compression dictionary from raw dictionary bytes and compression level.
    pub fn create(dict_bytes: &[u8], compression_level: i32) -> Result<Self, TTZipStatus> {
        if dict_bytes.is_empty() {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        let ptr = unsafe {
            ZSTD_createCDict(
                dict_bytes.as_ptr() as *const libc::c_void,
                dict_bytes.len(),
                compression_level as libc::c_int,
            )
        };
        let handle = NonNull::new(ptr).ok_or(TTZipStatus::ErrOutOfMemory)?;
        let dict_id = unsafe { ZSTD_getDictID_fromCDict(handle.as_ptr()) };
        let size = unsafe { ZSTD_sizeof_CDict(handle.as_ptr()) };

        Ok(Self {
            handle,
            dict_id,
            size,
        })
    }

    /// Returns the unique numeric ID of the dictionary (0 for raw content dictionaries).
    #[inline]
    pub fn dict_id(&self) -> u32 {
        self.dict_id
    }

    /// Returns memory consumption in bytes of the compiled CDict structures.
    #[inline]
    pub fn memory_size(&self) -> usize {
        self.size
    }

    /// Returns the raw pointer to the underlying `ZSTD_CDict`.
    #[inline]
    pub fn as_ptr(&self) -> *const ZstdCDictOpaque {
        self.handle.as_ptr()
    }
}

impl Drop for CDict {
    fn drop(&mut self) {
        unsafe {
            ZSTD_freeCDict(self.handle.as_ptr());
        }
    }
}

// MARK: - Safe RAII DDict (Decompression Dictionary)

/// Safe RAII wrapper for a pre-digested Zstandard Decompression Dictionary (`ZSTD_DDict`).
///
/// `DDict` contains pre-computed FSE and Huffman decoding tables.
/// It is completely thread-safe (`Send + Sync`) and can be shared across multiple threads
/// concurrently to eliminate decompression table reconstruction overhead.
pub struct DDict {
    handle: NonNull<ZstdDDictOpaque>,
    dict_id: u32,
    size: usize,
}

unsafe impl Send for DDict {}
unsafe impl Sync for DDict {}

impl DDict {
    /// Creates a digested decompression dictionary from raw dictionary bytes.
    pub fn create(dict_bytes: &[u8]) -> Result<Self, TTZipStatus> {
        if dict_bytes.is_empty() {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        let ptr = unsafe {
            ZSTD_createDDict(
                dict_bytes.as_ptr() as *const libc::c_void,
                dict_bytes.len(),
            )
        };
        let handle = NonNull::new(ptr).ok_or(TTZipStatus::ErrOutOfMemory)?;
        let dict_id = unsafe { ZSTD_getDictID_fromDDict(handle.as_ptr()) };
        let size = unsafe { ZSTD_sizeof_DDict(handle.as_ptr()) };

        Ok(Self {
            handle,
            dict_id,
            size,
        })
    }

    /// Returns the unique numeric ID of the dictionary (0 for raw content dictionaries).
    #[inline]
    pub fn dict_id(&self) -> u32 {
        self.dict_id
    }

    /// Returns memory consumption in bytes of the compiled DDict structures.
    #[inline]
    pub fn memory_size(&self) -> usize {
        self.size
    }

    /// Returns the raw pointer to the underlying `ZSTD_DDict`.
    #[inline]
    pub fn as_ptr(&self) -> *const ZstdDDictOpaque {
        self.handle.as_ptr()
    }
}

impl Drop for DDict {
    fn drop(&mut self) {
        unsafe {
            ZSTD_freeDDict(self.handle.as_ptr());
        }
    }
}

// MARK: - Pre-Trained Zstd Dictionary Handle

/// Container bundling raw dictionary bytes with pre-digested `CDict` and `DDict` tables.
#[derive(Clone)]
pub struct ZstdDictionary {
    name: String,
    dict_id: u32,
    raw_bytes: Arc<Vec<u8>>,
    cdict: Arc<CDict>,
    ddict: Arc<DDict>,
}

impl ZstdDictionary {
    /// Creates a `ZstdDictionary` from raw binary dictionary bytes at specified compression level.
    pub fn from_bytes(
        name: impl Into<String>,
        dict_bytes: Vec<u8>,
        level: i32,
    ) -> Result<Self, TTZipStatus> {
        let cdict = Arc::new(CDict::create(&dict_bytes, level)?);
        let ddict = Arc::new(DDict::create(&dict_bytes)?);
        let dict_id = cdict.dict_id();

        Ok(Self {
            name: name.into(),
            dict_id,
            raw_bytes: Arc::new(dict_bytes),
            cdict,
            ddict,
        })
    }

    /// Trains a custom dictionary from a representative corpus of small sample buffers.
    pub fn train(
        name: impl Into<String>,
        samples: &[&[u8]],
        target_dict_size: usize,
        level: i32,
    ) -> Result<Self, TTZipStatus> {
        let trained_bytes = zstd_train_dictionary(samples, target_dict_size, level)?;
        Self::from_bytes(name, trained_bytes, level)
    }

    /// Returns the descriptive label/name of the dictionary.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the dictionary ID.
    #[inline]
    pub fn dict_id(&self) -> u32 {
        self.dict_id
    }

    /// Returns the uncompiled dictionary bytes.
    #[inline]
    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw_bytes
    }

    /// Returns a reference to the compiled `CDict`.
    #[inline]
    pub fn cdict(&self) -> &Arc<CDict> {
        &self.cdict
    }

    /// Returns a reference to the compiled `DDict`.
    #[inline]
    pub fn ddict(&self) -> &Arc<DDict> {
        &self.ddict
    }

    /// Compresses a small payload using thread-local pooled CCtx and this dictionary's `CDict`.
    pub fn compress_small(&self, src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
        with_thread_local_zstd_cctx(|cctx| {
            cctx.compress_using_cdict_raw(src, dst, self.cdict.as_ptr())
        })
    }

    /// Decompresses a small payload using thread-local pooled DCtx and this dictionary's `DDict`.
    pub fn decompress_small(&self, src: &[u8], dst: &mut [u8]) -> Result<usize, TTZipStatus> {
        with_thread_local_zstd_dctx(|dctx| {
            dctx.decompress_using_ddict_raw(src, dst, self.ddict.as_ptr())
        })
    }
}

// MARK: - Dictionary Manager Registry

/// Thread-safe registry and cache for pre-trained Zstandard dictionaries.
pub struct ZstdDictionaryManager {
    by_name: RwLock<HashMap<String, Arc<ZstdDictionary>>>,
    by_id: RwLock<HashMap<u32, Arc<ZstdDictionary>>>,
    synthetic_id_gen: AtomicU32,
}

impl Default for ZstdDictionaryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ZstdDictionaryManager {
    /// Creates a new empty dictionary manager.
    pub fn new() -> Self {
        Self {
            by_name: RwLock::new(HashMap::new()),
            by_id: RwLock::new(HashMap::new()),
            synthetic_id_gen: AtomicU32::new(0x8000_0001),
        }
    }

    /// Returns the global singleton dictionary manager.
    pub fn global() -> &'static Self {
        static GLOBAL_MGR: LazyLock<ZstdDictionaryManager> = LazyLock::new(|| {
            let mgr = ZstdDictionaryManager::new();
            // Automatically register the 112KB standard corpus dictionary
            let _ = mgr.ensure_standard_112kb();
            mgr
        });
        &GLOBAL_MGR
    }

    /// Registers a dictionary in the manager cache.
    pub fn register(&self, dict: ZstdDictionary) -> Arc<ZstdDictionary> {
        let name = dict.name().to_string();
        let dict_id = if dict.dict_id() != 0 {
            dict.dict_id()
        } else {
            self.synthetic_id_gen.fetch_add(1, Ordering::Relaxed)
        };

        let arc = Arc::new(dict);
        self.by_name.write().insert(name, Arc::clone(&arc));
        self.by_id.write().insert(dict_id, Arc::clone(&arc));
        arc
    }

    /// Retrieves a dictionary by name.
    pub fn get_by_name(&self, name: &str) -> Option<Arc<ZstdDictionary>> {
        self.by_name.read().get(name).cloned()
    }

    /// Retrieves a dictionary by numeric dict ID.
    pub fn get_by_id(&self, dict_id: u32) -> Option<Arc<ZstdDictionary>> {
        self.by_id.read().get(&dict_id).cloned()
    }

    /// Returns or generates the 112KB standard shared dictionary.
    pub fn ensure_standard_112kb(&self) -> Arc<ZstdDictionary> {
        const STD_NAME: &str = "ttzip_std_112kb";
        if let Some(dict) = self.get_by_name(STD_NAME) {
            return dict;
        }

        // Build a synthetic high-entropy structured corpus for 112KB training
        let mut sample_corpus = Vec::new();
        let json_tmpl = br#"{"status":"success","code":200,"data":{"id":"item_#ID#","name":"TTZip File #ID#","path":"/usr/local/share/data/#ID#.json","tags":["zstd","compression","microkernel","archive"],"attributes":{"size":4096,"crc32":"0xABCD1234","compressed":true,"timestamp":"2026-08-29T00:00:00Z"}},"metadata":{"schema_version":"3.2.0","engine":"ttzip-rust"}}"#;
        let xml_tmpl = br#"<file id="item_#ID#" type="structured_document"><header><version>2.0</version><encoding>UTF-8</encoding><author>Witt Kung</author></header><content><node key="alpha">Value #ID#</node><node key="beta">Payload data</node></content></file>"#;
        let code_tmpl = br#"// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
pub struct Entry_#ID# { pub id: u64, pub name: String, pub flags: u32 }
impl Entry_#ID# { pub fn new() -> Self { Self { id: #ID#, name: String::from("item"), flags: 0 } } }"#;

        for i in 0..120 {
            let id_str = format!("{:04}", i);
            let s_json = String::from_utf8_lossy(json_tmpl).replace("#ID#", &id_str).into_bytes();
            let s_xml = String::from_utf8_lossy(xml_tmpl).replace("#ID#", &id_str).into_bytes();
            let s_code = String::from_utf8_lossy(code_tmpl).replace("#ID#", &id_str).into_bytes();
            sample_corpus.push(s_json);
            sample_corpus.push(s_xml);
            sample_corpus.push(s_code);
        }

        let sample_refs: Vec<&[u8]> = sample_corpus.iter().map(|v| v.as_slice()).collect();
        let dict = ZstdDictionary::train(STD_NAME, &sample_refs, ZSTD_STANDARD_DICTIONARY_SIZE_BYTES, 3)
            .unwrap_or_else(|_| {
                // Fallback: build a pseudo-dictionary if training buffer is smaller
                let mut fallback_buf = Vec::with_capacity(ZSTD_STANDARD_DICTIONARY_SIZE_BYTES);
                while fallback_buf.len() < ZSTD_STANDARD_DICTIONARY_SIZE_BYTES {
                    fallback_buf.extend_from_slice(json_tmpl);
                    fallback_buf.extend_from_slice(xml_tmpl);
                }
                fallback_buf.truncate(ZSTD_STANDARD_DICTIONARY_SIZE_BYTES);
                ZstdDictionary::from_bytes(STD_NAME, fallback_buf, 3).expect("fallback dictionary creation")
            });

        self.register(dict)
    }

    /// Compresses a small file (<4KB) with ultra-low latency using a specified named dictionary.
    pub fn compress_small_file(
        &self,
        dict_name: &str,
        src: &[u8],
        dst: &mut [u8],
    ) -> Result<usize, TTZipStatus> {
        let dict = self.get_by_name(dict_name).ok_or(TTZipStatus::ErrInvalidParam)?;
        dict.compress_small(src, dst)
    }

    /// Decompresses a small file (<4KB) with ultra-low latency using a specified dictionary ID.
    pub fn decompress_small_file(
        &self,
        dict_id: u32,
        src: &[u8],
        dst: &mut [u8],
    ) -> Result<usize, TTZipStatus> {
        let dict = self.get_by_id(dict_id).ok_or(TTZipStatus::ErrCorruptHeader)?;
        dict.decompress_small(src, dst)
    }
}

// MARK: - Dictionary Training & Zero-Copy Helpers

/// Trains a Zstandard dictionary from a list of sample buffers using libzstd `ZDICT_trainFromBuffer`.
pub fn zstd_train_dictionary(
    samples: &[&[u8]],
    target_dict_size: usize,
    _level: i32,
) -> Result<Vec<u8>, TTZipStatus> {
    if samples.is_empty() || target_dict_size == 0 {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let total_sample_bytes: usize = samples.iter().map(|s| s.len()).sum();
    let mut contiguous_samples = Vec::with_capacity(total_sample_bytes);
    let mut sample_sizes = Vec::with_capacity(samples.len());

    for s in samples {
        contiguous_samples.extend_from_slice(s);
        sample_sizes.push(s.len());
    }

    let mut dict_buf = vec![0u8; target_dict_size];

    let trained_size = unsafe {
        ZDICT_trainFromBuffer(
            dict_buf.as_mut_ptr() as *mut libc::c_void,
            dict_buf.len(),
            contiguous_samples.as_ptr() as *const libc::c_void,
            sample_sizes.as_ptr() as *const libc::size_t,
            samples.len() as libc::c_uint,
        )
    };

    if unsafe { ZDICT_isError(trained_size) } != 0 {
        Err(TTZipStatus::ErrCompressionFailed)
    } else {
        dict_buf.truncate(trained_size);
        Ok(dict_buf)
    }
}

/// Zero-copy compression of `src` into `dst` using an explicit `ZstdDictionary`.
pub fn zstd_compress_with_dict(
    src: &[u8],
    dst: &mut [u8],
    dict: &ZstdDictionary,
) -> Result<usize, TTZipStatus> {
    dict.compress_small(src, dst)
}

/// Zero-copy decompression of `src` into `dst` using an explicit `ZstdDictionary`.
pub fn zstd_decompress_with_dict(
    src: &[u8],
    dst: &mut [u8],
    dict: &ZstdDictionary,
) -> Result<usize, TTZipStatus> {
    dict.decompress_small(src, dst)
}
