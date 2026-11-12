// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Shared C-ABI v3 layout probe and cross-language enum discriminant parity helpers
//! for `cabi_extract_outcome_contract_tests`.

use std::fs;
use std::mem::{align_of, offset_of, size_of};
use std::path::{Path, PathBuf};

use ttzip_engine::platform::TTZipCpuCapsRaw;
use ttzip_engine::types::{
    TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEntryMetadata,
    TTZipErrorInfo, TTZipExecutionProvenance, TTZipExtractOptions, TTZipStatus,
    TTZIP_ABI_VERSION_3, TTZIP_COLLISION_POLICY_KEEP_BOTH, TTZIP_COLLISION_POLICY_REPLACE,
    TTZIP_COLLISION_POLICY_SKIP, TTZIP_COLLISION_POLICY_STOP,
};

pub const STATUS_BIJECTION: &[(&str, &str, TTZipStatus)] = &[
    ("TTZIP_STATUS_OK", "Ok", TTZipStatus::Ok),
    ("TTZIP_STATUS_EOF", "Eof", TTZipStatus::Eof),
    ("TTZIP_STATUS_CANCELLED", "Cancelled", TTZipStatus::Cancelled),
    ("TTZIP_STATUS_ERR_INVALID_PARAM", "ErrInvalidParam", TTZipStatus::ErrInvalidParam),
    ("TTZIP_STATUS_ERR_FILE_NOT_FOUND", "ErrFileNotFound", TTZipStatus::ErrFileNotFound),
    ("TTZIP_STATUS_ERR_MMAP_FAILED", "ErrMmapFailed", TTZipStatus::ErrMmapFailed),
    ("TTZIP_STATUS_ERR_CORRUPT_HEADER", "ErrCorruptHeader", TTZipStatus::ErrCorruptHeader),
    ("TTZIP_STATUS_ERR_INVALID_OFFSET", "ErrInvalidOffset", TTZipStatus::ErrInvalidOffset),
    (
        "TTZIP_STATUS_ERR_ARCHIVE_INIT_FAILED",
        "ErrArchiveInitFailed",
        TTZipStatus::ErrArchiveInitFailed,
    ),
    ("TTZIP_STATUS_ERR_OPEN_FAILED", "ErrOpenFailed", TTZipStatus::ErrOpenFailed),
    ("TTZIP_STATUS_ERR_PATH_TOO_LONG", "ErrPathTooLong", TTZipStatus::ErrPathTooLong),
    ("TTZIP_STATUS_ERR_OUT_OF_MEMORY", "ErrOutOfMemory", TTZipStatus::ErrOutOfMemory),
    ("TTZIP_STATUS_ERR_INVALID_PASSWORD", "ErrInvalidPassword", TTZipStatus::ErrInvalidPassword),
    (
        "TTZIP_STATUS_ERR_EXTRACTION_FAILED",
        "ErrExtractionFailed",
        TTZipStatus::ErrExtractionFailed,
    ),
    (
        "TTZIP_STATUS_ERR_COMPRESSION_FAILED",
        "ErrCompressionFailed",
        TTZipStatus::ErrCompressionFailed,
    ),
    (
        "TTZIP_STATUS_ERR_UNSUPPORTED_FEATURE",
        "ErrUnsupportedFeature",
        TTZipStatus::ErrUnsupportedFeature,
    ),
    ("TTZIP_STATUS_ERR_BUFFER_TOO_SMALL", "ErrBufferTooSmall", TTZipStatus::ErrBufferTooSmall),
    ("TTZIP_STATUS_ERR_IO", "ErrIo", TTZipStatus::ErrIo),
    (
        "TTZIP_STATUS_ERR_SOLID_BUDGET_EXCEEDED",
        "ErrSolidBudgetExceeded",
        TTZipStatus::ErrSolidBudgetExceeded,
    ),
    (
        "TTZIP_STATUS_ERR_SECURITY_VIOLATION",
        "ErrSecurityViolation",
        TTZipStatus::ErrSecurityViolation,
    ),
    ("TTZIP_STATUS_ERR_ABI_MISMATCH", "ErrAbiMismatch", TTZipStatus::ErrAbiMismatch),
    ("TTZIP_STATUS_ERR_PANIC_CAUGHT", "ErrPanicCaught", TTZipStatus::ErrPanicCaught),
];

