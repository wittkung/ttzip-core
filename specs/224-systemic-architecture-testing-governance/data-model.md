# Data Model: Systemic Architecture & Testing Governance

**Feature ID**: `224-systemic-architecture-testing-governance`  
**Classification**: `[Full SDD]`  
**Date**: 2026-08-24  

---

## 1. Core Data Entities

### 1.1 `TTZipEngineTag` (C-ABI & Rust Enum)
Represents the exact concrete engine executing an operation.

| Enum Case | Raw Value | Description |
| :--- | :---: | :--- |
| `Unknown` | 0 | Uninitialized / Unknown execution path |
| `RustRayonParallelZip` | 1 | Native Rust parallel Rayon ZIP compression engine |
| `RustStreamingParallelZip` | 2 | Native Rust streaming parallel ZIP writer with `pwrite` |
| `RustZeroCopy7zDecoder` | 3 | Native Rust streaming/mmap 7z decompression engine |
| `RustPure7zEncoder` | 4 | Native Rust 7z archive creation engine |
| `RustTarStreamEngine` | 5 | Native Rust TAR streaming engine |
| `RustInPlaceZip` | 6 | Native Rust in-place ZIP atomic rewrite engine |
| `RustInPlaceSevenZip` | 7 | Native Rust in-place 7z rewrite engine |
| `RustVfsParallelScanner` | 8 | Native Rust Vfs tree parallel directory scanner |
| `LibarchiveLegacy` | 100 | Legacy C libarchive wrapper fallback |
| `Cli7zFallback` | 101 | 7zz CLI process fallback |
| `SystemTarFallback` | 102 | macOS /usr/bin/tar fallback |

---

### 1.2 `TTZipExecutionProvenance` (C-ABI Struct)
Returned across the FFI boundary to describe the operation's execution trace.

| Field Name | Type | Description |
| :--- | :--- | :--- |
| `engine_tag` | `TTZipEngineTag` | The actual engine executed |
| `thread_count` | `uint32_t` | Concurrency worker count |
| `uncompressed_bytes` | `uint64_t` | Input raw bytes |
| `compressed_bytes` | `uint64_t` | Output compressed bytes |
| `kernel_duration_nanos`| `uint64_t` | Execution time inside Rust engine |
| `is_fallback` | `bool` | True if unexpected fallback occurred |
| `fallback_reason` | `char[128]` | Null-terminated ASCII fallback diagnostic message |

---

### 1.3 `EngineDispatchProvenance` (Swift Struct)
Swift domain representation of the execution report.

```swift
public struct EngineDispatchProvenance: Sendable, Equatable {
    public let engineTag: EngineExecutionTag
    public let threadCount: Int
    public let uncompressedBytes: Int64
    public let compressedBytes: Int64
    public let kernelDurationNanos: UInt64
    public let isFallback: Bool
    public let fallbackReason: String?
    public let ffiBridgeOverheadNanos: UInt64
    public let totalE2EDurationNanos: UInt64
    
    public var compressionRatio: Double
    public var throughputMBs: Double
}
```

---

### 1.4 `TTZipVfsMatchDto` (Zero-Allocation VFS Output)
Fixed-layout C-ABI DTO for populating preallocated search buffers without heap allocation.

```c
typedef struct {
    const char *name;
    const char *path;
    uint64_t uncompressed_size;
    uint64_t compressed_size;
    uint32_t crc32;
    int64_t score;
    bool is_directory;
    bool is_encrypted;
} TTZipVfsMatchDto;
```
