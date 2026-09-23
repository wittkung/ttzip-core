// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import Foundation

// MARK: - Command Pattern Default Facade Injection Extensions

extension CompressCommand {
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
        progress: (@Sendable (ArchiveProgress) -> Void)? = nil
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
            engineFacade: TTZipEngineFacade.shared
        )
    }
}

extension ExtractCommand {
    public convenience init(
        commandId: String = UUID().uuidString,
        description: String? = nil,
        archivePath: String,
        destinationDir: String,
        password: String? = nil,
        autoVaultUnlock: Bool = true,
        progress: (@Sendable (ArchiveProgress) -> Void)? = nil
    ) {
        self.init(
            commandId: commandId,
            description: description,
            archivePath: archivePath,
            destinationDir: destinationDir,
            password: password,
            autoVaultUnlock: autoVaultUnlock,
            progress: progress,
            engineFacade: TTZipEngineFacade.shared
        )
    }
}

extension RepairCommand {
    public convenience init(
        commandId: String = UUID().uuidString,
        description: String? = nil,
        damagedPath: String,
        outputPath: String
    ) {
        self.init(
            commandId: commandId,
            description: description,
            damagedPath: damagedPath,
            outputPath: outputPath,
            engineFacade: TTZipEngineFacade.shared
        )
    }
}

// MARK: - Engine Facade Main

/// Facade Pattern: Unified primary archive orchestration facade center (`TTZipEngineFacade`).
/// Provides streamlined high-level APIs for compression, extraction, password vault, integrity verification,
/// preview generation, security scanning, split-volume management, and archive self-healing repair.
public final class TTZipEngineFacade: TTZipEngineFacading, @unchecked Sendable {
    public static let shared = TTZipEngineFacade()
    
    public let historyManager: CommandHistoryManager
    internal let pipelineBuilderProvider: @Sendable () -> ArchivePipelineBuilder
    internal let reader: ArchiveReading
    internal let securityScanner: SecurityScanner
    internal let passwordVault: PasswordVaultManaging
    internal let integrityChecker: ArchiveIntegrityChecking
    internal let repairEngine: ArchiveRepairEngine
    internal let recoveryEngine: PasswordRecoveryEngine
    internal let splitEngine: NativeParallelEncryptedSplitEngine
    
    private convenience init() {
        self.init(
            historyManager: CommandHistoryManager.shared,
            pipelineBuilderProvider: { ArchivePipelineBuilder() },
            reader: ArchiveEngineFactory.makeReader(),
            securityScanner: SecurityScanner.shared,
            passwordVault: PasswordVaultManager.shared,
            integrityChecker: ArchiveEngineFactory.makeIntegrityChecker(),
            repairEngine: ArchiveRepairEngine(),
            recoveryEngine: PasswordRecoveryEngine(),
            splitEngine: NativeParallelEncryptedSplitEngine()
        )
    }
    
    internal init(
        historyManager: CommandHistoryManager = CommandHistoryManager.shared,
        pipelineBuilderProvider: @Sendable @escaping () -> ArchivePipelineBuilder = { ArchivePipelineBuilder() },
        reader: ArchiveReading = ArchiveEngineFactory.makeReader(),
        securityScanner: SecurityScanner = SecurityScanner.shared,
        passwordVault: PasswordVaultManaging = PasswordVaultManager.shared,
        integrityChecker: ArchiveIntegrityChecking = ArchiveEngineFactory.makeIntegrityChecker(),
        repairEngine: ArchiveRepairEngine = ArchiveRepairEngine(),
        recoveryEngine: PasswordRecoveryEngine = PasswordRecoveryEngine(),
        splitEngine: NativeParallelEncryptedSplitEngine = NativeParallelEncryptedSplitEngine()
    ) {
        self.historyManager = historyManager
        self.pipelineBuilderProvider = pipelineBuilderProvider
        self.reader = reader
        self.securityScanner = securityScanner
        self.passwordVault = passwordVault
        self.integrityChecker = integrityChecker
        self.repairEngine = repairEngine
        self.recoveryEngine = recoveryEngine
        self.splitEngine = splitEngine
    }
    
    // MARK: - Command Pattern Convenience Wrappers
    
