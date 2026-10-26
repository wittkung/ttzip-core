# R4 Design & Handoff Record: Archive Creation Capabilities, 7z Header Encryption, and 7z Metadata Preservation

**Group**: R4 (Creation Capabilities, 7z Encryption & Metadata End-to-End Repair)
**Scope**: `Q1-ISSUE-04` / `Q1-ISSUE-08` (`C3-03`, `C5-03c`), `Q1-ISSUE-05` / `Q1-ISSUE-06` (Swift & creation mapping: `C1-21`..`C1-29`, creation capabilities & fake ZIP fallback removal: `C1-30`..`C1-36`).

---

## Part 1: Four Canonical Contracts (四项核心契约)

### Contract 1: Capability Matrix Alignment Across UI, Swift, UniFFI, and Rust (`可识别 / 可读取 / 可创建 / 可加密 / 可分卷`)

Each archive/stream format exposes five orthogonal capability dimensions that agree across the macOS SwiftUI layer (`CompressIntegratedConfigSectionView`, `CompressFormSession`), the Swift SDK (`ArchiveCompressionFormat`, `ArchiveWriter`), the UniFFI boundary (`uniffi_api::archive`), and the Rust engine (`ttzip-archive`):

1. **Recognizable (`可识别`)**:
   - Magic-byte and suffix/MIME signature detection identifies the format deterministically (`Zip`, `SevenZip`, `Rar`, `Tar`, `TarGz`, `TarBz2`, `TarXz`, `TarZstd`, `TarLz4`, `TarBrotli`, `TarLzip`, `TarLrzip`, `Gzip`, `Bzip2`, `Xz`, `Zstd`, `Lz4`, `Brotli`, `Lzip`, `Lrzip`, `Snappy`, `Iso`, `Dmg`, `Wim`, `Cab`, `Rpm`, `Deb`, `Ar`, `Cpio`, `Xar`, `Aar`, `Lzh`, `Squashfs`, `Lzfse`).
   - Added `"xar"` to `ArchiveCompressionFormat.supportedArchiveExtensions` so Finder / drag-and-drop routing recognizes `.xar` archives.

2. **Readable / Extractable (`可读取`)**:
   - Supported for browsing and extraction (`inspect_archive`, `extract_archive`), including read-only container formats (`Rar`, `Dmg`, `Wim`, `Cab`, `Rpm`, `Deb`, `Lzh`, `Squashfs`, `Lzfse`).
   - Read-only formats are marked `isReadOnly == true` (`isWritable == false`) in `ArchiveCompressionFormat.swift`.

3. **Creatable (`可创建`)**:
   - **Multi-entry container formats supporting creation**:
     - `Zip` (`.zip`), `SevenZip` (`.7z`), `Tar` (`.tar`), `TarGz` (`.tar.gz` / `.tgz`), `TarBz2` (`.tar.bz2` / `.tbz2`), `TarXz` (`.tar.xz` / `.txz`), `TarZstd` (`.tar.zst` / `.tzst`), `TarLz4` (`.tar.lz4` / `.tlz4`), `TarBrotli` (`.tar.br` / `.tbr`), `TarLzip` (`.tar.lz` / `.tlz`), `TarLrzip` (`.tar.lrz` / `.tlrz`), `Iso` (`.iso`), `Cpio` (`.cpio`), `Xar` (`.xar`, pure-Rust XAR 1.0 `xar!` writer), `Ar` (`.ar` / `.a`, BSD `!<arch>\n` with `#1/<len>` extended filenames), `Aar` (`.aar` / `.yaa`, macOS native `libAppleArchive.dylib` `pbze` LZFSE-compressed Apple Archive stream).
   - **Single-file raw compression stream formats supporting creation**:
     - `Gzip` (`.gz`), `Bzip2` (`.bz2`), `Xz` (`.xz`), `Zstd` (`.zst`), `Lz4` (`.lz4`), `Brotli` (`.br`), `Lzip` (`.lz`), `Lrzip` (`.lrz`), `Snappy` (`.sz`).
   - **Explicitly non-creatable (read-only) formats**:
     - `Rar`, `Dmg`, `Cab`, `Rpm`, `Deb`, `Wim`, `Lzh`, `Squashfs`, `Lzfse`.
     - **Zero Fake-ZIP Fallback**: `format_config.rs`, `create.rs`, and `ffi/archive_ffi/create.rs` never fall back to `archive_write_set_format_zip` (`PK\x03\x04`) for unsupported formats. Calling creation with a non-creatable format returns `Err(TTZipStatus::ErrUnsupportedFeature)` before touching or overwriting any existing destination file.
     - **UI Alignment**: `ArchiveCompressionFormat.primary17WritableFormats`, `allWritableCases`, and `CompressIntegratedConfigSectionView.secondaryFormats` exclude all `isReadOnly == true` formats (`dmg`, `cab`, `rpm`, `deb`, `wim`, `rar`, `lzh`, `squashfs`, `lzfse`).
     - **Capability Ownership**: R4 owns the creation capability matrix and writer implementations (`Ar` `!<arch>\n`, `Aar` `pbze`, bounded streaming `Xar` `xar!`, `Cpio` `070701`, `Iso` `CD001`, `Zip`, `SevenZip`, `Tar*`, and single-file streams, plus explicit rejection of read-only `Dmg`, `Cab`, `Rpm`, `Deb`, `Wim`, `Rar`, `Squashfs`, `Lzh`, `Lzfse`). R2 owns the read/extract routing side, and H coordinates binding generation and final packaging.

