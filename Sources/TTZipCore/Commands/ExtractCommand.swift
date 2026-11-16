// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import TTZipCoreBindings
import TTZipCoreTypes
import Foundation

/// Concrete command encapsulating archive extraction with undoable directory rollback.
public final class ExtractCommand: ArchiveCommandProtocol, @unchecked Sendable {
    public let commandId: String
    public let description: String
    public var isUndoable: Bool { true }
    
    public let archivePath: String
    public let destinationDir: String
    public let password: String?
    public let autoVaultUnlock: Bool
    public let handling: CollisionHandling
    public let progress: (@Sendable (ArchiveProgress) -> Void)?
    
    private let engineFacade: TTZipEngineFacading
    private let lock = NSLock()
    
    private var newlyCreatedFileTree: [String] = []
    private var preExistingDirExisted: Bool = false
    private var backupDirPath: String? = nil
    private var isExecutedState: Bool = false
    
    public init(
        commandId: String = UUID().uuidString,
        description: String? = nil,
        archivePath: String,
        destinationDir: String,
        password: String? = nil,
        autoVaultUnlock: Bool = true,
        handling: CollisionHandling = .policy(.keepBoth),
        progress: (@Sendable (ArchiveProgress) -> Void)? = nil,
        engineFacade: TTZipEngineFacading
    ) {
        self.commandId = commandId
        self.archivePath = archivePath
        self.destinationDir = destinationDir
        self.password = password
        self.autoVaultUnlock = autoVaultUnlock
        self.handling = handling
        self.progress = progress
        self.engineFacade = engineFacade
        
        let archiveName = (archivePath as NSString).lastPathComponent
        let destName = (destinationDir as NSString).lastPathComponent
        self.description = description ?? "Extract [\(archiveName)] to [\(destName)]"
    }
    
    deinit {
        purgeBackupResources()
    }
    
    public func execute() async throws -> CommandResult {
        let startTime = CFAbsoluteTimeGetCurrent()
        let fm = FileManager.default
        
        let dirExistedBefore = fm.fileExists(atPath: destinationDir)
        let preExistingPaths = dirExistedBefore ? scanDirectorySet(dirPath: destinationDir) : Set<String>()
        var backupDirectory = createPreExtractionBackup(
            dirExistedBefore: dirExistedBefore,
            preExistingPaths: preExistingPaths,
            fileManager: fm
        )
        
        let extractResult: ExtractResult
        do {
            extractResult = try await engineFacade.quickExtract(
                archivePath: archivePath,
                destinationDir: destinationDir,
                password: password,
                autoVaultUnlock: autoVaultUnlock,
                handling: handling,
                progress: progress
            )
        } catch {
            rollbackFailedExtraction(
                dirExistedBefore: dirExistedBefore,
                preExistingPaths: preExistingPaths,
                backupDirectory: backupDirectory,
                fileManager: fm
            )
            throw error
        }
        
        let postExistingPaths = scanDirectorySet(dirPath: destinationDir)
        let sortedCreated = sortPathsDeepestFirst(postExistingPaths.subtracting(preExistingPaths))
        
        removeNoIndexMarkerIfPresent(fileManager: fm)
        backupDirectory = pruneUnchangedBackupFiles(
            backupDirectory: backupDirectory,
            preExistingPaths: preExistingPaths,
            fileManager: fm
        )
        saveExecutionState(created: sortedCreated, preExisted: dirExistedBefore, backupDir: backupDirectory)
        
        let duration = CFAbsoluteTimeGetCurrent() - startTime
        return CommandResult(
            commandId: commandId,
            success: true,
            message: "Extraction completed to \(destinationDir)",
            artifactsCreated: sortedCreated,
            backupPaths: [:],
            executionDuration: duration,
            metadata: ["unlockedPassword": extractResult.unlockedPassword ?? ""]
        )
    }
    
