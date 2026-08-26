# Feature Specification: LZ4 Deep Integration and Performance Verification

**Feature Branch**: `064-lz4-deep-integration`
**Created**: 2026-08-17
**Status**: Ready for Planning
**Input**: Full internal integration of native liblz4, dynamic acceleration passthrough, UMA zero-copy memory alignment, and recording pre/post performance benchmarks across all affected formats.

---

## Clarifications

### Session 2026-08-17
- **Q1: 涉及的核心性能验证范围有哪些？**
  - **A**: 覆盖 LZ4 内存编解码吞吐、TAR.LZ4 归档创建与解压、InMemory 基准套件与全格式矩阵回归。
- **Q2: 性能门禁要求是什么？**
  - **A**: Debug 模式下 LZ4 压缩吞吐 >= 6000 MB/s，Release 模式下 >= 10000 MB/s，且受影响测试 100% 零性能倒退（$\Delta \ge 0.0\%$）。

---

## User Scenarios & Testing

### User Story 1 - 原生 LZ4 深度集成与多档位加速验证 (Priority: P1)
系统能够通过原生 C 静态库直连 `LZ4_compress_fast` 与 `LZ4_decompress_safe`，支持 $1 \sim 65537$ 范围内的动态加速因子，在不同数据负载下实现微秒级极速编解码与内存零拷贝流转。

**Acceptance Scenarios**:
1. **Given** 任意内存载荷，**When** 调用 `LZ4LzoEngine.compress(data:acceleration:)`，**Then** 数据以原生速度压缩且解压后 100% 还原。
2. **Given** 连续批处理任务，**When** 运行测试，**Then** 内存开销稳定且无内存泄漏。

---

### User Story 2 - 全矩阵性能测试与差分比对审查 (Priority: P1)
捕获优化前后的各项性能指标（吞吐量 MB/s、耗时、压缩比），输出完整的差分审计表，断言零性能倒退。

**Acceptance Scenarios**:
1. **Given** 性能门禁测试套件，**When** 执行 `XCTestPerformanceMeasureTests`，**Then** LZ4 吞吐维持在 8000+ MB/s（Debug 模式）。
2. **Given** 全格式回归测试，**When** 执行 `AllFormatsAndAdvancedParametersMatrixTests`，**Then** 所有用例 100% 通过。

---

## Requirements

### Functional Requirements
- **FR-001**: 引擎必须 100% 采用内置静态 `liblz4` 原生 API，彻底消除对系统 `<compression.h>` 的依赖。
- **FR-002**: 必须提供加速因子 `acceleration` 的完整透明穿透。
- **FR-003**: 必须通过所有受影响场景的基准测试，并输出结构化差分对比表。

---

## Success Criteria

### Measurable Outcomes
- **SC-001**: LZ4 压缩吞吐在 Debug 模式达到 $\ge 8,000\text{ MB/s}$（远超 6,000 MB/s 底线）。
- **SC-002**: 受影响的归档测试用例 100% 绿灯通过，零倒退。
- **SC-003**: 产出完整的优化前 vs 优化后差分对比审计表。
