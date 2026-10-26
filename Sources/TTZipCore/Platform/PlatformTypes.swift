// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import os

/// Cross-platform path sanitization and normalization analysis outcome.
public struct PlatformPathNormalizationResult: Sendable, Equatable {
    public let originalPath: String
    public let normalizedPath: String
    public let isAbsolute: Bool
    public let isUNCPath: Bool
    public let isLongPath: Bool
    public let containsWindowsReservedDeviceName: Bool
    public let strippedAlternateDataStream: String?
    public let win32FormattedPath: String
    public let hasTraversalAttack: Bool
    
    public init(
        originalPath: String,
        normalizedPath: String,
        isAbsolute: Bool,
        isUNCPath: Bool,
        isLongPath: Bool,
        containsWindowsReservedDeviceName: Bool,
        strippedAlternateDataStream: String? = nil,
        win32FormattedPath: String,
        hasTraversalAttack: Bool = false
    ) {
        self.originalPath = originalPath
        self.normalizedPath = normalizedPath
        self.isAbsolute = isAbsolute
        self.isUNCPath = isUNCPath
        self.isLongPath = isLongPath
        self.containsWindowsReservedDeviceName = containsWindowsReservedDeviceName
        self.strippedAlternateDataStream = strippedAlternateDataStream
        self.win32FormattedPath = win32FormattedPath
        self.hasTraversalAttack = hasTraversalAttack
    }
}

/// Unified cross-platform file system metadata attributes.
public struct PlatformFileAttributes: Sendable, Equatable {
    public let size: Int64
    public let isDirectory: Bool
    public let isSymbolicLink: Bool
    public let creationTimeUnixSec: Int64
    public let modificationTimeUnixSec: Int64
    public let posixPermissions: UInt32
    public let isReadOnly: Bool
    public let isHidden: Bool
    
    public init(
        size: Int64,
        isDirectory: Bool,
        isSymbolicLink: Bool,
        creationTimeUnixSec: Int64,
        modificationTimeUnixSec: Int64,
        posixPermissions: UInt32,
        isReadOnly: Bool,
        isHidden: Bool
    ) {
        self.size = size
        self.isDirectory = isDirectory
        self.isSymbolicLink = isSymbolicLink
        self.creationTimeUnixSec = creationTimeUnixSec
        self.modificationTimeUnixSec = modificationTimeUnixSec
        self.posixPermissions = posixPermissions
        self.isReadOnly = isReadOnly
        self.isHidden = isHidden
    }
}

/// Virtual memory mapping descriptor with managed lifecycle ensuring strictly single unmap/close
/// and zero-copy scoped borrowing with Use-After-Free immunization.
public final class PlatformMmapResult: @unchecked Sendable {
    private struct State: Sendable {
        let rawAddress: UInt
        let size: Int
        var rawDescriptor: Int32
        var isUnmapped: Bool = false
        var activeBorrowers: Int = 0
        var pendingMunmap: Bool = false
    }

    private let stateLock: OSAllocatedUnfairLock<State>

    public init(pointer: UnsafeRawPointer, size: Int, rawDescriptor: Int32) {
        self.stateLock = OSAllocatedUnfairLock(initialState: State(
            rawAddress: UInt(bitPattern: pointer),
            size: size,
            rawDescriptor: rawDescriptor,
            isUnmapped: false,
            activeBorrowers: 0,
            pendingMunmap: false
        ))
    }

    deinit {
        unmap()
    }

    /// Backwards-compatible raw pointer accessor with lifecycle assertions.
    ///
    /// - Note: Prefer `withUnsafeBytes(_:)` for scoped borrow safety against concurrent unmapping.
    public var pointer: UnsafeRawPointer {
        let address: UInt = stateLock.withLock { state in
            precondition(!state.isUnmapped && !state.pendingMunmap, "Attempted to access pointer of an unmapped PlatformMmapResult (Use-After-Free detected)")
            return state.rawAddress
        }
        guard let ptr = UnsafeRawPointer(bitPattern: address) else {
            fatalError("PlatformMmapResult holds null address")
        }
        return ptr
    }

