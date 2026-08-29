// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

// Standalone self-contained C-ABI header for C & C++ SDKs.

#ifndef TTZIP_H
#define TTZIP_H

#include <stddef.h>
#include <stdint.h>
#include <stdbool.h>
#include <string.h>

#ifdef __cplusplus
extern "C" {
#endif

// MARK: - Status Codes

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
    TTZIP_STATUS_ERR_SECURITY_VIOLATION = -30,
    TTZIP_STATUS_ERR_PANIC_CAUGHT = -99
} TTZipStatus;

// MARK: - Formats, Levels & Methods

typedef enum TTZipArchiveFormat {
    TTZIP_ARCHIVE_FORMAT_AUTO = 0,
    TTZIP_ARCHIVE_FORMAT_ZIP = 1,
    TTZIP_ARCHIVE_FORMAT_SEVEN_ZIP = 2,
    TTZIP_ARCHIVE_FORMAT_TAR = 3,
    TTZIP_ARCHIVE_FORMAT_TAR_GZ = 4,
    TTZIP_ARCHIVE_FORMAT_TAR_BZ2 = 5,
    TTZIP_ARCHIVE_FORMAT_TAR_XZ = 6,
    TTZIP_ARCHIVE_FORMAT_TAR_ZSTD = 7,
    TTZIP_ARCHIVE_FORMAT_DMG = 8,
    TTZIP_ARCHIVE_FORMAT_LZFSE = 9,
    TTZIP_ARCHIVE_FORMAT_SNAPPY = 10,
    TTZIP_ARCHIVE_FORMAT_UNKNOWN = 99
} TTZipArchiveFormat;

typedef enum TTZipCompressionLevel {
    TTZIP_COMPRESSION_LEVEL_STORE = 0,
    TTZIP_COMPRESSION_LEVEL_FASTEST = 1,
    TTZIP_COMPRESSION_LEVEL_FAST = 3,
    TTZIP_COMPRESSION_LEVEL_NORMAL = 6,
    TTZIP_COMPRESSION_LEVEL_MAXIMUM = 9,
    TTZIP_COMPRESSION_LEVEL_ULTRA = 12
} TTZipCompressionLevel;

typedef enum TTZipEncryptionMethod {
    TTZIP_ENCRYPTION_NONE = 0,
    TTZIP_ENCRYPTION_ZIP_CRYPTO = 1,
    TTZIP_ENCRYPTION_AES128 = 2,
    TTZIP_ENCRYPTION_AES192 = 3,
    TTZIP_ENCRYPTION_AES256 = 4
} TTZipEncryptionMethod;

typedef enum TTZipLogLevel {
    TTZIP_LOG_LEVEL_DEBUG = 0,
    TTZIP_LOG_LEVEL_INFO = 1,
    TTZIP_LOG_LEVEL_WARNING = 2,
    TTZIP_LOG_LEVEL_ERROR = 3
} TTZipLogLevel;

typedef enum TTZipEngineTag {
    TTZIP_ENGINE_UNKNOWN = 0,
    TTZIP_ENGINE_RUST_RAYON_PARALLEL_ZIP = 1,
    TTZIP_ENGINE_RUST_STREAMING_PARALLEL_ZIP = 2,
    TTZIP_ENGINE_RUST_ZERO_COPY_7Z_DECODER = 3,
    TTZIP_ENGINE_RUST_PURE_7Z_ENCODER = 4,
    TTZIP_ENGINE_RUST_TAR_STREAM = 5,
    TTZIP_ENGINE_RUST_IN_PLACE_ZIP = 6,
    TTZIP_ENGINE_RUST_IN_PLACE_7Z = 7,
    TTZIP_ENGINE_RUST_VFS_PARALLEL_SCANNER = 8,
    TTZIP_ENGINE_LIBARCHIVE_LEGACY = 100,
    TTZIP_ENGINE_CLI_7Z_FALLBACK = 101,
    TTZIP_ENGINE_SYSTEM_TAR_FALLBACK = 102
} TTZipEngineTag;

