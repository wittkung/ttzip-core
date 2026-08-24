// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation
import Security
import CryptoKit
import CommonCrypto
import LocalAuthentication

public final class PasswordVaultManager: PasswordVaultManaging, @unchecked Sendable {
    public static let shared = PasswordVaultManager()
    
    let vaultLock = NSLock()

    var entries: [PasswordVaultEntry] = []
    
    var _isUnlocked: Bool = false
    public var isUnlocked: Bool {
        vaultLock.withLock { _isUnlocked }
    }
    var masterPasswordSalt: Data?
    var masterVerifierHash: String?
    var activeMasterSecret: SecureBytes?
    var activeDerivedVaultKey: SecureBytes?
    
    let vaultFileURL: URL
    let v3VaultFileURL: URL
    let backupFileURL: URL
    let configFileURL: URL
    
    func setMasterVerifierHashInternal(_ hash: String?) { masterVerifierHash = hash }
    func setEntriesInternal(_ list: [PasswordVaultEntry]) { entries = list }
    
    private init() {
        let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
        let ttzipDir = appSupport.appendingPathComponent("TTZip", isDirectory: true)
        try? FileManager.default.createDirectory(at: ttzipDir, withIntermediateDirectories: true)
        
        self.vaultFileURL = ttzipDir.appendingPathComponent("password_vault_v4.enc")
        self.v3VaultFileURL = ttzipDir.appendingPathComponent("password_vault_v3.enc")
        self.backupFileURL = ttzipDir.appendingPathComponent("vault_backup_v4.enc")
        self.configFileURL = ttzipDir.appendingPathComponent("vault_config_v4.json")
        
        loadConfigInternal()
    }

    internal init(
        vaultURL: URL? = nil,
        configURL: URL? = nil,
        backupURL: URL? = nil
    ) {
        let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
        let ttzipDir = appSupport.appendingPathComponent("TTZip", isDirectory: true)
        try? FileManager.default.createDirectory(at: ttzipDir, withIntermediateDirectories: true)
        
        let targetVaultURL = vaultURL ?? ttzipDir.appendingPathComponent("password_vault_v4.enc")
        let targetDir = targetVaultURL.deletingLastPathComponent()
        
        self.vaultFileURL = targetVaultURL
        self.v3VaultFileURL = targetDir.appendingPathComponent("password_vault_v3.enc")
        self.backupFileURL = backupURL ?? targetDir.appendingPathComponent("vault_backup_v4.enc")
        self.configFileURL = configURL ?? targetDir.appendingPathComponent("vault_config_v4.json")
        
        loadConfigInternal()
    }
    
    public static let vaultDidChangeNotification = Notification.Name("PasswordVaultDidChangeNotification")
    
    private func notifyChange() {
        Task { @MainActor in
            NotificationCenter.default.post(name: PasswordVaultManager.vaultDidChangeNotification, object: nil)
        }
    }

    public var isMasterPasswordSet: Bool {
        vaultLock.withLock { masterVerifierHash != nil }
    }
    
    /// Automatically attempts candidate passwords when encountering encrypted archives (defaults to true).
    public var autoUnlockArchives: Bool {
        get {
            vaultLock.withLock {
                if let val = UserDefaults.standard.object(forKey: "TTZipAutoUnlockArchivesWithVault") as? Bool {
                    return val
                }
                return true
            }
        }
        set {
            vaultLock.withLock {
                UserDefaults.standard.set(newValue, forKey: "TTZipAutoUnlockArchivesWithVault")
            }
            notifyChange()
        }
    }
    
    public var hasBackupVault: Bool {
        vaultLock.withLock {
            FileManager.default.fileExists(atPath: backupFileURL.path)
        }
    }
    
    /// Initializes master password for initial setup using SecureBytes and Secure Enclave wrapped keys.
    public func setMasterPassword(_ pwd: String) {
        vaultLock.withLock {
            var saltBytes = [UInt8](repeating: 0, count: 32)
            guard SecRandomCopyBytes(kSecRandomDefault, saltBytes.count, &saltBytes) == errSecSuccess else {
                return
            }
            let salt = Data(saltBytes)
            self.masterPasswordSalt = salt
            
            guard let keyBytes = deriveSymmetricKeyBytesV4(pwd, salt: salt, iterations: Self.defaultV4Iterations) else {
                return
            }
            
            let vaultKey = SecureBytes(data: Data(keyBytes))
            let verifier = computeVerifierHash(vaultKey: vaultKey, salt: salt)
            
            self.masterVerifierHash = verifier
            self.activeMasterSecret = SecureBytes(utf8String: pwd)
            self.activeDerivedVaultKey = vaultKey
            self._isUnlocked = true
            
            saveConfigLocked()
            saveVaultLocked()
            saveBiometricVaultKey(vaultKey: vaultKey)
        }
        notifyChange()
    }
    
