// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import XCTest
import Security
import LocalAuthentication
@testable import TTZipCore
#if canImport(TTZipCoreCrypto)
@testable import TTZipCoreCrypto
#endif

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
        XCTAssertEqual(manager.unlockVaultResult(with: "WrongPassword"), .invalidPassword)
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

    func testUnreadableVaultNeverEntersUnlockedStateOrReplacesCiphertext() throws {
        let vaultURL = tempDirectory.appendingPathComponent("damaged_vault.enc")
        let manager = PasswordVaultManager(
            vaultURL: vaultURL,
            configURL: tempDirectory.appendingPathComponent("damaged_config.json"),
            backupURL: tempDirectory.appendingPathComponent("damaged_backup.enc")
        )
        let password = "TaskOwnedMasterPassword@2026"
        manager.setMasterPassword(password)
        XCTAssertTrue(manager.isUnlocked)
        manager.lockVault()

        let damagedCiphertext = Data("damaged test vault".utf8)
        try damagedCiphertext.write(to: vaultURL, options: .atomic)
        XCTAssertEqual(manager.unlockVaultResult(with: password), .vaultUnreadable)
        XCTAssertFalse(manager.isUnlocked)
        XCTAssertTrue(manager.getEntries().isEmpty)
        XCTAssertEqual(try Data(contentsOf: vaultURL), damagedCiphertext)
        XCTAssertNotEqual(manager.unlockWithBiometricsResult(), .unlocked)
        XCTAssertFalse(manager.isUnlocked)
    }

    func testMissingConfigDoesNotInitializeOverExistingVault() throws {
        let vaultURL = tempDirectory.appendingPathComponent("existing_vault.enc")
        let configURL = tempDirectory.appendingPathComponent("existing_config.json")
        let manager = PasswordVaultManager(
            vaultURL: vaultURL,
            configURL: configURL,
            backupURL: tempDirectory.appendingPathComponent("existing_backup.enc")
        )
        manager.setMasterPassword("TaskOwnedMasterPassword@2026")
        manager.lockVault()
        let ciphertext = try Data(contentsOf: vaultURL)
        try FileManager.default.removeItem(at: configURL)

        let reopened = PasswordVaultManager(vaultURL: vaultURL, configURL: configURL)
        XCTAssertTrue(reopened.isMasterPasswordSet)
        XCTAssertEqual(reopened.unlockVaultResult(with: "TaskOwnedMasterPassword@2026"), .configurationMissing)
        XCTAssertFalse(reopened.isUnlocked)
        XCTAssertEqual(try Data(contentsOf: vaultURL), ciphertext)
        XCTAssertFalse(FileManager.default.fileExists(atPath: configURL.path))
    }

    func testResetPreservesEncryptedOriginalBeforeCreatingEmptyVault() throws {
        let vaultURL = tempDirectory.appendingPathComponent("reset_vault.enc")
        let configURL = tempDirectory.appendingPathComponent("reset_config.json")
        let manager = PasswordVaultManager(vaultURL: vaultURL, configURL: configURL)
        manager.setMasterPassword("OldTaskOwnedMaster@2026")
        manager.addEntry(label: "old entry", password: "task-owned-secret")
        let oldEntries = manager.getEntries()
        manager.lockVault()
        manager.vaultLock.withLock { manager.entries = oldEntries }
        let oldVault = try Data(contentsOf: vaultURL)
        let oldConfig = try Data(contentsOf: configURL)

        let backup = try manager.resetMasterPasswordPreservingEncryptedBackup("NewTaskOwnedMaster@2026")
        XCTAssertEqual(try Data(contentsOf: backup.appendingPathComponent(vaultURL.lastPathComponent)), oldVault)
        XCTAssertEqual(try Data(contentsOf: backup.appendingPathComponent(configURL.lastPathComponent)), oldConfig)
        XCTAssertTrue(manager.isUnlocked)
        XCTAssertTrue(manager.getEntries().isEmpty)

        manager.lockVault()
        XCTAssertEqual(manager.unlockVaultResult(with: "OldTaskOwnedMaster@2026"), .invalidPassword)
        XCTAssertEqual(manager.unlockVaultResult(with: "NewTaskOwnedMaster@2026"), .unlocked)
        XCTAssertTrue(manager.getEntries().isEmpty)
    }

    func testVerifiedEntryMutationsReportPersistenceAndLeaveMemoryUnchangedOnFailure() throws {
        let vaultURL = tempDirectory.appendingPathComponent("verified_entries.enc")
        let configURL = tempDirectory.appendingPathComponent("verified_entries.json")
        let manager = PasswordVaultManager(vaultURL: vaultURL, configURL: configURL)
        let id = UUID()
        XCTAssertEqual(manager.addEntryVerified(id: id, label: "archive", password: "task-owned-secret"), .locked)
        manager.setMasterPassword("TaskOwnedMaster@2026")
        XCTAssertEqual(manager.addEntryVerified(id: id, label: "archive", password: "task-owned-secret"), .saved(id))
        XCTAssertEqual(manager.addEntryVerified(id: id, label: "archive", password: "task-owned-secret"), .duplicateID)
        XCTAssertEqual(manager.removeEntryVerified(id: UUID()), .notFound)
        let secondID = UUID()
        XCTAssertEqual(manager.addEntryVerified(id: secondID, label: "second", password: "task-owned-secret-2"), .saved(secondID))
        XCTAssertEqual(manager.removeEntryVerified(id: secondID), .removed)
        manager.lockVault()
        XCTAssertEqual(manager.unlockVaultResult(with: "TaskOwnedMaster@2026"), .unlocked)
        XCTAssertEqual(manager.getEntries().map(\.id), [id])

        let externallyChangedCiphertext = Data("external change for test".utf8)
        try externallyChangedCiphertext.write(to: vaultURL, options: .atomic)
        XCTAssertEqual(manager.removeEntryVerified(id: id), .writeFailed)
        XCTAssertEqual(manager.getEntries().map(\.id), [id])
        XCTAssertEqual(try Data(contentsOf: vaultURL), externallyChangedCiphertext)
        let useCount = manager.getEntries()[0].useCount
        manager.recordUsage(id: id)
        XCTAssertEqual(manager.getEntries()[0].useCount, useCount)
        XCTAssertEqual(try Data(contentsOf: vaultURL), externallyChangedCiphertext)
    }

    func testBiometricKeyReadAndVaultDecryptionAreSeparateGates() throws {
        XCTAssertTrue(PasswordVaultManager.isCLIProcess)
        XCTAssertFalse(PasswordVaultManager.isCLIProcess(
            bundleIdentifier: "com.metastudyline.ttzip",
            processName: "TTZip",
            arguments: ["/tmp/test-build/TTZip.app/Contents/MacOS/TTZip"]
        ))
        XCTAssertTrue(PasswordVaultManager.isCLIProcess(
            bundleIdentifier: "org.example.TTZipTests",
            processName: "TTZipTests",
            arguments: ["/tmp/TTZipTests.xctest"]
        ))
        let vaultURL = tempDirectory.appendingPathComponent("biometric_vault.enc")
        let manager = PasswordVaultManager(
            vaultURL: vaultURL,
            configURL: tempDirectory.appendingPathComponent("biometric_config.json"),
            backupURL: tempDirectory.appendingPathComponent("biometric_backup.enc")
        )
        manager.setMasterPassword("TaskOwnedMasterPassword@2026")
        manager.addEntry(label: "test archive", password: "task-owned-secret")
        let keyData = try XCTUnwrap(manager.activeDerivedVaultKey).withUnsafeBytes { Data($0) }
        manager.lockVault()

        XCTAssertEqual(manager.unlockWithBiometricsResult { .missing }, .keyMissing)
        XCTAssertEqual(manager.unlockWithBiometricsResult { .cancelled }, .cancelled)
        XCTAssertEqual(manager.unlockWithBiometricsResult { .unavailable }, .unavailable)
        XCTAssertEqual(manager.unlockWithBiometricsResult { .failure(errSecAuthFailed) }, .keyAccessFailed)
        XCTAssertEqual(manager.unlockWithBiometricsResult { .success(Data(repeating: 0, count: 32)) }, .invalidKey)
        XCTAssertFalse(manager.isUnlocked)

        XCTAssertEqual(manager.unlockWithBiometricsResult { .success(keyData) }, .unlocked)
        XCTAssertTrue(manager.isUnlocked)
        XCTAssertEqual(manager.getEntries().count, 1)
        var repeatedRead = false
        XCTAssertEqual(manager.unlockWithBiometricsResult {
            repeatedRead = true
            return .missing
        }, .unlocked)
        XCTAssertFalse(repeatedRead)
        manager.lockVault()

        let damagedCiphertext = Data("damaged biometric test vault".utf8)
        try damagedCiphertext.write(to: vaultURL, options: .atomic)
        XCTAssertEqual(manager.unlockWithBiometricsResult { .success(keyData) }, .vaultUnreadable)
        XCTAssertFalse(manager.isUnlocked)
        XCTAssertEqual(try Data(contentsOf: vaultURL), damagedCiphertext)
    }

    func testCancelPendingBiometricAuthenticationInvalidatesContextAndPreventsLateUnlock() throws {
        let vaultURL = tempDirectory.appendingPathComponent("biometric_cancel_vault.enc")
        let manager = PasswordVaultManager(
            vaultURL: vaultURL,
            configURL: tempDirectory.appendingPathComponent("biometric_cancel_config.json"),
            backupURL: tempDirectory.appendingPathComponent("biometric_cancel_backup.enc")
        )
        manager.setMasterPassword("TaskOwnedMasterPassword@2026")
        manager.addEntry(label: "test archive", password: "task-owned-secret")
        let keyData = try XCTUnwrap(manager.activeDerivedVaultKey).withUnsafeBytes { Data($0) }
        manager.lockVault()

        let mockContext = MockBiometricAuthenticationContext()
        let readEntered = DispatchSemaphore(value: 0)
        let releaseRead = DispatchSemaphore(value: 0)
        let unlockCompleted = DispatchSemaphore(value: 0)

        manager.makeBiometricContext = { mockContext }
        manager.biometricKeyReaderOverride = { context in
            XCTAssertTrue((context as? MockBiometricAuthenticationContext) === mockContext)
            XCTAssertFalse(context.localizedReason.isEmpty)
            readEntered.signal()
            _ = releaseRead.wait(timeout: .now() + 5)
            return .success(keyData)
        }

        let resultBox = LockedResultBox()
        DispatchQueue.global(qos: .userInitiated).async {
            let outcome = manager.unlockWithBiometricsResult()
            resultBox.store(outcome)
            unlockCompleted.signal()
        }

        XCTAssertEqual(readEntered.wait(timeout: .now() + 5), .success)
        XCTAssertEqual(mockContext.invalidateCallCount, 0)

        manager.cancelPendingBiometricAuthentication()
        XCTAssertEqual(mockContext.invalidateCallCount, 1)

        releaseRead.signal()
        XCTAssertEqual(unlockCompleted.wait(timeout: .now() + 5), .success)
        XCTAssertEqual(resultBox.load(), .cancelled)
        XCTAssertFalse(manager.isUnlocked)
        XCTAssertTrue(manager.getEntries().isEmpty)
        XCTAssertNil(manager.activeBiometricContext)
    }

    func testLockVaultInvalidatesActiveBiometricContextAndPreventsLateUnlock() throws {
        let vaultURL = tempDirectory.appendingPathComponent("biometric_lock_vault.enc")
        let manager = PasswordVaultManager(
            vaultURL: vaultURL,
            configURL: tempDirectory.appendingPathComponent("biometric_lock_config.json"),
            backupURL: tempDirectory.appendingPathComponent("biometric_lock_backup.enc")
        )
        manager.setMasterPassword("TaskOwnedMasterPassword@2026")
        manager.addEntry(label: "test archive", password: "task-owned-secret")
        let keyData = try XCTUnwrap(manager.activeDerivedVaultKey).withUnsafeBytes { Data($0) }
        manager.lockVault()

        let mockContext = MockBiometricAuthenticationContext()
        let readEntered = DispatchSemaphore(value: 0)
        let releaseRead = DispatchSemaphore(value: 0)
        let unlockCompleted = DispatchSemaphore(value: 0)

        manager.makeBiometricContext = { mockContext }
        manager.biometricKeyReaderOverride = { _ in
            readEntered.signal()
            _ = releaseRead.wait(timeout: .now() + 5)
            return .success(keyData)
        }

        let resultBox = LockedResultBox()
        DispatchQueue.global(qos: .userInitiated).async {
            resultBox.store(manager.unlockWithBiometricsResult())
            unlockCompleted.signal()
        }

        XCTAssertEqual(readEntered.wait(timeout: .now() + 5), .success)
        manager.lockVault()
        XCTAssertEqual(mockContext.invalidateCallCount, 1)

        releaseRead.signal()
        XCTAssertEqual(unlockCompleted.wait(timeout: .now() + 5), .success)
        XCTAssertEqual(resultBox.load(), .cancelled)
        XCTAssertFalse(manager.isUnlocked)
        XCTAssertTrue(manager.getEntries().isEmpty)
    }

    func testIsCLIProcessRecognizesDevAppBundleAndExcludesVerificationCopies() {
        XCTAssertFalse(PasswordVaultManager.isCLIProcess(
            bundleIdentifier: "com.metastudyline.ttzip.dev",
            processName: "TTZip",
            arguments: ["/Applications/TTZip.app/Contents/MacOS/TTZip"]
        ))
        XCTAssertFalse(PasswordVaultManager.isCLIProcess(
            bundleIdentifier: "com.ttzip.app.dev",
            processName: "TTZip",
            arguments: ["/Applications/TTZip.app/Contents/MacOS/TTZip"]
        ))
        XCTAssertTrue(PasswordVaultManager.isCLIProcess(
            bundleIdentifier: "com.metastudyline.ttzip.cuverify-1234abcd",
            processName: "TTZip",
            arguments: ["/tmp/instance/TTZip.app/Contents/MacOS/TTZip"]
        ))
        XCTAssertTrue(PasswordVaultManager.isCLIProcess(
            bundleIdentifier: "com.metastudyline.ttzip.axverify-deadbeef",
            processName: "TTZip",
            arguments: ["/tmp/instance/TTZip.app/Contents/MacOS/TTZip"]
        ))
        XCTAssertTrue(PasswordVaultManager.isCLIProcess(
            bundleIdentifier: "com.metastudyline.ttzip.dev",
            processName: "TTZip",
            arguments: ["/Applications/TTZip.app/Contents/MacOS/TTZip", "--cli"]
        ))
        XCTAssertTrue(PasswordVaultManager.isCLIProcess(
            bundleIdentifier: "com.metastudyline.ttzip.dev",
            processName: "TTZip",
            arguments: ["/Applications/TTZip.app/Contents/MacOS/TTZip"],
            environment: ["XCTestConfigurationFilePath": "/tmp/test.xctestconfiguration"]
        ))
    }

    func testAdHocMissingEntitlementFallbackStoresInLoginKeychainAndEnforcesBiometricPolicy() throws {
        let vaultURL = tempDirectory.appendingPathComponent("adhoc_vault.enc")
        let manager = PasswordVaultManager(
            vaultURL: vaultURL,
            configURL: tempDirectory.appendingPathComponent("adhoc_config.json"),
            backupURL: tempDirectory.appendingPathComponent("adhoc_backup.enc")
        )
        let keychain = MockLoginKeychainFallbackStore()
        manager.customKeychainOperations = keychain.operations

        manager.setMasterPassword("AdHocMaster@2026")
        manager.addEntry(label: "dev archive", password: "adhoc-secret-password")
        XCTAssertEqual(keychain.accessControlAddAttempts, 1)
        XCTAssertEqual(keychain.loginKeychainSaveCount, 1)
        manager.lockVault()

        let policyEvaluated = LockedCounterBox()
        manager.biometricPolicyEvaluatorOverride = { _, reason in
            XCTAssertFalse(reason.isEmpty)
            policyEvaluated.increment()
            return .failure(LAError(.userCancel))
        }
        XCTAssertEqual(manager.unlockWithBiometricsResult(), .cancelled)
        XCTAssertEqual(policyEvaluated.load(), 1)
        XCTAssertFalse(manager.isUnlocked)

        manager.biometricPolicyEvaluatorOverride = { _, _ in
            policyEvaluated.increment()
            return .failure(LAError(.biometryNotAvailable))
        }
        XCTAssertEqual(manager.unlockWithBiometricsResult(), .unavailable)

        manager.biometricPolicyEvaluatorOverride = { _, _ in
            policyEvaluated.increment()
            return .failure(LAError(.authenticationFailed))
        }
        XCTAssertEqual(manager.unlockWithBiometricsResult(), .keyAccessFailed)

        manager.biometricPolicyEvaluatorOverride = { _, _ in
            policyEvaluated.increment()
            return .success(())
        }
        XCTAssertEqual(manager.unlockWithBiometricsResult(), .unlocked)
        XCTAssertTrue(manager.isUnlocked)
        XCTAssertEqual(manager.getEntries().map(\.password), ["adhoc-secret-password"])
    }

    func testDefaultTouchIDBootstrapsUninitializedAndEmptyVaultsWithoutOverwritingNonEmptyVault() throws {
        let freshVaultURL = tempDirectory.appendingPathComponent("fresh_touchid_vault.enc")
        let freshManager = PasswordVaultManager(
            vaultURL: freshVaultURL,
            configURL: tempDirectory.appendingPathComponent("fresh_touchid_config.json"),
            backupURL: tempDirectory.appendingPathComponent("fresh_touchid_backup.enc")
        )
        let keychain = MockLoginKeychainFallbackStore()
        freshManager.customKeychainOperations = keychain.operations
        let biometricChecks = LockedCounterBox()
        freshManager.biometricPolicyEvaluatorOverride = { _, _ in
            biometricChecks.increment()
            return .success(())
        }

        XCTAssertFalse(freshManager.isMasterPasswordSet)
        XCTAssertEqual(freshManager.unlockWithBiometricsResult(), .unlocked)
        XCTAssertTrue(freshManager.isUnlocked)
        XCTAssertTrue(freshManager.isMasterPasswordSet)
        XCTAssertEqual(biometricChecks.load(), 1)
        XCTAssertEqual(keychain.loginKeychainSaveCount, 1)

        freshManager.addEntry(label: "bootstrapped", password: "touchid-first-secret")
        freshManager.lockVault()
        XCTAssertFalse(freshManager.isUnlocked)

        XCTAssertEqual(freshManager.unlockWithBiometricsResult(), .unlocked)
        XCTAssertEqual(freshManager.getEntries().map(\.password), ["touchid-first-secret"])

        let emptyVaultURL = tempDirectory.appendingPathComponent("empty_missing_key_vault.enc")
        let emptyManager = PasswordVaultManager(
            vaultURL: emptyVaultURL,
            configURL: tempDirectory.appendingPathComponent("empty_missing_key_config.json"),
            backupURL: tempDirectory.appendingPathComponent("empty_missing_key_backup.enc")
        )
        emptyManager.setMasterPassword("InitialMaster@2026")
        emptyManager.lockVault()
        XCTAssertTrue(emptyManager.canBootstrapBiometricVault)
        XCTAssertEqual(emptyManager.unlockWithBiometricsResult { .missing }, .unlocked)
        XCTAssertTrue(emptyManager.isUnlocked)
    }

    func testLoginKeychainFallbackAndBootstrapHonorMidFlightBiometricCancellation() throws {
        let vaultURL = tempDirectory.appendingPathComponent("cancel_fallback_vault.enc")
        let manager = PasswordVaultManager(
            vaultURL: vaultURL,
            configURL: tempDirectory.appendingPathComponent("cancel_fallback_config.json"),
            backupURL: tempDirectory.appendingPathComponent("cancel_fallback_backup.enc")
        )
        let keychain = MockLoginKeychainFallbackStore()
        let mockContext = MockBiometricAuthenticationContext()
        manager.makeBiometricContext = { mockContext }
        manager.customKeychainOperations = keychain.operations
        manager.setMasterPassword("FallbackCancel@2026")
        manager.lockVault()

        let evalCalls = LockedCounterBox()
        manager.biometricPolicyEvaluatorOverride = { [weak manager] _, _ in
            evalCalls.increment()
            manager?.cancelPendingBiometricAuthentication()
            return .success(())
        }
        XCTAssertEqual(manager.unlockWithBiometricsResult(), .cancelled)
        XCTAssertEqual(evalCalls.load(), 1)
        XCTAssertGreaterThanOrEqual(mockContext.invalidateCallCount, 1)
        XCTAssertFalse(manager.isUnlocked)

        let bootstrapManager = PasswordVaultManager(
            vaultURL: tempDirectory.appendingPathComponent("cancel_bootstrap_vault.enc"),
            configURL: tempDirectory.appendingPathComponent("cancel_bootstrap_config.json"),
            backupURL: tempDirectory.appendingPathComponent("cancel_bootstrap_backup.enc")
        )
        let bootstrapChecks = LockedCounterBox()
        bootstrapManager.biometricPolicyEvaluatorOverride = { _, _ in
            bootstrapChecks.increment()
            return .success(())
        }
        let bootstrapResult = bootstrapManager.unlockWithBiometricsResult { [weak bootstrapManager] in
            bootstrapManager?.cancelPendingBiometricAuthentication()
            return .missing
        }
        XCTAssertEqual(bootstrapResult, .cancelled)
        XCTAssertEqual(bootstrapChecks.load(), 0)
        XCTAssertFalse(bootstrapManager.isUnlocked)
    }

    func testCompressCommandBacksUpAndRestoresOutputPathContainingBakSubstring() async throws {
        let bakParent = tempDirectory.appendingPathComponent("dir.bak_workspace")
        try FileManager.default.createDirectory(at: bakParent, withIntermediateDirectories: true)

        let sourceFile = bakParent.appendingPathComponent("payload.txt")
        try Data("new payload for compression".utf8).write(to: sourceFile)

        let outputArchive = bakParent.appendingPathComponent("project.bak_2026.zip")
        let originalSentinel = Data("ORIGINAL_USER_ARCHIVE_SENTINEL".utf8)
        try originalSentinel.write(to: outputArchive)

        let command = CompressCommand(
            inputs: [sourceFile.path],
            outputPath: outputArchive.path,
            format: .zip
        )
        _ = try await command.execute()
        XCTAssertNotEqual(try Data(contentsOf: outputArchive), originalSentinel)

        try await command.undo()
        XCTAssertEqual(try Data(contentsOf: outputArchive), originalSentinel)
    }

    func testArchiveWriterDoesNotLeakStagedOptionsWhenOverrideExecutorIsUsed() throws {
        let sourceFile = tempDirectory.appendingPathComponent("writer_input.txt")
        try Data(repeating: 0x41, count: 4096).write(to: sourceFile)
        let dummyOut = tempDirectory.appendingPathComponent("dummy_override.zip")
        let realOut = tempDirectory.appendingPathComponent("real_after_override.zst")

        let writer = ArchiveWriter()
        writer._createArchiveExecutorOverride = { _, _, _, _ in
            CompressionReport(
                uncompressedBytes: 4096,
                compressedBytes: 128,
                elapsedNanos: 1000,
                throughputMbs: 10.0,
                spaceSavingsPct: 90.0,
                engineProvenance: "mock",
                outcome: nil
            )
        }

        let recoveryOpts = ArchiveAdvancedOptions(
            dictionarySizeMB: 32,
            cpuThreads: 2,
            enableSolidArchive: false,
            encryptFileNames: false,
            zipEncryptionMethod: "ZipCrypto",
            zstdLevel: 15,
            zstdEnableLDM: true,
            recoveryRecordRedundancyPercent: 10
        )
        _ = try writer.createArchiveSyncWithOutcome(
            outputPath: dummyOut.path,
            format: .zip,
            level: .normal,
            inputPaths: [sourceFile.path],
            advancedOptions: recoveryOpts
        )

        writer._createArchiveExecutorOverride = nil
        let zstdOpts = ArchiveAdvancedOptions(
            dictionarySizeMB: 16,
            cpuThreads: 2,
            enableSolidArchive: true,
            encryptFileNames: false,
            zipEncryptionMethod: "AES-256",
            zstdLevel: 12,
            zstdEnableLDM: true,
            recoveryRecordRedundancyPercent: 0
        )
        let report = try writer.createArchiveSyncWithOutcome(
            outputPath: realOut.path,
            format: .zst,
            level: .normal,
            inputPaths: [sourceFile.path],
            advancedOptions: zstdOpts
        )
        XCTAssertGreaterThan(report.compressedBytes, 0)
        XCTAssertTrue(FileManager.default.fileExists(atPath: realOut.path))
    }

    func testImageAndDocumentServicesPreserveTotalsAcrossOperations() async throws {
        let imgService = TTZipImageRenderingService()
        let pngBytes = Data([
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
            0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
            0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41,
            0x54, 0x78, 0x9C, 0x63, 0x60, 0x60, 0x60, 0x60,
            0x00, 0x00, 0x00, 0x05, 0x00, 0x01, 0x87, 0xA1,
            0x49, 0x48, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
            0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82
        ])
        _ = try await imgService.probeMetadata(data: pngBytes, fileName: "one.png")
        XCTAssertEqual(imgService.totalImagesProcessed, 1)
        _ = try await imgService.probeMetadata(data: pngBytes, fileName: "two.png")
        XCTAssertEqual(imgService.totalImagesProcessed, 2)
        XCTAssertEqual(imgService.activeOperationsCount, 0)
        XCTAssertFalse(imgService.isProcessing)

        let docService = TTZipDocumentMetadataService()
        let plistBytes = Data("""
        <?xml version="1.0" encoding="UTF-8"?>
        <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
        <plist version="1.0"><dict><key>CFBundleName</key><string>TTZip</string></dict></plist>
        """.utf8)
        try await withThrowingTaskGroup(of: Void.self) { group in
            for _ in 0..<16 {
                group.addTask {
                    _ = try await docService.inspectPlist(data: plistBytes)
                }
            }
            try await group.waitForAll()
        }
        XCTAssertEqual(docService.totalDocumentsInspected, 16)
        XCTAssertEqual(docService.activeOperationsCount, 0)
        XCTAssertFalse(docService.isProcessing)
    }
}