pub const LEVEL_BIJECTION: &[(&str, &str, TTZipCompressionLevel)] = &[
    ("TTZIP_COMPRESSION_LEVEL_STORE", "Store", TTZipCompressionLevel::Store),
    ("TTZIP_COMPRESSION_LEVEL_FASTEST", "Fastest", TTZipCompressionLevel::Fastest),
    ("TTZIP_COMPRESSION_LEVEL_FAST", "Fast", TTZipCompressionLevel::Fast),
    ("TTZIP_COMPRESSION_LEVEL_NORMAL", "Normal", TTZipCompressionLevel::Normal),
    ("TTZIP_COMPRESSION_LEVEL_MAXIMUM", "Maximum", TTZipCompressionLevel::Maximum),
    ("TTZIP_COMPRESSION_LEVEL_ULTRA", "Ultra", TTZipCompressionLevel::Ultra),
];

pub const FORMAT_BIJECTION: &[(&str, &str, TTZipArchiveFormat)] = &[
    ("TTZIP_ARCHIVE_FORMAT_UNKNOWN", "Unknown", TTZipArchiveFormat::Unknown),
    ("TTZIP_ARCHIVE_FORMAT_SEVEN_ZIP", "SevenZip", TTZipArchiveFormat::SevenZip),
    ("TTZIP_ARCHIVE_FORMAT_ZIP", "Zip", TTZipArchiveFormat::Zip),
    ("TTZIP_ARCHIVE_FORMAT_ZIP64", "Zip64", TTZipArchiveFormat::Zip64),
    ("TTZIP_ARCHIVE_FORMAT_TAR", "Tar", TTZipArchiveFormat::Tar),
    ("TTZIP_ARCHIVE_FORMAT_RAR4", "Rar4", TTZipArchiveFormat::Rar4),
    ("TTZIP_ARCHIVE_FORMAT_RAR5", "Rar5", TTZipArchiveFormat::Rar5),
    ("TTZIP_ARCHIVE_FORMAT_CAB", "Cab", TTZipArchiveFormat::Cab),
    ("TTZIP_ARCHIVE_FORMAT_WIM", "Wim", TTZipArchiveFormat::Wim),
    ("TTZIP_ARCHIVE_FORMAT_ISO", "Iso", TTZipArchiveFormat::Iso),
    ("TTZIP_ARCHIVE_FORMAT_UDF", "Udf", TTZipArchiveFormat::Udf),
    ("TTZIP_ARCHIVE_FORMAT_DMG", "Dmg", TTZipArchiveFormat::Dmg),
    ("TTZIP_ARCHIVE_FORMAT_ARJ", "Arj", TTZipArchiveFormat::Arj),
    ("TTZIP_ARCHIVE_FORMAT_LZH", "Lzh", TTZipArchiveFormat::Lzh),
    ("TTZIP_ARCHIVE_FORMAT_CPIO", "Cpio", TTZipArchiveFormat::Cpio),
    ("TTZIP_ARCHIVE_FORMAT_CHM", "Chm", TTZipArchiveFormat::Chm),
    ("TTZIP_ARCHIVE_FORMAT_NSIS", "Nsis", TTZipArchiveFormat::Nsis),
    ("TTZIP_ARCHIVE_FORMAT_MSI", "Msi", TTZipArchiveFormat::Msi),
    ("TTZIP_ARCHIVE_FORMAT_GZIP", "Gzip", TTZipArchiveFormat::Gzip),
    ("TTZIP_ARCHIVE_FORMAT_BZIP2", "Bzip2", TTZipArchiveFormat::Bzip2),
    ("TTZIP_ARCHIVE_FORMAT_XZ", "Xz", TTZipArchiveFormat::Xz),
    ("TTZIP_ARCHIVE_FORMAT_ZSTD", "Zstd", TTZipArchiveFormat::Zstd),
    ("TTZIP_ARCHIVE_FORMAT_LZ4", "Lz4", TTZipArchiveFormat::Lz4),
    ("TTZIP_ARCHIVE_FORMAT_SNAPPY", "Snappy", TTZipArchiveFormat::Snappy),
    ("TTZIP_ARCHIVE_FORMAT_LZFSE", "Lzfse", TTZipArchiveFormat::Lzfse),
    ("TTZIP_ARCHIVE_FORMAT_BROTLI", "Brotli", TTZipArchiveFormat::Brotli),
    ("TTZIP_ARCHIVE_FORMAT_LZIP", "Lzip", TTZipArchiveFormat::Lzip),
    ("TTZIP_ARCHIVE_FORMAT_LRZIP", "Lrzip", TTZipArchiveFormat::Lrzip),
    ("TTZIP_ARCHIVE_FORMAT_MSLZ", "MsLz", TTZipArchiveFormat::MsLz),
    ("TTZIP_ARCHIVE_FORMAT_APFS", "Apfs", TTZipArchiveFormat::Apfs),
    ("TTZIP_ARCHIVE_FORMAT_HFS_PLUS", "HfsPlus", TTZipArchiveFormat::HfsPlus),
    ("TTZIP_ARCHIVE_FORMAT_VHD", "Vhd", TTZipArchiveFormat::Vhd),
    ("TTZIP_ARCHIVE_FORMAT_VHDX", "Vhdx", TTZipArchiveFormat::Vhdx),
    ("TTZIP_ARCHIVE_FORMAT_VMDK", "Vmdk", TTZipArchiveFormat::Vmdk),
    ("TTZIP_ARCHIVE_FORMAT_QCOW2", "Qcow2", TTZipArchiveFormat::Qcow2),
    ("TTZIP_ARCHIVE_FORMAT_EXT2", "Ext2", TTZipArchiveFormat::Ext2),
    ("TTZIP_ARCHIVE_FORMAT_EXT3", "Ext3", TTZipArchiveFormat::Ext3),
    ("TTZIP_ARCHIVE_FORMAT_EXT4", "Ext4", TTZipArchiveFormat::Ext4),
    ("TTZIP_ARCHIVE_FORMAT_FAT", "Fat", TTZipArchiveFormat::Fat),
    ("TTZIP_ARCHIVE_FORMAT_NTFS", "Ntfs", TTZipArchiveFormat::Ntfs),
    ("TTZIP_ARCHIVE_FORMAT_SQUASHFS", "Squashfs", TTZipArchiveFormat::Squashfs),
    ("TTZIP_ARCHIVE_FORMAT_CRAMFS", "Cramfs", TTZipArchiveFormat::Cramfs),
    ("TTZIP_ARCHIVE_FORMAT_AAR", "Aar", TTZipArchiveFormat::Aar),
    ("TTZIP_ARCHIVE_FORMAT_DEB", "Deb", TTZipArchiveFormat::Deb),
    ("TTZIP_ARCHIVE_FORMAT_RPM", "Rpm", TTZipArchiveFormat::Rpm),
    ("TTZIP_ARCHIVE_FORMAT_XAR", "Xar", TTZipArchiveFormat::Xar),
    ("TTZIP_ARCHIVE_FORMAT_AR", "Ar", TTZipArchiveFormat::Ar),
    ("TTZIP_ARCHIVE_FORMAT_PE_EXE", "PeExe", TTZipArchiveFormat::PeExe),
    ("TTZIP_ARCHIVE_FORMAT_ELF", "Elf", TTZipArchiveFormat::Elf),
    ("TTZIP_ARCHIVE_FORMAT_MACHO", "MachO", TTZipArchiveFormat::MachO),
    ("TTZIP_ARCHIVE_FORMAT_SWF", "Swf", TTZipArchiveFormat::Swf),
    ("TTZIP_ARCHIVE_FORMAT_FLAC", "Flac", TTZipArchiveFormat::Flac),
    ("TTZIP_ARCHIVE_FORMAT_DCX", "Dcx", TTZipArchiveFormat::Dcx),
    ("TTZIP_ARCHIVE_FORMAT_ARC", "Arc", TTZipArchiveFormat::Arc),
    ("TTZIP_ARCHIVE_FORMAT_ZOO", "Zoo", TTZipArchiveFormat::Zoo),
    ("TTZIP_ARCHIVE_FORMAT_PAK", "Pak", TTZipArchiveFormat::Pak),
    ("TTZIP_ARCHIVE_FORMAT_MUB", "Mub", TTZipArchiveFormat::Mub),
    ("TTZIP_ARCHIVE_FORMAT_WARC", "Warc", TTZipArchiveFormat::Warc),
    ("TTZIP_ARCHIVE_FORMAT_MTREE", "Mtree", TTZipArchiveFormat::Mtree),
    ("TTZIP_ARCHIVE_FORMAT_RAW", "Raw", TTZipArchiveFormat::Raw),
    ("TTZIP_ARCHIVE_FORMAT_EMPTY", "Empty", TTZipArchiveFormat::Empty),
    ("TTZIP_ARCHIVE_FORMAT_AUTO", "Auto", TTZipArchiveFormat::Auto),
    ("TTZIP_ARCHIVE_FORMAT_RAR", "Rar", TTZipArchiveFormat::Rar),
    ("TTZIP_ARCHIVE_FORMAT_TAR_GZ", "TarGz", TTZipArchiveFormat::TarGz),
    ("TTZIP_ARCHIVE_FORMAT_TAR_BZ2", "TarBz2", TTZipArchiveFormat::TarBz2),
    ("TTZIP_ARCHIVE_FORMAT_TAR_XZ", "TarXz", TTZipArchiveFormat::TarXz),
    ("TTZIP_ARCHIVE_FORMAT_TAR_ZSTD", "TarZstd", TTZipArchiveFormat::TarZstd),
    ("TTZIP_ARCHIVE_FORMAT_TAR_LZ4", "TarLz4", TTZipArchiveFormat::TarLz4),
    ("TTZIP_ARCHIVE_FORMAT_TAR_BROTLI", "TarBrotli", TTZipArchiveFormat::TarBrotli),
    ("TTZIP_ARCHIVE_FORMAT_TAR_LZIP", "TarLzip", TTZipArchiveFormat::TarLzip),
    ("TTZIP_ARCHIVE_FORMAT_TAR_LRZIP", "TarLrzip", TTZipArchiveFormat::TarLrzip),
];

