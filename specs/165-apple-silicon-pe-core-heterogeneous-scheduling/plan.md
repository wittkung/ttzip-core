# Implementation Plan: Apple Silicon P/E 核异构调度与 L2 Cache 簇感知并行加速 (Feature 165)

**Feature ID**: `165-apple-silicon-pe-core-heterogeneous-scheduling`  
**Created**: 2026-08-21  
**Status**: Ready for Tasks  

---

## 1. Technical Context & Constitution Check

### 1.1 Technical Context
- **Target Platform**: macOS 14+ (Sonoma / Sequoia), Apple Silicon ARM64 (M1/M2/M3/M4 Series), C11 Clang Native Bridge, POSIX Threads + Darwin QoS (`<pthread/qos.h>`).
- **Core Principles**:
  - Exact `sysctlbyname` hardware introspection (`hw.perflevel0/1.logicalcpu`, `hw.perflevel0/1.l2cachesize`, `hw.perflevel0/1.cpusperl2`).
  - Worker thread spawn-time static QoS initialization (`QOS_CLASS_USER_INTERACTIVE` for P-cores, `QOS_CLASS_UTILITY` for E-cores).
  - Mathematical L2 cache cluster aware chunk slicing keeping working sets resident in cache.
  - Zero lock contention and seamless fallback on x86_64 / non-Darwin.

### 1.2 Constitution Check
- [x] **Zero Cloud Quota / 100% Local**: All topology queries and scheduling logic run strictly locally in-kernel.
- [x] **Strict Native Library Dominance**: Direct POSIX C11 threadpool enhancement.
- [x] **Zero Bare Objects & Schema Strictness**: JSON telemetry contract (`contracts/pe-core-scheduling-schema.json`) enforces strict draft-07 types.
- [x] **60fps UI & Zero Regression**: Latency-critical tasks bound to P-cores, background tasks offloaded to E-cores.

---

## 2. Phase 0 & Phase 1 Artifacts Index

- [x] **Phase 0 Research**: [`research.md`](research.md)
  - `- R001 [SUBAGENT:research] 《Apple Silicon sysctl 硬件拓扑与 L2 Cache 簇内省机制》`
  - `- R002 [SUBAGENT:research] 《C11 线程池静态 QoS 线程初始化与 P/E 核双队列绑定》`
  - `- R003 [SUBAGENT:research] 《L2 Cache 簇容量感知最优无锁分片数学模型》`
- [x] **Phase 1 Data Model**: [`data-model.md`](data-model.md)
- [x] **Phase 1 Contract**: [`contracts/pe-core-scheduling-schema.json`](contracts/pe-core-scheduling-schema.json)
- [x] **Phase 1 Quickstart**: [`quickstart.md`](quickstart.md)

---

## 3. Component Breakdown & Planned Changes

### Component 1: Hardware Topology & L2 Cache Introspection (`Sources/CTTZipBridge/`)
- [NEW] `Sources/CTTZipBridge/include/ttzip_cpu_topology.h`: Header declaring `ttzip_cpu_topology_t`, `ttzip_cpu_topology_detect()`, `ttzip_compute_optimal_chunk_size()`.
- [NEW] `Sources/CTTZipBridge/ttzip_cpu_topology.c`: Implementation querying Darwin sysctl keys with one-time cached initialization.

### Component 2: Heterogeneous QoS Thread Pool Extension (`Sources/CTTZipBridge/`)
- [MODIFY] `Sources/CTTZipBridge/include/ttzip_threadpool.h`: Add `ttzip_qos_tier_t`, `ttzip_threadpool_shared_p()`, `ttzip_threadpool_shared_e()`, `ttzip_parallel_for_qos()`.
- [MODIFY] `Sources/CTTZipBridge/ttzip_threadpool.c`: Implement QoS worker spawn and dedicated queue routing.

### Component 3: C11 Unit Tests & Benchmark Runner (`Tests/c/`)
- [NEW] `Tests/c/test_cpu_topology.c`: Unit tests verifying topology detection and chunk sizing invariants.
- [MODIFY] `CMakeLists.txt`: Register `ttzip_cpu_topology.c` and `test_cpu_topology.c`.
- [MODIFY] `Tests/c/test_main.c`: Register `run_cpu_topology_tests()`.

---

## 4. Verification Plan

1. **C Unit Tests**:
   - `cmake --build build --target ttzip_c_test_runner && ./build/ttzip_c_test_runner cpu_topology`
2. **Full C Test Suites**:
   - `./build/ttzip_c_test_runner all` (25 suites)
3. **5-Gate Zero-Regression Pipeline**:
   - `./scripts/run_optimization_gate.sh --bail --json build/gate_report.json`
