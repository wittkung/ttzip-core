# Implementation Plan: 078-lzfse-dmg-windows-support

**Branch**: `078-lzfse-dmg-windows-support` | **Date**: 2026-08-18 | **Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/078-lzfse-dmg-windows-support/spec.md)

**Input**: Feature specification from `specs/078-lzfse-dmg-windows-support/spec.md`

## Summary

为 Windows 版与跨平台版 TTZip 补齐对 Apple DMG / LZFSE 归档的 100% 原生穿透解压能力。通过将 `apple/lzfse` 官方 C99 源码静态嵌入 `CTTZipBridge`，消除对 macOS 专有 `liblzfse.dylib` 的 `dlopen` 依赖；同时在进程内实现 UDIF `koly` trailer 与 `blkx` (0x80000006/0x80000007 `BLOCK_LZFSE`) 块解码管道，配合 Thread-Local Scratch 内存管理与微缓冲流式拉取架构，彻底解决 Windows 端打开现代 macOS DMG 时的解压失败问题，严格满足全矩阵性能门禁与四大系统工程铁律。

## Technical Context

**Language/Version**: Swift 6.0 (`swift-tools-version: 6.0`) + C99/C11 (MSVC, Apple Clang, LLVM Clang).

**Primary Dependencies**: `apple/lzfse` (BSD-3-Clause, static in-tree), `CTTZipBridge`, `Vendor/libdeflate.a`, `Vendor/liblzma.a`, LZMA SDK / 7-Zip APFS/HFS+ Handlers.

**Storage**: In-memory streaming buffers, page flyweight pool (`MemoryPageFlyweightPool`), zero intermediate temp disk allocations.

**Testing**: XCTest (`swift test --filter AccelerationInfrastructureTests`, `DMGLZFSEExtractionTests`, `XCTestPerformanceMeasureTests`).

**Target Platform**: Windows 10/11 (x64, ARM64), macOS 14.0+ (Apple Silicon NEON prioritized, Intel x86_64 compatible).

**Project Type**: Native High-Performance Archiving Engine & Desktop Client (TTZipCore + CTTZipBridge + TTZipCLI + TTZipApp).

**Performance Goals**: LZFSE 解压吞吐 $\ge 800\text{ MB/s}$（单核 x86_64/ARM64），50GB DMG 解压驻留内存 $\le 64\text{ MB}$，目录与分区扫描 $\le 10\text{ ms}$。

**Constraints**: 零外部 DLL/dylib 依赖，零动态内存泄漏，严格遵循四大系统工程铁律（流式第一性、纵深防御、确定性确界、真实预言机）。

**Scale/Scope**: 14 个 LZFSE 核心 C 文件嵌入，3 个核心解压引擎与桥接层重构，全量 525+ 测试用例无回归。

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- [x] **流式第一性 (Stream-First)**：彻底废弃全量 `mmap` 与 `malloc(total_size * 8)` 反模式；采用双缓冲微缓冲拉取管道（Micro-buffering Pull Pipeline），单块解压内存上限 $\le 2\text{MB}$，超大镜像 RSS 恒定 $\le 64\text{MB}$。
- [x] **纵深防御 (Invariant-First)**：DMG 内部 APFS / HFS+ 文件解压落盘强制开启 `ARCHIVE_EXTRACT_SECURE_SYMLINKS` 与 `O_NOFOLLOW` 延后 Fixup 倒序回写，免疫 TOCTOU 软链接劫持。
- [x] **确定性确界 (Bounds-First)**：解压结构体首字段植入 `magic`并在析构前置清零；所有 64 位扇区偏移计算经过 `SSIZE_MAX` Clamp；端序转换强制显式。
- [x] **真实预言机 (Oracle-First)**：基于真实 macOS 生成的包含 LZFSE 块的 APFS DMG 样本进行双向差分测试与模糊测试。
- [x] **热路径零成本抽象**：解压循环内部零共享锁（`NSLock`/`pthread_mutex`），Scratch Buffer 采用线程局部绑定（Thread-Local Arena），零热路径 `malloc`/`free`。

## Phase 0: Outline & Research Index

- R001 [SUBAGENT:research] 《LZFSE 官方 C99 源码结构与 CTTZipBridge 静态嵌入重构》：深入调研 apple/lzfse 源码文件与编译配置，消除 dlopen。
- R002 [SUBAGENT:research] 《Apple UDIF (DMG) 磁盘映像规范与 LZFSE 块解码挂载管道》：调研 koly trailer、blkx mish 表与 0x80000006/0x80000007 块解码集成。
- R003 [SUBAGENT:research] 《跨平台 Scratch Buffer 内存管理与微缓冲流式拉取模型》：设计 Thread-Local Scratch Arena 与双缓冲拉取管道，满足 RSS <= 64MB。