    private func createPreExtractionBackup(
        dirExistedBefore: Bool,
        preExistingPaths: Set<String>,
        fileManager: FileManager
    ) -> String? {
        guard dirExistedBefore && !preExistingPaths.isEmpty else { return nil }
        let tmpBackup = NSTemporaryDirectory() + "ttzip_extract_backup_" + UUID().uuidString
        guard (try? fileManager.createDirectory(atPath: tmpBackup, withIntermediateDirectories: true)) != nil else {
            return nil
        }
        for path in preExistingPaths where isRegularItem(atPath: path, fileManager: fileManager) {
            let rel = relativeDestinationSubpath(path)
            let targetBackup = (tmpBackup as NSString).appendingPathComponent(rel)
            let parentDir = (targetBackup as NSString).deletingLastPathComponent
            try? fileManager.createDirectory(atPath: parentDir, withIntermediateDirectories: true)
            try? fileManager.copyItem(atPath: path, toPath: targetBackup)
        }
        return tmpBackup
    }
    
    private func rollbackFailedExtraction(
        dirExistedBefore: Bool,
        preExistingPaths: Set<String>,
        backupDirectory: String?,
        fileManager: FileManager
    ) {
        if dirExistedBefore {
            let dirtyPaths = scanDirectorySet(dirPath: destinationDir).subtracting(preExistingPaths)
            for path in sortPathsDeepestFirst(dirtyPaths) {
                try? fileManager.removeItem(atPath: path)
            }
            if let backupDir = backupDirectory, fileManager.fileExists(atPath: backupDir) {
                try? fileManager.removeItem(atPath: backupDir)
            }
        } else if fileManager.fileExists(atPath: destinationDir) {
            try? fileManager.removeItem(atPath: destinationDir)
        }
    }
    
    private func removeNoIndexMarkerIfPresent(fileManager: FileManager) {
        let noIndexFilePath = (destinationDir as NSString).appendingPathComponent(".noindex")
        if fileManager.fileExists(atPath: noIndexFilePath) {
            try? fileManager.removeItem(atPath: noIndexFilePath)
        }
    }
    
    private func pruneUnchangedBackupFiles(
        backupDirectory: String?,
        preExistingPaths: Set<String>,
        fileManager: FileManager
    ) -> String? {
        guard let backupDir = backupDirectory, fileManager.fileExists(atPath: backupDir) else {
            return backupDirectory
        }
        var anyOverwritten = false
        for path in preExistingPaths {
            let rel = relativeDestinationSubpath(path)
            let targetBackup = (backupDir as NSString).appendingPathComponent(rel)
            if isBackupOverwritten(originalBackupPath: targetBackup, currentPath: path, fileManager: fileManager) {
                anyOverwritten = true
            }
        }
        if !anyOverwritten {
            try? fileManager.removeItem(atPath: backupDir)
            return nil
        }
        return backupDir
    }
    
    private func isBackupOverwritten(
        originalBackupPath: String,
        currentPath: String,
        fileManager: FileManager
    ) -> Bool {
        guard fileManager.fileExists(atPath: originalBackupPath),
              let origAttrs = try? fileManager.attributesOfItem(atPath: originalBackupPath),
              let currAttrs = try? fileManager.attributesOfItem(atPath: currentPath) else {
            return false
        }
        let dateChanged = (origAttrs[.modificationDate] as? Date) != (currAttrs[.modificationDate] as? Date)
        let sizeChanged = (origAttrs[.size] as? Int64) != (currAttrs[.size] as? Int64)
        if dateChanged || sizeChanged {
            return true
        }
        try? fileManager.removeItem(atPath: originalBackupPath)
        return false
    }
    