4. **Encryptable (`可加密`)**:
   - `Zip` (`.zip`, AES-256 / ZipCrypto) and `SevenZip` (`.7z`, AES-256-CBC + SHA-256 KDF) support archive encryption (`supportsPasswordEncryption == true`).
   - Only `SevenZip` (`.7z`) supports header/filename encryption (`encryptFileNames` / `-mhe=on`).
   - All other formats disable and clear `enableEncryption` when selected in `CompressFormSession.reconcileSelectedFormatWithInputs()`.

5. **Splittable (`可分卷`)**:
   - Multi-volume output (`splitVolumeSize > 0`, `.001`, `.002`, ...) is governed by `selectedFormat.supportsSplitVolume` and enforced via `VolumeStagingGuard` / `SplitVolumeWriter` / `StreamOutputTarget`.

---

### Contract 2: 7z Filename Encryption (`encryptFileNames` / `kEncodedHeader (0x17)`) & Typed Option Entrypoints

1. **When `encryptFileNames == true` and a non-empty password is provided**:
   - The 7z writer serializes the normal `kHeader (0x01)` (containing `kMainStreamsInfo` and `kFilesInfo` with UTF-16LE filenames, empty directory/file markers, symlink attributes, POSIX permissions, and `kMTime`), compresses it with LZMA2, pads and encrypts it with AES-256-CBC (CSPRNG 8-byte salt, 16-byte IV, `num_cycles_power = 19` SHA-256 KDF), and writes a `kEncodedHeader (0x17)` bootstrap section (including `K_CRC = 0x0A` over the unpacked `kHeader`) at `next_header_offset`.
   - **Without password**: Neither raw bytes nor `inspect_archive` / `load_archive_vfs_tree` / `7z l -p""` can list any file or directory names. `inspect_archive` / `load_archive_vfs_tree` with `password = None` returns `ErrInvalidPassword` / `TTZipError::InvalidPassword`.
   - **With wrong password**: `decode_encoded_header` and `decode_encoded_header_from_reader` verify unpacked header CRC, `VALID_TOP_LEVEL_NIDS`, and map inner header parse errors on AES-encrypted boot streams to `SevenZError::BadPassword` -> `TTZipStatus::ErrInvalidPassword`.
   - **With correct password**: Both TTZip (`inspect_archive`, `load_archive_vfs_tree`, `SevenZArchive::extract_all`) and external `/opt/homebrew/bin/7z` (`7z t -p...`, `7z l -p...`) verify and extract all entries, including empty directories, Unicode filenames, and split volumes (`.7z.001`, `.7z.002`).

2. **When `encryptFileNames == false` and a non-empty password is provided (Content-Only Encryption)**:
   - File streams in `kMainStreamsInfo` are encrypted with `CODER_ID_AES256` (`06 F1 07 01`) + `CODER_ID_LZMA2` (`21`), while the outer header at `next_header_offset` is written as plaintext `kHeader (0x01)`.
   - `inspect_archive` without a password succeeds and lists entries (including empty directories, Unicode filenames, and split volumes) with `is_encrypted = true` on encrypted entries; extracting payload without a password fails, and extracting with the valid password succeeds.