// MARK: - Data Structures

typedef struct TTZipExecutionProvenance {
    TTZipEngineTag engine_tag;
    uint32_t thread_count;
    uint64_t uncompressed_bytes;
    uint64_t compressed_bytes;
    uint64_t kernel_duration_nanos;
    bool is_fallback;
    char fallback_reason[128];
} TTZipExecutionProvenance;

typedef struct TTZipEntryMetadata {
    uint32_t struct_size;
    uint32_t abi_version;
    const char *path;
    uint64_t uncompressed_size;
    uint64_t compressed_size;
    uint32_t crc32;
    int64_t mtime_epoch_secs;
    uint32_t mode;
    bool is_directory;
    bool is_encrypted;
    uint16_t compression_method;
    const char *detected_encoding;
} TTZipEntryMetadata;

typedef bool (*TTZipProgressCallback)(uint64_t processed_bytes, uint64_t total_bytes, const char *current_entry, void *user_data);
typedef bool (*TTZipInspectCallback)(const TTZipEntryMetadata *entry, void *user_data);
typedef void (*TTZipLogCallback)(TTZipLogLevel level, const char *target_module, const char *message, const char *file, int32_t line, void *user_data);

typedef struct TTZipExtractOptions {
    uint32_t struct_size;
    uint32_t abi_version;
    const char *destination_path;
    const char *password;
    uint32_t thread_budget;
    bool overwrite_existing;
    bool preserve_permissions;
    bool dry_run;
    TTZipProgressCallback progress_callback;
    void *user_data;
} TTZipExtractOptions;

typedef struct TTZipCreateOptions {
    uint32_t struct_size;
    uint32_t abi_version;
    TTZipArchiveFormat format;
    TTZipCompressionLevel level;
    TTZipEncryptionMethod encryption;
    const char *password;
    uint32_t thread_budget;
    uint32_t solid_block_size_mb;
    TTZipProgressCallback progress_callback;
    void *user_data;
} TTZipCreateOptions;

typedef struct TTZipErrorInfo {
    TTZipStatus status;
    int32_t error_code;
    char message[512];
    char entry_path[256];
    uint64_t offset;
} TTZipErrorInfo;

typedef struct TTZipCpuCapsRaw {
    uint32_t logical_cores;
    size_t physical_page_size;
    uint32_t p_cores;
    uint32_t e_cores;
    bool has_arm_neon;
    bool has_arm_crypto;
    bool has_aes_ni;
    bool has_avx2;
    bool has_avx512;
    bool has_hardware_crc32;
} TTZipCpuCapsRaw;

// MARK: - Rust C-ABI Export Prototypes

const char *ttzip_rust_version(void);
TTZipStatus ttzip_rust_init(void);
const char *ttzip_rust_status_string(TTZipStatus status);
const char *ttzip_rust_last_error_message(void);
void ttzip_rust_clear_last_error(void);
bool ttzip_rust_is_hardware_accelerated(void);
uint32_t ttzip_rust_crc32(uint32_t crc, const uint8_t *data, size_t len);
uint32_t ttzip_rust_adler32(uint32_t adler, const uint8_t *data, size_t len);
uint64_t ttzip_rust_crc64(uint64_t seed, const uint8_t *data, size_t len);

TTZipStatus ttzip_rust_create_archive(
    const char *const *source_paths,
    size_t source_count,
    const char *destination_path,
    const TTZipCreateOptions *options
);

TTZipStatus ttzip_rust_extract_archive(
    const char *archive_path,
    const char *destination_path,
    const TTZipExtractOptions *options
);

TTZipStatus ttzip_rust_inspect_archive(
    const char *archive_path,
    const char *password,
    bool detect_encoding,
    TTZipInspectCallback callback,
    void *user_data
);

TTZipStatus ttzip_rust_archive_create_unified(
    const char *const *source_paths,
    size_t source_count,
    const char *destination_path,
    const TTZipCreateOptions *options,
    uint64_t split_volume_size_bytes
);

