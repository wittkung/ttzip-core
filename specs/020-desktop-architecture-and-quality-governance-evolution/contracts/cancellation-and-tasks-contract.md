# Contract: Cooperative Task Cancellation & UniFFI Bridge

- **Version**: 1.0.0
- **Scope**: Swift Task Layer <-> UniFFI Bridge <-> Rust Core Engine

---

## 1. Interface Signatures

### Swift 6 Native Layer
```swift
public protocol ArchiveTaskCoordinating: Sendable {
    func registerTask(name: String, type: ArchiveOperationType, totalBytes: Int64) -> TaskExecutionHandle
    func cancelTask(id: UUID)
    func pauseTask(id: UUID)
    func resumeTask(id: UUID)
}
```

### UniFFI Rust Boundary
```rust
#[uniffi::export]
pub fn create_archive_stream(
    source_paths: Vec<String>,
    output_path: String,
    format: ArchiveFormat,
    level: i32,
    password: Option<String>,
    progress: Option<Box<dyn ProgressHandler>>,
    token: Option<Arc<CancellationToken>>,
) -> Result<CompressionReport, TTZipError>;

#[uniffi::export]
pub fn extract_archive_stream(
    archive_path: String,
    destination_dir: String,
    password: Option<String>,
    progress: Option<Box<dyn ProgressHandler>>,
    token: Option<Arc<CancellationToken>>,
) -> Result<CompressionReport, TTZipError>;
```

## 2. Invariants & Guardrails
1. When `token.cancel()` is called from Swift, the Rust engine MUST abort all active chunk processing loops within $\le 100\text{ms}$.
2. Incomplete output files MUST be removed from disk upon cancellation to prevent corrupted archive state.
