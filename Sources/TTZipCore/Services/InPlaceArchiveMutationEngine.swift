// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import Foundation

// MARK: - Scoped Temporary Directory RAII Guard

/// RAII resource guard managing temporary directory lifecycle to guarantee atomic cleanup on scope exit or failure.
final class ScopedTempDirectory: @unchecked Sendable {
    let url: URL
    private var isDisarmed: Bool = false
    private let lock = NSLock()
    
    init(prefix: String) throws {
        let uniqueName = "\(prefix)_\(UUID().uuidString)"
        self.url = FileManager.default.temporaryDirectory.appendingPathComponent(uniqueName, isDirectory: true)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    }
    
    var path: String {
        url.path
    }
    
    /// Disarms the guard when ownership is transferred (e.g. to an active session).
    func disarm() {
        lock.lock()
        defer { lock.unlock() }
        isDisarmed = true
    }
    
    /// Cleans up the directory unless disarmed.
    func cleanup() {
        lock.lock()
        defer { lock.unlock() }
        if !isDisarmed {
            try? FileManager.default.removeItem(at: url)
        }
    }
    
    deinit {
        cleanup()
    }
}

// MARK: - In-Place Archive Mutation Engine

/// High-level in-place archive mutation and live editing coordinator with transactional lifecycle guarantees.
public final class InPlaceArchiveMutationEngine: @unchecked Sendable {
    public static let shared = InPlaceArchiveMutationEngine()
    
    private let lock = NSLock()
    private var activeSessions: [String: InPlaceEditSession] = [:]
    
    private init() {}
    
