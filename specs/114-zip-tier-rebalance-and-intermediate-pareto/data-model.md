# Data Model: ZIP 8-Tier Rebalancing & Intermediate Pareto Frontier

**Feature**: [`specs/114-zip-tier-rebalance-and-intermediate-pareto/spec.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/114-zip-tier-rebalance-and-intermediate-pareto/spec.md)  
**Date**: 2026-08-19  
**Status**: Completed  

---

## 1. Entities & Structures

### `ZipCompressionProfile`
Represents the strongly-typed execution profile for ZIP Deflate compression.

| Field Name | Type | Required | Description | Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `id` | `string` | Yes | Unique profile identifier | e.g. `"zip_tier_4_high"` |
| `name` | `string` | Yes | Human-readable title | e.g. `"High (4)"` |
| `level` | `ArchiveCompressionLevel` | Yes | Abstract compression level enum | `.store`, `.level1` .. `.level7` |
| `deflateLevel` | `int32` | Yes | Low-level C Deflate engine level | `0..12` |
| `zopfliIterations` | `int32` | Yes | Graph shortest path iteration count | `0..15` |
| `blockSplitting` | `bool` | Yes | Whether dynamic block splitting is enabled | `true` / `false` |
| `maxBlockSplits` | `int32` | Yes | Maximum dynamic block split points | `0..15` |
| `earlyExitThreshold` | `double` | Yes | Cost convergence early termination ratio | `0.0 .. 1.0` |
| `targetThroughputFloorMBs` | `double` | Yes | Apple Silicon throughput floor (MB/s) | `>= 0.0` |

---

## 2. The 8 Golden Standard Presets Definition Matrix

| Tier Index | Enum Level | Identifier | Name | `deflateLevel` | `zopfliIterations` | `blockSplitting` | `maxBlockSplits` | Target Floor (MB/s) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **0** | `.store` | `zip_tier_0_store` | Store (0) | 0 | 0 | false | 0 | 6,000.0 |
| **1** | `.level1` | `zip_tier_1_fast` | Fast (1) | 1 | 0 | false | 0 | 5,000.0 |
| **2** | `.level2` | `zip_tier_2_normal` | Normal (2) | 2 | 0 | false | 0 | 4,500.0 |
| **3** | `.level3` | `zip_tier_3_maximum` | Maximum (3) | 6 | 0 | false | 0 | 2,500.0 |
| **4** | `.level4` | `zip_tier_4_high` | High (4) | 12 | 0 | false | 0 | 150.0 |
| **5** | `.level5` | `zip_tier_5_graph_fast` | Graph Fast (5) | 12 | 2 | false | 0 | 20.0 |
| **6** | `.level6` | `zip_tier_6_ultra_zopfli` | Ultra Zopfli (6) | 12 | 5 | false | 0 | 4.0 |
| **7** | `.level7` | `zip_tier_7_extreme_peak`| Extreme Peak (7) | 12 | 15 | true | 15 | 0.25 |

---

## 3. C Structure Mapping: `TTZipZopfliOptions`

```c
typedef struct {
    int compression_level;     /* 0..12 mapping directly to deflateLevel */
    int num_iterations;        /* 0..15 mapping directly to zopfliIterations */
    int block_splitting;       /* 0 or 1 mapping to blockSplitting */
    int max_block_splits;      /* 0..15 mapping to maxBlockSplits */
    double early_exit_threshold; /* 0.0001 mapping to earlyExitThreshold */
} TTZipZopfliOptions;
```
