// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

#ifndef CTTZipBridge_h
#define CTTZipBridge_h

#include <stdbool.h>
#include <stdint.h>
#include "ttzip_engineFFI.h"

#ifdef __cplusplus
extern "C" {
#endif

/// Clones a file using APFS Copy-on-Write (CoW) or fallback physical copy via Rust microkernel.
int ttzip_rust_apfs_clone_file(const char * _Nonnull src, const char * _Nonnull dst, bool overwrite);

/// Clones file descriptor range via APFS fcopyfile in Rust microkernel.
int ttzip_rust_apfs_clone_range(int in_fd, int out_fd);

/// Preallocates contiguous physical extent space on APFS in Rust microkernel.
int ttzip_rust_apfs_preallocate(int fd, int64_t target_size);

/// Returns true if the path points to a macOS junk metadata artifact.
bool ttzip_rust_is_mac_junk(const char * _Nonnull path);

/// Fast file or directory removal via Rust microkernel.
int ttzip_rust_remove_path_fast(const char * _Nonnull path);

/// C-ABI structure for path sanitization and audit results.
typedef struct {
    uint32_t struct_size;
    uint32_t abi_version;
    char normalized_path[4096];
    char win32_formatted_path[4096];
    char stripped_ads[1024];
    bool has_traversal_attack;
    bool is_absolute;
    bool is_unc;
    bool is_long_path;
    bool is_windows_reserved;
    bool has_stripped_ads;
} TTZipPathSanitizationResult;

/// Sanitizes, normalizes, and audits a path against ZipSlip traversal, Win32 reserved device names,
/// NTFS Alternate Data Streams (ADS), and canonical NFC encoding via Rust microkernel.
int ttzip_rust_sanitize_path(const char * _Nonnull raw_path, TTZipPathSanitizationResult * _Nonnull out_result);

#ifdef __cplusplus
}
#endif

#endif /* CTTZipBridge_h */