3. **When `encryptFileNames == true` (or Encryption) Is Requested With an Empty or Missing Password (Fail-Closed Rejection)**:
   - Requesting `encryptFileNames == true` (`encrypt_file_names == Some(true)`) when `password` is `None` or empty (`""`), OR supplying an explicit empty password (`password == Some("")`) to archive creation, is **never** treated as a silent plaintext no-op.
   - Across UI (`CompressFormSession.startCompression()`), Swift (`ArchiveWriter.createArchiveInternal`), UniFFI (`create_archive_advanced_with_options` / `create_archive_advanced` / `create_archive_stream`), and Rust (`create_archive_explicit_with_files_detailed_opts` / `sevenz/writer.rs` / `ttzip_rust_create_archive_v2`), such requests **immediately fail** with `ArchiveError.invalidPassword` / `TTZipError::InvalidPassword` / `TTZipStatus::ErrInvalidPassword` before creating any output file, overwriting any pre-existing target file, or leaving behind any staging/split volume artifacts.
   - When the user creates an unencrypted archive in the UI (`enableEncryption == false`), `CompressFormSession.makeAdvancedOptions()` passes `encryptFileNames: false` and `password: nil`.

4. **Typed, Backward-Compatible Option Entrypoints (No `exclude_patterns` Smuggling)**:
   - `UniFFICreateArchiveConfig.exclude_patterns` is strictly a list of path exclusion patterns; literal patterns such as `"__ttzip_opt:encrypt_file_names=1"` are passed directly to `PathPatternFilter` and never intercepted as options.
   - **UniFFI Typed API**: `uniffi_api/archive.rs` defines `#[derive(uniffi::Record)] pub struct UniFFICreateArchiveExtraOptions { pub encrypt_file_names: Option<bool> }` and exports `#[uniffi::export] pub fn create_archive_advanced_with_options(config: UniFFICreateArchiveConfig, extra_options: UniFFICreateArchiveExtraOptions, progress: Option<Box<dyn ProgressHandler>>, token: Option<Arc<CancellationToken>>) -> Result<CompressionReport, TTZipError>`.
   - **C-ABI Versioned & Bridge Entrypoints**:
     - `ffi/archive_ffi/create.rs` exports `ttzip_rust_create_archive_v2(..., encrypt_file_names_mode: i32)` while keeping `ttzip_rust_create_archive` and `#[repr(C)] TTZipCreateOptions` layout unchanged.
     - `uniffi_api/archive.rs` exports `#[no_mangle] pub extern "C" fn ttzip_uniffi_stage_next_create_extra_options(encrypt_file_names_mode: i32)` (`-1` = unset/`None`, `0` = `Some(false)`, `1` = `Some(true)`), which `ArchiveWriter.swift` calls via `@_silgen_name("ttzip_uniffi_stage_next_create_extra_options")` immediately prior to `createArchiveAdvanced`. Once H runs `generate_uniffi.sh`, Swift can also call `createArchiveAdvancedWithOptions` directly.

---

### Contract 3: 7z Symlinks, Permissions, Timestamps (`C5-03c`) & Bounded XAR Creation

1. **7z Symlinks, Permissions, and Modification Timestamps**:
   - `uniffi_api/archive.rs` and `sevenz/writer.rs` check `std::fs::symlink_metadata` before `canonicalize` or `fs::metadata`, storing symlink target bytes as the stream payload, `S_IFLNK (0o120000)` + permission bits (`0755`, `0444`) in `K_WIN_ATTRIBUTES (0x15)`, `K_EMPTY_FILE (0x0F)`, and `NtTime` in `K_MTIME (0x14)`.
   - `sevenz/header/stream.rs` parses `K_MTIME (0x14)` into `SevenZFileEntry.mtime_epoch_secs`.

