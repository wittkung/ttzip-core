// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Multithreaded parallel compression engine and shared entry encryption/LFH builders for ZIP archives.

use super::data_descriptor::compute_zipcrypto_check_byte;
use super::parallel_batch::CompressedEntryResult;
use super::streaming_sink::{EntryPayload, SpillFile, STREAM_CHUNK_SIZE};
use super::types::{unix_to_dos_time, ZipCompressedItem, ZipInputItem};
use crate::codecs::deflate::{deflate_compress, deflate_compress_bound};
use crate::crypto::crc32::crc32_fast;
use crate::crypto::sha1::winzip_aes256_encrypt_and_tag;
use crate::crypto::winzip_aes::{WinZipAesEncrypter, WinZipAesKeyStrength, WinZipAesVersion};
use crate::types::{TTZipEncryptionMethod, TTZipStatus};
use crate::zip::extra::ZipExtraFields;
use crate::zip::parser::MAGIC_LFH;
use rayon::prelude::*;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use ttzip_types::options::ProgressAtomicSheet;

fn create_encrypted_spill_file() -> Result<SpillFile, TTZipStatus> {
    let temp_path = std::env::temp_dir().join(format!(
        "ttzip_enc_spill_{}_{}.tmp",
        std::process::id(),
        unsafe { libc::arc4random() }
    ));
    let file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .read(true)
        .open(&temp_path)
        .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    Ok(SpillFile {
        path: temp_path,
        file,
        len: 0,
    })
}

