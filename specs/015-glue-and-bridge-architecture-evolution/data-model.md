# Data Model & UniFFI IDL Specifications: 015 100% Pure UniFFI Architecture

- **Feature Directory**: `specs/015-glue-and-bridge-architecture-evolution`
- **Classification**: `[Full SDD]`
- **Status**: `Completed`
- **Created**: 2026-08-25
- **Author**: Antigravity AI & TTZip Architectural Governance Team

---

## 1. UniFFI Interface Definitions & Domain Models

### 1.1 `UniFFICreateOptions` (Compression Configuration Record)
```rust
#[derive(Clone, Debug, uniffi::Record)]
pub struct UniFFICreateOptions {
    pub format: ArchiveFormat,
    pub level: CompressionLevel,
    pub encryption: EncryptionMethod,
    pub password: Option<String>,
    pub thread_budget: u32,
    pub solid_block_size_mb: u32,
    pub split_volume_size_bytes: Option<u64>,
    pub skip_mac_junk: bool,
}
```

---

### 1.2 `UniFFIExtractOptions` (Extraction Configuration Record)
```rust
#[derive(Clone, Debug, uniffi::Record)]
pub struct UniFFIExtractOptions {
    pub password: Option<String>,
    pub thread_budget: u32,
    pub overwrite_existing: bool,
    pub preserve_permissions: bool,
    pub dry_run: bool,
}
```

---

### 1.3 `UniFFIEntryMetadata` (Archive Entry Record)
```rust
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UniFFIEntryMetadata {
    pub path: String,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub mtime_epoch_secs: i64,
    pub mode: u32,
    pub is_directory: bool,
    pub is_encrypted: bool,
    pub compression_method: String,
    pub detected_encoding: Option<String>,
}
```

---

### 1.4 `UniFFIVfsTree` (In-Memory VFS Object)
```rust
#[derive(uniffi::Object)]
pub struct UniFFIVfsTree {
    tree: parking_lot::RwLock<crate::fs::vfs::tree::VfsTree>,
}

#[uniffi::export]
impl UniFFIVfsTree {
    #[uniffi::constructor]
    pub fn build(entries: Vec<UniFFIEntryMetadata>, root_name: String) -> Arc<Self>;

    pub fn get_children(&self, dir_node_id: u32, offset: u32, limit: u32) -> VfsNodeSlice;
    pub fn fuzzy_search(&self, query: String) -> Vec<UniFFIVfsMatch>;
    pub fn render_tree(&self) -> String;
    pub fn get_stats(&self) -> VfsStats;
}
```

---

### 1.5 `CancellationToken` (Thread-Safe Atomic Cancellation Object)
```rust
#[derive(uniffi::Object)]
pub struct CancellationToken {
    cancelled: AtomicBool,
}

#[uniffi::export]
impl CancellationToken {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self>;
    pub fn cancel(&self);
    pub fn is_cancelled(&self) -> bool;
}
```

---

### 1.6 `ProgressHandler` (Native Callback Interface)
```rust
#[uniffi::export(callback_interface)]
pub trait ProgressHandler: Send + Sync {
    fn on_progress(&self, processed_bytes: u64, total_bytes: u64, current_entry: Option<String>) -> bool;
}
```

---

### 1.7 `TTZipEngineCore` (Unified Engine Facade Object)
```rust
#[derive(uniffi::Object)]
pub struct TTZipEngineCore;

#[uniffi::export]
impl TTZipEngineCore {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self>;

    pub fn create_archive(
        &self,
        input_paths: Vec<String>,
        output_path: String,
        options: UniFFICreateOptions,
        progress: Option<Box<dyn ProgressHandler>>,
        cancel_token: Option<Arc<CancellationToken>>,
    ) -> Result<CompressionReport, TTZipError>;

    pub fn extract_archive(
        &self,
        archive_path: String,
        destination_dir: String,
        options: UniFFIExtractOptions,
        progress: Option<Box<dyn ProgressHandler>>,
        cancel_token: Option<Arc<CancellationToken>>,
    ) -> Result<CompressionReport, TTZipError>;

    pub fn inspect_archive(
        &self,
        archive_path: String,
        password: Option<String>,
        cancel_token: Option<Arc<CancellationToken>>,
    ) -> Result<Vec<UniFFIEntryMetadata>, TTZipError>;

    pub fn extract_selected(
        &self,
        archive_path: String,
        target_entries: Vec<String>,
        destination_dir: String,
        password: Option<String>,
        cancel_token: Option<Arc<CancellationToken>>,
    ) -> Result<u64, TTZipError>;

    pub fn extract_audio_waveform(&self, path: String, bucket_count: u32) -> Result<Vec<f32>, TTZipError>;
    pub fn extract_audio_waveform_from_memory(&self, data: Vec<u8>, bucket_count: u32) -> Result<Vec<f32>, TTZipError>;
}
```
