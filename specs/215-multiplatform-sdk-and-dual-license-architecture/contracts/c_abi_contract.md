# Interface Contract: Public C11 ABI Subset (`ttzip.h`)

**Feature**: `215-multiplatform-sdk-and-dual-license-architecture`  
**Status**: `FROZEN` (Revised as a clean public subset of ttzip_rust_glue.h)  

---

## 1. Header Overview & Invariants

`ttzip.h` is the **minimal, public C11 SDK interface ($\le 100\text{ LOC}$)** exported for third-party C, C++, and foreign-language FFI consumers. Advanced internal subsystems (VFS cache, worker pools, password brute-force, hex diffing) reside in `ttzip_rust_glue.h` for internal `TTZipCore` usage.

- **Standard**: ANSI C11 (`<stdint.h>`, `<stdbool.h>`, `<stddef.h>`).
- **Panic Safety**: All functions are wrapped in `std::panic::catch_unwind`; returns `TTZIP_STATUS_ERR_PANIC_CAUGHT` (-99) on unhandled unwinds.
- **Linkage**: `extern "C"` with `#[no_mangle]` in `ttzip-glue`.

---

## 2. Public Header Definition (`Sources/CTTZipBridge/include/ttzip.h`)

```c
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

// Re-use core types from ttzip_rust_glue.h
#include "ttzip_rust_glue.h"

/* ========================================================================== */
/* Public High-Level Archive APIs (Thin Wrappers over unified engine)         */
/* ========================================================================== */

/**
 * Creates an archive from a list of source files or directories.
 */
TTZIP_API TTZipStatus ttzip_create_archive(
    const char *const *source_paths,
    size_t source_count,
    const char *destination_path,
    const TTZipCreateOptions *options
);

/**
 * Extracts an archive to the specified destination directory.
 */
TTZIP_API TTZipStatus ttzip_extract_archive(
    const char *archive_path,
    const char *destination_path,
    const TTZipExtractOptions *options
);

/**
 * Inspects entries in an archive, invoking callback for each item.
 */
TTZIP_API TTZipStatus ttzip_inspect_archive(
    const char *archive_path,
    const char *password,
    bool detect_encoding,
    TTZipInspectCallback callback,
    void *user_data
);

/* ========================================================================== */
/* In-Memory Codec Buffers                                                    */
/* ========================================================================== */

/**
 * Compresses an in-memory buffer using Deflate/Zstandard.
 */
TTZIP_API TTZipStatus ttzip_compress_buffer(
    TTZipArchiveFormat format,
    const uint8_t *src,
    size_t src_len,
    uint8_t *dst,
    size_t dst_capacity,
    int32_t level,
    size_t *out_len
);

/**
 * Decompresses an in-memory buffer.
 */
TTZIP_API TTZipStatus ttzip_decompress_buffer(
    TTZipArchiveFormat format,
    const uint8_t *src,
    size_t src_len,
    uint8_t *dst,
    size_t dst_capacity,
    size_t *out_len
);

/* ========================================================================== */
/* Version & System                                                           */
/* ========================================================================== */

TTZIP_API const char *ttzip_version(void);
TTZIP_API const char *ttzip_status_message(TTZipStatus status);
TTZIP_API bool ttzip_is_hardware_accelerated(void);

#ifdef __cplusplus
}
#endif

#endif /* TTZIP_H */
```
