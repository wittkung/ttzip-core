// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import LocalAuthentication
import Security
@testable import TTZipCore

final class MockLoginKeychainFallbackStore: @unchecked Sendable {
    private let lock = NSLock()
    private var storage: [String: Data] = [:]
    private(set) var accessControlAddAttempts = 0
    private(set) var loginKeychainSaveCount = 0

    var operations: PasswordVaultManager.KeychainOperations {
        PasswordVaultManager.KeychainOperations(
            add: { [weak self] query, _ in
                guard let self else { return errSecInternalError }
                return self.handleAdd(query as? [String: Any] ?? [:])
            },
            update: { [weak self] query, attrs in
                guard let self else { return errSecInternalError }
                return self.handleUpdate(
                    query as? [String: Any] ?? [:],
                    attrs: attrs as? [String: Any] ?? [:]
                )
            },
            copyMatching: { [weak self] query, result in
                guard let self else { return errSecInternalError }
                return self.handleCopyMatching(
                    query as? [String: Any] ?? [:],
                    result: result
                )
            },
            delete: { [weak self] query in
                guard let self else { return errSecInternalError }
                return self.handleDelete(query as? [String: Any] ?? [:])
            }
        )
    }

    private func handleAdd(_ dict: [String: Any]) -> OSStatus {
        lock.withLock {
            if dict[kSecAttrAccessControl as String] != nil {
                accessControlAddAttempts += 1
                return errSecMissingEntitlement
            }
            guard let account = dict[kSecAttrAccount as String] as? String,
                  let data = dict[kSecValueData as String] as? Data else {
                return errSecParam
            }
            storage[account] = data
            loginKeychainSaveCount += 1
            return errSecSuccess
        }
    }

    private func handleUpdate(_ query: [String: Any], attrs: [String: Any]) -> OSStatus {
        lock.withLock {
            guard let account = query[kSecAttrAccount as String] as? String else {
                return errSecParam
            }
            guard storage[account] != nil else {
                return errSecItemNotFound
            }
            if let data = attrs[kSecValueData as String] as? Data {
                storage[account] = data
            }
            return errSecSuccess
        }
    }

    private func handleCopyMatching(
        _ query: [String: Any],
        result: UnsafeMutablePointer<CFTypeRef?>?
    ) -> OSStatus {
        lock.withLock {
            guard let account = query[kSecAttrAccount as String] as? String,
                  let data = storage[account] else {
                return errSecItemNotFound
            }
            if query[kSecReturnAttributes as String] as? Bool == true {
                let attrs: [String: Any] = [kSecAttrAccount as String: account]
                result?.pointee = attrs as CFDictionary
                return errSecSuccess
            }
            if query[kSecUseAuthenticationContext as String] != nil {
                return errSecMissingEntitlement
            }
            result?.pointee = data as CFData
            return errSecSuccess
        }
    }

    private func handleDelete(_ query: [String: Any]) -> OSStatus {
        lock.withLock {
            guard let account = query[kSecAttrAccount as String] as? String else {
                return errSecParam
            }
            storage.removeValue(forKey: account)
            return errSecSuccess
        }
    }
}

final class MockBiometricAuthenticationContext: BiometricAuthenticationContextProtocol, @unchecked Sendable {
    private let lock = NSLock()
    private var storedLocalizedReason = ""
    private var storedInvalidateCallCount = 0

    var localizedReason: String {
        get {
            lock.withLock { storedLocalizedReason }
        }
        set {
            lock.withLock { storedLocalizedReason = newValue }
        }
    }

    var invalidateCallCount: Int {
        lock.withLock { storedInvalidateCallCount }
    }

    func invalidate() {
        lock.withLock {
            storedInvalidateCallCount += 1
        }
    }
}

final class LockedResultBox: @unchecked Sendable {
    private let lock = NSLock()
    private var value: VaultBiometricUnlockResult?

    func store(_ result: VaultBiometricUnlockResult) {
        lock.withLock {
            value = result
        }
    }

    func load() -> VaultBiometricUnlockResult? {
        lock.withLock {
            value
        }
    }
}

final class LockedCounterBox: @unchecked Sendable {
    private let lock = NSLock()
    private var value = 0

    func increment() {
        lock.withLock {
            value += 1
        }
    }

    func load() -> Int {
        lock.withLock {
            value
        }
    }
}
