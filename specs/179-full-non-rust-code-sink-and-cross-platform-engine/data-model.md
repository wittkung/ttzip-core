# Data Model: 179-full-non-rust-code-sink-and-cross-platform-engine

## 1. Path Sanitization Models (`rust/ttzip-glue/src/security/path_sanitizer.rs`)
- **`PathSanitizationResult`**:
  - `original_path: String`
  - `normalized_path: String`
  - `has_traversal_attack: bool`
  - `is_absolute: bool`
  - `is_unc: bool`
  - `is_long_path: bool`
  - `is_windows_reserved: bool`
  - `stripped_ads: Option<String>`
  - `win32_formatted_path: String`

## 2. Charset Detection Models (`rust/ttzip-glue/src/charset/`)
- **`CharsetDetectionResult`**:
  - `encoding_name: &'static str`
  - `confidence_score: f32`
  - `is_lossless: bool`

## 3. Parallel Directory Scanner Models (`rust/ttzip-glue/src/fs/scanner.rs`)
- **`ScannedEntry`**:
  - `src_path: PathBuf`
  - `rel_path: String`
  - `is_directory: bool`
  - `is_symlink: bool`
  - `symlink_target: Option<String>`
  - `file_size: u64`
  - `mtime_secs: i64`
  - `mode: u32`
  - `dev: u64`
  - `inode: u64`
- **`ScannerConfig`**:
  - `include_hidden: bool`
  - `skip_mac_junk: bool`
  - `follow_symlinks: bool`
  - `max_depth: usize`

## 4. Platform CPU & Memory Models (`rust/ttzip-glue/src/platform/`)
- **`CpuCapabilities`**:
  - `logical_cores: usize`
  - `p_cores: usize`
  - `e_cores: usize`
  - `page_size: usize`
  - `has_neon: bool`
  - `has_arm_crypto: bool`
  - `has_aes_ni: bool`
  - `has_avx2: bool`
  - `has_hardware_crc32: bool`
  - `has_pmull: bool`
  - `has_sha_ext: bool`
- **`ProcessMemorySnapshot`**:
  - `current_rss_bytes: u64`
  - `peak_rss_bytes: u64`
  - `virtual_size_bytes: u64`
