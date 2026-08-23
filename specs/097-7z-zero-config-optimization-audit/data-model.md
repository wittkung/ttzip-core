# Phase 1 Data Model: 7z Zero-Configuration & Performance Architecture

**Feature Directory**: `specs/097-7z-zero-config-optimization-audit`  
**Date**: 2026-08-18  
**Status**: Completed

---

## 1. Entities & Data Structures

### Entity: `SevenZipCompressionConfig`
Represents the runtime configuration for a 7z archive creation operation, determined automatically via zero-configuration heuristics.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `outputPath` | String | Yes | Absolute filesystem path for the output `.7z` archive file |
| `inputPaths` | Array<String> | Yes | Array of absolute source file/directory paths to include |
| `compressionLevel` | Integer (0..9) | Yes | Effective compression level (0=Store, 1=Fastest, 5=Normal, 9=Ultra) |
| `password` | String (optional) | No | Optional passphrase for AES-256 encryption |
| `blockSizeBytes` | Integer (262144..33554432) | Yes | Dynamically calculated LZMA2 block size in bytes (256KB..32MB) |
| `dictionarySizeBytes` | Integer (4096..33554432) | Yes | Dynamically selected dictionary size in bytes (4KB..32MB) |
| `threadCount` | Integer (1..64) | Yes | Dynamically detected physical/logical CPU core allocation |
| `isEntropyStoreBypass` | Boolean | Yes | Flag indicating whether high entropy ($H > 7.90$) triggered Store mode |

---

### Entity: `SevenZipExtractionConfig`
Represents the parameters and hardware dispatch context for 7z archive extraction.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `archivePath` | String | Yes | Absolute filesystem path to the source `.7z` or split-volume file |
| `destinationDir` | String | Yes | Target directory path for extracted files |
| `password` | String (optional) | No | Optional decryption passphrase |
| `skipMacJunk` | Boolean | Yes | Whether to filter AppleDouble (`._*`) and `.DS_Store` artifacts |
| `enableHardwareCrypto` | Boolean | Yes | Whether ARM64 NEON hardware AES-256 and SHA-256 KDF are engaged |
| `useDirectoryCache` | Boolean | Yes | Whether L1/L2 stack directory caching is active for APFS write reduction |

---

### Entity: `SevenZipArchiveInspectionResult`
Represents the structural metadata of a 7z archive extracted via zero-copy memory-mapped Header parsing.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `archivePath` | String | Yes | Absolute path of inspected archive |
| `fileSizeBytes` | Integer | Yes | Total archive file size on disk in bytes |
| `totalEntriesCount` | Integer | Yes | Total count of files and directories in the archive |
| `totalUncompressedBytes` | Integer | Yes | Sum of uncompressed file payload sizes |
| `encryptionTier` | String (enum: `none`, `dataOnly`, `headerAndData`) | Yes | Archive encryption classification |
| `entries` | Array<SevenZipEntryDescriptorItem> | Yes | List of individual entry descriptors |
| `inspectionDurationMs` | Number | Yes | Time elapsed during zero-copy inspection in milliseconds |

---

### Entity: `SevenZipEntryDescriptorItem`
Represents an individual file or directory record parsed from the 7z Header Database.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `relativePath` | String | Yes | Relative filesystem path inside the archive container |
| `isDirectory` | Boolean | Yes | Whether this entry represents a directory |
| `uncompressedSizeBytes` | Integer | Yes | Uncompressed size in bytes |
| `compressedSizeBytes` | Integer | Yes | Packed size in bytes within the stream |
| `crc32` | Integer (UInt32) | Yes | 32-bit CRC checksum of uncompressed stream |
| `isEncrypted` | Boolean | Yes | Whether this entry payload is AES-256 encrypted |

---

### Entity: `SevenZipEntropyEvaluation`
Represents the diagnostic result of dynamic entropy estimation for an input buffer.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `shannonEntropy` | Number (0.00..8.00) | Yes | Calculated Shannon entropy in bits per byte |
| `sampledBytes` | Integer | Yes | Number of bytes read across non-uniform sample points |
| `totalBufferBytes` | Integer | Yes | Total input buffer size in bytes |
| `recommendDirectStore` | Boolean | Yes | Whether entropy $> 7.90$ warrants Store mode downgrade |
| `evaluationDurationUs` | Number | Yes | Sampling execution duration in microseconds |
