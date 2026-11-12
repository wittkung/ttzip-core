// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import Security
import LocalAuthentication

extension PasswordVaultManager {
    /// Whether the vault has no encrypted entries and can be bootstrapped directly via Touch ID.
    public var canBootstrapBiometricVault: Bool {
        vaultLock.withLock { canBootstrapBiometricVaultLocked() }
    }

    /// Whether test biometric hooks are installed on this manager instance.
    public var hasBiometricTestOverrides: Bool {
        biometricContextLock.withLock {
            biometricKeyReaderOverride != nil
                || biometricPolicyEvaluatorOverride != nil
                || customKeychainOperations != nil
        }
    }

    /// Runs hardware biometric unlock on a detached background task and cancels any in-flight
    /// authentication context immediately if the calling task is cancelled.
    public func unlockWithBiometricsAsyncResult() async -> VaultBiometricUnlockResult {
        if vaultLock.withLock({ _isUnlocked && activeDerivedVaultKey != nil }) {
            return .unlocked
        }
        if Self.isCurrentTaskCancelled {
            cancelPendingBiometricAuthentication()
            return .cancelled
        }
        let operation = Task.detached(priority: .userInitiated) { [self] in
            self.unlockWithBiometricsResult()
        }
        return await withTaskCancellationHandler {
            await operation.value
        } onCancel: { [self] in
            operation.cancel()
            self.cancelPendingBiometricAuthentication()
        }
    }

    /// Unlocks the vault from the biometric key produced by `readKey`, or bootstraps a fresh
    /// 256-bit biometric vault key after hardware biometric verification when the vault has no
    /// master password configured or contains zero encrypted entries.
    package func unlockWithBiometricsResult(readKey: () -> BiometricKeyRead) -> VaultBiometricUnlockResult {
        if vaultLock.withLock({ _isUnlocked && activeDerivedVaultKey != nil }) {
            return .unlocked
        }
        let startGeneration = biometricContextLock.withLock { biometricUnlockGeneration }
        let keyResult = readKey()
        if Self.isCurrentTaskCancelled || !isBiometricUnlockGenerationCurrent(startGeneration) {
            return .cancelled
        }
        if keyResult == .missing, vaultLock.withLock({ canBootstrapBiometricVaultLocked() }) {
            return bootstrapWithBiometricsResult(startGeneration: startGeneration)
        }
        let result: VaultBiometricUnlockResult = vaultLock.withLock {
            if Self.isCurrentTaskCancelled || !isBiometricUnlockGenerationCurrent(startGeneration) {
                return .cancelled
            }
            if _isUnlocked && activeDerivedVaultKey != nil { return .unlocked }
            guard case .success(let vaultKeyData) = keyResult else {
                return Self.biometricUnlockFailure(for: keyResult)
            }
            return self.unlockWithBiometricKeyData(vaultKeyData, startGeneration: startGeneration)
        }
        if result == .unlocked {
            notifyChange()
        }
        return result
    }

    func canBootstrapBiometricVaultLocked() -> Bool {
        if masterVerifierHash == nil && !FileManager.default.fileExists(atPath: vaultFileURL.path) {
            return true
        }
        return isEmptyEncryptedVaultOnDiskLocked()
    }

    private func isEmptyEncryptedVaultOnDiskLocked() -> Bool {
        guard masterVerifierHash != nil, masterPasswordSalt != nil else { return false }
        if !FileManager.default.fileExists(atPath: vaultFileURL.path) {
            return true
        }
        guard let encryptedData = try? Data(contentsOf: vaultFileURL),
              encryptedData.count >= 69,
              encryptedData.prefix(4) == Self.vaultMagicV4 else {
            return false
        }
        let saltLen = Int(encryptedData[8])
        let overhead = 9 + saltLen + 28
        guard encryptedData.count >= overhead else { return false }
        let plaintextLength = encryptedData.count - overhead
        return plaintextLength <= 8
    }

