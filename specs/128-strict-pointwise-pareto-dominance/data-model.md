# Data Model: Strict Pointwise Pareto Dominance

## 1. `PointwiseParetoDominanceReport`

| Field | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `featureId` | String (`"128-strict-pointwise-pareto-dominance"`) | Yes | Unique Feature identifier |
| `datasetName` | String Enum | Yes | One of the 4 standard benchmark corpora names |
| `datasetSizeBytes` | Integer | Yes | Total uncompressed input bytes |
| `totalCompetitorPoints` | Integer | Yes | Count of evaluated competitor levels |
| `strictlyDominatedPoints` | Integer | Yes | Count of competitor points with a TTZip point in upper-right quadrant |
| `dominancePercentage` | Float | Yes | `(strictlyDominatedPoints / totalCompetitorPoints) * 100.0` |
| `evaluations` | Array<`PointwiseEvaluation`> | Yes | Individual level-by-level comparison records |

## 2. `PointwiseEvaluation`

| Field | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `competitorName` | String | Yes | Competitor identifier (e.g. `"libdeflate"`) |
| `competitorLevel` | Integer | Yes | Competitor compression level (1..12) |
| `competitorThroughputMBs` | Float | Yes | Throughput in MB/s |
| `competitorCompressedSizeBytes`| Integer | Yes | Output size in bytes |
| `dominatingTTZipTier` | Integer | Yes | Selected TTZip tier (0..12) |
| `ttzipThroughputMBs` | Float | Yes | TTZip throughput in MB/s |
| `ttzipCompressedSizeBytes` | Integer | Yes | TTZip output size in bytes |
| `speedAdvantageRatio` | Float | Yes | `ttzipThroughputMBs / competitorThroughputMBs` ($\ge 1.0$) |
| `sizeAdvantageRatio` | Float | Yes | `competitorCompressedSizeBytes / ttzipCompressedSizeBytes` ($\ge 1.0$) |
| `isStrictlyDominated` | Boolean | Yes | `speedAdvantageRatio >= 1.0 && sizeAdvantageRatio >= 1.0` |