pub fn sdk_root_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("core/rust")
        .parent()
        .expect("core")
        .join("sdk")
}

pub fn resolve_monorepo_bindings_dir(manifest_dir: &Path) -> Option<PathBuf> {
    let sdk = manifest_dir.parent()?.parent()?.join("sdk");
    if let Some(product_dir) = sdk.parent().and_then(Path::parent) {
        if product_dir.join("bindings/cpp/ttzip_engine.hpp").is_file()
            && product_dir.join("bindings/csharp/TTZipEngine.cs").is_file()
        {
            return Some(product_dir.to_path_buf());
        }
    }
    let canon_hdr = fs::canonicalize(sdk.join("include/ttzip.h")).ok()?;
    let product_dir = canon_hdr.parent()?.parent()?.parent()?.parent()?;
    if product_dir.join("bindings/cpp/ttzip_engine.hpp").is_file()
        && product_dir.join("bindings/csharp/TTZipEngine.cs").is_file()
    {
        Some(product_dir.to_path_buf())
    } else {
        None
    }
}

pub fn build_c_abi_probe_source() -> String {
    let status_names: Vec<&str> = STATUS_BIJECTION.iter().map(|(c, _, _)| *c).collect();
    let status_fmt = vec!["%d"; status_names.len()].join(",");
    let level_names: Vec<&str> = LEVEL_BIJECTION.iter().map(|(c, _, _)| *c).collect();
    let level_fmt = vec!["%d"; level_names.len()].join(",");
    let format_names: Vec<&str> = FORMAT_BIJECTION.iter().map(|(c, _, _)| *c).collect();
    let format_fmt = vec!["%d"; format_names.len()].join(",");
    format!(
        r#"#include "ttzip.h"
#include <stddef.h>
#include <stdio.h>
int main(void) {{
    TTZipErrorInfo err; ttzip_error_info_init(&err);
    TTZipExtractOptions opt; ttzip_extract_options_init(&opt);
    TTZipCreateOptions create_opt; ttzip_create_options_init(&create_opt);
    printf("abi=%u,%u,%u,%u,%u,%u\n", (unsigned)TTZIP_ABI_VERSION_3, (unsigned)TTZIP_ABI_VERSION,
        (unsigned)TTZIP_COLLISION_POLICY_KEEP_BOTH, (unsigned)TTZIP_COLLISION_POLICY_REPLACE,
        (unsigned)TTZIP_COLLISION_POLICY_SKIP, (unsigned)TTZIP_COLLISION_POLICY_STOP);
    printf("err=%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%u,%u,%d\n",
        sizeof(TTZipErrorInfo), _Alignof(TTZipErrorInfo), offsetof(TTZipErrorInfo, struct_size),
        offsetof(TTZipErrorInfo, abi_version), offsetof(TTZipErrorInfo, status), offsetof(TTZipErrorInfo, error_code),
        offsetof(TTZipErrorInfo, message), offsetof(TTZipErrorInfo, entry_path), offsetof(TTZipErrorInfo, offset),
        (unsigned)err.struct_size, (unsigned)err.abi_version, (int)err.status);
    printf("opt=%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%u,%u,%u,%d,%d\n",
        sizeof(TTZipExtractOptions), _Alignof(TTZipExtractOptions), offsetof(TTZipExtractOptions, struct_size),
        offsetof(TTZipExtractOptions, abi_version), offsetof(TTZipExtractOptions, destination_path),
        offsetof(TTZipExtractOptions, password), offsetof(TTZipExtractOptions, thread_budget),
        offsetof(TTZipExtractOptions, collision_policy), offsetof(TTZipExtractOptions, preserve_permissions),
        offsetof(TTZipExtractOptions, dry_run), offsetof(TTZipExtractOptions, progress_callback),
        offsetof(TTZipExtractOptions, user_data), (unsigned)opt.struct_size, (unsigned)opt.abi_version,
        (unsigned)opt.collision_policy, (int)opt.preserve_permissions, (int)opt.dry_run);
    printf("create=%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%u,%u,%d,%d,%d,%u\n",
        sizeof(TTZipCreateOptions), _Alignof(TTZipCreateOptions), offsetof(TTZipCreateOptions, struct_size),
        offsetof(TTZipCreateOptions, abi_version), offsetof(TTZipCreateOptions, format),
        offsetof(TTZipCreateOptions, level), offsetof(TTZipCreateOptions, encryption),
        offsetof(TTZipCreateOptions, password), offsetof(TTZipCreateOptions, thread_budget),
        offsetof(TTZipCreateOptions, solid_block_size_mb), offsetof(TTZipCreateOptions, progress_callback),
        offsetof(TTZipCreateOptions, user_data), (unsigned)create_opt.struct_size, (unsigned)create_opt.abi_version,
        (int)create_opt.format, (int)create_opt.level, (int)create_opt.encryption, (unsigned)create_opt.solid_block_size_mb);
    printf("meta=%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu\n",
        sizeof(TTZipEntryMetadata), _Alignof(TTZipEntryMetadata), offsetof(TTZipEntryMetadata, struct_size),
        offsetof(TTZipEntryMetadata, abi_version), offsetof(TTZipEntryMetadata, path),
        offsetof(TTZipEntryMetadata, uncompressed_size), offsetof(TTZipEntryMetadata, compressed_size),
        offsetof(TTZipEntryMetadata, crc32), offsetof(TTZipEntryMetadata, mtime_epoch_secs),
        offsetof(TTZipEntryMetadata, mode), offsetof(TTZipEntryMetadata, is_directory),
        offsetof(TTZipEntryMetadata, is_encrypted), offsetof(TTZipEntryMetadata, compression_method),
        offsetof(TTZipEntryMetadata, detected_encoding));
    printf("prov=%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu\n",
        sizeof(TTZipExecutionProvenance), _Alignof(TTZipExecutionProvenance),
        offsetof(TTZipExecutionProvenance, struct_size), offsetof(TTZipExecutionProvenance, abi_version),
        offsetof(TTZipExecutionProvenance, engine_tag), offsetof(TTZipExecutionProvenance, thread_count),
        offsetof(TTZipExecutionProvenance, uncompressed_bytes), offsetof(TTZipExecutionProvenance, compressed_bytes),
        offsetof(TTZipExecutionProvenance, kernel_duration_nanos), offsetof(TTZipExecutionProvenance, is_fallback),
        offsetof(TTZipExecutionProvenance, fallback_reason));
    printf("cpu=%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu,%zu\n",
        sizeof(TTZipCpuCapsRaw), _Alignof(TTZipCpuCapsRaw), offsetof(TTZipCpuCapsRaw, struct_size),
        offsetof(TTZipCpuCapsRaw, abi_version), offsetof(TTZipCpuCapsRaw, logical_cores),
        offsetof(TTZipCpuCapsRaw, physical_page_size), offsetof(TTZipCpuCapsRaw, p_cores),
        offsetof(TTZipCpuCapsRaw, e_cores), offsetof(TTZipCpuCapsRaw, has_arm_neon),
        offsetof(TTZipCpuCapsRaw, has_arm_crypto), offsetof(TTZipCpuCapsRaw, has_aes_ni),
        offsetof(TTZipCpuCapsRaw, has_avx2), offsetof(TTZipCpuCapsRaw, has_avx512),
        offsetof(TTZipCpuCapsRaw, has_hardware_crc32));
    printf("status={status_fmt}\n", {status_args});
    printf("level={level_fmt}\n", {level_args});
    printf("format={format_fmt}\n", {format_args});
    return 0;
}}
"#,
        status_args = status_names.iter().map(|n| format!("(int){n}")).collect::<Vec<_>>().join(","),
        level_args = level_names.iter().map(|n| format!("(int){n}")).collect::<Vec<_>>().join(","),
        format_args = format_names.iter().map(|n| format!("(int){n}")).collect::<Vec<_>>().join(","),
    )
}

