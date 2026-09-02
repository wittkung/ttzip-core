// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import Observation

// MARK: - Swift 6 Domain Enums and Models

/// Supported binary serialization and compression formats for delta patches in Swift.
public enum DeltaPatchFormat: String, Sendable, Codable, CaseIterable {
    /// Raw uncompressed byte-level delta instructions.
    case rawByteBlock
    /// Zstandard compressed delta payload.
    case zstdCompressed
    /// Standard Flate/Deflate compressed delta payload.
    case flateCompressed

    /// Converts from UniFFI enum representation.
    public init(from uniffiFormat: UniFfiDeltaFormat) {
        switch uniffiFormat {
        case .rawByteBlock: self = .rawByteBlock
        case .zstdCompressed: self = .zstdCompressed
        case .flateCompressed: self = .flateCompressed
        }
    }

    /// Converts to UniFFI enum representation.
    public var uniffiFormat: UniFfiDeltaFormat {
        switch self {
        case .rawByteBlock: return .rawByteBlock
        case .zstdCompressed: return .zstdCompressed
        case .flateCompressed: return .flateCompressed
        }
    }
}

/// Strongly-typed Swift 6 representation of a delta patch reconstruction result.
public struct DeltaPatchResult: Sendable, Codable, Hashable {
    /// Whether patch reconstruction and hash verification succeeded.
    public let success: Bool
    /// Size in bytes of the applied delta patch package.
    public let patchSize: UInt64
    /// Size in bytes of the reconstructed target payload.
    public let targetSize: UInt64
    /// Hex-encoded SHA-256 digest of the reconstructed target data.
    public let targetHash: String
    /// Whether the patch was executed directly in memory without disk staging.
    public let appliedInMemory: Bool
    /// Execution duration in milliseconds.
    public let durationMs: Double
    /// Reconstructed target binary bytes.
    public let patchedBytes: Data

    public init(
        success: Bool,
        patchSize: UInt64,
        targetSize: UInt64,
        targetHash: String,
        appliedInMemory: Bool,
        durationMs: Double,
        patchedBytes: Data
    ) {
        self.success = success
        self.patchSize = patchSize
        self.targetSize = targetSize
        self.targetHash = targetHash
        self.appliedInMemory = appliedInMemory
        self.durationMs = durationMs
        self.patchedBytes = patchedBytes
    }

    /// Initializes from UniFFI record.
    public init(from uniffiRecord: UniFfiDeltaPatchResult) {
        self.success = uniffiRecord.success
        self.patchSize = uniffiRecord.patchSize
        self.targetSize = uniffiRecord.targetSize
        self.targetHash = uniffiRecord.targetHash
        self.appliedInMemory = uniffiRecord.appliedInMemory
        self.durationMs = uniffiRecord.durationMs
        self.patchedBytes = uniffiRecord.patchedBytes
    }
}

/// Strongly-typed Swift 6 representation of an update candidate in an Appcast feed.
public struct AppcastReleaseItem: Sendable, Codable, Identifiable, Hashable {
    /// Unique identifier for this release candidate.
    public var id: String { "\(version)-\(buildNumber)" }
    /// Semantic version string (e.g. "1.2.0").
    public let version: String
    /// Monotonically increasing build integer.
    public let buildNumber: UInt64
    /// Minimum compatible macOS version requirement (e.g. "14.0").
    public let minOsVersion: String
    /// Optional URL pointing to release notes.
    public let releaseNotesUrl: URL?
    /// Full package download URL.
    public let downloadUrl: URL
    /// Full package payload size in bytes.
    public let downloadSize: UInt64
    /// Detached Ed25519 digital signature in Base64 representation.
    public let signatureEd25519: String
    /// Hex-encoded NIST SHA-256 digest of the full target package.
    public let sha256: String
    /// Optional URL for delta patch package from a specific previous base version.
    public let deltaPatchUrl: URL?
    /// Previous base version required by the delta patch.
    public let deltaBaseVersion: String?
    /// Detached Ed25519 digital signature of the delta patch payload.
    public let deltaSignatureEd25519: String?
    /// Delta patch package payload size in bytes.
    public let deltaSize: UInt64?
    /// Whether this update is marked as a critical security patch.
    public let isCritical: Bool
    /// Publication date.
    public let publishedAt: Date

