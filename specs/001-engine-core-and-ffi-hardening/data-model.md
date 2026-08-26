# Data Model: TTZip Engine Core & FFI Hardening

This document defines the core data structures, memory layouts, FFI representations, and state transitions for the TTZip engine.

---

## 1. FFI C-ABI Data Models

### 1.1 `TTZipErrorInfo` (Structured Error Context Envelope)
```c
typedef struct TTZipErrorInfo {
    TTZipStatus status;
    int32_t error_code;
    char message[512];
    char entry_path[256];
    uint64_t offset;
} TTZipErrorInfo;
```
- **Size**: $4 + 4 + 512 + 256 + 8 = 784$ bytes.
- **Alignment**: 8-byte aligned.
- **Allocation**: 100% stack allocated in caller frame. Zero dynamic allocations.
- **Invariants**:
  - `status == TTZIP_STATUS_OK` $\implies$ `message[0] == 0` and `error_code == 0`.
  - `message` and `entry_path` are guaranteed null-terminated C strings.

---

## 2. Rust Internal Data Structures

### 2.1 `Streaming7zExtractor` (Zero-Materialization 7z Decompressor)
```rust
pub struct Streaming7zExtractor<'a> {
    stream: Fl2DStream,
    info: &'a SevenZHeaderInfo,
    chunk_in: &'a [u8],
    in_pos: usize,
    chunk_out: Vec<u8>, // Fixed 1MB ring buffer
}
```
- **State Machine Transitions**:
  ```
  [Uninitialized] ── init() ──> [StreamReady]
                                     │
                    decompress_stream() (1MB chunks)
                                     ▼
                                [Dispatching] ── (File Complete) ──> [NextFile]
                                     │
                                (Payload EOF)
                                     ▼
                                 [Finished]
  ```

### 2.2 `BoundedMpscCompressor` (Parallel ZIP Pipe)
```rust
pub struct CompressedChunk {
    pub entry_index: usize,
    pub chunk_index: usize,
    pub is_last_chunk: bool,
    pub file_offset: u64,
    pub payload: Vec<u8>,
}
```
- **Capacity**: Bounded at 16 chunks (maximum 64MB RAM in flight).

### 2.3 `LruShard` (Arena LRU with Slot Reuse)
```rust
struct LruNode {
    key: String,
    raw_size: usize,
    compressed_size: usize,
    in_ram: bool,
    ram_data: Option<Arc<[u8]>>,
    disk_path: Option<PathBuf>,
    access_time: u64,
    prev: Option<usize>,
    next: Option<usize>,
    active: bool,
}

struct LruShard {
    map: HashMap<String, usize>,
    nodes: Vec<LruNode>,
    free_indices: Vec<usize>, // Recycled slot pool
    head: Option<usize>,
    tail: Option<usize>,
    ram_bytes: usize,
}
```
- **Slot Allocation Protocol**:
  ```rust
  fn allocate_node(&mut self, new_node: LruNode) -> usize {
      if let Some(free_idx) = self.free_indices.pop() {
          self.nodes[free_idx] = new_node;
          free_idx
      } else {
          let idx = self.nodes.len();
          self.nodes.push(new_node);
          idx
      }
  }
  ```

---

## 3. Swift Framework Concurrency Models

### 3.1 `ProgressBridgeContext`
```swift
public final class ProgressBridgeContext: @unchecked Sendable {
    public let progressHandler: (@Sendable (ArchiveProgress) -> Void)?
    public let cancellationCheck: (@Sendable () -> Bool)?
    public let totalExpectedBytes: Int64
    public let startTime: CFAbsoluteTime
    private var lastEmitTime: UInt64
    private let lock: NSLock
}
```
- **Thread Safety**: Sendable, synchronized via internal lock, throttled to 60Hz.

### 3.2 `ArchiveError` (Extended Engine Diagnostic Domain)
```swift
public enum ArchiveError: Error, LocalizedError, Equatable {
    case fileNotFound
    case readFailed(code: Int32, message: String? = nil)
    case invalidFormat
    case passwordRequired
    case wrongPassword(archivePath: String)
    case cancelled
    case engineFailure(code: Int32, message: String, entryPath: String? = nil, offset: UInt64 = 0)
}
```
