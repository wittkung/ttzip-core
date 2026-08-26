# Data Model & Schema Definitions (Feature 016)

**Feature**: 100% Grand Slam Win Rate Across All 16 Archive Formats  
**Directory**: `specs/016-all-16-formats-100-percent-grand-slam/`

---

## 1. Entity: `FormatBenchmarkMatchup`

Represents an individual 1v1 PK benchmark scenario comparing TTZip against a competitor CLI.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `format` | `String` (Enum) | Yes | Archive format (e.g. `"7z"`, `"zip"`, `"tar"`, `"tar.zst"`, `"brotli"`) |
| `dimensionName` | `String` | Yes | Name of dataset scenario (e.g. `"500MB 大文件数据块 (500MB)"`) |
| `level` | `Integer` | Yes | Compression level integer (1..9) |
| `isEncrypted` | `Boolean` | Yes | Whether archive is AES encrypted |
| `datasetSizeBytes` | `Integer` | Yes | Uncompressed original byte count |
| `ttzipCompressMBs` | `Number` | Yes | TTZip compression throughput in MB/s |
| `ttzipExtractMBs` | `Number` | Yes | TTZip decompression throughput in MB/s |
| `competitorName` | `String` | Yes | Name of competitor CLI (e.g. `"7-Zip 7zz CLI"`, `"brotli CLI"`) |
| `competitorCompressMBs` | `Number` | Yes | Competitor compression throughput in MB/s |
| `competitorExtractMBs` | `Number` | Yes | Competitor decompression throughput in MB/s |
| `compressSpeedup` | `Number` | Yes | Ratio of `ttzipCompressMBs / competitorCompressMBs` |
| `extractSpeedup` | `Number` | Yes | Ratio of `ttzipExtractMBs / competitorExtractMBs` |
| `isDominant` | `Boolean` | Yes | `true` if TTZip wins both compression & decompression |

---

## 2. Entity: `BenchmarkSummaryReport`

Represents the aggregated matrix results of a full 16-format benchmark run.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `timestamp` | `String` | Yes | ISO 8601 timestamp string |
| `totalMatchups` | `Integer` | Yes | Total number of PK matchups (e.g. 280) |
| `ttzipWins` | `Integer` | Yes | Count of matchups won by TTZip |
| `ttzipLosses` | `Integer` | Yes | Count of matchups lost by TTZip |
| `winRatePercent` | `Number` | Yes | Overall win percentage (`ttzipWins / totalMatchups * 100`) |
| `results` | `Array<FormatBenchmarkMatchup>` | Yes | Array of individual matchup records |

---

## 3. Entity: `PerformanceRegressionAudit`

Represents a zero-regression audit diff comparing previous and latest benchmark JSON reports.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `auditTimestamp` | `String` | Yes | ISO 8601 timestamp string |
| `baselineFile` | `String` | Yes | File path of baseline benchmark report JSON |
| `latestFile` | `String` | Yes | File path of latest benchmark report JSON |
| `improvedCount` | `Integer` | Yes | Count of scenarios with > +3.0% speedup |
| `neutralCount` | `Integer` | Yes | Count of scenarios within [-3.0%, +3.0%] |
| `regressedCount` | `Integer` | Yes | Count of scenarios with < -3.0% degradation |
| `maxRegressionPercent` | `Number` | Yes | Worst regression percentage recorded |
| `isPassed` | `Boolean` | Yes | `true` if max regression < 3.0% and no hard regressions |
