// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! 7-Zip Solid Archive Creation and Header Serialization Engine.
//!
//! Features multi-threaded Fast-LZMA2 solid compression, Store mode, UTF-16LE metadata encoding,
//! and accurate 7z SignatureHeader CRC calculations.

use crate::archive::unified::create_transaction::CreateTransactionContext;
use crate::codecs::lzma2::{Fl2CStream, Fl2InBuffer, Fl2OutBuffer};
use crate::crypto::aes256::aes256_cbc_encrypt;
use crate::crypto::crc32::crc32_fast;
use crate::crypto::sha256::sha256_7z_kdf;
pub use crate::sevenz::finish_contract::{
    create_7z_archive_explicit_split_with_header_encryption_in_txn, create_7z_archive_split_in_txn,
    create_7z_archive_split_with_header_encryption_in_txn,
};
use crate::sevenz::finish_contract::finalize_7z_header_and_signature;
pub use crate::sevenz::format::SevenZAesWriterConfig;
use crate::sevenz::format::*;
use crate::types::{TTZipCompressionLevel, TTZipCreateOptions, TTZipStatus};
use crate::zip::writer::{ZipCreateReport, ZipInputItem};
use std::ffi::CString;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const STREAM_CHUNK_SIZE: usize = 1024 * 1024;
const AES_STREAM_BUF_SIZE: usize = 64 * 1024;

/// Metadata-only descriptor for a disk-backed 7z input entry.
#[derive(Debug, Clone)]
pub(crate) struct SevenZDiskEntry {
    pub(crate) disk_path: Option<PathBuf>,
    pub(crate) symlink_target: Option<Vec<u8>>,
    pub(crate) rel_path: String,
    pub(crate) is_directory: bool,
    pub(crate) is_symlink: bool,
    pub(crate) posix_mode: u32,
    pub(crate) mtime: std::time::SystemTime,
    pub(crate) size: u64,
    pub(crate) is_empty_stream: bool,
}

/// Constructs 7z Metadata Header bytes for a solid block of items, supporting optional AES-256 coder.
pub fn build_7z_metadata_header_full(
    items: &[ZipInputItem],
    stream_sizes: &[u64],
    stream_crcs: &[u32],
    compressed_len: u64,
    uncompressed_len: u64,
    method_id: u64,
    coder_props: &[u8],
    aes_config: Option<&SevenZAesWriterConfig>,
) -> Vec<u8> {
    let refs: Vec<HeaderEntryRef<'_>> = items
        .iter()
        .map(|it| {
            let is_symlink = (it.mode & 0o170000) == 0o120000;
            let mtime = std::time::UNIX_EPOCH + Duration::from_secs(it.mtime_epoch_secs as u64);
            HeaderEntryRef {
                rel_path: &it.rel_path,
                is_directory: it.is_directory,
                is_empty_stream: it.is_directory || it.data.is_empty(),
                is_symlink,
                posix_mode: it.mode,
                mtime,
            }
        })
        .collect();
    build_7z_metadata_header_from_refs(
        &refs, stream_sizes, stream_crcs, compressed_len, uncompressed_len, method_id, coder_props, aes_config,
    )
}

/// Constructs 7z Metadata Header bytes for a solid block of items without encryption.
pub fn build_7z_metadata_header(
    items: &[ZipInputItem],
    stream_sizes: &[u64],
    stream_crcs: &[u32],
    compressed_len: u64,
    uncompressed_len: u64,
    method_id: u64,
    coder_props: &[u8],
) -> Vec<u8> {
    build_7z_metadata_header_full(
        items, stream_sizes, stream_crcs, compressed_len, uncompressed_len, method_id, coder_props, None,
    )
}

