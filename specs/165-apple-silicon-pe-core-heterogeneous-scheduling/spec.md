# Feature Specification: Apple Silicon P/E 核异构调度与 L2 Cache 簇感知并行加速 (Feature 165)

**Feature ID**: `165-apple-silicon-pe-core-heterogeneous-scheduling`  
**Created**: 2026-08-21  
**Status**: In Progress (Specification Phase)  
**Priority**: P0 (Microarchitecture, Apple Silicon NUMA/Topology, Energy & Latency)

---

## 1. Executive Summary

Apple M 系列芯片（M1/M2/M3/M4 系列）采用了高度非对称的异构微架构：
1. **性能核（P-Core, Firestorm/Avalanche/M-series）**：拥有宽流水线（8-10 decode/cycle）、大容量共享 L2 缓存（12MB~16MB/cluster）、超高单核频率与强劲 NEON/PMULL 向量单元，适合**低延迟即时解压与交互式打包**；
2. **能效核（E-Core, Icestorm/Blizzard/M-series）**：功耗极低，拥有独立的 4MB L2 缓存簇，适合**后台长时批处理归档、全零块扫描与熵计算**。

若采用传统的简单 POSIX 线程池均匀分发，解压主任务可能被调度到 E-Core 导致延迟翻倍，而后台大文件压缩占用全部 P-Core 会导致笔记本严重发热降频。
本特性的目标是：**在 C11 微内核 (`ttzip_threadpool.c`) 中构建 Apple Silicon 硬件拓扑感知器与异构 QoS 调度器，实现解压任务 P-Core 独占、后台归档 E-Core 降频节电、以及基于 L2 Cache 簇容量的无锁分片调度**。

---

## 2. User Scenarios

### User Scenario 1 (US1) - 毫秒级解压响应 P-Core 独占加速 (Low-Latency P-Core Decompress)
- **As a**: 点击打开数十 GB 归档文件的 macOS 桌面用户
- **I want to**: 解压任务以最高优先级在全量性能核（P-Cores）上并发跑满
- **So that**: 享受极致的 10+ GB/s 解压速度与零卡顿体验。

### User Scenario 2 (US2) - 后台静默大归档 E-Core 节能批处理 (Energy-Efficient E-Core Batch Compress)
- **As a**: 正在后台打包 50GB 代码库或媒体素材的开发者
- **I want to**: 压缩任务自动路由至能效核（E-Cores）或以 `QOS_CLASS_UTILITY` 执行
- **So that**: 笔记本保持凉爽静音（0 风扇噪音），且不抢占前台 Xcode 编译或 UI 渲染。

### User Scenario 3 (US3) - L2 Cache 簇感知无锁分片 (L2 Cluster-Aware Chunk Slicing)
- **As a**: 追求微架构吞吐极限的系统工程师
- **I want to**: 压缩块分片大小与 P-Core/E-Core 的 L2 缓存簇容量严格对齐
- **So that**: 消除跨核 L2 缓存一致性总线（Interconnect）抖动，达到理论最大带宽。

---

## 3. Functional Requirements

- **REQ-001 (Hardware Topology Introspection)**: 在 `ttzip_cpu_topology.c` 中通过 `sysctlbyname("hw.perflevel0.logicalcpu")` 与 `hw.perflevel1.logicalcpu` 精确探测 P-Core 与 E-Core 数量及各自 L2 Cache 大小。
- **REQ-002 (Heterogeneous QoS Thread Queues)**: 在 `ttzip_threadpool.c` 中划分两个独立的物理工作队列：`P-Core Queue` (`QOS_CLASS_USER_INTERACTIVE`) 与 `E-Core Queue` (`QOS_CLASS_UTILITY`)。
- **REQ-003 (QoS-Aware Task Dispatch)**: 提供 C11 调度接口 `ttzip_threadpool_dispatch_qos(pool, qos_tier, task_fn, ctx)`.
- **REQ-004 (L2 Cache Chunk Sizing)**: 自动根据目标 Core 类型的 L2 容量（P-Core: 8MB~16MB, E-Core: 2MB~4MB）计算最佳分片大小 `ttzip_compute_optimal_chunk_size(core_type, algo)`.
- **REQ-005 (Swift Concurrency QoS Binding)**: 在 `Sources/TTZipCore/ConcurrencyBridge.swift` 中暴露 `ThreadBudget.heterogeneousDispatch(...)`.

---

## 4. Success Criteria

1. **P-Core 调度纯净度**: 解压高优先任务下，100% 任务由 `QOS_CLASS_USER_INTERACTIVE` 线程执行；
2. **E-Core 功耗与发热降低**: 后台批处理模式下，CPU 核心温度与能耗显著下降，前台 UI 渲染维持 60fps；
3. **L2 缓存命中与分片提速**: L2 Cache 簇感知分片使大文件多核压缩吞吐提升 **5%~15%**；
4. **门禁与测试**: 原生 C11 拓扑测试套件 100% 通过，`./scripts/run_optimization_gate.sh` 维持全绿。
