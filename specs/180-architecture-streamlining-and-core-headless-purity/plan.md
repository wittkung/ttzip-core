# Implementation Plan: 180-architecture-streamlining-and-core-headless-purity

## Technical Context
- **Objective**: Complete architectural purification across TTZipCore, eliminating 4-layer onion wrappers, removing dummy mock headers in 7z, unifying Standards compliance under Rust, eliminating intermediate disk files in composite Tar streams, and enforcing headless purity.

---

## Constitution Check
- [x] **Headless Purity**: 0 UI dependencies in `TTZipCore`.
- [x] **Zero Code Duplication**: Standards and format inspection unified under Rust `standards::ffi`.
- [x] **SRP & LOC Budget**: All modified files maintained at $< 350\sim 500\text{ LOC}$.
- [x] **Zero Cloud Actions Quota**: 100% local validation.

---

## Phase 0: Research Items
- R001 [SUBAGENT:research] 《7z 引擎门面洋葱层清理与 HeaderReader 伪代码消除》: Completed.
- R002 [SUBAGENT:research] 《标准合规性与魔数嗅探全面下沉至 Rust》: Completed.
- R003 [SUBAGENT:research] 《TTZipCore 无头纯化与 FileClipboardStore 架构重构》: Completed.
- R004 [SUBAGENT:research] 《复合 Tar/Brotli/Zstd 管道流去中间临时文件》: Completed.

---

## Phase 1: Component Change List

### 1. 7z Engine & Header Reader
- **`Sources/TTZipCore/SevenZip/NativeSevenZipEngine.swift`**: Directly invoke `SevenZipCAdapter` / `ttzip_rust_create_archive` / `ttzip_rust_extract_archive`.
- **`Sources/TTZipCore/SevenZip/SevenZipHeaderReader.swift`**: Call `ttzip_rust_scan_entries` to obtain authentic 7z entry descriptors.
- **`Sources/TTZipCore/SevenZip/SevenZipParallelExtractor.swift` & `SevenZipParallelWriter.swift`**: Streamlined.

### 2. Standards & Magic Signature Delegation
- **`Sources/TTZipCore/Standards/ArchiveMagicSignatureScanner.swift`**: Delegate to `ttzip_rust_detect_format_buffer` / `ttzip_rust_detect_format_file`.
- **`Sources/TTZipCore/Standards/StandardsComplianceChecker.swift`**: Delegate to `ttzip_rust_check_compliance_file` / `ttzip_rust_check_compliance_buffer`.

### 3. Composite Tar Streaming & Headless Purification
- **`Sources/TTZipCore/Brotli/NativeBrotliEngine.swift`**: Stream directly through Rust memory pipes without writing intermediate `.tar` disk files.
- **`Sources/TTZipCore/Services/FileClipboardStore.swift`**: Move to `Sources/TTZipApp/Services/FileClipboardStore.swift`.

---

## Phase 2: Verification Plan
1. `cargo test --workspace` on all Rust crates.
2. `./scripts/build_rust.sh --release && ./scripts/build_tui.sh`.
3. `swift test` ensuring all 880+ tests pass with 0 failures and 0 warnings.
4. `./scripts/run_local_ci_gate.sh` full 7-stage CI validation.
