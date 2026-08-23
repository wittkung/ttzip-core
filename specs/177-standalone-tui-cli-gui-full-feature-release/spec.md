# Feature Specification: 177-standalone-tui-cli-gui-full-feature-release

## 1. Executive Summary & Strategic Motivation
With the core computation, codecs, concurrency, and I/O pipelines fully sunk into `ttzip-glue` in Features 174-176, the project now needs to expose these capabilities across **standalone terminal TUI/CLI (`bin/ttzip`)** and **native macOS GUI/CLI**, backed entirely by local automation scripts (zero GitHub Actions cloud quota consumption).

This feature accomplishes:
1. **`ttzip-tui` / Standalone CLI Full-Feature Release**:
   - Multi-volume archive creation (`ttzip create -v 100m`) and transparent multi-volume extraction (`ttzip extract archive.z01`).
   - High-throughput multi-core password dictionary recovery (`ttzip recover <archive> -d <dict.txt>`).
   - Archive repair & salvage command (`ttzip repair <damaged.zip/tar> -o <repaired>`).
   - Terminal 2D Pareto frontier & MIPS benchmark visualization (`ttzip bench --mips --pareto`).
   - Direct support for Snappy Framed (`.sz`) and pure Rust Brotli (`.tar.br`) in TUI/CLI.
2. **SwiftUI macOS Native App UX & VFS Integration**:
   - VFS instant preview backed by Rust 16-way sharded LZ4 cache pool.
   - QuickLook <10ms in-memory 7z solid stream extraction.
   - Password recovery progress dashboard in `PasswordVaultView`.
3. **Local-Only Zero-Cloud-Quota Automated Packaging & Gate**:
   - Local multi-target build script and standalone distribution packaging.
   - 100% local CI/CD gate with 0 remote GitHub Actions dependency.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Standalone CLI Multi-Volume & Repair
- **Given** a terminal user on Linux or macOS without Swift runtime
- **When** running `ttzip create -v 50m big.zip /path/to/data` or `ttzip repair corrupt.zip -o fixed.zip`
- **Then** the operations execute natively via `bin/ttzip` in seconds with zero extra dependencies.

### User Scenario 2: In-Terminal Password Recovery with Live Speedometer
- **Given** an encrypted archive
- **When** running `ttzip recover protected.zip -d wordlist.txt`
- **Then** recovery runs across all CPU cores with a live TUI/CLI throughput meter (>150,000 keys/sec).

### User Scenario 3: Real-Time Terminal 2D Pareto & MIPS Benchmark
- **Given** a user evaluating system compression throughput vs ratio
- **When** running `ttzip bench --pareto`
- **Then** the terminal renders a clean ASCII/Braille scatter chart of the Pareto frontier and Andrew's Upper Convex Hull.

---

## 3. Success Metrics
1. **Standalone Binary Capabilities**: `bin/ttzip` supports `create`, `extract`, `list`, `tui`, `bench`, `recover`, `repair`, `split` commands natively.
2. **Password Recovery Throughput**: >150,000 keys/sec on multi-core CPU.
3. **Zero Cloud Actions Quota**: 100% of compilation, verification, and regression tests execute locally via `./scripts/run_local_ci_gate.sh`.
4. **Zero Regression**: 100% pass rate across 186+ Rust tests and 866+ Swift tests.

---

## 4. Clarifications
- **Q1: How are the new CLI subcommands structured in `ttzip-tui`?**
  - **Decision**: In `rust/ttzip-tui/src/cli/args.rs`, `Commands` enum is extended with `Recover`, `Repair`, `Split`, `Join`, and `--pareto`/`--mips` flags on `Bench`.
- **Q2: How does the terminal 2D Pareto chart render in text mode?**
  - **Decision**: `ttzip-tui` uses `ratatui::widgets::canvas` with Braille subpixel characters (`⠋`, `⠙`, `⠹`...) and ANSI color gradients, mapping throughput (X-axis) against space savings percentage (Y-axis) with Andrew's Upper Convex Hull connecting frontier points.
- **Q3: How are local release packages produced without cloud CI?**
  - **Decision**: `./scripts/package_local_release.sh` is created to build and pack universal macOS binaries (`bin/ttzip`), zip release archives, generate checksums, and update documentation.

