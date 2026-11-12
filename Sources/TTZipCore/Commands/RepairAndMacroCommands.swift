// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Concrete command encapsulating archive scanning and recovery with transactional rollback.
public final class RepairCommand: ArchiveCommandProtocol, @unchecked Sendable {
    public let commandId: String
    public let description: String
    public var isUndoable: Bool { true }
    
    public let damagedPath: String
    public let outputPath: String
    private let engineFacade: TTZipEngineFacading
    private let lock = NSLock()
    
    private var createdArtifacts: [String] = []
    private var backupFilePath: String? = nil
    private var isExecutedState: Bool = false
    
    public init(
        commandId: String = UUID().uuidString,
        description: String? = nil,
        damagedPath: String,
        outputPath: String,
        engineFacade: TTZipEngineFacading
    ) {
        self.commandId = commandId
        self.damagedPath = damagedPath
        self.outputPath = outputPath
        self.engineFacade = engineFacade
        
        let file = (damagedPath as NSString).lastPathComponent
        self.description = description ?? "Repair damaged archive [\(file)]"
    }
    
    deinit {
        purgeBackupResources()
    }
    
    public func execute() async throws -> CommandResult {
        let startTime = CFAbsoluteTimeGetCurrent()
        let fm = FileManager.default
        let backupMade = createBackupIfNeeded(fileManager: fm)
        
        let recoveredCount: Int
        do {
            recoveredCount = try await engineFacade.repairArchive(damagedPath: damagedPath, outputPath: outputPath)
        } catch {
            removeFileIfExists(backupMade, fileManager: fm)
            throw error
        }
        
        let duration = CFAbsoluteTimeGetCurrent() - startTime
        let artifacts = fm.fileExists(atPath: outputPath) ? [outputPath] : []
        saveExecutionState(artifacts: artifacts, backupPath: backupMade)
        
        var backupDict: [String: String] = [:]
        if let b = backupMade {
            backupDict[outputPath] = b
        }
        
        return CommandResult(
            commandId: commandId,
            success: true,
            message: "Archive repaired successfully, recovered \(recoveredCount) data blocks",
            artifactsCreated: artifacts,
            backupPaths: backupDict,
            executionDuration: duration,
            metadata: ["recoveredCount": "\(recoveredCount)"]
        )
    }
    
    private func createBackupIfNeeded(fileManager: FileManager) -> String? {
        guard fileManager.fileExists(atPath: outputPath) else { return nil }
        let candidate = "\(outputPath).bak_\(UUID().uuidString)"
        try? fileManager.copyItem(atPath: outputPath, toPath: candidate)
        return candidate
    }
    
    private func removeFileIfExists(_ path: String?, fileManager: FileManager) {
        if let p = path, fileManager.fileExists(atPath: p) {
            try? fileManager.removeItem(atPath: p)
        }
    }
    
    public func undo() async throws {
        let (executed, artifacts, backup) = getUndoStateSnapshot()
        guard executed else {
            throw CommandError.invalidState(reason: "Repair command has not been executed; cannot undo.")
        }
        
        let fm = FileManager.default
        for path in artifacts where fm.fileExists(atPath: path) {
            try? fm.removeItem(atPath: path)
        }
        
        if let backup = backup, fm.fileExists(atPath: backup) {
            removeFileIfExists(outputPath, fileManager: fm)
            do {
                try fm.moveItem(atPath: backup, toPath: outputPath)
            } catch {
                throw CommandError.undoFailed(reason: "Failed to restore original backup during repair undo: \(error.localizedDescription)")
            }
        }
        
        resetExecutionStateOnUndoSuccess()
    }
    
    public func purgeBackupResources() {
        lock.lock()
        let b = self.backupFilePath
        self.backupFilePath = nil
        lock.unlock()
        removeFileIfExists(b, fileManager: .default)
    }
    
    // MARK: - Internal Synchronization Helpers
    
    private func saveExecutionState(artifacts: [String], backupPath: String?) {
        lock.lock()
        defer { lock.unlock() }
        self.createdArtifacts = artifacts
        self.backupFilePath = backupPath
        self.isExecutedState = true
    }
    
    private func getUndoStateSnapshot() -> (executed: Bool, artifacts: [String], backup: String?) {
        lock.lock()
        defer { lock.unlock() }
        return (self.isExecutedState, self.createdArtifacts, self.backupFilePath)
    }
    
    private func resetExecutionStateOnUndoSuccess() {
        lock.lock()
        defer { lock.unlock() }
        self.isExecutedState = false
        self.createdArtifacts.removeAll()
        self.backupFilePath = nil
    }
}

// MARK: - Macro Command

/// Composite / macro command orchestrating a sequence of archive operations with automated reverse rollback.
public final class MacroArchiveCommand: ArchiveCommandProtocol, @unchecked Sendable {
    public let commandId: String
    public let description: String
    public var isUndoable: Bool {
        commands.allSatisfy { $0.isUndoable }
    }
    
    public let commands: [ArchiveCommandProtocol]
    private let lock = NSLock()
    
