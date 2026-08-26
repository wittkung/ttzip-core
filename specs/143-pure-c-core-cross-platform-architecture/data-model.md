# Data Model: Pure C11 Core Engine & Cross-Platform Architecture

**Feature**: `143-pure-c-core-cross-platform-architecture`  
**Date**: 2026-08-20  
**Phase**: Phase 1 Design

---

## 1. Domain Entities Architecture

This data model specifies the Pure C11 core data structures and C ABI interfaces for `libttzip`:
1. **`TTZipArchiveConfig`**: Configuration payload for archive operations.
2. **`TTZipThreadPoolDescriptor`**: Cross-platform thread pool state and queue properties.
3. **`TTZipFSEntry`**: Unified cross-platform file metadata record.
4. **`TTZipHardwareDescriptor`**: CPU feature detection and vector dispatch flags.

---

## 2. Entity Specifications

### 2.1 `TTZipArchiveConfig`
Represents public C API parameters passed to `ttzip_archive_create()`.

| Field Name | Type | Required | Constraints / Enums | Description |
| :--- | :--- | :---: | :--- | :--- |
| `format` | `String` | Yes | Enum: `"zip"`, `"sevenZip"`, `"tar"`, `"tarZst"`, `"tarGz"`, `"tarBz2"`, `"tarXz"`, `"dmg"`, `"wim"` | Target archive container format. |
| `compressionLevel` | `Integer` | Yes | Range: `[-5, 22]` | Compression level. |
| `numThreads` | `Integer` | Yes | Range: `[0, 128]` (0 = auto-detect hardware cores) | Concurrency limit. |
| `splitVolumeSizeBytes`| `Integer` | No | $\ge 0$ (0 = disabled) | Maximum volume chunk size. |
| `isSolid` | `Boolean` | Yes | Boolean | Whether 7Z solid block stream is enabled. |
| `password` | `String` | No | String or null | Archive encryption password. |

---

### 2.2 `TTZipThreadPoolDescriptor`
Represents the state of the cross-platform thread pool `ttzip_threadpool_t`.

| Field Name | Type | Required | Constraints / Enums | Description |
| :--- | :--- | :---: | :--- | :--- |
| `backendType` | `String` | Yes | Enum: `"posixPthread"`, `"win32ThreadPool"`, `"win32NativeThreads"` | Underlying OS threading implementation. |
| `activeWorkerCount` | `Integer` | Yes | Range: `[1, 128]` | Currently running worker threads. |
| `queueCapacity` | `Integer` | Yes | $\ge 64$ | Maximum ring-buffer pending task queue capacity. |
| `isShutdown` | `Boolean` | Yes | Boolean | Whether the thread pool is terminating. |

---

### 2.3 `TTZipFSEntry`
Represents unified cross-platform file metadata.

| Field Name | Type | Required | Constraints / Enums | Description |
| :--- | :--- | :---: | :--- | :--- |
| `utf8RelativePath` | `String` | Yes | Non-empty UTF-8 string | Sanitized relative file path in archive. |
| `sizeBytes` | `Integer` | Yes | $\ge 0$ | File uncompressed size in bytes. |
| `mtimeEpochSeconds`| `Integer` | Yes | $\ge 0$ | Modification timestamp in UTC seconds. |
| `posixPermissions` | `Integer` | Yes | Octal range: `[0, 0777]` (standardized on Win32 to 0644/0755) | File access mode bits. |
| `isDirectory` | `Boolean` | Yes | Boolean | Directory entry flag. |
| `isSymlink` | `Boolean` | Yes | Boolean | Symbolic link flag. |

---

### 2.4 `TTZipHardwareDescriptor`
Represents detected CPU hardware vector extensions.

| Field Name | Type | Required | Constraints / Enums | Description |
| :--- | :--- | :---: | :--- | :--- |
| `arch` | `String` | Yes | Enum: `"arm64"`, `"x86_64"`, `"generic"` | Detected CPU architecture. |
| `hasPclmulqdq` | `Boolean` | Yes | Boolean | x86_64 PCLMULQDQ capability. |
| `hasSse42Crc32` | `Boolean` | Yes | Boolean | x86_64 SSE4.2 CRC32 capability. |
| `hasAvx2` | `Boolean` | Yes | Boolean | x86_64 AVX2 capability. |
| `hasArmPmull` | `Boolean` | Yes | Boolean | ARM64 PMULL capability. |
| `hasArmAcleCrc` | `Boolean` | Yes | Boolean | ARMv8 ACLE CRC32 capability. |
