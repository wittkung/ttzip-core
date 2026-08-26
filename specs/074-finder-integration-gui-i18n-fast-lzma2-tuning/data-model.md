# Data Model: macOS Finder Integration, GUI Localization, and Fast LZMA2 Micro-Tuning

**Feature**: `074-finder-integration-gui-i18n-fast-lzma2-tuning`  
**Date**: 2026-08-18

---

## 1. Finder Integration & QuickLook Entities

### `FinderContextMenuAction`
Represents an actionable command dispatched from the macOS Finder right-click context menu.

| Field | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `actionIdentifier` | `String` | Yes | Unique action ID (e.g., `extract_here`, `compress_quick_7z`, `inspect_archive`) |
| `title` | `String` | Yes | Localized menu title displayed to the user |
| `iconSystemName` | `String` | Yes | SF Symbols icon name |
| `targetURLs` | `[String]` | Yes | Absolute POSIX file URLs selected in Finder |
| `isArchiveTarget` | `Bool` | Yes | Whether the targets are archive files vs uncompressed files |

### `QuickLookPreviewData`
Represents the lightweight metadata and directory tree payload rendered inside the Spacebar QuickLook popup.

| Field | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `archivePath` | `String` | Yes | Absolute path to the archive file |
| `format` | `String` | Yes | Format identifier (e.g., `ZIP`, `7Z`, `TAR.ZST`) |
| `uncompressedSizeBytes` | `Int64` | Yes | Total uncompressed payload size |
| `compressedSizeBytes` | `Int64` | Yes | Physical archive file size on disk |
| `compressionRatioPercent`| `Double` | Yes | Compression space savings percentage |
| `totalEntriesCount` | `Int` | Yes | Total count of files and directories |
| `isEncrypted` | `Bool` | Yes | Whether the archive payload or header is encrypted |
| `rootNodes` | `[QuickLookTreeNode]`| Yes | Hierarchical tree nodes for preview |

### `QuickLookTreeNode`
Represents a single file or directory row inside the QuickLook preview tree.

| Field | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `name` | `String` | Yes | File or folder name |
| `path` | `String` | Yes | Relative path within the archive |
| `isDirectory` | `Bool` | Yes | Directory flag |
| `sizeBytes` | `Int64` | Yes | File uncompressed size |
| `formattedSize` | `String` | Yes | Formatted size string (e.g. `12.4 MB`) |
| `children` | `[QuickLookTreeNode]?`| No | Child entries if directory |

---

## 2. Desktop GUI Localization Entities

### `AppLocalizationConfiguration`
Represents the user's active language and formatting preferences.

| Field | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `language` | `String` | Yes | Active language code (`system`, `zhHans`, `en`, `zhHant`, `ja`, `de`, `fr`, `es`) |
| `unitStandard` | `String` | Yes | Storage unit standard (`si_decimal` vs `iec_binary`) |
| `effectiveLanguage` | `String` | Yes | Computed effective language after system fallback resolution |

---

## 3. Fast LZMA2 Tuning Entities

### `LZMA2TuningMetrics`
Represents pre- and post-optimization performance and throughput benchmarks.

| Field | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `benchmarkName` | `String` | Yes | Benchmark identifier (e.g., `7Z Level 1 Compression`) |
| `preThroughputMBs` | `Double` | Yes | Baseline throughput before optimization (MB/s) |
| `postThroughputMBs` | `Double` | Yes | Measured throughput after optimization (MB/s) |
| `deltaPercent` | `Double` | Yes | Percentage change ($\Delta\%$) |
| `isRegression` | `Bool` | Yes | True if $\Delta < -3.0\%$ |