pub(crate) fn init_fl2_stream(level: i32, threads: u32) -> Result<Fl2CStream, TTZipStatus> {
    let mut cstream = if threads > 1 {
        Fl2CStream::new_mt(threads)?
    } else {
        Fl2CStream::new()?
    };
    cstream.set_parameter(crate::codecs::lzma2::Fl2CParameter::CompressionLevel, level as usize)?;
    cstream.set_parameter(crate::codecs::lzma2::Fl2CParameter::OmitProperties, 1)?;
    cstream.init(0)?;
    Ok(cstream)
}

pub(crate) fn feed_fl2_slice<F>(
    cstream: &mut Fl2CStream,
    slice: &[u8],
    out_chunk: &mut [u8],
    mut on_out: F,
) -> Result<(), TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    let mut in_offset = 0;
    while in_offset < slice.len() {
        let rem = &slice[in_offset..];
        let mut in_buf = Fl2InBuffer {
            src: rem.as_ptr() as *const libc::c_void,
            size: rem.len(),
            pos: 0,
        };
        while in_buf.pos < in_buf.size {
            let mut out_buf = Fl2OutBuffer {
                dst: out_chunk.as_mut_ptr() as *mut libc::c_void,
                size: out_chunk.len(),
                pos: 0,
            };
            let res = cstream.compress_chunk(&mut in_buf, &mut out_buf)?;
            if out_buf.pos > 0 {
                on_out(&out_chunk[..out_buf.pos])?;
            }
            if res == 0 && in_buf.pos == 0 {
                break;
            }
        }
        in_offset += in_buf.pos;
    }
    Ok(())
}

pub(crate) fn finish_fl2_stream<F>(
    cstream: &mut Fl2CStream,
    out_chunk: &mut [u8],
    mut on_out: F,
) -> Result<u8, TTZipStatus>
where
    F: FnMut(&[u8]) -> Result<(), TTZipStatus>,
{
    loop {
        let mut out_buf = Fl2OutBuffer {
            dst: out_chunk.as_mut_ptr() as *mut libc::c_void,
            size: out_chunk.len(),
            pos: 0,
        };
        let remaining = cstream.end_stream(&mut out_buf)?;
        if out_buf.pos > 0 {
            on_out(&out_chunk[..out_buf.pos])?;
        }
        if remaining == 0 {
            break;
        }
    }
    Ok(cstream.dict_property())
}

/// Streaming AES-256-CBC state for bounded-memory 7z payload output.
struct Aes256StreamState {
    key: [u8; 32],
    salt: [u8; 16],
    initial_iv: [u8; 16],
    current_iv: [u8; 16],
    num_cycles_power: u32,
    carry: [u8; 16],
    carry_len: usize,
    enc_buf: Vec<u8>,
}

/// Bounded-memory payload sink that streams raw or AES-256-CBC encrypted bytes to any `Write` target.
pub(crate) struct SevenZPayloadSink<'a, W: Write> {
    writer: &'a mut W,
    raw_comp_len: u64,
    compressed_len: u64,
    aes: Option<Aes256StreamState>,
}

impl<'a, W: Write> SevenZPayloadSink<'a, W> {
    pub(crate) fn new(writer: &'a mut W, password: Option<&str>) -> Self {
        let aes = password.filter(|p| !p.is_empty()).map(|pass| {
            let mut salt = [0u8; 16];
            let mut iv = [0u8; 16];
            unsafe {
                libc::arc4random_buf(salt.as_mut_ptr() as *mut libc::c_void, 16);
                libc::arc4random_buf(iv.as_mut_ptr() as *mut libc::c_void, 16);
            }
            let num_cycles_power = 19u32;
            let key = sha256_7z_kdf(pass, &salt, num_cycles_power);
            Aes256StreamState {
                key,
                salt,
                initial_iv: iv,
                current_iv: iv,
                num_cycles_power,
                carry: [0u8; 16],
                carry_len: 0,
                enc_buf: vec![0u8; AES_STREAM_BUF_SIZE],
            }
        });
        Self { writer, raw_comp_len: 0, compressed_len: 0, aes }
    }

