# Implementation Plan: Single-Core 12-Tier Deflate Calibration and Full Pareto Frontier Supremacy

**Branch**: `126-single-core-pareto-supremacy-and-12tier-calibration` | **Date**: 2026-08-19 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/126-single-core-pareto-supremacy-and-12tier-calibration/spec.md`

---

## Summary

This feature resolves all non-Pareto optimal deficiencies in TTZip's single-core Deflate engine:
1. Eliminates the mid-tier vacuum collapse between $L_2$ (1.02 GB/s) and $L_3$ (17.3 MB/s) by implementing an ARM64 NEON 4-way compact lazy matcher (`HT-4`) operating at $\ge 800	ext{ MB/s}$ and producing $\le 3.20	ext{ MB}$ (beating `libdeflate L6` at 721.8 MB/s, 3.21 MB).
2. Eliminates internal self-domination, ratio inversion, and ratio plateaus across $L_3 \sim L_9$ by calibrating a mathematically strictly monotonic 12-tier parameter matrix ($	ext{Size}(L_{k+1}) < 	ext{Size}(L_k)$).
3. Slashes JSON/text token overhead by introducing an L1-resident 128KB hybrid 3-byte direct / 4-byte 2-way matchfinder with dual-literal batch emission, boosting $L_1$ to $\ge 5.8	ext{ GB/s}$ and $\le 0.90	ext{ MB}$ (beating `libdeflate L1` at 5.64 GB/s, 0.92 MB).
4. Delivers publication-grade multi-corpus Pareto frontier charts proving full outer-convex-hull envelope dominance across all 12 tiers.

---

## Technical Context

**Language/Version**: C11 / POSIX APIs (Core Engine) + Swift 6.0 (`swift-tools-version: 6.0`).  
**Primary Dependencies**: In-process C static library bindings (`CTTZipBridge`), Apple Silicon ARM64 NEON intrinsics (`<arm_neon.h>`), AppKit / CoreGraphics (Raster Pareto Plotter).  
**Storage**: N/A (In-memory streaming buffers, stack/thread-local tables, zero dynamic heap allocations in hot loops).  
**Testing**: `swift test` (XCTest), `ZipSingleCoreParetoFrontierPkTests`, `XCTestPerformanceMeasureTests`.  
**Target Platform**: macOS 14.0+ (Apple Silicon M1/M2/M3/M4 optimized, x86_64 compatible).  
**Project Type**: High-performance native archiving and compression engine (`TTZipCore` / `CTTZipBridge`).  
**Performance Goals**:
- Level 1 JSON Throughput: $\ge 5.8	ext{ GB/s}$ (Size $\le 0.90	ext{ MB}$).
- Level 4 enwik8 Throughput: $\ge 800	ext{ MB/s}$ (Size $\le 3.20	ext{ MB}$).
- Monotonicity: 100% monotonic size reduction $	ext{Size}(L_{k+1}) < 	ext{Size}(L_k)$ across all 12 levels.
- Extreme Peak Compression: Level 12 $\le 2.85	ext{ MB}$ on enwik8.  
**Constraints**:
- Zero heap allocation per 64KB chunk in hot compression paths.
- Matchfinder tables must remain resident in Apple Silicon L1 D-cache ($\le 128	ext{ KB}$).
- 100% RFC 1951 / RFC 1952 bitstream conformance and standard decompressor oracle parity.  
**Scale/Scope**: 12 compression levels, 3 benchmark corpora (100MB each), 5 competing software families.

---

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- [x] **Zero-Cost Abstraction on Hot Paths**: Matchfinder structures reside in thread-local storage or stack; zero `malloc`/`free` in chunk loops.
- [x] **No Shared Locks on Hot Paths**: Zero mutex or semaphore operations in compression worker paths.
- [x] **No Kernel Zeroing in Hot Loops**: Rolling 32KB offset rebasing (`vqaddq_s16`) eliminates per-chunk `memset`.
- [x] **Frozen Subsystem Respect**: Frozen files in `.agents/rules/zip-engine-freeze.md` are untouched. All modifications reside in `native_deflate/`, `ttzip_zopfli_engine.c`, and benchmark tests.
- [x] **Stream-First & Oracle-First Discipline**: 100% round-trip verified against system `gzip`/`unzip` and libdeflate.

---

## Project Structure & Planned Changes

### Documentation (this feature)

```text
specs/126-single-core-pareto-supremacy-and-12tier-calibration/
├── spec.md              # Feature specification & requirements
├── checklists/
│   └── requirements.md  # Quality validation checklist
├── plan.md              # Implementation plan (this file)
├── research.md          # Phase 0 consolidated research findings
├── data-model.md        # Phase 1 data entities and memory models
├── contracts/           # Phase 1 strict JSON schemas
│   ├── deflate-12tier-calibration-contract.json
│   └── pareto-benchmark-dataset-contract.json
├── quickstart.md        # Phase 1 validation scenarios and diagnostics
└── tasks.md             # Phase 2 implementation task list (generated in @speckit-tasks)
```

### Component Breakdown & Source File Modifications

```text
Sources/CTTZipBridge/
├── native_deflate/
│   ├── ttzip_deflate_engine.h       # [MODIFY] Add 12-tier parameter struct, hybrid fast mf and 4-way lazy mf types
│   ├── ttzip_deflate_engine.c       # [MODIFY] 12-tier monotonic parameter dispatch and option mapping
│   ├── ttzip_deflate_fast.c         # [MODIFY] 128KB hybrid 3-byte direct / 4-byte 2-way SWAR matchfinder
│   ├── ttzip_deflate_lazy.c         # [MODIFY] 64KB 4-way compact bucket lazy matchfinder with prefix+tail filter
│   └── ttzip_deflate_huffman.c      # [MODIFY] Fine-tuned dynamic/static Huffman cost evaluation
├── ttzip_zopfli_engine.c            # [MODIFY] Seamless 12-tier routing (Levels 1-9 to native C, 10-12 to Zopfli DAG)
└── include/
    └── ttzip_zopfli_engine.h        # [MODIFY] Header export for 12-tier unified compression API

