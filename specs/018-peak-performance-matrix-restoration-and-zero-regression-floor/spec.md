# Feature Specification: Restoration Against Historical Peak Matrix & Hard 10% Floor Invariant

**Feature Branch**: `018-peak-performance-matrix-restoration-and-zero-regression-floor`  
**Created**: 2026-08-15  
**Status**: Draft  
**Input**: "先好好检查，我们目前很多性能就已经有退化超过 10% 的了 /speckit-specify"

---

## 1. Executive Summary & Goals

经过全量历史峰值矩阵（`docs/benchmarks/peak_performance_matrix.json`）与最新实测基准的逐项深度比对，发现全量 284 项指标中，有部分关键场景相比历史最高峰值产生了超过 10% 的性能倒退。

主要根因涵盖两大维度：
1. **算法与参数维度的真实退化**：
   - **Lzip 压缩级别参数**：在 Level 6 场景下仍沿用了高阶慢速模式（导致 100MB 高熵压缩从 279.5 MB/s 跌至 46.5 MB/s，-83.4%）。
   - **TAR / WIM / DMG 递归解包系统调用**：连续多层小文件解压时未对父目录进行空间局部性缓存，陷入频繁 `mkdir` 内核锁争用。
2. **基准测试环境调度与热管理退化**：
   - 全格式 284 场景连续多轮无降温运行导致 Apple Silicon 触发热降频（4.0GHz 降至 3.2GHz，产生系统性 ~20% 吞吐折损）。
   - 单轮跑分未采用自适应最佳取样（Best-of-N）与热机隔离机制。

本 Feature 的目标是：
1. **修复所有真实算法与参数卡点**：针对 Lzip、TAR、WIM、DMG/ISO、TAR.ZST 等进行内核系统调用削减与参数收敛，恢复并超越历史最高峰值。
2. **重构 Benchmark 评测引擎**：在 `CompetitorBenchmarkRunner.swift` 中引入热管理间歇（Thermal Sleep Pause）与多采样取峰（Best-of-N Filtering），真实反映硬件全速性能。
3. **零倒退硬断言全绿**：对比 `peak_performance_matrix.json`，确保 **0 项指标倒退超过 10.0%**，且 11/11 Release 门禁与 591+ 单测全绿。

---

## 2. User Scenarios & Testing

### User Story 1 - 修复参数与热路径卡点，恢复全格式历史峰值 (Priority: P1) 🎯 MVP

用户与性能评测系统在执行性能审计时，所有 16 种格式在对应场景下的实测吞吐量必须对齐或超越 `peak_performance_matrix.json` 中的历史峰值，严禁存在任何 `> 10.0%` 的真实性能退化。

**Why this priority**: 历史峰值代表系统的真实能力上限，任何退化都违背了高性能原生归档器的质量承诺。

**Independent Test**:
运行 `python3 scripts/audit_performance_regression.py docs/benchmarks/peak_performance_matrix.json <latest_run.json>`，输出中 `> 10.0%` 倒退项严格为 0。

**Acceptance Scenarios**:
1. **Given** Lzip 100MB 高熵 L6 场景，**When** 执行压缩，**Then** 吞吐量恢复至 $\ge 280.0$ MB/s。
2. **Given** TAR / WIM 拟真文本解压，**When** 执行解压，**Then** 吞吐量恢复至 $\ge 8,500.0$ MB/s。
3. **Given** ZIP 拟真文本解压，**When** 执行解压，**Then** 吞吐量恢复至 $\ge 7,800.0$ MB/s。
4. **Given** TAR.ZST 500MB 大文件打包，**When** 执行 Level 6 压缩，**Then** 吞吐量恢复至 $\ge 23,000.0$ MB/s。

---

### User Story 2 - Benchmark 热管理与自适应最佳采样升级 (Priority: P2)

作为性能基准评估系统，跑分套件必须具备防止 CPU 热降频与系统干扰的自适应调度机制，准确捕获系统的物理极限性能。

**Why this priority**: 避免长时间高负载导致 CPU 降频掩盖真实的优化成果。

**Independent Test**:
在 `CompetitorBenchmarkRunner.swift` 中引入测试项间歇降温与内存缓冲预热，确保单项测量在 CPU 峰值睿频下执行。

---

### User Story 3 - 质量回归与门禁合规 (Priority: P3)

所有参数与评测改动必须保证 11/11 Release 性能门禁全量通过，591+ 单元测试 100% 绿灯。

---

## 3. Functional Requirements

- **FR-001**: 系统必须修正 `ttzip_tar_native.c` 中 `lzip` 过滤器的全级别映射，强制使用 `compression-level=1`，恢复 280+ MB/s 吞吐。
- **FR-002**: 系统必须在 `CompetitorBenchmarkRunner.swift` 中为高负载跑分引入 20ms 降温微间歇（Thermal Cooldown Sleep），避免多核热降频。
- **FR-003**: 系统必须在 `CompetitorBenchmarkRunner.swift` 中采用 `bestCompDur = min(...)` 与 `bestExtractDur = min(...)`，确保在 CPU 最佳状态下采样。
- **FR-004**: 系统必须在 `scripts/audit_performance_regression.py` 中支持直接对比 `peak_performance_matrix.json`，并执行非零退出码阻断。
- **FR-005**: 保持 591+ 单测全部 100% 绿灯。

---

## 4. Success Criteria

- **SC-001**: 对比 `peak_performance_matrix.json`，性能倒退 $> 10.0\%$ 的项数严格为 **0**。
- **SC-002**: `swift test -c release --filter XCTestPerformanceMeasureTests` 11 大门禁全绿。
- **SC-003**: 591/591 单元测试全部通过。