    pub(crate) fn write_chunk(&mut self, mut data: &[u8]) -> Result<(), TTZipStatus> {
        if data.is_empty() {
            return Ok(());
        }
        self.raw_comp_len += data.len() as u64;

        let Some(ref mut aes) = self.aes else {
            self.writer.write_all(data).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            self.compressed_len += data.len() as u64;
            return Ok(());
        };

        if aes.carry_len > 0 {
            let take = (16 - aes.carry_len).min(data.len());
            aes.carry[aes.carry_len..aes.carry_len + take].copy_from_slice(&data[..take]);
            aes.carry_len += take;
            data = &data[take..];
            if aes.carry_len == 16 {
                let mut out_block = [0u8; 16];
                aes256_cbc_encrypt(&aes.key, &aes.current_iv, &aes.carry, &mut out_block)
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                aes.current_iv = out_block;
                self.writer.write_all(&out_block).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                self.compressed_len += 16;
                aes.carry_len = 0;
            }
        }

        let full_len = (data.len() / 16) * 16;
        for chunk in data[..full_len].chunks(AES_STREAM_BUF_SIZE) {
            aes256_cbc_encrypt(&aes.key, &aes.current_iv, chunk, &mut aes.enc_buf[..chunk.len()])
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            aes.current_iv.copy_from_slice(&aes.enc_buf[chunk.len() - 16..chunk.len()]);
            self.writer.write_all(&aes.enc_buf[..chunk.len()]).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            self.compressed_len += chunk.len() as u64;
        }

        let rem = data.len() - full_len;
        if rem > 0 {
            aes.carry[..rem].copy_from_slice(&data[full_len..]);
            aes.carry_len = rem;
        }
        Ok(())
    }

    pub(crate) fn finish(mut self) -> Result<(u64, Option<SevenZAesWriterConfig>), TTZipStatus> {
        if let Some(ref mut aes) = self.aes {
            if self.raw_comp_len == 0 && self.compressed_len == 0 && aes.carry_len == 0 {
                return Ok((0, None));
            }
            if aes.carry_len > 0 || self.compressed_len == 0 {
                aes.carry[aes.carry_len..16].fill(0);
                let mut out_block = [0u8; 16];
                aes256_cbc_encrypt(&aes.key, &aes.current_iv, &aes.carry, &mut out_block)
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                aes.current_iv = out_block;
                self.writer.write_all(&out_block).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                self.compressed_len += 16;
                aes.carry_len = 0;
            }
            let cfg = SevenZAesWriterConfig {
                salt: aes.salt,
                iv: aes.initial_iv,
                num_cycles_power: aes.num_cycles_power,
                raw_comp_len: self.raw_comp_len,
            };
            Ok((self.compressed_len, Some(cfg)))
        } else {
            Ok((self.compressed_len, None))
        }
    }
}