pub fn expected_rust_abi_probe_lines() -> Vec<String> {
    vec![
        format!(
            "abi={},{},{},{},{},{}",
            TTZIP_ABI_VERSION_3,
            TTZIP_ABI_VERSION_3,
            TTZIP_COLLISION_POLICY_KEEP_BOTH,
            TTZIP_COLLISION_POLICY_REPLACE,
            TTZIP_COLLISION_POLICY_SKIP,
            TTZIP_COLLISION_POLICY_STOP,
        ),
        expected_rust_error_info_probe_line(),
        expected_rust_extract_options_probe_line(),
        expected_rust_create_options_probe_line(),
        expected_rust_metadata_probe_line(),
        expected_rust_provenance_probe_line(),
        expected_rust_cpu_probe_line(),
        format!(
            "status={}",
            STATUS_BIJECTION.iter().map(|(_, _, v)| (*v as i32).to_string()).collect::<Vec<_>>().join(","),
        ),
        format!(
            "level={}",
            LEVEL_BIJECTION.iter().map(|(_, _, v)| (*v as i32).to_string()).collect::<Vec<_>>().join(","),
        ),
        format!(
            "format={}",
            FORMAT_BIJECTION.iter().map(|(_, _, v)| (*v as i32).to_string()).collect::<Vec<_>>().join(","),
        ),
    ]
}