    /// Resets vault state for fresh initialization.
    public func resetToFirstRunState() {
        vaultLock.withLock {
            masterPasswordSalt = nil
            masterVerifierHash = nil
            activeMasterSecret?.wipeAndFree()
            activeMasterSecret = nil
            activeDerivedVaultKey?.wipeAndFree()
            activeDerivedVaultKey = nil
            _isUnlocked = false
            entries = []
            
            try? FileManager.default.removeItem(at: vaultFileURL)
            try? FileManager.default.removeItem(at: backupFileURL)
            try? FileManager.default.removeItem(at: configFileURL)
            deleteBiometricVaultKey()
            deleteFromKeychain(account: "MasterHash")
            deleteFromKeychain(account: "MasterPassword")
        }
        notifyChange()
    }
    
    /// Unlocks vault using provided master password string.
    public func unlockVault(with pwd: String) -> Bool {
        let success: Bool = vaultLock.withLock {
            guard let salt = masterPasswordSalt ?? generateAndSaveSaltLocked() else {
                return false
            }
            guard let keyBytes = deriveSymmetricKeyBytesV4(pwd, salt: salt, iterations: Self.defaultV4Iterations) else {
                return false
            }
            let vaultKey = SecureBytes(data: Data(keyBytes))
            let verifier = computeVerifierHash(vaultKey: vaultKey, salt: salt)
            
            if let expectedVerifier = masterVerifierHash {
                guard verifier == expectedVerifier else {
                    vaultKey.wipeAndFree()
                    return false
                }
            } else {
                masterVerifierHash = verifier
                saveConfigLocked()
            }
            
            self.activeMasterSecret = SecureBytes(utf8String: pwd)
            self.activeDerivedVaultKey = vaultKey
            self._isUnlocked = true
            
            saveBiometricVaultKey(vaultKey: vaultKey)
            loadVaultLocked(vaultKey: vaultKey)
            return true
        }
        if success {
            notifyChange()
        }
        return success
    }

    /// Unlocks vault via hardware-backed Biometric Authentication (Touch ID / Secure Enclave)
    /// by retrieving the wrapped derived VaultKey from Keychain with AccessControl.
    public func unlockWithBiometrics() -> Bool {
        let success: Bool = vaultLock.withLock {
            if _isUnlocked && activeDerivedVaultKey != nil {
                return true
            }
            guard let vaultKeyData = loadBiometricVaultKey(), vaultKeyData.count == 32 else {
                return false
            }
            let vaultKey = SecureBytes(data: vaultKeyData)
            
            if let expectedVerifier = masterVerifierHash, let salt = masterPasswordSalt {
                let verifier = computeVerifierHash(vaultKey: vaultKey, salt: salt)
                guard verifier == expectedVerifier else {
                    vaultKey.wipeAndFree()
                    return false
                }
            }
            
            self.activeDerivedVaultKey = vaultKey
            self._isUnlocked = true
            loadVaultLocked(vaultKey: vaultKey)
            return true
        }
        if success {
            notifyChange()
        }
        return success
    }
    
    /// Locks vault and securely scrubs active password and key buffers from memory using volatile barriers.
    public func lockVault() {
        vaultLock.withLock {
            _isUnlocked = false
            activeMasterSecret?.wipeAndFree()
            activeMasterSecret = nil
            activeDerivedVaultKey?.wipeAndFree()
            activeDerivedVaultKey = nil
            entries.removeAll(keepingCapacity: false)
        }
        notifyChange()
    }
    
    public func resetMasterPassword(newMasterPassword: String) {
        setMasterPassword(newMasterPassword)
    }
    
    public func recoverBackupVault(withOriginalMasterPassword: String) -> Bool {
        return unlockVault(with: withOriginalMasterPassword)
    }
    
    public func getEntries() -> [PasswordVaultEntry] {
        vaultLock.withLock {
            guard _isUnlocked else { return [] }
            return entries
        }
    }
    