    public func compressWithCommand(
        inputs: [String],
        outputPath: String,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        password: String? = nil,
        splitSize: Int64? = nil,
        filterOptions: ArchiveFilterOptions = ArchiveFilterOptions(),
        advancedOptions: ArchiveAdvancedOptions? = nil,
        progress: (@Sendable (ArchiveProgress) -> Void)? = nil,
        engineFacade: TTZipEngineFacading? = nil
    ) async throws -> CommandResult {
        let command = CompressCommand(
            inputs: inputs,
            outputPath: outputPath,
            format: format,
            level: level,
            password: password,
            splitSize: splitSize,
            filterOptions: filterOptions,
            advancedOptions: advancedOptions,
            progress: progress,
            engineFacade: engineFacade ?? self
        )
        return try await executeCommand(command)
    }
    
    public func extractWithCommand(
        archivePath: String,
        destinationDir: String,
        password: String? = nil,
        autoVaultUnlock: Bool = true,
        progress: (@Sendable (ArchiveProgress) -> Void)? = nil,
        engineFacade: TTZipEngineFacading? = nil
    ) async throws -> CommandResult {
        let command = ExtractCommand(
            archivePath: archivePath,
            destinationDir: destinationDir,
            password: password,
            autoVaultUnlock: autoVaultUnlock,
            progress: progress,
            engineFacade: engineFacade ?? self
        )
        return try await executeCommand(command)
    }
    
    public func repairWithCommand(
        damagedPath: String,
        outputPath: String
    ) async throws -> CommandResult {
        let command = RepairCommand(
            damagedPath: damagedPath,
            outputPath: outputPath,
            engineFacade: self
        )
        return try await executeCommand(command)
    }
}

// MARK: - Compress Facade

//
//


extension TTZipEngineFacade {
    public func quickCompress(
        inputs: [String],
        outputPath: String,
        format: ArchiveCompressionFormat = .zip,
        level: ArchiveCompressionLevel = .normal,
        password: String? = nil,
        splitSize: Int64? = nil,
        filterOptions: ArchiveFilterOptions = .defaultClean,
        advancedOptions: ArchiveAdvancedOptions? = nil,
        progress: (@Sendable (ArchiveProgress) -> Void)? = nil,
        token: CancellationToken? = nil
    ) async throws -> ArchiveOperationResult {
        guard !inputs.isEmpty && !outputPath.isEmpty else {
            throw ArchiveError.readFailed(code: -10)
        }
        
        let combinedProgress: @Sendable (ArchiveProgress) -> Void = { p in
            progress?(p)
        }
        
        if let splitBytes = splitSize, splitBytes > 0, (format == .sevenZip || format == .zip) {
            let splitFormat: NativeParallelEncryptedSplitEngine.SplitFormat = (format == .sevenZip) ? .sevenZip : .zip
            let outputDir = (outputPath as NSString).deletingLastPathComponent
            let baseName = (outputPath as NSString).lastPathComponent
            let targetDir = outputDir.isEmpty ? "." : outputDir
            
            let startTime = Date()
            let generatedVolumes = try await splitEngine.createStandardEncryptedSplitVolume(
                format: splitFormat,
                sourcePaths: inputs,
                outputDir: targetDir,
                baseName: baseName,
                splitVolumeSizeBytes: splitBytes,
                password: password ?? ""
            )
            
            let elapsed = max(0.001, Date().timeIntervalSince(startTime))
            var totalOrigBytes: Int64 = 0
            let fm = FileManager.default
            for p in inputs {
                if let attr = try? fm.attributesOfItem(atPath: p) {
                    if (attr[.type] as? FileAttributeType) == .typeDirectory {
                        let component = ArchiveComponentTreeBuilder.buildTree(fromDiskPath: p)
                        totalOrigBytes += component.sizeBytes
                    } else {
                        totalOrigBytes += (attr[.size] as? Int64) ?? 0
                    }
                }
            }
            
            var compressedSize: Int64 = 0
            for vol in generatedVolumes {
                if let attr = try? fm.attributesOfItem(atPath: vol) {
                    compressedSize += (attr[.size] as? Int64) ?? 0
                }
            }
            let rate = (Double(totalOrigBytes) / 1024.0 / 1024.0) / elapsed
            
            return ArchiveOperationResult(
                outputPath: outputPath,
                originalBytes: totalOrigBytes,
                compressedBytes: compressedSize,
                durationSeconds: elapsed,
                throughputMBs: rate
            )
        }
        
        var builder = pipelineBuilderProvider()
            .withInputPaths(inputs)
            .withOutputPath(outputPath)
            .withFormat(format)
            .withLevel(level)
            .withFilterOptions(filterOptions)
            .withPassword(password)
            .withSplitVolumeSize(splitSize)
            .withToken(token)
        
        if let adv = advancedOptions {
            builder = builder.withAdvancedOptions(adv)
        }
        
        builder = builder.withProgressHandler(combinedProgress)
        
        return try await builder.executeCreate()
    }
}

