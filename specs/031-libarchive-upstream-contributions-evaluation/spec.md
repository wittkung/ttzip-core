# Feature Specification: 031-libarchive-upstream-contributions-evaluation

**Feature Name**: TTZip 技术沉淀向 Upstream libarchive 贡献评估与演进规划 (031-libarchive-upstream-contributions-evaluation)  
**Status**: SPECIFIED  
**Priority**: P1 (Open Source Strategy & Core Infrastructure)  
**Target Modules**: `Vendor/libarchive-upstream/libarchive/`, `Sources/CTTZipBridge/`, `Sources/TTZipCore/`  

---

## 1. Background & Executive Summary

在开源贡献 PR #3388（[libarchive/libarchive#3388](https://github.com/libarchive/libarchive/pull/3388)）中，TTZip 团队成功为 `libarchive` 实现了 7-Zip AES-256 读取端解密与单缓冲区栈上装配 KDF 优化，解决了社区长达 8 年的待决需求（Issue #878）。

本特性的目标是**对 TTZip 现有的全部底层 C 桥接、汇编/SIMD 优化、文件系统加速与格式编解码工作进行全景式技术审查**，严格评估哪些成果适合作为独立的、高质量的 Pull Request 贡献回馈至 upstream `libarchive`，哪些成果应由其他上游项目（如 XZ Utils / LZMA SDK）承载，哪些成果应作为 TTZip 专有快速路径（Fast-Path）在内部长期保留。

---

## 2. Clarifications & Architectural Decisions

### Session 2026-08-15
- **Q1: 评估向 libarchive 贡献的判定准则是什么？**  
  **A1**: 
  1. **架构契合度 (Architectural Alignment)**：符合 libarchive 的单流/块流式抽象与跨平台 C99/POSIX 规范，不破坏 ABI/API。
  2. **跨平台与零倒退 (Cross-Platform & Zero Regression)**：硬件特化（如 ARM NEON、x86 SSE/AVX）必须具备纯 C 标量 fallback，并通过 autotools/CMake 宏保护。
  3. **生态价值与独立性 (Ecosystem Value & Modularity)**：每个 PR 必须职责单一，具备独立的回归测试覆盖，避免臃肿庞杂的单体补丁。
- **Q2: 为什么像 `libdeflate` 和 `ttzip_lzma_hc4_neon.c` 不建议直接提交给 libarchive？**  
  **A2**: 
  - `libdeflate` 设计为无状态全内存块编解码，无法提供可暂停的流式重入上下文（`z_stream`），强行在 libarchive 管道中引入会导致内存膨胀违背其流式初衷；
  - `libarchive` 的 7z/XZ 编解码完全外包委托给外部 `liblzma`，自身不维护压缩匹配查找器（Match Finder），因此 LZMA HC4 优化更适合提交至 `tukaani-project/xz`。
- **Q3: 贡献路线图的优先级梯队划分依据是什么？**  
  **A3**:
  - **Tier 1（最高优先级 / 极高合并概率）**：7z 写入端 AES-256 加密（PR #3388 的自然延续）、CRC32 硬件加速（解决 300 MB/s 历史瓶颈）、磁盘预分配 `F_PREALLOCATE`/`posix_fallocate`（显著改善解压 I/O 碎片）。
  - **Tier 2（高优先级 / 明确独立优化）**：ARM64 BCJ 指令向量化、`mmap` 顺序文件读取后端、Apple BSD LZFSE 压缩过滤器。
  - **Tier 3（内部保留 / 跨项目定向提交）**：libdeflate 旁路、LZMA HC4 NEON 匹配查找器、AAR 原生框架。

---

## 3. Candidate Contributions Matrix (候选贡献矩阵)

