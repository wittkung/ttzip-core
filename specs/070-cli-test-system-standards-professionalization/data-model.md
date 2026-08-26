# Data Model Specification: CLI Test System, Full Coverage, and Standards Professionalization

**Feature Branch**: `070-cli-test-system-standards-professionalization`  
**Date**: 2026-08-17  
**Status**: Draft  

---

## 1. Standards Catalog Data Models

### 1.1 `ArchiveFormatStandardSpec`
Represents the authoritative standards definition for an archive or compression format:

```swift
public struct ArchiveFormatStandardSpec: Sendable, Equatable, Identifiable {
    public let id: String                          // e.g. "zip", "7z", "tar.zst"
    public let format: ArchiveCompressionFormat    // Enum format identifier
    public let officialName: String                // e.g. "PKWARE ZIP File Format"
    public let standardCitations: [StandardCitation] // RFC, ISO, POSIX citations
    public let mimeType: String                    // e.g. "application/zip"
    public let appleUTI: String                    // e.g. "public.zip-archive"
    public let magicSignatures: [ArchiveMagicSignature] // Header/tail/sector signatures
    public let supportedEncryption: [EncryptionStandardSpec] // Supported crypto
    public let supportsMultiVolume: Bool           // Spanning support
    public let supportedExtraFields: [ZipExtraFieldStandardSpec] // Extra field definitions
}

public struct StandardCitation: Sendable, Equatable {
    public let organization: String                // e.g. "IETF", "ISO/IEC", "IEEE", "PKWARE"
    public let standardNumber: String              // e.g. "RFC 8878", "ISO/IEC 21320-1:2015", "APPNOTE v6.3.10"
    public let title: String                       // Full title of specification
    public let canonicalURL: String                // Web URL to authoritative document
}

public struct ArchiveMagicSignature: Sendable, Equatable {
    public enum Anchor: Sendable, Equatable {
        case head(offset: Int)                     // Fixed byte offset from file start
        case tail(offsetFromEOF: Int)              // Fixed byte offset from file end
        case sector(sectorIndex: Int, byteOffset: Int) // Fixed sector (e.g. ISO 9660 sector 16)
        case tarOffset(byteOffset: Int)            // TAR header magic at offset 257
    }
    public let anchor: Anchor
    public let bytes: [UInt8]                      // Expected magic byte sequence
    public let description: String                 // Human-readable signature description
}

public struct EncryptionStandardSpec: Sendable, Equatable {
    public let standardName: String                // e.g. "WinZip AES-256 (AE-2)", "7z AES-256 CBC"
    public let keyDerivationFunction: String       // e.g. "PBKDF2-HMAC-SHA1", "SHA-256 19-cycle"
    public let cipher: String                      // e.g. "AES-256-CTR", "AES-256-CBC"
    public let authenticationTag: String?          // e.g. "HMAC-SHA1 10-byte truncation"
}

public struct ZipExtraFieldStandardSpec: Sendable, Equatable {
    public let headerID: UInt16                    // e.g. 0x5455, 0x7075, 0x7875, 0x0001, 0x9901
    public let name: String                        // e.g. "Extended Timestamp", "Unicode Path"
    public let sourceSpecification: String         // e.g. "Info-ZIP", "PKWARE Zip64", "WinZip"
}
```

---

## 2. Differential Test Harness Data Models

### 2.1 `FileTreeManifest` & `ManifestEntry`
Represents the normalized filesystem tree captured after extraction for 1:1 cross-oracle comparison:

```swift
public struct FileTreeManifest: Sendable, Equatable {
    public let rootDirectory: String
    public let entries: [String: ManifestEntry]    // Keyed by normalized relative path
    public let totalByteSize: Int64
    public let totalFileCount: Int
    public let totalDirectoryCount: Int
    public let totalSymlinkCount: Int
}

public struct ManifestEntry: Sendable, Equatable {
    public enum EntryType: String, Sendable, Equatable {
        case regularFile = "regular"
        case directory = "directory"
        case symbolicLink = "symlink"
        case hardLink = "hardlink"
    }
    public let relativePath: String                // APFS normalized relative path
    public let entryType: EntryType
    public let byteSize: Int64
    public let sha256Checksum: String              // Hex-encoded SHA-256 hash (empty for dirs/symlinks)
    public let posixMode: UInt16                   // Lower 9 bits (0o777 permissions)
    public let symlinkTarget: String?              // Destination path if symlink
}

public struct DifferentialTestReport: Sendable, Equatable {
    public let format: ArchiveCompressionFormat
    public let targetOracle: String                // e.g. "/usr/bin/tar", "bsdtar", "7zz"
    public let isPassed: Bool
    public let ttzipManifest: FileTreeManifest
    public let oracleManifest: FileTreeManifest
    public let divergenceErrors: [String]          // Detailed descriptions of discrepancies
    public let hexDiffOutput: String?              // Formatted hex diff if payload differed
}
```

