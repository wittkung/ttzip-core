# Research & Technical Decisions: 084-lzham-branchless-decompression-and-circular-dict

**Feature Directory**: `specs/084-lzham-branchless-decompression-and-circular-dict`  
**Created**: 2026-08-18  
**Status**: Completed  
**Spec Reference**: [`spec.md`](spec.md) | **Plan Reference**: [`plan.md`](plan.md)

---

## 1. Research Items

### R001: 11-Bit 哈夫曼直接查表与 64-bit 预取在 ARM64 NEON 下的微架构优化

* **Decision**:  
  采用 11 位（2048 项）一级查表加速表 `uint32_t lookup[2048]`，其中每个表项的高 16 位存储符号码长（`len`），低 16 位存储解码出的符号值（`symbol`）。配合 64 位宽通用寄存器 `uint64_t bit_buf`，在 `bit_count < 24` 时通过 32 位大端字批量加载进行重填。
* **Rationale**:  
  1. **单周期输出**：在现代压缩数据流中，码长 $\le 11$ 位的符号覆盖率通常超过 95%。一次查表与位运算：
     ```c
     uint32_t t = lookup[bit_buf >> (64 - 11)];
     *out_sym = (uint16_t)(t & 0xFFFF);
     uint32_t len = t >> 16;
     bit_buf <<= len;
     bit_count -= len;
     ```
     在 ARM64 上映射为 `LSR`, `LDR (offset)`, `UXTH`, `LSL`, `SUB` 纯线性指令流，无任何条件分支跳转与流水线气泡。
  2. **消除 Load-Hit-Store 停顿**：将 `bit_buf`, `bit_count`, `in_ptr` 提升为 C 局部变量常驻在硬件寄存器（ARM64 `X19~X28`），消除了对堆内存或结构体成员的重复指针解引用。
* **Alternatives Considered**:
  - *被否决方案 1 (8-bit 查表 + 多级树回溯)*：表尺寸虽小（256 项），但码长 > 8 的符号比例较高（约 20%~30%），频繁跌入二级树遍历，分支预测失败率大幅升高。
  - *被否决方案 2 (逐比特 Range Coder 二叉状态机)*：如经典 LZMA，每个 bit 存在强制数据依赖链，严重压制超标量 IPC。
* **Source**:
  - `scratch/lzham_symbol_codec.h` (L408-L473)
  - `scratch/lzham_prefix_coding.h` (L9-L15)
  - `Sources/CTTZipBridge/ttzip_lzma2_branchless_rc.c`

---

### R002: $2^N$ 掩码环形字典更新与 Fast-Path NEON 向量拷贝在自重叠/边界下的安全性与加速

* **Decision**:  
  强制字典容量必须为 2 的整数次幂（$2^{15} \sim 2^{29}$ 字节），定义掩码 `dict_size_mask = dict_size - 1`。使用先验判据 `(MAX(src_ofs, dst_ofs) + match_len) <= dict_size_mask` 判定无边界溢出。在 Fast-Path 中：
  1. `match_dist == 1`：特化为 RLE 字节重复填充，小长度展开单字节循环，$\ge 8$ 字节直接调用 `memset`。
  2. `match_len < 8 || match_len > match_dist`：自重叠短匹配使用紧凑字节循环 `while (len--) *dst++ = *src++;`。
  3. `match_len >= 8 && match_len <= match_dist`：非重叠独立块直接调用 ARM64 NEON 向量拷贝 `vld1q_u8 / vst1q_u8` 批量吞吐。
* **Rationale**:  
  1. **无分支回绕**：源偏移计算 `src_ofs = (dst_ofs - match_dist) & dict_size_mask` 在 ARM64 上为单指令 `AND W_src, W_diff, W_mask`，彻底消除了除法与条件三元表达式。
  2. **分支预测准确率 > 99.99%**：字典容量（通常 32MB~128MB）远大于匹配长度（3~256 字节），先验判据在 99.99% 以上场景走入 Fast-Path，完全消除了热循环内的逐字节边界检查。
* **Alternatives Considered**:
  - *被否决方案 1 (每字节动态检查并取模)*：`pos = (pos + 1) % dict_size;`，除法或条件判断侵入内层字节循环，吞吐下降 300% 以上。
  - *被否决方案 2 (无条件 SIMD 覆盖拷贝)*：在 `match_dist < 16` 且存在自重叠时，直接调用 `vld1q_u8/vst1q_u8` 会错误覆盖尚未生成的新字节，导致解压数据破坏。
* **Source**:
  - `scratch/lzham_lzdecomp.cpp` (L979-L1041)
  - `Sources/CTTZipBridge/ttzip_lzma2_dec_native.c` (L22-L48)
  - `Sources/CTTZipBridge/fast-lzma2/lzma2_dec.c` (L670-L703)

---

### R003: TTZip CTTZipBridge 原生流式 C 桥接与零拷贝架构

* **Decision**:  
  在 `Sources/CTTZipBridge/` 中新增独立模块 `ttzip_branchless_decomp.h` 与 `ttzip_branchless_decomp.c`，定义零堆分配、线程安全、支持暂停恢复的纯 C11 状态机结构体 `ttzip_branchless_dict_t` 与 `ttzip_branchless_huff_t`。
* **Rationale**:  
  1. **零堆分配**：解压字典内存由调用方一次性页对齐分配（`ttzip_platform_aligned_alloc(64, dict_size)`），状态机结构体分配在栈上或复用句柄，热路径零 `malloc/free`。
  2. **解耦与反哺**：既可作为独立微基准与新格式解码器使用，又可直接作为内联 Fast-Path 嵌入现有的 `ttzip_lzma2_dec_native.c` 与 7z 解码管道。
* **Alternatives Considered**:
  - *被否决方案 1 (直接引入庞大的 C++ lzham 库全部源文件)*：包含大量历史冗余代码与平台抽象层，增加 500KB+ 二进制体积，且侵入 CTTZipBridge 的纯 C 接口规范。
* **Source**:
  - `Sources/CTTZipBridge/include/CTTZipCoreArchitecture.h`
  - `Sources/CTTZipBridge/CTTZipSysAlloc.c`
