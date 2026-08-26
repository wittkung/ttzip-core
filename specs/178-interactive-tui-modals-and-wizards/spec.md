# Feature Specification: 178-interactive-tui-modals-and-wizards

## 1. Executive Summary & Strategic Motivation
In Feature 177, we expanded the CLI capabilities of `bin/ttzip` with standalone subcommands (`recover`, `repair`, `split`, `join`, `bench --pareto`).

In **Feature 178**, we integrate these advanced capabilities seamlessly into the **full-screen interactive TUI explorer (`ttzip <archive>`)**, transforming the terminal into a cohesive, keyboard-driven graphical control center:
1. **Interactive Password Recovery Modal (`r` key / automatic on encrypted entries)**:
   - When browsing an encrypted ZIP or 7z archive, pressing `r` (or attempting extraction without a password) brings up the Password Recovery Modal.
   - Allows selecting built-in quick dictionary, common pin/passwords, or custom dictionary path.
   - Displays real-time live speed meter (keys/s), elapsed time, candidate progress bar, and instant match notification.
   - Upon finding password, automatically unlocks the VFS tree and decrypts selected entries with 0 friction.
2. **Interactive Archive Repair & Salvage Wizard (`R` key / automatic on corrupt archives)**:
   - When `ttzip` detects a corrupted archive (e.g. truncated central directory or damaged header), prompts the user with an interactive diagnosis dialog.
   - Pressing `R` launches the multi-step Repair Wizard: scanning damaged payload via NEON SIMD, listing salvageable items, and reconstructing the TOC into a clean target archive.
3. **Interactive 2D Pareto & MIPS Benchmark Overlay (`B` key)**:
   - Pressing `B` in the explorer opens an interactive Pareto Benchmark modal rendering real-time Braille 8-dot canvas, letting users toggle algorithms, test custom dictionary sizes, and inspect trade-off frontiers.
4. **Interactive Multi-Volume Creator & Split Manager (`S` key)**:
   - In the create/manage view, provides an interactive volume sizing dialog (e.g. CD 700MB, DVD 4.7GB, FAT32 4GB, Discord 25MB, Custom) with instant partition preview.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Interactive In-TUI Password Recovery
- **Given** a user exploring `protected.zip` in `ttzip`
- **When** pressing `r` or attempting to preview/extract an encrypted file
- **Then** a modal appears with live keys/s gauge and dictionary selector; matching password unlocks the preview immediately.

### User Scenario 2: Interactive Corrupted Archive Salvage
- **Given** a user opening a corrupted or truncated archive `corrupt.zip`
- **When** `ttzip` detects missing EOCD or corrupted headers
- **Then** a diagnostic banner suggests repair; pressing `R` executes NEON SIMD stream salvage and rebuilds a valid archive.

### User Scenario 3: Real-Time In-TUI Pareto & MIPS Dashboard
- **Given** a user in the TUI explorer
- **When** pressing `B`
- **Then** a full Ratatui Canvas modal opens showing the live 2D Pareto curve and Andrew's Upper Convex Hull with ASCII metrics.

---

## 3. Success Metrics
1. **Zero Context Switching**: Users can recover passwords, salvage corrupt archives, and inspect benchmark trade-offs entirely inside the interactive TUI without exiting to command line.
2. **Responsive Rendering**: TUI maintains 60 FPS event loop with zero blocking during multi-threaded Rayon recovery or NEON repair passes.
3. **100% Local CI & Tests**: All Rust unit tests and 872+ Swift tests pass with 0 warnings, and 7/7 local CI stages pass.

---

## 4. Clarifications
- **Q1: How do background CPU tasks communicate with the Ratatui event loop?**
  - **Decision**: Uses `tokio::sync::mpsc` or `crossbeam_channel` unbounded/bounded channel sending lightweight `TuiAsyncEvent::Progress(ProgressSnapshot)` and `TuiAsyncEvent::Completed(ResultPayload)` into `AppState::poll_events()`.
- **Q2: What happens when the user presses `Esc` during an active password recovery in TUI?**
  - **Decision**: Triggers `CancellationToken::cancel()`, gracefully terminating all Rayon worker threads in $<5\text{ms}$ and returning to `AppMode::Explorer`.
- **Q3: How are modal dialogs styled in TUI?**
  - **Decision**: Rendered with Centered Rect popup layouts (`ui/modals/`), double-bordered with `Kintsugi Gold` (`Color::Rgb(245, 158, 11)`) header highlights and electric blue progress gauges.

