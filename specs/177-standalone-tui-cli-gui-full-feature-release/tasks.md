# Tasks: 177-standalone-tui-cli-gui-full-feature-release

## Phase 1: Standalone CLI Subcommands Expansion (US1)
- [x] T001 [P] [US1] Extend `rust/ttzip-tui/src/cli/args.rs` with `Recover`, `Repair`, `Split`, `Join` commands and `-v / --volume-size` option in `Create`.
- [x] T002 [P] [US1] Implement `execute_recover` (multi-core in-memory dictionary recovery) and `execute_repair` in `rust/ttzip-tui/src/cli/handlers.rs`.
- [x] T003 [P] [US1] Implement `execute_split` and `execute_join` in `rust/ttzip-tui/src/cli/handlers.rs`.
- [x] T004 [P] [US1] Extend `rust/ttzip-tui/src/cli/format.rs` with Snappy (`.sz`) and Brotli (`.br`) container format detection and transparent multi-volume chain resolution.

## Phase 2: Terminal 2D Braille Pareto Frontier & MIPS Benchmark Plotter (US2)
- [x] T005 [P] [US2] Implement `TerminalBrailleCanvas` (U+2800..U+28FF) and Bresenham line drawing in `rust/ttzip-tui/src/cli/braille_plotter.rs`.
- [x] T006 [P] [US2] Implement `ParetoPlotCoordinateEngine` with $\log_{10}$ throughput projection in `rust/ttzip-tui/src/cli/braille_plotter.rs`.
- [x] T007 [P] [US2] Implement `execute_bench` with 7-Zip MIPS rating and ASCII/Braille Pareto chart output in `rust/ttzip-tui/src/cli/handlers.rs`.
- [x] T008 [P] [US2] Add unit and integration tests for Braille plotter and CLI commands in `rust/ttzip-tui/src/cli/tests.rs`.

## Phase 3: SwiftUI macOS VFS 16-Way LZ4 Cache & QuickLook 7z Solid Stream Integration (US3)
- [x] T009 [P] [US3] Connect `ArchiveExplorerView` and `FinderMillerColumnsView` with Rust 16-way sharded `VFSLz4CachePool` chunk prefetching.
- [x] T010 [P] [US3] Ensure `SevenZipSeekTable.swift` and QuickLook preview engine fully utilize <10ms in-memory early termination stream decoding.
- [x] T011 [P] [US3] Connect `PasswordVaultView` with Rust multi-core in-memory password verification engine.
- [x] T012 [P] [US3] Add unit tests for SwiftUI VFS and QuickLook integrations.

## Phase 4: Local-Only Automated Release Packaging Pipeline (US4)
- [x] T013 [P] [US4] Create `./scripts/package_local_release.sh` with single-command build, strip, DMG, CLI tarball, and checksum generation.
- [x] T014 [P] [US4] Update `Formula/ttzip-cli.rb` Homebrew tap generator in `package_local_release.sh`.
- [x] T015 [P] [US4] Test `./scripts/package_local_release.sh --skip-dmg` generating release artifacts in `dist/`.
- [x] T016 [P] [US4] Ensure 0 calls to GitHub Actions or remote cloud quotas.

## Phase 5: Verification, CI Gates & Standalone CLI Validation (US5)
- [x] T017 [US5] Run `cargo test --workspace` on all Rust crates (`ttzip-glue`, `ttzip-tui`).
- [x] T018 [US5] Run `./scripts/build_rust.sh --release` and `./scripts/build_tui.sh` and test `bin/ttzip bench --mips --pareto`.
- [x] T019 [US5] Run `swift test` ensuring all 866+ tests pass with 0 failures and 0 warnings.
- [x] T020 [US5] Run `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
