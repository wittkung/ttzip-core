# Phase 1 Data Model: Performance Benchmarking & Documentation Architecture

**Feature**: `073-performance-benchmarking-and-readme-reconstruction`
**Date**: 2026-08-18

---

## 1. Entities & Schema Definitions

### Entity: BenchmarkMeasurementRecord
Represents an empirical benchmark measurement captured under monotonic timing across a specific format, workload, and compression level.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `id` | `string` | Yes | Unique measurement identifier (e.g. `bench-zip-lvl1-smallfiles`) |
| `format` | `string` (enum: `zip`, `7z`, `tar`, `tar.zst`, `tar.gz`, `tar.bz2`, `tar.xz`, `wim`, `dmg`, `lz4`, `lzip`, `lrzip`, `aar`, `iso`, `brotli`, `snappy`) | Yes | Archive format tested |
| `compressionLevel` | `integer` (1..9) | Yes | Level of compression |
| `encryptionMode` | `string` (enum: `none`, `aes256`, `zipcrypto`) | Yes | Encryption algorithm used |
| `workloadType` | `string` (enum: `massive_small_files`, `log_text`, `high_entropy_binary`, `large_block_stream`) | Yes | Input workload classification |
| `inputSizeBytes` | `integer` | Yes | Uncompressed input size in bytes |
| `compressedSizeBytes` | `integer` | Yes | Resulting compressed archive size in bytes |
| `compressionRatioPercent` | `number` | Yes | Compressed size / uncompressed size * 100 |
| `packingThroughputMBs` | `number` | Yes | Monotonic packing throughput in MB/s |
| `extractionThroughputMBs` | `number` | Yes | Monotonic extraction throughput in MB/s |
| `competitorTool` | `string` | Yes | Baseline competitor tool name (e.g. `7-Zip 7zz (Max Multithread)`) |
| `competitorPackingMBs` | `number` | Yes | Competitor packing throughput in MB/s |
| `competitorExtractionMBs` | `number` | Yes | Competitor extraction throughput in MB/s |
| `packingSpeedupMultiplier` | `number` | Yes | TTZip packing throughput / competitor packing throughput |
| `extractionSpeedupMultiplier` | `number` | Yes | TTZip extraction throughput / competitor extraction throughput |

---

### Entity: FormatSupportCapability
Represents a format specification and TTZip engine support status.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `formatExtension` | `string` | Yes | Primary extension (e.g. `.zip`, `.tar.zst`) |
| `category` | `string` (enum: `primary_modern`, `high_compression`, `real_time`, `disk_image`, `multi_volume`, `legacy_read`) | Yes | Grouping classification |
| `compressionSupported` | `boolean` | Yes | True if TTZip can create/pack this format |
| `decompressionSupported` | `boolean` | Yes | True if TTZip can unpack/extract this format |
| `quickLookSupported` | `boolean` | Yes | True if TTZipApp can preview in-place without full extraction |
| `multiVolumeSupported` | `boolean` | Yes | True if multi-part split volume handling is supported |
| `underlyingEngine` | `string` | Yes | In-process C library (e.g. `libdeflate`, `zstd`, `LZMA SDK`, `libarchive`) |
| `governingStandard` | `string` | Yes | Reference standard (e.g. `PKWARE APPNOTE`, `RFC 8878`, `POSIX.1 Pax`) |

---

### Entity: DocumentationSection
Represents a mandatory section within the reconstructed `README.md` and documentation suite.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `sectionId` | `string` | Yes | Section identifier (e.g. `hero`, `cli_guide`, `benchmark_summary`) |
| `heading` | `string` | Yes | Markdown heading text |
| `mandatoryLinks` | `array<string>` | Yes | List of relative file links required in this section |
| `contentSummary` | `string` | Yes | Technical focus and invariants of the section |

---

### Entity: LicensePolicyDefinition
Defines the terms of the Source-Available & Anti-Copycat License.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `licenseName` | `string` | Yes | Official name (`TTZip Source-Available & Anti-Copycat License v1.0`) |
| `spdxIdentifier` | `string` | Yes | `LicenseRef-TTZip-Source-Available-1.0` |
| `permittedUses` | `array<string>` | Yes | Permitted non-commercial operations (inspect, audit, local CLI, personal app, PRs) |
| `prohibitedActions` | `array<string>` | Yes | Prohibited actions (app store publishing, white-label copycats, ad bundling, resale) |
| `enterpriseLicenseRequired` | `boolean` | Yes | True for commercial corporate / cloud deployments |
| `upstreamCarveOutExemption` | `string` | Yes | Explicit permissive licensing for contributions to upstream foundations |
