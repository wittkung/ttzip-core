# Data Model: TurboBench 4D Architecture Evolution Suite

**Feature**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/086-turbobench-architecture-evolution/spec.md)  
**Status**: Ready for Implementation  

---

## 1. Entity Definitions

### 1.1 ParetoPoint
Represents a single benchmark candidate in 2D trade-off space.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `id` | `String` | Yes | Unique identifier (e.g. "zstd_level_3") |
| `algorithm` | `String` | Yes | Display name of the algorithm (e.g. "Zstandard") |
| `level` | `Int` | Yes | Compression level ($1 \dots 22$) |
| `throughputMBs` | `Double` | Yes | Measured throughput in decimal MB/s |
| `spaceSavingsPct` | `Double` | Yes | Space savings percentage ($0.0 \dots 100.0\%$) |
| `compressedBytes` | `Int64` | Yes | Output payload size in bytes |
| `uncompressedBytes` | `Int64` | Yes | Original source size in bytes |
| `paretoRank` | `Int` | Yes | Non-dominated rank ($1 = \text{Frontier}, 2, \dots$) |
| `isParetoOptimal` | `Bool` | Yes | `true` if `paretoRank == 1` |
| `isOnConvexEnvelope`| `Bool` | Yes | `true` if on the upper convex envelope (supporting scalar weights) |

---

### 1.2 ParetoFrontierResult
Encapsulates the complete dataset of evaluated benchmark points and extracted Pareto subsets.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `totalPointsEvaluated`| `Int` | Yes | Count of all candidate points |
| `frontierPoints` | `[ParetoPoint]` | Yes | Ordered non-dominated Rank 1 Pareto points (sorted by throughput ascending) |
| `convexEnvelopePoints` | `[ParetoPoint]` | Yes | Ordered subset of points forming the upper convex hull |
| `allPoints` | `[ParetoPoint]` | Yes | Complete set of evaluated points with assigned ranks |
| `generatedAt` | `String` | Yes | ISO-8601 UTC timestamp |

---

### 1.3 TransferSpeedTier
Defines media bandwidth specifications and calculated end-to-end turnaround latency.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `tierName` | `String` | Yes | Medium name (e.g. "10Gbps LAN", "1Gbps LAN / 5G", "NVMe SSD", "Cloud WAN") |
| `bandwidthMBs` | `Double` | Yes | Nominal bandwidth in decimal MB/s |
| `rawTransferSeconds`| `Double` | Yes | Turnaround time without compression ($S_{\text{raw}} / V_{\text{media}}$) |
| `compressionSeconds`| `Double` | Yes | Time to compress on host |
| `compressedTransferSeconds`| `Double` | Yes | Time to transfer compressed bytes over media |
| `decompressionSeconds`| `Double` | Yes | Time to decompress on destination |
| `totalTurnaroundSeconds`| `Double` | Yes | $T_{\text{comp}} + T_{\text{transfer}} + T_{\text{decomp}}$ |
| `speedupRatio` | `Double` | Yes | $T_{\text{uncompressed}} / T_{\text{total}}$ |
| `isParetoWinner` | `Bool` | Yes | `true` if this algorithm achieves minimal total turnaround time in this tier |

---

### 1.4 TransferSpeedReport
Encapsulates the complete multi-tier media turnaround projection matrix for an algorithm benchmark run.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `sourceSizeBytes` | `Int64` | Yes | Original payload size |
| `algorithm` | `String` | Yes | Algorithm name |
| `level` | `Int` | Yes | Compression level |
| `tiers` | `[TransferSpeedTier]` | Yes | Projections across Cloud WAN, 1G, 10G, NVMe |
| `overallBestTierCount` | `Int` | Yes | Count of media tiers where this algorithm won Rank 1 |

---

### 1.5 ScenarioRecommendation
Represents an automated, scenario-driven codec recommendation.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `scenario` | `String` | Yes | Scenario name ("Instant Transfer", "Balanced Daily", "Cold Storage") |
| `measuredEntropy` | `Double` | Yes | Shannon entropy in bits/byte ($0.0 \dots 8.0$) |
| `trialCompressibilityRatio`| `Double` | Yes | 64KB strided trial compression ratio ($0.0 \dots 1.0$) |
| `recommendedAlgorithm` | `String` | Yes | Algorithm name (e.g. "Zstandard", "7Z-LZMA2", "Store") |
| `recommendedLevel` | `Int` | Yes | Recommended level (e.g. 1, 3, 9) |
| `rationale` | `String` | Yes | Human-readable explanation of why this was chosen |
| `projectedThroughputMBs`| `Double` | Yes | Expected throughput on host hardware |
| `projectedSpaceSavingsPct`| `Double` | Yes | Expected space savings percentage |
| `probeDurationMs` | `Double` | Yes | Total time taken by entropy and trial probers |

---

## 2. Invariants & Boundaries

1. **Space Savings Range**: $s \in (-\infty, 100.0\%]$. For uncompressible/expanding data, $s < 0.0\%$ is valid and clamped to visual margins in plots.
2. **Throughput Positive Lower Bound**: $v \ge 1.0\text{ MB/s}$ to guarantee valid logarithmic projection $\log_{10}(v) \ge 0$.
3. **Pareto Dominance Reflexivity**: A point never dominates itself. Ranks are positive integers $\ge 1$.
4. **Zero Bare Objects**: All Codable structures serialize to strict JSON without unconstrained polymorphic object maps.