2. **Bounded Streaming XAR Creation (`write_xar_entries`) & Independent Tool Extraction**:
   - `write_xar_entries` never loads whole files into memory or accumulates compressed payloads in an in-memory `Vec<u8>`.
   - Each regular file is read in fixed `64 KiB` chunks, streamed through `flate2::write::ZlibEncoder` directly into a temporary heap file (`TempFileGuard` in the staging directory, automatically removed on drop/error/cancellation), while incrementally computing both `extracted-checksum` (SHA-256) and `archived-checksum` (SHA-256) and checking cancellation/progress on every chunk.
   - TOC `<file>` nodes are organized as a proper hierarchical directory tree supporting nested directories, empty directories, regular files, and symbolic links (`<link type="file">...</link>` + `<target>...</target>`), then written ahead of the streamed temporary heap file into `StreamOutputTarget`.
   - Verified via independent `/usr/bin/xar -xf` extraction checking directory hierarchy, SHA-256 file content hashes, empty directories, and symbolic links (`symlink_metadata().file_type().is_symlink()`).

---

### Contract 4: Single-File Compression Streams vs. TAR Containers (`Q1-ISSUE-05`: `C1-21`..`C1-29`)

1. **Swift Format Mapping (`ArchiveWriter.swift`)**:
   - `.gz`, `.bz2`, `.xz`, `.zst`, `.lz4`, `.brotli`, `.lzip`, `.lrzip` map to `.gzip`, `.bzip2`, `.xz`, `.zstd`, `.lz4`, `.brotli`, `.lzip`, `.lrzip` (never `.tarGz`, `.tarBz2`, etc.).
2. **Input Count & Directory Rules**:
   - Pure single-stream formats (`Gzip`, `Bzip2`, `Xz`, `Zstd`, `Lz4`, `Brotli`, `Lzip`, `Lrzip`, `Snappy`) with a non-`.tar.*` destination path accept **only 1 regular file** and emit pure raw stream magic bytes without any `ustar` TAR header.
   - Multi-file, empty, symlink, or directory inputs to pure single-stream formats are rejected with `Err(TTZipStatus::ErrInvalidParam)` before touching any pre-existing destination file.
3. **UI Format Selection & Filename Stem Rules (`CompressFormSession.swift`, `CompressIntegratedConfigSectionView+Components.swift`)**:
   - Pure single-stream formats are exposed in the format menu and disabled when `!session.isSingleRegularFileInput`.
   - Adding multiple files or a directory while a single-stream format is selected automatically upgrades `selectedFormat` to `correspondingTarCompositeFormat ?? .tarZst`.
   - For a single regular file `report.csv`, switching to a single-stream format (e.g. `.gz`) preserves the inner extension (`outputName = "report.csv"` -> `report.csv.gz`), and switching back to a container format restores `outputName = "report"` (`report.tar.gz`).

---

### Part 2: Verification & External Tool Audit

### 1. Rust & Bazel Regression Suite (`r4_archive_creation_contract_regression_tests`)
- **Cargo command**:
  `cargo test --manifest-path products/ttzip/core/rust/Cargo.toml --test r4_archive_creation_contract_regression_tests --test uniffi_checksum_gate_test -- --nocapture` -> **7 passed + 3 passed, 0 failed**.
- **Bazel command**:
  `bazel --noblock_for_lock test //products/ttzip/core/rust/ttzip-engine:r4_archive_creation_contract_regression_tests //products/ttzip/core/rust/ttzip-engine:uniffi_checksum_gate_test` -> **2/2 PASSED (`darwin-sandbox`)**.
