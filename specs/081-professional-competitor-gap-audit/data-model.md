# Data Model: 081-professional-competitor-gap-audit

**Feature**: TTZip 对标顶级专业归档软件全维度差距审计与深度能力补齐  
**Spec Reference**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/081-professional-competitor-gap-audit/spec.md)  
**Date**: 2026-08-18  

---

## 1. Domain Entities & Type Definitions

### 1.1 `SplitVolumeConfig` (分卷归档配置)
Represents configuration for creating spanned/multi-volume archives.

| Field | Type | Description | Required |
| :--- | :--- | :--- | :--- |
| `volumeSizeBytes` | `Int64` | Target chunk size in bytes ($\ge 65536$) | Yes |
| `preset` | `VolumePreset` | Predefined enum: `cd700MB`, `dvd4700MB`, `fat32_4GB`, `email25MB`, `wechat100MB`, `custom` | Yes |
| `namingPattern` | `String` | Pattern enum: `numberedExtension` (`.7z.001`), `pkzipSpanned` (`.z01`), `rawSplit` (`.part1.rar`) | Yes |
| `cleanOnFailure` | `Bool` | Whether to purge generated volumes on failure | Yes |

```swift
public enum VolumePreset: String, Sendable, Codable, CaseIterable {
    case cd700MB = "cd_700mb"
    case dvd4700MB = "dvd_4700mb"
    case fat32_4GB = "fat32_4gb"
    case email25MB = "email_25mb"
    case wechat100MB = "wechat_100mb"
    case custom = "custom"
}

public enum VolumeNamingPattern: String, Sendable, Codable {
    case numberedExtension = "numbered_extension" // .7z.001, .zip.001, .tar.001
    case pkzipSpanned = "pkzip_spanned"           // .z01, .z02, .zip
    case rawSplit = "raw_split"                   // .001, .002
}
```

---

### 1.2 `RecoveryRecordPayload` (恢复记录元数据)
Represents the forward error correction (FEC) recovery record embedded in the archive.

| Field | Type | Description | Required |
| :--- | :--- | :--- | :--- |
| `recoveryPercent` | `Double` | Configured redundancy percentage ($1.0 \le p \le 30.0$) | Yes |
| `sliceSizeBytes` | `Int` | Size of each Cauchy RS slice in bytes (typically 65536) | Yes |
| `dataSlicesCount` | `Int` | Number of source data slices $K$ | Yes |
| `paritySlicesCount` | `Int` | Number of parity slices $M$ | Yes |
| `protectedPayloadLength`| `Int64` | Byte length of original archive payload protected by FEC | Yes |
| `rootChecksum` | `String` | BLAKE3 / SHA-256 hex string of protected payload | Yes |
| `eccAlgorithm` | `String` | Enum: `cauchy_rs_gf16` | Yes |

---

### 1.3 `ArchiveSearchQuery` & `ArchiveSearchResult` (穿透搜索查询与结果)
Represents fast in-memory search parameters and filtered items.

| Field | Type | Description | Required |
| :--- | :--- | :--- | :--- |
| `queryText` | `String` | Search keyword or regular expression pattern | Yes |
| `isRegex` | `Bool` | Whether `queryText` is a regular expression | Yes |
| `caseSensitive` | `Bool` | Whether search is case-sensitive | Yes |
| `minSizeBytes` | `Int64?` | Optional minimum uncompressed size filter | No |
| `maxSizeBytes` | `Int64?` | Optional maximum uncompressed size filter | No |
| `fileExtensions` | `[String]?` | Optional extension whitelist (e.g. `["png", "jpg"]`) | No |

| Field | Type | Description | Required |
| :--- | :--- | :--- | :--- |
| `matchedIndices` | `[Int32]` | Offsets of matching entries in columnar index | Yes |
| `matchedEntriesCount`| `Int` | Number of matching entries | Yes |
| `totalScannedEntries`| `Int` | Total entries scanned in archive | Yes |
| `searchDurationMs` | `Double` | Duration of search scan in milliseconds | Yes |

---

### 1.4 `HardwareBenchmarkMetric` (7-Zip 对齐 MIPS 基准结果)
Represents real-time hardware telemetry and compression/decompression MIPS ratings.

| Field | Type | Description | Required |
| :--- | :--- | :--- | :--- |
| `dictionarySizeMB` | `Int` | Benchmark dictionary size in MB (32, 64, 128, 256) | Yes |
| `threadCount` | `Int` | Number of parallel worker threads | Yes |
| `compressMIPS` | `Double` | Compressing MIPS rating (7-Zip standardized) | Yes |
| `decompressMIPS` | `Double` | Decompressing MIPS rating (7-Zip standardized) | Yes |
| `totalMIPS` | `Double` | Overall combined MIPS rating | Yes |
| `compressSpeedMBs` | `Double` | Compression physical throughput in MB/s | Yes |
| `decompressSpeedMBs`| `Double` | Decompression physical throughput in MB/s | Yes |
| `cpuUsagePercent` | `Double` | Measured CPU core utilization percentage ($0.0 \sim 100.0 \times \text{cores}$) | Yes |
| `ratingPerUsageMIPS`| `Double` | Normalized energy/usage efficiency score | Yes |

---

## 2. Invariant & Contract Mapping

All models have a 1:1 corresponding JSON schema under `contracts/` with `$schema: "http://json-schema.org/draft-07/schema#"` and zero unconstrained objects.
