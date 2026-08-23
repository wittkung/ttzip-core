# Feature Specification: 7Z Grand Slam Supremacy (32/32 All Conquest)

**Feature Name**: 7Z Grand Slam Supremacy  
**Feature Directory**: `specs/008-7z-grand-slam`  
**Status**: DRAFT  
**Target Milestone**: TTZip Core v2.0 - 7Z 100% Full Conquest  

---

## 1. Executive Summary & First-Principles Motivation

TTZip 在面向 macOS 14+ (Sonoma) Apple Silicon 架构的竞品 1v1 极限压测中，已在 ZIP (32/32 全胜)、TAR.GZ (16/16 全胜) 实现 100% 统治，在 7Z 格式的 32 项严苛对决中已取得 30~31 胜（胜率 96.9%）。

本特性针对最后 1 项尚未彻底超越 7-Zip 官方 `7zz` CLI 的极限瓶颈：
- **`500MB 大文件数据块 Level 1 (无加密) 压缩`**（当前 ~5,419 MB/s vs 7zz ~5,616 MB/s，差距仅 197 MB/s）。

同时，彻底固化已反超的 `500MB Level 1 (AES-256) 压缩`（5,518 MB/s vs 5,401 MB/s），并消除 10MB 文本与高熵流的分块碎片化波动，达成 **7Z 格式 32 战 32 胜（100% 满分全胜）大满贯统治**。

---

## 2. User Scenarios & User Stories

### User Story 1 (US1) - 500MB 大文件 Level 1 无加密压缩突破 6,000+ MB/s (Priority: P1) 🎯 MVP
- **作为** 专业工作站用户，
- **我希望** 在压缩 500MB 级别的大型二进制/全零/连续数据块为 7Z 格式时，
- **TTZip 能够** 在 16 逻辑核心满血微架构调度下，单流水线极速吞吐突破至 $\ge 6,000\text{ MB/s}$，全面超越 7-Zip 官方 `7zz` CLI。

### User Story 2 (US2) - 500MB 大文件 Level 1 AES-256 加密压缩稳胜 7zz (Priority: P2)
- **作为** 安全数据工程师，
- **我希望** 压缩 500MB 敏感数据至 7Z (AES-256) 格式时，
- **TTZip 能够** 保持 ARMv8 NEON 硬件加速与零拷贝加密流水线，维持 $\ge 5,600\text{ MB/s}$ 稳胜 7zz。

### User Story 3 (US3) - 消除中等体积流与高熵流波动，达成 32/32 全胜统治与零倒退 (Priority: P3)
- **作为** 系统质量保证负责人，
- **我希望** 消除 10MB 文本日志与高熵数据在并发压测下的分块碎片化波动，
- **TTZip 能够** 在全量 32 项 7Z 对决场景中实现 32/32 胜局（100% 胜率），且 11 大性能门禁 100% 绿灯。

---

## 3. Functional Requirements

- **FR-001**: `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` 必须针对大体积流（$\ge 64\text{MB}$）优化 GCD 并发分块调度，动态根据 L2 Cache 与核心数调优分块，消除多余内核态拷贝。
- **FR-002**: `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c` 必须对 Level 1 下的连续流采用专用极速字典状态机（`dict_size = 4096`, `nice_len = 273`, `depth = 1`, `LZMA_MF_HC3`）。
- **FR-003**: 压缩后元数据写盘必须保持单系统调用原子写入（`ttzip_7z_write_all` / 零碎片缓冲）。
- **FR-004**: 中等体积流（1MB~32MB）分块粒度必须锁定在 $\ge 1\text{MB}$ 黄金窗口，严禁生成小于 512KB 的过度碎片微块。
- **FR-005**: 任何变更必须严格执行 `audit_performance_regression.py`，保证零非预期倒退。

---

## 4. Success Criteria & Hard Performance Floor

- **SC-001**: 500MB Level 1 无加密 7Z 压缩吞吐实测达到 $\ge 5,800\text{ MB/s}$（战胜 7-Zip 7zz）。
- **SC-002**: 500MB Level 1 AES-256 7Z 压缩吞吐实测达到 $\ge 5,600\text{ MB/s}$（战胜 7-Zip 7zz）。
- **SC-003**: 7Z 格式竞品 1v1 对决达成 **32 战 32 胜（100% 全胜统治）**。
- **SC-004**: `./scripts/run_all_tests.sh` 560+ 单元测试 100% 绿灯通过，11 大硬性能门禁 100% 达标。
