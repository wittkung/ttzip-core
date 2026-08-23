# Data Model: Multi-Core Optimization Breakdown (Spec 141)

**Feature**: `141-multicore-optimization-breakdown-and-empirical-validation`  
**Date**: 2026-08-20  

---

## 1. MultiCoreOptimizationPoint (Enumeration)

| Identifier | Enum Case | Layer | Description |
| :--- | :--- | :--- | :--- |
| `OP-1` | `tlsZeroLock` | Memory | C11 `_Thread_local` zero-lock codec caching vs mutex allocation |
| `OP-2` | `blockParallel512KB` | Codec | 512KB chunk concurrent Deflate compression vs single-core |
| `OP-3` | `multiTileDecompress` | Codec | Multi-tile chunk concurrent decompression vs sequential |
| `OP-4` | `containerMultiFilePack`| Container | Multi-file concurrent ZIP archive creation vs serial |
| `OP-5` | `containerMultiFileExtract`| Container| Multi-file concurrent direct extraction vs serial |
| `OP-6` | `pmullHardwareChecksum`| Hashing | ARMv8 PMULL hardware vector CRC32/64 vs software table |
| `OP-7` | `apfsDirectIOPrealloc` | I/O | APFS `fstore_t` preallocation vs unbuffered incremental write |
| `OP-8` | `topologyQoSScheduling`| Scheduling| Apple Silicon QoS compute/IO separation vs default queue |

---

## 2. OptimizationPointResult (Entity)

| Field Name | Type | Required | Constraints / Semantics |
| :--- | :--- | :--- | :--- |
| `pointId` | `string` | Yes | Format: `OP-[1-8]` |
| `pointName` | `string` | Yes | Human-readable title |
| `layer` | `string` | Yes | `Memory` \| `Codec` \| `Container` \| `Hashing` \| `I/O` \| `Scheduling` |
| `baselineThroughputMBs` | `number` | Yes | Float $\ge 0.0$ (MB/s) |
| `optimizedThroughputMBs`| `number` | Yes | Float $\ge 0.0$ (MB/s) |
| `speedupRatio` | `number` | Yes | Float $\ge 0.0$ ($\text{optimized} / \text{baseline}$) |
| `isPositiveDelta` | `boolean` | Yes | True if $\text{speedupRatio} > 1.0$ |
| `integrityPassed` | `boolean` | Yes | True if SHA-256 / CRC32 matches 100% |

---

## 3. MultiCoreBreakdownSummary (Entity)

| Field Name | Type | Required | Constraints / Semantics |
| :--- | :--- | :--- | :--- |
| `totalPoints` | `integer` | Yes | Exactly 8 |
| `passedCount` | `integer` | Yes | Count of points with `isPositiveDelta == true` |
| `averageSpeedup` | `number` | Yes | Geometric or arithmetic mean of `speedupRatio` |
| `allPositiveDelta` | `boolean` | Yes | True if `passedCount == totalPoints` |
| `results` | `array<OptimizationPointResult>` | Yes | Exactly 8 elements matching JSON Schema contract |
