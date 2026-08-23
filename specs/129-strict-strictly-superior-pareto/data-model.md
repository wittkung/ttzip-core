# Data Model: Strict Dual-Axis Pareto Superiority

## 1. `StrictDualAxisSuperiorityReport`

| Field | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `featureId` | String (`"129-strict-strictly-superior-pareto"`) | Yes | Feature identifier |
| `datasetName` | String Enum | Yes | One of the 4 standard 100MB benchmark corpora |
| `datasetSizeBytes` | Integer | Yes | Total input size in bytes |
| `totalCompetitorPoints` | Integer | Yes | Evaluated competitor points count |
| `strictlySuperiorPoints` | Integer | Yes | Number of points with $S_{\text{TTZip}} > S_{\text{lib}} \land \text{Size}_{\text{TTZip}} < \text{Size}_{\text{lib}}$ |
| `superiorityPercentage` | Float | Yes | `(strictlySuperiorPoints / totalCompetitorPoints) * 100.0` |
| `evaluations` | Array<`StrictEvaluation`> | Yes | Level-by-level dual superiority verification records |

## 2. `StrictEvaluation`

| Field | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `competitorLevel` | Integer (1..12) | Yes | Competitor level |
| `competitorThroughputMBs` | Float | Yes | Throughput in MB/s |
| `competitorCompressedSizeBytes` | Integer | Yes | Compressed size in bytes |
| `superiorTTZipTier` | Integer (0..15) | Yes | TTZip tier achieving dual superiority |
| `ttzipThroughputMBs` | Float | Yes | TTZip throughput in MB/s |
| `ttzipCompressedSizeBytes` | Integer | Yes | TTZip compressed size in bytes |
| `speedAdvantageRatio` | Float ($> 1.00$) | Yes | `ttzipThroughputMBs / competitorThroughputMBs` |
| `sizeAdvantageRatio` | Float ($> 1.00$) | Yes | `competitorCompressedSizeBytes / ttzipCompressedSizeBytes` |
| `isStrictlySuperior` | Boolean (`true`) | Yes | Verified strictly faster and strictly smaller |
