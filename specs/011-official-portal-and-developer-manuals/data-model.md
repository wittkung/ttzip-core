# Data Model: Official Portal & Comprehensive Developer Manuals

- **Feature**: `specs/011-official-portal-and-developer-manuals`
- **Scope**: Portal Sitemap, Route Entities, SDK Language Descriptors, and Format Matrix

---

## 1. Entity Definitions

### 1.1 `PortalRoute`
Represents an authoritative public page within the `ttzip.app` domain.

| Field | Type | Description |
| :--- | :--- | :--- |
| `path` | `String` | URL relative path (e.g. `/index.html`, `/sdk.html`, `/cli.html`) |
| `title` | `String` | Page title for browser tab and SEO metadata |
| `description` | `String` | Meta description for search engines |
| `category` | `Enum` | `Product`, `Developer`, `Architecture`, `Legal`, `Support` |
| `navOrder` | `Integer` | Header navigation display priority |

### 1.2 `SDKDescriptor`
Represents an official Tier-1 programming language binding in the Developer Center.

| Field | Type | Description |
| :--- | :--- | :--- |
| `languageId` | `String` | Identifier: `cpp`, `rust`, `python`, `go`, `jvm`, `csharp`, `dart`, `swift` |
| `languageName` | `String` | Display Name (e.g. "Python 3.10+", "Rust 1.80+", "Go 1.22+") |
| `packageManagerCmd` | `String` | One-line install command (e.g. `pip install ttzip`, `cargo add ttzip-rs`) |
| `mweCodeSnippet` | `String` | Minimal Working Example (in-process zero-subprocess extraction) |
| `cAbiStandard` | `String` | `C-ABI 2.0 (libttzip_core.dylib)` |
| `isSubprocessFree` | `Boolean` | Always `true` (guaranteed by Constitution) |

### 1.3 `FormatSpecification`
Represents a supported compression/archive format in `/formats.html`.

| Field | Type | Description |
| :--- | :--- | :--- |
| `extension` | `String` | File extension (e.g. `.zip`, `.7z`, `.tar.zst`, `.rar`) |
| `formatName` | `String` | Full name (e.g. "Zstandard Streaming Tarball") |
| `rfcStandard` | `String` | RFC / Specification reference (e.g. `RFC 8878`, `RFC 1951`) |
| `decodeSpeed` | `String` | Benchmark throughput (e.g. `15.2 GB/s`, `48.1 GB/s`) |
| `canCompress` | `Boolean` | Whether creation/compression is supported |
| `canExtract` | `Boolean` | Whether extraction/decompression is supported |
| `encryptionSupport` | `String` | `AES-256-GCM`, `ZipCrypto`, `7z-AES`, `None` |
| `apfsCloneSupport` | `Boolean` | Whether APFS clonefile is available |

### 1.4 `LicenseChannel`
Represents a distribution and licensing tier in `/licensing.html`.

| Field | Type | Description |
| :--- | :--- | :--- |
| `channelId` | `String` | `community`, `direct`, `mas`, `steam` |
| `channelName` | `String` | Display name (e.g. "Mac App Store", "Direct DMG", "Community") |
| `priceCNY` | `String` | `¥0`, `¥28`, `¥29` |
| `licenseVerification`| `String` | `None (Open Source)`, `Ed25519 Cryptographic Key`, `App Store Receipt`, `Steam API` |
| `updateMechanism` | `String` | `Homebrew / Git`, `Sparkle Auto-Update`, `Mac App Store`, `SteamPipe` |
