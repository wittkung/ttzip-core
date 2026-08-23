# Tasks: Apple Silicon P/E 核异构调度与 L2 Cache 簇感知并行加速 (Feature 165)

**Feature ID**: `165-apple-silicon-pe-core-heterogeneous-scheduling`  
**Created**: 2026-08-21  
**Status**: Ready for Implementation  

---

## Phase 1: Setup & Topology Headers

- [x] T001 Create `Sources/CTTZipBridge/include/ttzip_cpu_topology.h` declaring topology struct and API signatures

---

## Phase 2: User Story 1 (P1) - Hardware Topology & L2 Cache Introspection

- [x] T002 [P] [US1] Implement `ttzip_cpu_topology.c` with Darwin `hw.perflevel0/1` sysctl queries and cross-platform fallbacks
- [x] T003 [P] [US1] Implement L2 cluster chunk calculation in `ttzip_compute_optimal_chunk_size`

---

## Phase 3: User Story 2 (P2) - Heterogeneous QoS Threadpool in C11

- [x] T004 [P] [US2] Update `Sources/CTTZipBridge/include/ttzip_threadpool.h` with `ttzip_qos_tier_t` and `ttzip_parallel_for_qos`
- [x] T005 [P] [US2] Update `Sources/CTTZipBridge/ttzip_threadpool.c` with Darwin QoS worker threads and P/E pool routing

---

## Phase 4: User Story 3 (P3) - Unit Tests & Build System Integration

- [x] T006 [P] [US3] Implement `Tests/c/test_cpu_topology.c` verifying topology queries and chunk sizing
- [x] T007 [P] [US3] Register new files and targets in `CMakeLists.txt` and `Tests/c/test_main.c`

---

## Phase 5: Verification & Zero-Regression Gating

- [x] T008 [US1] Build and run `./build/ttzip_c_test_runner cpu_topology`
- [x] T009 [US1] Build and run `./build/ttzip_c_test_runner all` (all 25 suites pass)
- [x] T010 [US1] Run `./scripts/run_optimization_gate.sh --bail --json build/gate_report.json`