fn stream_encrypt_aes256_to_spill(
    src_path: &Path,
    pass: &str,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<EntryPayload, TTZipStatus> {
    let mut src = File::open(src_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let mut spill = create_encrypted_spill_file()?;
    let mut salt = [0u8; 16];
    unsafe {
        libc::arc4random_buf(salt.as_mut_ptr() as *mut libc::c_void, salt.len());
    }
    let mut enc = WinZipAesEncrypter::new(
        WinZipAesVersion::AE2,
        WinZipAesKeyStrength::Aes256,
        pass,
        &salt,
    )?;
    spill
        .file
        .write_all(&salt)
        .and_then(|_| spill.file.write_all(&enc.pvv()))
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    spill.len += 18;

    let mut buf = [0u8; STREAM_CHUNK_SIZE];
    loop {
        if sheet.is_some_and(ProgressAtomicSheet::is_cancelled) {
            return Err(TTZipStatus::Cancelled);
        }
        let n = src.read(&mut buf).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        if n == 0 {
            break;
        }
        enc.encrypt_chunk(&mut buf[..n]);
        spill
            .file
            .write_all(&buf[..n])
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        spill.len += n as u64;
    }
    let (tag, _) = enc.finalize();
    spill
        .file
        .write_all(&tag)
        .and_then(|_| spill.file.flush())
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    spill.len += 10;
    Ok(EntryPayload::Spill(Arc::new(spill)))
}

fn stream_encrypt_zipcrypto_to_spill(
    src_path: &Path,
    pass: &str,
    crc: u32,
    mtime_secs: u32,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<EntryPayload, TTZipStatus> {
    let mut src = File::open(src_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let mut spill = create_encrypted_spill_file()?;
    let mut header = [0u8; 12];
    unsafe {
        libc::arc4random_buf(header.as_mut_ptr() as *mut libc::c_void, 11);
    }
    let (dos_time, _) = unix_to_dos_time(mtime_secs);
    header[11] = compute_zipcrypto_check_byte(crc, dos_time, true);
    let mut keys = crate::crypto::zipcrypto::ZipCryptoKeys::from_password(pass.as_bytes());
    keys.encrypt_slice(&mut header);
    spill
        .file
        .write_all(&header)
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    spill.len += 12;

    let mut buf = [0u8; STREAM_CHUNK_SIZE];
    loop {
        if sheet.is_some_and(ProgressAtomicSheet::is_cancelled) {
            return Err(TTZipStatus::Cancelled);
        }
        let n = src.read(&mut buf).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        if n == 0 {
            break;
        }
        keys.encrypt_slice(&mut buf[..n]);
        spill
            .file
            .write_all(&buf[..n])
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        spill.len += n as u64;
    }
    spill
        .file
        .flush()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    Ok(EntryPayload::Spill(Arc::new(spill)))
}

pub(super) fn encrypt_raw_zip_bytes(
    encryption: TTZipEncryptionMethod,
    password: Option<&str>,
    actual_method: u16,
    crc: u32,
    mtime_secs: u32,
    has_data_descriptor: bool,
    raw_bytes: Vec<u8>,
) -> Result<(u16, u8, bool, Vec<u8>), TTZipStatus> {
    match encryption {
        TTZipEncryptionMethod::Aes256 => {
            let pass = password.ok_or(TTZipStatus::ErrInvalidPassword)?;
            let mut salt = [0u8; 16];
            unsafe {
                libc::arc4random_buf(salt.as_mut_ptr() as *mut libc::c_void, salt.len());
            }
            let mut enc_payload = Vec::new();
            winzip_aes256_encrypt_and_tag(pass, &salt, &raw_bytes, &mut enc_payload)?;
            Ok((99u16, 3u8, true, enc_payload))
        }
        TTZipEncryptionMethod::ZipCrypto => {
            let pass = password.ok_or(TTZipStatus::ErrInvalidPassword)?;
            let mut enc_payload = Vec::with_capacity(12 + raw_bytes.len());
            let mut header = [0u8; 12];
            unsafe {
                libc::arc4random_buf(header.as_mut_ptr() as *mut libc::c_void, 11);
            }
            let (dos_time, _) = unix_to_dos_time(mtime_secs);
            header[11] = compute_zipcrypto_check_byte(crc, dos_time, has_data_descriptor);
            let mut keys = crate::crypto::zipcrypto::ZipCryptoKeys::from_password(pass.as_bytes());
            keys.encrypt_slice(&mut header);
            enc_payload.extend_from_slice(&header);
            let mut body = raw_bytes;
            keys.encrypt_slice(&mut body);
            enc_payload.extend_from_slice(&body);
            Ok((actual_method, 0u8, true, enc_payload))
        }
        _ => Ok((actual_method, 0u8, false, raw_bytes)),
    }
}

fn encrypt_disk_payload_to_spill(
    encryption: TTZipEncryptionMethod,
    password: Option<&str>,
    actual_method: u16,
    crc: u32,
    mtime_secs: u32,
    src_path: &Path,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<(u16, bool, EntryPayload), TTZipStatus> {
    let pass = password.ok_or(TTZipStatus::ErrInvalidPassword)?;
    if encryption == TTZipEncryptionMethod::Aes256 {
        let enc = stream_encrypt_aes256_to_spill(src_path, pass, sheet)?;
        Ok((99u16, true, enc))
    } else {
        let enc = stream_encrypt_zipcrypto_to_spill(src_path, pass, crc, mtime_secs, sheet)?;
        Ok((actual_method, true, enc))
    }
}

pub(super) fn encrypt_entry_payload(
    encryption: TTZipEncryptionMethod,
    password: Option<&str>,
    actual_method: u16,
    crc: u32,
    mtime_secs: u32,
    payload: EntryPayload,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<(u16, bool, EntryPayload), TTZipStatus> {
    if sheet.is_some_and(ProgressAtomicSheet::is_cancelled) {
        return Err(TTZipStatus::Cancelled);
    }
    if !matches!(
        encryption,
        TTZipEncryptionMethod::Aes256 | TTZipEncryptionMethod::ZipCrypto
    ) {
        return Ok((actual_method, false, payload));
    }
    match payload {
        EntryPayload::Memory(raw_bytes) => {
            let (comp_method, _, is_encrypted, enc_bytes) = encrypt_raw_zip_bytes(
                encryption,
                password,
                actual_method,
                crc,
                mtime_secs,
                true,
                raw_bytes,
            )?;
            Ok((comp_method, is_encrypted, EntryPayload::Memory(enc_bytes)))
        }
        EntryPayload::Spill(spill) => encrypt_disk_payload_to_spill(
            encryption,
            password,
            actual_method,
            crc,
            mtime_secs,
            &spill.path,
            sheet,
        ),
        EntryPayload::OriginalFile { path, .. } => encrypt_disk_payload_to_spill(
            encryption,
            password,
            actual_method,
            crc,
            mtime_secs,
            &path,
            sheet,
        ),
    }
}

pub(super) fn build_directory_entry_result(
    rel_path: String,
    mtime_secs: u32,
    mode: u32,
) -> CompressedEntryResult {
    let (dos_time, dos_date) = unix_to_dos_time(mtime_secs);
    let name_bytes = rel_path.as_bytes();
    let ts_extra = ZipExtraFields::build_extended_timestamp(mtime_secs);
    let mut header = Vec::with_capacity(30 + name_bytes.len() + ts_extra.len());
    for part in [
        &MAGIC_LFH.to_le_bytes()[..],
        &20u16.to_le_bytes(),
        &0x0800u16.to_le_bytes(),
        &0u16.to_le_bytes(),
        &dos_time.to_le_bytes(),
        &dos_date.to_le_bytes(),
        &[0u8; 12],
        &(name_bytes.len() as u16).to_le_bytes(),
        &(ts_extra.len() as u16).to_le_bytes(),
        name_bytes,
        &ts_extra,
    ] {
        header.extend_from_slice(part);
    }

    CompressedEntryResult {
        rel_path,
        uncompressed_size: 0,
        compressed_size: 0,
        crc32: 0,
        compression_method: 0,
        actual_method: 0,
        is_encrypted: false,
        mtime_secs,
        mode,
        is_directory: true,
        header_bytes: header,
        payload: EntryPayload::Memory(Vec::new()),
    }
}

pub(super) fn build_file_lfh_header(
    rel_path: &str,
    mtime_secs: u32,
    uncompressed_size: u64,
    compressed_size: u64,
    comp_method: u16,
    actual_method: u16,
    is_encrypted: bool,
) -> Vec<u8> {
    let (dos_time, dos_date) = unix_to_dos_time(mtime_secs);
    let name_bytes = rel_path.as_bytes();
    let is_zip64 = uncompressed_size >= 0xFFFF_FFFF || compressed_size >= 0xFFFF_FFFF;
    let mut extra_fields = Vec::new();
    if is_zip64 {
        extra_fields.extend_from_slice(&ZipExtraFields::build_zip64_extra(
            Some(uncompressed_size),
            Some(compressed_size),
            None,
        ));
    }
    if is_encrypted && comp_method == 99 {
        extra_fields.extend_from_slice(&ZipExtraFields::build_winzip_aes_extra(actual_method));
    }
    extra_fields.extend_from_slice(&ZipExtraFields::build_extended_timestamp(mtime_secs));

    let flag = if is_encrypted { 0x0809u16 } else { 0x0808u16 };
    let ver = if is_zip64 || is_encrypted { 45u16 } else { 20u16 };
    let mut header = Vec::with_capacity(30 + name_bytes.len() + extra_fields.len());
    for part in [
        &MAGIC_LFH.to_le_bytes()[..],
        &ver.to_le_bytes(),
        &flag.to_le_bytes(),
        &comp_method.to_le_bytes(),
        &dos_time.to_le_bytes(),
        &dos_date.to_le_bytes(),
        &[0u8; 12],
        &(name_bytes.len() as u16).to_le_bytes(),
        &(extra_fields.len() as u16).to_le_bytes(),
        name_bytes,
        &extra_fields,
    ] {
        header.extend_from_slice(part);
    }
    header
}

/// Compresses a single input item in-place with zero unnecessary buffer cloning.
#[inline]
fn compress_single_item(
    item: ZipInputItem,
    level: i32,
    encryption: TTZipEncryptionMethod,
    password: Option<&str>,
) -> Result<ZipCompressedItem, TTZipStatus> {
    if item.is_directory || item.data.is_empty() {
        return Ok(ZipCompressedItem {
            rel_path: item.rel_path,
            uncompressed_size: 0,
            compressed_size: 0,
            crc32: 0,
            compression_method: 0,
            actual_method: 0,
            aes_strength: 0,
            payload: Vec::new(),
            mtime_epoch_secs: item.mtime_epoch_secs,
            mode: item.mode,
            is_directory: item.is_directory,
            is_encrypted: false,
        });
    }

    let uncompressed_size = item.data.len() as u64;
    let crc32 = crc32_fast(0, &item.data);

    let (actual_method, raw_payload) = if level == 0 {
        (0u16, item.data)
    } else {
        let mut comp_buf = vec![0u8; deflate_compress_bound(item.data.len(), level)];
        let comp_len = deflate_compress(&item.data, &mut comp_buf, level)?;
        comp_buf.truncate(comp_len);
        (8u16, comp_buf)
    };

    let (compression_method, aes_strength, is_encrypted, final_payload) = encrypt_raw_zip_bytes(
        encryption,
        password,
        actual_method,
        crc32,
        item.mtime_epoch_secs,
        false,
        raw_payload,
    )?;

    let compressed_size = final_payload.len() as u64;

    Ok(ZipCompressedItem {
        rel_path: item.rel_path,
        uncompressed_size,
        compressed_size,
        crc32,
        compression_method,
        actual_method,
        aes_strength,
        payload: final_payload,
        mtime_epoch_secs: item.mtime_epoch_secs,
        mode: item.mode,
        is_directory: false,
        is_encrypted,
    })
}

/// Compresses a batch of `ZipInputItem`s in parallel across threads with cooperative cancellation support.
pub fn compress_items_parallel_with_token(
    items: Vec<ZipInputItem>,
    level: i32,
    encryption: TTZipEncryptionMethod,
    password: Option<&str>,
    thread_budget: u32,
    cancel_token: Option<&AtomicBool>,
) -> Result<Vec<ZipCompressedItem>, TTZipStatus> {
    if items.is_empty() {
        return Ok(Vec::new());
    }

    if let Some(token) = cancel_token {
        if token.load(Ordering::Acquire) {
            return Err(TTZipStatus::Cancelled);
        }
    }

    let pool_budget = (thread_budget as usize).clamp(1, 64);
    let compress_one = |item| {
        if let Some(token) = cancel_token
            && token.load(Ordering::Acquire)
        {
            return Err(TTZipStatus::Cancelled);
        }
        compress_single_item(item, level, encryption, password)
    };
    if items.len() <= 1 || pool_budget <= 1 {
        return items.into_iter().map(compress_one).collect();
    }

    let pool = crate::archive::shared_compute_pool(pool_budget);
    pool.install(|| items.into_par_iter().map(compress_one).collect())
}

/// Compresses a batch of `ZipInputItem`s in parallel across threads using work-stealing pool.
#[inline]
pub fn compress_items_parallel(
    items: Vec<ZipInputItem>,
    level: i32,
    encryption: TTZipEncryptionMethod,
    password: Option<&str>,
    thread_budget: u32,
) -> Result<Vec<ZipCompressedItem>, TTZipStatus> {
    compress_items_parallel_with_token(items, level, encryption, password, thread_budget, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_compression_basic_roundtrip() {
        let items = vec![
            ZipInputItem {
                rel_path: "file1.txt".to_string(),
                data: b"hello world from parallel stream 1".to_vec(),
                mtime_epoch_secs: 1700000000,
                mode: 0o644,
                is_directory: false,
            },
            ZipInputItem {
                rel_path: "file2.txt".to_string(),
                data: b"another chunk of data for parallel stream 2".to_vec(),
                mtime_epoch_secs: 1700000001,
                mode: 0o644,
                is_directory: false,
            },
        ];

        let results = compress_items_parallel(items, 6, TTZipEncryptionMethod::None, None, 4)
            .expect("compression should succeed");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].rel_path, "file1.txt");
        assert_eq!(results[1].rel_path, "file2.txt");
    }

    #[test]
    fn test_parallel_compression_cooperative_cancellation() {
        let items = vec![
            ZipInputItem {
                rel_path: "file1.txt".to_string(),
                data: vec![0x42; 1024 * 64],
                mtime_epoch_secs: 1700000000,
                mode: 0o644,
                is_directory: false,
            },
        ];

        let cancel_token = AtomicBool::new(true); // Pre-cancelled
        let res = compress_items_parallel_with_token(
            items,
            6,
            TTZipEncryptionMethod::None,
            None,
            4,
            Some(&cancel_token),
        );
        assert_eq!(res, Err(TTZipStatus::Cancelled));
    }

    #[test]
    fn test_encrypt_spilled_payload_aes256_and_zipcrypto() {
        let dir = tempfile::tempdir().expect("tempdir");
        let raw_path = dir.path().join("spilled_raw.bin");
        let plain: Vec<u8> = (0..(150 * 1024)).map(|i| ((i * 17 + 3) & 0xFF) as u8).collect();
        std::fs::write(&raw_path, &plain).expect("write spill");
        let crc = crc32_fast(0, &plain);

        let spill_file = File::open(&raw_path).expect("open spill");
        let spill_payload = EntryPayload::Spill(Arc::new(SpillFile {
            path: raw_path.clone(),
            file: spill_file,
            len: plain.len() as u64,
        }));

        let (method, is_enc, enc_payload) = encrypt_entry_payload(
            TTZipEncryptionMethod::Aes256,
            Some("secret-aes-pass"),
            8,
            crc,
            1700000000,
            spill_payload,
            None,
        )
        .expect("encrypt spill aes256");
        assert_eq!(method, 99);
        assert!(is_enc);
        let enc_bytes = match &enc_payload {
            EntryPayload::Spill(s) => std::fs::read(&s.path).expect("read encrypted spill"),
            _ => panic!("expected spilled encrypted payload"),
        };
        let mut dec_buf = vec![0u8; plain.len()];
        let dec_len = crate::crypto::sha1::winzip_aes256_decrypt_and_verify(
            "secret-aes-pass",
            &enc_bytes,
            &mut dec_buf,
        )
        .expect("decrypt spilled aes256");
        assert_eq!(&dec_buf[..dec_len], &plain[..]);

        std::fs::write(&raw_path, &plain).expect("rewrite orig");
        let orig_payload = EntryPayload::OriginalFile {
            path: raw_path,
            len: plain.len() as u64,
        };
        let (zc_method, zc_enc, zc_payload) = encrypt_entry_payload(
            TTZipEncryptionMethod::ZipCrypto,
            Some("secret-zc-pass"),
            0,
            crc,
            1700000000,
            orig_payload,
            None,
        )
        .expect("encrypt orig zipcrypto");
        assert_eq!(zc_method, 0);
        assert!(zc_enc);
        let zc_bytes = match &zc_payload {
            EntryPayload::Spill(s) => std::fs::read(&s.path).expect("read zc spill"),
            _ => panic!("expected spilled zipcrypto payload"),
        };
        let (dos_time, _) = unix_to_dos_time(1700000000);
        let dec_zc = crate::zip::reader_stream::decrypt_zipcrypto_slice(
            &zc_bytes,
            "secret-zc-pass",
            crc,
            dos_time,
            true,
        )
        .expect("decrypt zc");
        assert_eq!(dec_zc, plain);
    }

    #[test]
    fn test_preflight_verify_encrypted_zip_entries_bounded_probe() {
        use std::io::{Cursor, Seek, SeekFrom};

        struct CountingCursor {
            inner: Cursor<Vec<u8>>,
            bytes_read: usize,
        }
        impl Read for CountingCursor {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                let n = self.inner.read(buf)?;
                self.bytes_read += n;
                Ok(n)
            }
        }
        impl Seek for CountingCursor {
            fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
                self.inner.seek(pos)
            }
        }

        let large_data: Vec<u8> = (0..(96 * 1024)).map(|i| ((i * 73 + 19) & 0xFF) as u8).collect();
        let items = vec![ZipInputItem {
            rel_path: "encrypted_large.bin".to_string(),
            data: large_data,
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        }];
        let comp = compress_items_parallel(
            items,
            0,
            TTZipEncryptionMethod::Aes256,
            Some("preflight-pass"),
            1,
        )
        .expect("compress aes");
        let zip_bytes = crate::zip::writer::assemble_zip_archive(&comp).expect("assemble");
        let total_len = zip_bytes.len() as u64;
        let entries = crate::zip::reader_stream::parse_entries_from_reader(
            &mut Cursor::new(&zip_bytes),
            total_len,
        )
        .expect("parse entries");
        assert!(entries[0].compressed_size > 64 * 1024);

        let mut reader = CountingCursor {
            inner: Cursor::new(zip_bytes),
            bytes_read: 0,
        };
        crate::zip::reader_stream::preflight_verify_encrypted_zip_entries(
            &mut reader,
            total_len,
            &entries,
            Some("preflight-pass"),
        )
        .expect("valid password should pass preflight");
        assert!(
            reader.bytes_read <= 256,
            "expected bounded header+PVV probe read <= 256 bytes, got {}",
            reader.bytes_read
        );

        let bad = crate::zip::reader_stream::preflight_verify_encrypted_zip_entries(
            &mut reader,
            total_len,
            &entries,
            Some("wrong-pass"),
        );
        assert_eq!(bad, Err(TTZipStatus::ErrInvalidPassword));
    }
}
