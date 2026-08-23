# Phase 1 Quickstart & Validation Guide: 079-professional-grade-gap-audit

**Feature**: Comprehensive Professional Software Gap Audit & Architecture Plan
**Branch**: `079-professional-grade-gap-audit`
**Status**: Draft

---

## Validation Scenario 1: In-Place Archive File Edit & Live Synchronization

### Command
```bash
swift test --filter InPlaceArchiveEditSyncTests
```

### Expected Output
```text
Test Suite 'InPlaceArchiveEditSyncTests' passed at ...
	 Executed 5 tests, with 0 failures (0 unexpected) in 0.420 seconds
```
- Test asserts extraction into `NSTemporaryDirectory()/TTZipEdit_<UUID>/config.json`.
- Test simulates atomic safe-save (`renamex_np` / `replaceItemAtURL`) by external editor.
- Test asserts `FileWatcherEngine` captures directory event within 350ms debounce window.
- Test asserts shadow archive re-packed and swapped atomically without corrupting sibling entries.

### Failure Diagnostic
1. **`NOTE_DELETE` without directory trigger**: Check that `open(stagedDirectoryPath, O_EVTONLY)` is monitoring the parent directory, NOT just the single file descriptor.
2. **Hash mismatch after sync**: Verify that `stat.st_mtime` has settled and the file is not empty before computing updated SHA-256 and writing into archive.
3. **ZIP Central Directory corruption**: Check `ZipCentralDirectoryReader` offset alignment when stream-copying raw payload chunks.

---

## Validation Scenario 2: Quick Look HTML5 Header Preview Inspection

### Command
```bash
swift test --filter QuickLookPreviewEngineTests
```

### Expected Output
```text
Test Suite 'QuickLookPreviewEngineTests' passed at ...
	 Executed 8 tests, with 0 failures (0 unexpected) in 0.045 seconds
```
- Total header inspection + HTML5 rendering latency `<= 25ms` for multi-gigabyte `.zip`, `.7z`, and `.tar.zst` sample files.
- Returned HTML string contains UTF-8 clean table structure, dark mode media queries, and zero external network references.

### Failure Diagnostic
1. **Latency exceeds 50ms**: Check if payload bytes are being inadvertently read into memory. Ensure `ArchiveReader.inspect()` only reads format headers/central directories without uncompressing file streams.
2. **Malformed HTML entity**: Verify that entry filenames containing special characters (`<`, `>`, `&`, `"`, `'`) are passed through `escapeHTML()`.

---

## Validation Scenario 3: Cross-Platform File Sanitization & Mac Metadata Exclusion

### Command
```bash
swift test --filter CrossPlatformSanitizationTests
```

### Expected Output
```text
Test Suite 'CrossPlatformSanitizationTests' passed at ...
	 Executed 6 tests, with 0 failures (0 unexpected) in 0.180 seconds
```
- Pack test directory containing `.DS_Store`, `__MACOSX`, `._test.png`, `Thumbs.db`, `.git`, `.env` with `noMacMetadata = true`.
- Decompress resulting archive with standard system `unzip` / `tar`.
- Assert that `__MACOSX`, `.DS_Store`, and `._*` are 0% present in output directory.
- Assert that legitimate developer files (`.gitignore`, `.env`) are 100% preserved.

### Failure Diagnostic
1. **Leaked `._*` files in TAR**: Ensure `COPYFILE_DISABLE=1` is exported in the environment or `ARCHIVE_READDISK_NO_XATTR` is passed to libarchive disk reader.
2. **Missing `.env` / hidden dotfiles**: Verify that the filter engine targets `macMetadataNames` specifically, and does not blanket-reject all strings starting with `.`.

---

## Validation Scenario 4: In-Memory Archive Integrity Diagnostics (Test Archive)

### Command
```bash
swift test --filter ArchiveIntegrityCheckerTests
```

### Expected Output
```text
Test Suite 'ArchiveIntegrityCheckerTests' passed at ...
	 Executed 7 tests, with 0 failures (0 unexpected) in 0.350 seconds
```
- In-memory decoding throughput `>= 5,000 MB/s` across test archives.
- 0 bytes written to disk during verification pass.
- Artificially corrupted block in test sample correctly pinpointed with `errorType: "crc32_mismatch"`.

### Failure Diagnostic
1. **Disk I/O detected**: Check that the extractor sink is configured to `NULL` / `/dev/null` discarding stream buffer without calling `open(..., O_CREAT)`.
2. **False positive corruption on encrypted archive**: Ensure valid password was supplied, or return `overallStatus: "encrypted_missing_key"`.

---

## Validation Scenario 5: Global Operations Queue Concurrency & Dock Throttling

### Command
```bash
swift test --filter GlobalOperationsQueueTests
```

### Expected Output
```text
Test Suite 'GlobalOperationsQueueTests' passed at ...
	 Executed 8 tests, with 0 failures (0 unexpected) in 0.520 seconds
```
- Scheduling 20 concurrent operations with `maxConcurrentOperations = 3` ensures at most 3 jobs execute concurrently in parallel TaskGroups.
- Dock progress update rate is strictly clamped between 30Hz and 60Hz.
- Cancelled tasks stop downstream C decoding via cooperative cancellation flags.

### Failure Diagnostic
1. **Thread pool starvation**: Ensure worker tasks do not call blocking `DispatchSemaphore.wait()` or synchronous `sleep()`; all waits must use `Task.sleep()` or async continuations.
2. **Dock UI hitching**: Verify that `ThrottledProgressPublisher` drops intermediate progress frames when dispatch frequency exceeds 60Hz.
