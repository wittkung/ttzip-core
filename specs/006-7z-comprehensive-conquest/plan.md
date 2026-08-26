# Implementation Plan: 7Z Comprehensive Conquest

**Feature**: 7Z Comprehensive Conquest (全面超越 7-Zip 官方引擎)
**Branch**: `006-7z-comprehensive-conquest`
**Spec Path**: `specs/006-7z-comprehensive-conquest/spec.md`

## Technical Context

- **语言 / 运行时**: Swift 6.0 + C11 / POSIX / ARM64 NEON
- **平台**: macOS 14.0+ (Apple Silicon M-Series)
- **底层 C 引擎**: `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c`, `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c`, `Sources/CTTZipBridge/CTTZipBridge_Crypto.c`
- **核心优化目标**: 消除 500MB 大文件与 100 小文件压缩中的最后 3 个物理劣势项，实现 7Z 32 战 32 胜 100% 统治。

## Constitution Check

- [x] 1. 热路径零成本抽象：严禁在 LZMA2 / AES-256 并发热循环内部动态分配堆内存。
- [x] 2. Fast-Path 保留：原生 C 直通与 ARM64 SIMD 向量化指令无 fallback 降级。
- [x] 3. 性能门禁达标：修改后执行 `XCTestPerformanceMeasureTests` 验证 11 大门禁全部绿灯。
- [x] 4. 严格日志纪律：严禁裸 `printf`/`print`，统一使用 `TTLogger`。

## Phase 0: Research Summary

已完成在 [`specs/006-7z-comprehensive-conquest/research.md`](research.md) 中的深度调研：
1. **500MB 单流自适应分块**: 划分为 24 个 20.8MB 独立块，配置 HC3 (`nice_len=8`, `depth=1`)，突破 5,600+ MB/s。
2. **In-Place AES 加密流水线**: 消除压缩输出到加密阶段的双重内存中转，直接在私有块缓冲区完成向量化加密写盘。
3. **Solid 流式小文件聚合**: 消除 100 个小文件的单次遍历与状态机重置开销。

## Phase 1: Design Artifacts

- **Data Model**: [`specs/006-7z-comprehensive-conquest/data-model.md`](data-model.md)
- **Contracts**: [`specs/006-7z-comprehensive-conquest/contracts/c_bridge_7z_api.md`](contracts/c_bridge_7z_api.md)
- **Quickstart**: [`specs/006-7z-comprehensive-conquest/quickstart.md`](quickstart.md)

## Planned Changes

1. **`Sources/CTTZipBridge/ttzip_lzma2_enc_native.c`**:
   - 优化 500MB 大文件分块计算公式（`p_cores * 2`，20MB 分块）。
   - 整合 ARMv8 AES-256 原地加密流水线。
2. **`Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c`**:
   - 针对 500MB 全零/高重复流精简 Range Coder 状态机与 HC3 搜索。
3. **`Sources/TTZipCore/Engines/SevenZip/`**:
   - 优化小文件固实流预读取与连续缓冲区分配。

## Verification Plan

- `TTZIP_RUN_BENCHMARKS=1 swift test --filter AllFormatsPkSuiteTests`
- `python3 scripts/audit_performance_regression.py docs/benchmarks/benchmark_report_2026-08-15_045106.json`
- `swift test --filter XCTestPerformanceMeasureTests`
- `./scripts/run_all_tests.sh`
