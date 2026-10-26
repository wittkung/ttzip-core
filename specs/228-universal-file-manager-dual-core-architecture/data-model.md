# Data Model & State Machine Specifications

**Feature**: `228-universal-file-manager-dual-core-architecture`  
**Date**: 2026-09-26  
**Status**: Completed  

---

## 1. Domain Entities

### 1.1 Archive Microkernel (`products/ttzip/core`)

```rust
pub enum ArchiveFormat {
    Auto,
    Zip,
    SevenZip,
    Tar,
    Gzip,
    Bzip2,
    Xz,
    Zstd,
    Lz4,
}

pub struct ArchiveEntry {
    pub id: u64,
    pub rel_path: String,
    pub is_directory: bool,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub mtime_epoch_secs: i64,
    pub is_encrypted: bool,
    pub format: ArchiveFormat,
}
```

### 1.2 Universal Media Kit (`infra/ttmedia`)

```rust
pub enum MediaCategory {
    AudioVideo,
    Spreadsheet,
    Document,
    Ebook,
    Pdf,
    SourceCode,
    RasterImage,
}

pub struct MediaInspection {
    pub category: MediaCategory,
    pub mime_type: String,
    pub file_size: u64,
    pub title: Option<String>,
    pub dimensions: Option<(u32, u32)>,
    pub duration_seconds: Option<f64>,
    pub metadata_tags: HashMap<String, String>,
    pub supports_in_place_edit: bool,
}
```

### 1.3 Application Core (`products/ttzip/app-core`)

```rust
pub enum DeviceTransport {
    UsbMtp,
    UsbAdb,
    WirelessAdb,
}

pub enum DeviceConnectionState {
    Disconnected,
    Connecting,
    Authorizing,
    Connected,
    Transferring,
    Error(String),
}

pub struct DeviceDescriptor {
    pub uuid: String,
    pub display_name: String,
    pub transport: DeviceTransport,
    pub state: DeviceConnectionState,
    pub battery_percentage: Option<u8>,
    pub storage_partitions: Vec<StoragePartition>,
}

pub struct StoragePartition {
    pub partition_id: String,
    pub name: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub is_removable: bool,
}

pub enum TransferStatus {
    Queued,
    Active { current_rate_bps: u64, eta_seconds: u32 },
    Paused,
    Completed,
    Failed(String),
}

pub struct TransferJob {
    pub job_id: String,
    pub source_uri: String,
    pub destination_uri: String,
    pub total_bytes: u64,
    pub transferred_bytes: u64,
    pub status: TransferStatus,
    pub checkpoint_wal_path: Option<String>,
}
```

### 1.4 Plugin PKI & Security (`app-core::plugin`)

```rust
pub struct PluginCertificate {
    pub issuer_id: String,
    pub subject_id: String,
    pub public_key_ed25519: [u8; 32],
    pub valid_from_epoch: u64,
    pub valid_until_epoch: u64,
    pub allowed_capabilities: Vec<PluginCapability>,
    pub signature: [u8; 64],
}

pub enum PluginCapability {
    FsReadArchive,
    FsWriteTemp,
    NetworkFetch,
    ExecuteProcess,
}

pub struct PluginManifest {
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    pub entry_point: String,
    pub certificate_chain: Vec<PluginCertificate>,
    pub requested_capabilities: Vec<PluginCapability>,
    pub package_blake3_digest: [u8; 32],
}
```

---

## 2. State Machines

### 2.1 Device Lifecycle State Machine

```
              ┌───────────────┐
              │ Disconnected  │
              └───────┬───────┘
                      │ USB Insertion / Wi-Fi Discovery
                      ▼
              ┌───────────────┐
              │  Connecting   │
              └───────┬───────┘
                      │ MTP OpenSession / ADB Handshake
                      ▼
              ┌───────────────┐      User Reject / Timeout
              │  Authorizing  │─────────────────────────────┐
              └───────┬───────┘                             │
                      │ Trust Key Accepted                  ▼
                      ▼                             ┌───────────────┐
              ┌───────────────┐                     │     Error     │
      ┌───────┤   Connected   │◄────────────────────┤ (Unresponsive)│
      │       └───────┬───────┘     Retry Session   └───────────────┘
      │               │ Start Transfer Job                  ▲
      │               ▼                                     │
      │       ┌───────────────┐                             │
      │       │ Transferring  │─────────────────────────────┘
      │       └───────┬───────┘     I/O Staged Pipe Broken
      │               │ Queue Drain
      │               ▼
      │       ┌───────────────┐
      └──────►│ Disconnected  │ (Device Unplugged)
              └───────────────┘
```

### 2.2 Transfer Job State Machine

```
   [Enqueued] ──► [Active] ──► [Completed]
                     │  ▲
        Pause Event  │  │ Resume Event
                     ▼  │
                   [Paused]
                     │
           I/O Error │
                     ▼
                  [Failed]
```
