# Phase 1 Data Model: Single-Core Deflate Engine Architecture

**Feature Branch**: `137-libdeflate-single-core-consolidation`

**Date**: 2026-08-20

## Core Entities & Data Structures

### 1. `DeflateCompressionRequest`
Encapsulates a request for in-memory or chunk-based Deflate compression.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `sourceBytes` | `Int` | Yes | Uncompressed input buffer size in bytes |
| `destinationCapacity` | `Int` | Yes | Pre-allocated destination buffer capacity in bytes |
| `compressionLevel` | `Int` | Yes | Target Deflate level, clamped to `[0, 12]` |
| `isFinal` | `Boolean` | Yes | Whether this chunk represents the terminal block in the stream |

### 2. `DeflateCompressionResponse`
Encapsulates the result of a Deflate compression operation.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `status` | `String` (`SUCCESS`, `BUFFER_TOO_SMALL`, `INVALID_ARGUMENT`, `INTERNAL_ERROR`) | Yes | Result status code |
| `bytesWritten` | `Int` | Yes | Number of compressed bytes written to destination |
| `compressedRatio` | `Number` | Yes | Ratio of `bytesWritten` to `sourceBytes` |

### 3. `DeflateDecompressionRequest`
Encapsulates a request for in-memory or chunk-based Deflate decompression.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `compressedBytes` | `Int` | Yes | Compressed input payload size in bytes |
| `expectedOriginalSize` | `Int` | Yes | Expected decompressed uncompressed buffer capacity in bytes |

### 4. `DeflateDecompressionResponse`
Encapsulates the result of a Deflate decompression operation.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `status` | `String` (`SUCCESS`, `DECOMPRESSION_FAILED`, `CORRUPTED_STREAM`, `BUFFER_TOO_SMALL`) | Yes | Result status code |
| `bytesDecompressed` | `Int` | Yes | Actual uncompressed bytes written to destination buffer |

### 5. `StreamingDeflateChunkEvent`
Encapsulates an incremental stream processing event in `DeflateStreamEngine`.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `chunkIndex` | `Int` | Yes | Zero-based incremental chunk sequence number |
| `inputBytesProcessed` | `Int` | Yes | Number of source bytes consumed in this chunk |
| `outputBytesEmitted` | `Int` | Yes | Number of compressed/decompressed bytes produced in this chunk |
| `isStreamEnd` | `Boolean` | Yes | Whether stream termination (`Z_STREAM_END` / `EOF`) was reached |
