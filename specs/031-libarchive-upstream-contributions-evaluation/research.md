# Technical Research & Upstream Feasibility Analysis (031-libarchive-upstream-contributions-evaluation)

## Research Overview

本研究基于 upstream `libarchive` 源码（[`Vendor/libarchive-upstream/libarchive/`](file:///Users/kevintung/Documents/dev/TTZip/Vendor/libarchive-upstream/libarchive/)）与 TTZip 底层优化实现（[`Sources/CTTZipBridge/`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/)、[`Sources/TTZipCore/`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/)）进行逐项对比，提取并论证除 PR #3388（7z AES-256 解密）外最具价值的上游开源贡献。

---

## Item R001: 7-Zip AES-256 写入端加密支持 (`archive_write_set_format_7zip.c`)

- **Decision**: **采纳为 Tier 1 最高优先级贡献 PR**。基于 PR #3388 中已构建的跨平台 KDF (`kdf_7z_sha256()`) 与 `archive_cryptor` 基础设施，向 `archive_write_set_format_7zip.c` 补充写入端对称加密流水线与 Multi-Coder `BindPairs` 支持。
- **Rationale**:
  1. **功能闭环**：PR #3388 仅覆盖了解密读取端（Read-side），写入端（Write-side）是完成 7z 加密支持的最后一块拼图。
  2. **社区强烈需求**：libarchive issue 列表中关于 7z 写入加密的诉求集中（Issue #878 等）。
  3. **架构完全对齐**：对称加密接口（CommonCrypto, Windows CNG, OpenSSL EVP, mbedTLS）已在 `archive_cryptor.c` 就绪，无需新增外部依赖。
- **Alternatives Considered**:
  - *替代方案 A：仅在 TTZip 内部维护 7z 写入加密*：否决。TTZip 内部固然可直接调用 7zz 或自研写入器，但 upstream 缺失该能力会导致依赖 libarchive 的数万个开源项目无法生成加密 7z 归档。
  - *替代方案 B：在 PR #3388 中一并提交写入端*：否决。单 PR 代码变更超过 1500 行会极大增加审核阻力，拆分为 Read PR (#3388) 与 Write PR 是最佳工程实践。
- **Source**:
  - Upstream: [`Vendor/libarchive-upstream/libarchive/archive_write_set_format_7zip.c:234, 912, 948`](file:///Users/kevintung/Documents/dev/TTZip/Vendor/libarchive-upstream/libarchive/archive_write_set_format_7zip.c#L234), [`Vendor/libarchive-upstream/libarchive/archive_cryptor.c:838-897`](file:///Users/kevintung/Documents/dev/TTZip/Vendor/libarchive-upstream/libarchive/archive_cryptor.c#L838-L897)
  - TTZip: [`Sources/CTTZipBridge/CTTZipBridge_Crypto.c:460-510`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipBridge_Crypto.c#L460-L510), [`Sources/CTTZipBridge/ttzip_7z_header_writer.c:1-120`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_7z_header_writer.c#L1-L120)

---

## Item R002: CRC32 硬件加速指令集改造 (`archive_crc32.h`)

- **Decision**: **采纳为 Tier 1 最高优先级贡献 PR**。在 `archive_crc32.h` 中引入 ARMv8 ACLE `__crc32*` 与 x86 SSE4.2 / PCLMUL 硬件指令加速，保留 256 元素静态表作为兜底 fallback。
- **Rationale**:
  1. **历史性能瓶颈突破**：upstream 现有单字节标量查表吞吐仅 ~300 MB/s（源码注释明确说明："runs about 300MB/s on my 3GHz P4"），在无 zlib 环境下严重拖累 7z/Zip/RAR/LZOP 性能。
  2. **10x~100x 吞吐提升**：ARM64 ACLE 指令单核吞吐达 10+ ~ 30+ GB/s。
  3. **零破坏与纯宏包装**：通过 `#if defined(__ARM_FEATURE_CRC32)` 与 `#if defined(__SSE4_2__)` 条件编译，非支持平台完全无感回退，保持 100% ABI 兼容。
- **Alternatives Considered**:
  - *替代方案 A：强制要求所有环境链接 zlib*：否决。嵌入式或轻量级构建环境常关闭 zlib，`archive_crc32.h` 是 libarchive 的独立兜底保障，硬件加速内嵌价值极高。
  - *替代方案 B：引入外部 libdeflate 库依赖*：否决。libarchive 秉持极小依赖原则，直接使用编译器内置 ACLE / SSE Intrinsics 是最轻量纯粹的方案。
- **Source**:
  - Upstream: [`Vendor/libarchive-upstream/libarchive/archive_crc32.h:36-121`](file:///Users/kevintung/Documents/dev/TTZip/Vendor/libarchive-upstream/libarchive/archive_crc32.h#L36-L121)
  - TTZip: [`Sources/CTTZipBridge/CTTZipCRC32Neon.c:1-15`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipCRC32Neon.c#L1-L15), [`Sources/CTTZipBridge/CTTZipUtils.c:63-145`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipUtils.c#L63-L145)

---

## Item R003: POSIX / Darwin 磁盘空间预分配 (`archive_write_disk_posix.c`)

- **Decision**: **采纳为 Tier 1 贡献 PR**。在 `archive_write_disk_posix.c` 中为已知大小的常规文件增加物理预分配逻辑（Darwin 使用 `fcntl(F_PREALLOCATE)`，Linux/POSIX 使用 `posix_fallocate()`），并引入 `ARCHIVE_EXTRACT_PREALLOCATE` 提取标志。
- **Rationale**:
  1. **消除频繁扩容系统开销**：解压大文件时逐块 16KB 写入会导致文件系统持续分配 extent 块与更新元数据 B-Tree，预分配一次性获取连续存储块。
  2. **提早捕获磁盘空间不足**：避免解压至 99% 时因磁盘耗尽导致脏文件残留。
  3. **标准 POSIX 语义**：`posix_fallocate` 是标准 POSIX.1-2001 API，兼容性极佳。
- **Alternatives Considered**:
  - *替代方案 A：仅调用 `ftruncate` 扩容*：否决。`ftruncate` 仅修改逻辑文件大小，产生稀疏空洞，并不分配物理块，无法避免写入时的碎片与后续断电数据不一致。
  - *替代方案 B：默认无条件开启预分配*：否决。部分稀疏文件（Sparse File）或特殊文件系统可能不希望物理预分配，增加显式 `ARCHIVE_EXTRACT_PREALLOCATE` 控制标志最为稳妥。
- **Source**:
  - Upstream: [`Vendor/libarchive-upstream/libarchive/archive_write_disk_posix.c:990-1052, 1714-1792`](file:///Users/kevintung/Documents/dev/TTZip/Vendor/libarchive-upstream/libarchive/archive_write_disk_posix.c#L990-L1052), [`Vendor/libarchive-upstream/libarchive/archive.h:707-747`](file:///Users/kevintung/Documents/dev/TTZip/Vendor/libarchive-upstream/libarchive/archive.h#L707-L747)
  - TTZip: [`Sources/CTTZipBridge/CTTZipBridge_APFS.c:14-29`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipBridge_APFS.c#L14-L29)

---

## Item R004: ARM64 NEON BCJ 可执行指令向量化加速 (`archive_read_support_format_7zip.c`)

- **Decision**: **采纳为 Tier 2 贡献 PR**。将 `arm64_Convert` 中的逐条指令标量循环改造为 128 位 NEON 向量化跳步算法。
- **Rationale**:
  1. **吞吐提升显著**：可执行二进制文件中大部分并非跳转指令，128-bit NEON（`vld1q_u32`, `vandq_u32`, `vceqq_u32`, `vmaxvq_u32`）可一次检测 4 条指令，若全无跳转则 1 周期跳步 16 字节，整体转换耗时缩减 75%~85%。
  2. **局部高内聚**：改动局限于 `archive_read_support_format_7zip.c` 内的单个静态函数，审查风险极低。
- **Alternatives Considered**:
  - *替代方案 A：同时重写 x86/Thumb/PowerPC 的 SIMD 转换*：否决。先以 ARM64 作为基准验证 PR，待合并后再扩展其他架构。
- **Source**:
  - Upstream: [`Vendor/libarchive-upstream/libarchive/archive_read_support_format_7zip.c:4546-4597`](file:///Users/kevintung/Documents/dev/TTZip/Vendor/libarchive-upstream/libarchive/archive_read_support_format_7zip.c#L4546-L4597)
  - TTZip: [`Sources/CTTZipBridge/ttzip_bcj_arm64_neon.c:11-120`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_bcj_arm64_neon.c#L1-L120)

---

## Item R005: `mmap` + `posix_madvise` 顺序文件读取后端 (`archive_read_open_filename.c`)

- **Decision**: **采纳为 Tier 2 贡献 PR**。兑现 `archive_read_open_filename.c:439` 中标注的 TODO，为支持虚拟内存映射的系统引入可选的 `mmap` + `POSIX_MADV_SEQUENTIAL | POSIX_MADV_WILLNEED` 读取路径。
- **Rationale**:
  1. **顺应上游既定架构愿景**：源码中存在长达数年的 TODO 注释，明确期望 `mmap` 替代逐块 `read()`。
  2. **消除用户态/内核态拷贝**：大归档（如几 GB 的 TAR/ISO/ZIP）打开时可大幅降低 CPU 与系统调用开销。
- **Alternatives Considered**:
  - *替代方案 A：完全废弃 `read()` 仅保留 `mmap`*：否决。管道（Pipe）、FIFO、字符设备或 32 位地址空间受限系统仍必须使用流式 `read()`。
- **Source**:
  - Upstream: [`Vendor/libarchive-upstream/libarchive/archive_read_open_filename.c:429-465`](file:///Users/kevintung/Documents/dev/TTZip/Vendor/libarchive-upstream/libarchive/archive_read_open_filename.c#L429-L465)
  - TTZip: [`Sources/CTTZipBridge/CTTZipBridge_Mmap.c:30-53`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipBridge_Mmap.c#L30-L53)

---

## Item R006: Apple BSD LZFSE 压缩流过滤器 (`archive_read_support_filter_lzfse.c`)

- **Decision**: **采纳为 Tier 2 贡献 PR**。新增对 Apple 开源 `lzfse` 算法的流式过滤器支持。
- **Rationale**:
  - Apple 已将 `lzfse` 以 BSD 协议开源，Linux 和 BSD 均可直接编译。增加 `lzfse` 过滤器类似于现有的 `lz4`/`zstd` 支持，丰富了 libarchive 的现代压缩生态。
- **Alternatives Considered**:
  - *替代方案 A：同时引入 Apple Archive (AAR) 容器格式*：否决。AAR 容器深度依赖 macOS 专有文件系统属性，格式复杂度高且非通用标准，先行提交独立的 LZFSE 压缩过滤器阻力最小。
- **Source**:
  - Upstream: [`Vendor/libarchive-upstream/libarchive/archive_read_support_filter_lz4.c`](file:///Users/kevintung/Documents/dev/TTZip/Vendor/libarchive-upstream/libarchive/archive_read_support_filter_lz4.c), [`Vendor/libarchive-upstream/libarchive/archive_write_add_filter_lz4.c`](file:///Users/kevintung/Documents/dev/TTZip/Vendor/libarchive-upstream/libarchive/archive_write_add_filter_lz4.c)
  - TTZip: [`Sources/TTZipCore/NativeAppleArchiveEngine.swift:34-40`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/NativeAppleArchiveEngine.swift#L34-L40)

---

## Item R007: libdeflate 全缓冲多线程加速 (建议内部保留)

- **Decision**: **不建议提交 Upstream libarchive，在 TTZip 内部保留为专用 Fast-Path**。
- **Rationale**:
  - `libdeflate` 架构为全内存块原子编解码，不提供可暂停的 `z_stream` 流式重入状态机。libarchive 属于严格基于 Chunk 拉取的流式管道，强行适配需将整个流全量缓存于堆中，违背其流式内存控制哲学。libarchive 官方更推荐链接 API 兼容的 `zlib-ng`。
- **Alternatives Considered**:
  - *替代方案 A：在 libarchive 中引入分块滑动窗口以适配 libdeflate*：否决。代码侵入性极大且无法发挥 libdeflate 无边界匹配的性能优势。
- **Source**:
  - Upstream: [`Vendor/libarchive-upstream/libarchive/archive_write_add_filter_gzip.c:38-68`](file:///Users/kevintung/Documents/dev/TTZip/Vendor/libarchive-upstream/libarchive/archive_write_add_filter_gzip.c#L38-L68)
  - TTZip: [`Sources/CTTZipBridge/CTTZipExtract.c:293-318`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipExtract.c#L293-L318), [`Sources/TTZipCore/LibdeflateAccelerator.swift:1-45`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/LibdeflateAccelerator.swift#L1-L45)
