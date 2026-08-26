# Tasks: 195-modernize-build-rust-and-purge-vendor-build-artifacts

## Phase 1: Modernize build_rust.sh & Delete libTTZipVendor.a (US1)
- [x] T001 [P] [US1] Modernize `scripts/build_rust.sh` to output directly to `Vendor/TTZipVendor.xcframework/` without creating `Vendor/lib` or `Vendor/include`.
- [x] T002 [P] [US1] Delete `Vendor/libTTZipVendor.a`.

## Phase 2: Purge Upstream CMake Build Directories (US2)
- [x] T003 [P] [US2] Delete `Vendor/*/build*` directories across upstream packages (> 516 MB).

## Phase 3: CI Alignment & Final Verification (US3)
- [x] T004 [US3] Run `./scripts/build_rust.sh` to verify clean build directly into XCFramework.
- [x] T005 [US3] Run `./scripts/lint_loc_gate.sh` to ensure all files $\le 800\text{ LOC}$.
- [x] T006 [US3] Verify `swift build` and `swift test` 100% PASS with zero warnings.
- [x] T007 [US3] Run `cargo test --workspace` on all Rust crates.
- [x] T008 [US3] Run `./scripts/run_local_ci_gate.sh` full CI validation.
