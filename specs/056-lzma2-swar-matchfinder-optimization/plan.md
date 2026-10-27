# Implementation Plan: LZMA2 64-bit SWAR Match Finder Optimization

## Technical Context
* **目标模块**：`Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c`
* **关联接口**：`Sources/CTTZipBridge/include/ttzip_lzma_hc4_neon.h`、`Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c`
* **受保护的冻结文件**：`ZipParallelExtractor.swift`、`ZipParallelWriter.swift` 等（严格零修改）
* **核心变更**：将 `ttzip_match_len_neon` 升级为 64-bit 整数 SWAR 快速比对实现，同时保留原有全零向量旁路。

---

## Constitution Check
* [x] **热路径零成本抽象**：零堆分配、零动态多态，全部采用 `static inline` 或直接 C 函数调用。
* [x] **Fast-Path 旁路保留原则**：完整保留 64 字节向量全零探测与 2MB LZMA2 直通合成。
* [x] **吞吐硬门禁**：7Z Level 1 压缩 >= 3,200 MB/s (Debug) / >= 3,900 MB/s (Release)。
* [x] **冻结文件零侵入**：严格不修改任何被冻结的 ZIP 核心引擎代码。

---

## Phase 0: Research Items
- - R001 [SUBAGENT:research] 《SWAR 64-bit 整数与 NEON 128-bit 比对性能评估》：评估 Apple Silicon 架构下 GPR SWAR 与 NEON 向量横向规约的指令流水线差异（已在 `research.md` 闭环）。
- - R002 [SUBAGENT:research] 《全零块直通与稀疏数据快速旁路保全》：验证全零块直通合成机制在 7Z L1 引擎中的独立性与门禁达标情况（已在 `research.md` 闭环）。

---

## Phase 1: Design & Contracts Index
* `data-model.md`：定义 HC4 匹配器与 SWAR 边界数据结构。
* `contracts/lzma2_match_finder_contract.json`：定义 Match Finder 输入/输出/性能门禁强类型契约。
* `quickstart.md`：定义完整的验证场景、命令、预期输出与排查手册。

---

## Proposed Component Changes

### CTTZipBridge
* **[MODIFY]** [`Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c)
  * 重构 `ttzip_match_len_neon` 为高效的 64-bit SWAR 比对核心。
  * 保持函数名向后兼容，更新内部循环为 8 字节 `memcpy` + `v1 ^ v2` + `__builtin_ctzll` 单周期定位。
* **[MODIFY]** [`Sources/CTTZipBridge/include/ttzip_lzma_hc4_neon.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/ttzip_lzma_hc4_neon.h)
  * 保持头文件声明与注释对齐，确保 C 模块映射一致性。

### Tests
* **[TEST]** `Tests/TTZipTests/FastLZMA2Tests.swift`
* **[TEST]** `Tests/TTZipTests/SevenZipBridgeTests.swift`
* **[TEST]** `Tests/TTZipTests/PerformanceRegressionGuardTests.swift`
