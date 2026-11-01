// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

extension PasswordVaultManager {
    public func archivePassword(logicalKey: String, contentKey: String) throws -> String? {
        guard !logicalKey.isEmpty, !contentKey.isEmpty else {
            throw ArchiveVaultCredentialError.invalidIdentity
        }
        return try vaultLock.withLock {
            guard _isUnlocked else { throw ArchiveVaultCredentialError.locked }
            return entries.first {
                $0.archiveScope == ArchiveVaultScope(logicalKey: logicalKey, contentKey: contentKey)
            }?.password
        }
    }

    public func upsertArchivePassword(
        _ password: String, logicalKey: String, contentKey: String, label: String
    ) throws {
        guard !password.isEmpty, !logicalKey.isEmpty, !contentKey.isEmpty else {
            throw ArchiveVaultCredentialError.invalidIdentity
        }
        try mutateArchiveCredential {
            let old = $0.first { $0.archiveScope?.logicalKey == logicalKey }
            var next = $0.filter { $0.archiveScope?.logicalKey != logicalKey }
            next.append(PasswordVaultEntry(
                id: old?.id ?? UUID(),
                label: String(label.prefix(128)),
                password: password,
                category: "Archive",
                createdAt: old?.createdAt ?? Date(),
                archiveScope: ArchiveVaultScope(logicalKey: logicalKey, contentKey: contentKey)
            ))
            return next
        }
    }

    public func removeArchivePassword(logicalKey: String) throws {
        guard !logicalKey.isEmpty else { throw ArchiveVaultCredentialError.invalidIdentity }
        try mutateArchiveCredential {
            $0.filter { $0.archiveScope?.logicalKey != logicalKey }
        }
    }

    public func removeArchivePassword(
        logicalKey: String, contentKey: String, ifPasswordMatches password: String
    ) throws {
        guard !logicalKey.isEmpty, !contentKey.isEmpty else {
            throw ArchiveVaultCredentialError.invalidIdentity
        }
        try mutateArchiveCredential {
            $0.filter {
                $0.archiveScope?.logicalKey != logicalKey
                    || $0.archiveScope?.contentKey != contentKey
                    || $0.password != password
            }
        }
    }

    private func mutateArchiveCredential(
        _ transform: ([PasswordVaultEntry]) -> [PasswordVaultEntry]
    ) throws {
        let changed = try vaultLock.withLock {
            guard _isUnlocked else { throw ArchiveVaultCredentialError.locked }
            let previous = entries
            let next = transform(previous)
            guard next != previous else { return false }
            entries = next
            guard persistMutationLocked(previousEntries: previous) else {
                entries = previous
                throw ArchiveVaultCredentialError.writeFailed
            }
            return true
        }
        if changed { notifyChange() }
    }
}
