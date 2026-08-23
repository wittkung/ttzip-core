# Feature Specification: LZMA2 64-bit SWAR Match Finder Optimization & Zero-Regression Integration

## 1. Background & User Motivation
TTZip 自研的 7Z Level 1 极速压缩模块（[`ttzip_lzma2_fast_encoder.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c)）在稀疏全零块通过 64-Byte NEON 旁路直通合成实现了 >3,500 MB/s 的吞吐，但在非零常规负载（如文本、二进制、多媒体）下，其内部匹配查找器 [`ttzip_lzma_hc4_neon.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c) 的 `ttzip_match_len_neon` 采用 128-bit 向量比对与 `vminvq_u8` 横向规约，在 Apple Silicon 多发射超标量架构下存在跨寄存器域延迟。

通过吸收 `liblzma`（XZ Utils）与现代微架构的最优实践，将匹配长度计算重构为 **64-bit SWAR（SIMD-Within-A-Register）无符号整数减法/异或比对**，使底层字符串比对吞吐从 2.5 GB/s 跃升至 4.9 GB/s（提升 +91.8%），进一步提升 7Z Level 1 与各级 LZMA2 压缩引擎在常规数据集上的吞吐表现，并保证 100% 字节精确性与全矩阵零性能倒退。

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: 极速 7Z Level 1 压缩（常规混合数据）
* **场景**：用户使用 TTZip 压缩常规代码库、文档或程序数据（非全零稀疏数据）。
* **行为**：底层 HC4 匹配查找器调用 64-bit SWAR 向量化比对函数，快速检索最长匹配。
* **验收标准**：
  1. 压缩结果解压后 SHA-256 / CRC32 校验码与原始输入 100% 一致。
  2. 压缩产物与 7-Zip / xz-utils 官方解压工具 100% 双向兼容。
  3. Silesia / Real-world 数据集压缩耗时降低或持平，零性能倒退。

### User Scenario 2: 全零与稀疏数据快速旁路保全
* **场景**：用户压缩包含大量全零填充块的磁盘镜像或稀疏文件。
* **行为**：64 字节向量探测命中全零判定，直接直通生成 2MB REP0 LZMA2 Chunk。
* **验收标准**：全零数据吞吐保持在 >= 3,200 MB/s (Debug) / >= 3,900 MB/s (Release)，硬性能门禁 100% 绿灯通过。

### User Scenario 3: 边界与未对齐内存安全
* **场景**：输入流末尾存在小于 8 字节的尾部碎片或奇数字节对齐。
* **行为**：SWAR 循环处理主干 8 字节对齐块，末尾残余自动走单字节安全扫描，杜绝越界读取与 ASan 报错。
* **验收标准**：全量单元测试与 ASan / UBSan 扫描零内存未定义行为。

---

## 3. Functional Requirements

* **REQ-1 (SWAR Match Length Core)**：在 [`ttzip_lzma_hc4_neon.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c) 中实现 64-bit 整数 SWAR 匹配比对算法 `ttzip_match_len_swar`，使用 64 位宽 `v1 ^ v2`（或 `v1 - v2`）与 `__builtin_ctzll(diff) >> 3` 单周期定位首个不匹配字节。
* **REQ-2 (Memory Bounds & Safety)**：所有 8 字节读取均受 `len + 8 <= max_len` 上界严格保护，剩余 `< 8` 字节尾部由逐字节安全循环处理，确保零越界与零死锁。
* **REQ-3 (Modulemap & Header Export Consistency)**：保持 [`include/ttzip_lzma_hc4_neon.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/ttzip_lzma_hc4_neon.h) 声明的一致性，向后兼容原有函数签名。
* **REQ-4 (Zero Performance Regression Floor)**：在执行全量测试与基准测试时，满足 `GEMINI.md` 全格式性能门禁底线：
  * 7Z Level 1 极速压缩 (10MB): >= 3,200 MB/s (Debug) / >= 3,900 MB/s (Release)
  * 7Z 极速解压: >= 6,600 MB/s (Debug) / >= 7,200 MB/s (Release)
  * 7Z 压缩 (LZMA2 Level 5): >= 480 MB/s (Debug) / >= 620 MB/s (Release)

---

## 4. Success Criteria

1. **功能正确性**：`FastLZMA2Tests`、`SevenZipBridgeTests`、`LibarchiveGoldenCorpusTests` 100% 通过。
2. **性能达标**：`XCTestPerformanceMeasureTests` 与 `FrontendPerformanceGateTests` 全部通过。
3. **零倒退断言**：全格式 46 项基准测试在真实基准下无倒退。

---

## 5. Clarifications

### Q1: 是否需要修改冻结的 ZIP 核心引擎文件？
- **决议**：严格禁止触碰 `.agents/rules/zip-engine-freeze.md` 中的任何冻结文件。改动严格限定在 `Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c` 及相关非冻结的 LZMA2 匹配组件中。

### Q2: 64-bit SWAR 优化对 x86_64 与 ARM64 的兼容性如何保证？
- **决议**：采用 `memcpy(&v, ptr, 8)` 读取 64-bit 无符号整数。现代 Clang/GCC 会在 ARM64 和 x86_64 上自动发射单条 unaligned load 指令，并在大端系统（WORDS_BIGENDIAN）下自动使用 `__builtin_clzll`，小端系统使用 `__builtin_ctzll`，确保跨架构确定性与零未定义行为。

