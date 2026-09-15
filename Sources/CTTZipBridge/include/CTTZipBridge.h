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

// Hardware-accelerated direct C-ABI checksum and hash functions
uint32_t ttzip_rust_crc32(uint32_t crc, const uint8_t *data, size_t len);
uint32_t ttzip_rust_adler32(uint32_t adler, const uint8_t *data, size_t len);
uint64_t ttzip_rust_crc64(uint64_t seed, const uint8_t *data, size_t len);
uint64_t ttzip_rust_xxh3_64(const uint8_t *data, size_t len, uint64_t seed);
int32_t ttzip_rust_xxh3_128(const uint8_t *data, size_t len, uint64_t seed, uint8_t *out_16_bytes);
int32_t ttzip_rust_blake3(const uint8_t *data, size_t len, uint8_t *out_32_bytes);
int32_t ttzip_rust_blake3_keyed(const uint8_t *key_32_bytes, const uint8_t *data, size_t len, uint8_t *out_32_bytes);
int32_t ttzip_rust_md5(const uint8_t *data, size_t len, uint8_t *out_16_bytes);
int32_t ttzip_rust_sha1(const uint8_t *data, size_t len, uint8_t *out_20_bytes);
int32_t ttzip_rust_sha256(const uint8_t *data, size_t len, uint8_t *out_32_bytes);

// Hardware-accelerated Ciphers and Vault Security
int32_t ttzip_rust_aes256_ctr(const uint8_t *key, uint64_t initial_counter, const uint8_t *src, size_t len, uint8_t *dst);
int32_t ttzip_rust_aes256_cbc_decrypt(const uint8_t *key, const uint8_t *iv, const uint8_t *src, size_t len, uint8_t *dst);
int32_t ttzip_rust_aes256_cbc_encrypt(const uint8_t *key, const uint8_t *iv, const uint8_t *src, size_t len, uint8_t *dst);
int32_t ttzip_rust_vault_encrypt_key(const uint8_t *key, const uint8_t *iv, const uint8_t *src, size_t src_len, const uint8_t *aad, size_t aad_len, uint8_t *out_cipher, uint8_t *out_tag);
int32_t ttzip_rust_vault_decrypt_key(const uint8_t *key, const uint8_t *iv, const uint8_t *cipher, size_t cipher_len, const uint8_t *aad, size_t aad_len, const uint8_t *tag, uint8_t *out_plain);
int32_t ttzip_rust_chacha20_poly1305_encrypt(const uint8_t *key, const uint8_t *nonce, const uint8_t *src, size_t len, const uint8_t *aad, size_t aad_len, uint8_t *dst, uint8_t *out_tag);
int32_t ttzip_rust_chacha20_poly1305_decrypt(const uint8_t *key, const uint8_t *nonce, const uint8_t *src, size_t len, const uint8_t *aad, size_t aad_len, const uint8_t *tag, uint8_t *dst);
int32_t ttzip_rust_zipcrypto_decrypt(const uint8_t *password, size_t password_len, uint8_t *data, size_t len);
int32_t ttzip_rust_zipcrypto_encrypt(const uint8_t *password, size_t password_len, uint8_t *data, size_t len);

// Native Single-Format Compression Codecs C-ABI
int32_t ttzip_rust_deflate_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, int32_t level, size_t *out_len);
int32_t ttzip_rust_deflate_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
int32_t ttzip_rust_zlib_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, int32_t level, size_t *out_len);
int32_t ttzip_rust_zlib_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
int32_t ttzip_rust_gzip_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, int32_t level, size_t *out_len);
int32_t ttzip_rust_gzip_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
size_t ttzip_rust_deflate_compress_bound(size_t src_len, int32_t level);

int32_t ttzip_rust_zstd_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, int32_t level, size_t *out_len);
int32_t ttzip_rust_zstd_compress_advanced(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, int32_t level, uint32_t nb_workers, uint32_t job_size_mb, uint32_t overlap_log, uint32_t window_log, bool enable_ldm, size_t *out_len);
int32_t ttzip_rust_zstd_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
size_t ttzip_rust_zstd_compress_bound(size_t src_len);
uint64_t ttzip_rust_zstd_get_decompressed_size(const uint8_t *src, size_t src_len);

int32_t ttzip_rust_zstd_train_dict(const uint8_t *const *sample_ptrs, const size_t *sample_lens, size_t sample_count, size_t target_dict_size, int32_t level, uint8_t *out_dict, size_t dict_capacity, size_t *out_dict_len);
int32_t ttzip_rust_zstd_dict_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, const uint8_t *dict, size_t dict_len, int32_t level, size_t *out_len);
int32_t ttzip_rust_zstd_dict_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, const uint8_t *dict, size_t dict_len, size_t *out_len);

int32_t ttzip_rust_lz4_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
int32_t ttzip_rust_lz4_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
size_t ttzip_rust_lz4_compress_bound(size_t src_len);

