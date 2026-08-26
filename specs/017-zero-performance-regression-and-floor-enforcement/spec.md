# Feature Specification: Zero Performance Regression Governance & Hard Floor Invariant Enforcement

**Feature Branch**: `017-zero-performance-regression-and-floor-enforcement`  
**Created**: 2026-08-15  
**Status**: Draft  
**Input**: "所以 好好看看性能倒退情况 绝对有很多超过 10% 底线要求 /speckit-specify"

---

## 1. Executive Summary & Goals

在 Feature 016 将全 16 格式 284 项竞品对决胜率推升至 94.01%（解压胜率 95.77%）的大满贯攻坚之后，最新性能比对审计报告（`docs/benchmarks/latest_regression_audit.md`）揭示了 38 处不同程度的性能变动，其中数个极端场景出现了超过 10% 的严重性能倒退（如 `lzip` 在参数调整失误期间的极端退化、`wim` 海量小文件与高熵解压退化、`tar.xz` 高熵解压抖动、`7z` 拟真日志文本 AES 压缩抖动等）。

本 Feature（`017-zero-performance-regression-and-floor-enforcement`）的目标是：
1. **全面根因治理倒退项**：逐项排查并根治全部 38 处性能倒退告警，彻底消除所有 `> 10.0%` 的性能倒退，将核心场景倒退率控制在 `< 3.0%` 严格红线以内。
2. **固化绝对性能基线 (Performance Floor Lock)**：在 `docs/benchmarks/peak_performance_matrix.json` 中建立不可逾越的绝对历史峰值基线，并将自动化零倒退审查脚本（`scripts/audit_performance_regression.py`）作为 CI/CD 本地与流水线硬阻断门禁。
3. **保持大满贯胜率与 100% 单测通过率**：在达成零倒退的同时，稳固 94%+ / 冲刺 100% 竞品擂台胜率，保持 11/11 Release 性能门禁全绿与 591+ 全量单元测试 100% 绿灯。

---

## 2. User Scenarios & Testing

### User Story 1 - 彻底根除全格式 > 10% 性能倒退 (Priority: P1) 🎯 MVP

作为 TTZip 架构师与性能守门员，在运行自动化性能回归审计脚本时，系统必须验证全 16 格式、5 大物理数据集维度、全部压缩级别与加密状态下的 284 项指标，确保 **0 项超过 10.0% 的性能倒退**。

**Why this priority**: 10% 是绝对不可突破的性能安全红线，任何倒退超过 10% 的改动都属于重大回归，必须被硬性阻断并立刻修复。

**Independent Test**:
运行 `python3 scripts/audit_performance_regression.py`，输出中 `【🔴 严重倒退告警 (<-10.0%)】` 数量必须为 **0**。

**Acceptance Scenarios**:
1. **Given** 历史基准与最新构建的 284 项实测性能数据，**When** 运行零倒退审计脚本，**Then** 严重倒退（$< -10.0\%$）项数为 0。
2. **Given** Lzip 100MB 高熵场景，**When** 执行压缩与解压，**Then** 压缩吞吐恢复至 $\ge 270$ MB/s，解压吞吐恢复至 $\ge 1800$ MB/s（彻底修复参数失误引入的退化）。
3. **Given** WIM 格式海量小文件与高熵数据，**When** 执行打包与解包，**Then** 解压吞吐恢复至 $\ge 10,000$ MB/s，打包吞吐恢复至 $\ge 1,050$ MB/s。
4. **Given** 7Z 拟真文本 L1 AES 压缩，**When** 执行加密压缩，**Then** 吞吐恢复至 $\ge 3,300$ MB/s。
5. **Given** DMG 格式海量小文件 L6 解压，**When** 执行解压提取，**Then** 吞吐恢复至 $\ge 1,400$ MB/s。

---

### User Story 2 - 核心热路径性能倒退率严格收敛至 < 3.0% (Priority: P2)

作为性能调优工程师，在全量 284 项指标中，核心热路径（ZIP、7Z、TAR.ZST、TAR.XZ、TAR Direct 等）的任何吞吐回退必须严格收敛在 $\le 3.0\%$ 的测量噪声区间以内。

