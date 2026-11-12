// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

internal struct ConfirmedArtifactIdentity: Equatable, Sendable {
    let path: String
    let inode: UInt64?
    let fileSize: Int64?
}

/// Concrete command encapsulating archive compression with transactional rollback support.
public final class CompressCommand: ArchiveCommandProtocol, @unchecked Sendable {
    public let commandId: String
    public let description: String
    public var isUndoable: Bool { true }

    public let inputs: [String]
    public let outputPath: String
    public let format: ArchiveCompressionFormat
    public let level: ArchiveCompressionLevel
    public let password: String?
    public let splitSize: Int64?
    public let filterOptions: ArchiveFilterOptions
    public let advancedOptions: ArchiveAdvancedOptions?
    public let progress: (@Sendable (ArchiveProgress) -> Void)?

    private let archiveWriter: ArchiveWriting
    private let lock = NSLock()

    private var createdArtifacts: [String] = []
    private var confirmedArtifacts: [ConfirmedArtifactIdentity] = []
    private var protectedCleanupPaths: Set<String> = []
    private var backupFileMap: [String: String] = [:]
    private var isExecutedState: Bool = false

    public init(
        commandId: String = UUID().uuidString,
        description: String? = nil,
        inputs: [String],
        outputPath: String,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        password: String? = nil,
        splitSize: Int64? = nil,
        filterOptions: ArchiveFilterOptions = ArchiveFilterOptions(),
        advancedOptions: ArchiveAdvancedOptions? = nil,
        progress: (@Sendable (ArchiveProgress) -> Void)? = nil,
        archiveWriter: ArchiveWriting = ArchiveWriter()
    ) {
        self.commandId = commandId
        self.inputs = inputs
        self.outputPath = outputPath
        self.format = format
        self.level = level
        self.password = password
        self.splitSize = splitSize
        self.filterOptions = filterOptions
        self.advancedOptions = advancedOptions
        self.progress = progress
        self.archiveWriter = archiveWriter

        let targetName = (outputPath as NSString).lastPathComponent
        self.description = description ?? "Compress files to [\(targetName)] (\(format.rawValue.uppercased()))"
    }

    public convenience init(
        commandId: String = UUID().uuidString,
        description: String? = nil,
        inputs: [String],
        outputPath: String,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        password: String? = nil,
        splitSize: Int64? = nil,
        filterOptions: ArchiveFilterOptions = ArchiveFilterOptions(),
        advancedOptions: ArchiveAdvancedOptions? = nil,
        progress: (@Sendable (ArchiveProgress) -> Void)? = nil,
        engineFacade: TTZipEngineFacading?
    ) {
        self.init(
            commandId: commandId,
            description: description,
            inputs: inputs,
            outputPath: outputPath,
            format: format,
            level: level,
            password: password,
            splitSize: splitSize,
            filterOptions: filterOptions,
            advancedOptions: advancedOptions,
            progress: progress,
            archiveWriter: ArchiveWriter()
        )
    }

    deinit {
        purgeBackupResources()
    }

