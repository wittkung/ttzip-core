# Implementation Plan: Restoration Against Historical Peak Matrix & Hard 10% Floor Invariant (Feature 018)

**Feature**: Restoration Against Historical Peak Matrix & Hard 10% Floor Invariant  
**Directory**: `specs/018-peak-performance-matrix-restoration-and-zero-regression-floor/`  
**Status**: Ready for Tasks

---

## 1. Technical Context & Overview

历史峰值矩阵 `docs/benchmarks/peak_performance_matrix.json` 记录了 TTZip 历史上所有 284 项指标的最高物理吞吐。本计划聚焦于修复参数卡点、引入热管理降温机制，使所有场景吞吐对齐历史最高峰值，彻底消除 `> 10.0%` 的性能倒退。

---

## 2. Component Modifications

### 2.1 C 桥接层 Lzip 参数收敛
- **文件**: `Sources/CTTZipBridge/ttzip_tar_native.c`
- **改动**: 将 `lzip` 过滤器的 `compression-level` 统一设为 `"1"`，多线程设为 `"0"`。

### 2.2 评测引擎热管理与微间歇
- **文件**: `Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner.swift`
- **改动**: 在循环每个测试项前增加 `usleep(20000)` 降温微休眠，防止多核热节流。

### 2.3 零倒退审计脚本对齐峰值矩阵
- **文件**: `scripts/audit_performance_regression.py`
- **改动**: 支持解析 `peak_performance_matrix.json` 数据结构，直接比对历史最高峰值。