    /// Size of the memory-mapped virtual region in bytes.
    public var size: Int {
        stateLock.withLock { $0.size }
    }

    /// True if this mapping has been explicitly unmapped or marked for deallocation.
    public var isUnmapped: Bool {
        stateLock.withLock { $0.isUnmapped || $0.pendingMunmap }
    }

    /// Increments the active borrower count and returns the raw base pointer.
    /// Guarantees the underlying memory region will not be unmapped while the borrow is active.
    /// Callers MUST balance this with a corresponding `releaseBorrow()` call.
    public func retainBorrow() throws -> UnsafeRawPointer {
        let address: UInt = try stateLock.withLock { state in
            guard !state.isUnmapped && !state.pendingMunmap else {
                throw POSIXError(.EBADF)
            }
            state.activeBorrowers += 1
            return state.rawAddress
        }
        guard let ptr = UnsafeRawPointer(bitPattern: address) else {
            throw POSIXError(.EINVAL)
        }
        return ptr
    }

    /// Decrements the active borrower count. If unmapping was deferred pending this borrow,
    /// performs the physical `munmap` and closes the file descriptor once borrower count drops to zero.
    public func releaseBorrow() {
        let cleanup: (UInt, Int, Int32)? = stateLock.withLock { state in
            state.activeBorrowers -= 1
            if state.pendingMunmap && state.activeBorrowers == 0 {
                let addr = state.rawAddress
                let s = state.size
                let fd = state.rawDescriptor
                state.rawDescriptor = -1
                return (addr, s, fd)
            }
            return nil
        }
        if let (addr, s, fd) = cleanup {
            if s > 0, let p = UnsafeMutableRawPointer(bitPattern: addr) {
                munmap(p, s)
            }
            if fd >= 0 {
                close(fd)
            }
        }
    }

    /// Executes a scoped closure providing direct, zero-copy borrowed access to the mapped byte buffer.
    /// Guarantees that the underlying virtual memory mapping cannot be freed or unmapped while `body` is executing.
    ///
    /// - Parameter body: A closure taking an `UnsafeRawBufferPointer` to the memory-mapped bytes.
    /// - Returns: The value returned by `body`.
    /// - Throws: `POSIXError(.EBADF)` if the mapping is already unmapped, or any error thrown by `body`.
    public func withUnsafeBytes<R>(_ body: (UnsafeRawBufferPointer) throws -> R) throws -> R {
        let ptr = try retainBorrow()
        let sz = self.size
        defer {
            releaseBorrow()
        }
        let buffer = UnsafeRawBufferPointer(start: ptr, count: sz)
        return try body(buffer)
    }

    /// Explicitly unmaps the virtual memory region and closes the underlying file descriptor.
    /// If there are active borrowers currently executing inside `withUnsafeBytes`, physical
    /// unmapping is deferred until the last active borrower returns to prevent Use-After-Free crashes.
    public func unmap() {
        let immediateCleanup: (UInt, Int, Int32)? = stateLock.withLock { state in
            guard !state.isUnmapped else { return nil }
            state.isUnmapped = true
            if state.activeBorrowers > 0 {
                state.pendingMunmap = true
                return nil
            } else {
                let addr = state.rawAddress
                let s = state.size
                let fd = state.rawDescriptor
                state.rawDescriptor = -1
                return (addr, s, fd)
            }
        }

        if let (addr, s, fd) = immediateCleanup {
            if s > 0, let p = UnsafeMutableRawPointer(bitPattern: addr) {
                munmap(p, s)
            }
            if fd >= 0 {
                close(fd)
            }
        }
    }
}

/// CPU architecture and SIMD hardware acceleration feature mask.
public struct CPUFeatureSet: Sendable, Equatable {
    public let architecture: String
    public let logicalCores: Int
    public let pCores: Int
    public let eCores: Int
    public let physicalPageSize: Int
    public let hasARMNeon: Bool
    public let hasARMCrypto: Bool
    public let hasAESNI: Bool
    public let hasAVX2: Bool
    public let hasAVX512: Bool
    public let hasHardwareCRC32: Bool
    
