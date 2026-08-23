// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

import Foundation
import CTTZipBridge

/// Value type representing metrics and outcomes from password recovery exploration.
public struct PasswordRecoveryResult: Sendable {
    public let foundPassword: String?
    public let totalAttempts: Int64
    public let durationSeconds: Double
    
    public init(foundPassword: String?, totalAttempts: Int64, durationSeconds: Double) {
        self.foundPassword = foundPassword
        self.totalAttempts = totalAttempts
        self.durationSeconds = durationSeconds
    }
    
    public var attemptsPerSecond: Double {
        return durationSeconds > 0 ? Double(totalAttempts) / durationSeconds : 0
    }
}

/// Multi-threaded password verification and recovery engine.
///
/// Directly delegates to high-throughput multi-core Rust Rayon recovery pipelines.
public final class PasswordRecoveryEngine: @unchecked Sendable {
    public init() {}
    
    /// Tests dictionary candidate passwords against encrypted archive headers.
    public func recoverPassword(
        archivePath: String,
        dictionary: [String]
    ) async throws -> PasswordRecoveryResult {
        guard FileManager.default.fileExists(atPath: archivePath) else {
            throw ArchiveError.fileNotFound
        }
        
        let start = Date()
        let is7z = archivePath.lowercased().contains(".7z")
        if !is7z, let fastFound = Self.recoverFastInMemory(passwords: dictionary, archivePath: archivePath) {
            let duration = max(0.001, Date().timeIntervalSince(start))
            return PasswordRecoveryResult(
                foundPassword: fastFound,
                totalAttempts: Int64(dictionary.firstIndex(of: fastFound).map { $0 + 1 } ?? dictionary.count),
                durationSeconds: duration
            )
        }
        
        var attempts: Int64 = 0
        var foundPassword: String? = nil
        for pwd in dictionary {
            attempts += 1
            if await Self.testArchivePassword(archivePath: archivePath, password: pwd) {
                foundPassword = pwd
                break
            }
        }
        
        let duration = max(0.001, Date().timeIntervalSince(start))
        return PasswordRecoveryResult(
            foundPassword: foundPassword,
            totalAttempts: attempts,
            durationSeconds: duration
        )
    }
    
    /// Probes archive header and stream password in-process without full extraction.
    public static func testArchivePassword(archivePath: String, password: String) async -> Bool {
        if let fast = recoverFastInMemory(passwords: [password], archivePath: archivePath) {
            return fast == password
        }

        return CUnsafeBufferAdapter.withCString(archivePath) { cPath in
            CUnsafeBufferAdapter.withCString(password) { cPwd in
                guard let cPath = cPath, let cPwd = cPwd else { return false }
                var outBuffer = [UInt8](repeating: 0, count: 4096)
                var extractedLen: Int = 0
                let status = ttzip_rust_archive_extract_single_entry_memory(
                    cPath,
                    nil,
                    0,
                    cPwd,
                    &outBuffer,
                    outBuffer.count,
                    &extractedLen
                )
                return status == TTZIP_STATUS_OK
            }
        }
    }

    /// Fast in-memory multi-core dictionary recovery via native Rust C-ABI.
    public static func recoverFastInMemory(
        passwords: [String],
        archivePath: String
    ) -> String? {
        guard !passwords.isEmpty, FileManager.default.fileExists(atPath: archivePath) else {
            return nil
        }
        let cStrings = passwords.map { strdup($0) }
        defer {
            for ptr in cStrings {
                free(ptr)
            }
        }
        var outFound = [CChar](repeating: 0, count: 256)
        let ptrs = cStrings.map { UnsafePointer($0) }
        var attempts: UInt64 = 0
        
        return ptrs.withUnsafeBufferPointer { bufPtr -> String? in
            guard let basePtr = bufPtr.baseAddress else { return nil }
            return CUnsafeBufferAdapter.withCString(archivePath) { cPath in
                guard let cPath = cPath else { return nil }
                let status = ttzip_rust_password_recovery_start_dictionary(
                    cPath,
                    basePtr,
                    passwords.count,
                    nil,
                    &outFound,
                    outFound.count,
                    &attempts
                )
                if status == TTZIP_STATUS_OK {
                    return outFound.withUnsafeBufferPointer { ptr in
                        ptr.baseAddress.map { String(cString: $0) }
                    }
                }
                return nil
            }
        }
    }
}