    /// Begins a live in-place editing session by extracting the target entry to an isolated temporary staging file.
    /// Guarantees 100% atomic rollback and directory cleanup if extraction, disk write, or hashing fails.
    public func beginEditingSession(
        archivePath: String,
        entryPath: String,
        password: String? = nil
    ) async throws -> InPlaceEditSession {
        guard !archivePath.isEmpty, !entryPath.isEmpty else {
            throw ArchiveError.fileNotFound
        }
        
        let fm = FileManager.default
        guard fm.fileExists(atPath: archivePath) else {
            throw ArchiveError.fileNotFound
        }
        
        let tempGuard = try ScopedTempDirectory(prefix: "TTZip_Edit")
        defer { tempGuard.cleanup() }
        
        let stagedFile = tempGuard.url.appendingPathComponent(entryPath)
        let parentDir = stagedFile.deletingLastPathComponent()
        if !fm.fileExists(atPath: parentDir.path) {
            try fm.createDirectory(at: parentDir, withIntermediateDirectories: true)
        }
        
        guard let data = try await ArchiveSelectiveExtractor.shared.extractSingleEntryData(
            archivePath: archivePath,
            entryPath: entryPath,
            password: password
        ) else {
            throw ArchiveError.fileNotFound
        }
        
        try data.write(to: stagedFile, options: .atomic)
        
        let stagedFilePath = stagedFile.path
        let initialHash = try await NativeComputeDispatcher.shared.dispatchCompute(qos: .userInitiated) {
            let initialDiff = try? inspectStagingFileMutation(stagedPath: stagedFilePath, initialHash: "")
            return initialDiff?.newHash ?? (HashCalculator.calculateSHA256(filePath: stagedFilePath) ?? "")
        }
        
        let mtime = (try? fm.attributesOfItem(atPath: stagedFilePath)[.modificationDate] as? Date)?.timeIntervalSince1970 
            ?? Date().timeIntervalSince1970
        
        let session = InPlaceEditSession(
            archivePath: archivePath,
            entryPath: entryPath,
            stagedFilePath: stagedFilePath,
            stagedDirectoryPath: tempGuard.path,
            state: .active,
            initialHash: initialHash,
            lastKnownMtime: mtime
        )
        
        setSession(session)
        tempGuard.disarm()
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
    
    /// Asynchronously inspects whether an active staging file has mutated against its initial hash using pure Rust offloaded to background.
    public func inspectSessionMutationAsync(session: InPlaceEditSession) async throws -> UniFfiTransactionDiff {
        try await NativeComputeDispatcher.shared.dispatchCompute(qos: .userInitiated) {
            try self.inspectSessionMutation(session: session)
        }
    }
    
    /// Directly commits changes from a live editing session back into the target archive if modified.
    @discardableResult
    public func commitChangesDirectly(
        session: InPlaceEditSession,
        password: String? = nil
    ) async throws -> Bool {
        guard FileManager.default.fileExists(atPath: session.stagedFilePath) else { return false }
        
        let diff = try await NativeComputeDispatcher.shared.dispatchCompute(qos: .userInitiated) {
            try inspectStagingFileMutation(
                stagedPath: session.stagedFilePath,
                initialHash: session.initialHash
            )
        }
        
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
        try await NativeComputeDispatcher.shared.dispatchCompute(qos: .userInitiated) { [weak self] in
            guard let self = self else { return }
            try self.addFilesToArchiveSync(
                archivePath: archivePath,
                sourceFilePaths: sourceFilePaths,
                destinationVirtualFolder: destinationVirtualFolder,
                password: password
            )
        }
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
        VFSLz4CachePool.shared.clearSession(sessionId: archivePath)
    }
    
    /// Deletes specific entries from inside an existing archive using 100% pure Rust UniFFI in-place mutation.
    public func deleteEntriesFromArchive(
        archivePath: String,
        entryPathsToDelete: [String],
        password: String? = nil
    ) async throws {
        try await NativeComputeDispatcher.shared.dispatchCompute(qos: .userInitiated) { [weak self] in
            guard let self = self else { return }
            try self.deleteEntriesFromArchiveSync(
                archivePath: archivePath,
                entryPathsToDelete: entryPathsToDelete,
                password: password
            )
        }
    }
    
    /// Synchronously deletes specific entries from inside an existing archive using 100% pure Rust UniFFI in-place mutation.
    public func deleteEntriesFromArchiveSync(
        archivePath: String,
        entryPathsToDelete: [String],
        password: String? = nil
    ) throws {
        guard !entryPathsToDelete.isEmpty else { return }
        
        let actions = entryPathsToDelete.map { entry in
            InPlaceMutationAction(isDelete: true, entryPath: entry, sourcePath: nil)
        }
        
        try inPlaceMutateArchive(archivePath: archivePath, actions: actions)
        VFSLz4CachePool.shared.clearSession(sessionId: archivePath)
    }
    
    /// Removes specified files or directories from inside an existing archive.
    public func removeFilesFromArchive(
        archivePath: String,
        entryPaths: [String],
        password: String? = nil
    ) async throws {
        try await deleteEntriesFromArchive(
            archivePath: archivePath,
            entryPathsToDelete: entryPaths,
            password: password
        )
    }
    
    /// Synchronously removes specified files or directories from inside an existing archive.
    public func removeFilesFromArchiveSync(
        archivePath: String,
        entryPaths: [String],
        password: String? = nil
    ) throws {
        try deleteEntriesFromArchiveSync(
            archivePath: archivePath,
            entryPathsToDelete: entryPaths,
            password: password
        )
    }
    
    /// Replaces an existing entry in the archive with an external file.
    public func replaceEntry(
        archivePath: String,
        entryPath: String,
        sourceFilePath: String,
        password: String? = nil
    ) async throws {
        try await NativeComputeDispatcher.shared.dispatchCompute(qos: .userInitiated) { [weak self] in
            guard let self = self else { return }
            try self.replaceEntrySync(
                archivePath: archivePath,
                entryPath: entryPath,
                sourceFilePath: sourceFilePath,
                password: password
            )
        }
    }
    
    /// Synchronously replaces an existing entry in the archive with an external file.
    public func replaceEntrySync(
        archivePath: String,
        entryPath: String,
        sourceFilePath: String,
        password: String? = nil
    ) throws {
        let action = InPlaceMutationAction(isDelete: false, entryPath: entryPath, sourcePath: sourceFilePath)
        try inPlaceMutateArchive(archivePath: archivePath, actions: [action])
        VFSLz4CachePool.shared.clearSession(sessionId: archivePath)
    }
    
    /// Atomically renames an entry inside an existing archive.
    public func renameEntry(
        archivePath: String,
        oldEntryPath: String,
        newEntryPath: String,
        password: String? = nil
    ) async throws {
        try await NativeComputeDispatcher.shared.dispatchCompute(qos: .userInitiated) { [weak self] in
            guard let self = self else { return }
            try self.renameEntrySync(
                archivePath: archivePath,
                oldEntryPath: oldEntryPath,
                newEntryPath: newEntryPath,
                password: password
            )
        }
    }
    
    /// Asynchronously renames an entry inside an existing archive (alias for renameEntry).
    public func renameEntryAsync(
        archivePath: String,
        oldEntryPath: String,
        newEntryPath: String,
        password: String? = nil
    ) async throws {
        try await renameEntry(
            archivePath: archivePath,
            oldEntryPath: oldEntryPath,
            newEntryPath: newEntryPath,
            password: password
        )
    }
    
    /// Synchronously renames an entry inside an existing archive with RAII staging lifecycle.
    public func renameEntrySync(
        archivePath: String,
        oldEntryPath: String,
        newEntryPath: String,
        password: String? = nil
    ) throws {
        guard oldEntryPath != newEntryPath else { return }
        guard !oldEntryPath.isEmpty, !newEntryPath.isEmpty else {
            throw ArchiveError.fileNotFound
        }
        
        let tempGuard = try ScopedTempDirectory(prefix: "TTZip_Rename")
        defer { tempGuard.cleanup() }
        
        let tempStagingFile = tempGuard.url.appendingPathComponent("staged_entry")
        let data: Data
        do {
            data = try extractSingleEntryByPath(archivePath: archivePath, entryPath: oldEntryPath, password: password)
        } catch {
            throw ArchiveError.fileNotFound
        }
        
        try data.write(to: tempStagingFile, options: .atomic)
        
        let actions = [
            InPlaceMutationAction(isDelete: false, entryPath: newEntryPath, sourcePath: tempStagingFile.path),
            InPlaceMutationAction(isDelete: true, entryPath: oldEntryPath, sourcePath: nil)
        ]
        try inPlaceMutateArchive(archivePath: archivePath, actions: actions)
        VFSLz4CachePool.shared.clearSession(sessionId: archivePath)
        
        updateActiveSessionsForRename(
            archivePath: archivePath,
            oldEntryPath: oldEntryPath,
            newEntryPath: newEntryPath
        )
    }
    
    /// Closes and cleans up an in-place editing session and its temporary directory.
    public func closeEditingSession(session: InPlaceEditSession) {
        FileWatcherEngine.shared.stopWatching(sessionKey: session.sessionId)
        removeSession(sessionId: session.sessionId)
        try? FileManager.default.removeItem(atPath: session.stagedDirectoryPath)
    }
    
    /// Closes and cleans up an in-place editing session by session ID.
    public func closeEditingSession(sessionId: String) {
        FileWatcherEngine.shared.stopWatching(sessionKey: sessionId)
        if let session = getSession(sessionId: sessionId) {
            removeSession(sessionId: sessionId)
            try? FileManager.default.removeItem(atPath: session.stagedDirectoryPath)
        } else {
            removeSession(sessionId: sessionId)
        }
    }
    
    /// Discards an active editing session, reverting any unsaved changes and removing temporary files.
    public func discardEditingSession(session: InPlaceEditSession) {
        closeEditingSession(session: session)
    }
    
    /// Discards an active editing session by session ID.
    public func discardEditingSession(sessionId: String) {
        closeEditingSession(sessionId: sessionId)
    }
    
    /// Closes all active editing sessions and cleans up all associated temporary directories.
    public func closeAllSessions() {
        lock.lock()
        let sessions = Array(activeSessions.values)
        activeSessions.removeAll()
        lock.unlock()
        
        for session in sessions {
            FileWatcherEngine.shared.stopWatching(sessionKey: session.sessionId)
            try? FileManager.default.removeItem(atPath: session.stagedDirectoryPath)
        }
    }
    
    /// Retrieves an active in-place editing session by its ID.
    public func getSession(sessionId: String) -> InPlaceEditSession? {
        lock.lock()
        defer { lock.unlock() }
        return activeSessions[sessionId]
    }
    
    /// Returns all currently registered active editing sessions.
    public func allActiveSessions() -> [InPlaceEditSession] {
        lock.lock()
        defer { lock.unlock() }
        return Array(activeSessions.values)
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
    
    private func updateActiveSessionsForRename(archivePath: String, oldEntryPath: String, newEntryPath: String) {
        lock.lock()
        defer { lock.unlock() }
        for (key, session) in activeSessions {
            if session.archivePath == archivePath && session.entryPath == oldEntryPath {
                let updated = InPlaceEditSession(
                    sessionId: session.sessionId,
                    archivePath: session.archivePath,
                    entryPath: newEntryPath,
                    stagedFilePath: session.stagedFilePath,
                    stagedDirectoryPath: session.stagedDirectoryPath,
                    state: session.state,
                    initialHash: session.initialHash,
                    lastKnownMtime: session.lastKnownMtime,
                    hasUnsavedChanges: session.hasUnsavedChanges,
                    errorMessage: session.errorMessage
                )
                activeSessions[key] = updated
            }
        }
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

// MARK: - Sendable Conformance Extensions

extension UniFfiTransactionDiff: @unchecked Sendable {}
extension InPlaceMutationAction: @unchecked Sendable {}