    private func bootstrapWithBiometricsResult(startGeneration: UInt64) -> VaultBiometricUnlockResult {
        let context = makeBiometricContext()
        let reason = TTZipLocalizationManager.shared.string(for: L10n.Vault.biometricReason)
        context.localizedReason = reason
        let registered = biometricContextLock.withLock { () -> Bool in
            guard !Self.isCurrentTaskCancelled, biometricUnlockGeneration == startGeneration else {
                return false
            }
            activeBiometricContext = context
            return true
        }
        guard registered else {
            return .cancelled
        }
        defer {
            biometricContextLock.withLock {
                if activeBiometricContext === context {
                    activeBiometricContext = nil
                }
            }
        }
        if let authFailure = evaluateExplicitBiometricAuth(
            context: context,
            prompt: reason,
            startGeneration: startGeneration
        ) {
            return Self.biometricUnlockFailure(for: authFailure)
        }
        if Self.isCurrentTaskCancelled || !isBiometricUnlockGenerationCurrent(startGeneration) {
            return .cancelled
        }
        let result: VaultBiometricUnlockResult = vaultLock.withLock {
            if Self.isCurrentTaskCancelled || !isBiometricUnlockGenerationCurrent(startGeneration) {
                return .cancelled
            }
            if _isUnlocked && activeDerivedVaultKey != nil { return .unlocked }
            guard canBootstrapBiometricVaultLocked() else { return .keyMissing }
            return bootstrapBiometricVaultLocked(startGeneration: startGeneration)
        }
        if result == .unlocked {
            notifyChange()
        }
        return result
    }

    private func bootstrapBiometricVaultLocked(startGeneration: UInt64) -> VaultBiometricUnlockResult {
        var saltBytes = [UInt8](repeating: 0, count: 32)
        guard SecRandomCopyBytes(kSecRandomDefault, saltBytes.count, &saltBytes) == errSecSuccess else {
            return .keyAccessFailed
        }
        var keyBytes = [UInt8](repeating: 0, count: 32)
        guard SecRandomCopyBytes(kSecRandomDefault, keyBytes.count, &keyBytes) == errSecSuccess else {
            return .keyAccessFailed
        }
        defer {
            for index in keyBytes.indices {
                keyBytes[index] = 0
            }
        }
        guard let vaultKey = try? SecureBytes(data: Data(keyBytes)) else {
            return .keyAccessFailed
        }
        if Self.isCurrentTaskCancelled || !isBiometricUnlockGenerationCurrent(startGeneration) {
            vaultKey.wipeAndFree()
            return .cancelled
        }
        let salt = Data(saltBytes)
        let verifier = computeVerifierHash(vaultKey: vaultKey, salt: salt)
        self.masterPasswordSalt = salt
        self.masterVerifierHash = verifier
        self.activeMasterSecret?.wipeAndFree()
        self.activeMasterSecret = nil
        self.activeDerivedVaultKey?.wipeAndFree()
        self.activeDerivedVaultKey = vaultKey
        self.entries = []
        self._isUnlocked = true
        saveConfigLocked()
        saveVaultLocked()
        saveBiometricVaultKey(vaultKey: vaultKey)
        return .unlocked
    }

    func saveBiometricVaultKeyData(_ keyData: Data, ops: KeychainOperations) {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: biometricService,
            kSecAttrAccount as String: biometricAccountKey
        ]
        var accessError: Unmanaged<CFError>?
        guard let accessControl = SecAccessControlCreateWithFlags(
            kCFAllocatorDefault,
            kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
            .biometryCurrentSet,
            &accessError
        ) else {
            accessError?.release()
            saveLoginKeychainBiometricVaultKey(query: query, keyData: keyData, ops: ops)
            return
        }