/// Creates a complete 7z Solid archive from in-memory input items with optional AES-256 encryption.
pub fn create_7z_solid_archive_bytes_with_password(
    items: &[ZipInputItem],
    level: i32,
    threads: u32,
    password: Option<&str>,
) -> Result<Vec<u8>, TTZipStatus> {
    if items.is_empty() {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let mut stream_sizes = Vec::with_capacity(items.len());
    let mut stream_crcs = Vec::with_capacity(items.len());
    let mut uncompressed_len: u64 = 0;
    for item in items.iter().filter(|it| !it.is_directory && !it.data.is_empty()) {
        stream_sizes.push(item.data.len() as u64);
        stream_crcs.push(crc32_fast(0, &item.data));
        uncompressed_len += item.data.len() as u64;
    }

    let mut final_payload = Vec::with_capacity(uncompressed_len as usize);
    let mut sink = SevenZPayloadSink::new(&mut final_payload, password);
    let (method_id, coder_props) = if level == 0 || uncompressed_len == 0 {
        for item in items.iter().filter(|it| !it.is_directory && !it.data.is_empty()) {
            sink.write_chunk(&item.data)?;
        }
        (METHOD_COPY, Vec::new())
    } else {
        let mut cstream = init_fl2_stream(level, threads)?;
        let mut out_chunk = vec![0u8; STREAM_CHUNK_SIZE];
        for item in items.iter().filter(|it| !it.is_directory && !it.data.is_empty()) {
            feed_fl2_slice(&mut cstream, &item.data, &mut out_chunk, |comp| sink.write_chunk(comp))?;
        }
        let dict_prop = finish_fl2_stream(&mut cstream, &mut out_chunk, |comp| sink.write_chunk(comp))?;
        (METHOD_LZMA2, vec![dict_prop])
    };
    let (compressed_len, aes_config) = sink.finish()?;

    let header_bytes = build_7z_metadata_header_full(
        items, &stream_sizes, &stream_crcs, compressed_len, uncompressed_len, method_id, &coder_props, aes_config.as_ref(),
    );
    let sig_bytes = build_sig_header_bytes(compressed_len, &header_bytes);

    let mut out = Vec::with_capacity(32 + final_payload.len() + header_bytes.len());
    out.extend_from_slice(&sig_bytes);
    out.extend_from_slice(&final_payload);
    out.extend_from_slice(&header_bytes);
    Ok(out)
}

/// Creates a complete 7z Solid archive from in-memory input items.
pub fn create_7z_solid_archive_bytes(
    items: &[ZipInputItem],
    level: i32,
    threads: u32,
) -> Result<Vec<u8>, TTZipStatus> {
    create_7z_solid_archive_bytes_with_password(items, level, threads, None)
}

struct StreamDiskContext<'a> {
    options: &'a TTZipCreateOptions,
    cancel_flag: Option<&'a AtomicBool>,
    total_expected: u64,
    processed_bytes: u64,
    last_report: Instant,
}

impl StreamDiskContext<'_> {
    fn report(&mut self, entry_name: &str, force: bool) -> Result<(), TTZipStatus> {
        if self.cancel_flag.is_some_and(|f| f.load(Ordering::Relaxed)) {
            return Err(TTZipStatus::Cancelled);
        }
        if let Some(cb) = self.options.progress_callback {
            if force || self.last_report.elapsed() >= Duration::from_millis(16) {
                self.last_report = Instant::now();
                let rel_c = CString::new(entry_name).unwrap_or_default();
                let total = self.total_expected.max(self.processed_bytes);
                let keep_going = unsafe {
                    cb(self.processed_bytes, total, rel_c.as_ptr(), self.options.user_data)
                };
                if !keep_going {
                    return Err(TTZipStatus::Cancelled);
                }
            }
        }
        Ok(())
    }
}

