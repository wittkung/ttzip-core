# Feature Specification: Streamlining Redundant Swift C-Wrapper Tests & Dual-Engine Decoupling

**Feature ID**: `155-155-streamline-redundant`  
**Created**: 2026-08-20  
**Status**: Ready for Plan  

---

## 1. Problem Statement & Motivation

With the successful deployment of the zero-dependency C11 test framework (`tests/c/ttzip_test_harness.h`) and isolated CTest runners executing in **< 4ms**, low-level algorithmic, checksum, and container bitstream tests are now natively and losslessly verified in pure C.

However, `Tests/TTZipTests/` still contains duplicate, redundant Swift XCTest files that merely serve as thin FFI wrappers over C functions (e.g. `CRC32PmullDifferentialTests.swift`, `HardwareChecksumTests.swift`, `SingleCoreDeflateOracleTests.swift`, etc.). These redundant files:
1. Impose significant compilation and linking overhead on SwiftPM (`swift test`).
2. Duplicate test maintenance surface across two languages.
3. Obscure the clean architectural separation between C microkernel algorithms and Swift application/architecture layers.

---

## 2. User Scenarios & Priorities

### User Story 1 (Priority: P1) - Audit & Prune Redundant C-Wrapper Swift Tests
As a TTZip developer, I want all Swift test files whose assertions are 100% superseded by `tests/c/test_*.c` to be cleanly pruned from `Tests/TTZipTests/`, so that `swift test` builds and executes with maximum velocity without duplicate tests.

- **Acceptance Scenario 1.1**: Identify all Swift test classes that exclusively test C functions now covered in `tests/c/`.
- **Acceptance Scenario 1.2**: Remove these redundant test files safely while retaining Swift architectural pattern tests, ConcurrencyBridge tests, and AppKit GUI tests.
- **Acceptance Scenario 1.3**: `swift test` and `swift build --build-tests` compile cleanly with 0 errors and 0 warnings.

### User Story 2 (Priority: P2) - Solidify Dual-Engine Boundaries & CI Optimization
As a CI engineer, I want the Swift test suite and C test suite to have crystal-clear domain boundaries and optimal execution in `scripts/local-ci.sh`.

- **Acceptance Scenario 2.1**: CTest in Stage 2 validates 100% of microkernel algorithms in < 50ms.
- **Acceptance Scenario 2.2**: Swift test filter in Stage 5 validates all Swift architecture, design patterns, and AppKit state without running duplicate C tests.
- **Acceptance Scenario 2.3**: Full local CI execution completes faster with 0 warnings.

---

## 3. Functional Requirements

- **FR-001**: The system MUST audit and prune Swift test files that only wrap C functions already verified by `tests/c/test_*.c`.
- **FR-002**: The system MUST retain all Swift architectural pattern tests (Adapter, Bridge, Command, Observer, State, Visitor, etc.) and Swift 6 concurrency tests (`ConcurrencyBridgeTests.swift`).
- **FR-003**: The system MUST retain all AppKit GUI View and ViewModel tests in `Tests/TTZipAppTests/`.
- **FR-004**: The project MUST maintain 0 compiler and 0 linker warnings across all build configurations.

---

## 4. Success Criteria

- **SC-001 (Zero Loss of Coverage)**: 100% of microkernel mathematical and bitstream invariants remain fully verified in `tests/c/`.
- **SC-002 (Swift Test Acceleration)**: Swift test compilation and discovery time reduced due to removal of dead test files.
- **SC-003 (Zero Warnings)**: `swift build`, `swift build --build-tests`, and `cmake --build build` produce 0 warnings.
- **SC-004 (Local CI Green)**: All 5 stages of `scripts/local-ci.sh` pass cleanly with exit code 0.
