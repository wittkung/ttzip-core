# Quickstart Validation Guide: 015 Glue Code & Connection Layer Architecture Evolution

- **Feature Directory**: `specs/015-glue-and-bridge-architecture-evolution`
- **Classification**: `[Full SDD]`
- **Status**: `Ready`
- **Created**: 2026-08-25
- **Author**: Antigravity AI & TTZip Architectural Governance Team

---

## 1. Prerequisites & Environment Setup

- macOS Sonoma 14.0+ (Apple Silicon arm64 / Intel x86_64)
- Xcode 16.0+ with Swift 6 toolchain
- Rust 1.80.0+ with `uniffi-bindgen`
- Python 3.10+, Go 1.22+, OpenJDK 21+, Dart 3.0+, .NET 8.0+

---

## 2. End-to-End Validation Scenarios

### Scenario 1: Large File List Marshalling Stress Test (250,000 Files)
**Goal**: Verify that `CUnsafeBufferAdapter` and `TTZipPackedPathArena` marshal 250,000 files with $O(1)$ stack usage without stack overflow.

```bash
# 1. Run the Swift arena stress test
swift test --filter CUnsafeBufferAdapterTests/testLargePathListArenaAllocation
```
**Expected Outcome**: 250,000 path strings are packed and passed across FFI in $< 35\text{ms}$ with zero `EXC_BAD_ACCESS` or stack overflow signals.

---

### Scenario 2: Persistent VFS Session & Windowed Directory Paging
**Goal**: Verify that browsing inside an archive with 50,000 entries achieves $O(1)$ $< 1\text{ms}$ directory slice retrieval without re-parsing.

```bash
# 1. Run VFS zero-alloc pagination unit and performance tests
cargo test --package ttzip-engine -- test_vfs_windowed_paging --nocapture
swift test --filter RustVfsSessionTests/testWindowedPaginationThroughput
```
**Expected Outcome**: Subdirectory navigation takes $\le 0.5\text{ms}$ per 100-item page, zero full archive re-scans.

---

### Scenario 3: Real-Time Structured Task Cancellation
**Goal**: Verify that cancelling a Swift `Task` interrupts the underlying Rust streaming archive operation within $\le 50\text{ms}$.

```bash
# 1. Execute task cancellation test suite
swift test --filter TTZipEngineActorTests/testImmediateTaskCancellation
```
**Expected Outcome**: Operation immediately halts and returns `ArchiveError.cancelled` in $\le 50\text{ms}$.

---

### Scenario 4: Multi-Language SDK Out-of-Tree Smoke Matrix
**Goal**: Verify all Tier-1 SDKs compile and execute against canonical C-ABI / UniFFI bindings in clean isolated environments.

```bash
# 1. Run root distribution smoke test
make test-out-of-tree-smoke
```
**Expected Outcome**: 100% test pass rate across Python, C, C++, Go, Java, Dart, and C# SDKs.
