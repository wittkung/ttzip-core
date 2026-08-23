# Technical Plan: 全格式深度攻坚与全场景全面霸榜技术方案

## 一、 架构设计与技术路径

### 1. 7Z AES-256 硬件线速加解密内核 (`ttzip_7z_crypto_neon.c`)
- **文件**：`Sources/CTTZipBridge/ttzip_7z_crypto_neon.c`, `Sources/CTTZipBridge/include/ttzip_7z_crypto_neon.h`
- **方案**：
  - 接入 ARMv8-A Crypto Extensions 指令：`vaeseq_u8` + `vaesmcq_u8`（AES 加密轮）、`vaesdq_u8` + `vaesimcq_u8`（AES 解密轮）。
  - 使用 `vsha256h_u32` / `vsha256su0_u32` / `vsha256su1_u32` 向量化硬件计算 7z 密码哈希派生。
  - 在 `CTTZipBridge_7zNativeDecoder.c` 与 `SevenZipEngine.swift` 中挂接原生 C/NEON 解密通道，消除对外部 CLI 调用的依赖。

### 2. 7Z ARM64 寄存器常驻与无分支 Range Coder 解码器 (`ttzip_lzma2_branchless_rc.c`)
- **文件**：`Sources/CTTZipBridge/ttzip_lzma2_branchless_rc.c`
- **方案**：
  - 基于 DCC 2024 Branchless Range Coding 理论，使用 ARM64 `csel` / `cset` 重构概率区间更新：
    $$\text{mask} = -(\text{code} < \text{bound})$$
    $$\text{range} = (\text{bound} \ \& \ \text{mask}) \mid ((\text{range} - \text{bound}) \ \& \sim\text{mask})$$
  - 将 `Range`、`Code`、`probs`、`Rep0~Rep3` 历史距离全部持久化至局部寄存器，消除访存开销。
  - 引入 64B Cacheline 软件预取（`__builtin_prefetch`）。

### 3. 7Z Fast LZMA2 (FL2) Radix 匹配查找器与 Level 1 快速跳表 (`ttzip_lzma_radix_mf.c`)
- **文件**：`Sources/CTTZipBridge/ttzip_lzma_radix_mf.c`, `Sources/CTTZipBridge/include/ttzip_lzma_radix_mf.h`
- **方案**：
  - 构建 2/3 字节前缀基数表（Radix Table），替代传统的二叉树（BT4）指针追逐。
  - 在内存中组织紧凑的连续偏移数组，消除大字典下的 TLB Miss，使 Level 1 快速匹配达到 1,000+ MB/s 吞吐。

### 4. TAR.ZST / TAR.GZ 多核独立分块滑动窗口与流式流水线
- **文件**：`Sources/CTTZipBridge/CTTZipBridge_Zstd.c`, `Sources/CTTZipBridge/CTTZipBridge_GzParallel.c`, `Sources/CTTZipBridge/ttzip_tar_native.c`
- **方案**：
  - `TAR.ZST`：启用 `ZSTD_CCtx_setParameter(cctx, ZSTD_c_nbWorkers, ncores)` 与 `ZSTD_c_jobSize`（1MB~4MB 块），打通多核全速流式压缩。
  - `TAR.GZ`：参考 `pigz` 架构，将输入流按 256KB 分块分配给各 CPU 核心，使用线程局部 `libdeflate_compressor` 并发压缩并流式拼接 Header/Footer。
  - `TAR`：引入 NEON 向量化 TAR Header 512 字节块校验与字段解析。

### 5. 全格式复制 ZIP 架构经验与 APFS 小文件 I/O 聚合
- $\le 64\text{ KB}$ 小文件统一使用栈缓冲 `uint8_t local_stack_buf[65536]`。
- 全链路 64B 内存对齐 `posix_memalign`。
- 引入内存文件镜像聚合缓冲池（In-Memory Coalescing Buffer），`pwritev` 批量写盘消除 APFS Journaling Inode 串行创建锁。
- 256 深度文件描述符信号量控制（`dispatch_semaphore_create(256)`）。
- 内存级索引表与 $O(1)$ 随机访问 SeekTable。

### 6. 7Z AES-256 原生 In-Process C 并发解码架构
- 在 `CTTZipBridge_7zNativeDecoder.c` 中识别 7Z `kFolder` 编码器链的 AES-256 编解码器（Method ID `0x06F10701`）。
- 提取 `num_cycles_power`、`salt` 与 `iv`，通过 `ttzip_7z_kdf_sha256_neon` 生成密钥，调用 `ttzip_7z_aes256_cbc_decrypt_neon` 执行 512KB 分块并发解密。
- 在 `CTTZipBridge_7z.c` 中彻底剔除外部 fallback 降级。

### 7. AIS 汇编优化基础设施与全局派发表
- 依据 `docs/ASSEMBLY_INFRASTRUCTURE_ARCHITECTURE.md` 在 `Sources/CTTZipBridge/dispatch/` 接入 `g_ttzip_dispatch` 只读派发表。
- 挂载 4-Way PMULL CRC32 与无分支 Range Coder 汇编微内核。

---

## 二、 验证与防退步守则

1. **初始基准对比**：每次优化后运行 `AllFormatsPkSuiteTests`，对比 `universal_pre_optimization_baseline.json`。
2. **零退步红线**：若任何格式指标出现倒退，阻断提交并立查根因。
3. **单元测试回归**：运行 `swift test` 确保 559+ tests 全部通过。
4. **竞品全面碾压判定**：`docs/competitor_benchmark_report.json` 中 100% 场景 $\text{Speedup} \ge 1.0\text{x}$。
