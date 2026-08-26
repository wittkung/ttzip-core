// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

/// High-level in-place archive mutation and live editing coordinator.
public final class InPlaceArchiveMutationEngine: @unchecked Sendable {
    public static let shared = InPlaceArchiveMutationEngine()
    
    private let lock = NSLock()
    private var activeSessions: [String: InPlaceEditSession] = [:]
    
    private init() {}
    
    /// Begins a live in-place editing session by extracting target entry to an isolated temporary staging file.
    public func beginEditingSession(
        archivePath: String,
        entryPath: String,
        password: String? = nil
    ) async throws -> InPlaceEditSession {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("TTZip_Edit_\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        
        let fm = FileManager.default
        let stagedFile = tempDir.appendingPathComponent(entryPath)
        let parentDir = stagedFile.deletingLastPathComponent()
        try? fm.createDirectory(at: parentDir, withIntermediateDirectories: true)
        
        guard let data = try await ArchiveSelectiveExtractor.shared.extractSingleEntryData(
            archivePath: archivePath,
            entryPath: entryPath,
            password: password
        ) else {
            throw ArchiveError.fileNotFound
        }
        try data.write(to: stagedFile, options: .atomic)
        
        let initialDiff = try? inspectStagingFileMutation(stagedPath: stagedFile.path, initialHash: "")
        let initialHash = initialDiff?.newHash ?? (HashCalculator.calculateSHA256(filePath: stagedFile.path) ?? "")
        
        let session = InPlaceEditSession(
            archivePath: archivePath,
            entryPath: entryPath,
            stagedFilePath: stagedFile.path,
            stagedDirectoryPath: tempDir.path,
            state: .active,
            initialHash: initialHash,
            lastKnownMtime: (try? fm.attributesOfItem(atPath: stagedFile.path)[.modificationDate] as? Date)?.timeIntervalSince1970 ?? Date().timeIntervalSince1970
        )
        
        setSession(session)
        return session
    }
    
    /// Starts watching an active editing session and auto-syncs modifications back into the archive.
    public func startWatchingAndAutoSync(
        session: InPlaceEditSession,
        password: String? = nil,
        onSyncCompleted: (@Sendable (InPlaceEditSession, Result<Void, Error>) -> Void)? = nil
    ) {
        FileWatcherEngine.shared.watchEditingSession(session: session) { [weak self] updatedSession in
            Task {
                do {
                    try await self?.syncModifiedFile(session: updatedSession, password: password)
                    onSyncCompleted?(updatedSession, .success(()))
                } catch {
                    onSyncCompleted?(updatedSession, .failure(error))
                }
            }
        }
    }
    
    /// Inspects whether an active staging file has mutated against its initial hash using pure Rust.
    public func inspectSessionMutation(session: InPlaceEditSession) throws -> UniFfiTransactionDiff {
        try inspectStagingFileMutation(stagedPath: session.stagedFilePath, initialHash: session.initialHash)
    }
    
    /// Directly commits changes from a live editing session back into the target archive if modified.
    @discardableResult
    public func commitChangesDirectly(
        session: InPlaceEditSession,
        password: String? = nil
    ) async throws -> Bool {
        guard FileManager.default.fileExists(atPath: session.stagedFilePath) else { return false }
        
        let diff = try inspectStagingFileMutation(
            stagedPath: session.stagedFilePath,
            initialHash: session.initialHash
        )
        
        guard diff.hasChanged else {
            return false
        }
        
        try await addFilesToArchive(
            archivePath: session.archivePath,
            sourceFilePaths: [session.stagedFilePath],
            destinationVirtualFolder: (session.entryPath as NSString).deletingLastPathComponent,
            password: password
        )
        
        var updated = session
        updated.initialHash = diff.newHash
        updated.hasUnsavedChanges = false
        setSession(updated)
        return true
    }
    
    /// Syncs modified staged file back into the target archive.
    public func syncModifiedFile(session: InPlaceEditSession, password: String? = nil) async throws {
        try await commitChangesDirectly(session: session, password: password)
    }
    
    /// Adds files into an existing archive using in-place mutation or transactional repacking.
    public func addFilesToArchive(
        archivePath: String,
        sourceFilePaths: [String],
        destinationVirtualFolder: String? = nil,
        password: String? = nil
    ) async throws {
        try addFilesToArchiveSync(
            archivePath: archivePath,
            sourceFilePaths: sourceFilePaths,
            destinationVirtualFolder: destinationVirtualFolder,
            password: password
        )
    }
    
    /// Synchronously adds files into an existing archive using 100% pure Rust UniFFI in-place mutation.
    public func addFilesToArchiveSync(
        archivePath: String,
        sourceFilePaths: [String],
        destinationVirtualFolder: String? = nil,
        password: String? = nil
    ) throws {
        guard !sourceFilePaths.isEmpty else { return }
        
        let actions = sourceFilePaths.map { src -> InPlaceMutationAction in
            let baseName = (src as NSString).lastPathComponent
            let targetRelPath: String
            if let destFolder = destinationVirtualFolder, !destFolder.isEmpty, destFolder != "." {
                targetRelPath = (destFolder as NSString).appendingPathComponent(baseName)
            } else {
                targetRelPath = baseName
            }
            return InPlaceMutationAction(isDelete: false, entryPath: targetRelPath, sourcePath: src)
        }
        
        try inPlaceMutateArchive(archivePath: archivePath, actions: actions)
    }
    
    /// Deletes specific entries from inside an existing archive using 100% pure Rust UniFFI in-place mutation.
    public func deleteEntriesFromArchive(
        archivePath: String,
        entryPathsToDelete: [String],
        password: String? = nil
    ) async throws {
        guard !entryPathsToDelete.isEmpty else { return }
        
        let actions = entryPathsToDelete.map { entry in
            InPlaceMutationAction(isDelete: true, entryPath: entry, sourcePath: nil)
        }
        
        try inPlaceMutateArchive(archivePath: archivePath, actions: actions)
    }
    
    /// Closes and cleans up an in-place editing session and its temporary directory.
    public func closeEditingSession(session: InPlaceEditSession) {
        removeSession(sessionId: session.sessionId)
        try? FileManager.default.removeItem(atPath: session.stagedDirectoryPath)
    }
    
    /// Retrieves an active in-place editing session by its ID.
    public func getSession(sessionId: String) -> InPlaceEditSession? {
        lock.lock()
        defer { lock.unlock() }
        return activeSessions[sessionId]
    }
    
    private func setSession(_ session: InPlaceEditSession) {
        lock.lock()
        defer { lock.unlock() }
        activeSessions[session.sessionId] = session
    }
    
    private func removeSession(sessionId: String) {
        lock.lock()
        defer { lock.unlock() }
        activeSessions.removeValue(forKey: sessionId)
    }
}

// MARK: - In-Place Edit Session

/// State of an in-place editing session.
public enum InPlaceEditState: String, Sendable, Codable {
    case active
    case syncing
    case completed
    case failed
}

/// In-place editing session data model for tracking live temporary files.
public struct InPlaceEditSession: Sendable, Identifiable, Equatable {
    public var id: String { sessionId }
    public let sessionId: String
    public let archivePath: String
    public let entryPath: String
    public let stagedFilePath: String
    public let stagedDirectoryPath: String
    public var state: InPlaceEditState
    public var initialHash: String
    public var lastKnownMtime: Double
    public var hasUnsavedChanges: Bool
    public var errorMessage: String?
    
    public init(
        sessionId: String = UUID().uuidString,
        archivePath: String,
        entryPath: String,
        stagedFilePath: String,
        stagedDirectoryPath: String,
        state: InPlaceEditState = .active,
        initialHash: String = "",
        lastKnownMtime: Double = Date().timeIntervalSince1970,
        hasUnsavedChanges: Bool = false,
        errorMessage: String? = nil
    ) {
        self.sessionId = sessionId
        self.archivePath = archivePath
        self.entryPath = entryPath
        self.stagedFilePath = stagedFilePath
        self.stagedDirectoryPath = stagedDirectoryPath
        self.state = state
        self.initialHash = initialHash
        self.lastKnownMtime = lastKnownMtime
        self.hasUnsavedChanges = hasUnsavedChanges
        self.errorMessage = errorMessage
    }
}
