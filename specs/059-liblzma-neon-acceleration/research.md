# Research & Technical Decisions: Liblzma (XZ Utils) ARM NEON Match Finder Acceleration & Upstream Baseline Integration

**Feature Branch**: `059-liblzma-neon-acceleration`
**Date**: 2026-08-17
**Status**: Completed

---

## Research Item R001: liblzma memcmplen NEON 向量化比对适配

### 1. Decision
在 `Vendor/xz-upstream/src/liblzma/common/memcmplen.h` 中为 ARM64 架构（`#if defined(__aarch64__) || defined(__ARM_NEON)`）引入 128-bit NEON 向量展开与 64-bit SWAR 混合比对逻辑，并将 `LZMA_MEMCMPLEN_EXTRA` 调整为 16：
- **小端序模式 (Little-Endian)**：使用 `vld1q_u8` 进行 16 字节对齐载入，`veorq_u8` 比较，`vgetq_lane_u64` 分别提取低 64 位与高 64 位，利用 `__builtin_ctzll` 单指令获取首个不匹配字节偏移。
- **TTZip 独立环境 (`ttzip_lzma_hc4_neon.c`)**：维持两级混合架构（Tier 0 64-bit GPR SWAR 极速初筛 + Tier 1 128-bit NEON 展开 + 8B/1B 阶梯安全回退），兼顾短匹配极速淘汰与长匹配高吞吐。

### 2. Rationale
1. **消除寄存器跨域开销**：在 LZ77 匹配查找中，超过 80% 的哈希冲突节点在第 1~4 字节即失配。Tier 0 GPR 校验完全在通用寄存器完成，避免了 Apple Silicon 上 10~12 周期的 Vector-to-GPR 跨域数据传递延迟。
2. **长匹配吞吐翻倍**：一旦命中长匹配（如 273 字节的字典匹配），128-bit NEON 展开每次迭代处理 16 字节，比 64-bit SWAR 减少 50% 的循环跳转与加载指令，微基准吞吐达到 >4.9 GB/s（较标量提升 +91.8%）。
3. **零内存越界物理免疫**：在 `liblzma` 内部，通过将 `LZMA_MEMCMPLEN_EXTRA` 设为 16，`lz_encoder.c` 自动在字典末尾分配并清零 16 字节安全缓冲区；在通用切片上，严格遵循 `len + 16 <= limit` 并级联 8 字节与逐字节单标量收敛，彻底杜绝越界读取。

### 3. Alternatives Considered
- **纯 NEON 128-bit 向量比对（无 GPR Tier 0 初筛）**：否决。在哈希链高频失配场景下，频繁的向量加载与车道提取引入跨域延迟气泡，导致小文件压缩整体吞吐下降 8%~12%。
- **NEON `vceqq_u8` + `vminvq_u8` 全向量规约**：否决。`vminvq_u8` 跨通道规约指令执行延迟高达 3~4 cycles，且失配时仍需额外指令提取具体字节索引。
- **ARMv8.2-A SVE / SVE2 谓词比对指令 (`nmatch`)**：否决。Apple Silicon (M1~M4) 仅支持 ARMv8-A/ARMv9-A 基础架构与 NEON 128 位向量扩展，不支持 SVE/SVE2。

### 4. Source
- `Vendor/xz-upstream/src/liblzma/common/memcmplen.h:52-107`
- `Vendor/xz-upstream/src/liblzma/lz/lz_encoder_mf.c:49-70, 264-287, 381-408`
- `Vendor/xz-upstream/src/liblzma/lz/lz_encoder.c:116, 380-388`
- `Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c:11-127`
- `Tests/TTZipTests/HybridMatchFinderMicroTests.swift:165-225`

---

## Research Item R002: ARMv8 ACLE 硬件 CRC32 哈希加速集成

### 1. Decision
在 `Vendor/xz-upstream/src/liblzma/lz/lz_encoder_hash.h` 与 `Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c` 中，采用基于 ARM C Language Extensions (`<arm_acle.h>`) 的 `__crc32w(0, v) & mask` 硬件指令直通哈希计算；在非 ARMv8 CRC 架构上透明回退至 Fibonacci 乘法散列与现有软件查表宏。

