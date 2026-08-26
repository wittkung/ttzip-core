# Feature Specification: Decisive Dominance and Zero Regression (009)

**Feature Directory**: `specs/009-decisive-dominance-and-zero-regression/`  
**Status**: DRAFT  
**Author**: Antigravity CTO & Performance Architect  
**Created**: 2026-08-15  

---

## 1. Executive Summary & Goals

用户要求：
1. **彻底消除险胜，实现明显领先**：在所有归档格式（7Z、TAR.ZST、ZIP、TAR.GZ）全维度 92 项对决中，消除任何打平或仅微弱领先（< 1.05x）的场景，确保对竞品官方 CLI（7-Zip 7zz, Meta zstd -T0, Apple zip/ditto, bsdtar）取得 **>= 1.05x ~ 30x+ 的显著领先与绝对统治**。
2. **零性能回落铁律（Zero Regression Floor）**：相较于历史最佳基准，**绝对禁止出现任何 > 10% 的性能倒退**。所有核心场景必须保持或超越历史峰值。

---

## 2. User Scenarios & Acceptance Criteria

### User Story 1 (US1): 7Z 500MB 大文件与全部 32 项对决显著超越 (Decisive 7Z Victory)
- **As a** 用户处理大体积 7z 归档，
- **I want** 500MB 大文件与海量小文件压缩解压速度全面大幅超越 7-Zip 官方 7zz CLI，
- **So that** 无论无加密还是 AES-256 加密，无论 Level 1 还是 Level 6，TTZip 均保持明显领先（>= 1.05x），零打平，零落后。

### User Story 2 (US2): TAR.ZST 管道重构实现对决全面反超 (TAR.ZST Dominance)
- **As a** 用户处理 tar.zst 归档，
- **I want** 解压端与高熵不可压缩载荷压缩端全面超越 `zstd -T0` CLI，
- **So that** 攻克剩余的 6 个落后项，达成 92/92 全格式 100% 满分统治。

### User Story 3 (US3): 全场景零性能倒退断言 (Strict Zero-Regression Assertion)
- **As a** 架构师与系统用户，
- **I want** 每次优化提交均经过全量 92 项回归审计，
- **So that** 没有任何单项相对历史最优基准发生超过 10% 的性能回落。

---

## 3. Functional Requirements (FR)

- **FR-001 (7Z 500MB L1 无加密极速压缩)**:
  - 针对 500MB 快速模式，消除任何多余的系统调用与分块切换开销，采用无锁多核直接编码流水线，将吞吐稳定提升至 **>= 5,500 MB/s**（显著超越 7zz 的 5,150 MB/s）。
- **FR-002 (7Z AES-256 流水线持续领先)**:
  - 保持 POSIX 原生线程 KDF 与块编码重叠流水线，确保 500MB AES 压缩保持在 **>= 5,200 MB/s**。
- **FR-003 (TAR.ZST Direct In-Process 解码器)**:
  - 在解压端实现原生 Direct 多线程 ZSTD 流式解码器（`ZSTD_decompressStream` + 批量 tar entry 展开），彻底消除 libarchive 单线程 filter 瓶颈，解压吞吐从 3,600 MB/s 突破至 **>= 7,000 MB/s**。
- **FR-004 (高熵不可压缩数据极速旁路)**:
  - 对不可压缩的高熵载荷（香农熵 > 7.85），在 ZSTD 编码前直接启用 `ZSTD_c_strategy = ZSTD_fast` 与 negative compression bypass，将吞吐提升至 **>= 6,000 MB/s**。
- **FR-005 (测试生命周期零磁盘污染与瞬态降噪)**:
  - 在竞品测试框架中加入更彻底的临时文件清理与 APFS 脏页同步隔离，确保测试指标反映纯粹硬件极限算力。

---

## 4. Success Criteria (SC)

- **SC-001 (7Z 格式 32/32 全胜)**: 7Z 格式全量 32 项对决对 7-Zip 7zz **32 战 32 胜，胜率 100%**，无任何落后项。
- **SC-002 (TAR.ZST 全面反超)**: TAR.ZST 格式解压端与高熵端反超 `zstd -T0`，全格式 92 项总胜率达到 **95%+ ~ 100%**。
- **SC-003 (零倒退断言)**: 全格式 92 项对决比对历史最优基准，**回落 > 10% 项数为 0**。
- **SC-004 (门禁 100% 通过)**: 11 大性能门禁与 560+ 单测 100% 绿灯通过。
