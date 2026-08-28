// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

#ifndef CTTZipBridge_h
#define CTTZipBridge_h

#include <stddef.h>
#include <stdint.h>
#include <stdbool.h>
#include "ttzip_engineFFI.h"



#ifdef __cplusplus
extern "C" {
#endif

// Platform File Kind (for format sniffing)
typedef enum {
    TTZIP_KIND_UNKNOWN = 0,
    TTZIP_KIND_ARCHIVE = 1,
    TTZIP_KIND_IMAGE   = 2,
    TTZIP_KIND_AUDIO   = 3,
    TTZIP_KIND_VIDEO   = 4,
    TTZIP_KIND_TEXT    = 5,
    TTZIP_KIND_BINARY  = 6
} ttzip_file_kind_t;

// Hardware-accelerated direct C-ABI checksum functions
uint32_t ttzip_rust_crc32(uint32_t crc, const uint8_t *data, size_t len);
uint32_t ttzip_rust_adler32(uint32_t adler, const uint8_t *data, size_t len);
uint64_t ttzip_rust_crc64(uint64_t seed, const uint8_t *data, size_t len);

static inline uint32_t ttzip_fast_crc32(const uint8_t *ptr, size_t count) {
    return ttzip_rust_crc32(0, ptr, count);
}

static inline uint32_t ttzip_fast_adler32(const uint8_t *ptr, size_t count) {
    return ttzip_rust_adler32(1, ptr, count);
}

#ifdef __cplusplus
}
#endif

#endif /* CTTZipBridge_h */
