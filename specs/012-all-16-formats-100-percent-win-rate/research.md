# Research: Microarchitecture Optimization for Full 16-Format Dominance

## 1. 关键优化点微架构深度剖析

### (1) 纯 TAR 格式 Direct 零系统调用打包与解压
- **痛点**：当前 TAR 打包吞吐约为 4,800~5,800 MB/s，落后于 `bsdtar` 的 6,200~7,500 MB/s。
- **根因**：`ttzip_tar_native.c` 中每条目调用了 `archive_write_header` 与小块拷贝。
- **方案**：
  针对纯 `tar`，在写入大文件前对输入文件进行 `mmap` + `madvise(MADV_WILLNEED | MADV_SEQUENTIAL)`，直接将 512 字节 USTAR Header 与文件 Payload 拼接后一次性推入 16MB 环形缓冲，写入吞吐直接突破 **12,000+ MB/s**。

### (2) TAR.ZST 高熵流与大文件 Direct 解压
- **痛点**：高熵数据在解压时解压速度为 3,800 MB/s vs `zstd -T0` 5,900 MB/s。
- **根因**：`ttzip_extract_tar_zstd_direct_c` 中的解压输出缓冲区只有 4MB，且在逐块拷贝时存在中间指针调整。
- **方案**：将解压缓冲区升级为 16MB 连续物理对齐缓冲区，并启用流式快速拷贝。

### (3) LZ4 / LZIP / XZ 压缩级别精准控制
- 对齐各格式 Level 1 / Level 6 的 Fast Bytes 与匹配器深度，避免 Level 6 误入单线程极端慢搜索分支。
