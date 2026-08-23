# Implementation Plan: Libarchive-Inspired In-Process Native Test Log & Harness Architecture

**Feature ID**: `109-native-test-log-and-harness-architecture`  
**Status**: Ready for Tasks  

---

## 1. Technical Context

- **Current State**:
  - `TTZipTestObserver.swift` exists in `Tests/TTZipTests/` but only manages log capture clearing.
  - Raw `swift test` output is verbose and cluttered with 3,300+ lines of per-case lifecycle events.
  - `scripts/pretty_test.py` was a temporary external workaround that violated the zero-script architectural purity principle.
- **Target State**:
  - Full-featured in-process test reporter via `XCTestObservation` protocol.
  - Single-line suite reporting, on-demand failure trace dumping, and libarchive-style structured totals summary.
  - Full compliance with Swift 6.0 and zero-warning compilation.

---

## 2. Constitution & Invariant Check

- [x] **Zero External Script Dependencies**: 100% pure Swift / C in-process execution.
- [x] **Hot-Path Zero-Allocation**: Test logging operates strictly on heap-isolated memory ring buffers (`TTLogger`), having zero impact on compression hot paths.
- [x] **Zero Compiler Warnings**: Verified against `-Xswiftc -warnings-as-errors`.
- [x] **Performance Floor**: Test execution completes in < 40 seconds.

---

## 3. Phase 0 Research & Phase 1 Design Artifacts

- **Phase 0 Research**:
  - `- R001 [SUBAGENT:research] 《Libarchive C 基础库错误语义模型与 POSIX 头文件规范》`: [`research.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/109-libarchive-standards-and-zero-warning-hardening/research.md)
  - `- R002 [SUBAGENT:research] 《Swift 6.0 编译门禁与原生 XCTestObservation 进程内生命周期拦截》`: [`research.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/109-libarchive-standards-and-zero-warning-hardening/research.md)
- **Phase 1 Design**:
  - Data Model: [`data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/109-libarchive-standards-and-zero-warning-hardening/data-model.md)
  - Contract Schema: [`contracts/test_report_schema.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/109-libarchive-standards-and-zero-warning-hardening/contracts/test_report_schema.json)
  - Quickstart Guide: [`quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/109-libarchive-standards-and-zero-warning-hardening/quickstart.md)

---

## 4. Component Change Plan

### Component 1: `Tests/TTZipTests/TTZipTestObserver.swift`
- Expand `TTZipTestObserver` to conform fully to `XCTestObservation`:
  - `testBundleWillStart(_:)`: Print test session header.
  - `testSuiteWillStart(_:)` & `testSuiteDidFinish(_:)`: Aggregate test counts and elapsed time per suite, emitting a single compact formatted line.
  - `testCase(_:didRecord:)`: Capture failed assertion details, file path, and line number.
  - `testCaseDidFinish(_:)`: If failed, dump captured ring buffer; if passed, clear silently.
  - `testBundleDidFinish(_:)`: Print libarchive-grade Totals summary table.

### Component 2: `Sources/TTZipCore/Utilities/Logger.swift`
- Optimize `TTLogger`'s test mode:
  - Ring buffer capacity of 2,000 entries.
  - Safe formatting with ANSI colors and file:line markers.

### Component 3: `scripts/` Cleanup
- Remove obsolete temporary wrapper scripts (`scripts/pretty_test.py`).
- Update `./scripts/run_all_tests.sh` and `./scripts/lint_codebase_standards.sh` to run the native in-process test observer directly.
