// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! 7-Zip metadata header builders and AES-256-CBC payload streaming sink.

use crate::crypto::aes256::aes256_cbc_encrypt;
use crate::crypto::sha256::sha256_7z_kdf;
use crate::sevenz::finish_contract::finalize_7z_header_and_signature;
use crate::sevenz::format::{
    build_7z_metadata_header_from_refs, HeaderEntryRef, SevenZAesWriterConfig,
};
use crate::sevenz::writer::SevenZDiskEntry;
use crate::types::{TTZipCreateOptions, TTZipStatus};
use crate::zip::writer::ZipInputItem;
use std::io::Write;
use std::time::Duration;

const AES_STREAM_BUF_SIZE: usize = 64 * 1024;

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
        &refs,
        stream_sizes,
        stream_crcs,
        compressed_len,
        uncompressed_len,
        method_id,
        coder_props,
        aes_config,
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
        items,
        stream_sizes,
        stream_crcs,
        compressed_len,
        uncompressed_len,
        method_id,
        coder_props,
        None,
    )
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
        Self {
            writer,
            raw_comp_len: 0,
            compressed_len: 0,
            aes,
        }
    }

    pub(crate) fn write_chunk(&mut self, mut data: &[u8]) -> Result<(), TTZipStatus> {
        if data.is_empty() {
            return Ok(());
        }
        self.raw_comp_len += data.len() as u64;

        let Some(ref mut aes) = self.aes else {
            self.writer
                .write_all(data)
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
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
                self.writer
                    .write_all(&out_block)
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                self.compressed_len += 16;
                aes.carry_len = 0;
            }
        }

        let full_len = (data.len() / 16) * 16;
        for chunk in data[..full_len].chunks(AES_STREAM_BUF_SIZE) {
            aes256_cbc_encrypt(&aes.key, &aes.current_iv, chunk, &mut aes.enc_buf[..chunk.len()])
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            aes.current_iv
                .copy_from_slice(&aes.enc_buf[chunk.len() - 16..chunk.len()]);
            self.writer
                .write_all(&aes.enc_buf[..chunk.len()])
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
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
                self.writer
                    .write_all(&out_block)
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
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

pub(crate) struct StreamedSevenZDiskPayload {
    pub(crate) stream_sizes: Vec<u64>,
    pub(crate) stream_crcs: Vec<u32>,
    pub(crate) uncompressed_len: u64,
    pub(crate) compressed_len: u64,
    pub(crate) method_id: u64,
    pub(crate) coder_props: Vec<u8>,
    pub(crate) aes_config: Option<SevenZAesWriterConfig>,
}

pub(crate) fn validate_7z_encryption_policy(
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

pub(crate) fn build_and_finalize_7z_disk_header(
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
