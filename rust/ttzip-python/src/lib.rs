// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

// PyO3 native Python C-extension binding module with full Python Buffer Protocol support.

#![allow(clippy::useless_conversion)]

use pyo3::exceptions::{PyException, PyFileNotFoundError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyByteArray, PyBytes};
use std::ffi::{CStr, CString};
use std::io::Cursor;
use std::sync::Mutex;
use ttzip_engine::codecs::deflate::{
    deflate_compress, deflate_compress_bound, deflate_decompress,
};
use ttzip_engine::codecs::fast_blocks::{
    lz4_compress, lz4_compress_bound, lz4_decompress, lzfse_compress, lzfse_decompress,
    snappy_compress, snappy_decompress, snappy_max_compressed_length, snappy_uncompressed_length,
};
use ttzip_engine::codecs::zstd::{
    zstd_compress, zstd_compress_bound, zstd_decompress, zstd_decompress_stream_pipe,
    zstd_get_decompressed_size,
};
use ttzip_engine::ffi::*;
use ttzip_engine::platform::CpuCapabilities;
use ttzip_engine::types::*;

pyo3::create_exception!(_ttzip, TTZipError, PyException);
pyo3::create_exception!(_ttzip, AuthenticationError, TTZipError);
pyo3::create_exception!(_ttzip, CorruptArchiveError, TTZipError);
pyo3::create_exception!(_ttzip, SecurityError, TTZipError);