- **7 Comprehensive Regression Tests in `r4_archive_creation_contract_regression_tests.rs`**:
  1. `test_r4_7z_encrypt_file_names_on_off_empty_dir_unicode_and_split`: Verifies `encrypt_file_names = Some(true)` (`0x17` `kEncodedHeader`) vs `Some(false)` (`0x01` `kHeader`), split volumes (`.7z.001`), Unicode filenames, empty directories, and external `/opt/homebrew/bin/7z` listing/testing; also verifies `encrypt_file_names = Some(true)` with `password = None` or `Some("")` fails with `InvalidPassword` while preserving pre-existing target and leaving zero residual split volumes.
  2. `test_r4_encryption_request_strict_validation_across_all_entrypoints`: Verifies UniFFI (`create_archive_advanced_with_options`, staged `ttzip_uniffi_stage_next_create_extra_options`), Rust unified (`create_archive_explicit_with_files_detailed_opts`), and C-ABI (`ttzip_rust_create_archive`, `ttzip_rust_create_archive_v2`) reject empty/missing passwords and non-encryptable formats (`.tar`, `.xar`, `.gz`) without overwriting pre-existing files or leaving split volumes; also verifies literal `__ttzip_opt:encrypt_file_names=1` in `exclude_patterns` is treated strictly as a literal exclusion filter.
  3. `test_r4_7z_symlink_permissions_and_mtime_preserved`: Verifies top-level and nested symlinks (`S_IFLNK = 0o120000`) are stored without dereferencing targets, `0755`/`0444` modes and `K_MTIME (0x14)` (`1704067200`) are preserved, and `/opt/homebrew/bin/7z x` extracts real symlinks (`symlink_metadata().file_type().is_symlink()`).
  4. `test_r4_single_file_streams_produce_raw_streams_and_reject_multi_file`: Verifies `.gz`, `.bz2`, `.xz`, `.zst`, `.lz4`, `.br`, `.lz`, `.lrz`, `.sz` emit raw compressed streams (zero `ustar` TAR headers, verified with `/usr/bin/gzip -dc`) and reject multi-file or directory inputs with `InvalidParam` while preserving pre-existing targets.
  5. `test_r4_containers_no_fake_zip_fallback_and_readonly_rejected`: Verifies `Ar` (`!<arch>\n`, `/usr/bin/ar -t`), `Aar` (`pbze`, `/usr/bin/aa list`), `Cpio` (`070701`), `Xar` (`xar!`), and `Iso` emit authentic container magic bytes (never `PK\x03\x04`), and read-only formats (`Dmg`, `Cab`, `Rpm`, `Deb`, `Wim`, `Rar`, `Squashfs`, `Lzh`, `Lzfse`) fail with `UnsupportedFeature` without touching pre-existing target files.
  6. `test_r4_xar_bounded_streaming_external_xar_extract_cancel_and_failure_cleanup`: Creates a multi-MB incompressible payload (`2.5 MiB` PRNG stream) + nested directory hierarchy + multiple files (`512 KiB` + `384 KiB`) + empty directory + symbolic link (`nested/rel_link -> target.txt`), extracts with `/usr/bin/xar -xf`, and verifies SHA-256 content hashes, empty directory existence, and symbolic link preservation (`fs::read_link` == `target.txt`); also verifies mid-stream cancellation (`TTZipStatus::Cancelled`) and write failure rollback preserve pre-existing destination files and leave zero `.ttzip_stage_` or `.ttzip_xar_heap_` temp files.
  7. `test_r4_q1_copied_sample_7z_and_single_stream_verification`: Copies `/tmp/ttzip-q1-acceptance-20261017/archives/sample.7z` into an isolated temp directory and verifies listing and re-compression contracts.

### 2. Magic Byte & Independent External Tool Verification
> **Policy**: External tool assertions in `r4_archive_creation_contract_regression_tests.rs` use `assert!(Path::new(tool).exists(), ...)` so missing external tools fail loudly and are never silently skipped as passing.