TTZipStatus ttzip_rust_archive_extract_unified(
    const char *archive_path,
    const char *destination_path,
    const TTZipExtractOptions *options
);

TTZipStatus ttzip_rust_archive_extract_unified_v2(
    const char *archive_path,
    const char *destination_path,
    const TTZipExtractOptions *options,
    uint64_t *out_extracted_bytes,
    TTZipErrorInfo *out_error
);

TTZipStatus ttzip_rust_archive_inspect_unified(
    const char *archive_path,
    const char *password,
    bool detect_encoding,
    TTZipInspectCallback callback,
    void *user_data
);

TTZipStatus ttzip_rust_archive_extract_single_entry_memory(
    const char *archive_path,
    const char *entry_path,
    int64_t entry_index,
    const char *password,
    uint8_t *out_buffer,
    size_t buffer_capacity,
    size_t *out_extracted_len
);

TTZipStatus ttzip_rust_archive_extract_selected(
    const char *archive_path,
    const char *const *target_paths,
    size_t target_count,
    const char *destination_dir,
    const TTZipExtractOptions *options,
    size_t *out_extracted_count
);

TTZipStatus ttzip_rust_archive_verify_stream(
    const char *archive_path,
    const char *password,
    TTZipProgressCallback progress_callback,
    void *user_data,
    char **out_report_json
);

TTZipStatus ttzip_rust_archive_repair_unified(
    const char *damaged_path,
    const char *repaired_path,
    size_t *out_salvaged_count
);

TTZipStatus ttzip_rust_cpu_get_capabilities(TTZipCpuCapsRaw *out_caps);
bool ttzip_rust_get_last_execution_provenance(TTZipExecutionProvenance *out_provenance);
const char *ttzip_rust_engine_tag_name(TTZipEngineTag tag);
void ttzip_rust_free_string(char *ptr);

// Native Codecs C-ABI Declarations
TTZipStatus ttzip_rust_deflate_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, int32_t level, size_t *out_len);
TTZipStatus ttzip_rust_deflate_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
TTZipStatus ttzip_rust_zlib_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, int32_t level, size_t *out_len);
TTZipStatus ttzip_rust_zlib_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
TTZipStatus ttzip_rust_gzip_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, int32_t level, size_t *out_len);
TTZipStatus ttzip_rust_gzip_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
size_t ttzip_rust_deflate_compress_bound(size_t src_len, int32_t level);

TTZipStatus ttzip_rust_zstd_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, int32_t level, size_t *out_len);
TTZipStatus ttzip_rust_zstd_compress_advanced(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, int32_t level, uint32_t nb_workers, uint32_t job_size_mb, uint32_t overlap_log, uint32_t window_log, bool enable_ldm, size_t *out_len);
TTZipStatus ttzip_rust_zstd_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
size_t ttzip_rust_zstd_compress_bound(size_t src_len);
uint64_t ttzip_rust_zstd_get_decompressed_size(const uint8_t *src, size_t src_len);

TTZipStatus ttzip_rust_lz4_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
TTZipStatus ttzip_rust_lz4_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
size_t ttzip_rust_lz4_compress_bound(size_t src_len);

TTZipStatus ttzip_rust_snappy_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
TTZipStatus ttzip_rust_snappy_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
size_t ttzip_rust_snappy_max_compressed_length(size_t src_len);

TTZipStatus ttzip_rust_lzfse_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
TTZipStatus ttzip_rust_lzfse_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
size_t ttzip_rust_lzfse_compress_bound(size_t src_len);

TTZipStatus ttzip_rust_brotli_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, uint32_t quality, uint32_t lgwin, size_t *out_len);
TTZipStatus ttzip_rust_brotli_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);