Sources/TTZipCore/
└── Benchmark/
    ├── ParetoFrontierModels.swift   # [MODIFY] Tier metadata models and software family classifier
    └── RasterParetoPlotter.swift    # [MODIFY] High-precision Pareto convex hull & trajectory plotting

Tests/TTZipTests/
└── ZipSingleCoreParetoFrontierPkTests.swift # [MODIFY] 12-tier full matrix monotonic assertions & PK runners
```

---

## Phase 0: Research Items Index

- R001 [SUBAGENT:research] 《12 阶单调哈希链与 Lazy Match 参数矩阵设计》：分析 `Vendor/libdeflate-upstream/lib/deflate_compress.c` 与 `ttzip_deflate_engine.c`，构建严格单调递进的 12 阶参数梯度表。
- R002 [SUBAGENT:research] 《ARM64 NEON 2-Way / 4-Way 紧凑 Lazy Matcher 向量化设计》：设计 64KB L1 驻留的 4-Way 紧凑桶表与前缀+尾部双字过滤，填补 800~900 MB/s 中速空缺。
- R003 [SUBAGENT:research] 《自适应 3-Byte/4-Byte 混合直接哈希在 JSON/纯文本上的加速机制》：设计 128KB 混合 3-byte 直接哈希表与双字面量批量输出，突破 5.8 GB/s。

---

## Phase 1: Contracts & Artifacts Index

- `specs/126-single-core-pareto-supremacy-and-12tier-calibration/research.md`
- `specs/126-single-core-pareto-supremacy-and-12tier-calibration/data-model.md`
- `specs/126-single-core-pareto-supremacy-and-12tier-calibration/contracts/deflate-12tier-calibration-contract.json` [SUBAGENT:research]
- `specs/126-single-core-pareto-supremacy-and-12tier-calibration/contracts/pareto-benchmark-dataset-contract.json` [SUBAGENT:research]
- `specs/126-single-core-pareto-supremacy-and-12tier-calibration/quickstart.md`
