# Feature Specification: 188-grand-swift-test-purge-and-minimal-facade-tests

## 1. Executive Summary & Strategic Motivation
With the Safe Rust microkernel (`rust/ttzip-glue`) and its 21 comprehensive integration test suites as the **single source of truth**, keeping 100+ Swift test files in `Tests/TTZipTests/` is an anti-pattern.
1. **Purge 70+ Redundant Low-Level Swift Test Files**:
   - Delete all internal stream, format matrix, mutation fuzzing, differential oracle, and scaffolding tests from `Tests/TTZipTests/`.
2. **Establish Minimal High-Level Swift Facade Tests**:
   - Retain only high-level public API integration (`TTZipCoreIntegrationTests.swift`), CLI E2E tests (`CLICommandE2ETests.swift`), QuickLook preview tests (`QuickLookPreviewTests.swift`), and App Store audit tests (`AppStorePackageAuditTests.swift`).
3. **Streamline Local CI Gate**:
   - Update `./scripts/run_local_ci_gate.sh` to run the lean Swift suite in $<1\text{s}$ and the full Rust industrial suite in $<2\text{s}$.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Sub-Second Swift Test Execution
- **Given** running `swift test` on macOS
- **When** executing tests
- **Then** the suite completes all high-level facade tests in $<1.0\text{s}$ with 100% pass rate.

### User Scenario 2: Rust as the Sole Industrial Engine Testbed
- **Given** running `cargo test --workspace`
- **When** verifying algorithmic invariants, fuzzing, and compliance
- **Then** all 220+ tests execute cross-platform in $<2\text{s}$.

---

## 3. Success Metrics
1. **File Purge**: Delete 70+ redundant Swift test files (~10,000 LOC eliminated).
2. **Swift Test Count**: Swift tests reduced from 609 to $< 50$ clean, high-level E2E tests.
3. **Zero Regression**: 100% pass rate across Cargo and Swift tests.
