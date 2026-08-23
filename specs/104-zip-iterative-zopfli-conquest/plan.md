# Implementation Plan: Feature 104 (ZIP Iterative Zopfli Conquest)

## Technical Context
- **Module**: `CTTZipBridge` & `TTZipCore/Zip`
- **Goal**: Implement in-process C multi-pass iterative dynamic Huffman re-weighting with Q8.8 fixed-point arithmetic, 18-core multi-block parallel scheduling, and 32KB boundary history warmup to achieve strict Pareto dominance in Tiers 6 and 7 over `pigz -11` and `advzip -4`.

## Constitution & Performance Check
- [x] Zero dynamic allocations in hot path (reusable thread-local context).
- [x] Fast-path bypass preserved for Tiers 0..5 (libdeflate direct parallel).
- [x] Zero regression on existing 13 performance gate suites.
- [x] 100% physically grounded and reproducible benchmark verification.

## Phase 0: Research Items
- - R001 [SUBAGENT:research] 《Zopfli 动态 Huffman 符号统计重加权与 DAG 最短路径迭代收敛算法》: Complete (see `research.md`).
- - R002 [SUBAGENT:research] 《2MB Tile Chunking 与 32KB 跨块滑动字典在 18 核并发下的无锁内存布局》: Complete (see `research.md`).

## Phase 1: Contracts & Data Models
- Data models: `data-model.md`
- Schema: `contracts/zopfli-engine.schema.json`
- Verification guide: `quickstart.md`

## Planned Code Changes
1. **[MODIFY] `Sources/CTTZipBridge/include/ttzip_zopfli_engine.h`**:
   - Declare thread context structures and iterative compression interfaces.
2. **[MODIFY] `Sources/CTTZipBridge/ttzip_zopfli_engine.c`**:
   - Implement fixed-point log2, dynamic Huffman tree re-weighting loop, decision hash convergence detection, and 32KB history warmup matching.
3. **[MODIFY] `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift`**:
   - Align 2MB tile multi-block parallel dispatch with 32KB sliding history pointer passing into C engine.
4. **[MODIFY] `Tests/TTZipTests/ZipMultiCoreParetoFrontierPkTests.swift`**:
   - Verify Pareto frontier generation across all 8 tiers.