    public func execute() async throws -> CommandResult {
        let startTime = CFAbsoluteTimeGetCurrent()
        let outputDir = (outputPath as NSString).deletingLastPathComponent
        let baseName = (outputPath as NSString).lastPathComponent
        let baseStem = (baseName as NSString).deletingPathExtension

        let backupDict = createPreExecutionBackups(
            outputDir: outputDir,
            baseName: baseName,
            baseStem: baseStem
        )
        let operationOutcome: ExtractionOutcome?
        do {
            operationOutcome = try await performArchiveWrite()
        } catch {
            rollbackPreExecutionBackups(backupDict)
            throw error
        }

        let duration = CFAbsoluteTimeGetCurrent() - startTime
        let protectedPaths = Self.buildProtectedCleanupPaths(
            from: operationOutcome,
            outputDir: outputDir
        )
        let confirmed = collectConfirmedArtifacts(
            outputDir: outputDir,
            baseName: baseName,
            baseStem: baseStem,
            protectedPaths: protectedPaths
        )
        let sortedArtifacts = confirmed.map(\.path)
        saveExecutionState(
            artifacts: sortedArtifacts,
            confirmedArtifacts: confirmed,
            protectedPaths: protectedPaths,
            backupMap: backupDict
        )
        removeSpotlightNoIndexMarker(in: outputDir)

        let (totalOrigBytes, totalCompressedBytes) = computeCompressionByteTotals(
            confirmedArtifacts: confirmed
        )
        return buildCommandResult(
            duration: duration,
            sortedArtifacts: sortedArtifacts,
            backupDict: backupDict,
            totalOrigBytes: totalOrigBytes,
            totalCompressedBytes: totalCompressedBytes,
            operationOutcome: operationOutcome
        )
    }

    public func undo() async throws {
        let snapshot = getUndoStateSnapshot()
        guard snapshot.executed else {
            throw CommandError.invalidState(reason: "Command has not been executed yet; cannot undo.")
        }

        removeConfirmedArtifactsForUndo(
            confirmedArtifacts: snapshot.confirmedArtifacts,
            protectedPaths: snapshot.protectedPaths,
            backups: snapshot.backups
        )
        let unRestoredBackups = restoreBackupsForUndo(
            backups: snapshot.backups,
            protectedPaths: snapshot.protectedPaths
        )

        if !unRestoredBackups.isEmpty {
            saveExecutionState(
                artifacts: [],
                confirmedArtifacts: [],
                protectedPaths: snapshot.protectedPaths,
                backupMap: unRestoredBackups
            )
            throw CommandError.undoFailed(reason: "Failed to restore all backup files during undo.")
        }
        resetExecutionStateOnUndoSuccess()
    }

    public func purgeBackupResources() {
        lock.lock()
        defer { lock.unlock() }
        let fm = FileManager.default
        for (_, backupPath) in backupFileMap {
            try? fm.removeItem(atPath: backupPath)
        }
        backupFileMap.removeAll()
    }

    // MARK: - Execution & Undo Helpers

    private func createPreExecutionBackups(
        outputDir: String,
        baseName: String,
        baseStem: String
    ) -> [String: String] {
        let fm = FileManager.default
        guard fm.fileExists(atPath: outputDir),
              let dirContents = try? fm.contentsOfDirectory(atPath: outputDir) else {
            return [:]
        }
        var backupDict: [String: String] = [:]
        for item in dirContents {
            guard item == baseName || !Self.isTransientOrBackupName(item) else { continue }
            guard item == baseName || isSplitVolumeMatch(fileName: item, baseName: baseName, baseStem: baseStem) else {
                continue
            }
            let fullPath = (outputDir as NSString).appendingPathComponent(item)
            let backupPathCandidate = "\(fullPath).bak_\(UUID().uuidString)"
            if Self.cloneOrCopyBackup(from: fullPath, to: backupPathCandidate, fileManager: fm) {
                backupDict[fullPath] = backupPathCandidate
            }
        }
        return backupDict
    }

    private static func cloneOrCopyBackup(
        from sourcePath: String,
        to backupPath: String,
        fileManager: FileManager
    ) -> Bool {
        #if os(macOS)
        if clonefile(sourcePath, backupPath, 0) == 0 {
            return true
        }
        #endif
        do {
            try fileManager.copyItem(atPath: sourcePath, toPath: backupPath)
            return true
        } catch {
            return false
        }
    }