    public func undo() async throws {
        let (executed, createdTree, preExisted, backupDir) = getUndoStateSnapshot()
        guard executed else {
            throw CommandError.invalidState(reason: "Extraction command has not been executed; cannot undo.")
        }
        
        let fm = FileManager.default
        for path in createdTree where fm.fileExists(atPath: path) {
            try? fm.removeItem(atPath: path)
        }
        
        let backupRestoreFailed = restoreBackupFilesOnUndo(backupDir: backupDir, fileManager: fm)
        removeEmptyNewDestinationIfNeeded(preExisted: preExisted, fileManager: fm)
        
        if backupRestoreFailed {
            throw CommandError.undoFailed(reason: "Failed to restore backup directory during extraction undo.")
        }
        resetExecutionStateOnUndoSuccess()
    }
    
    private func restoreBackupFilesOnUndo(backupDir: String?, fileManager: FileManager) -> Bool {
        guard let backupDir = backupDir, fileManager.fileExists(atPath: backupDir) else {
            return false
        }
        var restoreFailed = false
        let backupPaths = scanDirectorySet(dirPath: backupDir)
        for bPath in backupPaths where isRegularItem(atPath: bPath, fileManager: fileManager) {
            let relPath = String(bPath.dropFirst(backupDir.count + 1))
            let origPath = (destinationDir as NSString).appendingPathComponent(relPath)
            if fileManager.fileExists(atPath: origPath) {
                try? fileManager.removeItem(atPath: origPath)
            }
            let parentDir = (origPath as NSString).deletingLastPathComponent
            try? fileManager.createDirectory(atPath: parentDir, withIntermediateDirectories: true)
            if (try? fileManager.copyItem(atPath: bPath, toPath: origPath)) == nil {
                restoreFailed = true
            }
        }
        if !restoreFailed {
            try? fileManager.removeItem(atPath: backupDir)
        }
        return restoreFailed
    }
    
    private func removeEmptyNewDestinationIfNeeded(preExisted: Bool, fileManager: FileManager) {
        guard !preExisted, fileManager.fileExists(atPath: destinationDir) else { return }
        if let contents = try? fileManager.contentsOfDirectory(atPath: destinationDir), contents.isEmpty {
            try? fileManager.removeItem(atPath: destinationDir)
        }
    }
    
    private func isRegularItem(atPath path: String, fileManager: FileManager) -> Bool {
        var isDir: ObjCBool = false
        return fileManager.fileExists(atPath: path, isDirectory: &isDir) && !isDir.boolValue
    }
    
    private func relativeDestinationSubpath(_ fullPath: String) -> String {
        String(fullPath.dropFirst(destinationDir.count).trimmingCharacters(in: CharacterSet(charactersIn: "/")))
    }
    
    private func sortPathsDeepestFirst(_ paths: Set<String>) -> [String] {
        paths.sorted {
            $0.components(separatedBy: "/").count > $1.components(separatedBy: "/").count
        }
    }
    
    public func purgeBackupResources() {
        lock.lock()
        let bDir = self.backupDirPath
        self.backupDirPath = nil
        lock.unlock()
        
        if let bDir = bDir, FileManager.default.fileExists(atPath: bDir) {
            try? FileManager.default.removeItem(atPath: bDir)
        }
    }
    
    // MARK: - Internal Synchronization Helpers
    
    private func saveExecutionState(created: [String], preExisted: Bool, backupDir: String?) {
        lock.lock()
        defer { lock.unlock() }
        self.newlyCreatedFileTree = created
        self.preExistingDirExisted = preExisted
        self.backupDirPath = backupDir
        self.isExecutedState = true
    }
    
    private func getUndoStateSnapshot() -> (executed: Bool, createdTree: [String], preExisted: Bool, backupDir: String?) {
        lock.lock()
        defer { lock.unlock() }
        return (self.isExecutedState, self.newlyCreatedFileTree, self.preExistingDirExisted, self.backupDirPath)
    }
    
    private func resetExecutionStateOnUndoSuccess() {
        lock.lock()
        defer { lock.unlock() }
        self.isExecutedState = false
        self.newlyCreatedFileTree.removeAll()
        self.backupDirPath = nil
    }
    
    private func scanDirectorySet(dirPath: String) -> Set<String> {
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
