# Phase 0 Research: 系统工程方法论与分块流式架构研究报告

**Feature Directory**: `specs/039-systemic-engineering-methodology-and-invariants-reconstruction`  
**Date**: 2026-08-16  
**Status**: Completed  
**Sources Baseline**: `Vendor/libarchive-upstream/libarchive/archive_read.c` & `Sources/CTTZipBridge/CTTZipBridge_7zSolid.c`

---

## R001: 7z 分块流式 Solid 压缩状态机与滑动窗口算法

### 1. 核心研究结论
- **全量内存分配的致命缺陷**：
  - 目前 `CTTZipBridge_7zSolid.c:56-61` 将所有待压缩文件合并为单一连续内存 `solid_buf`（`posix_memalign`）。当遇到 10GB~50GB 归档时，直接耗尽系统 RAM 导致 OOM。
- **分块流式 Solid 架构（Chunked Solid Streaming Pipeline）**：
  - 将输入文件流按固定窗口大小（如 32MB / 64MB）划分为独立的 Solid Block。
  - 每个 Block 单独进行 LZMA2 / Deflate 压缩并写入流式输出。
  - 在 7z 头部中通过 `SubStreamsInfo` 记录每个文件在对应 Block 内的子流偏移和真实 CRC32。
  - **内存上限控制**：单线程峰值内存恒定在 $64\text{MB} \sim 128\text{MB}$，与待压缩文件的总大小完全脱钩。

### 2. 决策与替代方案
- **Decision**: 制定 7z 分块流式压缩标准契约 `contracts/chunked_solid_stream_spec.json`，作为底层引擎下一阶段重构的强制架构规范。
- **Rationale**: 践行 Stream-First 原则，彻底消除内存失控隐患。
- **Source**: `Vendor/libarchive-upstream/libarchive/archive_write_set_format_7zip.c`

---

## R002: 系统级工程心智模型与防御性架构落地矩阵

### 1. 核心研究结论
- **四大系统工程铁律心法**：
  1. **流式第一性 (Stream-First)**：拉取模型、微缓冲驱动、零内存假设。
  2. **纵深防御 (Invariant-First)**：安全下沉至 POSIX AT-API、延后 Fixup 倒序回写、硬件防溢出。
  3. **确定性确界 (Bounds-First)**：Magic 首字段生命周期、memset_s 敏感擦除、SSIZE_MAX 转换 Clamp。
  4. **真实预言机 (Oracle-First)**：历史缺陷 .uu 语料库、系统原生 CLI 差分、崩溃优先模糊测试。

### 2. 决策与替代方案
- **Decision**: 将四大铁律正式写入 `.specify/memory/constitution.md`、`GEMINI.md` 与独立架构文档。
- **Rationale**: 固化为最高项目法则，形成跨版本、跨 Agent 的永久防御护城河。
- **Source**: `specs/036-libarchive-standards-architecture-workflow-synthesis/research.md`
