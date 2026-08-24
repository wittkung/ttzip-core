# 🐦 TTZip Swift 6 SDK Developer Guide

[![Swift 6](https://img.shields.io/badge/Swift-6.0%20Strict%20Concurrency-orange.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/Package.swift)
[![Platforms: macOS 14+](https://img.shields.io/badge/Platforms-macOS%2014.0%2B%20%7C%20iOS%2017.0%2B-blue.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/apple/README.md)
[![Concurrency: Sendable & Actors](https://img.shields.io/badge/Concurrency-Complete%20Actor%20Isolation-brightgreen.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/ARCHITECTURE.md)

The `TTZipCore` Swift package provides native Swift 6 bindings for TTZip, designed specifically for Apple platforms (macOS, iOS, visionOS). It features **Swift 6 Strict Concurrency**, actor isolation, `AsyncThrowingStream` progress pipelines, Keychain-backed password vaulting, and native Apple Silicon P-core / E-core hardware scheduling.

---

## 1. Swift Package Manager Integration

Add `TTZipCore` to your `Package.swift`:

```swift
// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "MyArchiveUtility",
    platforms: [.macOS(.v14), .iOS(.v17)],
    dependencies: [
        .package(path: "core") // Or remote git repository URL
    ],
    targets: [
        .target(
            name: "MyArchiveUtility",
            dependencies: [
                .product(name: "TTZipCore", package: "core"),
                .product(name: "CTTZipBridge", package: "core")
            ],
            swiftSettings: [
                .enableUpcomingFeature("StrictConcurrency")
            ]
        )
    ]
)
```

---

## 2. Architecture & Concurrency Model

`TTZipCore` isolates all native FFI operations within Swift actors and background tasks:

```
┌────────────────────────────────────────────────────────┐
│               SwiftUI / AppKit Presentation            │
│            (@MainActor · 60 FPS View Updates)          │
└───────────────────────────┬────────────────────────────┘
                            │ AsyncThrowingStream Yields
┌───────────────────────────▼────────────────────────────┐
│                  TTZipCore Swift 6 Actors              │
│   - ArchiveExtractor (actor)                           │
│   - ArchiveWriter (actor)                              │
│   - PasswordVaultManager (actor · Apple CryptoKit)     │
│   - RustVfsSession (Sendable Tree Handle)              │
└───────────────────────────┬────────────────────────────┘
                            │ C-ABI Pinned Pointer Exchange
┌───────────────────────────▼────────────────────────────┐
│               CTTZipBridge (module.modulemap)          │
│                Safe Rust Native Microkernel            │
└────────────────────────────────────────────────────────┘
```

---

## 3. Quickstart Code Examples

### 3.1 Streaming Archive Extraction with Real-Time Progress

Extract an archive asynchronously while observing real-time progress and cancellation:

```swift
import Foundation
import TTZipCore

@MainActor
class ExtractionViewModel: ObservableObject {
    @Published var progressFraction: Double = 0.0
    @Published var currentFile: String = ""
    @Published var isFinished: Bool = false

    private let extractor = ArchiveExtractor()

    func extractArchive(archiveURL: URL, destinationURL: URL, password: String? = nil) async {
        do {
            let progressStream = await extractor.extract(
                from: archiveURL,
                to: destinationURL,
                password: password
            )

            for try await progress in progressStream {
                self.progressFraction = progress.fractionCompleted
                self.currentFile = progress.currentFile
            }

            self.isFinished = true
            print("Extraction finished successfully to: \(destinationURL.path)")
        } catch {
            print("Extraction failed: \(error.localizedDescription)")
        }
    }
}
```

### 3.2 High-Throughput Parallel Archive Creation

Compress multiple directories and files using the Rayon multi-threaded writer:

```swift
import Foundation
import TTZipCore

func createBackupArchive() async throws {
    let sourceURLs = [
        URL(fileURLWithPath: "/Users/dev/Documents/Projects"),
        URL(fileURLWithPath: "/Users/dev/Documents/Notes.md")
    ]
    let destinationURL = URL(fileURLWithPath: "/Users/dev/Desktop/Backup_2026.zip")

    let writer = ArchiveWriter()
    try await writer.compress(
        sources: sourceURLs,
        destination: destinationURL,
        format: .zip,
        level: .normal, // Level 6
        password: nil,
        threads: 0      // 0 = Auto-tune across P/E-cores
    )

    print("Created archive at: \(destinationURL.path)")
}
```

### 3.3 Inspecting Archive Metadata & Asian Charset Detection

Inspect archive contents with automatic character set detection (GB18030, Shift-JIS, Big5, UTF-8):

```swift
import Foundation
import TTZipCore

func inspectLegacyArchive(archiveURL: URL) async throws {
    let reader = try await ArchiveReader.open(at: archiveURL)

    print("Listing entries for \(archiveURL.lastPathComponent):")
    for entry in reader.entries {
        print("""
        ------------------------------------------
        Path:               \(entry.path)
        Uncompressed Size:  \(ByteCountFormatter.string(fromByteCount: Int64(entry.uncompressedSize), countStyle: .file))
        CRC-32:             \(String(format: "%08X", entry.crc32))
        Encrypted:          \(entry.isEncrypted)
        Detected Charset:   \(entry.detectedEncoding ?? "UTF-8")
        """)
    }
}
```

---

## 4. Keychain-Integrated Password Vault

`PasswordVaultManager` encrypts credentials using **Apple CryptoKit AES-GCM** (256-bit) and locks credentials into the macOS Keychain:

```swift
import Foundation
import TTZipCore

func storeAndRetrieveArchiveKey() async throws {
    let vault = PasswordVaultManager.shared
    let archivePath = "/Users/dev/Secure/Financials.7z"
    let userSecret = "SuperSecretPassword123!"

    // 1. Save password to Keychain with Hardware Key Wrapping
    try await vault.storePassword(userSecret, forArchivePath: archivePath)
    print("Password stored securely in macOS Keychain.")

    // 2. Fetch password for transparent extraction
    if let retrievedSecret = try await vault.retrievePassword(forArchivePath: archivePath) {
        print("Retrieved secret from vault (Length: \(retrievedSecret.count))")
    }

    // 3. Clear sensitive memory
    await vault.wipeCachedCredentials()
}
```

---

## 5. Interactive VFS Session & Instant Search

For responsive SwiftUI `NSOutlineView` archive browsing, `RustVfsSession` holds an in-memory virtual tree and searches 100,000+ files with zero allocations:

```swift
import Foundation
import TTZipCore

func performFastVfsSearch(archiveURL: URL) throws {
    let session = try RustVfsSession(archiveURL: archiveURL)

    // Sub-millisecond zero-alloc fuzzy search
    let matches = try session.search(query: "report_2026.pdf", limit: 20)

    for match in matches {
        print("Found: \(match.path) | Size: \(match.uncompressedSize) | Score: \(match.score)")
    }
}
```

---

## 6. Apple Silicon Hardware Acceleration Sensing

Query CPU topology and SIMD feature vector:

```swift
import TTZipCore

func printCpuDiagnostics() {
    let tuner = AppleSiliconTuner.shared

    print("Is Hardware Accelerated: \(tuner.isHardwareAccelerated)")
    print("Performance Cores (P):  \(tuner.performanceCoreCount)")
    print("Efficiency Cores (E):   \(tuner.efficiencyCoreCount)")
    print("Optimal Thread Budget:  \(tuner.recommendedThreadBudget(for: .highThroughput))")
}
```

---

## 7. Error Types & Handling

`TTZipCore` converts native status codes into Swift `ArchiveError` exceptions:

```swift
public enum ArchiveError: Error, LocalizedError {
    case invalidParameter(String)
    case fileNotFound(URL)
    case corruptHeader(offset: UInt64, details: String)
    case invalidPassword
    case securityViolation(String) // Zip Slip attack detected
    case engineFailure(statusCode: Int32, message: String)
    case cancelled

    public var errorDescription: String? {
        switch self {
        case .invalidPassword:
            return "Incorrect password or authentication tag mismatch."
        case .securityViolation(let reason):
            return "Security violation prevented: \(reason)"
        case .engineFailure(_, let msg):
            return "Native Engine Error: \(msg)"
        default:
            return "Archive operation failed."
        }
    }
}
```