    public init(
        architecture: String,
        logicalCores: Int,
        pCores: Int = 0,
        eCores: Int = 0,
        physicalPageSize: Int,
        hasARMNeon: Bool,
        hasARMCrypto: Bool,
        hasAESNI: Bool,
        hasAVX2: Bool,
        hasAVX512: Bool,
        hasHardwareCRC32: Bool
    ) {
        self.architecture = architecture
        self.logicalCores = logicalCores
        self.pCores = pCores
        self.eCores = eCores
        self.physicalPageSize = physicalPageSize
        self.hasARMNeon = hasARMNeon
        self.hasARMCrypto = hasARMCrypto
        self.hasAESNI = hasAESNI
        self.hasAVX2 = hasAVX2
        self.hasAVX512 = hasAVX512
        self.hasHardwareCRC32 = hasHardwareCRC32
    }
}

// MARK: - Operating System

//
//


/// Cross-platform operating system discriminator.
public enum PlatformOperatingSystem: String, Sendable, Codable, CaseIterable {
    case macOS = "macOS"
    case windows = "Windows"
    case linux = "Linux"
    case unknown = "Unknown"
    
    /// Operating system platform in current execution environment.
    public static var current: PlatformOperatingSystem {
        #if os(macOS)
        return .macOS
        #elseif os(Windows)
        return .windows
        #elseif os(Linux)
        return .linux
        #else
        return .unknown
        #endif
    }
    
    /// True if current platform conforms to POSIX semantics.
    @inlinable
    public var isPOSIX: Bool {
        return self == .macOS || self == .linux
    }
    
    /// True if current platform is Windows.
    @inlinable
    public var isWindows: Bool {
        return self == .windows
    }
    
    /// Default hardware physical page alignment (16KB on Apple Silicon, 4KB on Generic/x86_64).
    @inlinable
    public var defaultPageAlignment: Int {
        #if os(macOS) && arch(arm64)
        return 16384
        #else
        return 4096
        #endif
    }
}

// MARK: - Path Sanitizer

//
//


/// Cross-platform path sanitization, normalization, and security auditing subsystem.
///
/// Fully backed by high-performance Swift / UniFFI path engine:
/// - Zero-allocation single-pass Zip Slip directory traversal neutralization and detection
/// - Win32 reserved device name interception (`CON`, `PRN`, `AUX`, `NUL`, `COM0-9`, `LPT0-9`, `CLOCK$`, `PhysicalDrive`)
/// - Win32 trailing space and dot normalization
/// - NTFS Alternate Data Stream (ADS) identification and stripping
/// - Unicode NFC canonical normalization
/// - Win32 extended-length path formatting (`\\?\` and `\\?\UNC\`)
public enum PlatformPathSanitizer: Sendable {
    
