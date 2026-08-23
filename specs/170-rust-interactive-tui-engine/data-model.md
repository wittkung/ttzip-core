# Data Model: TTZip 交互式 TUI 与 CLI 引擎 (Feature 170)

**Feature ID**: `170-rust-interactive-tui-engine`  
**Created**: 2026-08-21  
**Status**: Completed  
**Artifact**: Phase 1 Data Model & Types

---

## 1. TUI 核心状态与数据模型

### 1.1 应用运行模式与状态机 (App Mode & State)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Explorer,
    Search,
    Preview,
    Progress,
    Help,
    Exiting,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub archive_path: std::path::PathBuf,
    pub archive_format: String,
    pub total_size_bytes: u64,
    pub uncompressed_size_bytes: u64,
    pub entries_count: usize,
    pub selected_index: usize,
    pub search_query: String,
    pub preview_content: Option<PreviewData>,
    pub progress_state: Option<ProgressSnapshot>,
    pub current_mode: AppMode,
}
```

### 1.2 虚拟文件系统树 (VFS Tree Node)

```rust
#[derive(Debug, Clone)]
pub struct VfsNode {
    pub name: String,
    pub relative_path: String,
    pub is_dir: bool,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub is_encrypted: bool,
    pub children: Vec<VfsNode>,
    pub is_expanded: bool,
    pub is_selected: bool,
}
```

### 1.3 实时进度与吞吐快照 (Progress Snapshot)

```rust
#[derive(Debug, Clone)]
pub struct ProgressSnapshot {
    pub task_title: String,
    pub current_entry_name: String,
    pub processed_bytes: u64,
    pub total_bytes: u64,
    pub processed_entries: usize,
    pub total_entries: usize,
    pub instant_throughput_mb_per_sec: f64,
    pub elapsed_seconds: f64,
    pub eta_seconds: f64,
}
```

### 1.4 免解压预览数据 (Preview Data)

```rust
#[derive(Debug, Clone)]
pub enum PreviewData {
    Text {
        lines: Vec<String>,
        syntax_language: String,
        is_truncated: bool,
    },
    HexDump {
        offset_hex_pairs: Vec<(String, String, String)>, // (Offset, Hex, ASCII)
        total_bytes_displayed: usize,
    },
    Unsupported {
        reason: String,
        file_size_bytes: u64,
    },
}
```
