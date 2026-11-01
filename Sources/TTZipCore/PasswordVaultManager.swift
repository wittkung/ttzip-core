// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
import Security
import LocalAuthentication

public enum VaultBiometricUnlockResult: Sendable, Equatable {
    case unlocked
    case unavailable
    case cancelled
    case keyMissing
    case keyAccessFailed
    case invalidKey
    case vaultUnreadable
}

public enum VaultPasswordUnlockResult: Sendable, Equatable {
    case unlocked
    case invalidPassword
    case configurationMissing
    case vaultUnreadable
    case cancelled
}

public enum VaultResetError: Error, Sendable {
    case emptyPassword
    case vaultUnlocked
    case originalFilesMissing
    case newVaultNotSaved(backupDirectory: URL)
}

public enum VaultEntrySaveResult: Sendable, Equatable {
    case saved(UUID)
    case locked
    case duplicateID
    case writeFailed
}

public enum VaultEntryDeleteResult: Sendable, Equatable {
    case removed
    case notFound
    case locked
    case writeFailed
}

public enum ArchiveVaultCredentialError: Error, Sendable, Equatable {
    case locked
    case invalidIdentity
    case writeFailed
}

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

    public init(
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
    
    func notifyChange() {
        Task { @MainActor in
            NotificationCenter.default.post(name: PasswordVaultManager.vaultDidChangeNotification, object: nil)
        }
    }

    public var isMasterPasswordSet: Bool {
        vaultLock.withLock {
            masterVerifierHash != nil || FileManager.default.fileExists(atPath: vaultFileURL.path)
        }
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
        setMasterPassword(pwd, saveBiometricKey: true)
    }

    private func setMasterPassword(_ pwd: String, saveBiometricKey: Bool) {
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
            
            guard let vaultKey = try? SecureBytes(data: Data(keyBytes)),
                  let masterSecret = try? SecureBytes(utf8String: pwd) else {
                TTLogger.error("Failed to allocate SecureBytes memory during master password initialization")
                return
            }
            let verifier = computeVerifierHash(vaultKey: vaultKey, salt: salt)
            
            self.masterVerifierHash = verifier
            self.activeMasterSecret = masterSecret
            self.activeDerivedVaultKey = vaultKey
            self._isUnlocked = true
            
            saveConfigLocked()
            saveVaultLocked()
            if saveBiometricKey {
                saveBiometricVaultKey(vaultKey: vaultKey)
            }
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
        unlockVaultResult(with: pwd) == .unlocked
    }

    public func unlockVaultResult(with pwd: String) -> VaultPasswordUnlockResult {
        let result: VaultPasswordUnlockResult = vaultLock.withLock {
            guard let salt = masterPasswordSalt, let expectedVerifier = masterVerifierHash else { return .configurationMissing }
            guard let keyBytes = deriveSymmetricKeyBytesV4(pwd, salt: salt, iterations: Self.defaultV4Iterations) else {
                return .vaultUnreadable
            }
            guard let vaultKey = try? SecureBytes(data: Data(keyBytes)) else {
                TTLogger.error("Failed to allocate vaultKey SecureBytes during unlockVault")
                return .vaultUnreadable
            }
            let verifier = computeVerifierHash(vaultKey: vaultKey, salt: salt)
            
            guard verifier == expectedVerifier else {
                vaultKey.wipeAndFree()
                return .invalidPassword
            }
            
            guard let masterSecret = try? SecureBytes(utf8String: pwd) else {
                vaultKey.wipeAndFree()
                TTLogger.error("Failed to allocate masterSecret SecureBytes during unlockVault")
                return .vaultUnreadable
            }
            guard let loadedEntries = loadVaultLocked(vaultKey: vaultKey) else {
                masterSecret.wipeAndFree()
                vaultKey.wipeAndFree()
                return .vaultUnreadable
            }
            if withUnsafeCurrentTask(body: { $0?.isCancelled ?? false }) {
                masterSecret.wipeAndFree()
                vaultKey.wipeAndFree()
                return .cancelled
            }
            self.entries = loadedEntries
            self.activeMasterSecret = masterSecret
            self.activeDerivedVaultKey = vaultKey
            self._isUnlocked = true
            saveBiometricVaultKey(vaultKey: vaultKey)
            return .unlocked
        }
        if result == .unlocked {
            notifyChange()
        }
        return result
    }

    /// Unlocks vault via hardware-backed Biometric Authentication (Touch ID / Secure Enclave)
    /// by retrieving the wrapped derived VaultKey from Keychain with AccessControl.
    public func unlockWithBiometrics() -> Bool {
        unlockWithBiometricsResult() == .unlocked
    }

    public func unlockWithBiometricsResult() -> VaultBiometricUnlockResult {
        unlockWithBiometricsResult(readKey: loadBiometricVaultKey)
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
    
    public func recoverBackupVault(withOriginalMasterPassword: String) -> Bool {
        return unlockVault(with: withOriginalMasterPassword)
    }
    
    public func getEntries() -> [PasswordVaultEntry] {
        vaultLock.withLock {
            guard _isUnlocked else { return [] }
            return entries.filter { $0.archiveScope == nil }
        }
    }

    public static var isCLIProcess: Bool {
        isCLIProcess(
            bundleIdentifier: Bundle.main.bundleIdentifier,
            processName: ProcessInfo.processInfo.processName,
            arguments: CommandLine.arguments
        )
    }

    static func isCLIProcess(bundleIdentifier: String?, processName: String, arguments: [String]) -> Bool {
        if bundleIdentifier == "com.metastudyline.ttzip" || bundleIdentifier == "com.ttzip.app" {
            return false
        }
        let procName = processName.lowercased()
        if procName.contains("cli") || procName.contains("bench") || procName.contains("swift") || procName.contains("xctest") {
            return true
        }
        if arguments.contains(where: { $0.contains("bench") || $0.contains("test") || $0.contains("cli") }) {
            return true
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
    

}

// MARK: - Crypto v4 (PBKDF2-SHA256 600k rounds + Rust AES-256-GCM)

extension PasswordVaultManager {
    
    static let vaultMagicV4 = Data([0x54, 0x54, 0x56, 0x34]) // "TTV4"
    static let defaultV4Iterations: UInt32 = 600_000
    
    func deriveSymmetricKeyBytesV4(_ password: String, salt: Data, iterations: UInt32 = defaultV4Iterations) -> [UInt8]? {
        guard let derivedData = try? vaultDeriveKey(password: password, salt: salt, iterations: iterations) else {
            return nil
        }
        return Array(derivedData)
    }
    
    func computeVerifierHash(vaultKey: SecureBytes, salt: Data) -> String {
        return vaultKey.withUnsafeBytes { kBuf in
            guard let kBase = kBuf.baseAddress, kBuf.count >= 32 else { return "" }
            let keyData = Data(bytes: kBase, count: 32)
            return vaultComputeVerifier(key: keyData, salt: salt)
        }
    }
    
    func encryptDataV4WithKey(_ data: Data, vaultKey: SecureBytes) -> Data? {
        guard let encryptedPayload = vaultKey.withUnsafeBytes({ keyBuf -> Data? in
            guard let base = keyBuf.baseAddress, keyBuf.count >= 32 else { return nil }
            let keyData = Data(bytes: base, count: 32)
            return try? vaultEncryptPayload(key: keyData, payload: data)
        }) else {
            return nil
        }
        
        var result = Data()
        result.append(Self.vaultMagicV4) // 4 bytes
        var iterBigEndian = Self.defaultV4Iterations.bigEndian
        result.append(Data(bytes: &iterBigEndian, count: 4)) // 4 bytes
        
        let salt = masterPasswordSalt ?? Data(repeating: 0, count: 32)
        var saltLen = UInt8(salt.count)
        result.append(Data(bytes: &saltLen, count: 1)) // 1 byte
        result.append(salt) // 32 bytes
        result.append(encryptedPayload) // 12-byte IV + cipher + 16-byte tag
        return result
    }
    
    func decryptDataV4WithKey(_ data: Data, vaultKey: SecureBytes) -> Data? {
        guard data.count >= 69 else { return nil }
        let magic = data.prefix(4)
        guard magic == Self.vaultMagicV4 else { return nil }
        
        let saltLen = Int(data[8])
        guard data.count >= 9 + saltLen + 28 else { return nil }
        
        let payload = data.subdata(in: (9 + saltLen)..<data.count)
        return vaultKey.withUnsafeBytes { keyBuf -> Data? in
            guard let base = keyBuf.baseAddress, keyBuf.count >= 32 else { return nil }
            let keyData = Data(bytes: base, count: 32)
            return try? vaultDecryptPayload(key: keyData, encryptedData: payload)
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
    
    func loadVaultLocked(vaultKey: SecureBytes) -> [PasswordVaultEntry]? {
        guard let encryptedData = try? Data(contentsOf: vaultFileURL),
              let rawJSON = decryptDataV4WithKey(encryptedData, vaultKey: vaultKey) else {
            return nil
        }
        return try? JSONDecoder().decode([PasswordVaultEntry].self, from: rawJSON)
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

    func persistMutationLocked(previousEntries: [PasswordVaultEntry]) -> Bool {
        guard let vaultKey = activeDerivedVaultKey,
              let oldCiphertext = try? Data(contentsOf: vaultFileURL),
              let oldPayload = decryptDataV4WithKey(oldCiphertext, vaultKey: vaultKey),
              let diskEntries = try? JSONDecoder().decode([PasswordVaultEntry].self, from: oldPayload),
              diskEntries == previousEntries,
              let newPayload = try? JSONEncoder().encode(entries),
              let newCiphertext = encryptDataV4WithKey(newPayload, vaultKey: vaultKey) else {
            return false
        }
        do {
            try newCiphertext.write(to: vaultFileURL, options: .atomic)
            guard loadVaultLocked(vaultKey: vaultKey) == entries else {
                try oldCiphertext.write(to: vaultFileURL, options: .atomic)
                return false
            }
            return true
        } catch {
            TTLogger.error("Unable to persist vault entry mutation: \(error.localizedDescription)")
            return false
        }
    }
}

// MARK: - Hardware-Backed Biometric Keychain Storage (SecAccessControl)

extension PasswordVaultManager {

    enum BiometricKeyRead {
        case success(Data)
        case missing
        case cancelled
        case unavailable
        case failure(OSStatus)
    }

    private var biometricAccountKey: String { "TTZipVaultDerivedKey_Biometric_v2" }
    private var legacyBiometricAccountKey: String { "TTZipVaultDerivedKey_Biometric" }
    private var biometricService: String { "com.ttzip.app.vault.biometric" }
    
    func saveBiometricVaultKey(vaultKey: SecureBytes) {
        if PasswordVaultManager.isCLIProcess { return }
        
        var accessError: Unmanaged<CFError>?
        guard let accessControl = SecAccessControlCreateWithFlags(
            kCFAllocatorDefault,
            kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
            .biometryCurrentSet,
            &accessError
        ) else {
            accessError?.release()
            return
        }
        
        vaultKey.withUnsafeBytes { keyBuf in
            guard let keyBase = keyBuf.baseAddress else { return }
            let keyData = Data(bytes: keyBase, count: keyBuf.count)
            
            let query: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: biometricService,
                kSecAttrAccount as String: biometricAccountKey
            ]
            let add: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: biometricService,
                kSecAttrAccount as String: biometricAccountKey,
                kSecAttrAccessControl as String: accessControl,
                kSecValueData as String: keyData
            ]
            let updateStatus = SecItemUpdate(query as CFDictionary, [kSecValueData as String: keyData] as CFDictionary)
            if updateStatus == errSecSuccess { return }
            if updateStatus != errSecItemNotFound && updateStatus != errSecAuthFailed {
                TTLogger.error("Unable to update biometric vault key (status: \(updateStatus))")
                return
            }
            if updateStatus == errSecAuthFailed {
                let deleteStatus = SecItemDelete(query as CFDictionary)
                guard deleteStatus == errSecSuccess || deleteStatus == errSecItemNotFound else {
                    TTLogger.error("Unable to replace invalid biometric vault key (status: \(deleteStatus))")
                    return
                }
            }
            var addStatus = SecItemAdd(add as CFDictionary, nil)
            if addStatus == errSecDuplicateItem && updateStatus == errSecItemNotFound {
                let deleteStatus = SecItemDelete(query as CFDictionary)
                if deleteStatus == errSecSuccess {
                    addStatus = SecItemAdd(add as CFDictionary, nil)
                } else {
                    TTLogger.error("Unable to replace duplicate biometric vault key (status: \(deleteStatus))")
                    return
                }
            }
            if addStatus != errSecSuccess {
                TTLogger.error("Unable to store biometric vault key (status: \(addStatus))")
            }
        }
    }

    func loadBiometricVaultKey() -> BiometricKeyRead {
        if PasswordVaultManager.isCLIProcess { return .unavailable }
        let context = LAContext()
        context.localizedReason = TTZipLocalizationManager.shared.string(for: L10n.Vault.biometricReason)
        return readBiometricVaultKey(account: biometricAccountKey, context: context)
    }

    private func readBiometricVaultKey(account: String, context: LAContext) -> BiometricKeyRead {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: biometricService,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
            kSecUseAuthenticationContext as String: context
        ]
        
        var dataTypeRef: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &dataTypeRef)
        if status == errSecSuccess, let data = dataTypeRef as? Data {
            return .success(data)
        }
        switch status {
        case errSecItemNotFound: return .missing
        case errSecUserCanceled: return .cancelled
        case errSecInteractionNotAllowed, errSecAuthFailed: return .unavailable
        default: return .failure(status)
        }
    }
    
    func deleteBiometricVaultKey() {
        if PasswordVaultManager.isCLIProcess { return }
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: biometricService,
            kSecAttrAccount as String: biometricAccountKey
        ]
        SecItemDelete(query as CFDictionary)
        let legacyQuery: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: biometricService,
            kSecAttrAccount as String: legacyBiometricAccountKey
        ]
        SecItemDelete(legacyQuery as CFDictionary)
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