TTZipStatus ttzip_rust_fl2_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, int32_t level, uint32_t nb_threads, size_t *out_len);
TTZipStatus ttzip_rust_fl2_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, uint32_t nb_threads, size_t *out_len);
size_t ttzip_rust_fl2_compress_bound(size_t src_len);
uint64_t ttzip_rust_fl2_find_decompressed_size(const uint8_t *src, size_t src_len);

TTZipStatus ttzip_rust_bzip2_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, int32_t level, size_t *out_len);
TTZipStatus ttzip_rust_bzip2_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, size_t *out_len);
size_t ttzip_rust_bzip2_compress_bound(size_t src_len);

TTZipStatus ttzip_rust_zstd_train_dict(const uint8_t *const *sample_ptrs, const size_t *sample_lens, size_t sample_count, size_t target_dict_size, int32_t level, uint8_t *out_dict, size_t dict_capacity, size_t *out_dict_len);
TTZipStatus ttzip_rust_zstd_dict_compress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, const uint8_t *dict, size_t dict_len, int32_t level, size_t *out_len);
TTZipStatus ttzip_rust_zstd_dict_decompress(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_capacity, const uint8_t *dict, size_t dict_len, size_t *out_len);

uint64_t ttzip_rust_xxh3_64(const uint8_t *data, size_t len, uint64_t seed);
int32_t ttzip_rust_xxh3_128(const uint8_t *data, size_t len, uint64_t seed, uint8_t *out_16_bytes);
int32_t ttzip_rust_blake3(const uint8_t *data, size_t len, uint8_t *out_32_bytes);
int32_t ttzip_rust_blake3_keyed(const uint8_t *key_32_bytes, const uint8_t *data, size_t len, uint8_t *out_32_bytes);
int32_t ttzip_rust_md5(const uint8_t *data, size_t len, uint8_t *out_16_bytes);
int32_t ttzip_rust_sha1(const uint8_t *data, size_t len, uint8_t *out_20_bytes);
int32_t ttzip_rust_sha256(const uint8_t *data, size_t len, uint8_t *out_32_bytes);

int32_t ttzip_rust_aes256_ctr(const uint8_t *key, uint64_t initial_counter, const uint8_t *src, size_t len, uint8_t *dst);
int32_t ttzip_rust_aes256_cbc_decrypt(const uint8_t *key, const uint8_t *iv, const uint8_t *src, size_t len, uint8_t *dst);
int32_t ttzip_rust_aes256_cbc_encrypt(const uint8_t *key, const uint8_t *iv, const uint8_t *src, size_t len, uint8_t *dst);
int32_t ttzip_rust_vault_encrypt_key(const uint8_t *key, const uint8_t *iv, const uint8_t *src, size_t src_len, const uint8_t *aad, size_t aad_len, uint8_t *out_cipher, uint8_t *out_tag);
int32_t ttzip_rust_vault_decrypt_key(const uint8_t *key, const uint8_t *iv, const uint8_t *cipher, size_t cipher_len, const uint8_t *aad, size_t aad_len, const uint8_t *tag, uint8_t *out_plain);
int32_t ttzip_rust_chacha20_poly1305_encrypt(const uint8_t *key, const uint8_t *nonce, const uint8_t *src, size_t len, const uint8_t *aad, size_t aad_len, uint8_t *dst, uint8_t *out_tag);
int32_t ttzip_rust_chacha20_poly1305_decrypt(const uint8_t *key, const uint8_t *nonce, const uint8_t *src, size_t len, const uint8_t *aad, size_t aad_len, const uint8_t *tag, uint8_t *dst);
int32_t ttzip_rust_zipcrypto_decrypt(const uint8_t *password, size_t password_len, uint8_t *data, size_t len);
int32_t ttzip_rust_zipcrypto_encrypt(const uint8_t *password, size_t password_len, uint8_t *data, size_t len);

// MARK: - Inline C SDK Wrappers

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
        opt_copy.struct_size = (uint32_t)sizeof(TTZipCreateOptions);
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
        opt_copy.struct_size = (uint32_t)sizeof(TTZipExtractOptions);
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
