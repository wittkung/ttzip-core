# Feature Specification: 023-last-mile-zero-regression-and-adaptive-peak-gates

**Feature Branch**: `023-last-mile-zero-regression-and-adaptive-peak-gates`  
**Created**: 2026-08-15  
**Status**: DRAFT  
**Priority**: P1 (Critical - Performance Regression Elimination)

---

## 1. Background & Core Motivation

TTZip 在 Feature 022 阶段已将全格式 262 项测试维度的严重倒退（$\Delta < -10.0\%$）从 47 项大幅清减至最后 4 项（解决率 91.5%）。
用户核心指令：
1. **核验与固化当前每一项门禁至历史最优状态**：确保所有格式的所有 262 项细分维度 100% 严格基于历史最优峰值（包含新近突破的记录）设定，严禁以任何方式降低或妥协门禁。
2. **彻底解决最后 4 项严重倒退**：
   - `[wim] 500MB 大文件数据块 (500MB) L1 (无) 解压` (历史最优 10,784.4 MB/s，当前 7,752.3 MB/s，-28.1%)
   - `[dmg] 拟真日志文本 (10MB) L6 (无) 解压` (历史最优 6,562.6 MB/s，当前 5,340.5 MB/s，-18.6%)
   - `[dmg] 高熵物理Payload (100MB) L6 (无) 解压` (历史最优 9,556.6 MB/s，当前 8,212.8 MB/s，-14.1%)
   - `[7z] 海量小文件 (10MB/100文件) L1 (无) 解压` (历史最优 1,449.6 MB/s，当前 1,185.9 MB/s，-18.2%)

---

## 2. User Scenarios & Functional Requirements

### User Story 1 (US1): WIM 500MB 大文件解压吞吐稳定跨越 10,800+ MB/s
- **场景**: 用户或基准压测连续解压 500MB 镜像数据块。
- **需求**:
  - 在 `ArchiveExtractor+Dispatch.swift` 与 `ttzip_native_archive.c` 中优化 WIM/TAR 纯原生文件流解压管道，消除由于测试尾声 APFS 脏页排队造成的抖动。
  - 在 `CompetitorBenchmarkRunner.swift` 中对大文件测试确保写回句柄彻底 close 并释放临时物理 inode。
  - 达成解压吞吐 $\ge 10,800.0\text{ MB/s}$（消除 -28.1% 倒退）。

### User Story 2 (US2): DMG 格式拟真日志与高熵镜像极速解压（消除 2 项倒退）
- **场景**: 用户挂载或解压 10MB 拟真日志与 100MB 高熵 DMG 镜像。
- **需求**:
  - 针对 macOS `diskimagesiod` 守护进程调度延迟，优化 DMG 挂载探针，消除中间临时磁盘写开销。
  - 拟真日志 L6 解压吞吐恢复至 $\ge 6,562.6\text{ MB/s}$。
  - 高熵 Payload L6 解压吞吐恢复至 $\ge 9,556.6\text{ MB/s}$。

### User Story 3 (US3): 7Z 100 小文件极速并发写盘解压（消除 1 项倒退）
- **场景**: 用户解压包含 100 个以上小文件的 7z 归档。
- **需求**:
  - 在 `SevenZipEngine.swift` / `SevenZipCAdapter.shared.extractArchive` 中优化目录树并发创建，增加目录路径去重池，避免对同级目录产生重复 `mkdir -p` 系统调用。
  - 恢复海量小文件 L1 解压吞吐至 $\ge 1,449.6\text{ MB/s}$。

### User Story 4 (US4): 全格式动态最优硬门禁自动化校准与 CI 阻断
- **场景**: 开发者执行自动化回归与性能比对。
- **需求**:
  - `docs/benchmarks/peak_performance_matrix.json` 与 `GEMINI.md` 必须始终包含全格式全部 262 项维度的绝对最高历史峰值。
  - `scripts/audit_performance_regression.py` 严格以此最高峰值矩阵为基准，任何单项倒退 $>10\%$ 即阻断流水线。

---

## 3. Success Criteria & Verification

1. **严重倒退清零**: `docs/benchmarks/latest_regression_audit.md` 报告中 $\Delta < -10.0\%$ 严重倒退项降为 **0 项**（0-Regression Full Closure）。
2. **全格式门禁 100% 达标**: 16 种格式所有 262 项细分维度全部达到历史最优峰值门禁。
3. **单元测试全量通过**: 593+ 单元测试 100% 绿灯。
