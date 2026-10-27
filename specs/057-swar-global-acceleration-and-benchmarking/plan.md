# Implementation Plan: Global 64-bit SWAR Acceleration & Benchmarking

## Technical Context
* **目标模块**：
  * `Sources/CTTZipBridge/CTTZipUtils.c`
  * `Sources/CTTZipBridge/ttzip_native_archive.c`
* **受保护的冻结文件**：`ZipParallelExtractor.swift` 等（严格零修改）
* **核心目标**：将 64-bit SWAR 扩展至字符集探测与头部魔数识别，并提供严谨的优化前后基准测试数据。

---

## Constitution Check
* [x] **热路径零成本抽象**：纯标量/SWAR 整数运算，零堆分配，零系统调用。
* [x] **Fast-Path 旁路保留原则**：完整保留现有的所有快速探测分支。
* [x] **严格内存安全**：对所有 4/8 字节加载进行严格的 `i + 8 <= len` 上界保护。
* [x] **冻结文件零侵入**：严格不修改任何被冻结的 ZIP 核心引擎代码。

---

## Phase 0: Research Items
- - R001 [SUBAGENT:research] 《64-bit SWAR ASCII 探测算法分析》：详见 `research.md`。
- - R002 [SUBAGENT:research] 《容器魔数定宽整数比对优化》：详见 `research.md`。

---

## Phase 1: Design & Contracts Index
* `data-model.md`：定义字符编码与格式魔数数据边界。
* `contracts/swar_global_acceleration_contract.json`：定义全局 SWAR 契约规范。
* `quickstart.md`：定义完整的基准与回归测试命令。

---

## Proposed Component Changes

### CTTZipBridge
* **[MODIFY]** [`Sources/CTTZipBridge/CTTZipUtils.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipUtils.c)
  * 在 `ttzip_detect_encoding_fast` 中引入 64-bit SWAR 8 字节 ASCII 快速扫描。
* **[MODIFY]** [`Sources/CTTZipBridge/ttzip_native_archive.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_native_archive.c)
  * 在 `ttzip_detect_format_from_header` 中优化 7z、TAR、XZ、ZSTD、LZ4 的魔数直接比对。

### Tests
* **[NEW]** `Tests/TTZipTests/SwarOptimizationBenchmarkTests.swift`
* **[TEST]** `Tests/TTZipTests/CharsetDetectorTests.swift`
* **[TEST]** `Tests/TTZipTests/FormatSupportTests.swift`