fn expected_rust_error_info_probe_line() -> String {
    let rust_err = TTZipErrorInfo::empty();
    format!(
        "err={},{},{},{},{},{},{},{},{},{},{},{}",
        size_of::<TTZipErrorInfo>(),
        align_of::<TTZipErrorInfo>(),
        offset_of!(TTZipErrorInfo, struct_size),
        offset_of!(TTZipErrorInfo, abi_version),
        offset_of!(TTZipErrorInfo, status),
        offset_of!(TTZipErrorInfo, error_code),
        offset_of!(TTZipErrorInfo, message),
        offset_of!(TTZipErrorInfo, entry_path),
        offset_of!(TTZipErrorInfo, offset),
        rust_err.struct_size,
        rust_err.abi_version,
        rust_err.status as i32,
    )
}

fn expected_rust_extract_options_probe_line() -> String {
    let rust_opt = TTZipExtractOptions::default();
    format!(
        "opt={},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        size_of::<TTZipExtractOptions>(),
        align_of::<TTZipExtractOptions>(),
        offset_of!(TTZipExtractOptions, struct_size),
        offset_of!(TTZipExtractOptions, abi_version),
        offset_of!(TTZipExtractOptions, destination_path),
        offset_of!(TTZipExtractOptions, password),
        offset_of!(TTZipExtractOptions, thread_budget),
        offset_of!(TTZipExtractOptions, collision_policy),
        offset_of!(TTZipExtractOptions, preserve_permissions),
        offset_of!(TTZipExtractOptions, dry_run),
        offset_of!(TTZipExtractOptions, progress_callback),
        offset_of!(TTZipExtractOptions, user_data),
        rust_opt.struct_size,
        rust_opt.abi_version,
        rust_opt.collision_policy,
        i32::from(rust_opt.preserve_permissions),
        i32::from(rust_opt.dry_run),
    )
}

