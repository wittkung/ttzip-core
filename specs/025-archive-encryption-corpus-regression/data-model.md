# Phase 1 Data Model: Corpus-Driven Archive Encryption Regression & Acceptance Suite

## 1. Core Enumerations

### `ArchiveEncryptionTier`
Represents the structural classification of archive-level encryption.

| Case | Raw String Value | Description |
| :--- | :--- | :--- |
| `none` | `"NONE"` | Tier 0: No encryption detected. Full read/write without credentials. |
| `dataOnly` | `"DATA_ONLY"` | Tier 1: Payloads are encrypted; catalog/headers are plaintext. Can browse directory hierarchy without password. |
| `headerAndData` | `"HEADER_AND_DATA"` | Tier 2: Headers/catalog and payloads are encrypted. Directory cannot be traversed without password. |
| `unsupported` | `"UNSUPPORTED"` | Archive format does not expose reliable encryption metadata probes. |

### `ArchiveCipherAlgorithm`
Specific symmetric cipher or key derivation model used.

| Case | String Representation | Supported Formats |
| :--- | :--- | :--- |
| `winzipAES128` | `"WinZip-AES-128"` | ZIP (AE-1 / AE-2) |
| `winzipAES256` | `"WinZip-AES-256"` | ZIP (AE-1 / AE-2) |
| `zipCrypto` | `"ZipCrypto"` | ZIP (Legacy PKWARE) |
| `sevenZipAES256` | `"7z-AES-256"` | 7Z (Plain & Encoded Header) |
| `rar4AES128` | `"RAR4-AES-128"` | RAR (v2.9-4.x) |
| `rar5AES256` | `"RAR5-AES-256"` | RAR (v5.0+ PBKDF2/BLAKE2sp) |
| `unknown` | `"Unknown"` | Unrecognized extra fields |

---

## 2. Entities

### `ArchiveEntry` (Extended)
Represents a single catalog entry inside an archive.

| Field Name | Swift Type | Contract Type | Required | Description |
| :--- | :--- | :--- | :--- | :--- |
| `path` | `String` | `string` | Yes | Full normalized archive path (e.g. `"dir/file.txt"`). |
| `name` | `String` | `string` | Yes | Base file or folder name. |
| `uncompressedSize` | `Int64` | `integer` | Yes | Declared uncompressed size in bytes. |
| `isDirectory` | `Bool` | `boolean` | Yes | Flag indicating directory status. |
| `isEncrypted` | `Bool` | `boolean` | Yes | `true` if payload or header is encrypted. |
| `isDataEncrypted` | `Bool` | `boolean` | Yes | `true` if the entry content data is encrypted. |
| `isMetadataEncrypted` | `Bool` | `boolean` | Yes | `true` if entry metadata itself was encrypted. |
| `encryptionMethod` | `String?` | `string` (nullable) | No | Canonical cipher name (e.g. `"WinZip-AES-256"`). |
| `modificationDate` | `Date?` | `string` (date-time, nullable) | No | Timestamp of entry modification. |
| `detectedEncoding` | `String.Encoding?` | `string` (nullable) | No | Text encoding used for filename. |

### `ArchiveFixtureDescriptor`
Metadata describing a static test corpus fixture file.

| Field Name | Swift Type | Contract Type | Required | Description |
| :--- | :--- | :--- | :--- | :--- |
| `fixtureId` | `String` | `string` | Yes | Unique fixture identifier (e.g. `"zip_winzip_aes256_deflate"`). |
| `fileName` | `String` | `string` | Yes | File name in `Tests/TTZipTests/Fixtures/Encrypted/`. |
| `format` | `String` | `string` | Yes | Format identifier (`"zip"`, `"7z"`, `"rar4"`, `"rar5"`). |
| `expectedTier` | `ArchiveEncryptionTier` | `string` (enum) | Yes | Expected encryption tier (`"DATA_ONLY"`, `"HEADER_AND_DATA"`). |
| `expectedAlgorithm` | `ArchiveCipherAlgorithm` | `string` (enum) | Yes | Expected cipher algorithm. |
| `validPassphrases` | `[String]` | `array<string>` | Yes | Array of passphrases that successfully decrypt. |
| `invalidPassphrases` | `[String]` | `array<string>` | Yes | Array of passphrases that must fail cleanly. |
| `expectedEntriesCount` | `Int` | `integer` | Yes | Total entry count in archive. |
| `expectedPayloadSHA256` | `[String: String]` | `object` (string->string) | Yes | Map of entry path to expected SHA-256 digest. |

---

## 3. Error Model (`ArchiveError`)

| Error Case | Associated Values | Description |
| :--- | :--- | :--- |
| `passwordRequired` | `archivePath: String, tier: ArchiveEncryptionTier` | Password missing when opening Tier 2 or extracting Tier 1. |
| `wrongPassword` | `archivePath: String` | Passphrase rejected by PVV, PSWCHECK, or MAC verification. |
| `unsupportedEncryptionMethod` | `archivePath: String, method: String` | Cipher algorithm not supported on current platform. |
| `corruptedData` | `archivePath: String, entryPath: String` | Password correct but checksum/MAC validation failed. |
| `readFailed` | `code: Int32` | Underlying POSIX / C bridge error code. |
| `fileNotFound` | `archivePath: String` | Specified archive path does not exist. |
