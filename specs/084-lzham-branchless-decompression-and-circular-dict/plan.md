# Implementation Plan: 084-lzham-branchless-decompression-and-circular-dict

**Feature Directory**: `specs/084-lzham-branchless-decompression-and-circular-dict`  
**Created**: 2026-08-18  
**Status**: In Planning  
**Spec Reference**: [`spec.md`](spec.md)

---

## 1. Technical Context & Overview

### 1.1 Architecture & Objectives
借鉴 `richgel999/lzham_codec`（MIT / Public Domain）解压状态机中的“分支消除”与 $2^N$ 掩码环形字典更新模型，提升 TTZip 核心解压引擎的指令级并行度（IPC）与吞吐量：
1. **11-Bit 一级直接查表与 64-Bit 寄存器预取**：将高频哈夫曼符号解码从多级二叉树遍历转化为 1 个时钟周期的直接查表，单次内存加载输出 `symbol` 与 `len`；64-bit 预取缓冲与局部寄存器常驻，消除 Load-Hit-Store 停顿与结构体寻址开销。
2. **$2^N$ 掩码无分支环形字典更新**：严格限制字典大小为 2 的幂次方，使用位与掩码 `dict_size_mask` 单周期完成回绕偏移计算；通过 `(MAX(src_ofs, dst_ofs) + match_len) > dict_size_mask` 先验判据进行 Fast-Path / Slow-Path 精准分流，Fast-Path 配合 Apple Silicon ARM64 NEON 向量指令（`vld1q_u8` / `vst1q_u8`）实现高吞吐无分支复制。
3. **CTTZipBridge C 原生解压引擎封装**：构建纯 C11 原生 API（`ttzip_lzham_ring_dict_t`, `ttzip_branchless_huffman_dec_t`），满足 TTZip 热路径零堆分配与流式第一性铁律。

---

## 2. Constitution Check

- [x] **热路径零成本抽象 (Zero-Cost Abstraction)**：解码与字典更新内联在 C 循环中，严禁在热循环内引入动态分配、虚拟分发或加锁操作。
- [x] **Fast-Path 旁路保留**：Fast-Path 直接走 ARM64 NEON 向量拷贝，未跨边界时零分支；Slow-Path 仅在边界回绕与极小自重叠时安全回退。
- [x] **内存与对齐确界 (Bounds-First)**：字典内存采用 `ttzip_platform_aligned_alloc(64, ...)` 64 字节页对齐，所有指针与偏移经过 $2^N$ 掩码与越界断言校验。
- [x] **流式第一性 (Stream-First)**：解压支持基于分块微缓冲与无栈状态机流式暂停与恢复（`Protothreads / Coroutine` 风格）。

---

## 3. Phase 0: Research & Grounding

- R001 [SUBAGENT:research] 《11-Bit 哈夫曼直接查表与 64-bit 预取在 ARM64 NEON 下的微架构优化》：分析 64 位寄存器 bit_buf 预取、32 位大端一次性加载与 11 位查表在 ARM64 汇编指令序列（UBFX, LDR, LSR, ORR）下的表现。
- R002 [SUBAGENT:research] 《$2^N$ 掩码环形字典更新与 Fast-Path NEON 向量拷贝在自重叠/边界下的安全性与加速》：分析 `((MAX(src_ofs, dst_ofs) + match_len) > dict_size_mask)` 先验判据在 ARM64 NEON 下与 `vld1q_u8/vst1q_u8` 批量拷贝结合的实现方案，特别分析 `match_dist == 1` (RLE) 和 `match_dist < 16` 时的重叠安全性。
- R003 [SUBAGENT:research] 《TTZip CTTZipBridge 原生流式 C 桥接与零拷贝架构》：分析如何将 LZHAM 环形字典更新引擎与状态机解耦为纯 C11 原生 API，符合 TTZip 零堆分配与流式第一性铁律。

---

## 4. Phase 1: Design Artifacts

- C001 [SUBAGENT:research] 《数据模型规范》：落盘 [`data-model.md`](data-model.md)，定义环形字典状态结构体、哈夫曼查表加速表结构、比特流预取器。
- C002 [SUBAGENT:research] 《系统边界契约》：落盘 [`contracts/lzham-decomp-api.json`](contracts/lzham-decomp-api.json) 与 [`contracts/circular-dict-engine.json`](contracts/circular-dict-engine.json)。
- C003 [SUBAGENT:research] 《快速验收与验证指南》：落盘 [`quickstart.md`](quickstart.md)。

---

## 5. Proposed Changes & Component Impact

### Component: `Sources/CTTZipBridge/`
- **[NEW]** `include/ttzip_branchless_decomp.h`: 声明 11-bit 直接哈夫曼查表、64-bit 比特流预取与 $2^N$ 掩码环形字典更新的 C11 接口。
- **[NEW]** `ttzip_branchless_decomp.c`: 实现 ARM64 NEON 向量化 Fast-Path 复制、RLE 字节填充与 Slow-Path 安全回绕逻辑。
- **[MODIFY]** `include/CTTZipBridge.h`: 导出通用分支消除解压与环形字典 C 桥接符号。

### Component: `Tests/TTZipTests/`
- **[NEW]** `BranchlessDecompTests.swift`: 针对 11-bit 查表、64-bit 预取缓冲与 $2^N$ 掩码环形字典更新机制编写全覆盖单元测试与微基准测试。