int32_t ttzip_rust_snappy_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
int32_t ttzip_rust_snappy_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
size_t ttzip_rust_snappy_max_compressed_length(size_t src_len);
int32_t ttzip_rust_snappy_uncompressed_length(const uint8_t *src, size_t src_len, size_t *out_len);
int32_t ttzip_rust_snappy_frame_encode(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
int32_t ttzip_rust_snappy_frame_decode(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
size_t ttzip_rust_snappy_frame_max_encoded_length(size_t src_len);

int32_t ttzip_rust_lzfse_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
int32_t ttzip_rust_lzfse_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
int32_t ttzip_rust_lzfse_compress_raw(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
int32_t ttzip_rust_lzfse_decompress_raw(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t uncompressed_len, size_t *out_len);
int32_t ttzip_rust_lzfse_compress_stream(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
int32_t ttzip_rust_lzfse_decompress_stream(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
size_t ttzip_rust_lzfse_compress_bound(size_t src_len);
bool ttzip_rust_lzfse_validate(const uint8_t *src, size_t src_len);

int32_t ttzip_rust_lzvn_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
int32_t ttzip_rust_lzvn_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
int32_t ttzip_rust_lzvn_compress_raw(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
int32_t ttzip_rust_lzvn_decompress_raw(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t uncompressed_len, size_t *out_len);
size_t ttzip_rust_lzvn_compress_bound(size_t src_len);
bool ttzip_rust_lzvn_validate(const uint8_t *src, size_t src_len);

int32_t ttzip_rust_brotli_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, uint32_t quality, uint32_t lgwin, size_t *out_len);
int32_t ttzip_rust_brotli_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
size_t ttzip_rust_brotli_compress_bound(size_t src_len);

int32_t ttzip_rust_fl2_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, int32_t level, uint32_t nb_threads, size_t *out_len);
int32_t ttzip_rust_fl2_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, uint32_t nb_threads, size_t *out_len);
size_t ttzip_rust_fl2_compress_bound(size_t src_len);
uint64_t ttzip_rust_fl2_find_decompressed_size(const uint8_t *src, size_t src_len);

int32_t ttzip_rust_bzip2_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, int32_t level, size_t *out_len);
int32_t ttzip_rust_bzip2_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
size_t ttzip_rust_bzip2_compress_bound(size_t src_len);

static inline uint32_t ttzip_fast_crc32(const uint8_t *ptr, size_t count) {
    return ttzip_rust_crc32(0, ptr, count);
}

static inline uint32_t ttzip_fast_adler32(const uint8_t *ptr, size_t count) {
    return ttzip_rust_adler32(1, ptr, count);
}

// Structured Logging & Diagnostics C-ABI
typedef enum TTZipLogLevel {
    TTZIP_LOG_LEVEL_DEBUG = 0,
    TTZIP_LOG_LEVEL_INFO = 1,
    TTZIP_LOG_LEVEL_WARNING = 2,
    TTZIP_LOG_LEVEL_ERROR = 3
} TTZipLogLevel;

typedef enum TTZipStatus {
    TTZIP_STATUS_OK = 0,
    TTZIP_STATUS_EOF = 1,
    TTZIP_STATUS_CANCELLED = 2,
    TTZIP_STATUS_ERR_INVALID_PARAM = -1,
    TTZIP_STATUS_ERR_FILE_NOT_FOUND = -2,
    TTZIP_STATUS_ERR_MMAP_FAILED = -3,
    TTZIP_STATUS_ERR_CORRUPT_HEADER = -4,
    TTZIP_STATUS_ERR_INVALID_OFFSET = -5,
    TTZIP_STATUS_ERR_ARCHIVE_INIT_FAILED = -6,
    TTZIP_STATUS_ERR_OPEN_FAILED = -7,
    TTZIP_STATUS_ERR_PATH_TOO_LONG = -8,
    TTZIP_STATUS_ERR_OUT_OF_MEMORY = -9,
    TTZIP_STATUS_ERR_INVALID_PASSWORD = -10,
    TTZIP_STATUS_ERR_EXTRACTION_FAILED = -11,
    TTZIP_STATUS_ERR_COMPRESSION_FAILED = -12,
    TTZIP_STATUS_ERR_UNSUPPORTED_FEATURE = -13,
    TTZIP_STATUS_ERR_SOLID_BUDGET_EXCEEDED = -24,
    TTZIP_STATUS_ERR_SECURITY_VIOLATION = -30,
    TTZIP_STATUS_ERR_PANIC_CAUGHT = -99
} TTZipStatus;

typedef void (*TTZipLogCallback)(
    TTZipLogLevel level,
    const char *target_module,
    const char *message,
    const char *file,
    int32_t line,
    void *user_data
);

TTZipStatus ttzip_rust_set_logger(
    TTZipLogCallback callback,
    TTZipLogLevel min_level,
    void *user_data
);

void ttzip_rust_log(
    TTZipLogLevel level,
    const char *target,
    const char *message,
    const char *file,
    int32_t line
);

#ifdef __cplusplus
}
#endif

#endif /* CTTZipBridge_h */