    private func performArchiveWrite() async throws -> ExtractionOutcome? {
        let writeRequest = ArchiveWriteRequest(
            outputPath: outputPath,
            format: format,
            level: level,
            inputPaths: inputs,
            options: filterOptions,
            splitVolumeSizeBytes: splitSize,
            password: password,
            advancedOptions: advancedOptions ?? .defaultOptions,
            progressHandler: progress
        )
        if let defaultWriter = archiveWriter as? ArchiveWriter {
            let report = try await defaultWriter.createArchiveWithOutcome(writeRequest)
            return report.outcome
        }
        try await archiveWriter.createArchive(writeRequest)
        return nil
    }

    private func rollbackPreExecutionBackups(_ backupDict: [String: String]) {
        let fm = FileManager.default
        for (originalPath, backupPath) in backupDict {
            if fm.fileExists(atPath: originalPath) {
                try? fm.removeItem(atPath: originalPath)
            }
            if fm.fileExists(atPath: backupPath) {
                try? fm.moveItem(atPath: backupPath, toPath: originalPath)
            }
        }
    }

    internal static func buildProtectedCleanupPaths(
        from outcome: ExtractionOutcome?,
        outputDir: String
    ) -> Set<String> {
        guard let failedItems = outcome?.failed, !failedItems.isEmpty else { return [] }
        var paths = Set<String>()
        for item in failedItems {
            let raw = item.relativePath
            guard !raw.isEmpty else { continue }
            paths.insert(raw)
            paths.insert((raw as NSString).standardizingPath)
            let resolved = (raw as NSString).isAbsolutePath
                ? raw
                : (outputDir as NSString).appendingPathComponent(raw)
            paths.insert(resolved)
            paths.insert((resolved as NSString).standardizingPath)
        }
        return paths
    }

    private func collectConfirmedArtifacts(
        outputDir: String,
        baseName: String,
        baseStem: String,
        protectedPaths: Set<String>
    ) -> [ConfirmedArtifactIdentity] {
        let fm = FileManager.default
        var candidatePaths = Set<String>()
        if fm.fileExists(atPath: outputPath),
           shouldRecordConfirmedArtifact(
               fullPath: outputPath,
               fileName: baseName,
               baseName: baseName,
               baseStem: baseStem,
               protectedPaths: protectedPaths
           ) {
            candidatePaths.insert(outputPath)
        }
        if let dirContents = try? fm.contentsOfDirectory(atPath: outputDir) {
            for item in dirContents {
                let fullPath = (outputDir as NSString).appendingPathComponent(item)
                if shouldRecordConfirmedArtifact(
                    fullPath: fullPath,
                    fileName: item,
                    baseName: baseName,
                    baseStem: baseStem,
                    protectedPaths: protectedPaths
                ) {
                    candidatePaths.insert(fullPath)
                }
            }
        }
        return candidatePaths.sorted().map { Self.captureArtifactIdentity(atPath: $0, fileManager: fm) }
    }

    private func shouldRecordConfirmedArtifact(
        fullPath: String,
        fileName: String,
        baseName: String,
        baseStem: String,
        protectedPaths: Set<String>
    ) -> Bool {
        guard !Self.isProtectedCleanupPath(fullPath, protectedPaths: protectedPaths) else {
            return false
        }
        if fullPath == outputPath || fileName == baseName {
            return true
        }
        guard !Self.isTransientOrBackupName(fileName) else {
            return false
        }
        return isSplitVolumeMatch(fileName: fileName, baseName: baseName, baseStem: baseStem)
    }

    private static func captureArtifactIdentity(
        atPath path: String,
        fileManager: FileManager
    ) -> ConfirmedArtifactIdentity {
        let attr = try? fileManager.attributesOfItem(atPath: path)
        let inode = (attr?[.systemFileNumber] as? NSNumber)?.uint64Value
        let fileSize = (attr?[.size] as? NSNumber)?.int64Value
        return ConfirmedArtifactIdentity(path: path, inode: inode, fileSize: fileSize)
    }

    private func removeSpotlightNoIndexMarker(in outputDir: String) {
        let fm = FileManager.default
        let noIndexFilePath = (outputDir as NSString).appendingPathComponent(".noindex")
        if fm.fileExists(atPath: noIndexFilePath) {
            try? fm.removeItem(atPath: noIndexFilePath)
        }
    }