fn expected_rust_create_options_probe_line() -> String {
    let rust_create = TTZipCreateOptions::default();
    format!(
        "create={},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        size_of::<TTZipCreateOptions>(),
        align_of::<TTZipCreateOptions>(),
        offset_of!(TTZipCreateOptions, struct_size),
        offset_of!(TTZipCreateOptions, abi_version),
        offset_of!(TTZipCreateOptions, format),
        offset_of!(TTZipCreateOptions, level),
        offset_of!(TTZipCreateOptions, encryption),
        offset_of!(TTZipCreateOptions, password),
        offset_of!(TTZipCreateOptions, thread_budget),
        offset_of!(TTZipCreateOptions, solid_block_size_mb),
        offset_of!(TTZipCreateOptions, progress_callback),
        offset_of!(TTZipCreateOptions, user_data),
        rust_create.struct_size,
        rust_create.abi_version,
        rust_create.format as i32,
        rust_create.level as i32,
        rust_create.encryption as i32,
        rust_create.solid_block_size_mb,
    )
}

fn expected_rust_metadata_probe_line() -> String {
    format!(
        "meta={},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        size_of::<TTZipEntryMetadata>(),
        align_of::<TTZipEntryMetadata>(),
        offset_of!(TTZipEntryMetadata, struct_size),
        offset_of!(TTZipEntryMetadata, abi_version),
        offset_of!(TTZipEntryMetadata, path),
        offset_of!(TTZipEntryMetadata, uncompressed_size),
        offset_of!(TTZipEntryMetadata, compressed_size),
        offset_of!(TTZipEntryMetadata, crc32),
        offset_of!(TTZipEntryMetadata, mtime_epoch_secs),
        offset_of!(TTZipEntryMetadata, mode),
        offset_of!(TTZipEntryMetadata, is_directory),
        offset_of!(TTZipEntryMetadata, is_encrypted),
        offset_of!(TTZipEntryMetadata, compression_method),
        offset_of!(TTZipEntryMetadata, detected_encoding),
    )
}