fn stream_single_disk_entry<W: Write>(
    entry: &mut SevenZDiskEntry,
    cstream: &mut Option<Fl2CStream>,
    sink: &mut SevenZPayloadSink<'_, W>,
    ctx: &mut StreamDiskContext<'_>,
    read_buf: &mut [u8],
    out_chunk: &mut [u8],
) -> Result<Option<(u64, u32)>, TTZipStatus> {
    ctx.report(&entry.rel_path, false)?;
    if let Some(ref target_bytes) = entry.symlink_target {
        if target_bytes.is_empty() {
            entry.is_empty_stream = true;
            return Ok(None);
        }
        let entry_crc = crc32_fast(0, target_bytes);
        if let Some(fl2) = cstream {
            feed_fl2_slice(fl2, target_bytes, out_chunk, |comp| sink.write_chunk(comp))?;
        } else {
            sink.write_chunk(target_bytes)?;
        }
        let entry_len = target_bytes.len() as u64;
        ctx.processed_bytes += entry_len;
        entry.is_empty_stream = false;
        return Ok(Some((entry_len, entry_crc)));
    }
    let Some(ref disk_path) = entry.disk_path else {
        entry.is_empty_stream = true;
        return Ok(None);
    };
    let mut file = File::open(disk_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let mut entry_len: u64 = 0;
    let mut entry_crc: u32 = 0;
    loop {
        let n = file.read(read_buf).map_err(|_| TTZipStatus::ErrFileNotFound)?;
        if n == 0 {
            break;
        }
        let chunk = &read_buf[..n];
        entry_crc = crc32_fast(entry_crc, chunk);
        if let Some(fl2) = cstream {
            feed_fl2_slice(fl2, chunk, out_chunk, |comp| sink.write_chunk(comp))?;
        } else {
            sink.write_chunk(chunk)?;
        }
        entry_len += n as u64;
        ctx.processed_bytes += n as u64;
        ctx.report(&entry.rel_path, n >= 65536)?;
    }
    entry.is_empty_stream = entry_len == 0;
    Ok((entry_len > 0).then_some((entry_len, entry_crc)))
}

fn stream_disk_entries<W: Write>(
    entries: &mut [SevenZDiskEntry],
    mut cstream: Option<Fl2CStream>,
    sink: &mut SevenZPayloadSink<'_, W>,
    ctx: &mut StreamDiskContext<'_>,
    stream_sizes: &mut Vec<u64>,
    stream_crcs: &mut Vec<u32>,
) -> Result<(u64, u64, Vec<u8>), TTZipStatus> {
    let mut read_buf = vec![0u8; STREAM_CHUNK_SIZE];
    let mut out_chunk = vec![0u8; STREAM_CHUNK_SIZE];
    let mut total_uncomp: u64 = 0;

    for entry in entries.iter_mut() {
        if let Some((entry_len, entry_crc)) = stream_single_disk_entry(
            entry, &mut cstream, sink, ctx, &mut read_buf, &mut out_chunk,
        )? {
            total_uncomp += entry_len;
            stream_sizes.push(entry_len);
            stream_crcs.push(entry_crc);
        }
    }

    if let Some(ref mut fl2) = cstream {
        if total_uncomp > 0 {
            let dict_prop = finish_fl2_stream(fl2, &mut out_chunk, |comp| sink.write_chunk(comp))?;
            return Ok((total_uncomp, METHOD_LZMA2, vec![dict_prop]));
        }
    }
    Ok((total_uncomp, METHOD_COPY, Vec::new()))
}

fn resolve_stream_encoder(
    options: &TTZipCreateOptions,
    total_expected: u64,
) -> Result<(Option<Fl2CStream>, Option<&str>), TTZipStatus> {
    let level_int = match options.level {
        TTZipCompressionLevel::Store => 0,
        TTZipCompressionLevel::Fastest => 1,
        TTZipCompressionLevel::Fast => 3,
        TTZipCompressionLevel::Normal => 6,
        TTZipCompressionLevel::Maximum => 9,
        TTZipCompressionLevel::Ultra => 12,
    };
    let password = if !options.password.is_null() {
        unsafe { std::ffi::CStr::from_ptr(options.password).to_str().ok() }
    } else {
        None
    };
    let cstream = if level_int == 0 || total_expected == 0 {
        None
    } else {
        Some(init_fl2_stream(level_int.min(10), options.thread_budget.max(1))?)
    };
    Ok((cstream, password))
}

fn validate_7z_encryption_policy(
    options: &TTZipCreateOptions,
    password: Option<&str>,
    encrypt_file_names: Option<bool>,
) -> Result<bool, TTZipStatus> {
    let has_non_empty_password = password.is_some_and(|p| !p.is_empty());
    let req_enc = encrypt_file_names == Some(true)
        || options.encryption != crate::types::TTZipEncryptionMethod::None
        || password == Some("");
    if req_enc && !has_non_empty_password {
        return Err(TTZipStatus::ErrInvalidPassword);
    }
    Ok(encrypt_file_names.unwrap_or(has_non_empty_password) && has_non_empty_password)
}

struct StreamedSevenZDiskPayload {
    stream_sizes: Vec<u64>,
    stream_crcs: Vec<u32>,
    uncompressed_len: u64,
    compressed_len: u64,
    method_id: u64,
    coder_props: Vec<u8>,
    aes_config: Option<SevenZAesWriterConfig>,
}

fn encode_and_stream_7z_disk_payload(
    entries: &mut [SevenZDiskEntry],
    cstream: Option<Fl2CStream>,
    password: Option<&str>,
    ctx: &mut StreamDiskContext<'_>,
    first_name: &str,
    writer: &mut crate::archive::split::SevenZOutputTarget,
) -> Result<StreamedSevenZDiskPayload, TTZipStatus> {
    let mut stream_sizes = Vec::with_capacity(entries.len());
    let mut stream_crcs = Vec::with_capacity(entries.len());
    let mut sink = SevenZPayloadSink::new(&mut *writer, password);

    let (uncompressed_len, method_id, coder_props) = match stream_disk_entries(
        entries,
        cstream,
        &mut sink,
        ctx,
        &mut stream_sizes,
        &mut stream_crcs,
    ) {
        Ok(v) => v,
        Err(status) => {
            writer.abort_in_txn(status);
            return Err(status);
        }
    };
    let (compressed_len, aes_config) = match sink.finish() {
        Ok(v) => v,
        Err(status) => {
            writer.abort_in_txn(status);
            return Err(status);
        }
    };
    ctx.processed_bytes = uncompressed_len;
    if let Err(status) = ctx.report(first_name, true) {
        writer.abort_in_txn(status);
        return Err(status);
    }
    Ok(StreamedSevenZDiskPayload {
        stream_sizes,
        stream_crcs,
        uncompressed_len,
        compressed_len,
        method_id,
        coder_props,
        aes_config,
    })
}

fn build_and_finalize_7z_disk_header(
    entries: &[SevenZDiskEntry],
    payload: &StreamedSevenZDiskPayload,
    writer: crate::archive::split::SevenZOutputTarget,
    use_encoded_header: bool,
    password: Option<&str>,
) -> Result<u64, TTZipStatus> {
    let refs: Vec<HeaderEntryRef<'_>> = entries
        .iter()
        .map(|e| HeaderEntryRef {
            rel_path: &e.rel_path,
            is_directory: e.is_directory,
            is_empty_stream: e.is_empty_stream,
            is_symlink: e.is_symlink,
            posix_mode: e.posix_mode,
            mtime: e.mtime,
        })
        .collect();
    let plain_header_bytes = build_7z_metadata_header_from_refs(
        &refs,
        &payload.stream_sizes,
        &payload.stream_crcs,
        payload.compressed_len,
        payload.uncompressed_len,
        payload.method_id,
        &payload.coder_props,
        payload.aes_config.as_ref(),
    );
    finalize_7z_header_and_signature(
        writer,
        &plain_header_bytes,
        payload.compressed_len,
        use_encoded_header,
        password,
    )
}

pub(crate) fn write_7z_archive_streaming_from_disk_in_txn(
    dest_path: &Path,
    mut entries: Vec<SevenZDiskEntry>,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    encrypt_file_names: Option<bool>,
    cancel_flag: Option<&AtomicBool>,
    start_time: Instant,
    txn_ctx: Option<&CreateTransactionContext>,
) -> Result<ZipCreateReport, TTZipStatus> {
    if entries.is_empty() {
        return Err(TTZipStatus::ErrInvalidParam);
    }

    let total_expected: u64 = entries.iter().filter(|e| !e.is_directory).map(|e| e.size).sum();
    let (cstream, password) = resolve_stream_encoder(options, total_expected)?;
    let use_encoded_header = validate_7z_encryption_policy(options, password, encrypt_file_names)?;

    let first_name = entries[0].rel_path.clone();
    let mut ctx = StreamDiskContext {
        options,
        cancel_flag,
        total_expected,
        processed_bytes: 0,
        last_report: Instant::now(),
    };
    ctx.report(&first_name, true)?;

    let mut writer = crate::archive::split::SevenZOutputTarget::open_in_txn(
        dest_path,
        split_volume_size_bytes,
        STREAM_CHUNK_SIZE,
        txn_ctx,
    )
    .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    if writer.write_all(&[0u8; 32]).is_err() {
        writer.abort_in_txn(TTZipStatus::ErrCompressionFailed);
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    let payload = encode_and_stream_7z_disk_payload(
        &mut entries,
        cstream,
        password,
        &mut ctx,
        &first_name,
        &mut writer,
    )?;
    let total_compressed_bytes = build_and_finalize_7z_disk_header(
        &entries,
        &payload,
        writer,
        use_encoded_header,
        password,
    )?;

    Ok(ZipCreateReport {
        total_entries: entries.len(),
        total_uncompressed_bytes: payload.uncompressed_len,
        total_compressed_bytes,
        duration_ms: start_time.elapsed().as_millis() as u64,
    })
}

/// Creates a 7z archive file (single or split-volume) directly from input source paths with explicit header-encryption control.
pub fn create_7z_archive_split_with_header_encryption(
    dest_path: &Path,
    source_paths: &[PathBuf],
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    encrypt_file_names: Option<bool>,
) -> Result<ZipCreateReport, TTZipStatus> {
    create_7z_archive_split_with_header_encryption_in_txn(
        dest_path,
        source_paths,
        options,
        split_volume_size_bytes,
        encrypt_file_names,
        None,
    )
}

/// Creates a 7z archive file (single or split-volume) directly from input source paths.
pub fn create_7z_archive_split(
    dest_path: &Path,
    source_paths: &[PathBuf],
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<ZipCreateReport, TTZipStatus> {
    create_7z_archive_split_in_txn(dest_path, source_paths, options, split_volume_size_bytes, None)
}

/// Creates a 7z archive file directly from input source paths using bounded-memory disk streaming.
pub fn create_7z_archive(
    dest_path: &Path,
    source_paths: &[PathBuf],
    options: &TTZipCreateOptions,
) -> Result<ZipCreateReport, TTZipStatus> {
    create_7z_archive_split(dest_path, source_paths, options, 0)
}

/// Creates a 7z archive file (single or split-volume) from explicit pre-collected entries with explicit header-encryption control.
pub fn create_7z_archive_explicit_split_with_header_encryption(
    dest_path: &Path,
    entries: &[(PathBuf, String)],
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    encrypt_file_names: Option<bool>,
) -> Result<ZipCreateReport, TTZipStatus> {
    create_7z_archive_explicit_split_with_header_encryption_in_txn(
        dest_path,
        entries,
        options,
        split_volume_size_bytes,
        encrypt_file_names,
        None,
    )
}

/// Creates a 7z archive file (single or split-volume) from explicit pre-collected entries.
pub fn create_7z_archive_explicit_split(
    dest_path: &Path,
    entries: &[(PathBuf, String)],
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<ZipCreateReport, TTZipStatus> {
    create_7z_archive_explicit_split_with_header_encryption(
        dest_path,
        entries,
        options,
        split_volume_size_bytes,
        None,
    )
}

/// Creates a 7z archive file directly from explicit pre-collected entries using bounded-memory disk streaming.
pub fn create_7z_archive_explicit(
    dest_path: &Path,
    entries: &[(PathBuf, String)],
    options: &TTZipCreateOptions,
) -> Result<ZipCreateReport, TTZipStatus> {
    create_7z_archive_explicit_split(dest_path, entries, options, 0)
}