| 编号 | 贡献主题 (Candidate) | 目标文件 (Upstream Target) | 技术手段与原理 | 预期性能收益 | 上游适用性评估 | 推荐路线 (Action) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **C01** | **7z AES-256 写入端加密支持** | `archive_write_set_format_7zip.c`, `archive_cryptor.c` | 基于 PR #3388 KDF，补齐 Multi-Coder `BindPairs`、对称加密流水线与 `kEncodedHeader` | 补全 7z 写入加密核心能力 | **Tier 1 (极高)**：十年未竟功能闭环 | **提交 Upstream PR** |
| **C02** | **CRC32 硬件指令集加速** | `archive_crc32.h` | 引入 ARMv8 ACLE `__crc32*` 与 x86 SSE4.2/PCLMUL，保留查表 fallback | 吞吐从 300 MB/s 提升至 **10+ ~ 30+ GB/s** (10x~100x) | **Tier 1 (极高)**：零破坏、全格式收益 | **提交 Upstream PR** |
| **C03** | **POSIX / Darwin 磁盘空间预分配** | `archive_write_disk_posix.c`, `archive.h` | `fcntl(F_PREALLOCATE)` + `posix_fallocate()` + `ARCHIVE_EXTRACT_PREALLOCATE` | 消除写入扩容碎片，I/O 稳定性提升 20%~40% | **Tier 1 (极高)**：标准 POSIX/Darwin 增强 | **提交 Upstream PR** |
| **C04** | **ARM64 NEON BCJ 跳转指令向量化** | `archive_read_support_format_7zip.c` | 128-bit NEON 4 指令并行检测与无跳转块 16 字节跳步 | ARM64 指令转换吞吐提升 **4x ~ 8x** | **Tier 2 (高)**：独立算法优化 | **提交 Upstream PR** |
| **C05** | **`mmap` + `madvise` 顺序文件读取** | `archive_read_open_filename.c` | 兑现行 439 TODO，只读映射 + `POSIX_MADV_SEQUENTIAL` | 消除系统调用上下文切换，大归档打开提速 | **Tier 2 (高)**：兑现已有架构 TODO | **提交 Upstream PR** |
| **C06** | **Apple BSD LZFSE 压缩过滤器** | `archive_read_support_filter_lzfse.c`, `archive_write_add_filter_lzfse.c` | 挂载 Apple 官方 BSD 开源 `liblzfse` 编解码器 | 扩展支持 LZFSE 压缩流 | **Tier 2 (中高)**：类似 lz4/zstd 标准过滤器 | **提交 Upstream PR** |
| **C07** | **libdeflate 全缓冲多线程加速** | `Sources/CTTZipBridge/CTTZipExtract.c` | Thread-Local 无锁内存块解压池 + SIMD | 吞吐达到 6500~8800 MB/s | **Tier 3 (不建议)**：与流式架构冲突 | **TTZip 专有保留** |
| **C08** | **LZMA HC4 NEON 匹配查找器** | `Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c` | 16-byte NEON 向量匹配 + ACLE 硬件哈希 | 原生 LZMA 编码提速 2x~3x | **Tier 3 (建议转投 XZ)**：上游外包 liblzma | **保留或转投 XZ** |
| **C09** | **Apple Archive (AAR) 原生框架** | `Sources/TTZipCore/NativeAppleArchiveEngine.swift` | Swift 6 绑定 AppleArchive 框架 | macOS 14+ 原生生态适配 | **Tier 3 (不建议)**：专有生态依赖 | **TTZip 专有保留** |

---

## 4. User Scenarios & Acceptance Criteria

### User Scenario 1 (US1): 生成上游贡献技术决策白皮书与规划工件
- **场景**：工程团队评估并规划向 libarchive 的开源贡献演进路径。
- **行为**：生成完整的架构决策记录（ADR）、候选贡献技术规范、数据模型与验收验证指南。

### User Scenario 2 (US2): 输出 Tier 1 贡献（7z 写入加密、CRC32 硬件加速、磁盘预分配）设计原型与规范
- **场景**：为 Tier 1 三大核心贡献设计精确的 C 接口契约与兼容性测试用例。
- **行为**：生成标准 JSON Schema 契约文件与 quickstart 验证命令。

---

## 5. Success Criteria

1. **工件完整度**：完整产出 `spec.md`、`plan.md`、`research.md`、`data-model.md`、`contracts/`、`quickstart.md`。
2. **研究真实性**：所有技术评估均基于 upstream libarchive 与 TTZip 真实源码行号索引，禁止臆测。
3. **零通配契约约束**：`contracts/` 内全部 schema 声明 Draft-07 元规范，100% 消除裸 object 通配。