### 2. Rationale
1. **消除查表内存依赖与 L1 缓存竞争**：原生 `hash_4_calc()` 每次处理需 2 次读取 1 KiB 的 `hash_table` 内存查找表。使用 `__crc32w` 将 4 字节数据直接读入 32 位寄存器，单指令完成完整多项式雪崩混合，彻底消除了访存延迟。
2. **Apple Silicon 峰值流水线吞吐**：Apple M 系列核心上 `CRC32W` 指令执行延迟仅 1~2 周期，具备每周期 1 条指令的满吞吐能力。
3. **流规范完全独立性 (Bitstream Invariance)**：Match Finder 哈希仅用于在滑动字典窗口中快速索引重复候选串，属于编码器内部启发式加速，不影响 LZMA2 压缩字节流的标准格式；生成的归档可被全球所有标准解压器 100% 正常解压。

### 3. Alternatives Considered
- **完全沿用 liblzma 原生 `hash_table` 软件查表宏**：否决。查表产生串行 Load-to-Use 依赖，无法与算术单元并行发射，成为热路径瓶颈。
- **强制在压缩流中定义统一哈希算法**：否决。哈希算法属于实现细节，强制统一无任何兼容性收益，反而丧失硬件特化加速空间。
- **使用 NEON 向量寄存器计算哈希 (`veorq_u8` / `vtbl1_u8`)**：否决。单次哈希仅 4 字节，引入 GPR $\to$ Vector 寄存器跨域转移需 10~12 周期延迟，严重劣于 GPR 单指令 `__crc32w`。

### 4. Source
- `Vendor/xz-upstream/src/liblzma/lz/lz_encoder_hash.h:30-39, 54-76`
- `Vendor/xz-upstream/src/liblzma/lz/lz_encoder_hash_table.h:5-70`
- `Vendor/xz-upstream/src/liblzma/check/crc_common.h:80-115`
- `Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c:6-9, 134-144, 159-175`
- `Sources/CTTZipBridge/include/ttzip_lzma_hc4_neon.h:15-48`

---

## Research Item R003: Vendor/liblzma.a 编译系统与静态库重新打包

### 1. Decision
在 `Vendor/xz-upstream` 中使用 CMake Universal 2（`-DCMAKE_OSX_ARCHITECTURES="arm64;x86_64"`）编译输出优化后的静态库 `liblzma.a`，配置选项包含 `-DBUILD_SHARED_LIBS=OFF -DXZ_ARM64_CRC32=ON -DXZ_SMALL=OFF -DXZ_THREADS=yes`；随后通过 Apple 原生 `libtool -static` 将 `Vendor/lib/*.a` 统一合并为 `Vendor/libTTZipVendor.a`，并同步更新 `Vendor/TTZipVendor.xcframework`。

### 2. Rationale
1. **多架构安全与构建干净度**：CMake 原生支持 Universal 2 交叉编译与 ACLE 探测；构建 Flags 仅传递标准 `-O3 -fPIC -mmacosx-version-min=14.0`，利用源码级 `__attribute__((__target__("+crc")))` 避免全局 `-march` 导致 x86_64 切片编译报错。
2. **Mac App Store (MAS) 沙盒完全合规**：运行时硬件探测采用 Apple 官方公开的 `sysctlbyname("hw.optional.armv8_crc32")`，无私有 API，零 JIT 动态代码生成。
3. **SPM 进程内 100% 静态链接**：`Package.swift` 直接依赖 `TTZipVendor.xcframework`，Swift 编译器在链接阶段吸纳全部符号，实现零 CLI 进程开销的纯原生调用。

### 3. Alternatives Considered
- **Autotools 手工双架构编译与 lipo 合并**：否决。需维护两套 configure 流程与额外构建脚本，易受本地 Autoconf 版本差异影响。
- **SPM 直接源码管理 xz-upstream**：否决。`xz-upstream` 依赖复杂的预编译宏与 `config.h`，直接纳入 SPM Swift/C Target 会破坏上游纯净性与可维护性。

### 4. Source
- `Vendor/xz-upstream/CMakeLists.txt:493, 614-644, 1372-1395`
- `Vendor/xz-upstream/src/liblzma/check/crc32_arm64.h:40-44, 131-138`
- `scripts/build_libdeflate.sh:31-42, 64-69`
- `scripts/build_zlib_ng.sh:133-141`
- `Package.swift:28-49`
