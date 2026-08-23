# Data Model: Full Architecture Sinking & Swift-Rust Boundary Execution

## 1. CLI Commands & Parameter Data Structures (`rust/ttzip-tui/src/cli/args.rs`)

### `CliCommand` (Enum)
- `Create`: `{ archive: PathBuf, sources: Vec<PathBuf>, format: Option<String>, level: u8, password: Option<String>, threads: u32, volume_size: Option<String>, json: bool }`
- `Extract`: `{ archive: PathBuf, output: Option<PathBuf>, password: Option<String>, threads: u32, verbose: bool, json: bool }`
- `List`: `{ archive: PathBuf, password: Option<String>, recursive: bool, json: bool }`
- `Info`: `{ archive: PathBuf, json: bool }`
- `Check`: `{ archive: PathBuf, password: Option<String>, json: bool }`
- `Hash`: `{ archive: PathBuf, algorithm: Option<String>, json: bool }`
- `Diff`: `{ archive_a: PathBuf, archive_b: PathBuf, json: bool }`
- `Tree`: `{ archive: PathBuf, max_depth: Option<usize>, json: bool }`
- `Split`: `{ source_archive: PathBuf, volume_size: String, output_dir: Option<PathBuf>, naming: Option<String>, json: bool }`
- `Join`: `{ first_volume: PathBuf, output: PathBuf, json: bool }`
- `Repair`: `{ damaged_archive: PathBuf, output: PathBuf, format: Option<String>, json: bool }`
- `Recover`: `{ archive: PathBuf, dictionary: PathBuf, threads: Option<u32>, json: bool }`
- `Bench`: `{ mips: bool, pareto: bool, threads: u32, dict_mb: u32, iterations: u32, json: bool }`
- `Doctor`: `{ json: bool }`
- `Cat`: `{ archive: PathBuf, file_path: String, password: Option<String> }`
- `Comment`: `{ archive: PathBuf, comment: Option<String>, json: bool }`
- `Convert`: `{ source_archive: PathBuf, destination_archive: PathBuf, target_format: String, level: u8, json: bool }`
- `Delete`: `{ archive: PathBuf, file_paths: Vec<String>, json: bool }`
- `Lock`: `{ archive: PathBuf, json: bool }`
- `Update`: `{ archive: PathBuf, sources: Vec<PathBuf>, json: bool }`

---

## 2. Core C-ABI Interface Structures (`rust/ttzip-glue/src/ffi/` & `contracts/rust_swift_c_abi.h`)

### `TTZipEntryMetadata` (repr(C))
- `name`: `*const c_char` (UTF-8 path)
- `uncompressed_size`: `u64`
- `compressed_size`: `u64`
- `crc32`: `u32`
- `mtime`: `i64` (epoch seconds)
- `is_directory`: `bool`
- `is_encrypted`: `bool`
- `compression_method`: `u16`

### `TTZipIntegrityReport` (repr(C))
- `is_valid`: `bool`
- `format_name`: `*const c_char`
- `total_entries`: `usize`
- `corrupted_entries_count`: `usize`
- `error_message`: `*const c_char`

### `TTZipProgressUpdate` (repr(C))
- `processed_bytes`: `u64`
- `total_bytes`: `u64`
- `processed_files`: `u64`
- `total_files`: `u64`
- `throughput_mb_s`: `f64`
- `percent_complete`: `f32`

---

## 3. VFS & In-Memory Representation Models (`rust/ttzip-glue/src/vfs/`)

### `VFSNode`
- `id`: `u64`
- `name`: `String`
- `parent_id`: `Option<u64>`
- `is_dir`: `bool`
- `size`: `u64`
- `mtime`: `i64`
- `children`: `Vec<u64>`

### `VFSCacheBlock`
- `block_index`: `usize`
- `compressed_data`: `Vec<u8>` (LZ4 block)
- `uncompressed_len`: `usize`
- `access_timestamp`: `u64`