---

## 3. Fuzzing & Mutation Data Models

### 3.1 `FuzzMutationConfig` & `FuzzResult`
Represents the deterministic fuzzing parameters and execution results:

```swift
public struct FuzzMutationConfig: Sendable, Equatable {
    public enum MutationOperator: String, Sendable, Equatable {
        case bitFlip = "bit_flip"
        case byteReplace = "byte_replace"
        case corruptMagic = "corrupt_magic"
        case corruptCRC = "corrupt_crc"
        case truncateStream = "truncate_stream"
        case injectZipSlipPath = "zip_slip_path"
        case oversizeHeader = "oversize_header"
        case invalidDictSize = "invalid_dict_size"
    }
    public let seed: UInt64                        // 64-bit deterministic PRNG seed
    public let iterationCount: Int                 // Number of mutation rounds
    public let operators: [MutationOperator]
    public let targetFormat: ArchiveCompressionFormat
    public let crashDumpDirectory: String?         // Path to persist reproducers
}

public struct FuzzIterationResult: Sendable, Equatable {
    public let iteration: Int
    public let seed: UInt64
    public let appliedOperator: FuzzMutationConfig.MutationOperator
    public let originalByteSize: Int
    public let mutatedByteSize: Int
    public let exitCode: Int32                     // C error status code (must be negative on corrupt)
    public let caughtSwiftError: String?           // Structured Swift error caught
    public let isGracefullyRejected: Bool          // True if safely rejected without crash
    public let reproducerPath: String?             // Preserved file path if failure occurred
}
```

---

## 4. Test Telemetry Data Models

### 4.1 `TestTelemetryEvent`
Represents structured NDJSON events emitted during `ttzip-cli test --json`:

```swift
public struct TestTelemetryEvent: Sendable, Codable, Equatable {
    public enum EventType: String, Sendable, Codable, Equatable {
        case testRunStarted
        case suiteStarted
        case testCaseStarted
        case testCasePassed
        case testCaseFailed
        case testCaseSkipped
        case suiteFinished
        case testRunFinished
    }

    public struct TelemetryError: Sendable, Codable, Equatable {
        public let message: String
        public let diff: String?
        public let stackTrace: [String]?

        public init(message: String, diff: String? = nil, stackTrace: [String]? = nil) {
            self.message = message
            self.diff = diff
            self.stackTrace = stackTrace
        }
    }

    public struct TelemetryMetrics: Sendable, Codable, Equatable {
        public let totalTests: Int
        public let passedTests: Int
        public let failedTests: Int
        public let skippedTests: Int
        public let passRate: Double

        public init(totalTests: Int, passedTests: Int, failedTests: Int, skippedTests: Int, passRate: Double) {
            self.totalTests = totalTests
            self.passedTests = passedTests
            self.failedTests = failedTests
            self.skippedTests = skippedTests
            self.passRate = passRate
        }
    }

    public let eventType: EventType
    public let timestamp: String
    public let sessionID: String
    public let suiteName: String?
    public let testCaseName: String?
    public let durationMs: Double?
    public let error: TelemetryError?
    public let metrics: TelemetryMetrics?

    public init(
        eventType: EventType,
        timestamp: String,
        sessionID: String,
        suiteName: String? = nil,
        testCaseName: String? = nil,
        durationMs: Double? = nil,
        error: TelemetryError? = nil,
        metrics: TelemetryMetrics? = nil
    ) {
        self.eventType = eventType
        self.timestamp = timestamp
        self.sessionID = sessionID
        self.suiteName = suiteName
        self.testCaseName = testCaseName
        self.durationMs = durationMs
        self.error = error
        self.metrics = metrics
    }
}
```