| Format / Scenario | Magic Bytes Verified | External Tool Verification (Executed & Passed) |
| :--- | :--- | :--- |
| `7z` (`encryptFileNames = true`, single & `.7z.001` split) | `37 7a bc af 27 1c` + `0x17` (`kEncodedHeader`) | `/opt/homebrew/bin/7z l -p""` fails (filenames hidden); `/opt/homebrew/bin/7z t -p<pwd>` passes |
| `7z` (`encryptFileNames = false`, single & `.7z.001` split) | `37 7a bc af 27 1c` + `0x01` (`kHeader`) | `/opt/homebrew/bin/7z t -p<pwd>` passes; listing without password succeeds |
| `7z` POSIX symlink + `0755`/`0444` + `mtime=1704067200` | `K_MTIME (0x14)` + `K_WIN_ATTRIBUTES (0x15)` (`S_IFLNK`) | `/opt/homebrew/bin/7z t` & `/opt/homebrew/bin/7z x` pass; extracted symlink verified via `fs::symlink_metadata` |
| `Ar` (`probe.ar` with 18-char `single_payload.bin`) | `21 3c 61 72 63 68 3e 0a` (`!<arch>\n` BSD `#1/20`) | `/usr/bin/ar -t probe.ar` lists `single_payload.bin` |
| `Aar` (`probe.aar`) | `70 62 7a 65` (`pbze` LZFSE AppleArchive) | `/usr/bin/aa list -i probe.aar` lists `single_payload.bin` |
| `Xar` (`bounded.xar` with 2.5 MiB incompressible + multi-file + empty dir + symlink) | `78 61 72 21` (`xar!` v1 SHA-256 TOC) | `/usr/bin/xar -tf` & `/usr/bin/xar -xf` extract full hierarchy; SHA-256 hashes, empty dir, and symlink (`symlink_metadata().file_type().is_symlink()`) verified |
| Single streams (`.gz`, `.bz2`, `.xz`, `.zst`, `.lz4`, `.br`, `.lz`, `.lrz`, `.sz`) | `1f 8b`, `42 5a 68`, `fd 37 7a 58 5a 00`, `28 b5 2f fd`, `04 22 4d 18`, `4c 5a 49 50`, `4c 52 5a 49`, `ff 06 00 00 73 4e 61 50 70 59` | `/usr/bin/gzip -dc alpha.txt.gz` matches raw input byte-for-byte (no `ustar` TAR header) |
| Read-only formats (`Dmg`, `Cab`, `Rpm`, `Deb`, `Wim`, `Rar`, `Squashfs`, `Lzh`, `Lzfse`) | Rejected (`ErrUnsupportedFeature`) | Pre-existing target sentinel bytes remain untouched |

### 3. Swift Core Compilation & Code Quality Gate Verification
- `bazel --noblock_for_lock build //products/ttzip/core:TTZipCore` -> **SUCCEEDED** (`TTZipCoreTypes`, `TTZipCoreCrypto`, `TTZipCoreArchive`, `TTZipCoreMedia`, `TTZipCore` all compiled cleanly).
- `python3 tools/ci/lint_loc_gate.py` & `python3 tools/ci/lint_code_quality.py` -> **0 violations across all 16 R4 files** (all files `<= 800` lines, all functions `<= 80` lines and `<= 15` cyclomatic complexity).

---

## Part 3: Modified & Created File Inventory (Line Counts & SHA-256)

All 16 files in R4's scope obey the `<= 800` line gate (`max = 798` lines) and function quality gates:

