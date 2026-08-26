# Phase 0 Technical Research: Full-Matrix libdeflate Architecture

**Feature Directory**: `specs/053-chunked-deflate-compressor`
**Created**: 2026-08-17
**Status**: Completed

---

## R001 [SUBAGENT:research] 《DEFLATE 分块流式多线程无缝拼接与 BFINAL 机制》

### 1. Decision (选定方案)
- **自适应双轨路由**：
  - 小/常规文件（$\le 256\text{MB}$）：直通既有 Whole-Buffer TLS Fast-Path，单次调用 `libdeflate_deflate_compress`，保持 $\ge 2000\text{MB/s}$ 历史最优单文件吞吐。
  - 超大文件（$> 256\text{MB}$）：自动切入 1MB 分块流式管道，设置 `MAX_IN_FLIGHT = 32` 有界内存槽位，确保进程常驻内存（RSS）增量严格 $\le 64\text{MB}$。
- **RFC 1951 标准同步空存储块拼接 (Byte-Aligned Stored Block Sync / pigz Pattern)**：
  - 中间分块（第 $0 \dots N-2$ 块）：多线程压缩产物的首字节 `BFINAL` 清零（`byte[0] &= 0xFE`），块尾部追加 RFC 1951 规定的 4 字节标准字节对齐同步序列 `0x00, 0x00, 0xFF, 0xFF`。
  - 最终分块（第 $N-1$ 块）：保持 `BFINAL=1` 或在末尾追加 5 字节 RFC 1951 终结空存储块 `0x01, 0x00, 0x00, 0xFF, 0xFF`。
- **流式增量 CRC-32 累加**：在 1MB 切块读入时，利用 `libdeflate_crc32` / ARMv8 PMULL 硬件指令按分块增量计算全局 CRC-32，统一在 ZIP 头部与中央目录回写。

### 2. Rationale (选择理由)
- **100% 遵循 RFC 1951 与 PKWARE 规范**：RFC 1951 Section 3.2.4 明确规定空存储块（Stored Block, `BTYPE=00`）将忽略当前字节剩余 bit 并对齐到下一字节边界，是 `pigz`、`zlib (Z_SYNC_FLUSH)`、`ISA-L` 在并行 DEFLATE 拼接中的工业标准事实方案。
- **零解压端依赖**：生成的 ZIP 归档在 macOS Archive Utility、系统 `/usr/bin/unzip`、7-Zip、Windows 资源管理器及 Linux 环境下 100% 免配置无损解压。
- **零写线程位移开销**：由于各块在字节边界自然对齐，写入端线程只需按 Sequence 顺序执行标准 `write()` / Direct I/O，无需在写入线程逐字节做高开销的 Bit-shifting。
- **内存确界保障**：1MB 块尺寸 $\times$ 32 槽位 = 64MB 理论峰值常驻内存，满足宪法级内存门禁。

### 3. Alternatives Considered (被否决方案及理由)
- **被否决方案 1：直接复用 GZ 的 Multi-Member 拼接方案包装入 ZIP**
  - *否决理由*：ZIP Method 8 严格要求单一 DEFLATE 流，不支持把带 GZIP Header (0x1F8B) 与 GZIP Trailer 的 Member 串联当作 ZIP Payload，会导致标准 unzip 报 `invalid compression method` 或 CRC 校验失败。
- **被否决方案 2：写入端逐 bit 移位拼接 (Bitstream Shifting & Packing)**
  - *否决理由*：`libdeflate` 公开 API 不暴露末尾精确 bit 偏移；且在单核写入线程上对每秒数 GB 的压缩数据逐字节执行位移运算（Bit-shift）会严重拖慢流水线，使写线程成为性能瓶颈，违背热路径零成本抽象铁律。
- **被否决方案 3：全面放弃 libdeflate 改用传统 zlib deflateInit2 进行流式串行压缩**
  - *否决理由*：传统 zlib 单核压缩吞吐仅 80~120 MB/s，性能比 `libdeflate` 慢 3x~5x，无法满足 Apple Silicon 平台 $\ge 800\text{MB/s}$ 的硬性能门禁。

