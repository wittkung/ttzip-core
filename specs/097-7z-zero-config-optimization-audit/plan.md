# Implementation Plan: 097-7z-zero-config-optimization-audit

**Branch**: `097-7z-zero-config-optimization-audit` | **Date**: 2026-08-18 | **Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/097-7z-zero-config-optimization-audit/spec.md)

**Input**: Feature specification from `specs/097-7z-zero-config-optimization-audit/spec.md`

---

## Summary

经过全栈代码审计，TTZip 中的 7z 核心编解码管道（Fast-LZMA2 多核并行、SWAR/Radix 匹配查找、ARM64 NEON AES/SHA-256 硬件向量化、动态信息熵降级与两级 `mkdir_p` 缓存）已全面接通生产代码。本方案在固化当前 7z 性能门禁与反配置膨胀（Zero Configuration Creep）体系的同时，进一步打通 `ArchiveReader` 顶层对 7z 的零拷贝 `mmap` 快速检视旁路（`< 2ms` 响应），彻底消除检视阶段调用 `libarchive` 或落盘临时解压的隐患。

---

## Technical Context

**Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`) + C11 / POSIX APIs  
**Primary Dependencies**: In-process `CTTZipBridge`, `Vendor/liblzma.a`, `Vendor/libzstd.a`, `fast-lzma2`, CommonCrypto  
**Storage**: APFS direct I/O, POSIX `mmap` (MAP_SHARED / MAP_PRIVATE) zero-copy memory buffers  
**Testing**: `swift test` (XCTest, 525+ tests, `FastLZMA2Tests`, `FormatSupportTests`, `TouchIDAndHeaderEncryptionTests`)  
**Target Platform**: macOS 14.0+ (Apple Silicon ARM64 NEON prioritized, Intel x86_64 compatible)  
**Project Type**: Desktop Application + Swift Package Core + CLI Tool  
**Performance Goals**: 7z L1 压缩 $\ge 3,200\text{ MB/s}$，7z 极速解压 $\ge 6,600\text{ MB/s}$，7z 目录检视 $\le 5\text{ ms}$，KDF 派生 $\le 15\text{ ms}$  
**Constraints**: 零中间堆分配、热路径无锁化、零内核页清零、单任务常驻内存 $\le 64\text{ MB}$，零外部 CLI 子进程依赖  
**Scale/Scope**: 全量 7z 编解码与检视调用链路（`TTZipCore`, `CTTZipBridge`, `TTZipApp`, `TTZipCLI`）

---

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Invariant | Status | Verification & Evidence |
| :--- | :--- | :--- |
| **1. Stream-First (流式第一性)** | PASS | 单文件 $\ge 1\text{MB}$ 采用 `mmap` 按需分页；LZMA2 多块任务切分为 $256\text{KB} \sim 32\text{MB}$ 微缓冲处理，杜绝全量内存分配。 |
| **2. Invariant-First (纵深防御)** | PASS | 解压使用 `O_NOFOLLOW` 物理免疫 TOCTOU 软链接劫持；解密使用硬件 NEON CRC32 严格比对。 |
| **3. Bounds-First (确定性确界)** | PASS | 密钥释放调用 `ttzip_secure_zero` 物理擦除；64 位整型窄化经过 `__builtin_clz` 与 `SSIZE_MAX` Clamp。 |
| **4. Oracle-First (真实预言机)** | PASS | 测试面向真实 7z 签名二进制、密码候选池与 `FastLZMA2Tests` 真实物理吞吐验证。 |
| **5. Anti-Configuration Bloat** | PASS | 核心调度参数（P-Core 核心数、动态块大小、信息熵 Store 降级、字典大小）100% 由系统自适应决策。 |

---

## Phase 0: Grounded Research Findings

- R001 [SUBAGENT:research] 《7z 格式多核并行压缩与信息熵自适应路由机制》：验证了 `ttzip_estimate_buffer_entropy_dynamic` 动态信息熵采样判定（$H > 7.90 \rightarrow \text{Store}$）、基于物理 P-Core 拓扑的动态分块算法、SWAR/Radix 匹配查找器与异步后台 `pthread` KDF 派生掩盖机制（耗时 $0\text{ms}$ 感知延迟）。详见 [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/097-7z-zero-config-optimization-audit/research.md)。
- R002 [SUBAGENT:research] 《7z 格式并行解压与硬件向量化解密机制》：验证了基于 `mmap` + `ttzip_7z_read_varint` 零拷贝头部解析、ARM64 NEON SHA-256 硬件 KDF 与 512KB 并行 AES-256-CBC 向量化解密、LZMA2 字典重置分块并发调度及两级无锁 `mkdir_p` 缓存算法。详见 [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/097-7z-zero-config-optimization-audit/research.md)。
- R003 [SUBAGENT:research] 《7z 归档检视（Inspection）与目录树遍历架构》：查明了当前 `ArchiveReader.swift` 缺乏 7z 专用零拷贝 Fast-Path 的问题，确立了在 Swift 调度层接入 `NativeSevenZipEngine.inspectSevenZip` $\rightarrow$ `ttzip_native_inspect_archive` 极速通道（$< 2\text{ms}$ 响应）并在头部加密场景平滑回退的安全方案。详见 [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/097-7z-zero-config-optimization-audit/research.md)。

---

## Phase 1: Design Artifacts

- **Data Model**: [data-model.md](file:///Users/kevintung/Documents/dev/TTZip/specs/097-7z-zero-config-optimization-audit/data-model.md) 完整定义了 `SevenZipCompressionConfig`、`SevenZipExtractionConfig`、`SevenZipArchiveInspectionResult`、`SevenZipEntryDescriptorItem` 与 `SevenZipEntropyEvaluation` 强类型模型。
- **Contracts**: [contracts/](file:///Users/kevintung/Documents/dev/TTZip/specs/097-7z-zero-config-optimization-audit/contracts/) 下包含：
  - `7z-compression-request.json`
  - `7z-extraction-request.json`
  - `7z-inspection-result.json`
  - `7z-entropy-evaluation.json`
- **Quickstart Guide**: [quickstart.md](file:///Users/kevintung/Documents/dev/TTZip/specs/097-7z-zero-config-optimization-audit/quickstart.md) 提供了 4 大端到端物理回归验证场景及排错诊断说明。

---

## Component Change List

### 1. `Sources/TTZipCore/ArchiveReader.swift`
- **[MODIFY]** 在 `inspect` 函数中为 `.7z` 扩展名注入零拷贝 Fast-Path：优先调用 `NativeSevenZipEngine.shared.inspectSevenZip`，在 $< 2\text{ms}$ 内直接返回文件树；解析失败时无缝回退至既有密码候选池与 libarchive 探测逻辑。

### 2. `Sources/TTZipCore/SevenZip/NativeSevenZipEngine.swift`
- **[MODIFY]** 打通 `NativeSevenZipEngine.inspectSevenZip` 底层与 C 桥接层 `ttzip_native_inspect_archive` 的数据通道，消除 Mock 存根，将 C 层解析的 UTF-16LE 路径、未压缩大小及加密标志映射为标准 `ArchiveEntry` 数组。

### 3. `Tests/TTZipTests/`
- **[NEW]** 补充针对 7z 零拷贝极速检视与零配置性能回归的专项单元测试，断言未加密 7z 检视耗时 $\le 5\text{ms}$ 且与全量解压目录树严格 100% 一致。

---

## Complexity Tracking

> Constitution Check 全部通过，零违规项，无需复杂性豁免。