    public init(
        version: String,
        buildNumber: UInt64,
        minOsVersion: String,
        releaseNotesUrl: URL? = nil,
        downloadUrl: URL,
        downloadSize: UInt64,
        signatureEd25519: String,
        sha256: String,
        deltaPatchUrl: URL? = nil,
        deltaBaseVersion: String? = nil,
        deltaSignatureEd25519: String? = nil,
        deltaSize: UInt64? = nil,
        isCritical: Bool = false,
        publishedAt: Date = Date()
    ) {
        self.version = version
        self.buildNumber = buildNumber
        self.minOsVersion = minOsVersion
        self.releaseNotesUrl = releaseNotesUrl
        self.downloadUrl = downloadUrl
        self.downloadSize = downloadSize
        self.signatureEd25519 = signatureEd25519
        self.sha256 = sha256
        self.deltaPatchUrl = deltaPatchUrl
        self.deltaBaseVersion = deltaBaseVersion
        self.deltaSignatureEd25519 = deltaSignatureEd25519
        self.deltaSize = deltaSize
        self.isCritical = isCritical
        self.publishedAt = publishedAt
    }

    /// Initializes from UniFFI record.
    public init(from uniffiRecord: UniFfiAppcastItem) {
        self.version = uniffiRecord.version
        self.buildNumber = uniffiRecord.buildNumber
        self.minOsVersion = uniffiRecord.minOsVersion
        self.releaseNotesUrl = uniffiRecord.releaseNotesUrl.flatMap { URL(string: $0) }
        self.downloadUrl = URL(string: uniffiRecord.downloadUrl) ?? URL(fileURLWithPath: "/")
        self.downloadSize = uniffiRecord.downloadSize
        self.signatureEd25519 = uniffiRecord.signatureEd25519
        self.sha256 = uniffiRecord.sha256
        self.deltaPatchUrl = uniffiRecord.deltaPatchUrl.flatMap { URL(string: $0) }
        self.deltaBaseVersion = uniffiRecord.deltaBaseVersion
        self.deltaSignatureEd25519 = uniffiRecord.deltaSignatureEd25519
        self.deltaSize = uniffiRecord.deltaSize
        self.isCritical = uniffiRecord.isCritical
        self.publishedAt = Date(timeIntervalSince1970: TimeInterval(uniffiRecord.publishedAtEpochSecs))
    }
}

/// Parsed Appcast feed metadata and release item container.
public struct AppcastFeedMetadata: Sendable, Codable, Hashable {
    /// Distribution channel (e.g. "stable", "beta").
    public let channel: String
    /// Application feed title.
    public let title: String
    /// Source feed URL.
    public let feedUrl: URL
    /// Latest available semantic version string in the feed.
    public let latestVersion: String
    /// Latest available build integer in the feed.
    public let latestBuild: UInt64
    /// List of candidate release items.
    public let items: [AppcastReleaseItem]
    /// Whether feed digital signature passed cryptographic verification.
    public let signatureValid: Bool
    /// Timestamp when feed was checked.
    public let checkedAt: Date

    public init(
        channel: String,
        title: String,
        feedUrl: URL,
        latestVersion: String,
        latestBuild: UInt64,
        items: [AppcastReleaseItem],
        signatureValid: Bool,
        checkedAt: Date = Date()
    ) {
        self.channel = channel
        self.title = title
        self.feedUrl = feedUrl
        self.latestVersion = latestVersion
        self.latestBuild = latestBuild
        self.items = items
        self.signatureValid = signatureValid
        self.checkedAt = checkedAt
    }

    /// Initializes from UniFFI record.
    public init(from uniffiRecord: UniFfiAppcastMetadata) {
        self.channel = uniffiRecord.channel
        self.title = uniffiRecord.title
        self.feedUrl = URL(string: uniffiRecord.feedUrl) ?? URL(fileURLWithPath: "/")
        self.latestVersion = uniffiRecord.latestVersion
        self.latestBuild = uniffiRecord.latestBuild
        self.items = uniffiRecord.items.map { AppcastReleaseItem(from: $0) }
        self.signatureValid = uniffiRecord.signatureValid
        self.checkedAt = Date(timeIntervalSince1970: TimeInterval(uniffiRecord.checkedAtEpochSecs))
    }
}