*详细研究结论见 [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/078-lzfse-dmg-windows-support/research.md)*。

## Phase 1: Design & Contracts Index

- **Data Model**: [data-model.md](file:///Users/kevintung/Documents/dev/TTZip/specs/078-lzfse-dmg-windows-support/data-model.md)（定义 `LZFSEDecodeRequest`, `DMGUDIFDescriptor`, `UDIFChunkBlock` 等实体，零通配类型）
- **Contracts**:
  - [SUBAGENT:research] [lzfse-codec-contract.json](file:///Users/kevintung/Documents/dev/TTZip/specs/078-lzfse-dmg-windows-support/contracts/lzfse-codec-contract.json)（底层 C LZFSE/LZVN 块与流式编解码 Schema）
  - [SUBAGENT:research] [dmg-udif-schema.json](file:///Users/kevintung/Documents/dev/TTZip/specs/078-lzfse-dmg-windows-support/contracts/dmg-udif-schema.json)（DMG koly trailer 与 mish 块表描述符 Schema）
  - [SUBAGENT:research] [dmg-extraction-service-contract.json](file:///Users/kevintung/Documents/dev/TTZip/specs/078-lzfse-dmg-windows-support/contracts/dmg-extraction-service-contract.json)（DMG 穿透解压与事件广播 Triad 协议 Schema）
- **Quickstart Guide**: [quickstart.md](file:///Users/kevintung/Documents/dev/TTZip/specs/078-lzfse-dmg-windows-support/quickstart.md)（包含回归命令、预期输出及失败诊断）

## Project Structure & Component Changes

```text
TTZip/
├── Sources/
│   ├── CTTZipBridge/
│   │   ├── lzfse/                     # [NEW] 嵌入 apple/lzfse 官方 14 个核心 C99 源码与头文件
│   │   │   ├── lzfse.h
│   │   │   ├── lzfse_internal.h
│   │   │   ├── lzfse_tunables.h
│   │   │   ├── lzfse_encode_tables.h
│   │   │   ├── lzfse_fse.h
│   │   │   ├── lzvn_encode_base.h
│   │   │   ├── lzvn_decode_base.h
│   │   │   ├── lzfse_encode.c
│   │   │   ├── lzfse_encode_base.c
│   │   │   ├── lzfse_decode.c
│   │   │   ├── lzfse_decode_base.c
│   │   │   ├── lzfse_fse.c
│   │   │   ├── lzvn_encode_base.c
│   │   │   └── lzvn_decode_base.c
│   │   ├── CTTZipBridge_LZFSE.c       # [MODIFY] 消除 dlopen，重构为静态链接 + Thread-Local Scratch + 微缓冲流式解压
│   │   ├── ttzip_dmg_demux.c          # [NEW] C 语言原生 UDIF koly/blkx mish 块表解复用器
│   │   └── include/
│   │       ├── CTTZipBridge_LZFSE.h   # [MODIFY] 暴露微缓冲流式与块解压 API
│   │       └── ttzip_dmg_demux.h      # [NEW] 暴露 DMG UDIF 解析器接口
│   ├── TTZipCore/
│   │   ├── Adapters/
│   │   │   └── LzfseCAdapter.swift    # [NEW] Swift 强类型 LZFSE 适配器
│   │   ├── SevenZip/
│   │   │   └── DMGVirtualStreamAdapter.swift # [NEW] 将 DMG LZFSE 扇区流无缝桥接至 7z APFS/HFS+ Handler
│   │   └── ArchiveExtractor+Dispatch.swift   # [MODIFY] 挂载 DMG LZFSE 穿透解压 Fast-Path
│   └── Package.swift                  # [MODIFY] 添加 .headerSearchPath("lzfse")
└── Tests/TTZipTests/
    ├── AccelerationInfrastructureTests.swift # [MODIFY] 增强 LZFSE 静态绑定与吞吐断言
    └── DMGLZFSEExtractionTests.swift          # [NEW] 端到端 DMG (LZFSE 0x80000006/0x80000007) 穿透解压与大镜像内存门禁测试
```

## Complexity Tracking

*Constitution Check 100% Passed. 0 Unjustified Violations.*