    /// Executes cross-platform security sanitization, ZipSlip traversal detection,
    /// and canonical NFC normalization using pure Swift algorithms.
    ///
    /// - Parameter path: Input relative or absolute path.
    /// - Returns: Normalized path result containing canonical path, boundary flags, and reserved name markers.
    public static func sanitize(path: String) -> PlatformPathNormalizationResult {
        let trimmed = path.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty {
            return PlatformPathNormalizationResult(
                originalPath: path,
                normalizedPath: "",
                isAbsolute: false,
                isUNCPath: false,
                isLongPath: false,
                containsWindowsReservedDeviceName: false,
                strippedAlternateDataStream: nil,
                win32FormattedPath: "",
                hasTraversalAttack: false
            )
        }

        var hasTraversalAttack = path.contains("\0")

        // 1. Boundary & Protocol prefix detection
        var isUNC = false
        var isAbsolute = false

        if trimmed.hasPrefix("\\\\") || trimmed.hasPrefix("//") {
            isUNC = true
            isAbsolute = true
        } else if trimmed.hasPrefix("/") || trimmed.hasPrefix("\\") {
            isAbsolute = true
        }

        // 2. Windows drive letter check (e.g. C:, D:/, etc.)
        let utf8 = Array(trimmed.utf8)
        let hasDriveLetter = utf8.count >= 2
            && ((utf8[0] >= 0x41 && utf8[0] <= 0x5A) || (utf8[0] >= 0x61 && utf8[0] <= 0x7A))
            && utf8[1] == 0x3A /* ':' */

        if hasDriveLetter {
            isAbsolute = true
        }

        var containsReserved = trimmed.uppercased().contains("PHYSICALDRIVE")

        // 3. Fast Unicode NFC canonical normalization
        let nfcPath = trimmed.precomposedStringWithCanonicalMapping

        // 4. Single-pass slash/backslash segment splitting, ADS stripping, and ZipSlip stack normalization
        var cleanSegments: [String] = []
        cleanSegments.reserveCapacity(8)
        var strippedAds: String? = nil

        let rawSegments = nfcPath.split(omittingEmptySubsequences: true) { $0 == "/" || $0 == "\\" }

        for (segIdx, rawSegSubstring) in rawSegments.enumerated() {
            var seg = String(rawSegSubstring)
            if seg.isEmpty { continue }

            // NTFS Alternate Data Stream (ADS) identification & stripping
            let isDriveSeg = segIdx == 0 && hasDriveLetter && seg.count == 2 && seg.hasSuffix(":")
            if !isDriveSeg {
                if let colonIdx = seg.firstIndex(of: ":") {
                    let adsPart = String(seg[colonIdx...])
                    if strippedAds == nil {
                        strippedAds = adsPart
                    }
                    seg = String(seg[..<colonIdx])
                }
            }

            if seg.isEmpty { continue }

            // ZipSlip stack traversal check
            if seg == "." { continue }
            if seg == ".." {
                if !cleanSegments.isEmpty {
                    let lastIsDrive = cleanSegments.count == 1
                        && cleanSegments[0].count == 2
                        && cleanSegments[0].hasSuffix(":")
                    if lastIsDrive {
                        hasTraversalAttack = true
                    } else {
                        cleanSegments.removeLast()
                    }
                } else {
                    hasTraversalAttack = true
                }
                continue
            }

            // Check Windows reserved device names
            if !containsReserved && isWindowsReservedDeviceName(seg) {
                containsReserved = true
            }

            cleanSegments.append(seg)
        }

        let normalizedPath = cleanSegments.joined(separator: "/")
        let isLongPath = normalizedPath.utf16.count > 260

        // 5. Format Win32 path representation
        let win32Base = cleanSegments.joined(separator: "\\")
        let win32FormattedPath: String
        if isUNC {
            win32FormattedPath = "\\\\?\\UNC\\" + win32Base
        } else if isLongPath && isAbsolute {
            win32FormattedPath = "\\\\?\\" + win32Base
        } else {
            win32FormattedPath = win32Base
        }

        return PlatformPathNormalizationResult(
            originalPath: path,
            normalizedPath: normalizedPath,
            isAbsolute: isAbsolute,
            isUNCPath: isUNC,
            isLongPath: isLongPath,
            containsWindowsReservedDeviceName: containsReserved,
            strippedAlternateDataStream: strippedAds,
            win32FormattedPath: win32FormattedPath,
            hasTraversalAttack: hasTraversalAttack
        )
    }

    private static func isWindowsReservedDeviceName(_ segment: String) -> Bool {
        var s = segment
        while s.hasSuffix(" ") || s.hasSuffix(".") {
            s.removeLast()
        }
        if s.isEmpty { return false }

        let stem = s.split(separator: ".", maxSplits: 1, omittingEmptySubsequences: false).first.map(String.init) ?? s
        let trimmedStem = stem.trimmingCharacters(in: .whitespaces).uppercased()

        let exactNames: Set<String> = ["CON", "PRN", "AUX", "NUL", "CLOCK$"]
        if exactNames.contains(trimmedStem) {
            return true
        }

        if (trimmedStem.hasPrefix("COM") || trimmedStem.hasPrefix("LPT")) && trimmedStem.count == 4 {
            let lastChar = trimmedStem.last!
            if lastChar >= "0" && lastChar <= "9" {
                return true
            }
        }
        return false
    }
}

// MARK: - Monotonic Timer

//
//


