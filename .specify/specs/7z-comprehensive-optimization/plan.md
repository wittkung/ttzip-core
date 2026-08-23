# Technical Plan: 7Z 引擎全方位深度升级技术方案

## 一、 模块与架构设计

### Phase 1: Thread-Local 解码器池化与零分配
- **文件**：`Sources/CTTZipBridge/ttzip_lzma2_dec_native.c`, `Sources/CTTZipBridge/include/ttzip_lzma2_dec_native.h`
- **方案**：
  - 定义 `static __thread lzma_stream tls_lzma_strm` 与 `static __thread bool tls_lzma_inited`。
  - 使用 `lzma_raw_decoder` 初始化一次后循环复用，每次调用通过重置 `next_in`/`avail_in`/`next_out`/`avail_out` 并调用 `lzma_code(&strm, LZMA_FINISH)` 彻底消除每个 Block 的 malloc/free。

### Phase 2: ARM64 NEON BCJ 指令跳转表过滤器
- **文件**：`Sources/CTTZipBridge/ttzip_bcj_arm64_neon.c`, `Sources/CTTZipBridge/include/ttzip_bcj_arm64_neon.h`
- **方案**：
  - 利用 NEON 128-bit 向量化识别 ARM64 `B` (`0x14000000`) 与 `BL` (`0x94000000`) 指令，并行完成相对/绝对地址转换。
  - 在 `CTTZipBridge_7zNativeDecoder.c` 与 `CTTZipBridge_7zSolid.c` 中挂载 BCJ 过滤器。

### Phase 3: 7z 固实归档 $O(1)$ 随机访问索引表
- **文件**：`Sources/TTZipCore/SevenZip/SevenZipSeekTable.swift`
- **方案**：
  - 解析 7z Header 的 SubStreamsInfo 与 Folders，构建文件到 Folder/UnpackOffset 的索引。
  - 提供 `extractSingleFile(path:from:destination:)` API，只解压目标文件所属的最小 Solid 块。

### Phase 4: 7z-Zstd 现代混合容器支持
- **文件**：`Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c`, `Sources/CTTZipBridge/CTTZipBridge_7z.c`
- **方案**：
  - 注册 Method ID `0x4F71101`（Zstandard），直接挂接 `libzstd` 原生解码流。
