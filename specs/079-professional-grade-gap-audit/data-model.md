# Phase 1 Data Model: 079-professional-grade-gap-audit

**Feature**: Comprehensive Professional Software Gap Audit & Architecture Plan
**Branch**: `079-professional-grade-gap-audit`
**Status**: Draft

---

## 1. Core Domain Entities

### 1.1 InPlaceEditSession
Represents an active in-place file editing session where an archive entry is staged in a sandbox directory and monitored for external modifications.

| Field Name | Type | Required | Description | Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `sessionId` | `String` (UUID) | Yes | Unique identifier of the editing session | UUID v4 format |
| `archivePath` | `String` | Yes | Absolute path to target archive file on disk | Valid POSIX path |
| `entryPath` | `String` | Yes | Relative virtual path of entry within the archive | Non-empty string |
| `stagedFilePath` | `String` | Yes | Absolute path to temporary staged file in sandbox | Subpath of `NSTemporaryDirectory()` |
| `stagedDirectoryPath` | `String` | Yes | Absolute path to parent staging folder monitored by kqueue | Subpath of `NSTemporaryDirectory()` |
| `state` | `EditSessionState` (Enum) | Yes | Current lifecycle state of the session | `staged`, `listening`, `syncing`, `saved`, `closed`, `error` |
| `initialHash` | `String` | Yes | SHA-256 hex digest of file at extraction time | 64-char lowercase hex string |
| `lastKnownMtime` | `Double` | Yes | Timestamp of last recorded file modification | Unix epoch seconds |
| `hasUnsavedChanges` | `Bool` | Yes | Indicates whether staged file differs from archive state | Boolean |
| `errorMessage` | `String?` | No | Diagnostic error message if session encountered a fault | Nullable string |

---

### 1.2 QuickLookPreviewPayload
Represents the lightweight metadata and document structure serialized for Quick Look preview rendering.

| Field Name | Type | Required | Description | Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `archivePath` | `String` | Yes | Absolute path to the previewed archive | Valid POSIX path |
| `archiveName` | `String` | Yes | File name of the archive | Non-empty string |
| `formatIdentifier` | `String` | Yes | Normalized archive format identifier | `zip`, `7z`, `tar`, `gz`, `bz2`, `xz`, `zst`, `lz4`, `lz`, `lrz`, `aar`, `sz`, `wim`, `dmg`, `iso`, `rar`, `cab` |
| `uncompressedSizeBytes` | `Int64` | Yes | Total uncompressed payload volume in bytes | `>= 0` |
| `compressedSizeBytes` | `Int64` | Yes | Physical file size on disk in bytes | `>= 0` |
| `compressionRatioPercent` | `Double` | Yes | Space saving percentage `(1 - compressed / uncompressed) * 100` | Range `[0.0, 100.0]` |
| `totalEntriesCount` | `Int` | Yes | Total count of files and directories in archive | `>= 0` |
| `isEncrypted` | `Bool` | Yes | True if one or more entries are password-protected | Boolean |
| `rootNodes` | `Array<PreviewTreeNode>` | Yes | Hierarchical tree nodes for UI rendering | Array of `PreviewTreeNode` |

#### Sub-entity: PreviewTreeNode
| Field Name | Type | Required | Description | Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `id` | `String` | Yes | Unique node identifier (path or UUID) | Non-empty string |
| `name` | `String` | Yes | Basename of file or folder | Non-empty string |
| `relativePath` | `String` | Yes | Full relative path within the archive | Non-empty string |
| `isDirectory` | `Bool` | Yes | True if entry is a directory | Boolean |
| `uncompressedSizeBytes` | `Int64` | Yes | File size in bytes (0 for directories) | `>= 0` |
| `isEncrypted` | `Bool` | Yes | True if this specific entry is encrypted | Boolean |
| `children` | `Array<PreviewTreeNode>?` | No | Sub-nodes if entry is a directory | Nullable array |

---

### 1.3 FinderSyncActionRequest
Represents an action dispatched from macOS Finder context menus or Services.

