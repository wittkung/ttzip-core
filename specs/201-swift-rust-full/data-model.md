# Data Model: 201-swift-to-rust-full-architecture-sinking

## 1. CLI Commands & DTOs (`rust/ttzip-tui/src/cli/args.rs`)

### `Commands` (Enum)
- `List { archive: PathBuf, password: Option<String>, json: bool }`
- `Extract { archive: PathBuf, output: Option<PathBuf>, password: Option<String>, threads: u32, verbose: bool }`
- `Create { archive: PathBuf, sources: Vec<PathBuf>, format: Option<String>, level: u8, password: Option<String>, threads: u32, volume_size: Option<String> }`
- `Cat { archive: PathBuf, file_path: String, password: Option<String> }`
- `Check { archive: PathBuf, password: Option<String>, json: bool }`
- `Comment { archive: PathBuf, comment: Option<String>, json: bool }`
- `Convert { source_archive: PathBuf, destination_archive: PathBuf, target_format: String, level: u8 }`
- `Delete { archive: PathBuf, file_paths: Vec<String>, json: bool }`
- `Diff { archive_a: PathBuf, archive_b: PathBuf, json: bool }`
- `Hash { archive: PathBuf, algorithm: Option<String>, json: bool }`
- `Info { archive: PathBuf, json: bool }`
- `Lock { archive: PathBuf, json: bool }`
- `Tree { archive: PathBuf, max_depth: Option<usize>, json: bool }`
- `Update { archive: PathBuf, sources: Vec<PathBuf>, json: bool }`
- `Recover { archive: PathBuf, dictionary: PathBuf, threads: Option<u32>, json: bool }`
- `Repair { damaged_archive: PathBuf, output: PathBuf, format: Option<String>, json: bool }`
- `Split { source_archive: PathBuf, volume_size: String, output_dir: Option<PathBuf>, naming: Option<String> }`
- `Join { first_volume: PathBuf, output: PathBuf, json: bool }`
- `Bench { mips: bool, pareto: bool, threads: u32, dict_mb: u32, iterations: u32 }`
- `Doctor { json: bool }`

## 2. Shared C-ABI Contracts (`rust/ttzip-glue/src/ffi/`)

### `TTZipEntryMetadata` (C Struct)
- `name: *const c_char`
- `uncompressed_size: u64`
- `compressed_size: u64`
- `crc32: u32`
- `mtime: i64`
- `is_directory: bool`
- `is_encrypted: bool`
- `compression_method: u16`

### `TTZipIntegrityReport` (C Struct)
- `is_valid: bool`
- `format_name: *const c_char`
- `total_entries: usize`
- `corrupted_entries_count: usize`
- `error_message: *const c_char`
