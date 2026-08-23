# Implementation Plan: Full-Matrix libdeflate Architecture

**Feature Directory**: `specs/053-chunked-deflate-compressor`
**Created**: 2026-08-17
**Status**: Ready for Tasks

---

## 1. Technical Context

本项目规划覆盖三大核心工程支柱（P0/P1/P2）：
1. **【P0】超大文件自适应分块流式压缩器 (Chunked Stream Compressor)**：
   针对大于 256MB 的超大单文件，将现有的 Whole-Buffer 强行内存加载改为 1MB 分块多线程流式压缩管道。采用 RFC 1951 标准空存储块字节对齐同步（`0x00 0x00 0xFF 0xFF`），设置 `MAX_IN_FLIGHT = 32` 槽位，将常驻内存（RSS）严格恒定在 $\le 64\text{MB}$。
2. **【P1】Vendor/libdeflate 升级与双架构自动化构建 (v1.22)**：
   升级至官方最新稳定版 `v1.22`，编写 `scripts/build_libdeflate.sh` 实现 Universal 2 (`arm64` + `x86_64`) 一键编译与静态库打包。
3. **【P2】Windows 跨平台 CTTZipBridge 静态库矩阵 (CMake/MSVC)**：
   建立统一跨平台抽象层 `Sources/CTTZipBridge/include/CTTZipPlatform.h` (PAL)，并落地根目录 `CMakeLists.txt`，打通 Windows MSVC 与 Clang-cl 构建。

---

## 2. Constitution Check

- [x] **流式第一性 (Stream-First)**：彻底消除大文件一次性全量堆分配，单文件流式切块常驻内存 $\le 64\text{MB}$；杜绝 `Data(count:)` 内核页清零。
- [x] **纵深防御 (Invariant-First)**：文件写入保持 `O_NOFOLLOW` 与确定性权限；算术运算硬件防溢出。
- [x] **确定性确界 (Bounds-First)**：有界缓冲槽位（32 槽位）；跨平台 `SSIZE_MAX` Clamp；C 句柄释放 `magic = 0`。
- [x] **真实预言机 (Oracle-First)**：生成的 ZIP 归档必须经由系统原生 `/usr/bin/unzip`、macOS Archive Utility 与 7-Zip 双向差分测试。
- [x] **性能底线 (Hard Floors)**：小文件（$\le 256\text{MB}$）保留历史最优 Whole-buffer 旁路，零性能倒退。

---

## 3. Phase 0: Research Index

- - R001 [SUBAGENT:research] 《DEFLATE 分块流式多线程无缝拼接与 BFINAL 机制》：详见 [`research.md#R001`](file:///Users/kevintung/Documents/dev/TTZip/specs/053-chunked-deflate-compressor/research.md)
- - R002 [SUBAGENT:research] 《libdeflate v1.21+ 源码升级与 macOS Universal 2 自动化编译参数》：详见 [`research.md#R002`](file:///Users/kevintung/Documents/dev/TTZip/specs/053-chunked-deflate-compressor/research.md)
- - R003 [SUBAGENT:research] 《Windows MSVC / CMake 跨平台 C 桥接层与符号导出设计》：详见 [`research.md#R003`](file:///Users/kevintung/Documents/dev/TTZip/specs/053-chunked-deflate-compressor/research.md)

---

## 4. Phase 1: Design Artifacts & Contracts

- **Data Model**: [`data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/053-chunked-deflate-compressor/data-model.md)
- **Contracts** `[SUBAGENT:research]`:
  - [`contracts/chunked_pipeline_options.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/053-chunked-deflate-compressor/contracts/chunked_pipeline_options.json)
  - [`contracts/chunk_stream_event.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/053-chunked-deflate-compressor/contracts/chunk_stream_event.json)
  - [`contracts/cross_platform_build_manifest.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/053-chunked-deflate-compressor/contracts/cross_platform_build_manifest.json)
- **Validation Guide**: [`quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/053-chunked-deflate-compressor/quickstart.md)

---

## 5. Proposed Changes by Component

### Component 1: CTTZipBridge (C Platform & Streaming Core)

#### [NEW] [CTTZipPlatform.h](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/CTTZipPlatform.h)
- 跨平台系统抽象层 (PAL)，封装 `TTZIP_THREAD_LOCAL`、`TTZIP_API`、`ttzip_sleep_ms`、`ssize_t`、`O_BINARY` 与内存对齐分配。

#### [NEW] [CTTZipBridge_ZipChunkedStream.h](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/CTTZipBridge_ZipChunkedStream.h)
- 分块流式压缩器 C 接口定义，声明 `ttzip_zip_chunked_stream_init`、`write_chunk`、`finish` 与 `free`。

#### [NEW] [CTTZipBridge_ZipChunkedStream.c](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipBridge_ZipChunkedStream.c)
- 1MB 分块流式多线程压缩核心实现，包含 32 槽位有界环形缓冲、RFC 1951 字节对齐同步块注入、增量 CRC-32 累加与保序落盘。

#### [MODIFY] [CTTZipStreamCoder.c](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipStreamCoder.c)
- 引入 `CTTZipPlatform.h`，将原生 `__thread` 迁移为 `TTZIP_THREAD_LOCAL`。

---

### Component 2: TTZipCore (Swift Adaptive Pipeline & Adapters)

#### [NEW] [ChunkedDeflateStreamWriter.swift](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Zip/ChunkedDeflateStreamWriter.swift)
- Swift 强类型分块流式写入器包装，集成自适应路由：$\le 256\text{MB}$ 走 Whole-Buffer Fast-Path，$> 256\text{MB}$ 自动激活分块流式。

#### [MODIFY] [LibdeflateCAdapter.swift](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Adapters/LibdeflateCAdapter.swift)
- 增加流式分块压缩适配协议方法，桥接 C 接口。

---

### Component 3: Build Scripts & Tooling

#### [NEW] [build_libdeflate.sh](file:///Users/kevintung/Documents/dev/TTZip/scripts/build_libdeflate.sh)
- 自动化构建脚本，支持拉取 `libdeflate v1.22` 并编译产出 Universal 2 (`arm64` + `x86_64`) 静态库。

#### [NEW] [CMakeLists.txt](file:///Users/kevintung/Documents/dev/TTZip/CMakeLists.txt)
- 根目录跨平台 CMake 构建工程，支持 MSVC / Clang-cl 构建 `libdeflate` 与 `CTTZipBridge`。

---

### Component 4: Tests & Regression

#### [NEW] [ChunkedDeflateStreamingTests.swift](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/ChunkedDeflateStreamingTests.swift)
- 针对 1MB、256MB、500MB 及 1GB 样本的常驻内存（RSS $\le 64\text{MB}$）与差分测试（`/usr/bin/unzip` 100% 校验）。

---

## 6. Verification Plan

1. **自动化单元与内存测试**：`swift test --filter ChunkedDeflateStreamingTests`
2. **构建脚本验证**：`./scripts/build_libdeflate.sh && lipo -info Vendor/lib/libdeflate.a`
3. **CMake 跨平台工程校验**：`cmake -B build_cmake_test -S . && cmake --build build_cmake_test`
4. **全量回归与性能门禁**：`swift test && swift test --filter XCTestPerformanceMeasureTests`
