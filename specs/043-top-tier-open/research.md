# Phase 0 Research & Technology Decisions: 043-top-tier-open

**Feature Directory**: `specs/043-top-tier-open`  
**Status**: Completed  
**Sources Baseline**: `Package.swift`, `Sources/TTZipCore/Zip/ZipParallelExtractor.swift`, `.github/workflows/ci-cd.yml`, `Tests/TTZipTests/ArchiveMutationFuzzTests.swift`

---

## 1. Research Overview

本阶段针对 TTZip 升级为世界顶级开源系统工程所面临的 4 大关键技术问题开展了深度调研与实证分析，全部结论基于真实文件代码核对与官方规范：
- **R001**: SPM 零 UnsafeFlags 与相对路径 C 桥接架构方案研究
- **R002**: 基于 ARC/RAII 的 MmapBufferHandle 零拷贝与严格 Sendable 安全模型研究
- **R003**: 工业级 CI/CD 矩阵与 AddressSanitizer/ThreadSanitizer 最佳实践研究
- **R004**: 基于 LLVM LibFuzzer 与 Swift 的持续模糊测试 (Coverage-Guided Fuzzing) 基础设施研究

---

## 2. Research Decisions

### R001: SPM 零 UnsafeFlags 与相对路径 C 桥接架构
- **Decision**: 采用 SPM 官方标准的 `binaryTarget` (.xcframework) 封装底层静态库 + 相对路径 `cSettings` / `linkerSettings` 架构，全量清除 `.unsafeFlags` 与 `#filePath` 绝对路径计算。
- **Rationale**:
  1. 彻底解决 SE-0238 限制：消除了所有 Target 的 `.unsafeFlags`，使得 `TTZipCore` 符合作为版本化远端依赖被第三方项目安全引用的标准。
  2. 纯相对路径与完全可重定位：移除 `#filePath` 计算，在任何工作区路径或 CI 缓存环境下均具备确定性编译能力。
  3. 保留秒级构建速度与 MAS/Direct 双渠道二进制合规性。
- **Alternatives Considered**:
  - *被否决方案 1 (全 C 源码 SPM 编译)*：冷启动编译耗时由秒级激增至数分钟，且难以维护多平台的 `config.h`。
  - *被否决方案 2 (systemLibrary 依赖 Homebrew)*：破坏 MAS 沙盒合规与“100% In-Process C 静态绑定”的宪法铁律。
- **Source**:
  - `file:///Users/kevintung/Documents/dev/TTZip/Package.swift`
  - `file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/module.modulemap`
  - [SE-0238: Package Manager Build Settings](https://github.com/swiftlang/swift-evolution/blob/main/proposals/0238-package-manager-build-settings.md)
  - [SE-0272: Package Manager Binary Dependencies](https://github.com/swiftlang/swift-evolution/blob/main/proposals/0272-swiftpm-binary-dependencies.md)

---

### R002: 基于 ARC/RAII 的 MmapBufferHandle 零拷贝与严格 Sendable 安全模型
- **Decision**: 采用不可变只读 ARC 引用句柄 `MmapBufferHandle: Sendable`，彻底替换原有的裸 `mmap + defer munmap` 及 `@unchecked Sendable` 的 `SendablePointerBox`。
- **Rationale**:
  1. 编译期与运行时双重安全：完全满足 Swift 6 严格并发检查标准（Strict Concurrency Checking），杜绝异步逃逸导致的 Use-After-Free (UAF) 和 `EXC_BAD_ACCESS`。
  2. 性能零损耗：保持原有 `mmap` 的 8,000+ MB/s 直通性能，热路径无锁、无额外拷贝。
  3. RAII 确定性资源释放：`deinit` 自动托管 `munmap` 与 `close(fd)`，在抛错、提前返回等所有分支下均能 100% 清理。
- **Alternatives Considered**:
  - *被否决方案 1 (`Foundation.Data(contentsOf: .alwaysMapped)`)*：引入 Obj-C 运行时开销，释放时机受 autoreleasepool 影响不可控，且无法调用 `madvise` 调优页缓存。
  - *被否决方案 2 (Swift `~Copyable` Struct)*：单所有权无法在 `concurrentPerform` / `TaskGroup` 中跨线程安全共享读取。
- **Source**:
  - `file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Zip/ZipParallelExtractor.swift`
  - `file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Zip/ZipModels.swift`
  - [SE-0302: Sendable and @Sendable closures](https://github.com/swiftlang/swift-evolution/blob/main/proposals/0302-sendable-code.md)

---

### R003: 工业级 CI/CD 矩阵与 AddressSanitizer/ThreadSanitizer 最佳实践
- **Decision**: 采用“三层分治”工业级 CI/CD 流水线体系（PR Gate -> Unit Tests Full -> Sanitizers Matrix -> Performance Gate / Release），重构 `.github/workflows/ci-cd.yml`。
- **Rationale**:
  1. 补齐 `pull_request` 与 `push: branches: [main]` 触发器，实现持续集成看门狗。
  2. 消除测试缩水：全量运行 95+ 测试文件、620+ 用例。
  3. 引入 ASan 与 TSan 自动化分析矩阵，在 PR 阶段捕获 C 桥接与 Swift 并发的数据竞争与内存越界。
  4. CI 收敛耗时控制在 3 分钟以内。
- **Alternatives Considered**:
  - *被否决方案 1 (在 Sanitizer 下跑完全部 GB 级压测)*：导致 CPU 减速 3 倍、内存膨胀 3 倍，易触发 Actions 内存超限与 20 分钟超时。
- **Source**:
  - `file:///Users/kevintung/Documents/dev/TTZip/.github/workflows/ci-cd.yml`
  - `file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/`
  - Apple Swift SPM Sanitizer 文档 (`swift test --help`)

---

### R004: 基于 LLVM LibFuzzer 与 Swift 的持续模糊测试基础设施
- **Decision**: 采用“双轨制 Fuzzing 体系 (Two-Tier Fuzzing Architecture)”：Tier 1 升级 `ArchiveMutationFuzzTests` (Crash-First 现场先落盘 + 1,000+ 变异)，Tier 2 暴露符合 `LLVMFuzzerTestOneInput` 标准的 C/C++ Fuzz Harness (`ttzip_fuzz_extract_harness.c`) 并配置独立持续 Fuzzing 运行脚本。
- **Rationale**:
  1. 对标 `libarchive` 与 `zstd` 的业界黄金标准。
  2. 解决 `XCTest` 与 `libFuzzer` `main()` 入口冲突问题。
  3. 通过 Coverage-Guided 反馈与 AddressSanitizer，深度挖掘解析状态机深层逻辑中的安全漏洞。
- **Alternatives Considered**:
  - *被否决方案 1 (仅在 Swift Testing 中无反馈随机变异)*：盲目变异无法触达 C 引擎深度状态机。
- **Source**:
  - `file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/ArchiveMutationFuzzTests.swift`
  - `file:///Users/kevintung/Documents/dev/TTZip/specs/037-libarchive-golden-oracle-and-fuzz-integration/research.md`
  - LLVM LibFuzzer 官方规范