// MARK: - Extract Facade

// MARK: - Unified Extraction Facade

extension TTZipEngineFacade {
    public func quickExtract(
        archivePath: String,
        destinationDir: String,
        password: String? = nil,
        autoVaultUnlock: Bool = true,
        progress: (@Sendable (ArchiveProgress) -> Void)? = nil,
        token: CancellationToken? = nil
    ) async throws -> ExtractResult {
        guard !archivePath.isEmpty, !destinationDir.isEmpty, FileManager.default.fileExists(atPath: archivePath) else {
            throw ArchiveError.fileNotFound
        }
        
        let combinedProgress: @Sendable (ArchiveProgress) -> Void = { p in
            progress?(p)
        }
        
        if let explicitPwd = password, !explicitPwd.isEmpty {
            let elapsed = try await executePipelineExtract(
                archivePath: archivePath,
                destinationDir: destinationDir,
                password: explicitPwd,
                progress: combinedProgress,
                token: token
            )
            ArchivePasswordStore.shared.setPassword(explicitPwd, for: archivePath)
            return ExtractResult(
                archivePath: archivePath,
                destinationDir: destinationDir,
                durationSeconds: elapsed,
                unlockedPassword: explicitPwd,
                isVaultUnlocked: false
            )
        } else {
            do {
                let elapsed = try await executePipelineExtract(
                    archivePath: archivePath,
                    destinationDir: destinationDir,
                    password: nil,
                    progress: combinedProgress,
                    token: token
                )
                return ExtractResult(
                    archivePath: archivePath,
                    destinationDir: destinationDir,
                    durationSeconds: elapsed,
                    unlockedPassword: nil,
                    isVaultUnlocked: false
                )
            } catch {
                if token?.isCancelled() == true || Task.isCancelled {
                    throw ArchiveError.cancelled
                }
                let archiveErr = ArchiveError.from(error: error)
                guard archiveErr.isPasswordRelated else {
                    throw archiveErr
                }
                // Password is required, proceed to auto-unlock via vault candidates
            }
        }
        
        if autoVaultUnlock {
            let vaultEntries = passwordVault.getEntries()
            var matchedEntry: PasswordVaultEntry? = nil

            for entry in vaultEntries {
                do {
                    // Non-destructive in-memory inspection probe before touching disk
                    _ = try await reader.inspect(archivePath: archivePath, password: entry.password)
                    matchedEntry = entry
                    break
                } catch {
                    // Password probe failed, try next candidate
                }
            }

            if let entry = matchedEntry {
                let elapsed = try await executePipelineExtract(
                    archivePath: archivePath,
                    destinationDir: destinationDir,
                    password: entry.password,
                    progress: combinedProgress,
                    token: token
                )
                passwordVault.recordUsage(id: entry.id)
                ArchivePasswordStore.shared.setPassword(entry.password, for: archivePath)
                return ExtractResult(
                    archivePath: archivePath,
                    destinationDir: destinationDir,
                    durationSeconds: elapsed,
                    unlockedPassword: entry.password,
                    isVaultUnlocked: true
                )
            }
        }
        
        throw ArchiveError.passwordRequired
    }
    
