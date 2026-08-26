// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import XCTest
@testable import TTZipCore

final class HashCalculatorAndVaultTests: XCTestCase {
    
    private var tempDirectory: URL!
    
    override func setUp() {
        super.setUp()
        tempDirectory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try? FileManager.default.createDirectory(at: tempDirectory, withIntermediateDirectories: true)
    }
    
    override func tearDown() {
        if let tempDirectory {
            try? FileManager.default.removeItem(at: tempDirectory)
        }
        super.tearDown()
    }
    
    // MARK: - Task 3.4 Tests: HashCalculator & Hardware Acceleration
    
    func testHashCalculatorAllAlgorithms() throws {
        let testFile = tempDirectory.appendingPathComponent("sample_hash.txt")
        let content = "The quick brown fox jumps over the lazy dog"
        try content.write(to: testFile, atomically: true, encoding: .utf8)
        let filePath = testFile.path
        
        let calculator = HashCalculator()
        
        // 1. CRC32
        let crcResult = try calculator.computeHashSync(filePath: filePath, type: .crc32)
        XCTAssertEqual(crcResult.uppercased(), "414FA339")
        
        // 2. SHA-256
        let sha256Result = try calculator.computeHashSync(filePath: filePath, type: .sha256)
        XCTAssertEqual(sha256Result.lowercased(), "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592")
        
        // 3. SHA-1
        let sha1Result = try calculator.computeHashSync(filePath: filePath, type: .sha1)
        XCTAssertEqual(sha1Result.lowercased(), "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12")
        
        // 4. MD5
        let md5Result = try calculator.computeHashSync(filePath: filePath, type: .md5)
        XCTAssertEqual(md5Result.lowercased(), "9e107d9d372bb6826bd81d3542a419d6")
    }
    
    func testHashCalculatorAsync() async throws {
        let testFile = tempDirectory.appendingPathComponent("sample_async.txt")
        let content = "TTZip High-Performance Archiving Engine 2026"
        try content.write(to: testFile, atomically: true, encoding: .utf8)
        let filePath = testFile.path
        
        let calculator = HashCalculator()
        
        let sha256 = try await calculator.computeHash(filePath: filePath, type: .sha256)
        let crc32 = try await calculator.computeHash(filePath: filePath, type: .crc32)
        let md5 = try await calculator.computeHash(filePath: filePath, type: .md5)
        let sha1 = try await calculator.computeHash(filePath: filePath, type: .sha1)
        
        XCTAssertFalse(sha256.isEmpty)
        XCTAssertFalse(crc32.isEmpty)
        XCTAssertFalse(md5.isEmpty)
        XCTAssertFalse(sha1.isEmpty)
    }
    
    func testHardwareChecksumAdapter() {
        let sample = "123456789".data(using: .utf8)!
        let crc = HardwareChecksumAdapter.crc32(for: sample)
        XCTAssertEqual(crc, 0xCBF43926)
        
        let adler = HardwareChecksumAdapter.adler32(for: sample)
        XCTAssertEqual(adler, 0x091E01DE)
        
        let part1 = "12345".data(using: .utf8)!
        let part2 = "6789".data(using: .utf8)!
        let crc1 = HardwareChecksumAdapter.crc32(for: part1)
        let crc2 = HardwareChecksumAdapter.crc32(for: part2)
        let combined = HardwareChecksumAdapter.combineCRC32(crc1: crc1, crc2: crc2, len2: part2.count)
        XCTAssertEqual(combined, crc)
    }
    
    // MARK: - Task 1.4 Tests: Password Vault & Cryptographic Hardening
    
    func testVaultRustUniFFIDeriveAndEncryptDecryptRoundtrip() throws {
        let password = "SuperSecurePassword#2026"
        let salt = "FixedSaltForDeterministicTest".data(using: .utf8)!
        let iterations: UInt32 = 1000
        
        // 1. Derive key in Rust
        let keyData = try vaultDeriveKey(password: password, salt: salt, iterations: iterations)
        XCTAssertEqual(keyData.count, 32)
        
        // 2. Compute verifier in Rust
        let verifier = vaultComputeVerifier(key: keyData, salt: salt)
        XCTAssertEqual(verifier.count, 64)
        
        // 3. Encrypt payload in Rust
        let originalPayload = "ConfidentialCredentialDataPayload".data(using: .utf8)!
        let encrypted = try vaultEncryptPayload(key: keyData, payload: originalPayload)
        XCTAssertEqual(encrypted.count, 12 + originalPayload.count + 16)
        
        // 4. Decrypt payload in Rust
        let decrypted = try vaultDecryptPayload(key: keyData, encryptedData: encrypted)
        XCTAssertEqual(decrypted, originalPayload)
        
        // 5. Corrupted tag / wrong key should fail
        let wrongKey = Data(repeating: 0xFF, count: 32)
        XCTAssertThrowsError(try vaultDecryptPayload(key: wrongKey, encryptedData: encrypted))
    }
    
    func testPasswordVaultManagerIntegrationLifecycle() {
        let vaultURL = tempDirectory.appendingPathComponent("test_vault.enc")
        let configURL = tempDirectory.appendingPathComponent("test_config.json")
        let backupURL = tempDirectory.appendingPathComponent("test_backup.enc")
        
        let manager = PasswordVaultManager(vaultURL: vaultURL, configURL: configURL, backupURL: backupURL)
        
        XCTAssertFalse(manager.isUnlocked)
        XCTAssertFalse(manager.isMasterPasswordSet)
        
        // 1. Set Master Password
        let masterPassword = "TestMasterPassword@2026"
        manager.setMasterPassword(masterPassword)
        
        XCTAssertTrue(manager.isMasterPasswordSet)
        XCTAssertTrue(manager.isUnlocked)
        
        // 2. Add entries
        manager.addEntry(label: "GitHub", password: "gh_secret_token_123", category: "Dev")
        manager.addEntry(label: "ArchivePassword", password: "archive_pwd_456", category: "Archiving")
        
        let entries = manager.getEntries()
        XCTAssertEqual(entries.count, 2)
        XCTAssertEqual(entries[0].label, "GitHub")
        XCTAssertEqual(entries[1].label, "ArchivePassword")
        
        // 3. Lock Vault
        manager.lockVault()
        XCTAssertFalse(manager.isUnlocked)
        XCTAssertEqual(manager.getEntries().count, 0)
        
        // 4. Unlock with wrong password should fail
        let wrongUnlock = manager.unlockVault(with: "WrongPassword")
        XCTAssertFalse(wrongUnlock)
        XCTAssertFalse(manager.isUnlocked)
        
        // 5. Unlock with correct password should succeed and restore entries
        let correctUnlock = manager.unlockVault(with: masterPassword)
        XCTAssertTrue(correctUnlock)
        XCTAssertTrue(manager.isUnlocked)
        
        let restoredEntries = manager.getEntries()
        XCTAssertEqual(restoredEntries.count, 2)
        XCTAssertEqual(restoredEntries[0].password, "gh_secret_token_123")
        XCTAssertEqual(restoredEntries[1].password, "archive_pwd_456")
        
        // 6. Reset to first run state
        manager.resetToFirstRunState()
        XCTAssertFalse(manager.isUnlocked)
        XCTAssertFalse(manager.isMasterPasswordSet)
        XCTAssertEqual(manager.getEntries().count, 0)
    }
}
