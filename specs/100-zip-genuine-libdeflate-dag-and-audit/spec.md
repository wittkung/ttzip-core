# Feature Specification: Genuine Libdeflate DAG Routing & Codebase Disconnect Audit

**Feature ID**: `100-zip-genuine-libdeflate-dag-and-audit`  
**Status**: Draft  
**Author**: Antigravity CTO / Spec Kit Autonomous Pipeline  
**Target Platform**: macOS 14.0+ (Apple Silicon C11 / ARM64 SIMD & Swift 6.0)  
**Created**: 2026-08-18  

---

## 1. Executive Summary & Root Cause

在近期对多核 ZIP 压缩性能的深度审计中，发现底层 C 桥接层存在两处严重的逻辑断层与伪路由缺陷：
1. **`ttzip_raw_deflate_block_compress_with_dict` 偷换实现**：`Sources/CTTZipBridge/CTTZipStreamCoder.c` 中该函数未调用高性能 `libdeflate` 引擎，而是调用了系统旧版 `zlib` 的 `deflateInit2`，且硬编码 `level > 9 ? 9 : level`，导致传入的 Level 10/11/12 全被截断为 `zlib -9`，使得图论最短路径（`deflate_find_min_cost_path`）从未真正执行；
2. **`ttzip_get_tls_compressor` 魔法别名污染**：`CTTZipStreamCoder.c` 第 24 行硬编码了 `(level == 6 ? 4 : level)`，人为造成了 6 级被降级为 4 级的异味；
3. **全工程桥接层与算法分发器断层隐患**：需要全面排查 `Sources/CTTZipBridge/` 与 `Sources/TTZipCore/` 中所有格式（ZIP, 7Z, TAR.ZST, LZ4 等）的 C 桥接层与参数映射，彻底根除所有静默回退、虚假分发与硬编码截断。

---

## 2. Functional Requirements

### FR-001: 彻底重构 `ttzip_raw_deflate_block_compress` 为 100% 纯 `libdeflate` 原生驱动
- 彻底废除 `CTTZipStreamCoder.c` 中基于 `zlib.h` `deflateInit2` 的慢速实现；
- 直接使用 `libdeflate_alloc_compressor(level)` 与 `libdeflate_deflate_compress`；
- 原生支持 1~12 全等级：
  - Level 1~4: `deflate_compress_fast` (ARM NEON SIMD 极速贪婪哈希)
  - Level 5~9: `deflate_compress_slow` (Lazy/Lazy2 懒惰评估)
  - Level 10~12: `deflate_compress_near_optimal` (BT Matchfinder + Dijkstra/Viterbi DAG 最短路径动态规划)

### FR-002: 彻底清除 `ttzip_get_tls_compressor` 中的魔法篡改
- 移除 `(level == 6 ? 4 : level)` 等任何隐蔽的等级篡改，保证传入等级 $L \in [1, 12]$ 1:1 精确映射到 `libdeflate_alloc_compressor(L)`。

### FR-003: 全代码库 C 桥接层与参数分发器深度审计
- 审计范围：
  - `Sources/CTTZipBridge/CTTZipStreamCoder.c`
  - `Sources/CTTZipBridge/CTTZipBridge_ZipWrite.c`
  - `Sources/CTTZipBridge/CTTZipBridge_Crypto.c`
  - `Sources/CTTZipBridge/ttzip_lzma2_*.c`
  - `Sources/TTZipCore/Adapters/` 下所有 Adapter
- 断言：凡在对外接口或文档中声明使用了某高性能 C 库（如 libdeflate, liblzma, zstd, lz4），底层必须 100% 直接直通该静态库原生 C API，严禁静默回退到系统旧版 libc 或截断参数。

---

## 3. Success Criteria & Hard Performance Invariants

- **SC-001 (真实 DAG 最短路径激活)**：在 Level 10/11/12 下，LLDB/DTrace 符号断点或吞吐分析 100% 确认命中了 `deflate_compress_near_optimal` 与 `deflate_find_min_cost_path`；
- **SC-002 (零静默篡改与零截断)**：`CTTZipBridge` 全量头文件与 C 源文件中，零 `level == 6 ? 4` 式魔法篡改，零 `level > 9 ? 9` 截断；
- **SC-003 (全格式 16 种基准零倒退)**：全量 525+ 单元测试与 6 阶段 CI 门禁 100% 通过；
- **SC-004 (多核 ZIP 帕累托阶梯严格单调)**：Level 0 (Store) 到 Level 7 (Extreme) 在 100MB 真实语料上的压缩体积必须严格单调递减。
