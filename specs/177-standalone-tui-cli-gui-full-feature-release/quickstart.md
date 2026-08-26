# Quickstart Validation: 177-standalone-tui-cli-gui-full-feature-release

## Scenario 1: Standalone CLI Benchmark with 2D Braille Pareto Plot
- **Command**:
  ```bash
  bin/ttzip bench --mips --pareto
  ```
- **Expected Output**: ASCII/Braille scatter chart with Andrew's Upper Convex Hull and MIPS rating summary.
- **Failure Diagnostic**: Check terminal column width or PRNG seed generator.

---

## Scenario 2: Standalone CLI Split, Recover and Repair
- **Command**:
  ```bash
  cargo test --manifest-path rust/ttzip-tui/Cargo.toml
  ```
- **Expected Output**: TUI VFS, CLI subcommands (recover, repair, split, bench) pass with 0 failures.
- **Failure Diagnostic**: Verify command parsing and path existence assertions.

---

## Scenario 3: Local 0-Cloud Quota Release Packaging & Gate
- **Command**:
  ```bash
  ./scripts/package_local_release.sh --skip-dmg
  ./scripts/run_local_ci_gate.sh
  ```
- **Expected Output**: `dist/` contains release artifacts and local CI passes 7/7 stages.
- **Failure Diagnostic**: Check binary paths or strip flags.
