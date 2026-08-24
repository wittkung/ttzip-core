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
        
        let session = InPlaceEditSession(
            archivePath: archivePath,
            entryPath: entryPath,
            stagedFilePath: stagedFile.path,
            stagedDirectoryPath: tempDir.path,
            state: .active,
            initialHash: HashCalculator.calculateSHA256(filePath: stagedFile.path) ?? "",
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
    
    /// Syncs modified staged file back into the target archive.
    public func syncModifiedFile(session: InPlaceEditSession, password: String? = nil) async throws {
        guard FileManager.default.fileExists(atPath: session.stagedFilePath) else { return }
        
        try await addFilesToArchive(
            archivePath: session.archivePath,
            sourceFilePaths: [session.stagedFilePath],
            destinationVirtualFolder: (session.entryPath as NSString).deletingLastPathComponent,
            password: password
        )
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
    
    /// Synchronously adds files into an existing archive using in-place mutation.
    public func addFilesToArchiveSync(
        archivePath: String,
        sourceFilePaths: [String],
        destinationVirtualFolder: String? = nil,
        password: String? = nil
    ) throws {
        guard !sourceFilePaths.isEmpty else { return }
        
        let tempExtractDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("TTZip_Repack_\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempExtractDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempExtractDir) }

        let extractor = ArchiveExtractor()
        _ = try extractor.extractSync(archivePath: archivePath, destinationDir: tempExtractDir.path, password: password)

        let fm = FileManager.default
        for src in sourceFilePaths {
            let baseName = (src as NSString).lastPathComponent
            let targetRelPath: String
            if let destFolder = destinationVirtualFolder, !destFolder.isEmpty, destFolder != "." {
                targetRelPath = (destFolder as NSString).appendingPathComponent(baseName)
            } else {
                targetRelPath = baseName
            }
            let targetFull = tempExtractDir.appendingPathComponent(targetRelPath)
            try? fm.createDirectory(at: targetFull.deletingLastPathComponent(), withIntermediateDirectories: true)
            try? fm.removeItem(at: targetFull)
            try fm.copyItem(atPath: src, toPath: targetFull.path)
        }

        let tempOutputArchive = FileManager.default.temporaryDirectory
            .appendingPathComponent("TTZip_Repacked_\(UUID().uuidString).zip")
        defer { try? FileManager.default.removeItem(at: tempOutputArchive) }

        let items = (try? fm.contentsOfDirectory(atPath: tempExtractDir.path)) ?? []
        let inputPaths = items.map { tempExtractDir.appendingPathComponent($0).path }

        let writer = ArchiveWriter()
        try writer.createArchiveSync(
            outputPath: tempOutputArchive.path,
            format: .zip,
            level: .normal,
            inputPaths: inputPaths,
            options: .defaultClean,
            password: password
        )

        try? fm.removeItem(atPath: archivePath)
        try fm.moveItem(atPath: tempOutputArchive.path, toPath: archivePath)
    }
    
    /// Deletes specific entries from inside an existing archive.
    public func deleteEntriesFromArchive(
        archivePath: String,
        entryPathsToDelete: [String],
        password: String? = nil
    ) async throws {
        guard !entryPathsToDelete.isEmpty else { return }
        
        let tempExtractDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("TTZip_DelRepack_\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempExtractDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempExtractDir) }

        let extractor = ArchiveExtractor()
        _ = try extractor.extractSync(archivePath: archivePath, destinationDir: tempExtractDir.path, password: password)

        let fm = FileManager.default
        for entry in entryPathsToDelete {
            let targetPath = tempExtractDir.appendingPathComponent(entry).path
            try? fm.removeItem(atPath: targetPath)
        }

        let tempOutputArchive = FileManager.default.temporaryDirectory
            .appendingPathComponent("TTZip_DelRepacked_\(UUID().uuidString).zip")
        defer { try? FileManager.default.removeItem(at: tempOutputArchive) }

        let items = (try? fm.contentsOfDirectory(atPath: tempExtractDir.path)) ?? []
        let inputPaths = items.map { tempExtractDir.appendingPathComponent($0).path }

        let writer = ArchiveWriter()
        try writer.createArchiveSync(
            outputPath: tempOutputArchive.path,
            format: .zip,
            level: .normal,
            inputPaths: inputPaths,
            options: .defaultClean,
            password: password
        )

        try? fm.removeItem(atPath: archivePath)
        try fm.moveItem(atPath: tempOutputArchive.path, toPath: archivePath)
    }
    
    /// Closes and cleans up an in-place editing session and its temporary directory.
    public func closeEditingSession(session: InPlaceEditSession) {
        removeSession(sessionId: session.sessionId)
        try? FileManager.default.removeItem(atPath: session.stagedDirectoryPath)
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
