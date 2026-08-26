# Feature Specification: Corpus-Driven Archive Encryption Regression & Acceptance Suite

**Feature Branch**: `025-archive-encryption-corpus-regression`  
**Created**: 2026-08-15  
**Status**: Specified & Clarified  
**Input**: Comprehensive adoption and enhancement of libarchive's static encrypted corpus testing architecture, establishing an orthogonal encryption topology matrix (WinZip AES-128/256, 7z Header/Data AES-256, RAR4/RAR5 encrypted volumes, mixed partial encryption) and strict multi-tier passphrase acceptance gates in TTZip.

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Multi-Format Encrypted Corpus Decryption & Integrity (Priority: P1)
As an end user and developer, I want TTZip to accurately decrypt and verify archives across all major real-world encryption schemes (ZIP WinZip AES-128/256, ZipCrypto, 7z AES-256, RAR4 AES-128, RAR5 PBKDF2/AES-256) loaded from a dedicated static test corpus, ensuring zero checksum corruption.

**Why this priority**: Encryption correctness and data integrity across legacy and modern formats is a mission-critical core security requirement.

**Independent Test**:
Can be fully tested by executing `ArchiveEncryptionCorpusTests.swift` against `Tests/Fixtures/Encrypted/` fixture archives and verifying extracted bytes byte-for-byte against expected hash digest baselines.

**Acceptance Scenarios**:
1. **Given** a WinZip AES-256 encrypted ZIP archive with Deflate compression, **When** extracted with the correct passphrase, **Then** extraction completes successfully and payload data exactly matches uncompressed plaintext.
2. **Given** a 7z AES-256 data-encrypted archive, **When** extracted with the correct passphrase, **Then** all files are extracted without CRC/data errors.
3. **Given** a RAR5 PBKDF2/AES-256 encrypted archive, **When** extracted with the correct passphrase, **Then** all files and directories are restored with exact metadata.

---

### User Story 2 - Header Encryption & Metadata Protection Acceptance (Priority: P1)
As a security-conscious user, I want archives with encrypted file lists / headers (e.g., 7z `-mhe=on` or RAR5 encrypted filenames) to strictly hide entry names and file sizes until the correct passphrase is provided, while plain data-only encrypted archives allow directory listing without password prompt.

**Why this priority**: Distinguishing header encryption from data-only encryption prevents metadata leakage and ensures proper UI prompt sequencing.

**Independent Test**:
Can be fully tested by attempting entry traversal on header-encrypted vs data-encrypted archives without supplying a passphrase.

**Acceptance Scenarios**:
1. **Given** a 7z archive with encrypted headers, **When** `ArchiveReader` attempts to list entries without a passphrase, **Then** entry reading fails immediately with a designated `.passwordRequired` error without exposing internal file paths.
2. **Given** a 7z archive with data-only encryption (plaintext header), **When** `ArchiveReader` lists entries without a passphrase, **Then** entry names and uncompressed sizes are visible, but extracting file data without a passphrase fails with `.passwordRequired`.

---

### User Story 3 - Partially Encrypted Container & Mixed Security Boundary (Priority: P2)
As a user working with mixed archives, I want TTZip to seamlessly handle containers that contain both unencrypted files and encrypted files in the same archive, extracting plaintext files freely and selectively prompting only for encrypted files.

**Why this priority**: Real-world archives produced by legacy tools frequently mix encrypted and unencrypted entries. State machines must not latch into false-failure or false-success states.

**Independent Test**:
Can be fully tested by opening a mixed archive (e.g., `bar_unencrypted.txt` + `bar_encrypted.txt`), reading the plaintext entry without password, and verifying password requirement on the encrypted entry.

**Acceptance Scenarios**:
1. **Given** a mixed 7z / ZIP archive containing unencrypted `fileA.txt` and encrypted `fileB.txt`, **When** extracted without password, **Then** `fileA.txt` extracts cleanly while `fileB.txt` reports encryption status cleanly.
2. **Given** invalid or malformed passphrases supplied, **When** decryption fails, **Then** error handling returns structured `TTZipError.invalidPassword` without memory leakage or process crash.