fn expected_rust_provenance_probe_line() -> String {
    format!(
        "prov={},{},{},{},{},{},{},{},{},{},{}",
        size_of::<TTZipExecutionProvenance>(),
        align_of::<TTZipExecutionProvenance>(),
        offset_of!(TTZipExecutionProvenance, struct_size),
        offset_of!(TTZipExecutionProvenance, abi_version),
        offset_of!(TTZipExecutionProvenance, engine_tag),
        offset_of!(TTZipExecutionProvenance, thread_count),
        offset_of!(TTZipExecutionProvenance, uncompressed_bytes),
        offset_of!(TTZipExecutionProvenance, compressed_bytes),
        offset_of!(TTZipExecutionProvenance, kernel_duration_nanos),
        offset_of!(TTZipExecutionProvenance, is_fallback),
        offset_of!(TTZipExecutionProvenance, fallback_reason),
    )
}

fn expected_rust_cpu_probe_line() -> String {
    format!(
        "cpu={},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        size_of::<TTZipCpuCapsRaw>(),
        align_of::<TTZipCpuCapsRaw>(),
        offset_of!(TTZipCpuCapsRaw, struct_size),
        offset_of!(TTZipCpuCapsRaw, abi_version),
        offset_of!(TTZipCpuCapsRaw, logical_cores),
        offset_of!(TTZipCpuCapsRaw, physical_page_size),
        offset_of!(TTZipCpuCapsRaw, p_cores),
        offset_of!(TTZipCpuCapsRaw, e_cores),
        offset_of!(TTZipCpuCapsRaw, has_arm_neon),
        offset_of!(TTZipCpuCapsRaw, has_arm_crypto),
        offset_of!(TTZipCpuCapsRaw, has_aes_ni),
        offset_of!(TTZipCpuCapsRaw, has_avx2),
        offset_of!(TTZipCpuCapsRaw, has_avx512),
        offset_of!(TTZipCpuCapsRaw, has_hardware_crc32),
    )
}