| Lines | SHA-256 | File Path |
| ---: | :--- | :--- |
| 458 | `eca97d8a94051db4ceadab5f7210d434d4a32e589d07b1e8b4056822bb0d7d08` | `products/ttzip/core/rust/crates/ttzip-archive/src/sevenz/format.rs` |
| 658 | `d817d088e1fa163ceb4742a2d1253f48fe2d853449f0a7da3532b4d78956d6d4` | `products/ttzip/core/rust/crates/ttzip-archive/src/sevenz/header/stream.rs` |
| 695 | `4e8c8553dbedf911e99dd5c15f2ee837d94f07fe892f743feb9aa20f7feefdb8` | `products/ttzip/core/rust/crates/ttzip-archive/src/sevenz/encrypted_header.rs` |
| 798 | `13af819a76f03ac8b42043aae8b66f649e94c2189804edd6d366cc5bf42bca67` | `products/ttzip/core/rust/crates/ttzip-archive/src/sevenz/writer.rs` |
| 735 | `1f1b335cd7653498b3097d2b4655e5b42b3be9ac0e0e878baaadc7194d2b44bd` | `products/ttzip/core/rust/crates/ttzip-archive/src/archive/unified/format_config.rs` |
| 785 | `88f11168fe2ea08e26715ddfce9c85243af5d6d6b1086579bc98805d52e8367a` | `products/ttzip/core/rust/crates/ttzip-archive/src/archive/unified/create.rs` |
| 772 | `1ae190d6d17e57cde22405e38ce47b1a943be85e14e6a32f0091fbc56e1c3da4` | `products/ttzip/core/rust/crates/ttzip-archive/src/archive/unified/create_staging.rs` |
| 418 | `7f6a18991d38f66eae0df0e959c05eee0d3f7166a458d7660c4cdf59c163fabe` | `products/ttzip/core/rust/ttzip-engine/src/ffi/archive_ffi/create.rs` |
| 767 | `4493034a96219d059ffe28c0854987efde91081394075601815f991c2aa75e4e` | `products/ttzip/core/rust/ttzip-engine/src/uniffi_api/archive.rs` |
| 735 | `cc035e8ccef80a0e050d4bddc28c64711c567332eeaf9aac01f0d766f34a4d6b` | `products/ttzip/core/rust/ttzip-engine/tests/r4_archive_creation_contract_regression_tests.rs` |
| 378 | `ac55fd77353f9db860636a09b2339033271461900a687a7bab3d1b4091905e1e` | `products/ttzip/core/Sources/TTZipCore/ArchiveWriter.swift` |
| 575 | `324e8cb549aa290cd6ebeff439d84bff04fb5abc74942e0cf8d97ffda1f0f7fb` | `products/ttzip/core/Sources/TTZipCore/Types/ArchiveCompressionFormat.swift` |
| 542 | `9bd1ef04d5b3f001c74a1fb9d1f1af5d547075b86d1fe83e1d38b6c458f2a9d4` | `products/ttzip/core/Sources/TTZipCore/Types/ArchiveCompressionOptions.swift` |
| 489 | `d4a538aa662a2807d81acffe601a84536ff66a5144ce5607b4149a021ae2c81d` | `products/ttzip/apple/Sources/TTZipApp/ViewModels/CompressFormSession.swift` |
| 118 | `d9f86e4e604853c86a77241f64186e07c830d97c6f6ec2a1efa66cd3b2232d97` | `products/ttzip/apple/Sources/TTZipApp/Views/Components/CompressIntegratedConfigSectionView.swift` |
| 669 | `c4fd0ca3c44792519256c1fa98d83ce7b45dc98f44b778aef8b740fa3dab8fe3` | `products/ttzip/apple/Sources/TTZipApp/Views/Components/CompressIntegratedConfigSectionView+Components.swift` |

---

## Part 4: Delivery Status Summary (四项交付状态)

### 1. 已完成 (Completed)
- **加密请求明确生效或报错（拒绝无密码 `encryptFileNames=true` 或空密码）**：
  - 废除“无密码 `encryptFileNames=true` 静默降级为明文归档”的旧设计；在 UI (`CompressFormSession.swift`)、Swift 门面 (`ArchiveWriter.swift`)、UniFFI (`uniffi_api/archive.rs`)、Rust 统一创建层 (`archive/unified/create.rs` / `sevenz/writer.rs`) 与 C-ABI (`ffi/archive_ffi/create.rs`) 全链路强制校验。
  - 当请求加密（`encryptFileNames=true`、`encryption != None` 或传入空密码 `Some("")`）而未提供非空密码，或目标格式不支持加密（非 `.zip`/`.7z`）时，在触碰目标路径前立即返回 `ErrInvalidPassword` / `ArchiveError.passwordRequired` 或 `ErrUnsupportedFeature` / `ArchiveError.unsupportedEncryptionMethod`，保证不生成产物、不覆盖既有目标文件、不残留分卷（`.001` 等）。
- **有界流式 XAR 创建与独立工具提取验证**：
  - 重构 `write_xar_entries`（`create_staging.rs` + `format_config.rs`），移除逐文件 `fs::read` 与内存 `Vec<u8>` 堆累积；采用固定 `64 KiB` 缓冲区分块读取、流式 zlib 压缩并边写边算 `extracted-checksum` / `archived-checksum`（SHA-256），写入同目录 `ScopedTempFile` 临时堆文件，支持中途取消（`TTZipStatus::Cancelled`）、失败自动清理临时文件及保护已有目标文件。
  - 构建层级化 XML `<toc>` 树（`XarTreeNode`），完整支持嵌套目录、空目录、普通文件与符号链接（`<link type="file">target</link><target>target</target>`）。
