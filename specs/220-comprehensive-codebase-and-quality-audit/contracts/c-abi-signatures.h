// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

// Contract: C-ABI Public Surface Alignment Contract

#ifndef TTZIP_C_ABI_CONTRACT_H
#define TTZIP_C_ABI_CONTRACT_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

// 1. Core Lifecycle & Checksums
const char *ttzip_rust_version(void);
int32_t ttzip_rust_init(void);
const char *ttzip_rust_status_string(int32_t status);
bool ttzip_rust_is_hardware_accelerated(void);
uint32_t ttzip_rust_crc32(uint32_t crc, const uint8_t *data, size_t len);
uint32_t ttzip_rust_adler32(uint32_t adler, const uint8_t *data, size_t len);
uint64_t ttzip_rust_crc64(uint64_t seed, const uint8_t *data, size_t len);

// 2. High-Performance Buffer Compression
int32_t ttzip_rust_deflate_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, int32_t level, size_t *out_len);
int32_t ttzip_rust_deflate_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
int32_t ttzip_rust_zstd_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, int32_t level, size_t *out_len);
int32_t ttzip_rust_zstd_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
int32_t ttzip_rust_lz4_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
int32_t ttzip_rust_lz4_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);

#ifdef __cplusplus
}
#endif

#endif /* TTZIP_C_ABI_CONTRACT_H */
