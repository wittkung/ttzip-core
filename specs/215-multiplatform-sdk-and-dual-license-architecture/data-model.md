# Data Model & Entity Specifications: Multiplatform SDK & Core Engine

**Feature**: `215-multiplatform-sdk-and-dual-license-architecture`  
**Status**: `SPECIFIED` (Revised to match ttzip_rust_glue.h)  

---

## 1. C-ABI Core Types (`Sources/CTTZipBridge/include/`)

### 1.1 Status Codes (`TTZipStatus`)
```c
typedef enum TTZipStatus {
    TTZIP_STATUS_OK                      = 0,
    TTZIP_STATUS_EOF                     = 1,
    TTZIP_STATUS_CANCELLED               = 2,
    TTZIP_STATUS_ERR_INVALID_PARAM       = -1,
    TTZIP_STATUS_ERR_FILE_NOT_FOUND      = -2,
    TTZIP_STATUS_ERR_MMAP_FAILED         = -3,
    TTZIP_STATUS_ERR_CORRUPT_HEADER      = -4,
    TTZIP_STATUS_ERR_INVALID_OFFSET      = -5,
    TTZIP_STATUS_ERR_ARCHIVE_INIT_FAILED = -6,
    TTZIP_STATUS_ERR_OPEN_FAILED         = -7,
    TTZIP_STATUS_ERR_PATH_TOO_LONG       = -8,
    TTZIP_STATUS_ERR_OUT_OF_MEMORY       = -9,
    TTZIP_STATUS_ERR_INVALID_PASSWORD    = -10,
    TTZIP_STATUS_ERR_EXTRACTION_FAILED   = -11,
    TTZIP_STATUS_ERR_COMPRESSION_FAILED  = -12,
    TTZIP_STATUS_ERR_SECURITY_VIOLATION  = -30,
    TTZIP_STATUS_ERR_PANIC_CAUGHT        = -99
} TTZipStatus;
```

### 1.2 Archive Formats (`TTZipArchiveFormat`)
```c
typedef enum TTZipArchiveFormat {
    TTZIP_ARCHIVE_FORMAT_AUTO     = 0,
    TTZIP_ARCHIVE_FORMAT_ZIP      = 1,
    TTZIP_ARCHIVE_FORMAT_SEVEN_ZIP= 2,
    TTZIP_ARCHIVE_FORMAT_TAR      = 3,
    TTZIP_ARCHIVE_FORMAT_TAR_GZ   = 4,
    TTZIP_ARCHIVE_FORMAT_TAR_BZ2  = 5,
    TTZIP_ARCHIVE_FORMAT_TAR_XZ   = 6,
    TTZIP_ARCHIVE_FORMAT_TAR_ZSTD = 7,
    TTZIP_ARCHIVE_FORMAT_DMG      = 8,
    TTZIP_ARCHIVE_FORMAT_LZFSE    = 9,
    TTZIP_ARCHIVE_FORMAT_SNAPPY   = 10,
    TTZIP_ARCHIVE_FORMAT_UNKNOWN  = 99
} TTZipArchiveFormat;
```

### 1.3 Creation & Extraction Options
```c
typedef bool (*TTZipProgressCallback)(
    uint64_t processed_bytes,
    uint64_t total_bytes,
    const char *current_entry,
    void *user_data
);

typedef struct TTZipCreateOptions {
    TTZipArchiveFormat format;
    TTZipCompressionLevel level;
    TTZipEncryptionMethod encryption;
    const char *password;
    uint32_t thread_budget;
    uint32_t solid_block_size_mb;
    TTZipProgressCallback progress_callback;
    void *user_data;
} TTZipCreateOptions;

typedef struct TTZipExtractOptions {
    const char *destination_path;
    const char *password;
    uint32_t thread_budget;
    bool overwrite_existing;
    bool preserve_permissions;
    bool dry_run;
    TTZipProgressCallback progress_callback;
    void *user_data;
} TTZipExtractOptions;
```

### 1.4 Entry Metadata (`TTZipEntryMetadata`)
```c
typedef struct TTZipEntryMetadata {
    const char *path;
    uint64_t uncompressed_size;
    uint64_t compressed_size;
    uint32_t crc32;
    int64_t mtime_epoch_secs;
    uint32_t mode;
    bool is_directory;
    bool is_encrypted;
    uint16_t compression_method;
} TTZipEntryMetadata;
```

---

## 2. CLI Stream Data Model (`--json` NDJSON Schema)

### 2.1 Progress Event Object
```json
{
  "type": "progress",
  "data": {
    "bytes_processed": 104857600,
    "bytes_total": 524288000,
    "entries_processed": 42,
    "entries_total": 120,
    "current_file": "src/kernel/engine.rs",
    "ratio": 0.20,
    "speed_mb_s": 1450.5
  }
}
```

### 2.2 Completion Summary Object
```json
{
  "type": "complete",
  "data": {
    "status": "success",
    "total_input_bytes": 524288000,
    "total_output_bytes": 124890120,
    "compression_ratio": 0.2382,
    "duration_ms": 361.2,
    "throughput_mb_s": 1451.8,
    "entries_count": 120
  }
}
```

---

## 3. Package & License Manifest Data Model

### 3.1 `NOTICE` Specification
```
TTZip Multiplatform Compression Engine
Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
All rights reserved.

This product includes software developed by the TTZip Project
(https://github.com/wittkung/ttzip).

Dual-licensed under the BSD 3-Clause License and the Apache License, Version 2.0.
```

### 3.2 Rust Workspace Topology (`rust/Cargo.toml`)
```toml
[workspace]
members = [
    "ttzip-engine",
    "ttzip-glue",
    "ttzip-cli",
]
resolver = "2"

[workspace.package]
version = "1.0.0"
authors = ["Witt Kung <witt.w.kung@gmail.com>"]
license = "BSD-3-Clause OR Apache-2.0"
repository = "https://github.com/wittkung/ttzip"
```