/// State machine lifecycle for desktop auto updates.
public enum AppUpdateState: Sendable, Equatable {
    case idle
    case checking
    case updateAvailable(item: AppcastReleaseItem, isDeltaEligible: Bool)
    case upToDate
    case downloading(progress: Double)
    case patching(progress: Double)
    case readyToInstall(targetUrl: URL)
    case error(message: String)
}

// MARK: - TTZipAppUpdateService

/// Swift 6 `@Observable` and `Sendable` desktop system update and binary delta patch service.
///
/// Integrates directly with the Rust microkernel via Mozilla UniFFI to provide:
/// - Zero-disk-decompression in-memory binary delta patch application
/// - Detached Ed25519 signature verification for Appcast feeds and packages
/// - Version monotonicity enforcement preventing downgrade attacks
/// - AppGroup state synchronization and cache persistence
@Observable
public final class TTZipAppUpdateService: @unchecked Sendable {

    /// Shared singleton instance.
    public static let shared = TTZipAppUpdateService()

    /// Default AppGroup identifier for TTZip suite synchronization.
    public static let defaultAppGroupIdentifier = "group.com.wittkung.ttzip"

    private let lock = NSLock()
    private let systemService: UniFfiSystemService

    // MARK: - Published Observable State

    /// Current operational state of the update lifecycle.
    public private(set) var state: AppUpdateState = .idle
    /// Latest metadata parsed from update feed.
    public private(set) var latestMetadata: AppcastFeedMetadata?
    /// Date when the last update check occurred.
    public private(set) var lastCheckedDate: Date?
    /// Whether an active update check or download is currently in flight.
    public private(set) var isBusy: Bool = false
    /// Total cumulative binary delta patches applied during app lifecycle.
    public private(set) var cumulativePatchesApplied: Int = 0
    /// Total cumulative bandwidth saved in bytes via delta patching instead of full downloads.
    public private(set) var totalBandwidthSavedBytes: Int64 = 0
    /// Most recent localized error message if an operation failed.
    public private(set) var lastErrorMessage: String?

    // MARK: - Initialization

    /// Initializes service instance backed by Rust `UniFFISystemService`.
    public init() {
        self.systemService = UniFfiSystemService()
    }

    // MARK: - In-Memory Delta Patching

    /// Applies a binary delta patch onto base bytes directly in memory with optional target hash verification.
    ///
    /// - Parameters:
    ///   - baseBytes: Existing base binary payload.
    ///   - patchBytes: Compressed or raw TTZip delta patch package.
    ///   - expectedHash: Optional expected SHA-256 target digest for integrity verification.
    /// - Returns: Reconstructed target data and execution telemetry.
    public func applyDeltaPatchInMemory(
        baseBytes: Data,
        patchBytes: Data,
        expectedHash: String? = nil
    ) throws -> DeltaPatchResult {
        let uniffiResult = try systemService.applyDeltaPatch(
            baseBytes: baseBytes,
            patchBytes: patchBytes,
            expectedTargetHash: expectedHash
        )
        let domainResult = DeltaPatchResult(from: uniffiResult)

        lock.lock()
        cumulativePatchesApplied += 1
        let saved = Int64(domainResult.targetSize) - Int64(domainResult.patchSize)
        if saved > 0 {
            totalBandwidthSavedBytes += saved
        }
        lock.unlock()

        return domainResult
    }

    /// Creates a compressed binary delta patch from base bytes to target bytes.
    public func createDeltaPatch(
        baseBytes: Data,
        targetBytes: Data,
        format: DeltaPatchFormat = .rawByteBlock
    ) throws -> Data {
        return try systemService.createDeltaPatch(
            baseBytes: baseBytes,
            targetBytes: targetBytes,
            format: format.uniffiFormat
        )
    }

    // MARK: - Cryptographic Verification & Integrity

    /// Computes deterministic Merkle tree hash of a file or directory on disk.
    public func calculateTreeHash(for path: String) throws -> String {
        return try systemService.calculateTreeHash(rootPath: path)
    }

    /// Verifies detached Ed25519 digital signature of feed or binary bytes against Base64 public key.
    public func verifyAppcastSignature(
        data: Data,
        signatureBase64: String,
        publicKeyBase64: String
    ) throws -> Bool {
        return try systemService.verifyAppcastSignature(
            appcastBytes: data,
            signatureBase64: signatureBase64,
            publicKeyBase64: publicKeyBase64
        )
    }

