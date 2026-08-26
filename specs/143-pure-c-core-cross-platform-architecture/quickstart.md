# Quickstart & Verification Guide: Pure C11 Core & Cross-Platform

**Feature**: `143-pure-c-core-cross-platform-architecture`  
**Date**: 2026-08-20  
**Phase**: Phase 1 Design

---

## 1. Overview & Verification Scenarios

### Scenario 1: Standalone Pure C CMake Build Verification
Verifies that `libttzip` builds completely without Swift compiler or Apple SDKs.

- **Command**:
  ```bash
  cmake -B build-c -S . -DTTZIP_BUILD_SHARED=ON -DTTZIP_BUILD_CLI=ON && cmake --build build-c --config Release
  ```
- **Expected Output**:
  ```text
  [100%] Built target libttzip
  [100%] Built target ttzip-cli
  ```
- **Failure Diagnostic**:
  - *Symptom*: Missing `<dispatch/dispatch.h>` or syntax error on `^{}` block.
  - *Remediation*: Ensure the offending source file uses `ttzip_threadpool_submit()` with standard C function pointer callbacks.

---

### Scenario 2: Dual-ISA CRC64 Vector Verification
Validates that CRC64 achieves full hardware vector speedup across both ARM64 and x86_64 architectures.

- **Command**:
  ```bash
  swift test --filter CRC64HardwareTests
  ```
- **Expected Output**:
  ```text
  [CRC64 Hardware Benchmark] PMULL / PCLMULQDQ Throughput: >40,000 MB/s (Speedup: >30x over scalar)
  Test Suite 'CRC64HardwareTests' passed.
  ```
- **Failure Diagnostic**:
  - *Symptom*: CRC64 speed is $< 5,000\text{ MB/s}$ on x86_64.
  - *Remediation*: Confirm `ttzip_cpu_has_feature(TTZIP_CPU_FEAT_X86_PCLMULQDQ)` is returning true and vector loop is executing.
