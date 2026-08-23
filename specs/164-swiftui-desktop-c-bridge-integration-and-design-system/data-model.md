# Data Model: SwiftUI 桌面端与 C11 纯微内核深度打通 (Feature 164)

## 1. 实时进度流事件模型 (`ArchiveProgressEvent`)

Represents a lock-free realtime streaming telemetry snapshot dispatched from the C11 engine to SwiftUI views.

| Field Name | Type | Nullable | Constraints / Description |
| :--- | :--- | :---: | :--- |
| `operation_type` | `string` | No | One of: `compress`, `extract`, `inspect`, `verify` |
| `current_file_name` | `string` | No | File or directory currently being processed |
| `completed_files` | `integer` | No | Number of fully processed files |
| `total_files` | `integer` | No | Total count of files in the batch/archive |
| `bytes_processed` | `integer` | No | Cumulative uncompressed/compressed bytes handled |
| `total_bytes` | `integer` | No | Expected total bytes payload |
| `fraction_completed` | `number` | No | `0.0` to `1.0` clamped float progress |
| `instantaneous_mbs` | `number` | No | Rolling throughput in MB/s calculated over 16.6ms window |
| `estimated_time_remaining_sec` | `number` | Yes | Calculated ETA based on remaining bytes and current rate |
| `is_cancelled` | `boolean` | No | True if cancellation was signaled and acknowledged |

---

## 2. 虚拟化文件树行映射模型 (`VirtualTreeRow`)

A lightweight 16-byte projection record for 60fps rendering in `LazyVStack` and `NSOutlineView`.

| Field Name | Type | Nullable | Constraints / Description |
| :--- | :--- | :---: | :--- |
| `row_index` | `integer` | No | Zero-based position in the flattened visible list |
| `node_pointer_address` | `integer` | No | Memory address of native `ttzip_tree_node_t` in C arena |
| `relative_path` | `string` | No | Relative path within archive (e.g. `src/main.c`) |
| `display_name` | `string` | No | Base file name or directory name |
| `depth_level` | `integer` | No | Indentation depth in the hierarchy (0 for root items) |
| `is_directory` | `boolean` | No | True if item is a folder |
| `is_expanded` | `boolean` | No | True if folder children are currently projected in visible list |
| `uncompressed_size_bytes` | `integer` | No | Uncompressed payload size in bytes |
| `compressed_size_bytes` | `integer` | No | Compressed payload size in archive |

---

## 3. 设计系统三栏布局配置模型 (`LayoutGeometryConfig`)

Enforces strict WSJ Editorial and Kintsugi Gold layout constraints across all split panes.

| Field Name | Type | Nullable | Constraints / Description |
| :--- | :--- | :---: | :--- |
| `sidebar_width_pt` | `number` | No | Fixed/default at `200.0` pt (clamped 140pt - 280pt) |
| `workspace_min_width_pt` | `number` | No | Fixed at `450.0` pt (default 600.0pt) |
| `inspector_width_pt` | `number` | No | Fixed at `280.0` pt |
| `top_inset_padding_pt` | `number` | No | Fixed at `38.0` pt |
| `header_bar_height_pt` | `number` | No | Fixed at `52.0` pt |
| `golden_rule_y_offset_pt` | `number` | No | Fixed at `90.0` pt ($38 + 52$) across all 3 columns |
| `kintsugi_gold_color_hex` | `string` | No | Fixed at `#D4AF37` |
