# Tasks: Corpus-Driven Archive Encryption Regression & Acceptance Suite

**Feature Branch**: `025-archive-encryption-corpus-regression`  
**Status**: Ready for Implementation  

---

## Dependencies & User Story Flow

```
[Phase 1: Setup (T001-T002)] ➔ [Phase 2: Foundational (T003-T005)]
                                      │
        ┌─────────────────────────────┼─────────────────────────────┐
        ▼                             ▼                             ▼
[US1: Corpus Decryption]   [US2: Header Encryption]   [US3: Partially Encrypted]
     (T006-T009)                   (T010-T012)                 (T013-T015)
        │                             │                             │
        └─────────────────────────────┼─────────────────────────────┘
                                      ▼
                        [US4: Passphrase Fallback] (T016-T018)
                                      ▼
                        [Phase 7: Final Polish] (T019-T021)
```

---

## Phase 1: Setup Tasks

- [x] T001 [P] Configure SPM resource copying `.copy("Fixtures")` in `Package.swift`
- [x] T002 [P] Implement `TestFixtureLoader.swift` for `Bundle.module` binary extraction in `Tests/TTZipTests/TestFixtureLoader.swift`

---

## Phase 2: Foundational Tasks

- [x] T003 [P] Deploy static encrypted test corpus fixtures into `Tests/TTZipTests/Fixtures/Encrypted/`
- [x] T004 [P] Extend `ArchiveEntry.swift` with `isEncrypted`, `isDataEncrypted`, `isMetadataEncrypted`, and `encryptionMethod` in `Sources/TTZipCore/ArchiveEntry.swift`
- [x] T005 [P] Update `ArchiveReading` protocol and `ArchiveError` enum with 3-tier encryption variants in `Sources/TTZipCore/ArchiveReader.swift`

---

## Phase 3: User Story 1 — Multi-Format Encrypted Corpus Decryption & Integrity (Priority: P1)

**Story Goal**: Decrypt and verify WinZip AES-128/256, ZipCrypto, 7z AES-256, and RAR4/RAR5 encrypted archives against SHA-256 ground truth.

- [x] T006 [US1] Implement WinZip AES-128 and AES-256 Store/Deflate corpus matrix test in `Tests/TTZipTests/ArchiveEncryptionCorpusTests.swift`
- [x] T007 [US1] Implement 7z AES-256 data-encrypted decompression and CRC verification test in `Tests/TTZipTests/ArchiveEncryptionCorpusTests.swift`
- [x] T008 [US1] Implement RAR4 and RAR5 PBKDF2/AES-256 encrypted fixture decryption test in `Tests/TTZipTests/ArchiveEncryptionCorpusTests.swift`
- [x] T009 [US1] Implement legacy ZipCrypto decryption and integrity test in `Tests/TTZipTests/ArchiveEncryptionCorpusTests.swift`

---

## Phase 4: User Story 2 — Header Encryption & Metadata Protection Acceptance (Priority: P1)

**Story Goal**: Prevent metadata leakage by asserting that header-encrypted archives require a password before entry listing, while data-only archives allow directory traversal.

- [x] T010 [US2] Bridge C level `archive_entry_is_data_encrypted` and `archive_entry_is_metadata_encrypted` in `Sources/CTTZipBridge/CTTZipBridge_Archive.c`
- [x] T011 [US2] Implement zero-cost `probeEncryption(archivePath:)` in `Sources/TTZipCore/ArchiveReader.swift`
- [x] T012 [US2] Implement 3-tier introspection test suite in `Tests/TTZipTests/ArchiveEncryptionIntrospectionTests.swift`

---

## Phase 5: User Story 3 — Partially Encrypted Container & Mixed Security Boundary (Priority: P2)

**Story Goal**: Seamlessly handle mixed archives with both plaintext and encrypted entries, ensuring zero false-positive errors and memory-safe wrong-password handling.

- [x] T013 [US3] Implement mixed encrypted/unencrypted fixture verification test in `Tests/TTZipTests/ArchiveEncryptionCorpusTests.swift`
- [x] T014 [US3] Implement wrong-password PVV & PSWCHECK non-destructive rejection test in `Tests/TTZipTests/ArchiveEncryptionCorpusTests.swift`
- [x] T015 [US3] Implement corrupted HMAC-SHA1 auth tag rejection and error dispatch test in `Tests/TTZipTests/ArchiveEncryptionCorpusTests.swift`

---

## Phase 6: User Story 4 — Multi-Candidate Passphrase Fallback Pipeline (Priority: P2)

**Story Goal**: Accept prioritized lists of candidate passphrases and automatically try them in sequence until a valid key is found or candidates are safely exhausted.

- [x] T016 [US4] Implement multi-passphrase iteration in `ArchiveExtractor.swift` in `Sources/TTZipCore/ArchiveExtractor.swift`
- [x] T017 [US4] Implement multi-passphrase fallback automated test suite in `Tests/TTZipTests/ArchivePassphraseFallbackTests.swift`
- [x] T018 [US4] Implement PasswordVault v4 candidate list integration test in `Tests/TTZipTests/ArchivePassphraseFallbackTests.swift`

---

## Phase 7: Polish & Cross-Cutting Performance Verification

- [x] T019 [P] Execute full suite `ArchiveEncryptionCorpusTests` and assert aggregate execution time < 1000 ms
- [x] T020 [P] Execute `XCTestPerformanceMeasureTests` to enforce 262-dimension peak throughput zero regression
- [x] T021 [P] Run `./scripts/run_all_tests.sh` to confirm full regression green light across all 87+ test suites