| Field Name | Type | Required | Description | Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `actionIdentifier` | `String` | Yes | Action type to execute | `extract_here`, `extract_to_subfolder`, `inspect_archive`, `compress_quick_7z`, `compress_quick_zip`, `compress_separate`, `compress_and_delete_source`, `compress_modal_advanced` |
| `sourcePaths` | `Array<String>` | Yes | List of selected target paths in Finder | Array of absolute POSIX paths, length `>= 1` |
| `destinationDirectory` | `String?` | No | Target extraction/compression destination | Nullable absolute POSIX path |
| `sanitizeMacMetadata` | `Bool` | Yes | If true, strip `.DS_Store`, `__MACOSX`, etc. | Boolean |
| `password` | `String?` | No | Optional decryption password | Nullable string |

---

### 1.4 ArchiveIntegrityReport
Represents the result of an in-memory CRC32/SHA-256 integrity verification pass.

| Field Name | Type | Required | Description | Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `archivePath` | `String` | Yes | Absolute path to verified archive | Valid POSIX path |
| `totalEntriesCount` | `Int` | Yes | Total number of entries in archive | `>= 0` |
| `verifiedEntriesCount` | `Int` | Yes | Number of successfully decoded entries | `>= 0` |
| `corruptedEntriesCount` | `Int` | Yes | Number of entries with checksum mismatches or decode errors | `>= 0` |
| `overallStatus` | `IntegrityStatus` (Enum) | Yes | Overall verification result | `passed`, `corrupted`, `unreadable`, `encrypted_missing_key` |
| `verificationDurationSeconds` | `Double` | Yes | Total time spent in verification | `>= 0.0` |
| `averageThroughputMBs` | `Double` | Yes | In-memory verification throughput in MB/s | `>= 0.0` |
| `corruptedEntries` | `Array<CorruptedEntryDetail>` | Yes | Detailed error records for failed entries | Array of `CorruptedEntryDetail` |

#### Sub-entity: CorruptedEntryDetail
| Field Name | Type | Required | Description | Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `entryPath` | `String` | Yes | Path of corrupted file inside archive | Non-empty string |
| `errorType` | `String` | Yes | Category of corruption | `crc32_mismatch`, `header_damaged`, `block_truncated`, `invalid_dictionary` |
| `expectedChecksum` | `String` | Yes | Expected CRC32 or SHA-256 hex string | Hex string |
| `actualChecksum` | `String` | Yes | Calculated checksum hex string | Hex string |
| `diagnosticMessage` | `String` | Yes | Low-level C engine error description | Non-empty string |

---

### 1.5 GlobalOperationsQueueEvent
Represents a real-time event broadcasted by the global multi-task scheduler.

| Field Name | Type | Required | Description | Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `taskId` | `String` (UUID) | Yes | Unique task identifier | UUID v4 |
| `taskName` | `String` | Yes | Human-readable name (e.g. source filename) | Non-empty string |
| `operationType` | `String` | Yes | Type of archiving operation | `compress`, `extract`, `test`, `repair`, `batch_compress`, `batch_extract` |
| `state` | `String` | Yes | Current task execution status | `queued`, `running`, `paused`, `completed`, `failed`, `cancelled` |
| `priority` | `String` | Yes | Task priority level | `critical`, `userInitiated`, `utility`, `background` |
| `bytesProcessed` | `Int64` | Yes | Bytes processed so far | `>= 0` |
| `totalBytes` | `Int64` | Yes | Total expected bytes | `>= 0` |
| `fractionCompleted` | `Double` | Yes | Progress fraction | Range `[0.0, 1.0]` |
| `throughputMBs` | `Double` | Yes | Instantaneous throughput in MB/s | `>= 0.0` |
| `estimatedTimeRemainingSeconds` | `Double?` | No | Estimated remaining time | Nullable `>= 0.0` |
| `errorMessage` | `String?` | No | Error diagnostic message if failed | Nullable string |
