// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.
// PyO3 native Python C-extension binding module.

use pyo3::exceptions::{PyException, PyFileNotFoundError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::ffi::{CStr, CString};
use std::sync::Mutex;
use ttzip_glue::ffi::*;
use ttzip_glue::platform::CpuCapabilities;
use ttzip_glue::types::*;

pyo3::create_exception!(_ttzip, TTZipError, PyException);
pyo3::create_exception!(_ttzip, AuthenticationError, TTZipError);
pyo3::create_exception!(_ttzip, CorruptArchiveError, TTZipError);
pyo3::create_exception!(_ttzip, SecurityError, TTZipError);

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

/// Decompress raw in-memory buffer.
#[pyfunction]
#[pyo3(signature = (data, format="deflate"))]
fn decompress_buffer<'py>(py: Python<'py>, data: &[u8], format: &str) -> PyResult<Bound<'py, PyBytes>> {
    match format.to_lowercase().as_str() {
        "deflate" | "zip" => {
            let decompressed = miniz_oxide::inflate::decompress_to_vec(data)
                .map_err(|e| TTZipError::new_err(format!("Deflate decompression failed: {:?}", e)))?;
            Ok(PyBytes::new_bound(py, &decompressed))
        }
        "zstd" => {
            let mut dst = vec![0u8; data.len() * 4 + 4096];
            let mut out_len = 0usize;
            let decompressed_size = ttzip_rust_zstd_get_decompressed_size(data.as_ptr(), data.len());
            if decompressed_size > 0 && (decompressed_size as usize) > dst.len() {
                dst.resize(decompressed_size as usize, 0);
            }
            let status = ttzip_rust_zstd_decompress(data.as_ptr(), data.len(), dst.as_mut_ptr(), dst.len(), &mut out_len);
            if status != TTZipStatus::Ok {
                return Err(TTZipError::new_err(format!("Zstd decompression failed: code {}", status as i32)));
            }
            Ok(PyBytes::new_bound(py, &dst[..out_len]))
        }
        "lz4" => {
            if data.len() < 4 {
                return Err(TTZipError::new_err("Invalid LZ4 buffer: too short"));
            }
            let uncompressed_len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
            let mut dst = vec![0u8; uncompressed_len];
            let mut out_len = 0usize;
            let status = ttzip_rust_lz4_decompress(data[4..].as_ptr(), data.len() - 4, dst.as_mut_ptr(), dst.len(), &mut out_len);
            if status != TTZipStatus::Ok {
                return Err(TTZipError::new_err(format!("LZ4 decompression failed: code {}", status as i32)));
            }
            Ok(PyBytes::new_bound(py, &dst[..out_len]))
        }
        _ => Err(PyValueError::new_err(format!("Unsupported buffer format: {}", format))),
    }
}

/// Compress raw in-memory buffer.
#[pyfunction]
#[pyo3(signature = (data, format="deflate", level=6))]
fn compress_buffer<'py>(py: Python<'py>, data: &[u8], format: &str, level: i32) -> PyResult<Bound<'py, PyBytes>> {
    match format.to_lowercase().as_str() {
        "deflate" | "zip" => {
            let compressed = miniz_oxide::deflate::compress_to_vec(data, (level.clamp(0, 10)) as u8);
            Ok(PyBytes::new_bound(py, &compressed))
        }
        "zstd" => {
            let mut dst = vec![0u8; data.len() + 1024];
            let mut out_len = 0usize;
            let status = ttzip_rust_zstd_compress(data.as_ptr(), data.len(), dst.as_mut_ptr(), dst.len(), level, &mut out_len);
            if status != TTZipStatus::Ok {
                return Err(TTZipError::new_err(format!("Zstd compression failed: code {}", status as i32)));
            }
            Ok(PyBytes::new_bound(py, &dst[..out_len]))
        }
        "lz4" => {
            let mut dst = vec![0u8; data.len() + 1024 + 4];
            let uncompressed_len = (data.len() as u32).to_le_bytes();
            dst[0..4].copy_from_slice(&uncompressed_len);
            let mut out_len = 0usize;
            let status = ttzip_rust_lz4_compress(data.as_ptr(), data.len(), dst[4..].as_mut_ptr(), dst.len() - 4, &mut out_len);
            if status != TTZipStatus::Ok {
                return Err(TTZipError::new_err(format!("LZ4 compression failed: code {}", status as i32)));
            }
            Ok(PyBytes::new_bound(py, &dst[..(out_len + 4)]))
        }
        _ => Err(PyValueError::new_err(format!("Unsupported buffer format: {}", format))),
    }
}

/// Hardware SIMD accelerated CRC32 (>40 GB/s on Apple Silicon / AVX-512).
#[pyfunction]
#[pyo3(signature = (data, seed=0))]
fn crc32(data: &[u8], seed: u32) -> u32 {
    unsafe { ttzip_rust_crc32(seed, data.as_ptr(), data.len()) }
}

/// Hardware SIMD accelerated CRC64.
#[pyfunction]
#[pyo3(signature = (data, seed=0))]
fn crc64(data: &[u8], seed: u64) -> u64 {
    unsafe { ttzip_rust_crc64(seed, data.as_ptr(), data.len()) }
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
    use ttzip_glue::benchmark::corpus::BenchmarkCorpusType;
    use ttzip_glue::benchmark::runner::BenchmarkMatrixRunner;

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
    m.add_function(wrap_pyfunction!(crc32, m)?)?;
    m.add_function(wrap_pyfunction!(crc64, m)?)?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add_function(wrap_pyfunction!(is_hardware_accelerated, m)?)?;
    m.add_function(wrap_pyfunction!(benchmark_matrix, m)?)?;

    Ok(())
}
