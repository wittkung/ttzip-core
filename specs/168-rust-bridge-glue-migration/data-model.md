# Data Model: TTZip 核心胶水层 Rust 迁移 (Feature 168)

**Feature ID**: `168-rust-bridge-glue-migration`  
**Created**: 2026-08-21  
**Status**: Completed  
**Artifact**: Phase 1 Data Models & Type Specifications

---

## 1. 核心实体与类型系统 (Core Entities)

本数据模型定义了 Rust 胶水层（`ttzip-glue`）、C-ABI 边界（`ttzip_rust_glue.h`）以及 Swift 6 消费层（`TTZipCore`）之间的强类型映射，所有类型禁止裸通配符，具备 100% 确定性内存对齐与生命周期语义。

---

### 1.1 基础枚举与错误模型 (Enums & Error Codes)

```rust
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TTZipStatus {
    Ok = 0,
    Eof = 1,
    Cancelled = 2,
    ErrInvalidParam = -1,
    ErrFileNotFound = -2,
    ErrMmapFailed = -3,
    ErrCorruptHeader = -4,
    ErrInvalidOffset = -5,
    ErrArchiveInitFailed = -6,
    ErrOpenFailed = -7,
    ErrPathTooLong = -8,
    ErrOutOfMemory = -9,
    ErrInvalidPassword = -10,
    ErrExtractionFailed = -11,
    ErrCompressionFailed = -12,
    ErrSecurityViolation = -30,
    ErrPanicCaught = -99,
}
```

```rust
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TTZipArchiveFormat {
    Auto = 0,
    Zip = 1,
    SevenZip = 2,
    Tar = 3,
    TarGz = 4,
    TarBz2 = 5,
    TarXz = 6,
    TarZstd = 7,
    Dmg = 8,
    Lzfse = 9,
    Snappy = 10,
    Unknown = 99,
}
```

```rust
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TTZipCompressionLevel {
    Store = 0,
    Fastest = 1,
    Fast = 3,
    Normal = 6,
    Maximum = 9,
    Ultra = 12,
}
```

```rust
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TTZipEncryptionMethod {
    None = 0,
    ZipCrypto = 1,
    Aes128 = 2,
    Aes192 = 3,
    Aes256 = 4,
}
```

```rust
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TTZipLogLevel {
    Debug = 0,
    Info = 1,
    Warning = 2,
    Error = 3,
}
```

---

### 1.2 归档元数据条目 (Archive Entry Model)

```rust
#[repr(C)]
pub struct TTZipEntryMetadata {
    pub path: *const libc::c_char,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub mtime_epoch_secs: i64,
    pub mode: u32,
    pub is_directory: bool,
    pub is_encrypted: bool,
    pub compression_method: u16,
}
```

---

### 1.3 操作选项与配置 (Operation Options)

```rust
#[repr(C)]
pub struct TTZipExtractOptions {
    pub destination_path: *const libc::c_char,
    pub password: *const libc::c_char,
    pub thread_budget: u32,
    pub overwrite_existing: bool,
    pub preserve_permissions: bool,
    pub dry_run: bool,
    pub progress_callback: Option<unsafe extern "C" fn(processed_bytes: u64, total_bytes: u64, current_entry: *const libc::c_char, user_data: *mut libc::c_void) -> bool>,
    pub user_data: *mut libc::c_void,
}
```

```rust
#[repr(C)]
pub struct TTZipCreateOptions {
    pub format: TTZipArchiveFormat,
    pub level: TTZipCompressionLevel,
    pub encryption: TTZipEncryptionMethod,
    pub password: *const libc::c_char,
    pub thread_budget: u32,
    pub solid_block_size_mb: u32,
    pub progress_callback: Option<unsafe extern "C" fn(processed_bytes: u64, total_bytes: u64, current_entry: *const libc::c_char, user_data: *mut libc::c_void) -> bool>,
    pub user_data: *mut libc::c_void,
}
```

---

### 1.4 流式微缓冲上下文句柄 (Stream Pipeline Contexts)

```rust
/// 不透明句柄，生命周期由 Rust 端 RAII 接管
pub struct TTZipStreamReaderContext {
    pub(crate) inner: Box<dyn std::io::Read + Send>,
    pub(crate) buffer: Box<[u8]>,
    pub(crate) total_consumed: u64,
    pub(crate) is_eof: bool,
}

pub struct TTZipStreamWriterContext {
    pub(crate) inner: Box<dyn std::io::Write + Send>,
    pub(crate) buffer: Box<[u8]>,
    pub(crate) total_written: u64,
}
```

---

### 1.5 硬件加密与校验上下文 (Crypto & Checksum Contexts)

```rust
#[repr(C)]
pub struct TTZipAes256Context {
    pub key: [u8; 32],
    pub iv_or_counter: [u8; 16],
    pub round_keys_enc: [u8; 240], // 15 rounds * 16 bytes
    pub round_keys_dec: [u8; 240],
}
```

---

## 2. 字段与契约双向一致性核对表 (Field Consistency Matrix)

| 数据模型字段 | 对应 C-ABI 字段 | 对应 JSON Schema 属性 (`contracts/*.json`) | 类型约束 | 必填性 |
| :--- | :--- | :--- | :--- | :--- |
| `TTZipStatus` | `int32_t status` | `status: string / integer` | Enum: `Ok(0)`, `Eof(1)`, ... | 必填 |
| `TTZipArchiveFormat` | `uint32_t format` | `format: string` | Enum: `"zip"`, `"7z"`, `"tar"`, ... | 必填 |
| `TTZipCompressionLevel`| `uint32_t level` | `level: integer` | Enum: `0, 1, 3, 6, 9, 12` | 必填 |
| `TTZipExtractOptions.password` | `const char*` | `password: string / null` | UTF-8 String (可空) | 选填 |
| `TTZipExtractOptions.thread_budget` | `uint32_t` | `thread_budget: integer` | Minimum: `1`, Maximum: `256` | 必填 |
| `TTZipEntryMetadata.path` | `const char*` | `path: string` | Non-empty UTF-8 String | 必填 |
| `TTZipEntryMetadata.uncompressed_size` | `uint64_t` | `uncompressed_size: integer` | Minimum: `0` | 必填 |
| `TTZipEntryMetadata.crc32` | `uint32_t` | `crc32: integer` | Unsigned 32-bit Integer | 必填 |