**Why this priority**: 项目全局规则规定核心场景性能倒退 $> 3.0\%$ 属于违规，必须实施精细化调优使性能稳步攀升或严格持平。

**Independent Test**:
运行 `python3 scripts/audit_performance_regression.py`，核心热路径倒退告警数为 0，整体提升项数明显大于持平与轻微抖动项数。

**Acceptance Scenarios**:
1. **Given** ZIP Level 1 / Level 6 / AES 与 Store 场景，**When** 运行性能门禁测试，**Then** 吞吐不仅不发生倒退，且全面达标 Release 门禁底线。
2. **Given** 7Z Level 1 / Level 5 LZMA2 场景，**When** 运行单测门禁，**Then** 吞吐全部高于历史基线。

---

### User Story 3 - 保持 11/11 Release 性能门禁与 591+ 全量单测 100% 绿灯 (Priority: P3)

作为质量保障系统，所有性能修复与参数收敛改动必须通过全量 591+ 单元测试与 11 项 XCTest 性能测试。

**Why this priority**: 性能治理绝不能破坏功能正确性、内存安全与数据完整性。

**Independent Test**:
- `swift test` 591/591 测试通过。
- `swift test -c release --filter XCTestPerformanceMeasureTests` 11/11 门禁通过。

**Acceptance Scenarios**:
1. **Given** 全量单元测试套件，**When** 执行 `swift test`，**Then** 591 个测试用例无一失败。
2. **Given** Release 性能门禁，**When** 执行 `swift test -c release --filter XCTestPerformanceMeasureTests`，**Then** 11 项硬门禁全部通过。

---

## 3. Functional Requirements

- **FR-001**: 系统必须修正 `ttzip_tar_native.c` 中 `lzip` 过滤器的压缩级别映射，确保快速级别正确使用 `compression-level=1`，彻底恢复 Lzip 在高熵与大文件下的百兆与千兆级吞吐。
- **FR-002**: 系统必须优化 WIM 格式在海量小文件遍历时的目录元数据缓存与直通 I/O，消除小文件打包与高熵解压时的 14% 吞吐退化。
- **FR-003**: 系统必须优化 7Z ARMv8 AES-256 加密管线在 10MB 文本场景下的线程启动与块调度开销，恢复 3,300+ MB/s 吞吐。
- **FR-004**: 系统必须优化 DMG / ISO 镜像提取器在小文件多层目录下的递归解包开销，消除 14.4% 的解压退化。
- **FR-005**: 系统必须优化 TAR.BZ2 与 TAR.XZ 在高熵大块下的多线程流式解压上下文复用，消除解压吞吐抖动。
- **FR-006**: 系统必须在 `scripts/audit_performance_regression.py` 中引入双级门禁（严格警告 $> 3.0\%$，阻断失败 $> 10.0\%$），并在检测到任何 $> 10.0\%$ 倒退时退出非零状态码。
- **FR-007**: 保持 `AllFormatsPkSuiteTests` 竞品擂台总胜率 $\ge 94.0\%$，解压胜率 $\ge 95.0\%$。
- **FR-008**: 保持 591+ 单元测试 100% 绿灯，0 裸日志违规。

---

## 4. Success Criteria

- **SC-001 (零严重倒退)**: 全 284 项对决指标中，性能倒退 $> 10.0\%$ 的项数严格为 **0**。
- **SC-002 (核心路径收敛)**: 核心热路径（ZIP, 7Z, TAR.ZST, TAR.XZ）性能倒退率全部收敛在 $< 3.0\%$ 以内。
- **SC-003 (性能硬门禁 100% 通过)**: `swift test -c release --filter XCTestPerformanceMeasureTests` 11/11 项全量通过。
- **SC-004 (全量单测 100% 绿灯)**: `swift test` 591/591 测试通过。

---

## 5. Clarifications

### Session 2026-08-15
- **Q**: 如何判定哪些场景属于真实性能倒退 vs 系统噪声？
  - **A**: 单次测量的轻微浮动（$\le 3.0\%$）属于系统调度抖动；变动在 $3.0\% \sim 10.0\%$ 属于轻微倒退告警；变动 $> 10.0\%$ 属于严重性能倒退，必须立刻定位根因并修复。