    public func extractSingleEntry(
        archivePath: String,
        entryPath: String,
        destinationDir: String,
        password: String? = nil
    ) async throws {
        guard !archivePath.isEmpty, !destinationDir.isEmpty, FileManager.default.fileExists(atPath: archivePath) else {
            throw ArchiveError.fileNotFound
        }
        
        let explicitPwd = password ?? ArchivePasswordStore.shared.getPassword(for: archivePath)
        let extractor = ArchiveEngineFactory.makeExtractor()
        try await extractor.extractSingleFile(
            archivePath: archivePath,
            entryPath: entryPath,
            destinationDir: destinationDir,
            password: explicitPwd
        )
    }
    
    internal func executePipelineExtract(
        archivePath: String,
        destinationDir: String,
        password: String?,
        progress: (@Sendable (ArchiveProgress) -> Void)?,
        token: CancellationToken? = nil
    ) async throws -> Double {
        var builder = pipelineBuilderProvider()
            .withArchivePath(archivePath)
            .withDestinationDir(destinationDir)
            .withPassword(password)
            .withToken(token)
        
        if let progress = progress {
            builder = builder.withProgressHandler(progress)
        }
        
        let res = try await builder.executeExtract()
        return res.durationSeconds
    }
}

// MARK: - Inspect Facade

//
//


// MARK: - Unified Archive Inspection & Structure Probing Facade

extension TTZipEngineFacade {
    public func inspectArchive(
        archivePath: String,
        password: String? = nil,
        autoVaultUnlock: Bool = true
    ) async throws -> ArchiveInspectionResult {
        guard !archivePath.isEmpty, FileManager.default.fileExists(atPath: archivePath) else {
            throw ArchiveError.fileNotFound
        }
        
        let explicitPwd = password ?? ArchivePasswordStore.shared.getPassword(for: archivePath)
        
        do {
            let entries = try await reader.inspect(archivePath: archivePath, password: explicitPwd)
            let treeNode = ArchiveComponentTreeBuilder.buildTree(from: entries)
            let securityReport = securityScanner.scanEntriesForReport(entries)
            if let p = explicitPwd, !p.isEmpty {
                ArchivePasswordStore.shared.setPassword(p, for: archivePath)
            }
            return ArchiveInspectionResult(
                archivePath: archivePath,
                entries: entries,
                treeNode: treeNode,
                securityReport: securityReport,
                unlockedPassword: explicitPwd
            )
        } catch ArchiveError.passwordRequired {
            // Fall through to password vault auto-unlock
        } catch {
            if explicitPwd != nil && explicitPwd?.isEmpty == false {
                throw error
            }
        }
        
        if autoVaultUnlock {
            let vaultEntries = passwordVault.getEntries()
            for entry in vaultEntries {
                if let entries = try? await reader.inspect(archivePath: archivePath, password: entry.password) {
                    passwordVault.recordUsage(id: entry.id)
                    ArchivePasswordStore.shared.setPassword(entry.password, for: archivePath)
                    let treeNode = ArchiveComponentTreeBuilder.buildTree(from: entries)
                    let securityReport = securityScanner.scanEntriesForReport(entries)
                    return ArchiveInspectionResult(
                        archivePath: archivePath,
                        entries: entries,
                        treeNode: treeNode,
                        securityReport: securityReport,
                        unlockedPassword: entry.password
                    )
                }
            }
        }
        
        throw ArchiveError.passwordRequired
    }
    
    // MARK: - Auxiliary High-Level Facade (Integrity, Repair & Password Recovery)
    
    public func verifyIntegrity(archivePath: String) async throws -> HashVerificationResult {
        let crc = integrityChecker.computeCRC32(filePath: archivePath)
        let sha = try await integrityChecker.computeSHA256(filePath: archivePath)
        return HashVerificationResult(filePath: archivePath, crc32: crc, sha256: sha)
    }
    
    public func repairArchive(damagedPath: String, outputPath: String) async throws -> Int {
        guard !damagedPath.isEmpty, !outputPath.isEmpty, FileManager.default.fileExists(atPath: damagedPath) else {
            throw ArchiveError.fileNotFound
        }
        return try await repairEngine.repairArchive(damagedArchivePath: damagedPath, repairedOutputPath: outputPath)
    }
    
    public func recoverPassword(
        archivePath: String,
        dictionary: [String]
    ) async throws -> PasswordRecoveryResult {
        return try await recoveryEngine.recoverPassword(archivePath: archivePath, dictionary: dictionary)
    }
}
