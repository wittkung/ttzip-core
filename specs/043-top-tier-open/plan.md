# Implementation Plan: TTZip 顶尖开源工程对标与架构硬化改造 (Top-Tier Open Source Alignment)

**Feature Branch**: `043-top-tier-open`  
**Feature Directory**: `specs/043-top-tier-open`  
**Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/043-top-tier-open/spec.md)  
**Created**: 2026-08-17  
**Status**: Planned  

---

## 1. Technical Context & Subagent Research Dispatches

- **Runtime**: Swift 6.0 (`swift-tools-version: 6.0`) + Clang POSIX.
- **Key Modules Affected**:
  - `Package.swift`: SPM 依赖、编译与链接标志中枢
  - `Sources/TTZipCore/Adapters/`: 新增 `MmapBufferHandle` RAII 安全虚拟内存管理器
  - `Sources/TTZipCore/Zip/`: `ZipParallelExtractor` 接入 `MmapBufferHandle` 替换裸指针
  - `.github/workflows/ci-cd.yml`: 工业级多层触发与 Sanitizer 矩阵 CI 流水线
  - `Tests/TTZipTests/`: 覆盖率引导 Fuzzing 确定性语料库测试与 Mmap 句柄单元测试
- **Phase 0 Research Dispatches**:
  - - R001 [SUBAGENT:research] 《SPM 零 UnsafeFlags 与相对路径 C 桥接架构方案》：消除 `#filePath` 绝对路径与 `.unsafeFlags`，采用 `binaryTarget` (.xcframework) + 相对路径 `cSettings`。
  - - R002 [SUBAGENT:research] 《基于 ARC/RAII 的 MmapBufferHandle 零拷贝与严格 Sendable 安全模型》：消除裸 `@unchecked Sendable` 逃逸，构建只读不可变 `MmapBufferHandle: Sendable`，由 ARC 生命周期自动确定性触发 `munmap`。
  - - R003 [SUBAGENT:research] 《工业级 CI/CD 矩阵与 AddressSanitizer/ThreadSanitizer 最佳实践》：补全 PR 触发器、全量 95+ 测试套件并行执行、Sanitizer 矩阵扫描与 SwiftLint 静态代码门禁。
  - - R004 [SUBAGENT:research] 《基于 LLVM LibFuzzer 与 Swift 的持续模糊测试 (Coverage-Guided Fuzzing) 基础设施》：建立双轨制 Fuzzing 体系（Tier 1 XCTest 语料库回归 + Tier 2 LLVMFuzzerTestOneInput C/Swift Harness）。

---

## 2. Constitution Check (Constitution Level 0 & Invariants)

- **I. 流式第一性 (Stream-First)**:
  - [x] `MmapBufferHandle` 基于只读虚拟内存分页，零中间 `Data(count:)` 内核页清零，零堆内存二次拷贝。
- **II. 纵深防御 (Invariant-First)**:
  - [x] 所有指针切片均经过 `offset + length <= count` 边界防御检查。
- **III. 确定性确界 (Bounds-First)**:
  - [x] 虚拟内存解映射与文件描述符关闭由 `deinit` 原子触发，无论何种抛错路径均 100% 确定性释放。
- **IV. 真实预言机 (Oracle-First)**:
  - [x] CI 全量运行 95+ 测试套件，双向对齐历史缺陷语料库与 AddressSanitizer 动态检测。
- **V. 性能硬门禁 (Hot-Path Floor)**:
  - [x] 改造后严格执行 `swift test --filter XCTestPerformanceMeasureTests` 验证 13 项门禁 100% 达标。

---

## 3. Phase 0 & Phase 1 Artifacts

