// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

// C-ABI Interoperability Header Contract (Reconstructed).

#ifndef TTZIP_RUST_GLUE_RECONSTRUCTED_H
#define TTZIP_RUST_GLUE_RECONSTRUCTED_H

#include <stddef.h>
#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

// ============================================================================
// 1. Status Codes & Error Diagnostics Contract
// ============================================================================

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

/**
 * Returns a pointer to a thread-local, null-terminated diagnostic description
 * of the most recent error on the calling thread.
 * Returns NULL if the last operation succeeded.
 */
const char* ttzip_rust_last_error_message(void);

/**
 * Clears the thread-local error description on the calling thread.
 */
void ttzip_rust_clear_last_error(void);

// ============================================================================
// 2. Archive Entry Metadata & Inspect Contract
// ============================================================================

typedef struct TTZipEntryMetadata {
    const char* path;
    uint64_t uncompressed_size;
    uint64_t compressed_size;
    uint32_t crc32;
    int64_t mtime_epoch_secs;
    uint32_t mode;
    bool is_directory;
    bool is_encrypted;
    uint16_t compression_method;
    const char* detected_encoding; // UTF-8, GB18030, Shift-JIS, Big5, etc.
} TTZipEntryMetadata;

typedef bool (*TTZipProgressCallback)(
    uint64_t processed_bytes,
    uint64_t total_bytes,
    const char* current_entry,
    void* user_data
);

typedef bool (*TTZipInspectCallback)(
    const TTZipEntryMetadata* entry,
    void* user_data
);

typedef struct TTZipExtractOptions {
    const char* destination_path;
    const char* password;
    uint32_t thread_budget;
    bool overwrite_existing;
    bool preserve_permissions;
    bool dry_run;
    TTZipProgressCallback progress_callback;
    void* user_data;
} TTZipExtractOptions;

typedef struct TTZipCreateOptions {
    int32_t format;
    int32_t level;
    int32_t encryption;
    const char* password;
    uint32_t thread_budget;
    uint32_t solid_block_size_mb;
    TTZipProgressCallback progress_callback;
    void* user_data;
} TTZipCreateOptions;

// ============================================================================
// 3. Unified Archive Lifecycle Endpoints
// ============================================================================

TTZipStatus ttzip_rust_archive_create_unified(
    const char* const* source_paths,
    size_t source_count,
    const char* destination_path,
    const TTZipCreateOptions* options,
    uint64_t split_volume_size_bytes
);

TTZipStatus ttzip_rust_archive_extract_unified(
    const char* archive_path,
    const char* destination_path,
    const TTZipExtractOptions* options
);

TTZipStatus ttzip_rust_archive_inspect_unified(
    const char* archive_path,
    const char* password,
    bool detect_encoding,
    TTZipInspectCallback callback,
    void* user_data
);

TTZipStatus ttzip_rust_archive_extract_single_entry_memory(
    const char* archive_path,
    const char* entry_path,
    int64_t entry_index,
    const char* password,
    uint8_t* out_buffer,
    size_t buffer_capacity,
    size_t* out_extracted_len
);

void ttzip_rust_free_string(char* ptr);

// ============================================================================
// 4. VFS Session Lifecycle Endpoints
// ============================================================================

typedef struct TTZipVfsTreeHandle* TTZipVfsTreeHandlePtr;

TTZipVfsTreeHandlePtr ttzip_rust_vfs_tree_build(
    const TTZipEntryMetadata* entries,
    size_t count,
    const char* root_name
);

void ttzip_rust_vfs_tree_free(TTZipVfsTreeHandlePtr handle);

TTZipStatus ttzip_rust_vfs_tree_render(
    TTZipVfsTreeHandlePtr handle,
    char** out_rendered_str
);

TTZipStatus ttzip_rust_vfs_fuzzy_search(
    TTZipVfsTreeHandlePtr handle,
    const char* query,
    uint32_t* out_matched_node_ids,
    size_t max_matches,
    size_t* out_match_count
);

#ifdef __cplusplus
}
#endif

#endif // TTZIP_RUST_GLUE_RECONSTRUCTED_H
