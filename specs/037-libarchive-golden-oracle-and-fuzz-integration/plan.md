# Implementation Plan: libarchive 黄金预言机语料库、变异模糊测试与系统差分测试工程落地

**Feature Directory**: `specs/037-libarchive-golden-oracle-and-fuzz-integration`  
**Created**: 2026-08-16  
**Status**: In Progress  
**Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/037-libarchive-golden-oracle-and-fuzz-integration/spec.md)

---

## Technical Context

- **项目基线**: Swift 6.0 + macOS 14.0+ + XCTest。
- **核心对标**: `Vendor/libarchive-upstream/test_utils/test_main.c` (extract_reference_file) 与 `libarchive/test/test_fuzz.c`。
- **落地内容**:
  1. `Sources/TTZipCore/Utilities/UUDecoder.swift`：纯 Swift 高性能流式 UUEncode 解码器。
  2. `Tests/TTZipTests/Fixtures/GoldenCorpus/*.uu`：精选 upstream 历史缺陷样本。
  3. `Tests/TTZipTests/ArchiveGoldenCorpusTests.swift`：黄金预言机全量回归测试。
  4. `Tests/TTZipTests/ArchiveMutationFuzzTests.swift`：变异模糊测试与崩溃优先转储门禁。
  5. `Tests/TTZipTests/SystemDifferentialTests.swift`：macOS 系统 `/usr/bin/tar`、`/usr/bin/unzip` 双向差分测试。

---

## Constitution Check

- [x] **Zero-Cost Abstraction**: 测试工具和解码器不侵入热路径，测试沙盒隔离安全。
- [x] **No Subprocess for Core**: 仅在 `SystemDifferentialTests` 中通过 `Process` 调度系统原生 CLI 进行差分对比，核心引擎保持 100% In-process C 绑定。
- [x] **Zero Bare Logging**: 测试日志采用 XCTest / TTLogger 规范。
- [x] **Frozen Subsystems**: 零侵入已冻结的 ZIP 核心引擎。

---

## Phase 0: Research Items

- R001: 《UUEncode 规范与纯 Swift 流式解码实现》：基于 `test_main.c:3230-3288` 的 6-bit 展开算法，设计零临时分配的 Swift `UUDecoder.decode(uuString:) -> Data`。
- R002: 《In-Process 变异模糊测试与崩溃现场优先转储》：基于 `test_fuzz.c:27-44` 的 1% 字节变异与 `fuzz_crash_reproducer.bin` 崩溃前预转储哲学。

---

## Phase 1: Artifacts & Contracts

- `data-model.md`: 定义 `UUDecodedFixture`、`FuzzMutationConfig` 与 `DifferentialTestResult` 数据模型。
- `contracts/testing_oracle_spec.json`: 黄金预言机与测试架构契约。
- `quickstart.md`: 运行黄金语料测试、模糊测试与系统差分测试的验证命令。

---

## Phase 2: Implementation Checklist

### 1. 核心解码器与黄金语料库
- [ ] 实现 `Sources/TTZipCore/Utilities/UUDecoder.swift`
- [ ] 创建 `Tests/TTZipTests/Fixtures/GoldenCorpus/` 并引入经典样本
- [ ] 实现 `Tests/TTZipTests/ArchiveGoldenCorpusTests.swift`

### 2. 变异模糊测试门禁
- [ ] 实现 `Tests/TTZipTests/ArchiveMutationFuzzTests.swift`（含崩溃现场优先转储与双模式消费）

### 3. 系统原生工具双向差分测试
- [ ] 实现 `Tests/TTZipTests/SystemDifferentialTests.swift`（针对 `/usr/bin/tar`, `/usr/bin/unzip`）

### 4. 验证与回归
- [ ] 执行全套新测试验证