/// Hardware timer calibration and resolution metadata.
public struct PlatformTimerCalibrationInfo: Sendable, Codable {
    public let platformOS: String
    public let architecture: String
    public let timerBackend: String
    public let frequencyHz: UInt64
    public let timebaseNumer: UInt32
    public let timebaseDenom: UInt32
    public let resolutionNanos: UInt64
    public let overheadNanos: UInt64

    public init(
        platformOS: String,
        architecture: String,
        timerBackend: String,
        frequencyHz: UInt64,
        timebaseNumer: UInt32,
        timebaseDenom: UInt32,
        resolutionNanos: UInt64,
        overheadNanos: UInt64
    ) {
        self.platformOS = platformOS
        self.architecture = architecture
        self.timerBackend = timerBackend
        self.frequencyHz = frequencyHz
        self.timebaseNumer = timebaseNumer
        self.timebaseDenom = timebaseDenom
        self.resolutionNanos = resolutionNanos
        self.overheadNanos = overheadNanos
    }
}

/// High-precision cross-platform monotonic timer and clock calibration service.
/// Conforms to TurboBench / lzbench nanosecond lock-free monotonic clock semantics.
public final class PlatformMonotonicTimer: Sendable {
    public static let shared = PlatformMonotonicTimer()

    private init() {}

    /// Explicitly initializes the timer subsystem and caches hardware frequency constants.
    @inline(__always)
    public static func initialize() {
        _ = shared
    }

    /// Current monotonic timestamp in nanoseconds (UInt64).
    @inline(__always)
    public static func nowNanoseconds() -> UInt64 {
        return clock_gettime_nsec_np(CLOCK_UPTIME_RAW)
    }

    /// Current monotonic timestamp in seconds (Double).
    @inline(__always)
    public static func nowSeconds() -> Double {
        return Double(nowNanoseconds()) / 1_000_000_000.0
    }

    /// Current raw hardware tick count.
    @inline(__always)
    public static func rawTicks() -> UInt64 {
        return mach_absolute_time()
    }

    /// Converts raw hardware tick differences to nanoseconds.
    @inline(__always)
    public static func ticksToNanoseconds(_ ticks: UInt64) -> UInt64 {
        var info = mach_timebase_info()
        mach_timebase_info(&info)
        return (ticks * UInt64(info.numer)) / UInt64(info.denom)
    }

    /// Converts raw hardware tick differences to seconds.
    @inline(__always)
    public static func ticksToSeconds(_ ticks: UInt64) -> Double {
        return Double(ticksToNanoseconds(ticks)) / 1_000_000_000.0
    }

    /// Hardware timer calibration and resolution metadata.
    public static func calibrationInfo() -> PlatformTimerCalibrationInfo {
        var info = mach_timebase_info()
        mach_timebase_info(&info)

        return PlatformTimerCalibrationInfo(
            platformOS: "macOS",
            architecture: "ARM64",
            timerBackend: "mach_continuous_time",
            frequencyHz: 1_000_000_000,
            timebaseNumer: info.numer,
            timebaseDenom: info.denom,
            resolutionNanos: 1,
            overheadNanos: 1
        )
    }

    /// Measures execution elapsed time for synchronous closures.
    @inline(__always)
    public static func measure<T>(_ block: () throws -> T) rethrows -> (result: T, elapsedNanos: UInt64, elapsedSeconds: Double) {
        let t0 = nowNanoseconds()
        let result = try block()
        let t1 = nowNanoseconds()
        let elapsedNanos = (t1 >= t0) ? (t1 - t0) : 1
        let elapsedSec = Double(elapsedNanos) / 1_000_000_000.0
        return (result, elapsedNanos, elapsedSec)
    }

    /// Measures execution elapsed time for asynchronous closures.
    @inline(__always)
    public static func measureAsync<T>(_ block: () async throws -> T) async rethrows -> (result: T, elapsedNanos: UInt64, elapsedSeconds: Double) {
        let t0 = nowNanoseconds()
        let result = try await block()
        let t1 = nowNanoseconds()
        let elapsedNanos = (t1 >= t0) ? (t1 - t0) : 1
        let elapsedSec = Double(elapsedNanos) / 1_000_000_000.0
        return (result, elapsedNanos, elapsedSec)
    }
}