### 4. Source (查阅来源)
- `Sources/CTTZipBridge/CTTZipBridge_GzParallel.c:18-19, 101-163, 239-253`
- `Sources/CTTZipBridge/CTTZipStreamCoder.c:8-42`
- `Sources/CTTZipBridge/CTTZipBridge_ZipWrite.c:265-274`
- `Vendor/include/libdeflate.h:69-115, 211-240`
- IETF RFC 1951 (DEFLATE Compressed Data Format Specification) Section 3.2.3 & 3.2.4
- IETF RFC 1952 (GZIP File Format Specification) Section 2.2
- PKWARE APPNOTE.TXT (.ZIP File Format Specification) Section 4.4.4

---

## R002 [SUBAGENT:research] 《libdeflate v1.21+ 源码升级与 macOS Universal 2 自动化编译参数》

### 1. Decision (选定方案)
- **版本选型与管理策略**：
  - 选定升级至官方稳定标签 `v1.22`（内建 ARMv8.2-A+crypto/PMULL 与 AVX-VNNI/AVX512 增强指令集，并完善了 binutils/Clang 汇编器探测逻辑）。
  - 双轨落盘：公用头文件同步至 `Vendor/include/libdeflate.h` 与 `Vendor/TTZipVendor.xcframework/macos-arm64/Headers/libdeflate.h`，静态库产物同步至 `Vendor/lib/libdeflate.a` 并通过 `libtool -static` 聚合打包进 `Vendor/libTTZipVendor.a` 与 `Vendor/TTZipVendor.xcframework`。
- **CMake 编译参数矩阵**：
  ```bash
  cmake -B build -S . \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_OSX_ARCHITECTURES="arm64;x86_64" \
    -DCMAKE_OSX_DEPLOYMENT_TARGET=14.0 \
    -DCMAKE_C_FLAGS_RELEASE="-O3" \
    -DLIBDEFLATE_BUILD_STATIC_LIB=ON \
    -DLIBDEFLATE_BUILD_SHARED_LIB=OFF \
    -DLIBDEFLATE_BUILD_GZIP=OFF \
    -DLIBDEFLATE_BUILD_TESTS=OFF
  ```
- **自动化构建脚本设计 (`scripts/build_libdeflate.sh`)**：
  具备自动化全生命周期：环境探测 ➔ 浅克隆指定 Tag ➔ CMake Universal 2 交叉编译 ➔ `lipo -info` 架构物理断言 ➔ 产物复制到 `Vendor/lib/` 与 `Vendor/include/` ➔ 重新打包 `Vendor/libTTZipVendor.a` 与 `TTZipVendor.xcframework` ➔ 自动校验。

### 2. Rationale (选择理由)
- **构建系统演进与跨架构支持**：`libdeflate` 官方在近版本中全面转向 CMake。通过 `-DCMAKE_OSX_ARCHITECTURES="arm64;x86_64"` 与 Apple Clang 配合，无需手动分别编译两套 arch 再用 `lipo` 手工缝合，CMake 内部自动生成多架构 Mach-O fat 静态库。
- **SIMD 与指令集运行时多态派发**：`libdeflate` 内部针对不同架构实现了运行时 CPU 特性探测与专用函数多态派发机制。在 Clang 下开启 `-O3` 时，编译器会保留所有架构专有的快速函数切片，无需在编译期强制锁定全局单一 CPU 架构。
- **消除无效目标与打包最小化**：关闭共享库与 CLI 工具构建，阻断 `.dylib` 与 `libdeflate-gzip` 可执行文件的构建，保持静态库纯净性。
- **SPM 与 Vendor 架构无缝集成**：脚本自动联动更新 `Vendor/lib/libdeflate.a` 与 `Vendor/TTZipVendor.xcframework`，保证 SPM 构建链路无需调整任何配置。

### 3. Alternatives Considered (被否决方案及理由)
- **被否决方案 1：使用 GNU Makefile + 手动 `lipo -create` 组合**
  - *否决理由*：`libdeflate` 官方已在后续版本中弃用原生 Makefile 构建系统，维护成本高；且需要执行两次全量 make 再调用 `lipo -create` 合并，链路冗长且易错。
- **被否决方案 2：将 libdeflate 源码直接作为 SPM C Target 源码内联编译**
  - *否决理由*：`libdeflate` 源码包含大量根据不同架构条件编译的汇编文件及平台特性检测头文件，直接内联进 Swift Package 的 C Target 会导致 SPM 在跨平台/跨架构时的编译参数配置过于复杂，且无法与现有的 `TTZipVendor.xcframework` 预编译静态库标准管理体系保持一致。
