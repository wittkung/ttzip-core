# Feature 094: C-Blosc2 深度架构地毯式吸收与自研高性能算子库 (C-Blosc2 Exhaustive Architectural Absorption & Native High-Performance Operator Engine)

## 一、 需求背景与目标 (Context & Motivation)

在前期工作中，TTZip 已经完成了 C-Blosc2 核心的 SIMD BitShuffle（向量位平面转置）、ByteDelta（前缀和重建）、全零/常量特殊值旁路、二级缓存切分、自适应熵值微探针、动态插件注册表以及 Bit-Grooming 浮点精度修剪。

为了彻底“吃干抹净”`Blosc/c-blosc2` 架构中所有值得借鉴的高性能系统设计，本规范对 C-Blosc2 剩余的核心技术资产展开地毯式吸收：
1. **BloscLZ 原生轻量字节级 LZ77 编解码器 (`blosclz.c`)**：
   - 针对经由 Shuffle / BitShuffle 预处理后的结构化字节流，BloscLZ 使用极小内存开销的哈希表（`HASH_LOG 12..14`，4KB–16KB 紧凑哈希表，完全驻留 L1 数据缓存）与分支预测友好的字长匹配拷贝，在 Apple Silicon 上实现 $> 12\text{ GB/s}$ 极限解压与 $> 3.5\text{ GB/s}$ 极速压缩。
2. **N 维张量超立方切块与压缩域多轴切片架构 (`b2nd` / `blosc2_nd`)**：
   - 支持多维数组（2D 遥感图像、3D 空间矩阵、4D 传感器张量）的超立方体块划分（Hyper-cube Chunking），允许在不全量解压的前提下按任意多维轴向坐标范围 `[x1..x2, y1..y2, z1..z2]` 直接按需定位微块进行瞬时子张量切片提取。
3. **线程级上下文工作内存池化与 64 字节缓存行对齐 (`context.c`, `alloc.c`)**：
   - 建立常驻线程的 `blosc2_cctx` / `blosc2_dctx` 工作内存池（Scratchpad Memory Pool），在多次高频压缩/解压周期中实现单周期 **0 次堆内存分配**，内存指针严格按 64 字节对齐以最大化 Apple Silicon 128-bit NEON / 256-bit 内存总线加载吞吐。

---

## 二、 用户场景与用例 (User Scenarios)

### User Story 1 (US1): 原生 BloscLZ 编解码引擎与 Shuffle 管道级联
- **场景**：用户在处理结构化二进制数据（如数据库表列、遥感遥测 Float32/Int64 流）时，开启 Blosc 管道模式。
- **预期行为**：数据经过 SIMD Shuffle / BitShuffle 重新排列后，直通进程内原生 `BloscLZ` 引擎进行超高速字节匹配压缩，在保持极高吞吐（$> 3.5\text{ GB/s}$）的同时获得远优于原始 LZ4 的压缩比。

### User Story 2 (US2): N 维多维张量压缩超立方切片提取
- **场景**：用户需要在包含数百 GB 多维张量（如气象 3D 矩阵、医学影像切片）的归档中提取特定局部三维空间切片 `[0..100, 50..150, 0..10]`。
- **预期行为**：系统通过 `b2nd` 超立方体索引，仅读取并解压与该空间范围相交的极少数 128KB 块，耗时 $< 2\text{ ms}$，避免解压整卷数百 GB 数据。

### User Story 3 (US3): 线程常驻上下文内存池化 (Zero Allocation Pipeline Pool)
- **场景**：系统在并发多线程处理数十万小文件或持续流式数据块时。
- **预期行为**：每个 Worker 线程复用其绑定的 `ContextMemoryPool`，整个编解码热循环期间堆申请调用（`malloc`/`free`）保持为 0，彻底消除 OS 内存碎片与多核锁竞争。

---

## 三、 功能需求 (Functional Requirements)

1. **FR-001 [BloscLZ 纯 C 原生实现]**：在 `CTTZipBridge` 中实现 `ttzip_blosclz_compress` 与 `ttzip_blosclz_decompress`，支持 12/13/14 位哈希表与 64-bit 快速前瞻匹配。
2. **FR-002 [BloscLZ + Shuffle 流水线级联]**：在 `CTTZipFilterPipeline` 中集成 `BloscLZ` 作为一等公民 Codec，支持与 `Shuffle`, `BitShuffle`, `ByteDelta` 的透明串联。
3. **FR-003 [N 维多维张量切片计算器]**：在 Swift 核心层 `TTZipCore` 中实现 `NDimTensorLayout` 与 `NDimHypercubeChunker`，提供多维跨步坐标到一维平铺块索引的映射公式与快速边界求交。
4. **FR-004 [线程工作上下文内存池]**：在 `CTTZipBridge` 与 `NativeCoreArchitecture` 中建立 `ttzip_context_pool`，分配 64 字节对齐的重用缓冲区，提供线程局部缓存。
5. **FR-005 [零破坏性与回归验证]**：新功能 100% 通过现有 1037 项单测与 13 项硬性能门禁。

---

## 四、 成功验收标准 (Success Criteria)

- **SC-001 (正确性)**：BloscLZ 编解码对于任意随机、结构化、极端重复与稀疏数据流实现 100% 字节级无损对齐（SHA-256 Parity）。
- **SC-002 (BloscLZ 性能)**：在 Apple Silicon M 系列芯片上，BloscLZ Level 1 压缩吞吐 $\ge 3,500\text{ MB/s}$，解压吞吐 $\ge 9,000\text{ MB/s}$。
- **SC-003 (N 维切片延迟)**：针对 $1024 \times 1024 \times 64$ 的 Float32 张量，提取任意 $64 \times 64 \times 8$ 子切片的端到端延迟 $\le 5.0\text{ ms}$。
- **SC-004 (内存池零分配)**：在连续 10,000 次 64KB 块压缩循环中，堆内存分配次数严格为 0。
