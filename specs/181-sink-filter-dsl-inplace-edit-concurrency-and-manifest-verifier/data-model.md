# Data Model: 181-sink-filter-dsl-inplace-edit-concurrency-and-manifest-verifier

## 1. Filter DSL AST Models (`rust/ttzip-glue/src/fs/filter_dsl.rs`)
- **`FilterAstNode`**:
  - `NamePattern(Glob)`
  - `SizeComparison(Ordering, u64)`
  - `DateComparison(Ordering, i64)`
  - `IsDirectory(bool)`
  - `And(Box<FilterAstNode>, Box<FilterAstNode>)`
  - `Or(Box<FilterAstNode>, Box<FilterAstNode>)`
  - `Not(Box<FilterAstNode>)`

## 2. In-Place Edit Models (`rust/ttzip-glue/src/archive/in_place_edit.rs`)
- **`InPlaceEditAction`**:
  - `Append { local_file_path: PathBuf, archive_entry_name: String }`
  - `Replace { local_file_path: PathBuf, archive_entry_name: String }`
  - `Delete { archive_entry_name: String }`

## 3. Differential Manifest Models (`rust/ttzip-glue/src/testing/differential.rs`)
- **`ManifestEntry`**:
  - `rel_path: String`
  - `size: u64`
  - `sha256: [u8; 32]`
  - `mode: u32`
  - `mtime: i64`
