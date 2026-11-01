// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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
}