    /// Validates version monotonicity preventing downgrade attacks.
    public func checkVersionMonotonicity(
        currentVersion: String,
        incomingVersion: String
    ) throws -> Bool {
        return try systemService.checkVersionMonotonicity(
            currentVersion: currentVersion,
            incomingVersion: incomingVersion
        )
    }

    // MARK: - Appcast Feed Inspection & State Machine

    /// Parses JSON representation of an Appcast feed.
    public func parseAppcastJson(_ jsonString: String) throws -> AppcastFeedMetadata {
        let uniffiMeta = try systemService.parseAppcastJson(jsonContent: jsonString)
        return AppcastFeedMetadata(from: uniffiMeta)
    }

    /// Checks for available updates given an Appcast metadata payload and current local app version.
    public func evaluateUpdateCandidates(
        metadata: AppcastFeedMetadata,
        currentVersion: String,
        currentBuild: UInt64
    ) -> AppUpdateState {
        lock.lock()
        self.latestMetadata = metadata
        self.lastCheckedDate = Date()
        lock.unlock()

        // Find applicable strictly newer release item
        let candidates = metadata.items.filter { item in
            if item.buildNumber > currentBuild {
                return true
            } else if item.buildNumber == currentBuild {
                return self.isSemverStrictlyGreater(incoming: item.version, current: currentVersion)
            } else {
                return false
            }
        }

        guard let target = candidates.max(by: { $0.buildNumber < $1.buildNumber }) else {
            let newState: AppUpdateState = .upToDate
            lock.lock()
            self.state = newState
            lock.unlock()
            return newState
        }

        // Check if delta patch is available for current version
        let isDelta = target.deltaPatchUrl != nil && target.deltaBaseVersion == currentVersion
        let newState: AppUpdateState = .updateAvailable(item: target, isDeltaEligible: isDelta)
        lock.lock()
        self.state = newState
        lock.unlock()
        return newState
    }

    private func isSemverStrictlyGreater(incoming: String, current: String) -> Bool {
        let parseTokens = { (s: String) -> [UInt64] in
            let clean = s.trimmingCharacters(in: .whitespacesAndNewlines).trimmingCharacters(in: CharacterSet(charactersIn: "vV"))
            return clean.split { $0 == "." || $0 == "-" || $0 == "+" }.compactMap { UInt64($0) }
        }
        let inTokens = parseTokens(incoming)
        let curTokens = parseTokens(current)
        let maxLen = max(inTokens.count, curTokens.count)
        for i in 0..<maxLen {
            let inVal = i < inTokens.count ? inTokens[i] : 0
            let curVal = i < curTokens.count ? curTokens[i] : 0
            if inVal > curVal { return true }
            if inVal < curVal { return false }
        }
        return false
    }

    // MARK: - AppGroup & State Synchronization

    /// Persists last update check timestamp and latest version to AppGroup or standard UserDefaults.
    public func syncStateToAppGroup(suiteName: String? = defaultAppGroupIdentifier) {
        let defaults = suiteName.flatMap { UserDefaults(suiteName: $0) } ?? UserDefaults.standard
        lock.lock()
        let checked = lastCheckedDate
        let version = latestMetadata?.latestVersion
        let patches = cumulativePatchesApplied
        let savedBytes = totalBandwidthSavedBytes
        lock.unlock()

        defaults.set(checked?.timeIntervalSince1970, forKey: "TTZipLastUpdateCheckSecs")
        defaults.set(version, forKey: "TTZipLatestAvailableVersion")
        defaults.set(patches, forKey: "TTZipCumulativePatchesApplied")
        defaults.set(savedBytes, forKey: "TTZipTotalBandwidthSavedBytes")
    }

    /// Loads the last recorded update check timestamp from AppGroup or standard UserDefaults.
    public func loadLastCheckTimestamp(suiteName: String? = defaultAppGroupIdentifier) -> Date? {
        let defaults = suiteName.flatMap { UserDefaults(suiteName: $0) } ?? UserDefaults.standard
        let secs = defaults.double(forKey: "TTZipLastUpdateCheckSecs")
        guard secs > 0 else { return nil }
        return Date(timeIntervalSince1970: secs)
    }

    /// Resets all in-memory update metrics and state.
    public func resetState() {
        lock.lock()
        defer { lock.unlock() }
        self.state = .idle
        self.latestMetadata = nil
        self.lastCheckedDate = nil
        self.isBusy = false
        self.lastErrorMessage = nil
    }
}