    private var executedSubCommands: [ArchiveCommandProtocol] = []
    private var isExecutedState: Bool = false
    
    public init(
        commandId: String = UUID().uuidString,
        description: String? = nil,
        commands: [ArchiveCommandProtocol]
    ) {
        self.commandId = commandId
        self.commands = commands
        self.description = description ?? "Macro command (\(commands.count) atomic steps)"
    }
    
    deinit {
        purgeBackupResources()
    }
    
    public func execute() async throws -> CommandResult {
        let startTime = CFAbsoluteTimeGetCurrent()
        clearExecutedList()
        
        var combinedArtifacts: [String] = []
        var combinedBackups: [String: String] = [:]
        
        for (index, command) in commands.enumerated() {
            do {
                let subResult = try await command.execute()
                appendExecutedCommand(command)
                combinedArtifacts.append(contentsOf: subResult.artifactsCreated)
                for (k, v) in subResult.backupPaths {
                    combinedBackups[k] = v
                }
            } catch {
                let rollbackErrors = await performRollback()
                throw CommandError.macroExecutionFailed(
                    failedIndex: index,
                    underlyingError: error.localizedDescription,
                    rollbackErrors: rollbackErrors
                )
            }
        }
        
        let duration = CFAbsoluteTimeGetCurrent() - startTime
        markAsExecuted()
        
        return CommandResult(
            commandId: commandId,
            success: true,
            message: "Macro command executed successfully (\(commands.count) sub-steps)",
            artifactsCreated: combinedArtifacts,
            backupPaths: combinedBackups,
            executionDuration: duration
        )
    }
    
    public func undo() async throws {
        let (executed, toUndo) = getUndoStateAndReset()
        guard executed || !toUndo.isEmpty else {
            throw CommandError.invalidState(reason: "Macro command has not been executed; cannot undo.")
        }
        
        var undoErrors: [String] = []
        var remainingCommands = toUndo
        
        for command in toUndo.reversed() {
            do {
                if command.isUndoable {
                    try await command.undo()
                }
                _ = remainingCommands.popLast()
            } catch {
                undoErrors.append("Undo failed for [\(command.description)]: \(error.localizedDescription)")
            }
        }
        
        if !undoErrors.isEmpty {
            restoreUnfinishedState(remainingCommands)
            throw CommandError.undoFailed(reason: undoErrors.joined(separator: "; "))
        }
    }
    
    public func purgeBackupResources() {
        for command in commands {
            command.purgeBackupResources()
        }
    }
    
    // MARK: - Rollback Helpers
    
    private func performRollback() async -> [String] {
        let toRollback = getExecutedListAndReset()
        var rollbackErrors: [String] = []
        for command in toRollback.reversed() where command.isUndoable {
            do {
                try await command.undo()
            } catch {
                rollbackErrors.append("Rollback failed for [\(command.description)]: \(error.localizedDescription)")
            }
        }
        return rollbackErrors
    }
    
    // MARK: - Internal Synchronization Helpers
    
    private func clearExecutedList() {
        lock.lock()
        defer { lock.unlock() }
        executedSubCommands.removeAll()
        isExecutedState = false
    }
    
    private func appendExecutedCommand(_ command: ArchiveCommandProtocol) {
        lock.lock()
        defer { lock.unlock() }
        executedSubCommands.append(command)
    }
    
    private func markAsExecuted() {
        lock.lock()
        defer { lock.unlock() }
        isExecutedState = true
    }
    
    private func restoreUnfinishedState(_ unfinished: [ArchiveCommandProtocol]) {
        lock.lock()
        defer { lock.unlock() }
        executedSubCommands = unfinished
        isExecutedState = !unfinished.isEmpty
    }
    
    private func getExecutedListAndReset() -> [ArchiveCommandProtocol] {
        lock.lock()
        defer { lock.unlock() }
        let list = executedSubCommands
        executedSubCommands.removeAll()
        isExecutedState = false
        return list
    }
    
    private func getUndoStateAndReset() -> (executed: Bool, list: [ArchiveCommandProtocol]) {
        lock.lock()
        defer { lock.unlock() }
        let wasExecuted = isExecutedState
        let list = executedSubCommands
        isExecutedState = false
        executedSubCommands.removeAll()
        return (wasExecuted, list)
    }
}

/// Standalone Write-Ahead Journal transaction tracker for differential archive extractions.
public struct DifferentialExtractTransaction: Sendable {
    public let destinationPath: String
    private(set) public var createdPaths: [String] = []
    
    public init(destinationPath: String) {
        self.destinationPath = destinationPath
    }
    
    public mutating func recordCreated(path: String, isDirectory: Bool) {
        createdPaths.append(path)
    }
    
    public func executeRollback(fileManager: FileManager = .default) {
        let sorted = createdPaths.sorted {
            $0.components(separatedBy: "/").count > $1.components(separatedBy: "/").count
        }
        for path in sorted {
            try? fileManager.removeItem(atPath: path)
        }
    }
}
