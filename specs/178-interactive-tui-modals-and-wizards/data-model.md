# Data Model: 178-interactive-tui-modals-and-wizards

## 1. Multi-Modal State Models (`rust/ttzip-tui/src/app/types.rs` & `state.rs`)
- **`AppMode`**:
  - `Explorer`, `Search`, `Preview`, `Progress`, `PasswordRecovery`, `RepairWizard`, `ParetoBenchmark`, `SplitManager`, `Help`, `Exiting`
- **`RecoveryModalState`**:
  - `dict_choice: usize` (0: Built-in Top 10K, 1: PIN Brute, 2: Custom)
  - `custom_dict_path: String`
  - `tested_count: usize`
  - `total_words: usize`
  - `keys_per_sec: f64`
  - `found_password: Option<String>`
  - `is_running: bool`
- **`RepairModalState`**:
  - `stage: usize` (0: Diagnostics, 1: Rescued Table, 2: Output Path & Assembly, 3: Success)
  - `salvaged_entries: Vec<RescuedEntryDto>`
  - `output_path: String`
  - `is_repairing: bool`
- **`ParetoModalState`**:
  - `points: Vec<ParetoPointRaw>`
  - `filter_mode: usize` (0: All, 1: Convex Only, 2: Optimal Only)
  - `zoom: f64`
  - `selected_idx: usize`
- **`SplitModalState`**:
  - `preset_idx: usize`
  - `custom_size_str: String`
  - `naming_scheme: usize` (0: .7z.001, 1: .z01, 2: .001)
  - `calculated_volumes: Vec<(String, u64)>`
