# Quickstart Validation: 178-interactive-tui-modals-and-wizards

## Scenario 1: Interactive TUI Modal Launch & Keybindings
- **Command**:
  ```bash
  cargo test --manifest-path rust/ttzip-tui/Cargo.toml
  ```
- **Expected Output**: All unit tests for `RecoveryModalState`, `RepairModalState`, `ParetoModalState`, and `SplitModalState` pass with 0 failures.
- **Failure Diagnostic**: Verify `AppMode` event handlers and bounds checks.

---

## Scenario 2: Standalone Binary Interactive Smoke Test
- **Command**:
  ```bash
  bin/ttzip --help
  ```
- **Expected Output**: Help shows standalone commands and interactive usage notes.
- **Failure Diagnostic**: Check subcommand parser flags.

---

## Scenario 3: Local 0-Cloud Quota CI Verification
- **Command**:
  ```bash
  ./scripts/run_local_ci_gate.sh
  ```
- **Expected Output**: 7/7 local CI stages pass.
- **Failure Diagnostic**: Check unit test logs or fuzz timeout.