fn parse_enum_entries(source: &str, header_marker: &str) -> Vec<(String, i32)> {
    let start = source
        .find(header_marker)
        .unwrap_or_else(|| panic!("missing enum {header_marker}"));
    let after_header = &source[start + header_marker.len()..];
    let open_brace = after_header.find('{').expect("missing enum '{'");
    let body_region = &after_header[open_brace + 1..];
    let close_brace = body_region.find('}').expect("missing enum '}'");
    body_region[..close_brace]
        .lines()
        .filter_map(|line| {
            let trimmed = line.split("//").next().unwrap_or("").trim().trim_end_matches(',');
            let (name, val) = trimmed.split_once('=')?;
            Some((
                name.trim().to_string(),
                val.trim().parse::<i32>().expect("integer discriminant"),
            ))
        })
        .collect()
}

pub fn assert_cross_language_enum_parity_for_manifest(manifest_dir: &Path) -> bool {
    let c_hdr_path = manifest_dir
        .parent()
        .expect("core/rust")
        .parent()
        .expect("core")
        .join("sdk/include/ttzip.h");
    let c_hdr = fs::read_to_string(&c_hdr_path).expect("read sdk/include/ttzip.h");

    let expected_c_status: Vec<(String, i32)> = STATUS_BIJECTION
        .iter()
        .map(|(c, _, v)| ((*c).to_string(), *v as i32))
        .collect();
    let expected_c_level: Vec<(String, i32)> = LEVEL_BIJECTION
        .iter()
        .map(|(c, _, v)| ((*c).to_string(), *v as i32))
        .collect();
    let expected_c_format: Vec<(String, i32)> = FORMAT_BIJECTION
        .iter()
        .map(|(c, _, v)| ((*c).to_string(), *v as i32))
        .collect();

    assert_eq!(parse_enum_entries(&c_hdr, "typedef enum TTZipStatus"), expected_c_status);
    assert_eq!(parse_enum_entries(&c_hdr, "typedef enum TTZipCompressionLevel"), expected_c_level);
    assert_eq!(parse_enum_entries(&c_hdr, "typedef enum TTZipArchiveFormat"), expected_c_format);

    let Some(product_dir) = resolve_monorepo_bindings_dir(manifest_dir) else {
        return false;
    };
    let cpp_hdr = fs::read_to_string(product_dir.join("bindings/cpp/ttzip_engine.hpp"))
        .expect("read ttzip_engine.hpp");
    let cs_src = fs::read_to_string(product_dir.join("bindings/csharp/TTZipEngine.cs"))
        .expect("read TTZipEngine.cs");

    let expected_host_status: Vec<(String, i32)> = STATUS_BIJECTION
        .iter()
        .map(|(_, h, v)| ((*h).to_string(), *v as i32))
        .collect();
    let expected_host_level: Vec<(String, i32)> = LEVEL_BIJECTION
        .iter()
        .map(|(_, h, v)| ((*h).to_string(), *v as i32))
        .collect();
    let expected_host_format: Vec<(String, i32)> = FORMAT_BIJECTION
        .iter()
        .map(|(_, h, v)| ((*h).to_string(), *v as i32))
        .collect();

    assert_eq!(
        parse_enum_entries(&cpp_hdr, "enum class TTZipStatus : int32_t"),
        expected_host_status
    );
    assert_eq!(
        parse_enum_entries(&cs_src, "public enum TTZipStatus : int"),
        expected_host_status
    );
    assert_eq!(
        parse_enum_entries(&cpp_hdr, "enum class CompressionLevel : int32_t"),
        expected_host_level
    );
    assert_eq!(
        parse_enum_entries(&cs_src, "public enum CompressionLevel : int"),
        expected_host_level
    );
    assert_eq!(
        parse_enum_entries(&cpp_hdr, "enum class ArchiveFormat : int32_t"),
        expected_host_format
    );
    assert_eq!(
        parse_enum_entries(&cs_src, "public enum ArchiveFormat : int"),
        expected_host_format
    );
    true
}
