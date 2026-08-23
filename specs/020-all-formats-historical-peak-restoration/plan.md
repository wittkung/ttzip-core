# Implementation Plan: 020 All-Formats Historical Peak Restoration & Zero-Gap Performance Alignment

**Feature Directory**: `specs/020-all-formats-historical-peak-restoration/`  
**Status**: Ready for Tasks  

---

## 1. Technical Context

TTZip 在支持 16 种格式全矩阵压缩与解压的过程中，针对 500MB 零流大文件、100 个小文件以及高熵数据块在部分格式下存在与历史峰值矩阵（`docs/benchmarks/peak_performance_matrix.json`）差距超过 10% 的情况。
本计划旨在全面实施三项核心调优：
1. **7z 500MB 大文件与海量小文件并发分块调优**（`encode_zero_chunk_2mb` 零堆直通 + 自适应微块切分 + L1 Cache 驻留字典）；
2. **TAR.XZ / LZIP / LZ4 多核 Stream Filter 拓扑固化**（`block-size=16MB` + `threads=0` + 8MB 缓冲）；
3. **海量小文件 ZIP 批处理与 128MB Arena 内存级单次落盘**。

---

## 2. Constitution & Performance Check

| 检查项 | 约束要求 | 落实机制 |
| :--- | :--- | :--- |
| **热路径零成本抽象** | 零中间堆分配、零冗余系统调用 | C 引擎 Arena 连续切分，消除 `malloc`/`free` 锁争用 |
| **Fast-Path 旁路保留** | 保留格式专属与硬件特化路径 | 7z / ZIP / TAR 直通原生 C 引擎，零 Task/模板开销 |
| **吞吐硬门禁** | 满足全部 11 项吞吐底线 | 执行 `XCTestPerformanceMeasureTests` 门禁 |
| **零性能倒退铁律** | 严禁核心场景倒退 $> 10.0\%$ | 执行 `audit_performance_regression.py` 自动化审查 |
| **日志与编译规范** | 0 warnings, 0 raw print | 使用 `TTLogger`，编译警告保持 0 |

---

## 3. Phase 0: Research Items

- - R001 [SUBAGENT:research] 《7z 500MB 大文件与海量小文件并发分块调优调研》：已完成，选定 `encode_zero_chunk_2mb` 零堆直通与自适应微块细分方案。
- - R002 [SUBAGENT:research] 《TAR.XZ / LZIP / LZ4 多核编解码器与 Stream Filter 性能对齐调研》：已完成，选定 16MB 分块 + `threads=0` + 8MB 缓冲拓扑方案。
- - R003 [SUBAGENT:research] 《海量小文件下 ZIP 与 TAR.GZ 批量 I/O 与 Arena 内存布局调研》：已完成，选定批处理分块 + 统一 Arena + 单次 `pwrite_all` 方案。

---

## 4. Phase 1: Contracts & Data Model

- [x] `data-model.md`: 数据模型定义与实体字段约束。
- [x] `contracts/fast_path_dispatch.schema.json`: 强类型接口与调度契约（零裸通配）。
- [x] `quickstart.md`: 验收场景与断言命令。

---

## 5. Component Changes Breakdown

### CTTZipBridge (C 桥接层与底层引擎)
- `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c`: 在 `ttzip_lzma2_compress_block_tuned` 中增加 `is_zero_block` 的 `encode_zero_chunk_2mb` 极速直通分支，消除 liblzma 堆分配；优化 Level 1 的 64KB L1 Cache 驻留字典配置。
- `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c`: 在海量小文件场景下启用自适应微块切分（`block_size = clamp(total / (p_cores * 4), 256KB, 512KB)`）。
- `Sources/CTTZipBridge/ttzip_tar_native.c`: 固化 `tar.xz`（`block-size=16MB` + `threads=0`）、`lzip`（Level 1 锁定 + `threads=0`）、`lz4`（`stream-checksum=0` + `block-size=7`）以及 8MB 读取缓冲与 `last_parent_dir` 缓存。
- `Sources/CTTZipBridge/CTTZipBridge_ZipWrite.c` & `CTTZipBridge_ZipWriterCore.c`: 扩展内存聚合缓冲区至 128MB，批处理小文件并执行单次 `pwrite_all` 落盘。

### TTZipCore (Swift 核心调度层)
- `Sources/TTZipCore/ArchiveWriter+Dispatch.swift`: 确保 16 种格式全部直通经过调优的 C 引擎。
- `Sources/TTZipCore/ArchiveExtractor+Dispatch.swift`: 确保 16 种格式解压全部直通 C 引擎 Fast-Path。
