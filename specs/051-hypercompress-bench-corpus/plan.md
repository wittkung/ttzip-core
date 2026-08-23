# Implementation Plan: HyperCompressBench Benchmark Suite

**Feature Branch**: `051-hypercompress-bench-corpus`  
**Created**: 2026-08-17  
**Status**: Draft  
**Source Spec**: [spec.md](./spec.md)

---

## 1. Technical Context

HyperCompressBench tests TTZip against high-cardinality micro-file workloads (tens of thousands of 1KB~64KB files comprising JSON, logs, and high-entropy random chunks). This shifts the performance critical path from CPU codec throughput to filesystem VFS metadata traversal, system call batching, memory page caching, and per-file allocation elimination.

- **Target Platforms**: macOS 14+ (Sonoma, APFS), Windows 11 (NTFS).
- **Core Modules Involved**:
  - `Sources/TTZipCore/Zip/ZipDirectoryScanner.swift` (VFS Traversal)
  - `Sources/TTZipCore/NativeCoreArchitecture.swift` (Page-aligned buffer management)
  - `Tests/TTZipTests/HyperCompressCorpusGenerator.swift` (Deterministic PRNG generator)
  - `Tests/TTZipTests/HyperCompressBatchGateTests.swift` (Small-file batch performance gates)
  - `Tests/TTZipTests/DirectoryScanPerformanceTests.swift` (50,000-node directory scanner gates)
- **Performance Invariants**:
  - Batch small-file compression (500+ files) >= 50 MB/s (Debug) / >= 70 MB/s (Release).
  - 50,000-node directory traversal <= 250 ms (>= 200,000 nodes/s).
  - Zero heap allocation in hot directory traversal / batch compression inner loops.

---

## 2. Constitution & Rules Check

| Invariant / Rule | Compliance Strategy | Status |
| :--- | :--- | :--- |
| **Hot Path Zero-Cost Abstraction** | No dynamic tree objects (`ArchiveComponentTree`) in scanner; linear array buffers. | PASS |
| **Fast-Path Bypass Retention** | C batch compression paths (`ttzip_extract_zip_c_parallel`) called directly. | PASS |
| **Hard Performance Floor** | Strict XCTest assertions enforcing >= 70 MB/s Release batch compression. | PASS |
| **Stream-First & Zero-Alloc** | Page-aligned 4MB circular buffers, thread-local compressor state reuse. | PASS |
| **Zero Bare Objects in Contracts** | All JSON Schemas fully typed with explicit draft-07 properties and validations. | PASS |
| **Deterministic Isolation** | In-memory / temporary sandbox generation with zero Git repo inode pollution. | PASS |

---

## 3. Phase 0: Technical Research Index

- **R001 [SUBAGENT:research] 《跨平台海量小文件目录遍历与元数据读取架构》**：调研 APFS (getattrlistbulk/fts) 与 Windows NTFS 在 50,000 节点深层树遍历下的锁开销与无锁批处理管道。已在 [research.md](./research.md) 落地。
- **R002 [SUBAGENT:research] 《内存零拷贝与线程局部池化微文件批处理压缩架构》**：调研小文件批处理 Fast-Path 门禁要求 (>= 70 MB/s Release, >= 50 MB/s Debug)，消除 per-file malloc/free 与 Data(count:) 零填充。已在 [research.md](./research.md) 落地。
- **R003 [SUBAGENT:research] 《零磁盘膨胀的确定性微文件生成器》**：设计高吞吐 (>= 1500 MB/s) 确定性 PRNG 生成器，生成 JSON / Log / 高熵伪随机碎片，零 Git 仓库污染。已在 [research.md](./research.md) 落地。

---

## 4. Phase 1: Design Artifacts Index

- **Data Model**: [data-model.md](./data-model.md) — 包含 `MicroCorpusProfile`, `SyntheticFileItem`, `DirectoryScanMetric`, `HyperCompressBatchResult`, `HyperCompressSuiteReport` 完整类型定义。
- **Contracts / JSON Schemas**:
  - `contracts/hypercompress-profile.schema.json` [SUBAGENT:research]
  - `contracts/hypercompress-batch-result.schema.json` [SUBAGENT:research]
  - `contracts/hypercompress-suite-report.schema.json` [SUBAGENT:research]
- **Validation Guide**: [quickstart.md](./quickstart.md) — 包含 Scenario 1 (500文件门禁), Scenario 2 (50k节点遍历), Scenario 3 (混合熵早退与哈希校验) 可执行命令与排查指南。

---

## 5. Component Modification List

### `Tests/TTZipTests/` (Test Fixtures & Generators)
- **[NEW] `HyperCompressCorpusGenerator.swift`**:
  - Implements deterministic SplitMix64 / XorShift128+ PRNG.
  - Synthesizes Micro-JSON (40%), Server Logs (40%), and High-Entropy Blobs (20%).
  - Supports both in-memory byte buffers and on-disk temporary sandbox trees.
- **[NEW] `HyperCompressBatchGateTests.swift`**:
  - Tests 500~2,000 micro-files batch compression floor (>= 70 MB/s Release).
  - Validates ZIP, TAR.ZST, and 7Z small-file Fast-Path performance.
- **[NEW] `DirectoryScanPerformanceTests.swift`**:
  - Benchmarks `ZipDirectoryScanner` over 1,000, 10,000, and 50,000 nodes.
  - Asserts <= 10ms, <= 60ms, and <= 250ms latency floors.
- **[NEW] `HyperCompressIntegrityAndEntropyTests.swift`**:
  - Validates 100% round-trip CRC32/SHA-256 hash match on all micro-files.
  - Tests match-finder early-exit speedup on high-entropy binary fragments.

### `Sources/TTZipCore/` (Engine Optimization Verification)
- **[VERIFY] `Sources/TTZipCore/Zip/ZipDirectoryScanner.swift`**:
  - Assert zero allocations and `FTS_NOCHDIR | FTS_NOSTAT` flag integrity.
- **[VERIFY] `Sources/TTZipCore/Zip/ZipParallelCompressor.swift`**:
  - Confirm thread-local compressor state reuse across batch file items.
