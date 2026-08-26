# Phase 0 Research: 178-interactive-tui-modals-and-wizards

## Research Item R001: TUI Async Non-Blocking Event-Driven Architecture & Multi-Modal State Machine
- **Decision**: Extend `AppMode` with `PasswordRecovery`, `RepairWizard`, `ParetoBenchmark`, and `SplitManager`. Bridge Rayon and I/O worker threads to the main 60 FPS TUI loop via `crossbeam_channel::bounded(512)` with "blocking first event + batch try-next drain" pattern, achieving $<5\text{ms}$ atomic cancellation via `CancellationToken`.
- **Rationale**: 
  - Pure OS thread channels avoid Tokio runtime bloat (+1.5MB) while providing 30ns cross-thread message dispatch.
  - Chunk batching (1,000 words for Rayon / 64KB for I/O) guarantees microsecond cancellation response and smooth 60 FPS rendering without UI starvation.
- **Alternatives Considered**: 
  - *Tokio async channels*: Unnecessary runtime complexity for CPU-bound hashing and blocking disk I/O.
  - *Global `Arc<RwLock<AppState>>`*: High lock contention and cache-line bouncing during Rayon compute loops.
- **Source**: 
  - `rust/ttzip-tui/src/app/types.rs:L11-19`
  - `rust/ttzip-tui/src/app/state.rs:L25-161`
  - `rust/ttzip-tui/src/event.rs:L20-140`
  - `rust/ttzip-glue/src/runtime/cancellation.rs:L46-101`

---

## Research Item R002: Interactive Password Recovery & Repair Wizard Modal Layouts
- **Decision**: Implement `PasswordRecoveryModal` (dictionary preset picker, real-time keys/s gauge, ETA, one-click auto-unlock) and `RepairWizardModal` (two-stage: read-only NEON SIMD diagnostic scan -> user confirmation -> reconstructed archive assembly) with Kintsugi Gold dual borders and `centered_rect_adaptive`.
- **Rationale**: 
  - `centered_rect_adaptive` maintains guaranteed minimum dimensions (width 72, height 20) on small $80 \times 24$ terminals while scaling gracefully on 4K displays.
  - Two-stage repair prevents accidental data corruption by previewing salvaged items before writing.
- **Alternatives Considered**: 
  - *Silent automatic repair overwriting originals*: High data loss risk.
  - *Fixed hardcoded popup geometry*: Causes Ratatui layout overflow panics on small windows.
- **Source**: 
  - `rust/ttzip-tui/src/cli/handlers/recover.rs:L62-113`
  - `rust/ttzip-tui/src/cli/handlers/repair.rs:L17-71`
  - `rust/ttzip-tui/src/ui/progress.rs:L21-73`

---

## Research Item R003: Interactive 2D Pareto Canvas & Split Manager in Ratatui
- **Decision**: Implement `ParetoBenchmarkModal` using `ratatui::widgets::canvas::Canvas` with `Marker::Braille` and $\log_{10}$ X-axis projection, and `SplitManagerModal` with pre-configured media sizing presets (CD 700MB, DVD 4.7GB, FAT32 4GB, Discord 25MB/500MB, Custom) and instant volume naming preview.
- **Rationale**: 
  - `Marker::Braille` provides $2 \times 4$ subpixel geometry without reallocating string buffers every frame.
  - Sizing presets decouple storage target constraints from container naming schemes.
- **Alternatives Considered**: 
  - *Re-rendering raw terminal strings via Paragraph*: Inflexible layout scaling and high memory churn.
- **Source**: 
  - `rust/ttzip-tui/src/cli/braille_plotter.rs:L63-182`
  - `rust/ttzip-glue/src/archive/split/mod.rs:L18-36`
  - `rust/ttzip-glue/src/bench/pareto.rs:L37-142`
