// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.
// Canonical C11 Header.

#ifndef TTZIP_H
#define TTZIP_H

#include "ttzip_rust_glue.h"

#ifdef __cplusplus
extern "C" {
#endif

static inline const char *ttzip_version(void) {
    return ttzip_rust_version();
}

static inline bool ttzip_is_hardware_accelerated(void) {
    return ttzip_rust_is_hardware_accelerated();
}

static inline uint32_t ttzip_crc32(const uint8_t *data, size_t len) {
    return ttzip_rust_crc32(0, data, len);
}

static inline uint64_t ttzip_crc64(const uint8_t *data, size_t len) {
    return ttzip_rust_crc64(0, data, len);
}

static inline TTZipStatus ttzip_create_archive(
    const char *const *source_paths,
    size_t source_count,
    const char *destination_path,
    const TTZipCreateOptions *options
) {
    TTZipCreateOptions opt_copy;
    if (options) {
        opt_copy = *options;
    } else {
        memset(&opt_copy, 0, sizeof(opt_copy));
    }
    if (opt_copy.struct_size == 0) {
        opt_copy.struct_size = sizeof(TTZipCreateOptions);
    }
    if (opt_copy.abi_version == 0) {
        opt_copy.abi_version = 2;
    }
    return ttzip_rust_create_archive(source_paths, source_count, destination_path, &opt_copy);
}

static inline TTZipStatus ttzip_extract_archive(
    const char *archive_path,
    const char *destination_path,
    const TTZipExtractOptions *options
) {
    TTZipExtractOptions opt_copy;
    if (options) {
        opt_copy = *options;
    } else {
        memset(&opt_copy, 0, sizeof(opt_copy));
    }
    if (opt_copy.struct_size == 0) {
        opt_copy.struct_size = sizeof(TTZipExtractOptions);
    }
    if (opt_copy.abi_version == 0) {
        opt_copy.abi_version = 2;
    }
    if (destination_path) {
        opt_copy.destination_path = destination_path;
    }
    return ttzip_rust_extract_archive(archive_path, destination_path, &opt_copy);
}

static inline TTZipStatus ttzip_inspect_archive(
    const char *archive_path,
    const char *password,
    bool detect_encoding,
    TTZipInspectCallback callback,
    void *user_data
) {
    return ttzip_rust_inspect_archive(archive_path, password, detect_encoding, callback, user_data);
}

static inline const char *ttzip_status_string(TTZipStatus status) {
    switch (status) {
        case TTZIP_STATUS_OK: return "OK";
        case TTZIP_STATUS_EOF: return "End of File";
        case TTZIP_STATUS_CANCELLED: return "Cancelled";
        case TTZIP_STATUS_ERR_INVALID_PARAM: return "Invalid Parameter";
        case TTZIP_STATUS_ERR_FILE_NOT_FOUND: return "File Not Found";
        case TTZIP_STATUS_ERR_MMAP_FAILED: return "Memory Map Failed";
        case TTZIP_STATUS_ERR_CORRUPT_HEADER: return "Corrupt Header";
        case TTZIP_STATUS_ERR_INVALID_OFFSET: return "Invalid Offset";
        case TTZIP_STATUS_ERR_ARCHIVE_INIT_FAILED: return "Archive Init Failed";
        case TTZIP_STATUS_ERR_OPEN_FAILED: return "Open Failed";
        case TTZIP_STATUS_ERR_PATH_TOO_LONG: return "Path Too Long";
        case TTZIP_STATUS_ERR_OUT_OF_MEMORY: return "Out of Memory";
        case TTZIP_STATUS_ERR_INVALID_PASSWORD: return "Invalid Password / Authentication Failed";
        case TTZIP_STATUS_ERR_EXTRACTION_FAILED: return "Extraction Failed";
        case TTZIP_STATUS_ERR_COMPRESSION_FAILED: return "Compression Failed";
        case TTZIP_STATUS_ERR_SECURITY_VIOLATION: return "Security Violation";
        case TTZIP_STATUS_ERR_PANIC_CAUGHT: return "Panic Caught";
        default: return "Unknown Error";
    }
}

#ifdef __cplusplus
}
#endif

#endif // TTZIP_H
