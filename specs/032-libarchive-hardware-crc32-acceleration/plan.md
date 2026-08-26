# Implementation Plan: 032-libarchive-hardware-crc32-acceleration

**Feature**: libarchive `archive_crc32.h` ARMv8 ACLE 与通用硬件加速改造 (032-libarchive-hardware-crc32-acceleration)  
**Status**: DESIGN_COMPLETE  
**Phase**: Phase 1 Design Complete  

---

## 1. Technical Context & Upstream Architectural Baseline

本规划在 `Vendor/libarchive-upstream/libarchive/archive_crc32.h` 中实现 ARMv8 ACLE 硬件加速与纯 C 静态查表兜底，解决 2009 年 256 表串行单字节查表的 720 MB/s 吞吐瓶颈。

- **Upstream Target**: `Vendor/libarchive-upstream/libarchive/archive_crc32.h`
- **Algorithm**: ARMv8 ACLE `__crc32b` (前置对齐与尾部) + `__crc32d` (8 路展开处理 64 字节) + IEEE 802.3 多项式 (`0xEDB88320`)。
- **Invariant**: 纯头文件内联实现，零外部库依赖，100% 接口与多项式兼容。

---

## 2. Constitution Check

- [x] **Zero-Cost Hot Paths**: 硬件指令直接发射，无任何堆内存分配，无冗余系统调用。
- [x] **Platform Compatibility**: 覆盖 Apple Silicon macOS, Linux ARM64, 并包含纯 C99 查表 fallback。
- [x] **Freeze Files**: 未修改 TTZip 内部冻结文件（如 `ZipParallelExtractor.swift` 等）。
- [x] **Logging Discipline**: 内部头文件绝不调用任何裸打印函数。

---

## 3. Phase 0: Outline & Research Index

- - R001 [SUBAGENT:research] 《ARMv8 ACLE 硬件加速与多项式一致性》：验证 ACLE `__crc32*` 与 IEEE 802.3 多项式逐位一致性，排查 x86 SSE4.2 Castagnoli 差异。
- - R002 [SUBAGENT:research] 《8 字节内存对齐与 64 字节展开优化》：分析 Cache Line 跨界惩罚与超标量流水线展开饱和度。

*(完整研究结论见 [`specs/032-libarchive-hardware-crc32-acceleration/research.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/032-libarchive-hardware-crc32-acceleration/research.md))*

---

## 4. Phase 1: Design Artifacts & Schemas

- **Data Model**: [`specs/032-libarchive-hardware-crc32-acceleration/data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/032-libarchive-hardware-crc32-acceleration/data-model.md)
- **Contracts**:
  - `contracts/crc32-context-schema.json` [SUBAGENT:research]
  - `contracts/crc32-benchmark-schema.json` [SUBAGENT:research]
- **Validation Guide**: [`specs/032-libarchive-hardware-crc32-acceleration/quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/032-libarchive-hardware-crc32-acceleration/quickstart.md)

---

## 5. Changes by Component

### Component: Upstream libarchive Header (`Vendor/libarchive-upstream/libarchive/`)

#### [MODIFY] [archive_crc32.h](file:///Users/kevintung/Documents/dev/TTZip/Vendor/libarchive-upstream/libarchive/archive_crc32.h)
- 引入 `LIBARCHIVE_HAS_ARM_CRC32` 宏探测条件编译（ARMv8 ACLE / Apple Silicon）。
- 实现 8 字节前置对齐 + 64 字节（8 路 `__crc32d`）超标量主循环 + 尾部单字节处理。
- 保留原有 256 表作为非支持环境的兜底 fallback。
