// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import Security

extension PasswordVaultManager {
    /// Starts a fresh vault only after copying the existing encrypted vault and its verifier.
    /// The backup still requires the previous master password to read its entries.
    public func resetMasterPasswordPreservingEncryptedBackup(_ newPassword: String) throws -> URL {
        guard !newPassword.isEmpty else { throw VaultResetError.emptyPassword }
        let backupDirectory: URL = try vaultLock.withLock {
            guard !_isUnlocked else { throw VaultResetError.vaultUnlocked }
            try requireOriginalVaultFiles()
            let directory = try createVaultRecoveryDirectory()
            do {
                try copyOriginalVaultFiles(into: directory)
            } catch {
                try? FileManager.default.removeItem(at: directory)
                throw error
            }
            do {
                try installNewMasterPassword(newPassword, backupDirectory: directory)
            } catch {
                rollbackVaultReset(backupDirectory: directory)
                throw VaultResetError.newVaultNotSaved(backupDirectory: directory)
            }
            return directory
        }
        notifyChange()
        return backupDirectory
    }

    /// Rejects the reset when the vault or its config file is absent, because
    /// neither the old verifier nor the old salt could be recovered afterwards.
    private func requireOriginalVaultFiles() throws {
        guard FileManager.default.fileExists(atPath: vaultFileURL.path),
              FileManager.default.fileExists(atPath: configFileURL.path) else {
            throw VaultResetError.originalFilesMissing
        }
    }

    /// Creates a private recovery directory that holds the pre-reset artifacts.
    private func createVaultRecoveryDirectory() throws -> URL {
        let root = vaultFileURL.deletingLastPathComponent()
            .appendingPathComponent("VaultRecovery", isDirectory: true)
        let directory = root.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(
            at: directory,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: 0o700]
        )
        return directory
    }

    /// Copies the encrypted vault, its config and the optional backup into the recovery directory.
    private func copyOriginalVaultFiles(into directory: URL) throws {
        try FileManager.default.copyItem(
            at: vaultFileURL,
            to: directory.appendingPathComponent(vaultFileURL.lastPathComponent)
        )
        try FileManager.default.copyItem(
            at: configFileURL,
            to: directory.appendingPathComponent(configFileURL.lastPathComponent)
        )
        if FileManager.default.fileExists(atPath: backupFileURL.path) {
            try FileManager.default.copyItem(
                at: backupFileURL,
                to: directory.appendingPathComponent(backupFileURL.lastPathComponent)
            )
        }
    }

    /// Derives the new master secret and writes the fresh, empty vault.
    private func installNewMasterPassword(_ newPassword: String, backupDirectory directory: URL) throws {
        var saltBytes = [UInt8](repeating: 0, count: 32)
        guard SecRandomCopyBytes(kSecRandomDefault, saltBytes.count, &saltBytes) == errSecSuccess else {
            throw VaultResetError.newVaultNotSaved(backupDirectory: directory)
        }
        let salt = Data(saltBytes)
        guard let keyBytes = deriveSymmetricKeyBytesV4(
            newPassword,
            salt: salt,
            iterations: Self.defaultV4Iterations
        ), let key = try? SecureBytes(data: Data(keyBytes)),
              let secret = try? SecureBytes(utf8String: newPassword) else {
            throw VaultResetError.newVaultNotSaved(backupDirectory: directory)
        }
        masterPasswordSalt = salt
        activeDerivedVaultKey = key
        activeMasterSecret = secret
        masterVerifierHash = computeVerifierHash(vaultKey: key, salt: salt)
        entries = []
        _isUnlocked = true
        saveConfigLocked()
        saveVaultLocked()
        try verifyInstalledVault(key: key, backupDirectory: directory)
        saveBiometricVaultKey(vaultKey: key)
    }

    /// Confirms that the freshly written config and vault round-trip to the new verifier.
    private func verifyInstalledVault(key: SecureBytes, backupDirectory directory: URL) throws {
        guard let config = try? Data(contentsOf: configFileURL),
              let dictionary = try? JSONSerialization.jsonObject(with: config) as? [String: String],
              dictionary["verifierHash"] == masterVerifierHash,
              loadVaultLocked(vaultKey: key)?.isEmpty == true else {
            throw VaultResetError.newVaultNotSaved(backupDirectory: directory)
        }
    }

    /// Restores the pre-reset vault and config after a failed installation.
    private func rollbackVaultReset(backupDirectory directory: URL) {
        activeMasterSecret?.wipeAndFree()
        activeDerivedVaultKey?.wipeAndFree()
        activeMasterSecret = nil
        activeDerivedVaultKey = nil
        entries = []
        _isUnlocked = false
        masterPasswordSalt = nil
        masterVerifierHash = nil
        try? Data(contentsOf: directory.appendingPathComponent(vaultFileURL.lastPathComponent))
            .write(to: vaultFileURL, options: .atomic)
        try? Data(contentsOf: directory.appendingPathComponent(configFileURL.lastPathComponent))
            .write(to: configFileURL, options: .atomic)
        loadConfigInternal()
    }
}
