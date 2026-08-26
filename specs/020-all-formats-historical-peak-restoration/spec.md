# Feature Specification: 020 All-Formats Historical Peak Restoration & Zero-Gap Performance Alignment

**Feature Branch**: `specs/020-all-formats-historical-peak-restoration`  
**Created**: 2026-08-15  
**Status**: Draft  
**Input**: User description: "我们还有大量倒退超过 10% 的，需要全面回到历史最优 /speckit-specify"

---

## 1. User Scenarios & Testing *(mandatory)*

### User Story 1 - 消除所有全格式与历史峰值矩阵超过 10% 的性能倒退 (Priority: P1)

作为 macOS 高性能归档解压引擎的核心用户与自动化流水线，系统必须确保在 16 种格式（ZIP, 7Z, TAR, TAR.ZST, TAR.GZ, TAR.BZ2, TAR.XZ, LZIP, LZ4, BROTLI, LRZIP, AAR, SNAPPY, WIM, DMG, ISO）与 4 种典型 Payload（500MB 大文件数据块、拟真日志文本 10MB、高熵物理 Payload 100MB、海量小文件 10MB/100文件）在 Level 1 / Level 6 / AES-256 全矩阵下的吞吐与耗时，严格贴合或超越 `docs/benchmarks/peak_performance_matrix.json` 历史峰值记录，彻底消灭所有倒退超过 10% 的异常项。

**Why this priority**: 性能是 TTZip 的立身之本。任何超过 10% 的性能倒退都会破坏用户体验并阻断流水线合并。

**Independent Test**: 运行全格式基准测试 `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests` 并执行 `python3 scripts/audit_performance_regression.py`，确认无任何倒退超过 10.0% 的阻断项。

**Acceptance Scenarios**:
1. **Given** 500MB 大文件数据块，**When** 执行 7Z Level 1 压缩，**Then** 压缩吞吐达到 $\ge 18,000$ MB/s。
2. **Given** 10MB 海量小文件（100文件），**When** 执行 ZIP / TAR / TAR.GZ 压缩与解压，**Then** 吞吐完全恢复至历史峰值水平（ZIP 压缩 $\ge 6,000$ MB/s，TAR 压缩 $\ge 3,500$ MB/s）。
3. **Given** 100MB 高熵 Payload，**When** 执行 TAR.XZ / LZIP / LRZIP / LZ4 解压与压缩，**Then** 通过直接 Store 旁路与多核流式处理达到历史最高吞吐。

---

### User Story 2 - 零开销热路径与底层 C 引擎对齐 (Priority: P2)

确保 `ArchiveWriter`、`ArchiveExtractor` 以及底层 C 桥接层在编解码热路径上完全对齐历史验证过的最佳无损实现（如 commit `604d44d`、`fa1c8a2`、`c1225d7`），严禁任何中间堆分配、冗余系统调用、冗余文件遍历或同步 CRC32 盘扫描。

**Why this priority**: 阻断非必要的抽象层开销，从根本上杜绝性能抖动与回归。

**Independent Test**: 运行 `swift test --filter XCTestPerformanceMeasureTests` 与 `FrontendPerformanceGateTests`，验证所有性能门禁 100% 达标。

**Acceptance Scenarios**:
1. **Given** 任意归档操作，**When** `progressHandler == nil` 时，**Then** 跳过任何不必要的预扫描或对象树构建。
2. **Given** 多文件打包，**When** C 引擎扫描目录时，**Then** 通过单次批量 `pread`/`pwrite` 或 `mmap` 处理，杜绝重复 `stat`。

---

### User Story 3 - 591+ 全量单测与零 Warning 质量保证 (Priority: P3)

确保在完成所有历史峰值对齐与性能调优后，代码库 591+ 项单元测试 100% 绿色通过，且无任何编译器警告与未捕获异常。

**Why this priority**: 性能提升绝不以牺牲功能正确性、线程安全或边界兼容性为代价。

**Independent Test**: 执行 `swift build`、`swift build --build-tests` 与 `swift test`。

**Acceptance Scenarios**:
1. **Given** 完整的工程源码，**When** 执行编译，**Then** 产生 0 错误、0 警告。
2. **Given** 全量单元测试套件，**When** 执行 `swift test`，**Then** 591 个测试全部通过。

---

## 2. Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 针对 `docs/benchmarks/peak_performance_matrix.json` 中记录的 284 项基准场景，系统必须消除所有与历史峰值差距 $> 10.0\%$ 的倒退项。
- **FR-002**: `ArchiveWriter` 与 `ArchiveExtractor` 必须对 16 种格式全部分发至经过历史验证的最优原生 C / SIMD Fast-Path，禁止回退至慢速通用路径。
- **FR-003**: 针对海量小文件，C 引擎必须使用 Arena 缓冲池与预分配机制，杜绝 per-file 的 `malloc`/`free` 锁争用与重复 `mkdir` 系统调用。
- **FR-004**: 针对 500MB 大文件与高熵不可压缩数据，7z、ZIP、TAR.ZST 等格式必须正确使用全核多固实块并发（`-ms=128m` / `dispatch_apply`）与快速 Shannon 熵 Store 旁路。
- **FR-005**: 针对 TAR.XZ 与 LZIP / LZ4，多核 LZMA2 / liblzma 解压与压缩管道必须正确配置多线程（`threads=0`、`block-size=16M`）。
- **FR-006**: 所有改动必须通过 `swift test`（591+ 测试）与 `XCTestPerformanceMeasureTests` 门禁。

---

## 3. Success Criteria *(mandatory)*

- **SC-001**: 运行 `python3 scripts/audit_performance_regression.py` 审计最新基准报告，严重性能倒退（$> 10.0\%$）项数降为 **0**。
- **SC-002**: `swift test --filter XCTestPerformanceMeasureTests` 中定义的 11 项吞吐底线全部超越。
- **SC-003**: `swift test` 全量回归测试 **591/591** 项 100% 绿色通过。
- **SC-004**: `swift build` 与 `swift build --build-tests` 维持 **0 warnings, 0 errors**。

---

## 4. Clarifications

### Clarification Session 2026-08-15
- **Q1: 性能基准对比源与判定标准？**
  - **Resolution**: 以 `docs/benchmarks/peak_performance_matrix.json`（共 284 项历史全格式各维度最高峰值）为单一基准源。变动比例 $< -10.0\%$ 判定为必须修复的严重倒退。
- **Q2: 调优与对齐范围？**
  - **Resolution**: 覆盖 16 种格式（ZIP, 7Z, TAR, TAR.ZST, TAR.GZ, TAR.BZ2, TAR.XZ, LZIP, LZ4, BROTLI, LRZIP, AAR, SNAPPY, WIM, DMG, ISO）与全部 4 类测试数据集，优先对齐 commit `604d44d` / `fa1c8a2` 的成熟 C 原生编解码流。
- **Q3: 冻结文件与架构约束？**
  - **Resolution**: 严格遵守 `.agents/rules/zip-engine-freeze.md` 与 `GEMINI.md` 的性能铁律与设计模式热路径隔离原则。
