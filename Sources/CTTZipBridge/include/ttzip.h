// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.
// Minimal Public C11 SDK Header Interface.

#ifndef TTZIP_H
#define TTZIP_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#if defined(_WIN32) || defined(__CYGWIN__)
  #ifdef TTZIP_BUILD_SHARED
    #define TTZIP_API __declspec(dllexport)
  #else
    #define TTZIP_API __declspec(dllimport)
  #endif
#else
  #define TTZIP_API __attribute__((visibility("default")))
#endif

// Import foundational types from bridge
#include "ttzip_rust_glue.h"

/* ========================================================================== */
/* Public High-Level Archive APIs                                             */
/* ========================================================================== */

/**
 * Creates an archive from a list of source file/directory paths.
 */
static inline TTZipStatus ttzip_create_archive(
    const char *const *source_paths,
    size_t source_count,
    const char *destination_path,
    const TTZipCreateOptions *options
) {
    return ttzip_rust_create_archive(source_paths, source_count, destination_path, options);
}

/**
 * Extracts an archive to a destination directory.
 */
static inline TTZipStatus ttzip_extract_archive(
    const char *archive_path,
    const char *destination_path,
    const TTZipExtractOptions *options
) {
    return ttzip_rust_extract_archive(archive_path, destination_path, options);
}

/**
 * Inspects archive entries without extraction.
 */
static inline TTZipStatus ttzip_inspect_archive(
    const char *archive_path,
    const char *password,
    bool detect_encoding,
    TTZipInspectCallback callback,
    void *user_data
) {
    return ttzip_rust_inspect_archive(archive_path, password, detect_encoding, callback, user_data);
}

/* ========================================================================== */
/* In-Memory Buffer Compression & Decompression                               */
/* ========================================================================== */

/**
 * Decompresses an in-memory Deflate buffer.
 */
static inline TTZipStatus ttzip_deflate_decompress(
    const uint8_t *src,
    size_t src_len,
    uint8_t *dst,
    size_t dst_capacity,
    size_t *out_len
) {
    return ttzip_rust_deflate_decompress(src, src_len, dst, dst_capacity, out_len);
}

/**
 * Compresses an in-memory buffer using Deflate.
 */
static inline TTZipStatus ttzip_deflate_compress(
    const uint8_t *src,
    size_t src_len,
    uint8_t *dst,
    size_t dst_capacity,
    int32_t level,
    size_t *out_len
) {
    return ttzip_rust_deflate_compress(src, src_len, dst, dst_capacity, level, out_len);
}

/**
 * Decompresses an in-memory Zstandard buffer.
 */
static inline TTZipStatus ttzip_zstd_decompress(
    const uint8_t *src,
    size_t src_len,
    uint8_t *dst,
    size_t dst_capacity,
    size_t *out_len
) {
    return ttzip_rust_zstd_decompress(src, src_len, dst, dst_capacity, out_len);
}

/* ========================================================================== */
/* Checksums and Cryptography                                                 */
/* ========================================================================== */

static inline uint32_t ttzip_crc32(const uint8_t *data, size_t len) {
    return ttzip_rust_crc32(0, data, len);
}

static inline uint64_t ttzip_crc64(const uint8_t *data, size_t len) {
    return ttzip_rust_crc64(0, data, len);
}

/* ========================================================================== */
/* System & Version Information                                               */
/* ========================================================================== */

static inline const char *ttzip_version(void) {
    return ttzip_rust_version();
}

static inline const char *ttzip_status_string(TTZipStatus status) {
    return ttzip_rust_status_string(status);
}

static inline bool ttzip_is_hardware_accelerated(void) {
    return ttzip_rust_is_hardware_accelerated();
}

#ifdef __cplusplus
}
#endif

#endif /* TTZIP_H */
