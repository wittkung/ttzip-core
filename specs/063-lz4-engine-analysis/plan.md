# Implementation Plan: LZ4 Engine Deep Analysis and Architecture Integration

**Branch**: `063-lz4-engine-analysis` | **Date**: 2026-08-17 | **Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/063-lz4-engine-analysis/spec.md)

---

## Summary

全面剖析官方开源库 `lz4/lz4`（v1.10.0+）的算法原理（Zero-Entropy、Token 4-bit 结构、Wild Copy 向量化批量吞吐、L1 哈希表与动态加速因子），对比 TTZip 当前链路（Apple `compression.h` vs 原生 `liblz4`），明确技术债务与优化空间；建立基于原生静态 `liblz4` 的架构演进方案，规划大体积 TAR.LZ4 毫秒级穿透与 VFS 两级临时解压缓存池架构，并完成全矩阵性能门禁验证。

---

## Technical Context

- **Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`) + C11 / POSIX APIs
- **Primary Dependencies**: 原生 `liblz4.a` (v1.10.0), `libarchive.a`, `libdeflate.a`
- **Storage**: 内存物理页对齐缓冲区（`PlatformMemory` 16KB 页对齐） + `/tmp` 稀疏 VFS 缓存
- **Testing**: `swift test --filter XCTestPerformanceMeasureTests`, `AllFormatsPkSuiteTests`, `Phase123FeatureCoverageTests`
- **Target Platform**: macOS 14.0+ (Apple Silicon NEON + Intel x86_64), 跨平台 Windows MSVC 纯 C 准备
- **Project Type**: 高性能归档系统核心引擎（100% In-Process C 静态库绑定）
- **Performance Goals**: LZ4 内存编解码吞吐 >= 6000 MB/s (Debug) / >= 10000 MB/s (Release)，解压峰值 >= 4000 MB/s
- **Constraints**: 零热路径动态堆分配，单任务内存常驻稳定受控，100% 比特级无损还原

---

## Constitution Check

- [x] **Zero-Cost Abstraction on Hot Paths**: 热路径严禁引入动态对象树，使用未初始化裸指针与 `Data(bytesNoCopy:)`。
- [x] **Stream-First Invariant**: 消除全量无限内存分配假设，面向 16KB 页对齐与分块流式拉取管道。
- [x] **Invariant-First & Bounds-First**: 64 位整数经过 `SSIZE_MAX` Clamp，内存释放前清理状态。
- [x] **Zero-Regression Hard Performance Floor**: 严格遵守全格式历史最优基准门禁矩阵。

---

## Phase 0: Research Studies

- [x] `- R001 [SUBAGENT:research] 《LZ4 算法内核与官方库架构原理深度调研》`：研究官方开源库核心算法、Token 结构、Wild Copy 向量化、零熵编码及单核 4~5 GB/s 解压机理。
- [x] `- R002 [SUBAGENT:research] 《TTZip 现有 LZ4 链路与 Apple compression.h 差异对比》`：对比 TTZip 现有 C 桥接与 Apple `compression.h`，分析加速因子失效与格式割裂等技术债。
- [x] `- R003 [SUBAGENT:research] 《大体积 TAR.LZ4 极速穿透与 VFS 临时解压缓存池利用方案》`：设计基于 TarSeekTable 的毫秒级穿透与 RAM-LZ4 / Disk-LZ4 两级 VFS 缓存池架构。

---

## Phase 1: Design Artifacts

- [x] **Data Model**: [data-model.md](file:///Users/kevintung/Documents/dev/TTZip/specs/063-lz4-engine-analysis/data-model.md)
- [x] **Contract Schema**: [contracts/lz4_engine_contract.json](file:///Users/kevintung/Documents/dev/TTZip/specs/063-lz4-engine-analysis/contracts/lz4_engine_contract.json)
- [x] **Quickstart & Verification Guide**: [quickstart.md](file:///Users/kevintung/Documents/dev/TTZip/specs/063-lz4-engine-analysis/quickstart.md)

---

## Project Structure & Component Changes

```text
TTZip/
├── Sources/
│   ├── CTTZipBridge/
│   │   ├── CTTZipStreamCoder.c          # [ANALYSIS/TARGET] 统一废弃 Apple compression.h 宏分支，直连原生 liblz4
│   │   ├── CTTZipBridge_GzParallel.c    # [VERIFY] 保持 LZ4F_ 多线程分块并发流式生成
│   │   └── include/CTTZipBridge.h       # [VERIFY] 维持 C 桥接 API 稳定
│   └── TTZipCore/
│       ├── ProfessionalAlgorithmsSuite.swift  # [VERIFY] LZ4LzoEngine 加速因子直通
│       └── ArchiveEngineFamilyFactory.swift   # [VERIFY] 格式分发路由
├── Vendor/
│   ├── include/lz4.h                    # [INCLUDED] 原生 LZ4 v1.10.0 头文件
│   ├── include/lz4frame.h               # [INCLUDED] 原生 LZ4 Frame 头文件
│   └── lib/liblz4.a                     # [INCLUDED] 原生预编译静态库
└── Tests/TTZipTests/
    ├── Phase123FeatureCoverageTests.swift     # [TEST] 原生数据一致性回归
    └── XCTestPerformanceMeasureTests.swift    # [TEST] 性能硬门禁回归
```
