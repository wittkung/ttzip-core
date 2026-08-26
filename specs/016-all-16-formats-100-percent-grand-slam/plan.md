# Implementation Plan: 100% Grand Slam Win Rate Across All 16 Archive Formats & Zero-Regression Performance

**Branch**: `016-all-16-formats-100-percent-grand-slam` | **Date**: 2026-08-15 | **Spec**: [`spec.md`](spec.md)

---

## 1. Summary

在当前全 16 格式 280 场竞品 1v1 PK 对决中，TTZip 已取得 241 场胜利（86.07% 胜率）。本计划针对剩余 39 处负场（Brotli 16 项失败、TAR.XZ 解压、纯 TAR 大文件/小文件打包、TAR.ZST 高熵解压、LZIP 500MB、LRZIP、LZ4）实施针对性架构优化，达到 100% 胜率，同时坚决捍卫零性能倒退底线（核心场景倒退 < 3.0%）。

---

## 2. Technical Context

- **Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`) + C11 / POSIX APIs.
- **Primary Dependencies**: In-Process C Static Libraries (`libarchive.a`, `libdeflate.a`, `liblzma.a`, `libzstd.a`, `liblz4.a`, `libb2.a`), macOS native `Compression.framework` (`COMPRESSION_BROTLI`).
- **Platform**: macOS 14.0+ (Apple Silicon NEON prioritized, Intel compatible).
- **Testing Framework**: XCTest (`swift test`, `AllFormatsPkSuiteTests`, `XCTestPerformanceMeasureTests`).
- **Performance Goals**: 100% Win Rate (280/280 PK matchups won), regression < 3.0%, 11 performance gates PASS.

---

## 3. Constitution Check

- [x] **Zero-Cost Abstractions on Hot Paths**: 无中间堆分配、无锁开销、无零填充 `Data(count:)`。
- [x] **100% In-Process Execution**: 无外部 CLI 子进程调用，完全兼容 MAS 沙盒。
- [x] **Frozen Files Respected**: 严格遵守 `zip-engine-freeze.md`，不修改受冻结文件。
- [x] **Logging Discipline**: 零裸 `print` / `printf`，统一采用 `TTLogger`。
- [x] **Regression Rule**: 核心场景性能倒退必须严格控制在 `< 3.0%`。

---

## 4. Architectural Breakdown & Component Changes

```text
TTZip/
├── Sources/
│   ├── CTTZipBridge/
│   │   ├── ttzip_tar_native.c           # [MODIFY] 纯 TAR Direct I/O 流式打包 Fast-Path、LZIP/LZ4 参数调优
│   │   ├── ttzip_tar_zstd.c             # [MODIFY] ZSTD 32MB 解压缓冲区与多线程流水线调优
│   │   ├── CTTZipBridge_Archive.c       # [MODIFY] 归档创建路由分发与 Brotli / Direct TAR 挂接
│   │   └── include/CTTZipBridge.h       # [MODIFY] 导出 C 接口与函数原型
│   ├── TTZipCore/
│   │   ├── Brotli/
│   │   │   └── NativeBrotliEngine.swift # [NEW] 原生 In-Process Brotli 压缩与解压引擎 (Apple Compression)
│   │   ├── TemplateMethod/
│   │   │   └── TarArchiveEngineTemplate.swift # [MODIFY] 挂接 NativeBrotliEngine、TAR.XZ 多核解压、纯 TAR 极速直通
│   │   ├── ArchiveWriter+Dispatch.swift # [MODIFY] 挂接 Brotli / Direct TAR 极速打包路由
│   │   ├── ArchiveExtractor+Dispatch.swift # [MODIFY] 挂接 Brotli / TAR.XZ 多核解压路由
│   │   └── Benchmark/
│   │       ├── CompetitorBenchmarkRunner.swift # [MODIFY] 修正 Brotli 临时文件命名与诊断集成
│   │       └── CompetitorBenchmarkRunner+ExtendedExecutors.swift # [MODIFY] 保证竞品对决公正性
│   └── TTZipCLI/
└── Tests/
    └── TTZipTests/
        └── AllFormatDiagnosticSuiteTests.swift # [MODIFY] 开启 Brotli 原生打包诊断断言
```

---

## 5. Phase 0 & Phase 1 Artifact Index

- **Phase 0 Research**: [`research.md`](research.md)
- **Phase 1 Data Model**: [`data-model.md`](data-model.md)
- **Phase 1 Contracts**:
  - [`contracts/benchmark-matrix-schema.json`](contracts/benchmark-matrix-schema.json)
  - [`contracts/regression-audit-schema.json`](contracts/regression-audit-schema.json)
  - [`contracts/format-operation-contract.json`](contracts/format-operation-contract.json)
- **Phase 1 Quickstart**: [`quickstart.md`](quickstart.md)
