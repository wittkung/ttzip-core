# Data Model: 200-swift-to-rust-sinking-and-standalone-cli-architecture

## 1. Core Data Transfer Objects (DTOs)

### `VfsTreeContractDto`
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VfsTreeContractDto {
    pub root_path: String,
    pub total_entries_count: usize,
    pub total_uncompressed_bytes: u64,
    pub nodes: Vec<VfsNodeContractDto>,
}
```

### `VfsNodeContractDto`
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VfsNodeContractDto {
    pub name: String,
    pub relative_path: String,
    pub is_directory: bool,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub is_encrypted: bool,
    pub match_indices: Option<Vec<usize>>,
}
```

### `RecoverResultDto`
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverResultDto {
    pub archive: String,
    pub recovered: bool,
    pub password: Option<String>,
    pub total_tested: usize,
    pub elapsed_ms: u64,
    pub speed_keys_per_sec: f64,
}
```

### `RepairResultDto`
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairResultDto {
    pub damaged_archive: String,
    pub repaired_archive: String,
    pub format: String,
    pub salvaged_entries: usize,
    pub elapsed_ms: u64,
}
```

### `SplitResultDto` & `JoinResultDto`
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitResultDto {
    pub source_archive: String,
    pub volume_count: usize,
    pub volume_size_bytes: u64,
    pub volumes: Vec<String>,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinResultDto {
    pub first_volume: String,
    pub output: String,
    pub volume_count: usize,
    pub total_bytes: u64,
    pub volumes: Vec<String>,
    pub elapsed_ms: u64,
}
```
