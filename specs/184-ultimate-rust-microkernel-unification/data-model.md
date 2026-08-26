# Data Model: 184-ultimate-rust-microkernel-unification

## 1. Unified Archive Request Models (`rust/ttzip-glue/src/archive/unified.rs`)
- **`UnifiedArchiveCreateRequest`**:
  - `output_path: PathBuf`
  - `format: ArchiveFormat`
  - `level: CompressionLevel`
  - `input_paths: Vec<PathBuf>`
  - `password: Option<String>`
  - `split_volume_size_bytes: Option<u64>`
  - `skip_mac_junk: bool`

- **`UnifiedArchiveExtractRequest`**:
  - `archive_path: PathBuf`
  - `destination_dir: PathBuf`
  - `password: Option<String>`
  - `selected_entries: Option<Vec<String>>`

## 2. Unified VFS Node Models (`rust/ttzip-glue/src/fs/vfs.rs`)
- **`VfsNode`**:
  - `name: String`
  - `is_directory: bool`
  - `byte_size: u64`
  - `compressed_size: u64`
  - `crc32: u32`
  - `mtime: i64`
  - `children: Vec<VfsNode>`