    private func computeCompressionByteTotals(
        confirmedArtifacts: [ConfirmedArtifactIdentity]
    ) -> (originalBytes: Int64, compressedBytes: Int64) {
        let fm = FileManager.default
        var totalOrigBytes: Int64 = 0
        for inputPath in inputs {
            guard let attr = try? fm.attributesOfItem(atPath: inputPath) else { continue }
            if (attr[.type] as? FileAttributeType) == .typeDirectory {
                let component = ArchiveComponentTreeBuilder.buildTree(fromDiskPath: inputPath)
                totalOrigBytes += component.sizeBytes
            } else {
                totalOrigBytes += (attr[.size] as? NSNumber)?.int64Value ?? 0
            }
        }
        let totalCompressedBytes = confirmedArtifacts.reduce(Int64(0)) { partial, item in
            if let recordedSize = item.fileSize {
                return partial + recordedSize
            }
            let attr = try? fm.attributesOfItem(atPath: item.path)
            return partial + ((attr?[.size] as? NSNumber)?.int64Value ?? 0)
        }
        return (totalOrigBytes, totalCompressedBytes)
    }

    private func buildCommandResult(
        duration: Double,
        sortedArtifacts: [String],
        backupDict: [String: String],
        totalOrigBytes: Int64,
        totalCompressedBytes: Int64,
        operationOutcome: ExtractionOutcome?
    ) -> CommandResult {
        var metadata: [String: String] = [
            "compressedSize": "\(totalCompressedBytes)",
            "originalSize": "\(totalOrigBytes)"
        ]
        let resultMessage: String
        if let outcome = operationOutcome, outcome.status == .completedWithFailures {
            metadata["outcomeStatus"] = "completedWithFailures"
            metadata["cleanupFailureCount"] = "\(outcome.failed.count)"
            metadata["cleanupFailedPaths"] = outcome.failed.map(\.relativePath).joined(separator: ", ")
            resultMessage = "Archive compression completed in \(String(format: "%.2f", duration))s with \(outcome.failed.count) post-commit cleanup warning(s)"
        } else {
            metadata["outcomeStatus"] = "completed"
            resultMessage = "Archive compression completed in \(String(format: "%.2f", duration))s"
        }
        return CommandResult(
            commandId: commandId,
            success: true,
            message: resultMessage,
            artifactsCreated: sortedArtifacts,
            backupPaths: backupDict,
            executionDuration: duration,
            metadata: metadata
        )
    }

    private func removeConfirmedArtifactsForUndo(
        confirmedArtifacts: [ConfirmedArtifactIdentity],
        protectedPaths: Set<String>,
        backups: [String: String]
    ) {
        let fm = FileManager.default
        for artifact in confirmedArtifacts {
            guard !Self.isProtectedCleanupPath(artifact.path, protectedPaths: protectedPaths) else {
                continue
            }
            guard backups[artifact.path] == nil else {
                continue
            }
            guard fm.fileExists(atPath: artifact.path),
                  Self.matchesRecordedInode(artifact, fileManager: fm) else {
                continue
            }
            try? fm.removeItem(atPath: artifact.path)
        }
    }

    private func restoreBackupsForUndo(
        backups: [String: String],
        protectedPaths: Set<String>
    ) -> [String: String] {
        let fm = FileManager.default
        var unRestoredBackups: [String: String] = [:]
        for (origPath, backupPath) in backups {
            guard !Self.isProtectedCleanupPath(origPath, protectedPaths: protectedPaths) else {
                continue
            }
            guard fm.fileExists(atPath: backupPath) else { continue }
            if fm.fileExists(atPath: origPath) {
                try? fm.removeItem(atPath: origPath)
            }
            do {
                try fm.moveItem(atPath: backupPath, toPath: origPath)
            } catch {
                unRestoredBackups[origPath] = backupPath
            }
        }
        return unRestoredBackups
    }

