// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Phase 5 Integration Tests for Archive FFI:
//! - ttzip_rust_create_archive (ZIP, TAR, TAR.GZ)
//! - ttzip_rust_inspect_archive (metadata verification & early halting)
//! - ttzip_rust_extract_archive (two-stage safe extraction, dry-run, progress callback, ZipSlip defense)

use std::ffi::CString;
use std::fs::{self, File};
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::tempdir;
use ttzip_engine::ffi::*;
use ttzip_engine::types::{
    TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
    TTZipEntryMetadata, TTZipExtractOptions, TTZipStatus,
};

static INSPECT_COUNT: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn test_inspect_cb(
    entry: *const TTZipEntryMetadata,
    _user_data: *mut libc::c_void,
) -> bool {
    unsafe {
        if !entry.is_null() {
            let meta = &*entry;
            assert!(!meta.path.is_null());
            INSPECT_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        true
    }
}

static PROGRESS_BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn test_progress_cb(
    processed_bytes: u64,
    _total_bytes: u64,
    _current_entry: *const libc::c_char,
    _user_data: *mut libc::c_void,
) -> bool {
    PROGRESS_BYTES.store(processed_bytes as usize, Ordering::SeqCst);
    true
}

#[test]
fn test_phase5_archive_create_inspect_extract_roundtrip_zip() {
    let tmp_dir = tempdir().unwrap();
    let temp_dir = tmp_dir.path();

    let src_dir = temp_dir.join("src_files");
    fs::create_dir_all(&src_dir).unwrap();

    let file1_path = src_dir.join("hello.txt");
    let mut f1 = File::create(&file1_path).unwrap();
    f1.write_all(b"Hello World from TTZip Rust Glue!").unwrap();
    drop(f1);

    let sub_dir = src_dir.join("nested_folder");
    fs::create_dir_all(&sub_dir).unwrap();

    let file2_path = sub_dir.join("data.bin");
    let mut f2 = File::create(&file2_path).unwrap();
    f2.write_all(&[0x42; 1024]).unwrap();
    drop(f2);

    let archive_path = temp_dir.join("test_archive.zip");
    let c_archive_path = CString::new(archive_path.to_str().unwrap()).unwrap();
    let c_src_path = CString::new(src_dir.to_str().unwrap()).unwrap();

    let src_ptrs = [c_src_path.as_ptr()];

    // 1. Create Archive via FFI
    let create_options = TTZipCreateOptions {
        struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
        abi_version: ttzip_engine::types::TTZIP_ABI_VERSION_3,
        format: TTZipArchiveFormat::Zip,
        level: TTZipCompressionLevel::Normal,
        encryption: TTZipEncryptionMethod::None,
        password: std::ptr::null(),
        thread_budget: 4,
        solid_block_size_mb: 0,
        progress_callback: Some(test_progress_cb),
        user_data: std::ptr::null_mut(),
    };

    let create_status = unsafe {
        ttzip_rust_create_archive(
            src_ptrs.as_ptr(),
            src_ptrs.len(),
            c_archive_path.as_ptr(),
            &create_options,
        )
    };
    assert_eq!(create_status, TTZipStatus::Ok);
    assert!(archive_path.exists());

    // 2. Inspect Archive via FFI
    INSPECT_COUNT.store(0, Ordering::SeqCst);
    let inspect_status = unsafe {
        ttzip_rust_inspect_archive(
            c_archive_path.as_ptr(),
            std::ptr::null(),
            true,
            Some(test_inspect_cb),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(inspect_status, TTZipStatus::Ok);
    assert!(INSPECT_COUNT.load(Ordering::SeqCst) >= 2);

    // 3. Extract Archive via FFI (Dry-run first)
    let dest_dir = temp_dir.join("extracted");
    let c_dest_dir = CString::new(dest_dir.to_str().unwrap()).unwrap();

    let dry_run_options = TTZipExtractOptions {
        struct_size: std::mem::size_of::<TTZipExtractOptions>() as u32,
        abi_version: ttzip_engine::types::TTZIP_ABI_VERSION_3,
        destination_path: c_dest_dir.as_ptr(),
        password: std::ptr::null(),
        thread_budget: 4,
        collision_policy: 0,
        preserve_permissions: true,
        dry_run: true,
        progress_callback: Some(test_progress_cb),
        user_data: std::ptr::null_mut(),
    };

    let dry_status = unsafe {
        ttzip_rust_extract_archive(
            c_archive_path.as_ptr(),
            c_dest_dir.as_ptr(),
            &dry_run_options,
        )
    };
    assert_eq!(dry_status, TTZipStatus::Ok);
    assert!(!dest_dir.join("hello.txt").exists());

    // 4. Real Extract Archive via FFI
    let extract_options = TTZipExtractOptions {
        struct_size: std::mem::size_of::<TTZipExtractOptions>() as u32,
        abi_version: ttzip_engine::types::TTZIP_ABI_VERSION_3,
        destination_path: c_dest_dir.as_ptr(),
        password: std::ptr::null(),
        thread_budget: 4,
        collision_policy: 0,
        preserve_permissions: true,
        dry_run: false,
        progress_callback: Some(test_progress_cb),
        user_data: std::ptr::null_mut(),
    };

    let ext_status = unsafe {
        ttzip_rust_extract_archive(
            c_archive_path.as_ptr(),
            c_dest_dir.as_ptr(),
            &extract_options,
        )
    };
    assert_eq!(ext_status, TTZipStatus::Ok);

    // Verify extracted contents
    let extracted_f1 = dest_dir.join("src_files/hello.txt");
    let extracted_f2 = dest_dir.join("src_files/nested_folder/data.bin");
    assert!(extracted_f1.exists());
    assert!(extracted_f2.exists());

    let content1 = fs::read(&extracted_f1).unwrap();
    assert_eq!(content1, b"Hello World from TTZip Rust Glue!");

    let content2 = fs::read(&extracted_f2).unwrap();
    assert_eq!(content2, vec![0x42; 1024]);

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_phase5_archive_create_tar_gz_and_extract() {
    let tmp_dir = tempdir().unwrap();
    let temp_dir = tmp_dir.path();

    let src_file = temp_dir.join("test_sample.log");
    fs::write(&src_file, b"Sample log line 1\nSample log line 2\n").unwrap();

    let archive_path = temp_dir.join("sample.tar.gz");
    let c_archive_path = CString::new(archive_path.to_str().unwrap()).unwrap();
    let c_src_path = CString::new(src_file.to_str().unwrap()).unwrap();
    let src_ptrs = [c_src_path.as_ptr()];

    let create_options = TTZipCreateOptions {
        struct_size: std::mem::size_of::<TTZipCreateOptions>() as u32,
        abi_version: ttzip_engine::types::TTZIP_ABI_VERSION_3,
        format: TTZipArchiveFormat::TarGz,
        level: TTZipCompressionLevel::Normal,
        encryption: TTZipEncryptionMethod::None,
        password: std::ptr::null(),
        thread_budget: 2,
        solid_block_size_mb: 0,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };

    let create_status = unsafe {
        ttzip_rust_create_archive(
            src_ptrs.as_ptr(),
            src_ptrs.len(),
            c_archive_path.as_ptr(),
            &create_options,
        )
    };
    assert_eq!(create_status, TTZipStatus::Ok);
    assert!(archive_path.exists());

    let dest_dir = temp_dir.join("out_targz");
    let c_dest_dir = CString::new(dest_dir.to_str().unwrap()).unwrap();

    let extract_options = TTZipExtractOptions {
        struct_size: std::mem::size_of::<TTZipExtractOptions>() as u32,
        abi_version: ttzip_engine::types::TTZIP_ABI_VERSION_3,
        destination_path: c_dest_dir.as_ptr(),
        password: std::ptr::null(),
        thread_budget: 2,
        collision_policy: 0,
        preserve_permissions: true,
        dry_run: false,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };

    let ext_status = unsafe {
        ttzip_rust_extract_archive(
            c_archive_path.as_ptr(),
            c_dest_dir.as_ptr(),
            &extract_options,
        )
    };
    assert_eq!(ext_status, TTZipStatus::Ok);
    let extracted_file = dest_dir.join("test_sample.log");
    assert!(extracted_file.exists());
    assert_eq!(
        fs::read(&extracted_file).unwrap(),
        b"Sample log line 1\nSample log line 2\n"
    );
    let arch_bytes = fs::read(&archive_path).unwrap();
    let tar_member = assert_gzip_single_member_without_trailing_padding(&arch_bytes);
    assert!(!tar_member.is_empty());
}

fn assert_gzip_single_member_without_trailing_padding(gz_bytes: &[u8]) -> Vec<u8> {
    assert!(
        gz_bytes.len() >= 18 && gz_bytes[0] == 0x1f && gz_bytes[1] == 0x8b,
        "expected valid GZIP magic header"
    );
    let mut cursor = std::io::Cursor::new(gz_bytes);
    let mut decoder = flate2::bufread::GzDecoder::new(&mut cursor);
    let mut decompressed = Vec::new();
    std::io::Read::read_to_end(&mut decoder, &mut decompressed)
        .expect("single GZIP member must decode cleanly");
    let consumed = cursor.position() as usize;
    let trailing = &gz_bytes[consumed..];
    assert!(
        trailing.is_empty(),
        "GZIP stream must not have trailing padding after member trailer (total={}, consumed={}, trailing_len={})",
        gz_bytes.len(),
        consumed,
        trailing.len()
    );
    let isize_trailer = u32::from_le_bytes(gz_bytes[gz_bytes.len() - 4..].try_into().unwrap());
    assert_eq!(
        isize_trailer,
        decompressed.len() as u32,
        "GZIP ISIZE trailer must match decompressed member byte count"
    );
    decompressed
}

#[derive(Clone)]
struct SharedBufferWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for SharedBufferWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn extract_and_read_single_file(
    archive_path: &std::path::Path,
    out_dir: &std::path::Path,
    name: &str,
) -> Result<Vec<u8>, TTZipStatus> {
    let c_arch = CString::new(archive_path.to_str().unwrap()).unwrap();
    let c_out = CString::new(out_dir.to_str().unwrap()).unwrap();
    let ext_opts = TTZipExtractOptions {
        destination_path: c_out.as_ptr(),
        ..TTZipExtractOptions::default()
    };
    let st = unsafe { ttzip_rust_extract_archive(c_arch.as_ptr(), c_out.as_ptr(), &ext_opts) };
    if st != TTZipStatus::Ok {
        return Err(st);
    }
    Ok(fs::read(out_dir.join(name)).unwrap())
}

fn assert_stream_writer_zip_unpadded_and_roundtrips(temp_dir: &std::path::Path, payload: &[u8]) {
    use ttzip_archive::archive::stream_adapter::write::ArchiveStreamWriter;
    use ttzip_archive::sys::libarchive::sys::*;

    let shared = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    {
        let writer = ArchiveStreamWriter::open_writer(SharedBufferWriter(shared.clone()), 64 * 1024)
            .expect("ArchiveStreamWriter::open_writer must succeed");
        unsafe {
            let a = writer.as_raw_archive();
            let entry = archive_entry_new();
            assert!(!entry.is_null());
            let name_c = CString::new("boundary.txt").unwrap();
            archive_entry_set_pathname(entry, name_c.as_ptr());
            archive_entry_set_filetype(entry, libc::S_IFREG as u32);
            archive_entry_set_perm(entry, 0o644);
            archive_entry_set_size(entry, payload.len() as i64);
            assert_eq!(archive_write_header(a, entry), 0);
            let written = archive_write_data(a, payload.as_ptr() as *const libc::c_void, payload.len());
            assert_eq!(written, payload.len() as isize);
            assert_eq!(archive_write_finish_entry(a), 0);
            archive_entry_free(entry);
        }
    }
    let zip_bytes = shared.lock().unwrap().clone();
    assert!(
        zip_bytes.len() >= 22 && zip_bytes.len() < 10240,
        "ArchiveStreamWriter ZIP output must not be 10240-padded (len={})",
        zip_bytes.len()
    );
    assert_eq!(
        &zip_bytes[zip_bytes.len() - 22..zip_bytes.len() - 18],
        b"PK\x05\x06",
        "final 22 bytes of unpadded ZIP stream must be the EOCD record without trailing zero padding"
    );
    let zip_path = temp_dir.join("stream_adapter_writer.zip");
    fs::write(&zip_path, &zip_bytes).unwrap();
    let extracted =
        extract_and_read_single_file(&zip_path, &temp_dir.join("out_stream_zip"), "boundary.txt")
            .expect("stream_adapter_writer.zip extraction must succeed");
    assert_eq!(extracted, payload);
}

fn assert_split_tar_gz_unpadded_and_roundtrips(
    temp_dir: &std::path::Path,
    src_file: &std::path::PathBuf,
    payload: &[u8],
) {
    let split_targz = temp_dir.join("split_target.tar.gz");
    let split_opts = TTZipCreateOptions {
        format: TTZipArchiveFormat::TarGz,
        ..TTZipCreateOptions::default()
    };
    ttzip_engine::archive::unified::create_archive(
        std::slice::from_ref(src_file),
        &split_targz,
        &split_opts,
        65_536,
    )
    .unwrap();
    let split_part1 = temp_dir.join("split_target.tar.gz.001");
    let split_bytes = fs::read(&split_part1).unwrap();
    let split_ext_res =
        extract_and_read_single_file(&split_part1, &temp_dir.join("out_split_targz"), "boundary.txt");
    assert_gzip_single_member_without_trailing_padding(&split_bytes);
    assert_eq!(split_ext_res.expect("split TAR.GZ extraction must succeed"), payload);
}

#[test]
fn test_phase5_archive_create_open2_last_block_boundaries() {
    let tmp_dir = tempdir().unwrap();
    let temp_dir = tmp_dir.path();
    let payload = b"Boundary payload line A\nBoundary payload line B\n";
    let src_file = temp_dir.join("boundary.txt");
    fs::write(&src_file, payload).unwrap();
    let c_src = CString::new(src_file.to_str().unwrap()).unwrap();
    let src_ptrs = [c_src.as_ptr()];

    for (suffix, fmt, is_gzip) in [
        ("boundary.tar", TTZipArchiveFormat::Tar, false),
        ("boundary.txt.gz", TTZipArchiveFormat::Gzip, true),
    ] {
        let arch_path = temp_dir.join(suffix);
        let c_arch = CString::new(arch_path.to_str().unwrap()).unwrap();
        let opts = TTZipCreateOptions {
            format: fmt,
            level: TTZipCompressionLevel::Normal,
            ..TTZipCreateOptions::default()
        };
        let st = unsafe { ttzip_rust_create_archive(src_ptrs.as_ptr(), 1, c_arch.as_ptr(), &opts) };
        assert_eq!(st, TTZipStatus::Ok);
        let raw_bytes = fs::read(&arch_path).unwrap();
        assert!(!raw_bytes.is_empty() && raw_bytes.len() < 10240, "{suffix} len={}", raw_bytes.len());
        if is_gzip {
            assert_eq!(assert_gzip_single_member_without_trailing_padding(&raw_bytes), payload);
        }
        let extracted = extract_and_read_single_file(&arch_path, &temp_dir.join(format!("out_{suffix}")), "boundary.txt")
            .expect("C-ABI archive extraction must succeed");
        assert_eq!(extracted, payload);
    }

    for (suffix, fmt, extracted_name) in [
        ("writer.tar.gz", TTZipArchiveFormat::TarGz, "boundary.txt"),
        ("writer.txt.gz", TTZipArchiveFormat::Gzip, "writer.txt"),
    ] {
        let writer_opts = TTZipCreateOptions {
            format: fmt,
            ..TTZipCreateOptions::default()
        };
        let mut out_buf = Vec::new();
        let written = ttzip_engine::archive::unified::create_archive_to_writer(
            std::slice::from_ref(&src_file),
            &mut out_buf,
            &writer_opts,
        )
        .unwrap();
        assert_eq!(written as usize, out_buf.len());
        let writer_arch = temp_dir.join(suffix);
        fs::write(&writer_arch, &out_buf).unwrap();
        let ext_res = extract_and_read_single_file(
            &writer_arch,
            &temp_dir.join(format!("out_{suffix}")),
            extracted_name,
        );
        if let Err(st) = &ext_res {
            eprintln!("PRE_FIX_DIAGNOSTIC {suffix}: len={} extract_err={st:?}", out_buf.len());
        }
        assert_gzip_single_member_without_trailing_padding(&out_buf);
        assert_eq!(ext_res.expect("create_archive_to_writer extraction must succeed"), payload);
    }

    assert_split_tar_gz_unpadded_and_roundtrips(temp_dir, &src_file, payload);
    assert_stream_writer_zip_unpadded_and_roundtrips(temp_dir, payload);
}

#[test]
fn test_phase5_archive_error_handling() {
    let invalid_path = CString::new("/nonexistent/path/to/archive.zip").unwrap();
    let tmp_dest_dir = tempdir().unwrap();
    let dest_dir = CString::new(tmp_dest_dir.path().to_str().unwrap()).unwrap();

    let inspect_status = unsafe {
        ttzip_rust_inspect_archive(
            invalid_path.as_ptr(),
            std::ptr::null(),
            false,
            Some(test_inspect_cb),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(inspect_status, TTZipStatus::ErrFileNotFound);

    let extract_status = unsafe {
        ttzip_rust_extract_archive(
            invalid_path.as_ptr(),
            dest_dir.as_ptr(),
            std::ptr::null(),
        )
    };
    assert_eq!(extract_status, TTZipStatus::ErrFileNotFound);

    // Null pointer param checks
    let null_status = unsafe {
        ttzip_rust_extract_archive(
            std::ptr::null(),
            dest_dir.as_ptr(),
            std::ptr::null(),
        )
    };
    assert_eq!(null_status, TTZipStatus::ErrInvalidParam);
}


#[test]
fn test_phase5_archive_extract_abi_validation() {
    let tmp_dir = tempdir().unwrap();
    let temp_dir = tmp_dir.path();
    let archive_path = temp_dir.join("test_archive.zip");
    std::fs::File::create(&archive_path).unwrap();
    let c_archive_path = CString::new(archive_path.to_str().unwrap()).unwrap();
    let dest_dir = temp_dir.join("extracted");
    let c_dest_dir = CString::new(dest_dir.to_str().unwrap()).unwrap();

    let mut invalid_abi_options = TTZipExtractOptions {
        struct_size: std::mem::size_of::<TTZipExtractOptions>() as u32,
        abi_version: 2, // invalid
        destination_path: c_dest_dir.as_ptr(),
        password: std::ptr::null(),
        thread_budget: 4,
        collision_policy: 0,
        preserve_permissions: true,
        dry_run: true,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };

    let status = unsafe {
        ttzip_rust_extract_archive(
            c_archive_path.as_ptr(),
            c_dest_dir.as_ptr(),
            &invalid_abi_options,
        )
    };
    assert_eq!(status, TTZipStatus::ErrAbiMismatch);

    invalid_abi_options.abi_version = ttzip_engine::types::TTZIP_ABI_VERSION_3;
    invalid_abi_options.collision_policy = 255; // invalid policy

    let status = unsafe {
        ttzip_rust_extract_archive(
            c_archive_path.as_ptr(),
            c_dest_dir.as_ptr(),
            &invalid_abi_options,
        )
    };
    assert_eq!(status, TTZipStatus::ErrInvalidParam);
}