/// Safely extract immutable byte slice from any Python buffer object (bytes, bytearray, memoryview).
fn extract_input_bytes<'a, 'py>(_py: Python<'py>, obj: &'a Bound<'py, PyAny>) -> PyResult<&'a [u8]> {
    if let Ok(bytes) = obj.downcast::<PyBytes>() {
        return Ok(bytes.as_bytes());
    }
    if let Ok(byte_array) = obj.downcast::<PyByteArray>() {
        return Ok(unsafe { byte_array.as_bytes() });
    }
    obj.extract::<&'a [u8]>()
}

/// Safely extract mutable byte slice from a mutable Python buffer (bytearray).
fn extract_mut_bytes<'a, 'py>(_py: Python<'py>, obj: &'a Bound<'py, PyAny>) -> PyResult<&'a mut [u8]> {
    if let Ok(byte_array) = obj.downcast::<PyByteArray>() {
        return Ok(unsafe { byte_array.as_bytes_mut() });
    }
    Err(PyValueError::new_err(
        "Destination buffer must be a mutable bytearray (e.g. bytearray(size))",
    ))
}

#[pyclass(get_all, set_all, module = "ttzip._ttzip")]
#[derive(Clone, Debug)]
pub struct PyEntryMetadata {
    pub path: String,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub mtime_epoch_secs: i64,
    pub is_directory: bool,
    pub is_encrypted: bool,
}

#[pymethods]
impl PyEntryMetadata {
    fn __repr__(&self) -> String {
        format!(
            "EntryMetadata(path='{}', size={}, crc={:#010x}, is_dir={})",
            self.path, self.uncompressed_size, self.crc32, self.is_directory
        )
    }
}

#[pyclass(get_all, module = "ttzip._ttzip")]
#[derive(Clone, Debug)]
pub struct PyBenchmarkPointResult {
    pub algorithm: String,
    pub level: i32,
    pub display_name: String,
    pub original_size_bytes: usize,
    pub compressed_size_bytes: usize,
    pub space_savings_pct: f64,
    pub compress_throughput_mbs: f64,
    pub decompress_throughput_mbs: f64,
    pub is_pareto_optimal: bool,
}

#[pymethods]
impl PyBenchmarkPointResult {
    fn __repr__(&self) -> String {
        format!(
            "<Point {:<10} L{} Saved: {:.1}% Comp: {:.1} MB/s Decomp: {:.1} MB/s Optimal: {}>",
            self.algorithm, self.level, self.space_savings_pct,
            self.compress_throughput_mbs, self.decompress_throughput_mbs, self.is_pareto_optimal
        )
    }
}

#[pyclass(get_all, module = "ttzip._ttzip")]
#[derive(Clone, Debug)]
pub struct PyBenchmarkMatrixReport {
    pub total_points_evaluated: usize,
    pub pareto_optimal_count: usize,
    pub peak_compress_throughput_mbs: f64,
    pub peak_decompress_throughput_mbs: f64,
    pub max_space_savings_pct: f64,
    pub points: Vec<PyBenchmarkPointResult>,
    pub passed_gate: bool,
}

#[pymethods]
impl PyBenchmarkMatrixReport {
    fn __repr__(&self) -> String {
        format!(
            "<BenchmarkMatrixReport: {} points, peak comp: {:.1} MB/s, peak decomp: {:.1} MB/s, pass={}>",
            self.total_points_evaluated, self.peak_compress_throughput_mbs,
            self.peak_decompress_throughput_mbs, self.passed_gate
        )
    }
}

fn map_status_to_pyerr(status: TTZipStatus, msg: &str) -> PyErr {
    match status {
        TTZipStatus::Ok | TTZipStatus::Eof => PyValueError::new_err(msg.to_string()),
        TTZipStatus::ErrFileNotFound => PyFileNotFoundError::new_err(msg.to_string()),
        TTZipStatus::ErrInvalidPassword => AuthenticationError::new_err(msg.to_string()),
        TTZipStatus::ErrSecurityViolation => SecurityError::new_err(msg.to_string()),
        TTZipStatus::ErrCorruptHeader => CorruptArchiveError::new_err(msg.to_string()),
        _ => TTZipError::new_err(format!("TTZip Error (code {}): {}", status as i32, msg)),
    }
}

fn parse_archive_format(fmt: &str) -> TTZipArchiveFormat {
    match fmt.to_lowercase().as_str() {
        "auto" => TTZipArchiveFormat::Auto,
        "zip" => TTZipArchiveFormat::Zip,
        "7z" | "sevenz" => TTZipArchiveFormat::SevenZip,
        "tar" => TTZipArchiveFormat::Tar,
        "tgz" | "tar.gz" => TTZipArchiveFormat::TarGz,
        "tbz2" | "tar.bz2" => TTZipArchiveFormat::TarBz2,
        "txz" | "tar.xz" => TTZipArchiveFormat::TarXz,
        "tar.zst" | "tar.zstd" => TTZipArchiveFormat::TarZstd,
        "dmg" => TTZipArchiveFormat::Dmg,
        "lzfse" => TTZipArchiveFormat::Lzfse,
        "snappy" | "sz" => TTZipArchiveFormat::Snappy,
        _ => TTZipArchiveFormat::Auto,
    }
}

/// Compress files or directories into an archive with GIL released.
#[pyfunction]
#[pyo3(signature = (sources, destination, format="auto", level=6, password=None, threads=0))]
fn compress(
    py: Python<'_>,
    sources: Vec<String>,
    destination: String,
    format: &str,
    level: i32,
    password: Option<String>,
    threads: u32,
) -> PyResult<()> {
    let fmt = parse_archive_format(format);
    let opt_level = match level {
        0 => TTZipCompressionLevel::Store,
        1..=3 => TTZipCompressionLevel::Fastest,
        4..=6 => TTZipCompressionLevel::Normal,
        7..=9 => TTZipCompressionLevel::Maximum,
        _ => TTZipCompressionLevel::Ultra,
    };

    let (status, msg) = py.allow_threads(move || {
        let c_sources: Vec<CString> = sources
            .iter()
            .map(|s| CString::new(s.as_str()).unwrap_or_default())
            .collect();
        let c_source_ptrs: Vec<*const libc::c_char> = c_sources.iter().map(|s| s.as_ptr()).collect();
        let c_dest = match CString::new(destination.as_str()) {
            Ok(c) => c,
            Err(e) => return (TTZipStatus::ErrInvalidParam, format!("Invalid destination: {}", e)),
        };
        let c_pwd = password.as_deref().map(|p| CString::new(p).unwrap_or_default());

        let options = TTZipCreateOptions {
            struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
            abi_version: TTZIP_ABI_VERSION_2,
            format: fmt,
            level: opt_level,
            encryption: if c_pwd.is_some() {
                TTZipEncryptionMethod::Aes256
            } else {
                TTZipEncryptionMethod::None
            },
            password: c_pwd.as_ref().map_or(std::ptr::null(), |p| p.as_ptr()),
            thread_budget: threads,
            solid_block_size_mb: 64,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        let res = unsafe {
            ttzip_rust_create_archive(
                c_source_ptrs.as_ptr(),
                c_source_ptrs.len(),
                c_dest.as_ptr(),
                &options,
            )
        };

        let err_msg = if res != TTZipStatus::Ok {
            res.as_str().to_string()
        } else {
            String::new()
        };

        (res, err_msg)
    });

    if status != TTZipStatus::Ok {
        return Err(map_status_to_pyerr(status, &msg));
    }

    Ok(())
}

/// Extract an archive safely to a destination directory with GIL released.
#[pyfunction]
#[pyo3(signature = (archive, destination, password=None, threads=0))]
fn extract(
    py: Python<'_>,
    archive: String,
    destination: String,
    password: Option<String>,
    threads: u32,
) -> PyResult<()> {
    let (status, msg) = py.allow_threads(move || {
        let c_archive = match CString::new(archive.as_str()) {
            Ok(c) => c,
            Err(e) => return (TTZipStatus::ErrInvalidParam, format!("Invalid archive path: {}", e)),
        };
        let c_dest = match CString::new(destination.as_str()) {
            Ok(c) => c,
            Err(e) => return (TTZipStatus::ErrInvalidParam, format!("Invalid destination path: {}", e)),
        };
        let c_pwd = password.as_deref().map(|p| CString::new(p).unwrap_or_default());

        let options = TTZipExtractOptions {
            struct_size: std::mem::size_of::<TTZipExtractOptions>() as u32,
            abi_version: TTZIP_ABI_VERSION_2,
            destination_path: c_dest.as_ptr(),
            password: c_pwd.as_ref().map_or(std::ptr::null(), |p| p.as_ptr()),
            thread_budget: threads,
            overwrite_existing: true,
            preserve_permissions: true,
            dry_run: false,
            progress_callback: None,
            user_data: std::ptr::null_mut(),
        };

        let res = unsafe {
            ttzip_rust_extract_archive(c_archive.as_ptr(), c_dest.as_ptr(), &options)
        };

        let err_msg = if res != TTZipStatus::Ok {
            res.as_str().to_string()
        } else {
            String::new()
        };

        (res, err_msg)
    });

    if status != TTZipStatus::Ok {
        return Err(map_status_to_pyerr(status, &msg));
    }

    Ok(())
}

/// Inspect archive entry metadata without disk extraction.
#[pyfunction]
#[pyo3(signature = (archive, password=None))]
fn inspect(py: Python<'_>, archive: String, password: Option<String>) -> PyResult<Vec<PyEntryMetadata>> {
    let (status, msg, entries) = py.allow_threads(move || {
        let c_archive = match CString::new(archive.as_str()) {
            Ok(c) => c,
            Err(e) => return (TTZipStatus::ErrInvalidParam, format!("Invalid archive path: {}", e), Vec::new()),
        };
        let c_pwd = password.as_deref().map(|p| CString::new(p).unwrap_or_default());
        let collected: Mutex<Vec<PyEntryMetadata>> = Mutex::new(Vec::new());

        extern "C" fn inspect_callback(entry: *const TTZipEntryMetadata, user_data: *mut libc::c_void) -> bool {
            if entry.is_null() || user_data.is_null() {
                return false;
            }
            unsafe {
                let meta = &*entry;
                let path_str = if meta.path.is_null() {
                    String::new()
                } else {
                    CStr::from_ptr(meta.path).to_string_lossy().into_owned()
                };
                let item = PyEntryMetadata {
                    path: path_str,
                    uncompressed_size: meta.uncompressed_size,
                    compressed_size: meta.compressed_size,
                    crc32: meta.crc32,
                    mtime_epoch_secs: meta.mtime_epoch_secs,
                    is_directory: meta.is_directory,
                    is_encrypted: meta.is_encrypted,
                };
                let list = &*(user_data as *const Mutex<Vec<PyEntryMetadata>>);
                if let Ok(mut guard) = list.lock() {
                    guard.push(item);
                }
            }
            true
        }

        let res = unsafe {
            ttzip_rust_inspect_archive(
                c_archive.as_ptr(),
                c_pwd.as_ref().map_or(std::ptr::null(), |p| p.as_ptr()),
                true,
                Some(inspect_callback),
                &collected as *const _ as *mut libc::c_void,
            )
        };

        let err_msg = if res != TTZipStatus::Ok {
            res.as_str().to_string()
        } else {
            String::new()
        };

        let items = collected.into_inner().unwrap_or_default();
        (res, err_msg, items)
    });

    if status != TTZipStatus::Ok {
        return Err(map_status_to_pyerr(status, &msg));
    }

    Ok(entries)
}

/// Decompress raw in-memory buffer with GIL released and Zstandard streaming fallback.
#[pyfunction]
#[pyo3(signature = (data, format="deflate"))]
fn decompress_buffer<'py>(py: Python<'py>, data: &Bound<'py, PyAny>, format: &str) -> PyResult<Bound<'py, PyBytes>> {
    let input_bytes = extract_input_bytes(py, data)?;
    let fmt = format.to_lowercase();

    let decompressed: Vec<u8> = py.allow_threads(|| -> Result<Vec<u8>, String> {
        match fmt.as_str() {
            "deflate" | "zip" => {
                let mut dst = vec![0u8; (input_bytes.len() * 4 + 4096).max(65536)];
                loop {
                    match deflate_decompress(input_bytes, &mut dst) {
                        Ok(written) => {
                            dst.truncate(written);
                            break Ok(dst);
                        }
                        Err(TTZipStatus::ErrExtractionFailed) if dst.len() < 2 * 1024 * 1024 * 1024 => {
                            dst.resize(dst.len() * 2, 0u8);
                        }
                        Err(st) => break Err(format!("Deflate decompression failed: code {}", st as i32)),
                    }
                }
            }
            "zstd" => {
                let content_size = zstd_get_decompressed_size(input_bytes).unwrap_or(0);
                if content_size > 0 && content_size <= 2 * 1024 * 1024 * 1024 {
                    let mut dst = vec![0u8; content_size as usize];
                    if let Ok(written) = zstd_decompress(input_bytes, &mut dst) {
                        dst.truncate(written);
                        return Ok(dst);
                    }
                }

                // Streaming fallback decoding for frames without content size or dynamic expansion
                let mut cursor = Cursor::new(input_bytes);
                let mut out_buf = Vec::with_capacity(input_bytes.len() * 4 + 4096);
                match zstd_decompress_stream_pipe(&mut cursor, &mut out_buf, None) {
                    Ok(_) => Ok(out_buf),
                    Err(st) => Err(format!("Zstandard decompression failed: code {}", st as i32)),
                }
            }
            "lz4" => {
                if input_bytes.len() < 4 {
                    return Err("Invalid LZ4 buffer: buffer too short".to_string());
                }
                let uncompressed_len = u32::from_le_bytes([input_bytes[0], input_bytes[1], input_bytes[2], input_bytes[3]]) as usize;
                if uncompressed_len > 0 && uncompressed_len <= 1024 * 1024 * 1024 {
                    let mut dst = vec![0u8; uncompressed_len];
                    match lz4_decompress(&input_bytes[4..], &mut dst) {
                        Ok(written) => {
                            dst.truncate(written);
                            Ok(dst)
                        }
                        Err(st) => Err(format!("LZ4 decompression failed: code {}", st as i32)),
                    }
                } else {
                    // Fallback to raw block
                    let mut dst = vec![0u8; input_bytes.len() * 8 + 4096];
                    match lz4_decompress(input_bytes, &mut dst) {
                        Ok(written) => {
                            dst.truncate(written);
                            Ok(dst)
                        }
                        Err(st) => Err(format!("LZ4 block decompression failed: code {}", st as i32)),
                    }
                }
            }
            "snappy" | "sz" => {
                let uncomp_len = snappy_uncompressed_length(input_bytes)
                    .map_err(|st| format!("Snappy length parse error: code {}", st as i32))?;
                let mut dst = vec![0u8; uncomp_len];
                let written = snappy_decompress(input_bytes, &mut dst)
                    .map_err(|st| format!("Snappy decompression error: code {}", st as i32))?;
                dst.truncate(written);
                Ok(dst)
            }
            "lzfse" => {
                let mut dst = vec![0u8; input_bytes.len() * 8 + 65536];
                let written = lzfse_decompress(input_bytes, &mut dst)
                    .map_err(|st| format!("LZFSE decompression error: code {}", st as i32))?;
                dst.truncate(written);
                Ok(dst)
            }
            _ => Err(format!("Unsupported decompression format: {}", fmt)),
        }
    }).map_err(TTZipError::new_err)?;

    Ok(PyBytes::new_bound(py, &decompressed))
}

/// Compress raw in-memory buffer with GIL released.
#[pyfunction]
#[pyo3(signature = (data, format="deflate", level=6))]
fn compress_buffer<'py>(py: Python<'py>, data: &Bound<'py, PyAny>, format: &str, level: i32) -> PyResult<Bound<'py, PyBytes>> {
    let input_bytes = extract_input_bytes(py, data)?;
    let fmt = format.to_lowercase();

    let compressed: Vec<u8> = py.allow_threads(|| -> Result<Vec<u8>, String> {
        match fmt.as_str() {
            "deflate" | "zip" => {
                let cl = level.clamp(0, 12);
                let bound = deflate_compress_bound(input_bytes.len(), cl);
                let mut dst = vec![0u8; bound];
                match deflate_compress(input_bytes, &mut dst, cl) {
                    Ok(written) => {
                        dst.truncate(written);
                        Ok(dst)
                    }
                    Err(st) => Err(format!("Deflate compression failed: code {}", st as i32)),
                }
            }
            "zstd" => {
                let bound = zstd_compress_bound(input_bytes.len()) + 128;
                let mut dst = vec![0u8; bound];
                match zstd_compress(input_bytes, &mut dst, level) {
                    Ok(written) => {
                        dst.truncate(written);
                        Ok(dst)
                    }
                    Err(st) => Err(format!("Zstandard compression failed: code {}", st as i32)),
                }
            }
            "lz4" => {
                let bound = lz4_compress_bound(input_bytes.len()) + 4;
                let mut dst = vec![0u8; bound];
                let uncompressed_len = (input_bytes.len() as u32).to_le_bytes();
                dst[0..4].copy_from_slice(&uncompressed_len);
                match lz4_compress(input_bytes, &mut dst[4..]) {
                    Ok(written) => {
                        dst.truncate(written + 4);
                        Ok(dst)
                    }
                    Err(st) => Err(format!("LZ4 compression failed: code {}", st as i32)),
                }
            }
            "snappy" | "sz" => {
                let bound = snappy_max_compressed_length(input_bytes.len());
                let mut dst = vec![0u8; bound];
                match snappy_compress(input_bytes, &mut dst) {
                    Ok(written) => {
                        dst.truncate(written);
                        Ok(dst)
                    }
                    Err(st) => Err(format!("Snappy compression failed: code {}", st as i32)),
                }
            }
            "lzfse" => {
                let mut dst = vec![0u8; input_bytes.len() + 1024];
                match lzfse_compress(input_bytes, &mut dst) {
                    Ok(written) => {
                        dst.truncate(written);
                        Ok(dst)
                    }
                    Err(st) => Err(format!("LZFSE compression failed: code {}", st as i32)),
                }
            }
            _ => Err(format!("Unsupported compression format: {}", fmt)),
        }
    }).map_err(TTZipError::new_err)?;

    Ok(PyBytes::new_bound(py, &compressed))
}

/// Zero-copy decompression directly into a pre-allocated mutable buffer with GIL released.
#[pyfunction]
#[pyo3(signature = (data, dst_buffer, format="deflate"))]
fn decompress_into(py: Python<'_>, data: &Bound<'_, PyAny>, dst_buffer: &Bound<'_, PyAny>, format: &str) -> PyResult<usize> {
    let input_bytes = extract_input_bytes(py, data)?;
    let dst_slice = extract_mut_bytes(py, dst_buffer)?;
    let dst_ptr = dst_slice.as_mut_ptr() as usize;
    let dst_len = dst_slice.len();
    let fmt = format.to_lowercase();

    let written = py.allow_threads(move || -> Result<usize, String> {
        let out_slice = unsafe { std::slice::from_raw_parts_mut(dst_ptr as *mut u8, dst_len) };
        match fmt.as_str() {
            "deflate" | "zip" => {
                deflate_decompress(input_bytes, out_slice)
                    .map_err(|st| format!("Deflate direct decompression failed: code {}", st as i32))
            }
            "zstd" => {
                zstd_decompress(input_bytes, out_slice)
                    .map_err(|st| format!("Zstandard direct decompression failed: code {}", st as i32))
            }
            "lz4" => {
                let src_data = if input_bytes.len() >= 4 { &input_bytes[4..] } else { input_bytes };
                lz4_decompress(src_data, out_slice)
                    .map_err(|st| format!("LZ4 direct decompression failed: code {}", st as i32))
            }
            "snappy" | "sz" => {
                snappy_decompress(input_bytes, out_slice)
                    .map_err(|st| format!("Snappy direct decompression failed: code {}", st as i32))
            }
            "lzfse" => {
                lzfse_decompress(input_bytes, out_slice)
                    .map_err(|st| format!("LZFSE direct decompression failed: code {}", st as i32))
            }
            _ => Err(format!("Unsupported format: {}", fmt)),
        }
    }).map_err(TTZipError::new_err)?;

    Ok(written)
}

/// Hardware SIMD accelerated CRC32 (>40 GB/s on Apple Silicon / AVX-512).
#[pyfunction]
#[pyo3(signature = (data, seed=0))]
fn crc32(py: Python<'_>, data: &Bound<'_, PyAny>, seed: u32) -> PyResult<u32> {
    let bytes = extract_input_bytes(py, data)?;
    Ok(unsafe { ttzip_rust_crc32(seed, bytes.as_ptr(), bytes.len()) })
}

/// Hardware SIMD accelerated CRC64.
#[pyfunction]
#[pyo3(signature = (data, seed=0))]
fn crc64(py: Python<'_>, data: &Bound<'_, PyAny>, seed: u64) -> PyResult<u64> {
    let bytes = extract_input_bytes(py, data)?;
    Ok(unsafe { ttzip_rust_crc64(seed, bytes.as_ptr(), bytes.len()) })
}

/// Return engine version string.
#[pyfunction]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Check if hardware acceleration is active.
#[pyfunction]
fn is_hardware_accelerated() -> bool {
    let caps = CpuCapabilities::get();
    caps.has_hardware_crc32 || caps.has_arm_neon || caps.has_avx2
}

/// Run full multi-algorithm matrix benchmark.
#[pyfunction]
#[pyo3(signature = (corpus_type="synthetic_json", corpus_size=65536, iterations=1))]
fn benchmark_matrix(
    py: Python<'_>,
    corpus_type: &str,
    corpus_size: usize,
    iterations: usize,
) -> PyResult<PyBenchmarkMatrixReport> {
    use ttzip_engine::benchmark::corpus::BenchmarkCorpusType;
    use ttzip_engine::benchmark::runner::BenchmarkMatrixRunner;

    let c_type = match corpus_type.to_lowercase().as_str() {
        "calgary" | "text" => BenchmarkCorpusType::Calgary,
        "xml" | "json" => BenchmarkCorpusType::Xml,
        "random" => BenchmarkCorpusType::Random,
        "binary" => BenchmarkCorpusType::Binary,
        _ => BenchmarkCorpusType::Silesia,
    };

    let res = py.allow_threads(move || {
        BenchmarkMatrixRunner::run_matrix(c_type, corpus_size, iterations)
    });

    match res {
        Ok(report) => {
            let pts = report
                .points
                .into_iter()
                .map(|p| PyBenchmarkPointResult {
                    algorithm: p.algorithm,
                    level: p.level,
                    display_name: p.display_name,
                    original_size_bytes: p.original_size_bytes,
                    compressed_size_bytes: p.compressed_size_bytes,
                    space_savings_pct: p.space_savings_pct,
                    compress_throughput_mbs: p.compress_throughput_mbs,
                    decompress_throughput_mbs: p.decompress_throughput_mbs,
                    is_pareto_optimal: p.is_pareto_optimal,
                })
                .collect();

            Ok(PyBenchmarkMatrixReport {
                total_points_evaluated: report.total_points_evaluated,
                pareto_optimal_count: report.pareto_optimal_count,
                peak_compress_throughput_mbs: report.peak_compress_throughput_mbs,
                peak_decompress_throughput_mbs: report.peak_decompress_throughput_mbs,
                max_space_savings_pct: report.max_space_savings_pct,
                points: pts,
                passed_gate: report.passed_gate,
            })
        }
        Err(status) => Err(TTZipError::new_err(format!("Benchmark failed: code {}", status as i32))),
    }
}

/// Native Python C-Extension module declaration for `ttzip._ttzip`.
#[pymodule]
fn _ttzip(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyEntryMetadata>()?;
    m.add_class::<PyBenchmarkPointResult>()?;
    m.add_class::<PyBenchmarkMatrixReport>()?;
    m.add("TTZipError", m.py().get_type_bound::<TTZipError>())?;
    m.add("AuthenticationError", m.py().get_type_bound::<AuthenticationError>())?;
    m.add("CorruptArchiveError", m.py().get_type_bound::<CorruptArchiveError>())?;
    m.add("SecurityError", m.py().get_type_bound::<SecurityError>())?;

    m.add_function(wrap_pyfunction!(compress, m)?)?;
    m.add_function(wrap_pyfunction!(extract, m)?)?;
    m.add_function(wrap_pyfunction!(inspect, m)?)?;
    m.add_function(wrap_pyfunction!(compress_buffer, m)?)?;
    m.add_function(wrap_pyfunction!(decompress_buffer, m)?)?;
    m.add_function(wrap_pyfunction!(decompress_into, m)?)?;
    m.add_function(wrap_pyfunction!(crc32, m)?)?;
    m.add_function(wrap_pyfunction!(crc64, m)?)?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add_function(wrap_pyfunction!(is_hardware_accelerated, m)?)?;
    m.add_function(wrap_pyfunction!(benchmark_matrix, m)?)?;

    Ok(())
}
