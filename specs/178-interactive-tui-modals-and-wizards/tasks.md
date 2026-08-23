# Tasks: 178-interactive-tui-modals-and-wizards

## Phase 1: TUI Multi-Modal State Machine & Asynchronous Channels (US1)
- [x] T001 [P] [US1] Extend `AppMode` in `rust/ttzip-tui/src/app/types.rs` with `PasswordRecovery`, `RepairWizard`, `ParetoBenchmark`, `SplitManager`.
- [x] T002 [P] [US1] Extend `AppState` in `rust/ttzip-tui/src/app/state.rs` with modal states and batch event drain handling.
- [x] T003 [P] [US1] Implement `centered_rect_adaptive` and common modal frame helpers in `rust/ttzip-tui/src/ui/modals/mod.rs`.
- [x] T004 [P] [US1] Map top-level keybindings (`r`, `R`, `B`, `S`) in `rust/ttzip-tui/src/app/input.rs`.

## Phase 2: Interactive Password Recovery Modal & Live Speedometer (US2)
- [x] T005 [P] [US2] Implement `RecoveryModalState` and background Rayon worker dispatcher in `rust/ttzip-tui/src/app/recovery_runner.rs`.
- [x] T006 [P] [US2] Implement `render_recovery_modal` (dictionary picker, live keys/s gauge, hit notification) in `rust/ttzip-tui/src/ui/modals/recovery.rs`.
- [x] T007 [P] [US2] Handle password recovery key navigation (`Tab`, `Enter`, `Esc`, `c`) and auto-unlock in `rust/ttzip-tui/src/app/input.rs`.
- [x] T008 [P] [US2] Add unit tests for password recovery modal state transitions in `rust/ttzip-tui/src/app/tests.rs`.

## Phase 3: Interactive Corrupted Archive Repair & Salvage Wizard (US3)
- [x] T009 [P] [US3] Implement `RepairModalState` and background NEON salvage scanner in `rust/ttzip-tui/src/app/repair_runner.rs`.
- [x] T010 [P] [US3] Implement `render_repair_modal` (diagnostic header, rescued entries table, output path input) in `rust/ttzip-tui/src/ui/modals/repair.rs`.
- [x] T011 [P] [US3] Handle repair wizard key navigation and TOC assembly in `rust/ttzip-tui/src/app/input.rs`.
- [x] T012 [P] [US3] Add unit tests for repair wizard in `rust/ttzip-tui/src/app/tests.rs`.

## Phase 4: Interactive 2D Pareto Canvas & Split Manager Modals (US4)
- [x] T013 [P] [US4] Implement `render_pareto_modal` using `ratatui::widgets::canvas::Canvas` (Marker::Braille, $\log_{10}$ scaling, filter toggles) in `rust/ttzip-tui/src/ui/modals/pareto.rs`.
- [x] T014 [P] [US4] Implement `render_split_modal` (presets CD/DVD/FAT32/Discord, dynamic partition preview) in `rust/ttzip-tui/src/ui/modals/split.rs`.
- [x] T015 [P] [US4] Handle Pareto zoom/filter keys and Split preset selection in `rust/ttzip-tui/src/app/input.rs`.
- [x] T016 [P] [US4] Add unit tests for Pareto canvas and Split manager in `rust/ttzip-tui/src/ui/tests.rs`.

## Phase 5: Verification, CI Gates & Standalone CLI Validation (US5)
- [x] T017 [US5] Run `cargo test --workspace` on all Rust crates (`ttzip-glue`, `ttzip-tui`).
- [x] T018 [US5] Run `./scripts/build_rust.sh --release` and `./scripts/build_tui.sh` and verify interactive `bin/ttzip`.
- [x] T019 [US5] Run `swift test` ensuring all 872+ tests pass with 0 failures and 0 warnings.
- [x] T020 [US5] Run `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
