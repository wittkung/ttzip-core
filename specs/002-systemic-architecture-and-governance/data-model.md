# Data Model: TTZip Systemic Architecture & Engineering Governance

- **Feature ID**: `002-systemic-architecture-and-governance`
- **Pipeline Mode**: `[Full SDD]`
- **Status**: `RESOLVED`

---

## 1. C-ABI Structured Error Entity (`TTZipErrorInfo`)

```c
typedef struct {
    int32_t code;                  /* TTZipStatus enum code */
    uint32_t domain;               /* 1: Archive, 2: Crypto, 3: VFS, 4: FFI */
    char message[512];             /* Null-terminated human readable UTF-8 message */
    char source_file[128];         /* Source file identifier where error occurred */
    uint32_t line_number;          /* Line number where error was recorded */
} TTZipErrorInfo;
```

### Invariants:
- `code` is strictly negative for error conditions (`< 0`) and zero (`TTZIP_STATUS_OK`) on success.
- `message` is guaranteed null-terminated within the 512-byte buffer.
- `domain` maps deterministically to Swift high-level error categories (`ArchiveError`, `CryptoError`, `VFSError`, `BridgeError`).

---

## 2. Zero-Fragmentation Packed String Array (`TTZipPackedStringArray`)

```c
typedef struct {
    uint32_t count;                /* Total number of strings */
    uint32_t total_payload_bytes;  /* Total size in bytes of packed string data */
    const uint32_t *offsets;       /* Byte offset of each string in payload */
    const uint32_t *lengths;       /* Byte length of each string (excluding null) */
    const char *payload;           /* Contiguous UTF-8 bytes */
} TTZipPackedStringArray;
```

### Memory Layout:
```
+-------------------------------------------------------------------------------+
| count (4B) | total_bytes (4B) | offsets[0..count-1] | lengths[0..count-1]     |
+-------------------------------------------------------------------------------+
| payload: "str0\0str1\0str2\0...strN\0"                                        |
+-------------------------------------------------------------------------------+
```

---

## 3. Reference-Counted Cancellation Handle Entity (`CancellationToken`)

### Rust Model:
```rust
pub struct CancellationToken {
    is_cancelled: AtomicBool,
    ref_count: AtomicUsize,
}
```

### Lifecycle State Transitions:
```mermaid
stateDiagram-v2
    [*] --> Allocated: Swift Task Execution Starts
    Allocated --> Retained: ttzip_rust_cancellation_token_retain()
    Retained --> Polled: Rust worker checks is_cancelled()
    Polled --> Cancelled: Swift calls cancel() -> store(true)
    Polled --> Released: ttzip_rust_cancellation_token_release()
    Cancelled --> Released: ttzip_rust_cancellation_token_release()
    Released --> [*]: ref_count == 0 -> Box::from_raw() deallocates
```

---

## 4. Two-Phase Shard Eviction Model (`VFSLz4CachePool`)

### Eviction Data Structures:
```rust
pub struct EvictionTask {
    pub spill_path: PathBuf,
    pub payload_bytes: Vec<u8>,
}

pub struct ShardEvictionPlan {
    pub freed_ram_bytes: usize,
    pub tasks: Vec<EvictionTask>,
}
```