- **Phase 0 Research**: [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/043-top-tier-open/research.md) (全部 4 项研究结论完备)
- **Phase 1 Data Model**: [data-model.md](file:///Users/kevintung/Documents/dev/TTZip/specs/043-top-tier-open/data-model.md)
- **Phase 1 Contracts**:
  - [spm_configuration_contract.json](file:///Users/kevintung/Documents/dev/TTZip/specs/043-top-tier-open/contracts/spm_configuration_contract.json) [SUBAGENT:research]
  - [memory_safety_contract.json](file:///Users/kevintung/Documents/dev/TTZip/specs/043-top-tier-open/contracts/memory_safety_contract.json) [SUBAGENT:research]
  - [ci_matrix_contract.json](file:///Users/kevintung/Documents/dev/TTZip/specs/043-top-tier-open/contracts/ci_matrix_contract.json) [SUBAGENT:research]
  - [fuzzing_infrastructure_contract.json](file:///Users/kevintung/Documents/dev/TTZip/specs/043-top-tier-open/contracts/fuzzing_infrastructure_contract.json) [SUBAGENT:research]
- **Phase 1 Quickstart**: [quickstart.md](file:///Users/kevintung/Documents/dev/TTZip/specs/043-top-tier-open/quickstart.md)

---

## 4. Planned File Changes by Component

### Package Management & Distribution Layer
- **[MODIFY]** `Package.swift`: 清除 `.unsafeFlags` 与 `#filePath` 绝对路径，引入标准相对路径 `cSettings` 与 `linkerSettings`。

### Memory Management & Concurrency Layer
- **[NEW]** `Sources/TTZipCore/Adapters/MmapBufferHandle.swift`: 实现基于 ARC/RAII 的不可变只读 `MmapBufferHandle: Sendable`。
- **[MODIFY]** `Sources/TTZipCore/Zip/ZipParallelExtractor.swift`: 接入 `MmapBufferHandle` 替换裸指针与 `defer munmap`。
- **[NEW]** `Tests/TTZipTests/MmapBufferHandleTests.swift`: 编写 RAII 句柄生命周期、边界防御与并发读取单测。

### CI/CD & Static Quality Watchdog
- **[MODIFY]** `.github/workflows/ci-cd.yml`: 重构为多阶段工业级流水线（PR 触发、Lint、全量并行测试、Sanitizers 矩阵、Release 打包）。
- **[NEW]** `.swiftlint.yml`: 声明代码规范与文件长度门禁。

### Repository Hygiene & Libarchive Testing Infrastructure
- **[MODIFY]** `.gitignore`: 增加根目录临时测试文件免疫规则。
- **[DELETE]** 根目录 ad-hoc 测试残留文件 (`test.7z`, `test.txt`, `test1.txt`, `test2.txt`, `test_7z_c.c`, `test_7z_create.swift`, `test_7z_out/`, `test_bug.7z`, `test_comp.7z`, `test_empty.swift`, `test_out/`, `test_out2/`, `test_out3/`, `test_out4/`, `test_store.7z`, `t1.txt`, `t2.txt`)。
- **[NEW]** `Tests/TTZipTests/LibarchiveUUDecoder.swift`: 实现纯内存 `.uu` 文本高速解码器。
- **[NEW]** `Tests/TTZipTests/TTZipAssertions.swift`: 实现 POSIX 文件系统与内存安全原语断言库。
- **[NEW]** `Tests/TTZipTests/LibarchiveGoldenCorpusTests.swift`: 加载 30+ 官方 `.uu` 黄金样本执行端到端闭环安全断言。
- **[MODIFY]** `Tests/TTZipTests/ArchiveMutationFuzzTests.swift`: 升级为带 Crash-First 现场先落盘的 1,000+ 变异语料库测试。

---

## 5. Verification Plan

### Automated Regression
```bash
# 1. 纯净编译验证
swift build

# 2. Mmap 句柄单测验证
swift test --filter MmapBufferHandleTests

# 3. Libarchive 黄金语料库回归验证
swift test --filter LibarchiveGoldenCorpusTests

# 4. 全量 95+ 测试套件并行回归
swift test --parallel

# 5. 性能硬门禁全绿核查
swift test --filter XCTestPerformanceMeasureTests
```

