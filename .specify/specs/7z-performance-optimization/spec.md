# Feature Specification: 7Z 引擎实质性性能优化 (7Z Engine Performance Optimization)

## 一、 现状与动机 (Problem Statement & Motivation)

TTZip 在 macOS Apple Silicon 平台支持 7Z 归档格式的压缩与解压。
通过对当前 7Z 引擎（`CTTZipBridge_7zNativeDecoder.c`, `CTTZipBridge_7zSolid.c`, `ttzip_lzma2_enc_native.c`, `SevenZipEngine.swift`, `SevenZipParallelWriter.swift`, `SevenZipParallelExtractor.swift`）进行深度代码审计与 Benchmark 采样发现：

1. **解压路径单线程与固定数组上限**：
   - `CTTZipBridge_7zNativeDecoder.c` 中 `files` 和 `stream_sizes` 被硬编码为 `calloc(1024, ...)`，无法处理多于 1024 个文件的归档。
   - `ttzip_lzma2_decode_block_native` 在解析多个独立 LZMA2 块时仍走单线程同步执行，未能发挥 Apple Silicon 18 核全核并发解压优势。
   - 生产代码中残留 `fprintf(stderr)` 调试打印，污染流水线并消耗 I/O。
2. **压缩输入收集单线程 I/O 阻塞**：
   - `CTTZipBridge_7zSolid.c` 在聚合输入文件到 Solid 缓冲时，采用单线程逐文件 `open/read/close` 遍历，面对海量小文件时吞吐低下（仅 61.6 MB/s）。
   - 内存缓冲未按页面对齐，缺乏类似于 ZIP 引擎的并发预读（`pread` + `dispatch_apply`）加速。
3. **大文件内存消耗过高**：
   - 当前 Solid 压缩采用整体 `malloc(total_uncompressed_bytes)` 缓冲，对巨型文件产生不必要的内存压力。

## 二、 用户故事 (User Stories)

- **US-001 (极速解压)**: 作为用户，在解压大型或多文件 7z 压缩包时，TTZip 应利用多核并发与零堆锁竞争实现极速解压，吞吐显著超越单线程工具。
- **US-002 (批量小文件打包)**: 作为用户，在对成百上千个小文件进行 7z 打包时，TTZip 应该并发并行读取并批量写出，无 I/O 线程阻塞。
- **US-003 (大容量兼容与稳定性)**: 作为用户，包含数万个文件的复杂目录结构能够稳定解压和压缩，绝不因静态 1024 文件上限而发生截断或崩溃。

## 三、 功能需求清单 (Functional Requirements)

- [x] **FR-001**: 动态扩展 7Z 元数据解析容量（支持动态扩容或栈/堆弹性缓冲区），解除 1024 静态文件上限限制。
- [x] **FR-002**: 在 `CTTZipBridge_7zNativeDecoder.c` 中启用多 Block 多核并发 LZMA2 解压（GCD `dispatch_apply` 分块并发），提高多块 7z 文件解压吞吐。
- [x] **FR-003**: 移除 `CTTZipBridge_7zNativeDecoder.c` 中的裸 `fprintf(stderr)` 调试日志，接入 `ttzip_log_c` / `TTLogger` 统一规范。
- [x] **FR-004**: 在 `CTTZipBridge_7zSolid.c` 中使用并发多核 I/O 读取（`dispatch_apply` + `pread` + 栈缓冲）加速 Solid 缓冲区加载，大幅拉升小文件打包吞吐。
- [x] **FR-005**: 优化 LZMA2 编码与解码输出端的内存对齐（64 字节 Cache-line 对齐），提升 SIMD 访问效率。

## 四、 验收条件 (Acceptance Criteria)

- **AC-001**: `swift test` 550+ 项单元测试全部通过，0 warnings / 0 failures。
- **AC-002**: 7Z 压缩与解压吞吐在 `XCTestPerformanceMeasureTests` 与 `ttzip-cli bench -f 7z` 中全线提升，无任何场景性能退步。
- **AC-003**: 彻底消除裸 `fprintf` / `printf`，符合 GEMINI.md 日志纪律。
