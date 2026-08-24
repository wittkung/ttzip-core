// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

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

/// Multi-threaded password verification and recovery engine (100% Pure Mozilla UniFFI Engine).
public final class PasswordRecoveryEngine: Sendable {
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
    
    /// Probes archive stream password in-process without full extraction via UniFFI.
    public static func testArchivePassword(archivePath: String, password: String) async -> Bool {
        do {
            _ = try extractSingleEntryStream(archivePath: archivePath, entryIndex: 0, password: password)
            return true
        } catch {
            return false
        }
    }

    /// Fast in-memory dictionary recovery via native UniFFI stream decryption.
    public static func recoverFastInMemory(
        passwords: [String],
        archivePath: String
    ) -> String? {
        guard !passwords.isEmpty, FileManager.default.fileExists(atPath: archivePath) else {
            return nil
        }
        for pwd in passwords {
            if (try? extractSingleEntryStream(archivePath: archivePath, entryIndex: 0, password: pwd)) != nil {
                return pwd
            }
        }
        return nil
    }
}