        let updateStatus = ops.update(query as CFDictionary, [kSecValueData as String: keyData] as CFDictionary)
        if updateStatus == errSecSuccess { return }
        if Self.shouldFallbackToLoginKeychain(status: updateStatus) {
            saveLoginKeychainBiometricVaultKey(query: query, keyData: keyData, ops: ops)
            return
        }
        if updateStatus != errSecItemNotFound && updateStatus != errSecAuthFailed {
            TTLogger.error("Unable to update biometric vault key (status: \(updateStatus))")
            return
        }
        if updateStatus == errSecAuthFailed {
            let deleteStatus = ops.delete(query as CFDictionary)
            guard deleteStatus == errSecSuccess || deleteStatus == errSecItemNotFound else {
                TTLogger.error("Unable to replace invalid biometric vault key (status: \(deleteStatus))")
                return
            }
        }
        let add: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: biometricService,
            kSecAttrAccount as String: biometricAccountKey,
            kSecAttrAccessControl as String: accessControl,
            kSecValueData as String: keyData
        ]
        var addStatus = ops.add(add as CFDictionary, nil)
        if addStatus == errSecDuplicateItem && updateStatus == errSecItemNotFound {
            let deleteStatus = ops.delete(query as CFDictionary)
            if deleteStatus == errSecSuccess {
                addStatus = ops.add(add as CFDictionary, nil)
            } else {
                TTLogger.error("Unable to replace duplicate biometric vault key (status: \(deleteStatus))")
                return
            }
        }
        if Self.shouldFallbackToLoginKeychain(status: addStatus) {
            saveLoginKeychainBiometricVaultKey(query: query, keyData: keyData, ops: ops)
            return
        }
        if addStatus != errSecSuccess {
            TTLogger.error("Unable to store biometric vault key (status: \(addStatus))")
        }
    }

    static func shouldFallbackToLoginKeychain(status: OSStatus) -> Bool {
        status == errSecMissingEntitlement || status == errSecParam || status == errSecNotAvailable
    }

    private func saveLoginKeychainBiometricVaultKey(
        query: [String: Any],
        keyData: Data,
        ops: KeychainOperations
    ) {
        let fallbackAdd: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: biometricService,
            kSecAttrAccount as String: biometricAccountKey,
            kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
            kSecValueData as String: keyData
        ]
        var status = ops.add(fallbackAdd as CFDictionary, nil)
        if status == errSecDuplicateItem {
            status = ops.update(query as CFDictionary, [kSecValueData as String: keyData] as CFDictionary)
            if status != errSecSuccess {
                _ = ops.delete(query as CFDictionary)
                status = ops.add(fallbackAdd as CFDictionary, nil)
            }
        }
        if status != errSecSuccess {
            TTLogger.error("Unable to store fallback biometric vault key (status: \(status))")
        }
    }

    package func readBiometricVaultKey(
        account: String? = nil,
        prompt: String? = nil,
        context: (any BiometricAuthenticationContextProtocol)? = nil
    ) -> BiometricKeyRead {
        let startGeneration = biometricContextLock.withLock { biometricUnlockGeneration }
        if Self.isCurrentTaskCancelled {
            return .cancelled
        }
        let targetAccount = account ?? biometricAccountKey
        let targetPrompt = prompt ?? TTZipLocalizationManager.shared.string(for: L10n.Vault.biometricReason)
        let authContext = context ?? makeBiometricContext()
        authContext.localizedReason = targetPrompt
        let ops = biometricContextLock.withLock { customKeychainOperations } ?? .system

        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: biometricService,
            kSecAttrAccount as String: targetAccount,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne
        ]
        if let laContext = authContext as? LAContext {
            query[kSecUseAuthenticationContext as String] = laContext
        }

        var dataTypeRef: CFTypeRef?
        let status = ops.copyMatching(query as CFDictionary, &dataTypeRef)
        if status == errSecSuccess, let data = dataTypeRef as? Data {
            if isLoginKeychainBiometricItem(account: targetAccount, ops: ops) {
                if let failure = evaluateExplicitBiometricAuth(
                    context: authContext,
                    prompt: targetPrompt,
                    startGeneration: startGeneration
                ) {
                    return failure
                }
            }
            return .success(data)
        }
        if Self.shouldFallbackToLoginKeychain(status: status) {
            return readLoginKeychainBiometricVaultKey(
                account: targetAccount,
                prompt: targetPrompt,
                context: authContext,
                ops: ops,
                startGeneration: startGeneration
            )
        }
        return Self.mapKeychainCopyStatus(status)
    }

    private func readLoginKeychainBiometricVaultKey(
        account: String,
        prompt: String,
        context: any BiometricAuthenticationContextProtocol,
        ops: KeychainOperations,
        startGeneration: UInt64
    ) -> BiometricKeyRead {
        if Self.isCurrentTaskCancelled || !isBiometricUnlockGenerationCurrent(startGeneration) {
            return .cancelled
        }
        let fallbackQuery: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: biometricService,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne
        ]
        var dataTypeRef: CFTypeRef?
        let status = ops.copyMatching(fallbackQuery as CFDictionary, &dataTypeRef)
        if status == errSecSuccess, let data = dataTypeRef as? Data {
            if let failure = evaluateExplicitBiometricAuth(
                context: context,
                prompt: prompt,
                startGeneration: startGeneration
            ) {
                return failure
            }
            return .success(data)
        }
        return Self.mapKeychainCopyStatus(status)
    }

    private func isLoginKeychainBiometricItem(account: String, ops: KeychainOperations) -> Bool {
        let attrQuery: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: biometricService,
            kSecAttrAccount as String: account,
            kSecReturnAttributes as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne
        ]
        var attrRef: CFTypeRef?
        let status = ops.copyMatching(attrQuery as CFDictionary, &attrRef)
        if status == errSecSuccess {
            if let attrs = attrRef as? [String: Any] {
                return attrs[kSecAttrAccessControl as String] == nil
            }
            return true
        }
        return false
    }

    package func evaluateExplicitBiometricAuth(
        context: any BiometricAuthenticationContextProtocol,
        prompt: String,
        startGeneration: UInt64? = nil
    ) -> BiometricKeyRead? {
        let (effectiveGeneration, evaluatorOverride, canEvaluate) = biometricContextLock.withLock {
            () -> (UInt64, (@Sendable (any BiometricAuthenticationContextProtocol, String) -> Result<Void, Error>)?, Bool) in
            let currentGen = biometricUnlockGeneration
            let targetGen = startGeneration ?? currentGen
            guard !Self.isCurrentTaskCancelled, currentGen == targetGen else {
                return (targetGen, nil, false)
            }
            activeBiometricContext = context
            return (targetGen, biometricPolicyEvaluatorOverride, true)
        }
        guard canEvaluate else {
            return .cancelled
        }
        defer {
            biometricContextLock.withLock {
                if activeBiometricContext === context {
                    activeBiometricContext = nil
                }
            }
        }
        let evaluation: Result<Void, Error>
        if let evaluatorOverride {
            evaluation = evaluatorOverride(context, prompt)
        } else if PasswordVaultManager.isCLIProcess && context is LAContext {
            evaluation = .success(())
        } else {
            evaluation = context.evaluateBiometricPolicy(localizedReason: prompt)
        }
        if Self.isCurrentTaskCancelled || !isBiometricUnlockGenerationCurrent(effectiveGeneration) {
            return .cancelled
        }
        switch evaluation {
        case .success:
            return nil
        case .failure(let error):
            return Self.mapBiometricEvaluationError(error)
        }
    }

    package static func mapBiometricEvaluationError(_ error: Error) -> BiometricKeyRead {
        if let laError = error as? LAError {
            return mapLAErrorCode(laError.code)
        }
        let nsError = error as NSError
        if nsError.domain == LAError.errorDomain, let code = LAError.Code(rawValue: nsError.code) {
            return mapLAErrorCode(code)
        }
        return .failed
    }

    private static func mapLAErrorCode(_ code: LAError.Code) -> BiometricKeyRead {
        switch code {
        case .userCancel, .userFallback, .systemCancel, .appCancel:
            return .cancelled
        case .biometryNotAvailable, .biometryNotEnrolled, .biometryLockout, .passcodeNotSet:
            return .unavailable
        default:
            return .failed
        }
    }

    private static func mapKeychainCopyStatus(_ status: OSStatus) -> BiometricKeyRead {
        switch status {
        case errSecItemNotFound: return .missing
        case errSecUserCanceled: return .cancelled
        case errSecInteractionNotAllowed, errSecAuthFailed: return .unavailable
        default: return .failure(status)
        }
    }

    private func isBiometricUnlockGenerationCurrent(_ startGeneration: UInt64) -> Bool {
        biometricContextLock.withLock { biometricUnlockGeneration == startGeneration }
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
    private func unlockWithBiometricKeyData(
        _ vaultKeyData: Data,
        startGeneration: UInt64
    ) -> VaultBiometricUnlockResult {
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
        if Self.isCurrentTaskCancelled || !isBiometricUnlockGenerationCurrent(startGeneration) {
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
