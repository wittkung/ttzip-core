# Quickstart Validation Guide: Corpus-Driven Archive Encryption Regression & Acceptance Suite

## Scenario 1: Run Full Encrypted Corpus Test Suite

Verify all static encrypted archive fixtures across ZIP (WinZip AES-128/256), 7z (Header/Data AES-256), and RAR4/RAR5 pass decryption and SHA-256 digest checks.

- **Command**:
  ```bash
  swift test --filter ArchiveEncryptionCorpusTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'ArchiveEncryptionCorpusTests' passed at ...
  Executed 15 tests, with 0 failures (0 unexpected) in 0.420 seconds
  ```
- **Failure Diagnostic**:
  - If a test fails with `missingResource`, ensure `Package.swift` has `resources: [.copy("Fixtures")]` inside `TTZipTests` target.
  - If a test fails with `checksumMismatch`, check if the decrypted buffer was sliced incorrectly or if AES-CTR counter initialization skipped the 16-byte alignment.

---

## Scenario 2: 3-Tier Encryption Introspection & Header Defense Verification

Verify that header-encrypted archives (7z `-mhe=on`, RAR5 encrypted filenames) are blocked from entry listing without a password, while data-only encrypted archives allow directory traversal.

- **Command**:
  ```bash
  swift test --filter ArchiveEncryptionIntrospectionTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'ArchiveEncryptionIntrospectionTests' passed at ...
  Executed 8 tests, with 0 failures (0 unexpected) in 0.180 seconds
  ```
- **Failure Diagnostic**:
  - If header-encrypted archives return an empty entry list instead of throwing `passwordRequired(tier: .headerAndData)`, verify `ArchiveReader.swift` checks `isMetadataEncrypted` before proceeding to parse entry blocks.
  - If data-only archives throw `passwordRequired` on inspection, verify `ArchiveReader.swift` only probes entry headers without calling data decompressors.

---

## Scenario 3: Multi-Candidate Passphrase Fallback Pipeline

Verify that providing a candidate list `["wrong1", "wrong2", "correct"]` successfully decrypts the payload without dangling state or memory leaks.

- **Command**:
  ```bash
  swift test --filter ArchivePassphraseFallbackTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'ArchivePassphraseFallbackTests' passed at ...
  Executed 6 tests, with 0 failures (0 unexpected) in 0.110 seconds
  ```
- **Failure Diagnostic**:
  - If fallback stalls or returns `wrongPassword` despite correct candidate presence, check if the decryption stream context was not reset between candidate trials in `ArchiveExtractor.swift`.
  - If memory leaks occur, ensure `CUnsafeBufferAdapter` and KDF context structs are freed on each trial loop iteration.

---

## Scenario 4: Global Performance & Regression Hard Gates

Verify that adding the corpus testing suite introduces zero regressions to TTZip's existing 262-dimension peak throughput gates.

- **Command**:
  ```bash
  swift test --filter XCTestPerformanceMeasureTests
  ```
- **Expected Output**:
  ```text
  Test Suite 'XCTestPerformanceMeasureTests' passed at ...
  Executed 12 tests, with 0 failures (0 unexpected)
  ```
- **Failure Diagnostic**:
  - If throughput falls below 1800 MB/s for AES decryption, inspect whether dynamic object instantiations (e.g. `ArchiveEntry` heap allocations) were inadvertently placed inside inner compression/decompression hot loops.
