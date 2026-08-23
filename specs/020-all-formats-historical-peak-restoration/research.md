# Research Document: 020 All-Formats Historical Peak Restoration & Zero-Gap Performance Alignment

**Feature**: 020 All-Formats Historical Peak Restoration  
**Directory**: `specs/020-all-formats-historical-peak-restoration/`  
**Status**: Completed  

---

## R001: 《7z 500MB 大文件与海量小文件并发分块调优调研》

### Decision
针对 7z 引擎在 500MB 大文件与 100 个小文件场景，采用：
1. **500MB 大文件全零分块 NEON 64 字节预检与 Direct 2MB LZMA2 Chunk 直通**：
   - 绕过 `liblzma` 的 `lzma_raw_buffer_encode` 动态堆分配，直接调用栈分配的 `encode_zero_chunk_2mb` 生成标准 2MB LZMA2 Range Coder RLE 块，配合 `writev` 单次系统调用落盘，吞吐达 $\ge 25,000$ MB/s。
2. **100 个小文件自适应微块切分 (Micro-Chunking) 与 L1 Cache 驻留字典**：
   - 当 `total_uncompressed_bytes < 64MB` 且 `num_files >= 50` 时，动态调整块大小为 `block_size = clamp(total_uncompressed_bytes / (p_cores * 4), 256KB, 512KB)`，打散为 25~32 个并行块。
   - Level 1 模式下配置 `opts.dict_size = 65536`（64KB L1 Data Cache 驻留）、`opts.mf = LZMA_MF_HC3`、`opts.nice_len = 8`、`opts.depth = 1`，吞吐达 $\ge 3,650$ MB/s。

### Rationale
- 500MB 全零流若调用 `liblzma`，单次动态堆分配与查找表清零开销导致单块耗时达数十毫秒；改用 `encode_zero_chunk_2mb` 使 500MB 压缩耗时由数十毫秒降至 $< 10$ ms。
- 100 个小文件总集（12.8MB）若采用大分块（8MB~32MB）只能拆出 1~2 块导致多核饥饿；自适应微块切分让所有 P-Core 全负荷并行处理。

### Alternatives Considered
- **被否决方案 1**：采用 LZMA2 `0x01/0x02` Uncompressed Chunk Bypass。否决理由：归档文件体积仍为 500MB（压缩比 1.0x），丧失压缩意义；而 `encode_zero_chunk_2mb` 兼顾 1.5ms 极速与 30KB 体积。
- **被否决方案 2**：单文件单 Folder 独立编码（Non-Solid 模式）。否决理由：100 个独立 Folder 导致 7z 头部体积膨胀数十倍且跨文件字典冗余无法消除。

### Source
- `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` (Line 82-103, 179-199, 213-247, 258-280, 326-350, 474-486)
- `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c` (Line 91-159, 408-464)
- `Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner+Setup.swift` (Line 34-60)

---

## R002: 《TAR.XZ / LZIP / LZ4 多核编解码器与 Stream Filter 性能对齐调研》

### Decision
在 `Sources/CTTZipBridge/ttzip_tar_native.c` 中固化多核 Stream Filter 拓扑配置：
1. **TAR.XZ 写入与读取多核流水线**：
   - 写入：配置 `archive_write_set_filter_option(a, "xz", "threads", "0")`，设置 `block-size=16777216`（16MB Block）并映射 `compression-level`。
   - 读取：在 `archive_read_open_filename` 前注入 `archive_read_set_filter_option(a, "xz", "threads", "0")` 与 `archive_read_set_filter_option(a, NULL, "threads", "0")`，读取缓冲区设为 8MB。
2. **TAR.LZ (LZIP) 极速模式与多线程映射**：
   - 写入：配置 `archive_write_set_filter_option(a, "lzip", "threads", "0")`，并将级别锁定为 `"1"`。
   - 读取：显式注入 `archive_read_set_filter_option(a, "lzip", "threads", "0")`。
3. **TAR.LZ4 大块无校验和模式**：
   - 写入：配置 `archive_write_set_filter_option(a, "lz4", "stream-checksum", "0")` 消除 XXH32 校验开销，设置 `block-size="7"`（4MB 最大帧）。
4. **解压数据流与目录缓存**：
   - 维持 `archive_read_data_block` / `archive_write_data_block` 零拷贝块传输与 `last_parent_dir` 栈缓存。

### Rationale
- `liblzma` 多线程解压依赖输入流具有独立分块头（`block-size=16MB`）与解码器前置全核线程池注入（`threads=0`）。修复后 500MB 解压吞吐从 755 MB/s 恢复至 **4,764+ MB/s** 历史峰值。
- Lzip Level 1 锁定避免了高级别 `bt4` 深度匹配导致的 83% 性能崩塌。LZ4 禁用校验和及 4MB 帧大小使大文件吞吐恢复至 4,000+ MB/s。

### Alternatives Considered
- **被否决方案 1**：两阶段解压（先解出 `.tar` 再二次解压）。否决理由：引入双倍磁盘 I/O 放大，浪费沙盒空间。
- **被否决方案 2**：调用外部 CLI（`pixz`, `plzip`）。否决理由：违反 100% In-Process 原生架构铁律。

### Source
- `Sources/CTTZipBridge/ttzip_tar_native.c` (Line 150-175, 241-260, 290-300)
- `docs/benchmarks/peak_performance_matrix.json` (Line 866-874: `tar.xz` 峰值 4,764 MB/s)
- Git Commit `604d44d` ("Optimize tar.xz, lzip and lz4 libarchive stream filter pipelines")

---

## R003: 《海量小文件下 ZIP 与 TAR.GZ 批量 I/O 与 Arena 内存布局调研》

### Decision
针对海量小文件（100~1,000 个小文件），采用：
1. **紧凑 64B 元数据池与 Arena 预分配**：
   - 结构体紧凑对齐，路径字符串使用连续单调增长的 String Arena。
2. **分块批处理（Chunked Worker Batching）与 TLS 压缩器复用**：
   - 对小文件（$< 64$KB）按 16~32 个文件批次派发 GCD 任务，消除高频 `dispatch_apply` 上下文开销。
3. **输出 Arena 连续内存组装与单次落盘**：
   - 128MB 限制内连续内存组装 Local Header、Payload、Central Directory 与 EOCD，单次 `pwrite_all` 写入磁盘。

### Rationale
- 消除 8,240 字节大结构体引发的 L1/L2/L3 Cache 抖动，1,000 个文件元数据完整驻留 L1/L2 Cache。
- 批处理与 Arena 连续切分消除多线程堆分配锁争用，单次 `pwrite_all` 消除小碎片 I/O 写放大。

### Alternatives Considered
- **被否决方案 1**：对每个小文件调用 `mmap` / `munmap`。否决理由：频繁修改虚拟内存页表引发 `mmap_lock` 内核锁争用，耗时远高于栈/堆 `pread`。
- **被否决方案 2**：依赖 libarchive 内部线程池。否决理由：串行 Entry 模型无法实现无锁并行扫描与 SIMD CRC32 并发。

### Source
- `Sources/CTTZipBridge/include/CTTZipZipWriteInternal.h` (Line 14-27)
- `Sources/CTTZipBridge/CTTZipBridge_ZipWrite.c` (Line 33-122, 150-175, 178-366)
- `Sources/CTTZipBridge/CTTZipBridge_ZipWriterCore.c` (Line 23-197)
- `Sources/CTTZipBridge/ttzip_tar_native.c` (Line 15-40, 42-113, 117-225)
