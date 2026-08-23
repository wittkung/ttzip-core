# Feature Specification: 189-production-core-de-tox-and-pure-facade-sinking

## 1. Executive Summary & Strategic Motivation
A rigorous audit revealed 5 anomalous redundancies within the `Sources/TTZipCore` production module:
1. **Misplaced Test Harnesses in Production Target**: 17 files in `Sources/TTZipCore/Testing/` belong in test/bench targets, not production core libraries.
2. **Mock Artifacts in Production**: `Sources/TTZipCore/Mocks/MockFacades.swift` is an anti-pattern in production code.
3. **Duplicate AST Interpreter**: 5 files in `Sources/TTZipCore/InterpreterPattern/` replicate logic already native in `rust/ttzip-glue/src/fs/filter_dsl.rs`.
4. **Duplicate Visitor AST Traversal**: 7 files in `Sources/TTZipCore/VisitorPattern/` replicate VFS tree rendering and scanning already native in Rust `fs/vfs/tree.rs`.
5. **Legacy Deflate State Machine**: 3 files in `Sources/TTZipCore/Pipeline/DeflateStreamEngine*.swift` replicate hardware-accelerated Rust Libdeflate streams.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Clean Production Binary Without Test Bloat
- **Given** compiling the release application and CLI
- **When** building `TTZipCore`
- **Then** zero test frameworks, mocks, or duplicate interpreters are compiled into production binaries.

### User Scenario 2: Instant Delegated Execution
- **Given** evaluating filter DSL rules or rendering tree views
- **When** executing via Swift facades
- **Then** execution is directly handled by Rust C-ABI with zero Swift interpreter overhead.

---

## 3. Success Metrics
1. **Source Code Purge**: Delete 33 misplaced/duplicate Swift files (~6,500 LOC eliminated).
2. **Production Purity**: `Sources/TTZipCore/` contains 0 testing harnesses and 0 mocks.
3. **Zero Regression**: 100% pass rate on `cargo test`, `swift test`, and `./scripts/run_local_ci_gate.sh`.
