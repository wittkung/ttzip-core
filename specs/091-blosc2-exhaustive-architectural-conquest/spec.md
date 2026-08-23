# Feature Specification: Blosc2 Exhaustive Architectural Conquest (全景架构穷尽式吸收与集成)

**Feature Directory**: `specs/091-blosc2-exhaustive-architectural-conquest/`  
**Status**: DRAFT  
**Priority**: P1  
**Created**: 2026-08-18  

---

## Executive Summary

基于对 `https://github.com/Blosc/c-blosc2` (BSD 3-Clause) 官方代码库与架构规范的全面地毯式扫描，本规范深入吸纳 C-Blosc2 剩余的高阶核心范式与关键特性，在 TTZip 原生内核中实现：
1. **动态滤镜与编解码插件注册中枢 (Dynamic Filter & Codec Plugin Registry)**：支持自定义前置/后置滤镜与编解码器的动态安全注入与内联极速分发。
2. **微块级懒加载与区间零拷贝切片 (Block-Level Lazy Slicing & Zero-Copy Extraction)**：超大压缩 Chunk 内仅按需解压相交的 128KB 微块，使小区间读取速度提升 $10\times\text{--}50\times$。
3. **浮点精密量化与 Bit-Grooming 滤镜族 (Lossy Precision Quantization & Bit-Grooming Filters)**：支持有效数字截断与动态定点缩放，极大激增后置 BitShuffle 压缩比。
4. **Blosc2 官方 Frame v2 标准容器与 Metalayer 元数据体系 (Blosc2 Frame Container & Metalayer Subsystem)**：提供完整的 Blosc2 Frame 二进制比特流序列化、稀疏分块存储与元数据层穿透。

---

## User Scenarios & Functional Requirements

### User Story 1 (P1): 动态滤镜与编解码插件注册中枢 (Dynamic Plugin Registry)
- **As a** 性能工程师或上层归档模块，
- **I want to** 动态注册自定义预处理/后处理滤镜与编解码器，
- **So that** 系统具备强大的可扩展性，同时保持热路径零虚函数开销与零额外堆分配。

#### Functional Requirements:
1. `FR1.1`: 提供 `ttzip_plugin_register_filter(id, name, forward_fn, backward_fn)` 接口，区分保留 ID ($0\text{--}159$) 与用户扩展 ID ($160\text{--}255$)。
2. `FR1.2`: 提供 `ttzip_plugin_register_codec(id, name, compress_fn, decompress_fn)` 接口。
3. `FR1.3`: 在流水线调度层实现快速分支查表与无锁线程局部调度，内置常用滤镜走直接内联分支，扩展插件走静态函数指针表。

---

### User Story 2 (P1): 微块级懒加载与区间零拷贝切片 (Block-Level Lazy Slicing)
- **As a** 快速预览引擎 (QuickLook) 或大文件检索器，
- **I want to** 仅读取大压缩块中某一个连续字节范围（如 Mach-O 头部、EXIF 标签、文件首尾 4KB），
- **So that** 引擎无需解压整个 4MB--16MB 的完整 Chunk，而是仅解压相交的 128KB L1D 微块。

#### Functional Requirements:
1. `FR2.1`: 提供 `ttzip_schunk_get_slice_buffer(schunk, start_byte, length, dst, dst_capacity)` 接口。
2. `FR2.2`: 核心调度器根据 `start_byte` 和 `length` 精确计算跨越的起始 Chunk、起始 Block、结束 Chunk 与结束 Block。
3. `FR2.3`: 对于未落入目标区间的非相交微块，执行 $0$ 解压操作，直接短路跳过。
4. `FR2.4`: 对于落入特殊值（`SPECIAL_ZERO` / `SPECIAL_VALUE`）区间的切片，直接调用硬件 `memset` 填充，零解压计算开销。

---

### User Story 3 (P2): 浮点精密量化与 Bit-Grooming 滤镜 (Bit-Grooming & Precision Quantization)
- **As a** 科学计算或传感器大数据归档用户，
- **I want to** 对浮点数组应用指定的有效位数保留 (Significant Digits / Bit-Grooming) 与线性量化，
- **So that** 冗余浮点尾数被系统性清零或规整化，与 BitShuffle 产生超强协同增益。

#### Functional Requirements:
1. `FR3.1`: 实现 IEEE-754 Single (Float32) 与 Double (Float64) 的 Bit-Grooming 算法（根据有效位数 $NSD$ 动态屏蔽冗余 mantissa 比特）。
2. `FR3.2`: 实现 Float32 动态线性量化 (`Dynamic Scale & Offset`)，将连续范围浮点数可逆映射为定点整型。
3. `FR3.3`: 在 Swift 层通过 `Blosc2FilterBridge` 对外暴露精度控制参数。

---

### User Story 4 (P2): Blosc2 Frame v2 标准容器与 Metalayers 序列化 (Frame & Metalayers)
- **As a** 跨平台归档系统，
- **I want to** 读写符合 Blosc2 Frame 格式标准（Header, Chunk Index Table, Metalayers, Trailer）的二进制流，
- **So that** TTZip 能够与外部科学计算生态（Python `blosc2` / Caterva / HDF5）无缝互通。

#### Functional Requirements:
1. `FR4.1`: 实现 Blosc2 Frame 头部解析与生成（Magic `b2fr`，格式版本，64 位压缩/未压缩尺寸，Chunk 尺寸与 Block 尺寸）。
2. `FR4.2`: 支持变长 Metalayer（键值对元数据）的添加、读取与自压缩嵌入。
3. `FR4.3`: 提供完整的 Frame 磁盘流式导出与双向解析器。

---

## Success Criteria

1. **区间切片加速比**：在 4MB 压缩 Chunk 中读取 4KB 前缀切片，耗时相比全量解压缩减 $\ge 90.0\%$，加速比 $\ge 10.0\times$。
2. **Bit-Grooming 压缩增益**：对浮点传感器数据在保证 3 位有效数字的前提下，结合 BitShuffle + Deflate 压缩比相比裸 Deflate 提升 $\ge 500.0\%$。
3. **插件注册开销**：动态插件调度相比硬编码静态分发，单次块处理调用延迟增量 $\le 5\text{ ns}$。
4. **全量系统回归与性能门禁**：全量测试套件 100% 通过，13 项性能门禁零倒退。
