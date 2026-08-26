# Feature Specification: 022-full-matrix-zero-regression-and-throughput-closure

**Feature Name**: 全格式矩阵 28 项倒退攻坚与吞吐闭环 (022-full-matrix-zero-regression-and-throughput-closure)  
**Status**: DRAFT  
**Priority**: P1 (最高)  
**Target Release**: Release & CI/CD Pipeline Gate  

---

## 1. Background & Executive Summary

在 Feature 021 中，我们成功实现了 APFS 零拷贝与基准测试的严格解耦，消灭了 19 项性能倒退，并达成了 11 项 Release 性能硬门禁中的 10 项。
目前全格式 46 项基准测试（共 262 项细分维度）中，相比历史最优基准 `604d44d`，仍剩余 **28 项严重性能倒退（$< -10.0\%$）** 与 **1 项硬门禁指标差量（TAR.ZST Direct 50MB 达标 90.6%）**。

本 Feature 旨在从底层 C 桥接层 I/O 缓冲区对齐、异步批量 `pwrite`、Apple Silicon L2 Cache 行友好分块、TAR 变体 Direct 流式解析器以及 DMG 管道零拷贝写入等方面进行深度优化，彻底将 28 项倒退清零，实现全矩阵 100% 零性能倒退。

---

## 2. User Stories

### User Story 1: ZIP 大文件与高熵物理写盘解压性能恢复 (Priority: P1)
- **As a** 用户与性能测试工程师
- **I want** ZIP 500MB 大文件与 100MB 高熵 Payload 在纯物理写盘解压时达到最高吞吐（500MB 解压 $\ge 9,000\text{ MB/s}$，高熵解压恢复至历史最优水平）
- **So that** 在不依赖 APFS 零拷贝假象的前提下，获得真实最高解压效率。

#### Acceptance Scenarios
1. `[zip] 500MB 大文件数据块 (500MB) L1/L6 (无/AES) 解压` 吞吐从 `6,180 ~ 6,531 MB/s` 回升至 $\ge 9,500\text{ MB/s}$。
2. `[zip] 高熵物理Payload (100MB) L1/L6 (无/AES) 解压` 采用批量预分配写入，消除单块 I/O 阻塞。
3. `[zip] 拟真日志文本 (10MB) L1/L6 解压` 恢复至 $\ge 6,000\text{ MB/s}$。

### User Story 2: 7Z 100MB 高熵 Payload 解压吞吐恢复 (Priority: P1)
- **As a** 用户与归档处理引擎
- **I want** 7Z 高熵与大块数据解压充分利用 Apple Silicon L2 缓存
- **So that** 7Z 解压吞吐全面恢复至历史最优峰值（$\ge 7,500\text{ MB/s}$），消灭 4 项严重倒退。

#### Acceptance Scenarios
1. `[7z] 高熵物理Payload (100MB) L1/L6 (无/AES) 解压` 吞吐从 `5,774 ~ 6,528 MB/s` 回升至 $\ge 7,500\text{ MB/s}$。
2. 7Z AES-256 加密解压快速路径保持零锁与 NEON SIMD 并行加速。

### User Story 3: DMG 格式管道流式写入与解压零冗余拷贝 (Priority: P1)
- **As a** macOS 用户
- **I want** DMG 格式打包与解压直通管道，避免中间磁盘临时文件产生
- **So that** DMG 拟真日志与海量小文件的压缩解压吞吐提升 30% 以上，消除 4 项严重倒退。

#### Acceptance Scenarios
1. `[dmg] 拟真日志文本 (10MB) L1 压缩与解压` 吞吐分别回升至 $\ge 2,800\text{ MB/s}$ 与 $\ge 7,000\text{ MB/s}$。
2. `[dmg] 拟真日志文本 (10MB) L1 (AES)` 压缩与解压吞吐分别回升至 $\ge 2,400\text{ MB/s}$ 与 $\ge 5,200\text{ MB/s}$。

### User Story 4: TAR / TAR.ZST / TAR.GZ / LZ4 Direct 流式解析与打包收敛 (Priority: P1)
- **As a** 系统级开发者
- **I want** Tar 变体引擎与 TAR.ZST 50MB Direct 打包达到 $\ge 19,000\text{ MB/s}$ 硬门禁底线
- **So that** TAR / TAR.ZST / TAR.GZ / LZ4 的 12 项倒退彻底清零，全部 11 项 Release 门禁 100% 绿灯。

#### Acceptance Scenarios
1. `TAR.ZST Direct 50MB` 打包吞吐从 `17,223 MB/s` 突破 $\ge 19,000\text{ MB/s}$。
2. `[tar] 拟真日志文本 (10MB) L1/L6 解压` 吞吐回升至 $\ge 7,500\text{ MB/s}$。
3. `[tar.zst]` 与 `[tar.gz]` 拟真日志与高熵压缩解压倒退全部收敛在 $\le 3.0\%$ 以内。

---

## 3. Functional Requirements

- **FR-001**: 优化 `Sources/CTTZipBridge/CTTZipExtract.c`，在大文件解压与多文件解压时使用预热目标文件缓冲区与批量 `pwrite`，恢复 ZIP 解压吞吐。
- **FR-002**: 优化 7Z 解压引擎环形缓冲区为 256KB 对齐，消灭 7Z 高熵解压 4 项倒退。
- **FR-003**: 优化 DMG 架构消除中间磁盘临时文件，直通内存与管道。
- **FR-004**: 调优 `ttzip_tar_zstd_direct.c`，使 TAR.ZST 50MB 打包稳定跨越 19,000 MB/s 门禁。
- **FR-005**: 运行 `python3 scripts/audit_performance_regression.py` 验证全矩阵 28 项 $>10\%$ 倒退完全清零。

---

## 4. Success Criteria

- **SC-001**: 11 项 Release 性能硬门禁 100% 绿灯通过（包括 TAR.ZST Direct 50MB $\ge 19,000\text{ MB/s}$）。
- **SC-002**: 全格式 46 项基准测试（262 维）相比 `604d44d` 的严重倒退项（$>10.0\%$）数量为 **0**。
- **SC-003**: 全量 593+ 单元测试 0 失败，0 泄漏。
- **SC-004**: 代码保持零裸 `print`/`printf`，严格遵从 `TTLogger` 与零成本抽象铁律。
