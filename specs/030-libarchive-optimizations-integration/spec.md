# Feature Specification: 030-libarchive-optimizations-integration

**Feature Name**: 将 libarchive 官方 PR (PR #3388) 优化成果与 7z AES-256 解密引擎集成至 TTZip (030-libarchive-optimizations-integration)  
**Status**: SPECIFIED  
**Priority**: P1 (Core Infrastructure & Cryptographic Parity)  
**Target Module**: `Vendor/lib/libarchive.a`, `Sources/CTTZipBridge/`, `Sources/TTZipCore/`  

---

## 1. Background & Executive Summary

在开源贡献工作（[libarchive/libarchive#3388](https://github.com/libarchive/libarchive/pull/3388)）中，我们成功实现了 7-Zip AES-256 加密归档解密驱动（Codec `0x06F10701`），填补了 `libarchive` 自 2017 年以来长达 8 年的功能空白。在此过程中，我们实现了多项关键工程与算法优化：

1. **单缓冲区栈上装配与原地 64 位计数器更新 (Single-Buffer Stack Assembly & In-Place Counter KDF)**：
   - 传统 7z KDF 每次迭代需 3 次 `SHA256_Update` 调用（$3 \times 2^{19} = 1,572,864$ 次调用）。
   - 我们构建了 `kdf_buf[536]` 栈上连续内存布局 `[Salt | Password | Counter]`，单轮循环仅更新尾部 8 字节计数器，调用次数减少 66.7%，执行耗时从 7.87 ms 压缩至 6.05 ms，且达成 **零堆内存动态分配 (Zero Heap Allocation)**。
2. **多 Folder 密钥缓存机制 (KDF Key Caching)**：
   - 针对非固实（Non-Solid）多 Folder 归档，复用相同 Salt 与 Iteration Power 的派生密钥，消除冗余的 524,288 次哈希计算。
3. **全头加密 (`kEncodedHeader`) 递归中央目录解码**：
   - 支持 7z 核心元数据加密（Header Encryption `0x17`）的透传解密与内存流递归解析。
4. **流式 AES-256-CBC 16 字节对齐解码与零双重缓冲 (Streaming CBC Zero Double-Buffering)**。

### 本特性核心目标
将上述优化成果完整引入 TTZip 代码库：
1. **替换 Vendor 静态库**：以 `Vendor/libarchive-upstream` 编译出的最新静态库替换 `Vendor/lib/libarchive.a`，使 TTZip 的多格式检查 (`ttzip_inspect_archive_v2`) 与通用提取 (`ttzip_extract_archive_advanced`) 原生具备 7z AES-256（含数据流与全头加密）解密能力。
2. **重构 TTZip 原生 KDF 实现 (`ttzip_7z_kdf_arm64.c`)**：消除 `malloc`/`free` 动态内存分配与全局加锁瓶颈，移植栈上装配与原地计数器更新机制。
3. **端到端加密回归与性能守卫**：增加 7z 加密解密双引擎（Native Parallel 与 Libarchive Fallback）全场景测试用例，执行全格式 46 场景基准测试，确保全矩阵零性能倒退（Zero Regression）。

---

## 2. Clarifications & Architectural Decisions

### Session 2026-08-15
- **Q1: `Vendor/lib/libarchive.a` 替换后是否会对现有的 TAR/ISO/CAB 格式解压或归档产生影响？**  
  **A1**: 不会。PR #3388 中所有的 7z 解密逻辑均采用零侵入设计（Zero Codec Mutation），仅在 `archive_read_support_format_7zip.c` 内部以及 `archive_cryptor.c` 的私有接口中新增 7z Codec 支持，未修改任何 public header (`archive.h`, `archive_entry.h`)，且 300+ libarchive 既有测试与 TTZip 全量测试 100% 保持兼容。
- **Q2: `ttzip_7z_kdf_arm64.c` 原生 KDF 优化策略是什么？**  
  **A2**: 采用与 libarchive PR 相同的栈缓冲区 `kdf_buf[536]`（512 字节 UTF-16LE 密码 + 16 字节 Salt + 8 字节计数器），在栈上完成 UTF-8 到 UTF-16LE 转换与原地递增，彻底移除 `malloc(full_entry_len)` 与 `malloc(utf16_len)`，使单次 KDF 达到 0 堆分配与 $O(1)$ 内存空间。
- **Q3: 为什么需要为 TTZip 同时保留 Native C 7z 引擎与 Libarchive 7z 引擎？**  
  **A3**: Native C 引擎具备 Apple Silicon NEON 与多线程并行解压能力，是极致吞吐的第一选择；Libarchive 7z 引擎具备完整的流式解析器与容错能力，作为第二道安全防线。两者均支持 7z AES-256 解密后，形成了高吞吐与高容错的双引擎闭环。

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1 (US1): 加密 7z 归档（含全头加密）原生穿透浏览
- **场景**：用户使用 TTZip（UI 或 CLI）打开一个受密码保护的 7z 压缩包（无论是仅数据加密，还是 `kEncodedHeader` 全头加密）。
- **行为**：TTZip 提示输入密码后，`ttzip_inspect_archive_v2` 调用升级后的 `libarchive` 核心，正确解密并列出所有文件节点，标记加密标志 `is_data_encrypted` 与 `is_meta_encrypted`。

### User Scenario 2 (US2): 双引擎原生解压与透明降级
- **场景**：用户解压加密 7z 归档。
- **行为**：TTZip 首先通过原生快速路径 `ttzip_7z_extract_native_parallel_c` 执行多线程解密与解压；若遇到特异编码或非标准 block，无缝降级至 `ttzip_extract_7z_libarchive_c`，解压成功且解出的文件内容与校验码 100% 正确。

### User Scenario 3 (US3): 原生 KDF 零堆分配与极速密钥派生
- **场景**：高频触发 7z 密钥派生或处理包含大量独立加密 Folder 的归档。
- **行为**：`ttzip_7z_kdf_sha256_armv8` 使用栈上固定内存 `kdf_buf` 与原地计数器更新，单次派生耗时 $\le 15\text{ ms}$（Release 下 $\le 7\text{ ms}$），单次派生堆内存分配为 0 字节，操作结束敏感内存自动洗消（`memset` 清零）。

---

## 3. Functional Requirements & Technical Boundaries

- **FR-01**: `Vendor/lib/libarchive.a` 必须包含 `_decrypto_aes_cbc_init`、`_decrypto_aes_cbc_update`、`_decrypto_aes_cbc_release` 与 `_kdf_7z_sha256` 符号，支持 7z Codec `0x06F10701`。
- **FR-02**: `ttzip_7z_kdf_arm64.c` 中的 `ttzip_7z_kdf_sha256_armv8` 必须重构为栈上缓冲区 `kdf_buf[536]` 模式，严禁调用 `malloc`/`free`。
- **FR-03**: `ttzip_7z_kdf_arm64.c` 必须保留 Apple Silicon 硬件加速指令集（`vsha256hq_u32` 等）与 CommonCrypto 双重可用路径，保证最高吞吐。
- **FR-04**: 所有敏感密码与派生密钥必须在作用域结束时执行显式内存洗消（Scrubbing）。
- **FR-05**: 严禁修改冻结文件（如 `ZipParallelExtractor.swift` 等，详见 GEMINI.md）。
- **FR-06**: `swift test` 全量测试通过，且 `XCTestPerformanceMeasureTests` 吞吐门禁 100% 达标。

---

## 4. Success Criteria

1. **测试指标**：`swift test` 全量测试通过，新增 `Libarchive7zEncryptionTests` 测试集 100% 绿灯。
2. **内存指标**：KDF 密钥派生阶段热路径堆分配降为 0。
3. **性能指标**：全格式 46 场景基准测试无任何倒退（$\Delta \ge -3.0\%$）。
