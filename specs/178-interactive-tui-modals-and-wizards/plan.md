# Implementation Plan: 178-interactive-tui-modals-and-wizards

## Technical Context
- **Target Architecture**: Ratatui 0.28 full-screen interactive TUI explorer (`rust/ttzip-tui`) + Safe Rust concurrency and codec cores (`rust/ttzip-glue`).
- **Core Capabilities Integrating**:
  1. **Interactive Multi-Modal State Machine**: Extended `AppMode` with non-blocking Rayon/I/O crossbeam channel workers and $<5\text{ms}$ cancellation.
  2. **Password Recovery Modal (`r`)**: Interactive dictionary selection, real-time keys/s speedometer, candidate progress bar, and one-key auto-unlock.
  3. **Archive Repair Wizard (`R`)**: NEON SIMD diagnostic scan, salvageable item list preview, and TOC reconstruction.
  4. **2D Pareto Benchmark Canvas Modal (`B`)**: Interactive Braille subpixel canvas with $\log_{10}$ scaling, algorithm filters, and live MIPS scoring.
  5. **Split Manager Modal (`S`)**: Storage media presets (CD, DVD, FAT32, Discord, Custom) with instant volume sequence preview.

---

## Constitution Check
- [x] **Principle 1: Safe Rust First**: All modal logic, channels, and layouts implemented in pure Safe Rust.
- [x] **Principle 2: Zero Cloud Actions Quota**: 100% of testing and verification runs locally.
- [x] **Principle 3: Responsive Event Loop**: Background CPU tasks communicate asynchronously via channels; UI maintains 60 FPS.
- [x] **Principle 4: SRP LOC Budget**: All new files strictly kept under `< 350 LOC`.

---

## Phase 0: Research Items Index
- R001 [SUBAGENT:research] 《TUI 异步非阻塞事件驱动与多模态状态机设计方案》: Completed.
- R002 [SUBAGENT:research] 《交互式密码恢复与自愈修复向导模态框 UI 布局方案》: Completed.
- R003 [SUBAGENT:research] 《交互式 2D 帕累托画布与多分卷管理器在 Ratatui 中的动态渲染与按键映射方案》: Completed.

---

## Phase 1: Architecture Artifacts & Component Change List

### 1. `rust/ttzip-tui/src/app/` State Machine & Types
- **`types.rs`**: Add `PasswordRecovery`, `RepairWizard`, `ParetoBenchmark`, `SplitManager` to `AppMode`.
- **`state.rs`**: Add modal state containers (`RecoveryModalState`, `RepairModalState`, `ParetoModalState`, `SplitModalState`).
- **`input.rs`**: Map `r`, `R`, `B`, `S`, `Tab`, `Enter`, `Esc` to modal actions.

### 2. `rust/ttzip-tui/src/ui/` Modals & Widgets
- **`ui/modals/mod.rs`**: Modal entry and `centered_rect_adaptive` layout helper.
- **`ui/modals/recovery.rs`**: Interactive password recovery modal renderer.
- **`ui/modals/repair.rs`**: Interactive repair and salvage wizard renderer.
- **`ui/modals/pareto.rs`**: Interactive 2D Pareto Braille canvas modal renderer.
- **`ui/modals/split.rs`**: Interactive split manager modal renderer.

---

## Phase 2: Verification Plan
1. `cargo test --manifest-path rust/ttzip-tui/Cargo.toml` verifying all modal transitions and rendering.
2. Build standalone `bin/ttzip` and verify keyboard interactions.
3. `swift test` across all 872+ tests.
4. `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