---

### User Story 4 - Multi-Candidate Passphrase Fallback Pipeline (Priority: P2)
As a user leveraging the TTZip Password Vault, I want the archive engine to accept a prioritized list of candidate passphrases and automatically try them in sequence until a valid key is found or candidates are safely exhausted.

**Why this priority**: Enables seamless password manager integration and eliminates manual trial-and-error password prompts.

**Independent Test**:
Can be fully tested by passing `["wrong1", "wrong2", "correctPassword"]` to the decryption pipeline and asserting that candidate 3 successfully decrypts the payload without residual corrupted state.

**Acceptance Scenarios**:
1. **Given** a candidate passphrase list where the 3rd entry is correct, **When** decryption begins, **Then** candidate 1 and 2 fail silently and candidate 3 decrypts the stream.
2. **Given** a candidate list with no valid password, **When** all candidates fail, **Then** engine raises a final `.invalidPassword` error.

---

## Edge Cases

- **Zero-Byte Encrypted File**: Archive containing empty (0-byte) encrypted entries must not crash KDF/AES initialization.
- **Corrupted Auth Tag / MAC**: WinZip AES-256 archives with mutated HMAC auth tags must fail with checksum verification error rather than silent data corruption.
- **Special Characters & Encodings**: Passphrases with UTF-8 non-ASCII characters (e.g., CJK glyphs, emoji) must be correctly normalized across engine bindings.
- **Malformed PBKDF2 Salt / Iteration Count**: Defend against malicious iteration counts (e.g. DoS attempts with $2^{31}$ iterations).

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide an `ArchiveFixtureLoader` capable of loading base64/binary test fixtures from `Tests/Fixtures/Encrypted/` without network dependencies.
- **FR-002**: System MUST verify WinZip AES-128 and AES-256 decryption across Store and Deflate compression formats.
- **FR-003**: System MUST verify 7z AES-256 decryption across header-encrypted, data-encrypted, and partially-encrypted configurations.
- **FR-004**: System MUST verify RAR4 (AES-128) and RAR5 (AES-256 PBKDF2) encrypted archive reading and integrity verification.
- **FR-005**: System MUST accurately report 3-tier encryption status: `hasEncryptedEntries` (archive level), `isDataEncrypted` (entry level), and `isMetadataEncrypted` (header level).
- **FR-006**: System MUST support multi-passphrase fallback iteration without memory leaks or state machine lockup.
- **FR-007**: System MUST validate AES MAC / HMAC auth tags and reject tampered ciphertexts.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the encrypted test suite (`ArchiveEncryptionCorpusTests.swift`) passes across all 15+ static encrypted corpus fixture files.
- **SC-002**: Decryption throughput and verification for corpus test execution runs in under 1000 ms in aggregate in Debug mode.
- **SC-003**: Zero regressions in existing performance measure gates (`XCTestPerformanceMeasureTests` and `AllFormatsPkSuiteTests`).
- **SC-004**: Zero memory leaks or dangling state pointers detected during multi-passphrase failure stress runs.

---

## Clarifications

### Clarification Session 2026-08-15
- **Q1: Which formats and fixtures should be prioritized in the initial static corpus?**
  - *Answer*: Priority on ZIP (WinZip AES-128/256 + ZipCrypto), 7z (Header/Data/Partial AES-256), and RAR4/RAR5 encrypted fixtures derived and aligned with libarchive's gold-standard test fixtures.
- **Q2: How should binary fixtures be stored in the repo?**
  - *Answer*: Encoded as raw fixture binaries in `Tests/Fixtures/Encrypted/` and loaded dynamically via `ArchiveFixtureLoader`.
- **Q3: Does this require modifying the frozen core ZIP files?**
  - *Answer*: No. The corpus regression suite builds on the external API testing layer (`ArchiveReader`, `ArchiveExtractor`, `PasswordVault`), keeping frozen engines intact.
