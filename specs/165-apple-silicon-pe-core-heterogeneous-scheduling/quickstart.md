# Quickstart Guide: Apple Silicon P/E 核异构调度验证 (Feature 165)

## Scenario 1: 硬件拓扑与 L2 Cache 簇正确性验证
- **Command**:
  ```bash
  ./build/ttzip_c_test_runner cpu_topology
  ```
- **Expected Output**:
  - `[PASS] test_apple_silicon_topology_detection`
  - `[PASS] test_l2_cache_cluster_chunk_calculation`
  - 验证正确识别 P-Core / E-Core 数量，且 L2 Cache 簇分片在 256KB~4MB 范围内核算正确。

---

## Scenario 2: 异构 QoS 调度纯净度验证
- **Command**:
  ```bash
  ./build/ttzip_c_test_runner threadpool_qos
  ```
- **Expected Output**:
  - `[PASS] test_p_core_threadpool_qos_dispatch`
  - `[PASS] test_e_core_threadpool_qos_dispatch`
  - 验证 P-Core 队列执行延迟 < 1ms，E-Core 队列在低优先级稳态运行。

---

## Scenario 3: 全量 5 重门禁回归验证
- **Command**:
  ```bash
  ./scripts/run_optimization_gate.sh --bail --json build/gate_report.json
  ```
- **Expected Output**:
  - All 5 stages pass in < 5 seconds with zero regressions.