- **类型化、向后兼容的选项入口（彻底移除 `exclude_patterns` 夹带）**：
  - 移除 `ArchiveWriter.swift` 与 `uniffi_api/archive.rs` 中通过 `exclude_patterns` 传递 `__ttzip_opt:encrypt_file_names=...` 的做法，`exclude_patterns` 恢复纯字面排除过滤语义。
  - 新增类型化 UniFFI 扩展入口 `UniFFICreateArchiveExtraOptions { pub encrypt_file_names: Option<bool> }` 与 `#[uniffi::export] pub fn create_archive_advanced_with_options(...)`，同时提供保持既有 `repr(C)` 布局兼容的 C-ABI 入口 `ttzip_rust_create_archive_v2` 与 `ttzip_uniffi_stage_next_create_extra_options`，使 `ArchiveWriter.swift` 在 H 重新生成绑定前后均能通过类型化通道传递 `encryptFileNames`，且不破坏 `uniffi_checksum_gate_test.rs`。
- **7z 文件名加密、符号链接/权限/时间戳保留、单文件流与容器分离、移除伪装 ZIP 回退**：全部保持完好并通过回归测试。

### 2. 已验证 (Verified)
- **源码级专项测试 (Cargo & Bazel)**：`r4_archive_creation_contract_regression_tests`（7 项测试）与 `uniffi_checksum_gate_test`（3 项测试）在 Cargo 与 Bazel (`darwin-sandbox`) 下均 100% 通过。
- **独立外部工具提取与校验（强制存在性断言，无条件跳过）**：
  - `/usr/bin/xar -tf` 与 `/usr/bin/xar -xf` 实际提取包含 2.5 MiB 不可压缩数据、多文件、嵌套目录、空目录及符号链接的 `.xar` 归档，逐一核对目录结构、文件 SHA-256 哈希、空目录及符号链接（`symlink_metadata().file_type().is_symlink()` 且 `read_link` 目标一致）；
  - `/opt/homebrew/bin/7z` (`7z l`, `7z t`, `7z x`) 验证 7z 文件名加密隐藏、分卷、符号链接提取与权限/时间戳保留；
  - `/usr/bin/ar -t`、`/usr/bin/aa list`、`/usr/bin/gzip -dc` 验证 `Ar`、`Aar` 与单文件压缩流。
- **Swift Core 编译与代码质量门禁**：`bazel --noblock_for_lock build //products/ttzip/core:TTZipCore` 编译通过；`lint_loc_gate.py`（`<= 800` 行）与 `lint_code_quality.py`（函数 `<= 80` 行、圈复杂度 `<= 15`）在 R4 全部 16 个文件上零违规。

### 3. 尚未验证 (Not Yet Verified)
- **完整 Swift 应用模块 (`//products/ttzip/apple:TTZipAppViews` / `TTZipApp`) 连编**：`TTZipCore` 编译通过不能替代完整 Swift 应用模块连编；当前白名单外的 `TTZipAppServices` 存在其他组并行修改，完整 App 模块连编待 H 汇总各组修改后统一验证。
- **固定包 (`TTZip.app`) 真实 GUI 端到端交互验收**：待 H 统一执行全仓 `ttaction run`、打包与签名后，由 Q1 对固定包执行真实 GUI 验收。

### 4. 依赖其他组 (Dependencies on Other Groups)
- **依赖 R2（读取、浏览、提取与格式检测路由）**：
  - R4 负责创建能力与 UI 可创建格式暴露边界（`Dmg`、`Cab`、`Rpm`、`Deb`、`Wim`、`Rar`、`Squashfs`、`Lzh`、`Lzfse` 为只读格式，拒绝创建；`Ar`、`Aar`、`Cpio`、`Xar`、`Iso` 及 9 种单文件流支持真实创建）；R2 负责上述只读格式、单文件流及 `Ar`/`Aar`/`Xar` 容器在浏览与提取侧的路由配合（注：V1 为媒体解复用组，不涉及归档格式路由）。
- **依赖 H（生成绑定、全仓门禁与打包集成）**：
  - R4 已完成 `UniFFICreateArchiveExtraOptions`、`create_archive_advanced_with_options`、`ttzip_rust_create_archive_v2` 与 `ttzip_uniffi_stage_next_create_extra_options` 的完整契约与 Swift/Rust 接线；H 在集成阶段可选择运行 `generate_uniffi.sh` 刷新 `ttzip_engine.swift` 绑定并统一执行全仓 `ttaction run`、App 连编与打包签名。
。
