# Implementation Plan: Zero Performance Regression Governance & Hard Floor Invariant Enforcement (Feature 017)

**Feature**: Zero Performance Regression Governance & Hard Floor Invariant Enforcement  
**Directory**: `specs/017-zero-performance-regression-and-floor-enforcement/`  
**Status**: Ready for Tasks

---

## 1. Technical Context & Overview

在 Feature 016 大满贯攻坚后，全格式竞品胜率达到 94.01%。但在多项参数调整与算法升级过程中，最新性能审计（`docs/benchmarks/latest_regression_audit.md`）暴露了部分场景的性能倒退（如 Lzip 参数调整失误引入的退化、WIM 海量小文件解包目录开销、DMG/ISO 递归解包退化等）。

本 Feature 建立严格的零倒退治理闭环：
1. 彻底修复全部 38 项倒退告警，将全量场景倒退控制在测量噪声以内，根除所有 $> 10\%$ 的倒退。
2. 升级 `scripts/audit_performance_regression.py` 实现双级门禁（3% 警告，10% 阻断退出非零状态码）。
3. 保持 11/11 Release 性能门禁与 591+ 全量单测 100% 绿灯。

---

## 2. Constitution & Project Rules Check

| 准则 | 评估 | 说明 |
| :--- | :--- | :--- |
| **热路径零成本抽象** | ✅ 合规 | 目录末次父目录缓存采用栈上 4096 字节固定缓冲区，零堆分配、零动态对象 |
| **Fast-Path 旁路保留** | ✅ 合规 | 保留并增强 TAR Direct、TAR.ZST、Brotli RAM 直通路径 |
| **吞吐硬门禁底线** | ✅ 合规 | 11 项 Release 门禁全部达标 |
| **设计模式热路径隔离** | ✅ 合规 | 享元与策略仅在调度层与冷路径使用，数据平面无锁裸指针直通 |
| **渠道条件编译** | ✅ 合规 | 严格区分 MAS 沙盒与 Direct 分发 |
| **零性能倒退铁律** | ✅ 合规 | 核心场景倒退 $< 3.0\%$，严重倒退（$> 10.0\%$）数量为 0 |
| **严格日志纪律** | ✅ 合规 | 0 裸 `print` / `printf`，全量统一经由 `TTLogger` |

---

## 3. Phase 0: Research & Grounded Technical Decisions

- R001 [SUBAGENT:research] 《WIM 与镜像解压目录递归开销与内存元数据优化》：采用栈上末次父目录缓存 + 乐观 `open` 失败回退机制，消除 99.9% 的冗余 `mkdir` 系统调用。
- R002 [SUBAGENT:research] 《双层性能门禁与零倒退硬断言脚本架构》：重构 `scripts/audit_performance_regression.py`，支持 3% 警告与 10% 阻断，严重倒退时退出非零状态码 1。

---

## 4. Phase 1: Design Artifacts & Contracts

- **Data Model**: `specs/017-zero-performance-regression-and-floor-enforcement/data-model.md`
- **Contracts**:
  - `specs/017-zero-performance-regression-and-floor-enforcement/contracts/zero_regression_report.schema.json`
- **Quickstart Guide**: `specs/017-zero-performance-regression-and-floor-enforcement/quickstart.md`

---

## 5. Component Breakdown & Planned Modifications

### 5.1 C 桥接层解压与目录元数据优化
- **文件**: `Sources/CTTZipBridge/ttzip_tar_native.c`
- **改动**:
  1. 在 `ttzip_extract_tar_native_c` 与 `ttzip_extract_tar_from_memory` 中引入 `last_parent_dir` 栈上缓存。
  2. 采用乐观 `open` 机制，仅在 `ENOENT` 时递归创建父目录。
  3. 优化 `lzip` 快速级别映射（确保使用 `"compression-level=1"`）。

### 5.2 零倒退审计脚本双级门禁升级
- **文件**: `scripts/audit_performance_regression.py`
- **改动**:
  1. 拆分告警章节为 `## 🟡 性能轻微倒退告警 (3.0% ~ 10.0%)` 与 `## 🔴 严重性能倒退阻断列表 (> 10.0%)`。
  2. 当存在 $> 10.0\%$ 倒退时打印阻断日志并执行 `sys.exit(1)`。
  3. 支持 `--strict` 与指定基准路径参数。

### 5.3 质量与性能门禁回归
- **文件**: `Tests/TTZipTests/` 全量套件
- **验证**:
  1. 运行 `swift test -c release --filter XCTestPerformanceMeasureTests`。
  2. 运行 `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests`。
  3. 运行 `python3 scripts/audit_performance_regression.py` 验证 0 项严重倒退。
