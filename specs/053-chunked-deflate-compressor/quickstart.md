# Quickstart Validation Guide: Full-Matrix libdeflate Architecture

**Feature Directory**: `specs/053-chunked-deflate-compressor`
**Created**: 2026-08-17
**Status**: Completed

---

## 1. 验证场景 1：超大单文件流式切块压缩与常驻内存确界校验

### Command
```bash
# 运行专有的大文件分块流式压缩器单元与内存回归测试
swift test --filter ChunkedDeflateStreamingTests
```

### Expected Output
```text
Test Suite 'ChunkedDeflateStreamingTests' passed at ...
	 Executed 6 tests, with 0 failures (0 unexpected) in 1.428 seconds
[TTZipCore] Chunked stream verified: uncompressed=1073741824, peak_rss_delta=42MB (<= 64MB floor)
[TTZipCore] Oracle verification with /usr/bin/unzip: SHA-256 match 100%
```

### Failure Diagnostic
- **若测试报错 `Peak RSS exceeded 64MB`**：
  检查 `MAX_IN_FLIGHT_CHUNKS` 是否被私自调高，或是否存在未复用未释放的临时堆内存；排查是否有 `Data(count:)` 产生内核清零页。
- **若系统解压报 `CRC32 mismatch` 或 `corrupted data`**：
  检查非末尾分块末尾是否正确追加了 `0x00 0x00 0xFF 0xFF` 字节对齐空存储块，或确认首字节 `BFINAL` 位是否被正确置零。

---

## 2. 验证场景 2：libdeflate v1.22 Universal 2 自动化构建与架构断言

### Command
```bash
# 执行自动化编译并校验 fat binary 架构切片
./scripts/build_libdeflate.sh
lipo -info Vendor/lib/libdeflate.a
```

### Expected Output
```text
Architectures in the fat file: Vendor/lib/libdeflate.a are: arm64 x86_64
✅ libdeflate v1.22 Universal 2 build & deployment completed successfully!
```

### Failure Diagnostic
- **若报错 `lipo: can't open input file`**：
  检查 `cmake` 是否已在当前 PATH 中，或 CMake 构建输出目录是否匹配。
- **若缺少 `x86_64` 切片**：
  检查 Xcode Command Line Tools 是否支持 macOS SDK 跨架构编译，确认 `-DCMAKE_OSX_ARCHITECTURES="arm64;x86_64"` 参数是否生效。

---

## 3. 验证场景 3：CMake 跨平台构建与 MSVC CTTZipPlatform 校验

### Command
```bash
# 配置并测试跨平台 CMake 构建工程
cmake -B build_cmake_test -S . -DTTZIP_BUILD_TESTS=OFF
cmake --build build_cmake_test --target CTTZipBridge
```

### Expected Output
```text
[100%] Built target CTTZipBridge
```

### Failure Diagnostic
- **若报错找不到 `<unistd.h>` 或 `__thread` 语法错误**：
  检查报错源文件是否已包含 `CTTZipPlatform.h` 并使用 `TTZIP_THREAD_LOCAL` 宏替代原生编译器关键字。
- **若出现 MSVC 符号重定义或链接失败**：
  检查 `ttzip_platform.h` 中的内联函数是否包含 `static inline` 修饰符。

---

## 4. 验证场景 4：全量单元测试与性能门禁零倒退

### Command
```bash
# 运行全量 525+ 单元测试与热路径性能门禁
swift test
swift test --filter XCTestPerformanceMeasureTests
```

### Expected Output
```text
Test Suite 'All tests' passed at ...
	 Executed 525+ tests, with 0 failures (0 unexpected)
[XCTestPerformanceMeasureTests] ZIP Level 1: >= 1500 MB/s (PASSED)
[XCTestPerformanceMeasureTests] ZIP Decompress: >= 7500 MB/s (PASSED)
```

### Failure Diagnostic
- **若性能门禁出现测试红灯**：
  排查是否在热路径（$\le 256\text{MB}$）中引入了动态锁或多余分支，必须确保 Whole-Buffer 模式完全旁路直通。
