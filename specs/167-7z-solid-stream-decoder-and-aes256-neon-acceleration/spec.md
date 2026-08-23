# Feature Specification: 7z Solid 流式解压引擎与 ARM64 AES-256 / SHA-256 硬件密码加速 (Feature 167)

**Feature ID**: `167-7z-solid-stream-decoder-and-aes256-neon-acceleration`  
**Created**: 2026-08-21  
**Status**: In Progress (Specification Phase)  
**Priority**: P0 (Core Performance, Crypto Acceleration, User Experience)

---

## 1. Executive Summary

7z 格式在日常生产中是高压缩比与加密归档的绝对主力（如 LZMA2 固实压缩 Solid Archive 与 AES-256 密码保护）。
然而，传统解压实现存在两大核心瓶颈：
1. **Solid 固实块内存爆炸与全量解包开销**：当用户仅需提取或预览 Solid 归档中的单个文件时，传统方案必须将整个固实流（可能数 GB）全量解压到内存或临时磁盘；
2. **密码解密与 KDF 密钥派生计算密集**：7z 使用 2^19 ~ 2^24 次 SHA-256 轮次的 Key Derivation Function (KDF) 以及 AES-256-CBC 密码解密。若采用通用软件实现，单次密码验证耗时高达数百毫秒，解密吞吐被限制在 300~500 MB/s。

本特性的目标是：**深度打磨 TTZip 的原生 C11 7z 解码与密码子系统，实现 (1) Solid 固实块按需流式跳过与单条目内存零拷贝提取；(2) 深度挖掘 Apple Silicon ARM64 Crypto 指令集（`aese` / `aesd` / `aesmc` / `aesimc` 与 `sha256h` / `sha256su0/1`），将 AES-256-CBC 解密吞吐推升至 3+ GB/s，KDF 验证耗时缩短 80%+；(3) 每次交付均通过 `./scripts/benchmark_ab.sh` 5 轮交替采样验证无回归。**

---

## 2. User Scenarios

### User Scenario 1 (US1) - Solid 7z 归档单条目流式秒级提取 (Solid 7z On-Demand Streaming Extraction)
- **As a**: macOS 桌面端用户 / Finder 深度用户
- **I want to**: 在包含上千个文件的 10GB Solid 7z 归档中，按下空格键即时 Quick Look 预览或拖拽提取第 500 个小文件
- **So that**: 引擎仅流式解压前序依赖流至内存环形缓冲区，提取目标条目后立即停止，无需解压整个 10GB 归档，耗时从 30 秒缩短至 100 毫秒以内。

### User Scenario 2 (US2) - ARM64 硬件指令级 AES-256-CBC 极速解密 (Hardware Accelerated AES-256-CBC)
- **As a**: 处理大体积加密 7z 归档的企业开发者
- **I want to**: 解压受密码保护的 7z 归档
- **So that**: 引擎自动探测并启用 ARM64 Crypto Extension 向量管道，以超过 3 GB/s 的线速解密数据流，CPU 占用与发热降低 60%。

### User Scenario 3 (US3) - ARM64 硬件 SHA-256 高并发密钥派生 (Hardware SHA-256 KDF Engine)
- **As a**: 输入密码解密 7z 文件的用户
- **I want to**: 快速校验密码正确性并派生 256-bit AES 密钥
- **So that**: 密码弹窗在毫秒级内完成校验响应，绝无 UI 卡顿。

---

## 3. Functional Requirements

- **REQ-001 (Solid Stream Chunk Pipeline)**: `ttzip_7z_block_decoder.c` / `CTTZipBridge_7zSolid.c` 必须支持流式分块解码器，允许在解压 Solid 块时为非目标文件提供 `NULL` 或丢弃目标回调，以最高吞吐跳过无关字节。
- **REQ-002 (ARM64 Crypto Extension AES-256)**: `ttzip_7z_crypto_neon.c` 必须实现基于 ARM64 内联汇编 / ACLE 内建函数（`vaeseq_u8`, `vaesdq_u8`, `vaesmcq_u8`, `vaesimcq_u8`）的 14 轮 AES-256-CBC 加解密，单核吞吐 $\ge 2.5	ext{ GB/s}$。
- **REQ-003 (ARM64 Hardware SHA-256 KDF)**: `ttzip_7z_kdf_arm64.c` 必须实现基于 `vsha256hq_u32`, `vsha256h2q_u32`, `vsha256su0q_u32`, `vsha256su1q_u32` 的 7z 多轮 KDF 派生加速，支持 $2^{19} \sim 2^{24}$ 轮快速收敛。
- **REQ-004 (Cross-Platform / Pure C Fallback)**: 在非 ARM64 或不支持 Crypto Extension 的硬件上，自动无缝回退至纯 C 安全解密与标准 SHA-256，保证 100% 行为与数据一致性。
- **REQ-005 (C Unit Test & Statistical Verification)**: 编写 `Tests/c/test_7z_crypto_neon.c` 验证 AES-256-CBC 标准向量、7z 加密包解密，并运行 `./scripts/benchmark_ab.sh` 进行 5 轮交替采样验证。

---

## 4. Success Criteria

1. **AES-256 解密吞吐**: 在 Apple Silicon ARM64 上，连续内存解密吞吐 $\ge 2.5	ext{ GB/s}$（较软件解密提升 $\ge 400\%$）；
2. **KDF 密钥派生延迟**: $2^{19}$ 轮 7z SHA-256 密钥派生耗时在单个 P-Core 上 $\le 25	ext{ ms}$；
3. **正确性与零崩溃**: 26 套 C 单元测试 100% 通过（包含极端畸变密码与错误 IV 注入）；
4. **统计 A/B 门禁无回归**: `./scripts/benchmark_ab.sh HEAD~1 HEAD --runs 5` 保持 `PASSED_NO_REGRESSION`。
