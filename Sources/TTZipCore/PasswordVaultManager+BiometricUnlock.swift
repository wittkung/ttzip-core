// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

extension PasswordVaultManager {
    /// Unlocks the vault from the biometric key produced by `readKey`.
    ///
    /// The stored key carries the keychain's `biometryCurrentSet` access control, so a change
    /// to the enrolled biometrics invalidates it: `readKey` then reports a failure and this
    /// request does not unlock the vault.
    func unlockWithBiometricsResult(readKey: () -> BiometricKeyRead) -> VaultBiometricUnlockResult {
        if vaultLock.withLock({ _isUnlocked && activeDerivedVaultKey != nil }) {
            return .unlocked
        }
        let keyResult = readKey()
        if Self.isCurrentTaskCancelled { return .cancelled }
        let result: VaultBiometricUnlockResult = vaultLock.withLock {
            if Self.isCurrentTaskCancelled { return .cancelled }
            if _isUnlocked && activeDerivedVaultKey != nil { return .unlocked }
            guard case .success(let vaultKeyData) = keyResult else {
                return Self.biometricUnlockFailure(for: keyResult)
            }
            return self.unlockWithBiometricKeyData(vaultKeyData)
        }
        if result == .unlocked {
            notifyChange()
        }
        return result
    }

    /// Maps a failed biometric key read onto its unlock outcome.
    private static func biometricUnlockFailure(for keyResult: BiometricKeyRead) -> VaultBiometricUnlockResult {
        switch keyResult {
        case .missing: return .keyMissing
        case .cancelled: return .cancelled
        case .unavailable: return .unavailable
        case .failure: return .keyAccessFailed
        case .success: return .invalidKey
        }
    }

    /// Validates the unwrapped biometric key and commits the unlocked vault state.
    ///
    /// The derived key must match the stored verifier before any vault entry is
    /// decrypted, and the decrypted entries must load before the vault is
    /// marked unlocked.
    private func unlockWithBiometricKeyData(_ vaultKeyData: Data) -> VaultBiometricUnlockResult {
        guard vaultKeyData.count == 32 else {
            return .invalidKey
        }
        guard let vaultKey = try? SecureBytes(data: vaultKeyData) else {
            TTLogger.error("Failed to allocate vaultKey SecureBytes during unlockWithBiometrics")
            return .keyAccessFailed
        }
        guard let expectedVerifier = masterVerifierHash, let salt = masterPasswordSalt,
              computeVerifierHash(vaultKey: vaultKey, salt: salt) == expectedVerifier else {
            vaultKey.wipeAndFree()
            return .invalidKey
        }
        guard let loadedEntries = loadVaultLocked(vaultKey: vaultKey) else {
            vaultKey.wipeAndFree()
            return .vaultUnreadable
        }
        if Self.isCurrentTaskCancelled {
            vaultKey.wipeAndFree()
            return .cancelled
        }
        self.entries = loadedEntries
        self.activeDerivedVaultKey = vaultKey
        self._isUnlocked = true
        return .unlocked
    }

    /// Reports whether the current task has already been cancelled.
    private static var isCurrentTaskCancelled: Bool {
        withUnsafeCurrentTask(body: { $0?.isCancelled ?? false })
    }
}
