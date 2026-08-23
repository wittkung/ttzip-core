# Data Model: 177-standalone-tui-cli-gui-full-feature-release

## 1. CLI Command Models (`rust/ttzip-tui/src/cli/args.rs`)
- **`Commands`**:
  - `List`: path, json, resolve_symlinks
  - `Extract`: archive, output, password, format
  - `Create`: archive, sources, format, level, password, threads, volume_size
  - `Recover`: archive, dictionary, threads, json
  - `Repair`: damaged_archive, output, format, json
  - `Split`: source_archive, volume_size, output_dir, naming
  - `Join`: first_volume, output, json
  - `Bench`: mips, pareto, dict_size_mb, threads, iterations, json, width, height

## 2. Braille Plotter Models (`rust/ttzip-tui/src/cli/braille_plotter.rs`)
- **`BrailleCell`**:
  - `mask: u8`
  - `fg_color: Option<Color>`
- **`TerminalBrailleCanvas`**:
  - `cols: usize`
  - `rows: usize`
  - `cells: Vec<BrailleCell>`
- **`ParetoPlotCoordinateEngine`**:
  - `min_log_x: f64`
  - `max_log_x: f64`
  - `min_y: f64`
  - `max_y: f64`
  - `dot_w: usize`
  - `dot_h: usize`