- **被否决方案 3：开启 `-DLIBDEFLATE_BUILD_SHARED_LIB=ON` 采用 dylib 动态库分发**
  - *否决理由*：TTZip 面向 Mac App Store (MAS 沙盒) 与 Direct 独立分发。采用动态库会导致额外的代码签名、@rpath 加载配置与沙盒隔离复杂性，违反项目 100% In-Process C 静态绑定的性能与安全要求。

### 4. Source (查阅来源)
- `/Users/kevintung/Documents/dev/TTZip/Package.swift:28-48`
- `/Users/kevintung/Documents/dev/TTZip/ACKNOWLEDGEMENTS.md:20`
- `/Users/kevintung/Documents/dev/TTZip/Vendor/TTZipVendor.xcframework/Info.plist`
- `Vendor/include/libdeflate.h` 与 `Vendor/TTZipVendor.xcframework/macos-arm64/Headers/libdeflate.h`
- GitHub `ebiggers/libdeflate` 官方仓库 `CMakeLists.txt`

---

## R003 [SUBAGENT:research] 《Windows MSVC / CMake 跨平台 C 桥接层与符号导出设计》

### 1. Decision (选定方案)
- **建立统一平台抽象层 `CTTZipPlatform.h` (PAL)**：
  在 `Sources/CTTZipBridge/include/` 中引入完备的 PAL，集中封装 `TTZIP_THREAD_LOCAL`、`TTZIP_API`、`ttzip_sleep_ms`、`ttzip_secure_zero`、`ssize_t`、`O_BINARY` 以及预取宏，全面替换直接包含 `<unistd.h>`、`<compression.h>` 与裸 `__thread` 的语法。
- **根目录落地跨平台 `CMakeLists.txt`**：
  采用模块化结构，以 `MSVC` + `/utf-8` + `/MD` 作为 Windows 编译基线，对 ARM NEON 与 Apple 专有模块（如 `APFS`、`LZFSE`）实施平台条件编译，支持自动化生成 Windows x86_64 与 ARM64 的 `CTTZipBridge.lib`。
- **分层保留 Fast-Path 旁路**：
  macOS 平台继续直通 AppleClang SIMD 与 APFS 零拷贝，Windows/Linux 平台无缝回退至 Win32 BCrypt / lz4 开源实现与 `Sleep()`。

### 2. Rationale (选择理由)
- **彻底消除跨编译器编译阻断**：MSVC 在 C 模式下对 POSIX 头文件（`<unistd.h>`）和 GCC 扩展（`__thread`, `__builtin_prefetch`）零容忍。通过统一在头文件最前置注入 `CTTZipPlatform.h`，无需在数十个 C 源文件中书写复杂的平台 `#ifdef`，保证代码纯净与可维护性。
- **Windows 二进制 I/O 正确性**：Windows CRT 默认以文本模式打开文件，必须显式定义 `O_BINARY` 并在 MSVC 下映射到 `_O_BINARY`，防止回车换行符转义导致压缩包标头和 CRC32 损坏。
- **零运行时开销**：所有平台抽象均采用 `static inline`、预处理器宏或编译期常量，在 Release 模式下完全内联，达成热路径零成本抽象。

### 3. Alternatives Considered (被否决方案及理由)
- **被否决方案 1：引入外部第三方跨平台兼容库（如 POSIX-for-Windows、Pthreads-w32 或 libuv）**
  - *否决理由*：TTZip 严格遵循“零冗余依赖与热路径零成本抽象”原则。引入庞大的第三方抽象库会急剧增加二进制体积与构建复杂度，且动态封装会导致热路径调用开销，违背 TTZip 性能铁律。
- **被否决方案 2：在每个 `.c` 文件内部直接散落书写 `#ifdef _WIN32`**
  - *否决理由*：导致代码重复率极高、维护困难，且极易遗漏 `O_BINARY` 或 `ssize_t` 等边缘类型定义，违反 TTZip 架构整洁与防御性编程铁律。

### 4. Source (查阅来源)
- `/Users/kevintung/Documents/dev/TTZip/Package.swift`
- `/Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/ttzip_platform.h`
- `/Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/ttzip_windows.h`
- `/Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/CTTZipCommon.h`
- `/Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipStreamCoder.c`
- `/Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipExtract.c`
- `/Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipPlatformTimer.c`
- ISO/IEC 9899:2011 (C11 Standard) §6.7.3.1
- Microsoft Learn: Thread Local Storage (TLS) Specification
