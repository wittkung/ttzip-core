// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import TTZipCoreBindings
import TTZipCoreTypes
import Foundation

extension PasswordVaultManager {
    public func addEntry(id: UUID = UUID(), label: String, password: String, category: String = "General") {
        _ = addEntryVerified(id: id, label: label, password: password, category: category)
    }

    /// Reports success only after the encrypted file is written and read back.
    public func addEntryVerified(id: UUID = UUID(), label: String, password: String, category: String = "General") -> VaultEntrySaveResult {
        let result: VaultEntrySaveResult = vaultLock.withLock {
            guard _isUnlocked else { return .locked }
            guard !entries.contains(where: { $0.id == id }) else { return .duplicateID }
            let previous = entries
            let finalLabel = label.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? "Password" : label.trimmingCharacters(in: .whitespacesAndNewlines)
            let finalCategory = category.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? "General" : category.trimmingCharacters(in: .whitespacesAndNewlines)
            let newEntry = PasswordVaultEntry(id: id, label: finalLabel, password: password, category: finalCategory)
            entries.append(newEntry)
            guard persistMutationLocked(previousEntries: previous) else {
                entries = previous
                return .writeFailed
            }
            return .saved(id)
        }
        if case .saved = result { notifyChange() }
        return result
    }

    public func removeEntry(id: UUID) {
        _ = removeEntryVerified(id: id)
    }

    public func removeEntryVerified(id: UUID) -> VaultEntryDeleteResult {
        let result: VaultEntryDeleteResult = vaultLock.withLock {
            guard _isUnlocked else { return .locked }
            guard entries.contains(where: { $0.id == id }) else { return .notFound }
            let previous = entries
            entries.removeAll { $0.id == id }
            guard persistMutationLocked(previousEntries: previous) else {
                entries = previous
                return .writeFailed
            }
            return .removed
        }
        if result == .removed { notifyChange() }
        return result
    }

    public func recordUsage(id: UUID) {
        let saved: Bool = vaultLock.withLock {
            guard _isUnlocked else { return false }
            if let idx = entries.firstIndex(where: { $0.id == id }) {
                let previous = entries
                entries[idx].useCount += 1
                entries[idx].lastUsedAt = Date()
                guard persistMutationLocked(previousEntries: previous) else {
                    entries = previous
                    return false
                }
                return true
            }
            return false
        }
        if saved { notifyChange() }
    }

    // MARK: - Password Generation & Strength Evaluation

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
