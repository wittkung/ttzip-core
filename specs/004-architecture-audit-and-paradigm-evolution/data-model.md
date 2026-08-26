# Data Model: Full Architectural Audit and Paradigm Evolution

- **Feature ID**: `004-architecture-audit-and-paradigm-evolution`
- **Target Subsystems**: `ttzip-engine` (Rust Core), `CTTZipBridge` (C-ABI Layer), `TTZipCore` (Swift 6 SDK), `TTZipApp` (SwiftUI Presentation Layer)
- **Status**: `COMPLETE`

---

## 1. Rust Native Data Structures (Core Engine)

### 1.1 `VfsArena` (Struct-of-Arrays Flat Index)
```rust
pub type NodeId = u32;

/// Contiguous Arena-Allocated VFS Index.
#[derive(Debug, Clone)]
pub struct VfsArena {
    /// Contiguous UTF-8 bytes for all interned file/directory names.
    pub string_arena: Vec<u8>,
    
    /// Node hierarchy pointers (Array indices).
    pub parent_ids: Vec<NodeId>,
    pub first_child_ids: Vec<Option<NodeId>>,
    pub next_sibling_ids: Vec<Option<NodeId>>,
    
    /// String slice references into string_arena.
    pub name_offsets: Vec<(u32, u32)>, // (offset, length)
    
    /// Metadata attributes (SoA columns for SIMD/cache line packing).
    pub uncompressed_sizes: Vec<u64>,
    pub compressed_sizes: Vec<u64>,
    pub crc32s: Vec<u32>,
    pub mtime_epoch_secs: Vec<i64>,
    pub modes: Vec<u32>,
    pub flags: Vec<u8>, // Bit 0: is_directory, Bit 1: is_encrypted, Bit 2: is_symlink
    
    /// Root directory node ID (typically 0).
    pub root_id: NodeId,
    pub total_nodes: usize,
}
```

### 1.2 `ExtractionTaskDAG` (Work-Stealing Task Model)
```rust
pub struct ExtractionTask {
    pub task_id: u32,
    pub source_offset: u64,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    pub target_path: PathBuf,
    pub mode: u32,
    pub mtime: i64,
    pub crc32: u32,
    pub compression_method: u16,
    pub is_directory: bool,
    pub is_symlink: bool,
    pub symlink_target: Option<String>,
}

pub struct ExtractionTaskDAG {
    pub directories: Vec<PathBuf>,
    pub files: Vec<ExtractionTask>,
    pub total_uncompressed_bytes: u64,
    pub total_compressed_bytes: u64,
}
```

---

## 2. C-ABI Interface Data Transfer Models (`CTTZipBridge`)

### 2.1 `TTZipPackedEntryArray` (Zero-Copy Batch Entry Buffer)
```c
typedef struct {
    const uint8_t  *utf8_bytes;          // Contiguous UTF-8 bytes of all file paths
    size_t          total_bytes_len;     // Total byte length of utf8_bytes
    const uint32_t *path_offsets;        // Start offset of each path in utf8_bytes
    const uint32_t *path_lens;           // Byte length of each path
    const uint64_t *uncompressed_sizes;  // Array of uncompressed sizes in bytes
    const uint64_t *compressed_sizes;    // Array of compressed sizes in bytes
    const uint32_t *crc32s;              // Array of expected CRC32 checksums
    const int64_t  *mtimes;              // Array of modification times (epoch seconds)
    const uint32_t *modes;               // Array of POSIX file modes (e.g. 0o644, 0o755)
    const uint8_t  *flags;               // Array of bitflags (0x1: dir, 0x2: enc, 0x4: symlink)
    size_t          count;               // Total number of entries in the batch
} TTZipPackedEntryArray;
```

### 2.2 `TTZipVfsChildSliceDto` (Windowed View Query Result)
```c
typedef struct {
    uint32_t node_id;
    const char *name_utf8;
    uint32_t name_len;
    uint64_t uncompressed_size;
    uint64_t compressed_size;
    uint32_t crc32;
    int64_t  mtime_epoch_secs;
    uint32_t mode;
    bool is_directory;
    bool is_encrypted;
    bool has_children;
} TTZipVfsNodeSummary;

typedef struct {
    const TTZipVfsNodeSummary *nodes;
    size_t count;
    size_t total_in_directory;
} TTZipVfsChildSliceDto;
```

---

## 3. Swift 6 Presentation & Service Models (`TTZipApp` & `TTZipCore`)

### 3.1 Decoupled `@Observable` Sub-State Graph
```swift
@Observable
@MainActor
public final class NavigationState {
    public var activeTab: WorkspaceTab = .home
    public var currentDirectory: URL = FileManager.default.homeDirectoryForCurrentUser
    public var pathHistory: [URL] = []
    public var pathHistoryIndex: Int = 0
}

@Observable
@MainActor
public final class ArchiveExplorerState {
    public var currentArchivePath: String?
    public var activePassword: String?
    public var searchQuery: String = ""
    public var selectedNodeId: UInt32?
}

@Observable
@MainActor
public final class TaskExecutionState {
    public var isLoading: Bool = false
    public var statusMessage: String = ""
    public var progressValue: Double = 0.0
    public var currentSpeedMBs: Double = 0.0
    public var canCancelTask: Bool = false
}

@Observable
@MainActor
public final class OverlayState {
    public var showCompressModal: Bool = false
    public var showExtractModal: Bool = false
    public var showPasswordPrompt: Bool = false
    public var pendingEncryptedPath: String?
}
```

### 3.2 `EphemeralResourceBroker` Actor
```swift
public actor EphemeralResourceBroker {
    public static let shared = EphemeralResourceBroker()
    
    private let thumbnailCache: NSCache<NSString, NSImage>
    private let metadataCache: NSCache<NSString, NSDictionary>
    private var tempPreviewFiles: Set<URL>
    private var memoryPressureSource: DispatchSourceMemoryPressure?
    
    public func requestThumbnail(for path: String, targetSize: CGSize) async -> NSImage?
    public func cachePreviewData(_ data: Data, for path: String) async -> URL
    public func evictUnderPressure() async
}
```
