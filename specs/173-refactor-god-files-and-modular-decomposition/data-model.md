# Data Model: 173-refactor-god-files-and-modular-decomposition

## 1. Swift Standards & Registry Models
- **`StandardsComplianceReport`**:
  - `isCompliant: Bool`
  - `format: ArchiveCompressionFormat`
  - `violationReasons: [String]`
  - `detectedHeaderType: String`
  - `isZip64Detected: Bool`
  - `isEncryptedDetected: Bool`
- **`ArchiveFormatStandardSpec`**:
  - `format: ArchiveCompressionFormat`
  - `canonicalName: String`
  - `mimeType: String`
  - `primaryExtension: String`
  - `supportedExtensions: [String]`
  - `signatures: [ArchiveMagicSignature]`
  - `citation: StandardCitation`

## 2. Testing & Oracle Models
- **`DifferentialTestReport`**:
  - `isPassed: Bool`
  - `divergenceErrors: [String]`
  - `ttzipManifest: FileTreeManifest`
  - `oracleManifest: FileTreeManifest`
  - `durationSeconds: Double`
- **`FileTreeManifest`**:
  - `rootPath: String`
  - `entries: [String: ManifestEntry]`

## 3. Rust TUI & CLI Data Models
- **`VfsNode` / `VfsTree`**:
  - `name: String`
  - `rel_path: String`
  - `is_directory: bool`
  - `uncompressed_size: u64`
  - `compressed_size: u64`
  - `crc32: u32`
  - `children: Vec<VfsNode>`
  - `is_expanded: bool`
  - `is_selected: bool`
- **`Cli` & `Commands`**:
  - `archive: Option<PathBuf>`
  - `subcommand: Option<Commands>` (`List`, `Extract`, `Create`)
