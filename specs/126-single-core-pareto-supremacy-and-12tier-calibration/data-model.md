# Phase 1 Data Model: Single-Core 12-Tier Deflate Calibration and Full Pareto Frontier Supremacy

**Feature Directory**: `specs/126-single-core-pareto-supremacy-and-12tier-calibration`  
**Date**: 2026-08-19  
**Status**: Ready

---

## 1. Core Data Entities

### 1.1 `DeflateTierConfiguration` (12-Tier Monotonic Calibration Matrix)

Represents the exact algorithmic configuration for each Deflate compression level ($1 \le k \le 12$).

| Field Name | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `level` | `Int32` | $1 \le 	ext{level} \le 12$ | The compression level identifier |
| `matchFinderType` | `String` | Enum: `fast_hybrid_hash34`, `fast_greedy_hash4`, `compact_4way_lazy`, `compact_chain_lazy`, `deep_chain_lazy`, `zopfli_dag` | The underlying matchfinder engine |
| `maxChainDepth` | `UInt32` | $0 \le 	ext{depth} \le 256$ | Maximum chain probes per match lookup (0 for direct hash) |
| `niceMatchLength` | `UInt32` | $32 \le 	ext{len} \le 258$ | Early termination match length threshold |
| `lookaheadSteps` | `UInt32` | $0 \le 	ext{steps} \le 2$ | Lookahead evaluation depth ($0=$ Greedy, $1=$ 1-Step, $2=$ 2-Step) |
| `zopfliIterations` | `UInt32` | $0 \le 	ext{iter} \le 30$ | Number of dynamic programming cost refinement passes |
| `enableBlockSplitting` | `Boolean` | `true` / `false` | Whether recursive dynamic block splitting is active |
| `targetThroughputMinMBs` | `Double` | $> 0.0$ | Minimum expected physical throughput floor |
| `maxExpectedSizeEnwik8MB` | `Double` | $> 0.0$ | Maximum expected output size on enwik8 100MB |

---

### 1.2 `MatchfinderMemoryStructures` (C Bridge Structures)

Represents the L1-cache resident memory structures used in `Sources/CTTZipBridge/native_deflate/`.

#### A. `ttzip_deflate_hybrid_fast_mf_t` (Level 1: 128 KB Total)
```c
typedef struct {
    uint16_t hash3_tab[32768];    /* 64 KB: Direct 1-way lookup for 3-byte tokens */
    uint16_t hash4_tab[16384][2]; /* 64 KB: 2-way bucket table for 4+ byte sequences */
    uint32_t base_offset;         /* Rolling base offset for zero-cost rebasing */
} ttzip_deflate_hybrid_fast_mf_t;
```

#### B. `ttzip_deflate_4way_lazy_mf_t` (Level 3 & 4: 64 KB Total)
```c
typedef struct {
    uint16_t hash_tab[8192][4];   /* 64 KB: 4-way compact bucket table (16-bit relative offsets) */
    uint32_t base_offset;         /* Rolling base offset for zero-cost rebasing */
} ttzip_deflate_4way_lazy_mf_t;
```

#### C. `ttzip_deflate_chain_lazy_mf_t` (Level 5 ~ 9: 192 KB Total)
```c
typedef struct {
    uint16_t head_tab[32768];     /* 64 KB: Head index per hash bucket */
    uint16_t prev_tab[65536];     /* 128 KB: Linked-list chain index pointers */
    uint32_t base_offset;         /* Rolling base offset */
} ttzip_deflate_chain_lazy_mf_t;
```

---

### 1.3 `ParetoPointRecord` (Benchmark Metric Record)

| Field Name | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `toolId` | `String` | Non-empty | Unique engine identifier (e.g. `ttzip_1core_l4`) |
| `algorithm` | `String` | Non-empty | Algorithm display name (e.g. `TTZip L4 (Normal)`) |
| `family` | `String` | Enum: `ttzip`, `libdeflate`, `sevenZip`, `appleNative`, `minizipNg`, `pigz` | Software family cluster |
| `level` | `Int` | $\ge 0$ | Engine compression level |
| `throughputMBs` | `Double` | $> 0.0$ | Single-core physical throughput in MB/s |
| `compressedBytes` | `Int64` | $> 0$ | Exact compressed file size in bytes |
| `spaceSavingsPct` | `Double` | $0.0 \le 	ext{pct} \le 100.0$ | $(1.0 - 	ext{compressedBytes} / 	ext{originalBytes}) 	imes 100.0$ |
| `corpusId` | `String` | Enum: `enwik8_100mb`, `mixed_compound100mb`, `structured_json100mb` | Benchmark dataset identifier |

---

### 1.4 `ParetoDuelResultSet` (Head-to-Head Comparative Result)

| Field Name | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `corpusId` | `String` | Non-empty | Evaluated dataset identifier |
| `datasetSha256` | `String` | 64-char hex string | SHA-256 digest of input corpus |
| `ttzipPoints` | `Array<ParetoPointRecord>` | Exactly 12 points ($L_1 \sim L_{12}$) | TTZip measured data points |
| `competitorPoints` | `Array<ParetoPointRecord>` | $\ge 1$ point | Competitor baseline data points |
| `isMonotonic` | `Boolean` | `true` | Assertion that $	ext{Size}(L_{k+1}) < 	ext{Size}(L_k)$ for all $1 \le k \le 11$ |
| `paretoDominationScore` | `Double` | $0.0 \le 	ext{score} \le 1.0$ | Ratio of competitor points dominated or matched by TTZip |
| `chartExportPath` | `String` | Valid absolute path | Path to exported 2x retina PNG chart |

---

## 2. Invariants and Business Rules

1. **Strict Ratio Monotonicity Invariant**:
   $$orall k \in [1, 11]: \quad 	ext{CompressedBytes}(L_{k+1}) < 	ext{CompressedBytes}(L_k)$$
   Under no circumstances may $L_{k+1}$ produce a file size greater than or equal to $L_k$.
2. **Speed-Ratio Trade-off Invariant**:
   $$orall k \in [1, 11]: \quad 	ext{ThroughputMBs}(L_k) > 	ext{ThroughputMBs}(L_{k+1})$$
3. **Zero-Heap Allocation Invariant**:
   All matchfinder structures (`ttzip_deflate_hybrid_fast_mf_t`, `ttzip_deflate_4way_lazy_mf_t`, etc.) must be allocated as thread-local static variables or on the stack. No `malloc` or `free` calls allowed inside block compression loops.
