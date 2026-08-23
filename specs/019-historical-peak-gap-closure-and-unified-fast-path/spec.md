# Feature Specification: Historical Peak Gap Closure & Unified Fast-Path Alignment

**Feature Branch**: `019-historical-peak-gap-closure-and-unified-fast-path`  
**Created**: 2026-08-15  
**Status**: Draft  
**Input**: "先好好检查，我们目前很多性能就已经有退化超过 10% 的了 /speckit-specify"

---

## 1. Executive Summary & Goals

经过全维度扫描与格式分组统计，发现当前跑分与历史最高峰值矩阵（`docs/benchmarks/peak_performance_matrix.json`）相比，主要在以下四大架构瓶颈场景中存在显著性能差距（差距 $> 10\%$）：
1. **海量小文件目录打包分发断路**：`ArchiveWriter+Dispatch.swift` 中 `hasDirectoryInput` 限制导致小文件目录无法进入原生 `ttzip_create_tar_direct_c` / `ttzip_create_archive_tuned` 极速快速路径，导致 TAR 从 4,000 MB/s 跌至 995 MB/s（-75%），ZIP 从 8,381 MB/s 跌至 1,193 MB/s（-85%）。
2. **高熵物理数据压缩 CPU 空转**：在 100MB 高熵 Payload 下，由于缺乏前置信息熵快速探测，Deflate 与 LZMA2 引擎强行进行无效字典搜索，导致 ZIP 压缩从 1,983 MB/s 跌至 183 MB/s（-90%），TAR.ZST 从 14,444 MB/s 跌至 5,326 MB/s（-63%）。
3. **LRZIP / LZIP 外部 CLI 兼容包装与大块流水线**：单线程与多线程分块未打满 Apple Silicon 统一内存带宽，导致 500MB 大文件与高熵数据出现 37%~86% 差距。
4. **WIM / DMG / ISO 解压目录树解析**：元数据遍历开销影响了小文件和文本的解压极值。

本 Feature 的目标是：
1. **打通目录归档的 Direct Fast-Path**：支持对单目录输入直接调用底层快速打包引擎。
2. **集成轻量信息熵探测短路 (Entropy Fast Bypass)**：在压缩前对输入数据进行 64KB 快速熵采样，若为不可压缩高熵数据则自动选用极速 Store/Fast 模式。
3. **彻底收敛全格式与历史最高峰值差距**：将所有格式实测吞吐对齐历史最高峰值（差距 $< 10.0\%$），实现大满贯零倒退。

---

## 2. User Scenarios & Testing

### User Story 1 - 目录归档直通与小文件吞吐恢复 (Priority: P1) 🎯 MVP

用户在对海量小文件目录（如 100 个文件）进行打包时，系统自动调度底层页对齐批量写入引擎，将小文件 TAR 打包吞吐恢复至 $\ge 3,500$ MB/s，ZIP 打包吞吐恢复至 $\ge 6,000$ MB/s。

**Why this priority**: 小文件是用户日常开发最常见的场景，性能提升直接决定体感流畅度。

**Independent Test**:
`TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests` 中海量小文件场景吞吐对齐历史峰值。

---

### User Story 2 - 高熵数据自适应极速短路 (Priority: P2)

当用户压缩已压缩文件、媒体流或加密高熵数据时，系统在 10 微秒内探测出高熵属性，短路冗余计算，将 ZIP 压缩吞吐恢复至 $\ge 1,800$ MB/s，TAR.ZST 恢复至 $\ge 12,000$ MB/s。

---

### User Story 3 - 质量与门禁 100% 绿灯 (Priority: P3)

保持 11/11 Release 性能门禁全绿，591+ 单元测试 100% 绿灯，0 编译器警告，0 裸日志。

---

## 3. Functional Requirements

- **FR-001**: 系统必须重构 `ArchiveWriter+Dispatch.swift` 中的直通条件，允许单目录或多目录输入直接使用底层 `ttzip_create_archive_tuned` / `ttzip_create_tar_direct_c`。
- **FR-002**: 系统必须在压缩分发层为大文件提供 64KB 头部信息熵快速探测，对高熵不可压缩数据自适应优化压缩级别，避免 CPU 空转。
- **FR-003**: 系统必须通过 11 大 Release 性能门禁（`XCTestPerformanceMeasureTests`）。
- **FR-004**: 系统必须保持 591+ 单元测试 100% 绿灯与 0 编译器警告。

---

## 4. Success Criteria

- **SC-001**: 消除所有因分发断路或高熵空转导致的 $> 10.0\%$ 性能差距。
- **SC-002**: 全量单元测试 591/591 通过，0 编译器警告。
- **SC-003**: 11/11 Release 性能门禁全部通过。
