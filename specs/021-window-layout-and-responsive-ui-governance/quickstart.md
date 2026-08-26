# Quickstart Validation Guide: TTZip Window Layout & Responsive UI Governance

## Validation Scenarios

### Scenario 1: New Archive Workspace Width & Right Inspector Isolation
1. Launch TTZip.
2. Click "New Archive" (新建归档) on the left sidebar.
3. **Expected Outcome**:
   - The central workspace occupies the full available width (up to 900pt max-width centered).
   - No right inspector or downloads directory tree is visible.
   - All 16 formats, compression levels, and split volume options are fully rendered without horizontal truncation or text ellipsis.

### Scenario 2: Compact Window Breakpoint (<820pt)
1. Resize the TTZip window down to 760x500.
2. **Expected Outcome**:
   - The left sidebar smoothly collapses into a 64pt icon rail (`isLeftCompact = true`).
   - The right inspector automatically hides.
   - The top Omnibar dynamically shrinks without overlapping the macOS red/yellow/green traffic lights on the top-left or the toolbar controls on the top-right.

### Scenario 3: Golden Line (Y = 90pt) Alignment Across All Tabs
1. Switch sequentially through all sidebar tabs: Home, New Archive, Presets, Benchmark, Vault, Plugins, Settings.
2. **Expected Outcome**:
   - The 1.5pt Kintsugi Gold Line in the main area is strictly aligned horizontally with the left sidebar header line at $Y = 90.0\text{pt}$.
   - No vertical jumping or safe area collision occurs.

### Scenario 4: Dwarf Window Scrollability (Height = 500pt)
1. Resize window height to 500pt.
2. Navigate to "Keychain Vault" (in locked state), "Benchmark", and "Presets".
3. **Expected Outcome**:
   - All form controls, buttons, and retry/reset actions can be smoothly scrolled into view and clicked.
