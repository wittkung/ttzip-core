# Feature Specification: 026-zero-regression-final-mile

**Feature Branch**: `026-zero-regression-final-mile`  
**Created**: 2026-08-15  
**Status**: DRAFT  
**Priority**: P1 (Critical - Zero Regression Final Mile Closure)

---

## 1. Background & Core Motivation

在 Feature 025 中，TTZip 将 10% 以上的严重倒退从最初的 47 项收敛至**最后 6 项**（达标率 97.6%），并在 ZIP 500MB AES 解密（12.0 GB/s）和 TAR.ZST 500MB 打包（23.0 GB/s）上再次打破历史最高纪录。
用户核心指令：
1. **核验并确保每一项门禁均基于历史最优状态（计算上前面做出的全部优化）设定**，坚决不进行任何下调。
2. **彻底攻克最后 6 项严重倒退**：
   - `[7z] 100MB 高熵 L1 (AES) 解压` ($6,675.0\text{ MB/s} \rightarrow \ge 8,171.5\text{ MB/s}$) 与 `[7z] 100MB 高熵 L1 (无) 解压` ($6,264.1\text{ MB/s} \rightarrow \ge 7,334.1\text{ MB/s}$)。
   - `[7z] 100MB 高熵 L1 (无) 压缩` ($4,799.8\text{ MB/s} \rightarrow \ge 5,664.6\text{ MB/s}$)。
   - `[tar] 10MB 拟真日志 L6 (无) 解压` ($7,091.4\text{ MB/s} \rightarrow \ge 8,654.9\text{ MB/s}$) 与 `[tar.zst] 10MB 拟真日志 L1 (无) 解压` ($4,869.2\text{ MB/s} \rightarrow \ge 5,496.2\text{ MB/s}$)。
   - `[wim] 500MB 大文件 L1 (无) 解压` ($9,209.1\text{ MB/s} \rightarrow \ge 10,784.4\text{ MB/s}$)。

---

## 2. User Scenarios & Functional Requirements

### User Story 1 (US1): 7Z 100MB 高熵数据块 256KB Cache 对齐与 Direct 解压 (Priority: P1)
- **需求**:
  - 在 `CTTZipBridge_7zNativeDecoder.c` 中优化高熵数据块的解密和解压缓冲区，对齐至 256KB L2 Cache 页边界，消除分块切换流水线气泡。
  - 恢复 7Z 100MB 高熵解压吞吐至 $\ge 8,171.5\text{ MB/s}$。

### User Story 2 (US2): TAR / TAR.ZST 10MB 单文件 Direct I/O 与解压旁路 (Priority: P1)
- **需求**:
  - 在 `ttzip_tar_native.c` 中针对单文件归档解压实施 Fast-Path 旁路，避免针对单文件创建哈希表查找开销。
  - 恢复 TAR 10MB 日志解压吞吐至 $\ge 8,654.9\text{ MB/s}$。

### User Story 3 (US3): WIM 500MB 大文件 Direct I/O 顺序预读 (Priority: P1)
- **需求**:
  - 在 `ttzip_native_archive.c` 中为大文件归档解压注入 `posix_fadvise(..., POSIX_FADV_SEQUENTIAL)` 与 `fcntl(F_RDAHEAD)` 提示，锁定 $\ge 10,784.4\text{ MB/s}$。

---

## 3. Success Criteria & Verification

1. **严重倒退完全清零**: `docs/benchmarks/latest_regression_audit.md` 中 $\Delta < -10.0\%$ 严重倒退项数量为 0。
2. **全格式 262 项门禁 100% 达标**。
3. **单元测试全量通过**: 593+ 单元测试 100% 绿灯。
