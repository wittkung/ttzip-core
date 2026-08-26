# Implementation Plan: 177-standalone-tui-cli-gui-full-feature-release

## Technical Context
- **Target Architecture**: Self-sufficient standalone Safe Rust binary (`rust/ttzip-tui` -> `bin/ttzip`) + Thin native macOS SwiftUI App (`Sources/TTZipApp`) + Local 0-cloud-quota packaging scripts.
- **Core Components Releasing**:
  1. **TUI & CLI Subcommands**: `recover`, `repair`, `split`, `join`, `bench --mips --pareto`, `create -v`, transparent multi-volume `list`/`extract`, Snappy/Brotli native support.
  2. **Terminal Braille 2D Pareto Plotter**: Unicode 8-dot $2 \times 4$ subpixel canvas, $\log_{10}$ throughput projection, and Andrew's Upper Convex Hull rasterization.
  3. **SwiftUI macOS App UX & VFS Integration**: 16-way sharded LZ4 VFS cache pool prefetching and QuickLook <10ms 7z solid stream early termination.
  4. **Local 0-Cloud Quota Packaging**: Automated `./scripts/package_local_release.sh` building DMG, App bundle, CLI tarball, and Homebrew formula.

---

## Constitution Check
- [x] **Principle 1: Safe Rust First**: Full terminal features are built directly on `ttzip-glue` with 0 external runtime dependencies.
- [x] **Principle 2: Zero Cloud Actions Quota**: All building, testing, linting, packaging, and validation run 100% locally.
- [x] **Principle 3: Subpixel High Fidelity**: Terminal Pareto plotting uses Braille 8-dot rasterization for maximum precision without graphical protocol lock-in.
- [x] **Principle 4: Zero Breaking Changes**: All existing CLI commands and SwiftUI workflows maintain backward compatibility, passing all tests and 7/7 local CI stages.

---

## Phase 0: Research Items Index
- R001 [SUBAGENT:research] 《`ttzip-tui` / 独立 CLI 子命令与 Ratatui 交互体系设计方案》: Completed.
- R002 [SUBAGENT:research] 《终端 2D 帕累托散点与 Andrew 凸包 Braille 字符渲染方案》: Completed.
- R003 [SUBAGENT:research] 《SwiftUI macOS 客户端与 Rust VFS 缓存池/7z 内存截断深度集成方案》: Completed.
- R004 [SUBAGENT:research] 《本地 0 云端额度多目标自动化构建与发布打包方案》: Completed.

---

## Phase 1: Architecture Artifacts & Component Change List

### 1. `rust/ttzip-tui/` Modules
- **`src/cli/args.rs`**: Add `Recover`, `Repair`, `Split`, `Join`, and `--pareto`/`--mips` options to `Bench`.
- **`src/cli/handlers.rs`**: Implement `execute_recover`, `execute_repair`, `execute_split`, `execute_join`, `execute_bench`.
- **`src/cli/braille_plotter.rs`**: Implement `TerminalBrailleCanvas`, `ParetoPlotCoordinateEngine`, and ANSI/Braille chart generator.
- **`src/cli/format.rs`**: Add Snappy (`.sz`) and Brotli (`.br`) container detection.

### 2. SwiftUI Integration
- **`Sources/TTZipCore/SevenZip/SevenZipSeekTable.swift`**: Early termination solid stream extraction.
- **`Sources/TTZipApp/ViewModels/AppViewState+ArchiveOperations.swift`**: VFS LZ4 cache pool prefetching.

### 3. Local Release Packaging
- **`scripts/package_local_release.sh`**: Single-command local release pipeline.

---

## Phase 2: Verification Plan
1. `cargo test --workspace` across all unit, property, and integration tests.
2. `./scripts/build_rust.sh --release` and `./scripts/build_tui.sh`.
3. Test standalone `bin/ttzip` subcommands (`bench --pareto`, `recover`, `repair`, `split`).
4. `swift test` across all 866+ tests ensuring 100% green.
5. `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
