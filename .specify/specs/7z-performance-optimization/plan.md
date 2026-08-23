# Technical Plan: 7Z 引擎实质性性能优化 (Technical Plan & Architecture Blueprint)

## 一、 架构与设计选型

### 1. 7Z 原生并发解压重构 (`CTTZipBridge_7zNativeDecoder.c`)
- **动态元数据缓冲**：将固定 1024 大小的 `files` 和 `stream_sizes` 升级为基于 `num_files` 动态分配（`calloc(max(num_files, 64), ...)`），并在 $\le 64$ 文件时使用栈局部缓存。
- **多 Block 并发解码**：当 `block_count > 1` 时，使用 `dispatch_apply(block_count, queue, ...)` 将每个独立的 LZMA2 块并发解码到 `unpack_buf` 的对应 `unpack_offset`，消除单线程瓶颈。
- **日志规范化**：彻底移除 L389 `fprintf(stderr, ...)`。

### 2. 7Z Solid 打包并发 I/O 预载 (`CTTZipBridge_7zSolid.c`)
- **并发读取**：对聚合入 `solid_buf` 的所有小文件/中文件，预先计算每个文件的 offset 映射表，使用 `dispatch_apply(list.count, queue, ...)` 配合 `pread` 并发加载，多核全速填满 Solid 缓冲区。
- **NEON CRC32 向量化**：在每个线程内独立计算其对应文件的 CRC32，规避写入后的二次串行扫描。

### 3. 内存与缓存对齐
- 所有中间与最终分配采用 `posix_memalign(&ptr, 64, size)` 保证 64 字节 Cache-line 对齐，提升 ARM64 NEON 与 LZMA 范围解码器访存性能。

---

## 二、 受影响文件清单

- `[MODIFY]` `Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c`：动态元数据、多 Block 并发解码、日志清理。
- `[MODIFY]` `Sources/CTTZipBridge/CTTZipBridge_7zSolid.c`：并发 `pread` 预载输入流、NEON CRC 并行计算。
- `[MODIFY]` `Sources/TTZipCore/SevenZip/SevenZipEngine.swift`：调度层快速通道调优。
- `[MODIFY]` `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift`：添加或提升 7Z 性能门禁测试。
- `[MODIFY]` `GEMINI.md`：同步 7Z 性能指标要求。

---

## 三、 验证计划

1. **自动化构建与单元测试**：`swift test`（验证全部 550+ 测试通过）。
2. **性能基准对比测试**：`swift test --filter XCTestPerformanceMeasureTests` 与 `swift run ttzip-cli bench -f 7z`。
