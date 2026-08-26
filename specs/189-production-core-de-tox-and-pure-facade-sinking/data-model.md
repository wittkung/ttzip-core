# Data Model: 189-production-core-de-tox-and-pure-facade-sinking

## 1. Filter DSL Invocation Model
- **`FilterDSLEvaluationRequest`**:
  - `expression: String`
  - `file_name: String`
  - `file_size: u64`
  - `is_directory: bool`
  - `unix_timestamp: i64`
  - `result: bool`

## 2. VFS Tree Rendering Model
- **`VfsTreeRenderOptions`**:
  - `use_unicode: bool`
  - `max_depth: usize`
  - `show_sizes: bool`