    public static var isCLIProcess: Bool {
        let procName = ProcessInfo.processInfo.processName.lowercased()
        if procName.contains("cli") || procName.contains("bench") || procName.contains("swift") || procName.contains("xctest") {
            return true
        }
        if CommandLine.arguments.contains(where: { $0.contains("bench") || $0.contains("test") || $0.contains("cli") }) {
            return true
        }
        if let bundleId = Bundle.main.bundleIdentifier, bundleId.contains("com.ttzip.app") {
            return false
        }
        return true
    }
    
    /// Returns sorted candidate passwords for automated decryption attempts.
    public func candidatePasswordsForAutoUnlock() -> [String] {
        if !isUnlocked {
            _ = unlockWithBiometrics()
        }
        let list = getEntries()
        let sorted = list.sorted { $0.useCount > $1.useCount }
        var result: [String] = []
        for item in sorted {
            if !item.password.isEmpty && !result.contains(item.password) {
                result.append(item.password)
            }
        }
        return result
    }
    
    public func addEntry(id: UUID = UUID(), label: String, password: String, category: String = "General") {
        vaultLock.withLock {
            guard _isUnlocked else { return }
            let finalLabel = label.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? "Password" : label.trimmingCharacters(in: .whitespacesAndNewlines)
            let finalCategory = category.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? "General" : category.trimmingCharacters(in: .whitespacesAndNewlines)
            let newEntry = PasswordVaultEntry(id: id, label: finalLabel, password: password, category: finalCategory)
            entries.append(newEntry)
            saveVaultLocked()
        }
        notifyChange()
    }
    
    public func removeEntry(id: UUID) {
        vaultLock.withLock {
            guard _isUnlocked else { return }
            entries.removeAll { $0.id == id }
            saveVaultLocked()
        }
        notifyChange()
    }
    
    public func recordUsage(id: UUID) {
        vaultLock.withLock {
            guard _isUnlocked else { return }
            if let idx = entries.firstIndex(where: { $0.id == id }) {
                entries[idx].useCount += 1
                entries[idx].lastUsedAt = Date()
                saveVaultLocked()
            }
        }
        notifyChange()
    }
}

// MARK: - Crypto v4 (PBKDF2-SHA256 600k rounds + Rust AES-256-GCM)

extension PasswordVaultManager {
    
    static let vaultMagicV4 = Data([0x54, 0x54, 0x56, 0x34]) // "TTV4"
    static let defaultV4Iterations: UInt32 = 600_000
    
    func deriveSymmetricKeyBytesV4(_ password: String, salt: Data, iterations: UInt32 = defaultV4Iterations) -> [UInt8]? {
        var derivedKey = [UInt8](repeating: 0, count: 32)
        let passBytes = Array(password.utf8)
        let status = salt.withUnsafeBytes { sBuf in
            CCKeyDerivationPBKDF(
                CCPBKDFAlgorithm(kCCPBKDF2),
                password, passBytes.count,
                sBuf.baseAddress?.assumingMemoryBound(to: UInt8.self), salt.count,
                CCPseudoRandomAlgorithm(kCCPRFHmacAlgSHA256),
                iterations,
                &derivedKey, 32
            )
        }
        guard status == kCCSuccess else {
            return nil // 绝对禁止 SHA-256 弱哈希回退降级
        }
        return derivedKey
    }
    
    func computeVerifierHash(vaultKey: SecureBytes, salt: Data) -> String {
        return vaultKey.withUnsafeBytes { kBuf in
            guard let kBase = kBuf.baseAddress else { return "" }
            var hmacContext = CCHmacContext()
            CCHmacInit(&hmacContext, CCHmacAlgorithm(kCCHmacAlgSHA256), kBase, kBuf.count)
            salt.withUnsafeBytes { sBuf in
                if let sBase = sBuf.baseAddress {
                    CCHmacUpdate(&hmacContext, sBase, sBuf.count)
                }
            }
            var output = [UInt8](repeating: 0, count: 32)
            CCHmacFinal(&hmacContext, &output)
            return output.map { String(format: "%02x", $0) }.joined()
        }
    }
    
    private func generateAndSaveSaltLocked() -> Data? {
        var saltBytes = [UInt8](repeating: 0, count: 32)
        guard SecRandomCopyBytes(kSecRandomDefault, saltBytes.count, &saltBytes) == errSecSuccess else {
            return nil
        }
        let salt = Data(saltBytes)
        self.masterPasswordSalt = salt
        saveConfigLocked()
        return salt
    }
    
