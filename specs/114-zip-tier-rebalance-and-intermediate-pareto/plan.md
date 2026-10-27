# Implementation Plan: ZIP 8-Tier Rebalancing & Intermediate Pareto Frontier Bridge

**Feature**: [`specs/114-zip-tier-rebalance-and-intermediate-pareto/spec.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/114-zip-tier-rebalance-and-intermediate-pareto/spec.md)  
**Date**: 2026-08-19  
**Status**: Ready  

---

## 1. Technical Context

TTZip defines an 8-tier golden standard compression hierarchy (Tiers 0..7). Previously, Tier 1 (`fast`) and Tier 2 (`fastPlus`) both utilized greedy LZ77 matching, generating identical compressed sizes (~3.96 MB on 100MB corpus) and overlapping points on Pareto frontier curves. Concurrently, a significant 210x throughput cliff existed between Tier 3 (Maximum: 3.23 MB @ 4.28 GB/s) and Tier 5 (Graph Fast: 2.87 MB @ 20.4 MB/s).

This plan rebalances the 8 standard tiers by eliminating legacy Tier 2 redundancy, promoting legacy Tier 3 (`normal`) to Tier 2 and legacy Tier 4 (`maximum`) to Tier 3, and creating a new Tier 4 (High Compression) powered by `libdeflate Level 12` near-optimal dynamic programming.

---

## 2. Constitution & Rules Check

- **Hot-Path Zero Allocation**: All profiles use static structures and thread-local buffers.
- **Fast-Path Bypass**: `ttzip_zopfli_engine.c` dispatches Tier 4 (`target_level == 4`) directly to `ttzip_libdeflate_compress(..., 12)` bypassing Zopfli iterative loops.
- **Single Source of Truth**: All tier parameters are defined centrally in `ZipCompressionProfile.swift`.
- **Zero Configuration Creep**: Standard tiers are fixed at 8 (0..7), maintaining UI and CLI simplicity.

---

## 3. Phase 0: Grounded Research Index

- - R001 [SUBAGENT:research] 《新 Tier 4 (High Ratio) 算法选型与吞吐边界实测分析》：在 3.23 MB 与 2.87 MB 之间，通过 libdeflate L12 精准定位 3.06 MB @ 195 MB/s 的最优实现路径。（已完成，见 [`research.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/114-zip-tier-rebalance-and-intermediate-pareto/research.md)）
- - R002 [SUBAGENT:research] 《8 大黄金标准档位在 Swift 与 C 调度层的重构与无缝对接》：全链路审计 `ZipCompressionProfile`、`ArchiveWriter+Dispatch`、`ZipExtremeBlockWriter` 与 C 桥接层分发。（已完成，见 [`research.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/114-zip-tier-rebalance-and-intermediate-pareto/research.md)）

---

## 4. Phase 1: Design Artifacts Index

- **Data Model**: [`data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/114-zip-tier-rebalance-and-intermediate-pareto/data-model.md)
- **JSON Schema Contract**: [`contracts/zip_compression_profile_schema.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/114-zip-tier-rebalance-and-intermediate-pareto/contracts/zip_compression_profile_schema.json)
- **Quickstart Guide**: [`quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/114-zip-tier-rebalance-and-intermediate-pareto/quickstart.md)

---

## 5. Component Modification Manifest

### 1. `Sources/TTZipCore/Zip/ZipCompressionProfile.swift`
- Update `ZipCompressionProfile` static presets:
  - `store`: Tier 0 (`.store`, 0)
  - `fast`: Tier 1 (`.level1`, 1)
  - `normal`: Tier 2 (`.level2`, 2, `deflateLevel = 2`)
  - `maximum`: Tier 3 (`.level3`, 6, `deflateLevel = 6`)
  - `high`: Tier 4 (`.level4`, 12, `deflateLevel = 12`) — *NEW*
  - `graphFast`: Tier 5 (`.level5`, 12, `zopfliIterations = 2`)
  - `ultraZopfli`: Tier 6 (`.level6`, 12, `zopfliIterations = 5`)
  - `extremePeak`: Tier 7 (`.level7`, 12, `zopfliIterations = 15`, `blockSplitting = true`)
- Maintain backward compatibility alias `public static let fastPlus = normal`.

### 2. `Sources/TTZipCore/ArchiveWriter+Dispatch.swift`
- Ensure single large file extreme block compression router handles `.level4`, `.level5`, `.level6`, `.level7`.

### 3. `Sources/CTTZipBridge/ttzip_zopfli_engine.c`
- In `ttzip_zopfli_init_options`: Update mapping so `level == 4` maps to L12, `level == 5` maps to `iterations = 2`.
- In `ttzip_zopfli_compress_block_with_history`: Ensure `target_level == 4` or `target_level == 12` fast-paths to `ttzip_libdeflate_compress(..., 12)`.

### 4. `Sources/TTZipApp/Views/Components/CompressIntegratedConfigSectionView.swift`
- Align UI tier titles and throughput indicators to the 8 standard profiles.

### 5. `Tests/TTZipTests/ZipSingleCoreParetoFrontierPkTests.swift` & `ZipMultiCoreParetoFrontierPkTests.swift`
- Align all 8 profiles and benchmark runners with streaming `TestTerminalRenderer` output.