    private static func matchesRecordedInode(
        _ artifact: ConfirmedArtifactIdentity,
        fileManager: FileManager
    ) -> Bool {
        guard let expectedInode = artifact.inode else { return true }
        guard let attr = try? fileManager.attributesOfItem(atPath: artifact.path),
              let currentInode = (attr[.systemFileNumber] as? NSNumber)?.uint64Value else {
            return false
        }
        return currentInode == expectedInode
    }

    private static func isProtectedCleanupPath(_ path: String, protectedPaths: Set<String>) -> Bool {
        guard !protectedPaths.isEmpty else { return false }
        let standardized = (path as NSString).standardizingPath
        return protectedPaths.contains(path) || protectedPaths.contains(standardized)
    }

    private static func isTransientOrBackupName(_ value: String) -> Bool {
        let fileName = (value as NSString).lastPathComponent
        let ext = (fileName as NSString).pathExtension.lowercased()
        return ext.hasPrefix("bak_")
            || fileName.contains(".ttzip_create_txn_")
            || fileName.contains(".ttzip_create_bak_")
            || fileName.contains(".ttzip_staging_")
    }

    // MARK: - Internal Synchronization Helpers

    private struct UndoStateSnapshot {
        let executed: Bool
        let confirmedArtifacts: [ConfirmedArtifactIdentity]
        let protectedPaths: Set<String>
        let backups: [String: String]
    }

    private func saveExecutionState(
        artifacts: [String],
        confirmedArtifacts: [ConfirmedArtifactIdentity],
        protectedPaths: Set<String>,
        backupMap: [String: String]
    ) {
        lock.lock()
        defer { lock.unlock() }
        self.createdArtifacts = artifacts
        self.confirmedArtifacts = confirmedArtifacts
        self.protectedCleanupPaths = protectedPaths
        self.backupFileMap = backupMap
        self.isExecutedState = true
    }

    private func getUndoStateSnapshot() -> UndoStateSnapshot {
        lock.lock()
        defer { lock.unlock() }
        return UndoStateSnapshot(
            executed: self.isExecutedState,
            confirmedArtifacts: self.confirmedArtifacts,
            protectedPaths: self.protectedCleanupPaths,
            backups: self.backupFileMap
        )
    }

    private func resetExecutionStateOnUndoSuccess() {
        lock.lock()
        defer { lock.unlock() }
        self.isExecutedState = false
        self.createdArtifacts.removeAll()
        self.confirmedArtifacts.removeAll()
        self.protectedCleanupPaths.removeAll()
        self.backupFileMap.removeAll()
    }
}

// MARK: - Validation & Volume Pattern Helpers

extension CompressCommand {
    internal func isSplitVolumeMatch(fileName: String, baseName: String, baseStem: String) -> Bool {
        if fileName.hasPrefix(baseName + ".") { return true }
        if fileName.hasPrefix(baseStem + ".") {
            let ext = (fileName as NSString).pathExtension.lowercased()
            if ext.hasPrefix("z") && ext.dropFirst().allSatisfy({ $0.isNumber }) { return true }
            if ext.allSatisfy({ $0.isNumber }) && !ext.isEmpty { return true }
            if ext.hasPrefix("part") { return true }
        }
        return false
    }

    internal func scanDirectorySet(dirPath: String) -> Set<String> {
        let fm = FileManager.default
        guard fm.fileExists(atPath: dirPath) else { return [] }
        var result = Set<String>()
        if let enumerator = fm.enumerator(atPath: dirPath) {
            while let relativePath = enumerator.nextObject() as? String {
                let fullPath = (dirPath as NSString).appendingPathComponent(relativePath)
                result.insert(fullPath)
            }
        }
        return result
    }
}