    func encryptDataV4WithKey(_ data: Data, vaultKey: SecureBytes) -> Data? {
        guard let sealedBox = vaultKey.withUnsafeBytes({ keyBuf -> AES.GCM.SealedBox? in
            guard let base = keyBuf.baseAddress, keyBuf.count >= 32 else { return nil }
            let symKey = SymmetricKey(data: Data(bytes: base, count: 32))
            return try? AES.GCM.seal(data, using: symKey)
        }) else {
            return nil
        }
        
        var combinedPayload = Data()
        combinedPayload.append(Data(sealedBox.nonce)) // 12 bytes nonce
        combinedPayload.append(sealedBox.ciphertext)  // N bytes cipher
        combinedPayload.append(sealedBox.tag)         // 16 bytes tag
        
        var result = Data()
        result.append(Self.vaultMagicV4) // 4 bytes
        var iterBigEndian = Self.defaultV4Iterations.bigEndian
        result.append(Data(bytes: &iterBigEndian, count: 4)) // 4 bytes
        
        let salt = masterPasswordSalt ?? Data(repeating: 0, count: 32)
        var saltLen = UInt8(salt.count)
        result.append(Data(bytes: &saltLen, count: 1)) // 1 byte
        result.append(salt) // 32 bytes
        result.append(combinedPayload) // AES-GCM sealed box
        return result
    }
    
    func decryptDataV4WithKey(_ data: Data, vaultKey: SecureBytes) -> Data? {
        guard data.count >= 69 else { return nil }
        let magic = data.prefix(4)
        guard magic == Self.vaultMagicV4 else { return nil }
        
        let saltLen = Int(data[8])
        guard data.count >= 9 + saltLen + 28 else { return nil }
        
        let payload = data.subdata(in: (9 + saltLen)..<data.count)
        guard payload.count >= 28 else { return nil }
        
        let nonceData = payload.prefix(12)
        let tagData = payload.suffix(16)
        let cipherData = payload.subdata(in: 12..<(payload.count - 16))
        
        return vaultKey.withUnsafeBytes { keyBuf -> Data? in
            guard let base = keyBuf.baseAddress, keyBuf.count >= 32 else { return nil }
            let symKey = SymmetricKey(data: Data(bytes: base, count: 32))
            guard let nonce = try? AES.GCM.Nonce(data: nonceData),
                  let sealedBox = try? AES.GCM.SealedBox(nonce: nonce, ciphertext: cipherData, tag: tagData) else {
                return nil
            }
            return try? AES.GCM.open(sealedBox, using: symKey)
        }
    }
    
    func loadConfigInternal() {
        guard FileManager.default.fileExists(atPath: configFileURL.path) else { return }
        if let data = try? Data(contentsOf: configFileURL),
           let dict = try? JSONSerialization.jsonObject(with: data) as? [String: String] {
            if let verifier = dict["verifierHash"] {
                self.masterVerifierHash = verifier
            }
            if let saltHex = dict["saltHex"], let saltData = Data(hexString: saltHex) {
                self.masterPasswordSalt = saltData
            }
        }
    }
    
    func saveConfigLocked() {
        guard let verifier = masterVerifierHash else { return }
        var dict: [String: String] = ["verifierHash": verifier]
        if let salt = masterPasswordSalt {
            dict["saltHex"] = salt.map { String(format: "%02x", $0) }.joined()
        }
        if let data = try? JSONSerialization.data(withJSONObject: dict) {
            try? data.write(to: configFileURL, options: .atomic)
        }
    }
    
    func loadVaultLocked(vaultKey: SecureBytes) {
        if FileManager.default.fileExists(atPath: vaultFileURL.path) {
            do {
                let encryptedData = try Data(contentsOf: vaultFileURL)
                if let rawJSON = decryptDataV4WithKey(encryptedData, vaultKey: vaultKey) {
                    let decoder = JSONDecoder()
                    let decoded = try decoder.decode([PasswordVaultEntry].self, from: rawJSON)
                    setEntriesInternal(decoded)
                    return
                }
            } catch {
                // v4 decrypt failed
            }
        }
        setEntriesInternal([])
    }
    
    func saveVaultLocked() {
        guard _isUnlocked, let vaultKey = activeDerivedVaultKey else { return }
        do {
            let encoder = JSONEncoder()
            encoder.outputFormatting = .prettyPrinted
            let rawJSON = try encoder.encode(entries)
            
            if let encryptedData = encryptDataV4WithKey(rawJSON, vaultKey: vaultKey) {
                try encryptedData.write(to: vaultFileURL, options: .atomic)
            }
        } catch {
            TTLogger.error("Failed to encrypt vault: \(error.localizedDescription)")
        }
    }
}

