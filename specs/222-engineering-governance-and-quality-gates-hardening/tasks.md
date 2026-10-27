# Tasks: Engineering Governance and Quality Gates Hardening

**Feature**: `222-engineering-governance-and-quality-gates-hardening`  
**Status**: Ready for Implementation  
**Spec**: [`spec.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/222-engineering-governance-and-quality-gates-hardening/spec.md) | **Plan**: [`plan.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/222-engineering-governance-and-quality-gates-hardening/plan.md)

---

## Phase 1: Setup & Foundational Invariants

- [x] **T001**: Create automated C-ABI symbol extractor and verification script in [`scripts/verify_cabi_symbols.sh`](file:///Users/kevintung/Documents/dev/TTZip/scripts/verify_cabi_symbols.sh) to parse `ttzip_rust_glue.h` and assert defined symbols in `libTTZipVendor.a`.
- [x] **T002**: Ensure executable permissions (`chmod +x`) on [`scripts/verify_cabi_symbols.sh`](file:///Users/kevintung/Documents/dev/TTZip/scripts/verify_cabi_symbols.sh) and verify clean exit on current codebase.

---

## Phase 2: User Story 1 - Automated C-ABI & Static Library Symbol Gate (Priority: P1)

- [x] **T003**: Create Swift test suite [`Tests/TTZipTests/CABISymbolGateTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/CABISymbolGateTests.swift) to dynamically verify that key C-ABI function pointers resolve without crashing at test runtime.
- [x] **T004**: Integrate `./scripts/verify_cabi_symbols.sh` into [`scripts/run_local_ci_gate.sh`](file:///Users/kevintung/Documents/dev/TTZip/scripts/run_local_ci_gate.sh) as mandatory Stage 1.5 before compilation and benchmark execution.

---

## Phase 3: User Story 2 - Large-Scale & Split Volume Verification Suite (Priority: P1)

- [x] **T005**: Implement [`Tests/TTZipTests/LargeVolumeStressTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/LargeVolumeStressTests.swift) with tests for 10,000 synthetic nested entries, testing differential rollback and ensuring pre-existing destination files are untouched.
- [x] **T006**: Add multi-volume split archive (`.001`, `.002`, `.003`) virtual inspection and single-entry extraction tests in [`Tests/TTZipTests/LargeVolumeStressTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/LargeVolumeStressTests.swift) asserting zero intermediate files created in `/tmp`.

---

## Phase 4: User Story 3 - Sensitive Memory Sanitizer & Lifetime Hardening (Priority: P2)

- [x] **T007**: Create sanitizer test harness script [`scripts/run_sanitizers.sh`](file:///Users/kevintung/Documents/dev/TTZip/scripts/run_sanitizers.sh) supporting `--asan` (AddressSanitizer) and `--tsan` (ThreadSanitizer) for Swift and Rust workspaces.
- [x] **T008**: Add unit test in [`Tests/TTZipTests/VaultMemorySanitizationTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/VaultMemorySanitizationTests.swift) asserting zero page retention after multiple encryption/decryption cycles with `SecureBytes`.

---

## Phase 5: CI/CD Pipeline Integration & Verification

- [x] **T009**: Run `./scripts/lint_loc_gate.sh` to enforce $\le 800$ LOC across all new script and test files.
- [x] **T010**: Execute `./scripts/run_local_ci_gate.sh` and verify all 5 stages pass 100%.