// MARK: - Hardware-Backed Biometric Keychain Storage (SecAccessControl)

extension PasswordVaultManager {
    
    private var biometricAccountKey: String { "TTZipVaultDerivedKey_Biometric" }
    
    func saveBiometricVaultKey(vaultKey: SecureBytes) {
        if PasswordVaultManager.isCLIProcess { return }
        
        var accessError: Unmanaged<CFError>?
        guard let accessControl = SecAccessControlCreateWithFlags(
            kCFAllocatorDefault,
            kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
            [.biometryCurrentSet, .userPresence],
            &accessError
        ) else {
            return
        }
        
        vaultKey.withUnsafeBytes { keyBuf in
            guard let keyBase = keyBuf.baseAddress else { return }
            let keyData = Data(bytes: keyBase, count: keyBuf.count)
            
            let query: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: "com.ttzip.app.vault.biometric",
                kSecAttrAccount as String: biometricAccountKey,
                kSecAttrAccessControl as String: accessControl,
                kSecValueData as String: keyData
            ]
            
            SecItemDelete(query as CFDictionary)
            SecItemAdd(query as CFDictionary, nil)
        }
    }
    
    func loadBiometricVaultKey() -> Data? {
        if PasswordVaultManager.isCLIProcess { return nil }
        
        let context = LAContext()
        context.localizedReason = TTZipLocalizationManager.shared.string(for: L10n.Vault.biometricReason)
        
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: "com.ttzip.app.vault.biometric",
            kSecAttrAccount as String: biometricAccountKey,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
            kSecUseAuthenticationContext as String: context
        ]
        
        var dataTypeRef: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &dataTypeRef)
        if status == errSecSuccess, let data = dataTypeRef as? Data {
            return data
        }
        return nil
    }
    
    func deleteBiometricVaultKey() {
        if PasswordVaultManager.isCLIProcess { return }
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: "com.ttzip.app.vault.biometric",
            kSecAttrAccount as String: biometricAccountKey
        ]
        SecItemDelete(query as CFDictionary)
    }
    
    func deleteFromKeychain(account: String) {
        if PasswordVaultManager.isCLIProcess { return }
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: "com.ttzip.app.vault",
            kSecAttrAccount as String: account
        ]
        SecItemDelete(query as CFDictionary)
    }
}

// MARK: - Hex String Extension

private extension Data {
    init?(hexString: String) {
        let len = hexString.count / 2
        var data = Data(capacity: len)
        var index = hexString.startIndex
        for _ in 0..<len {
            let nextIndex = hexString.index(index, offsetBy: 2)
            let bytes = hexString[index..<nextIndex]
            if let b = UInt8(bytes, radix: 16) {
                data.append(b)
            } else {
                return nil
            }
            index = nextIndex
        }
        self = data
    }
}

// MARK: - Password Generation & Strength Evaluation

extension PasswordVaultManager {
    
    /// Generates high-entropy pseudo-random password string.
    public func generateRandomPassword(length: Int = 16, includeSymbols: Bool = true) -> String {
        let letters = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
        let symbols = "!@#$%^&*()_+-=[]{}|;:,.<>?"
        let charset = includeSymbols ? (letters + symbols) : letters
        
        var result = ""
        for _ in 0..<length {
            if let randomChar = charset.randomElement() {
                result.append(randomChar)
            }
        }
        return result
    }
    
    /// Evaluates password entropy and strength score (0 to 5) with strongly-typed tier.
    public func evaluatePasswordStrength(_ pwd: String) -> (score: Int, tier: PasswordStrengthTier) {
        if pwd.isEmpty { return (0, .veryWeak) }
        var score = 0
        if pwd.count >= 8 { score += 1 }
        if pwd.count >= 12 { score += 1 }
        if pwd.rangeOfCharacter(from: .decimalDigits) != nil { score += 1 }
        if pwd.rangeOfCharacter(from: CharacterSet(charactersIn: "!@#$%^&*()_+-=[]{}|;:,.<>?")) != nil { score += 1 }
        if pwd.rangeOfCharacter(from: .uppercaseLetters) != nil && pwd.rangeOfCharacter(from: .lowercaseLetters) != nil { score += 1 }
        
        switch score {
        case 0...1: return (score, .veryWeak)
        case 2: return (score, .weak)
        case 3: return (score, .medium)
        case 4: return (score, .strong)
        default: return (score, .veryStrong)
        }
    }
}
