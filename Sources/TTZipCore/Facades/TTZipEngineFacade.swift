// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

// MARK: - Command Pattern Default Facade Injection Extensions

extension ExtractCommand {
    public convenience init(
        commandId: String = UUID().uuidString,
        description: String? = nil,
        archivePath: String,
        destinationDir: String,
        password: String? = nil,
        autoVaultUnlock: Bool = true,
        handling: CollisionHandling = .policy(.keepBoth),
        progress: (@Sendable (ArchiveProgress) -> Void)? = nil
    ) {
        self.init(
            commandId: commandId,
            description: description,
            archivePath: archivePath,
            destinationDir: destinationDir,
            password: password,
            autoVaultUnlock: autoVaultUnlock,
            handling: handling,
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
    internal let snapshotBridge: VfsSnapshotBridge
    
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
        splitEngine: NativeParallelEncryptedSplitEngine = NativeParallelEncryptedSplitEngine(),
        snapshotBridge: VfsSnapshotBridge = .live
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
        self.snapshotBridge = snapshotBridge
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
            archiveWriter: ArchiveWriter()
        )
        return try await executeCommand(command)
    }
    
    public func extractWithCommand(
        archivePath: String,
        destinationDir: String,
        password: String? = nil,
        autoVaultUnlock: Bool = true,
        handling: CollisionHandling = .policy(.keepBoth),
        progress: (@Sendable (ArchiveProgress) -> Void)? = nil,
        engineFacade: TTZipEngineFacading? = nil
    ) async throws -> CommandResult {
        let command = ExtractCommand(
            archivePath: archivePath,
            destinationDir: destinationDir,
            password: password,
            autoVaultUnlock: autoVaultUnlock,
            handling: handling,
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
        
        let result = try await builder.executeCreate()
        
        if let splitBytes = splitSize, splitBytes > 0 {
            return resolveSplitOperationResult(outputPath: outputPath, baseResult: result)
        }
        
        return result
    }

    private func resolveSplitOperationResult(
        outputPath: String,
        baseResult: ArchiveOperationResult
    ) -> ArchiveOperationResult {
        let fm = FileManager.default
        let stem = (outputPath as NSString).deletingPathExtension
        let candidates = ["\(outputPath).001", "\(stem).z01", outputPath]
        guard let seed = candidates.first(where: { fm.fileExists(atPath: $0) }) else {
            return baseResult
        }
        let volumes = (try? detectSplitVolumeChain(seedPath: seed)) ?? [seed]
        var compressedSize: Int64 = 0
        for vol in volumes {
            let attr = try? fm.attributesOfItem(atPath: vol)
            compressedSize += (attr?[.size] as? Int64) ?? 0
        }
        let resolvedPath = volumes.first ?? seed
        let resolvedSize = compressedSize > 0 ? compressedSize : baseResult.compressedBytes
        return ArchiveOperationResult(
            outputPath: resolvedPath,
            originalBytes: baseResult.originalBytes,
            compressedBytes: resolvedSize,
            durationSeconds: baseResult.durationSeconds,
            throughputMBs: baseResult.throughputMBs
        )
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
        handling: CollisionHandling = .policy(.keepBoth),
        progress: (@Sendable (ArchiveProgress) -> Void)? = nil,
        token: CancellationToken? = nil
    ) async throws -> ExtractResult {
        guard !archivePath.isEmpty, !destinationDir.isEmpty, FileManager.default.fileExists(atPath: archivePath) else {
            throw ArchiveError.fileNotFound
        }
        
        let startTime = CFAbsoluteTimeGetCurrent()
        let combinedProgress: ProgressSink = { p in
            progress?(p)
        }

        let initialPolicy = handling.requestPolicy

        var candidatePasswords: [String?] = []
        if let explicitPwd = password, !explicitPwd.isEmpty {
            candidatePasswords.append(explicitPwd)
        } else {
            candidatePasswords.append(nil)
            if autoVaultUnlock {
                let vaultCandidates = passwordVault.getEntries()
                for entry in vaultCandidates {
                    candidatePasswords.append(entry.password)
                }
            }
        }

        var lastError: Error?
        for candidate in candidatePasswords {
            let request = ExtractionRequest(
                archivePath: archivePath,
                destinationRoot: destinationDir,
                password: candidate,
                entryFilter: nil,
                rootAtCommonParent: false,
                policy: initialPolicy
            )

            do {
                let outcome = try await extract(
                    request,
                    handling: handling,
                    progress: combinedProgress,
                    token: token
                )
                if token?.isCancelled() == true || Task.isCancelled {
                    throw ArchiveError.cancelled
                }
                switch outcome.status {
                case .stopped:
                    throw ArchiveError.extractionStopped
                case .completedWithFailures:
                    throw ArchiveError.partialExtractionFailure(outcome: outcome)
                case .completed:
                    break
                }
                let elapsed = max(0.001, CFAbsoluteTimeGetCurrent() - startTime)
                if let pwd = candidate {
                    ArchivePasswordStore.shared.setPassword(pwd, for: archivePath)
                }
                return ExtractResult(
                    archivePath: archivePath,
                    destinationDir: destinationDir,
                    durationSeconds: elapsed,
                    unlockedPassword: candidate,
                    isVaultUnlocked: candidate != nil && candidate != password,
                    outcome: outcome
                )
            } catch {
                if token?.isCancelled() == true || Task.isCancelled {
                    throw ArchiveError.cancelled
                }
                if let archiveErr = error as? ArchiveError {
                    if !archiveErr.isPasswordRelated {
                        throw archiveErr
                    }
                    lastError = archiveErr
                    continue
                }
                let archiveErr = ArchiveError.from(error: error)
                if !archiveErr.isPasswordRelated {
                    throw archiveErr
                }
                lastError = archiveErr
            }
        }

        if let err = lastError {
            throw err
        }
        throw ArchiveError.passwordRequired
    }

    public func quickExtract(
        archivePath: String,
        destinationDir: String,
        password: String?,
        autoVaultUnlock: Bool,
        progress: (@Sendable (ArchiveProgress) -> Void)?
    ) async throws -> ExtractResult {
        return try await quickExtract(
            archivePath: archivePath,
            destinationDir: destinationDir,
            password: password,
            autoVaultUnlock: autoVaultUnlock,
            handling: .policy(.keepBoth),
            progress: progress,
            token: nil
        )
    }

    public func quickExtract(
        archivePath: String,
        destinationDir: String,
        password: String?,
        autoVaultUnlock: Bool,
        progress: (@Sendable (ArchiveProgress) -> Void)?,
        token: CancellationToken?
    ) async throws -> ExtractResult {
        return try await quickExtract(
            archivePath: archivePath,
            destinationDir: destinationDir,
            password: password,
            autoVaultUnlock: autoVaultUnlock,
            handling: .policy(.keepBoth),
            progress: progress,
            token: token
        )
    }
    
    /// Extracts one entry (a directory entry includes its descendants) directly into `destinationDir`.
    @discardableResult
    public func extractSingleEntry(
        archivePath: String,
        entryPath: String,
        destinationDir: String,
        password: String? = nil,
        handling: CollisionHandling = .policy(.keepBoth)
    ) async throws -> ExtractionOutcome {
        let explicitPwd = password ?? ArchivePasswordStore.shared.getPassword(for: archivePath)
        return try await extractEntries(
            archivePath: archivePath,
            entryPaths: [entryPath],
            destinationDir: destinationDir,
            password: explicitPwd,
            handling: handling
        )
    }

    /// Extracts the selected entries, plus every descendant of a selected directory, through the
    /// collision-safe session loop. Entries are placed relative to their deepest common parent:
    /// selecting `docs/report.pdf` writes `<destinationDir>/report.pdf`.
    @discardableResult
    public func extractEntries(
        archivePath: String,
        entryPaths: [String],
        destinationDir: String,
        password: String?,
        handling: CollisionHandling = .policy(.keepBoth),
        progress: ProgressSink? = nil
    ) async throws -> ExtractionOutcome {
        guard !archivePath.isEmpty, !destinationDir.isEmpty, FileManager.default.fileExists(atPath: archivePath) else {
            throw ArchiveError.fileNotFound
        }
        guard !entryPaths.isEmpty else { return .nothingSelected }
        try FileManager.default.createDirectory(atPath: destinationDir, withIntermediateDirectories: true)

        let spotlightCreated = ArchiveExtractor.preventSpotlightIndexing(at: destinationDir)
        defer {
            ArchiveExtractor.removeSpotlightPrevention(at: destinationDir, wasCreated: spotlightCreated)
        }

        let request = ExtractionRequest(
            archivePath: archivePath,
            destinationRoot: destinationDir,
            password: password,
            entryFilter: entryPaths,
            rootAtCommonParent: true,
            policy: handling.requestPolicy
        )
        let outcome: ExtractionOutcome
        do {
            outcome = try await extract(request, handling: handling, progress: progress)
        } catch {
            throw ArchiveError.from(error: error)
        }
        ArchiveExtractor.propagateQuarantine(from: archivePath, to: destinationDir)
        return outcome
    }
    
    internal func executePipelineExtract(
        archivePath: String,
        destinationDir: String,
        password: String?,
        progress: (@Sendable (ArchiveProgress) -> Void)?,
        token: CancellationToken? = nil
    ) async throws -> Double {
        let startTime = CFAbsoluteTimeGetCurrent()
        let request = ExtractionRequest(
            archivePath: archivePath,
            destinationRoot: destinationDir,
            password: password,
            entryFilter: nil,
            rootAtCommonParent: false,
            policy: .keepBoth
        )
        let outcome = try await extract(request, handling: .policy(.keepBoth), progress: progress, token: token)
        switch outcome.status {
        case .stopped:
            throw ArchiveError.extractionStopped
        case .completedWithFailures:
            throw ArchiveError.partialExtractionFailure(outcome: outcome)
        case .completed:
            return max(0.001, CFAbsoluteTimeGetCurrent() - startTime)
        }
    }
}


// MARK: - Inspect Facade

//
//


// MARK: - Unified Archive Inspection & Structure Probing Facade

extension TTZipEngineFacade {
    public func openVfsSession(archivePath: String, password: String? = nil) throws -> VfsSession {
        let explicitPwd = password ?? ArchivePasswordStore.shared.getPassword(for: archivePath)
        return try TTZipCore.openVfsSession(archivePath: archivePath, password: explicitPwd)
    }

    public func openArchiveSession(archivePath: String, password: String? = nil) throws -> VfsSessionStatus {
        let explicitPwd = password ?? ArchivePasswordStore.shared.getPassword(for: archivePath)
        return try TTZipCore.openArchiveSession(archivePath: archivePath, passwordHint: explicitPwd)
    }

    /// Legacy snapshot entry returning an owning `OpaquePointer` erased as `Any`.
    ///
    /// The caller must transfer this one reference to an owning wrapper or release it once
    /// through the matching snapshot C-ABI. This method does not manage the returned lease.
    /// The temporary output view is not returned. For object-based VFS queries, use
    /// `openVfsSession`; that interface is not a compact-snapshot representation.
    ///
    /// Missing bridge symbols report `engineFailure` with unsupported-feature status.
    /// A null native result reports `openFailed`, since this ABI exposes no detailed cause.
    public func openVfsSnapshot(archivePath: String, password: String? = nil) async throws -> Any {
        try snapshotBridge.open(archivePath: archivePath, password: password)
    }

    /// Inspects an archive and reports the credential it used, without touching the shared
    /// password cache.
    ///
    /// The inspection is a query: it reads the shared cache to pick a candidate, keeps the
    /// existing vault retry policy and reports the credential that worked in
    /// `unlockedPassword`, but it never publishes, restores or removes a shared credential.
    /// Writing the shared cache is an authorization act and belongs to the owner that accepts
    /// the result, in that owner's own acceptance block. Publishing from here would also let a
    /// credential read before an `await` overwrite a newer credential committed while the
    /// inspection was in flight.
    ///
    /// The remaining side effects of the query are unchanged: reading the cache refreshes its
    /// LRU recency, and a vault unlock still records its usage.
    public func inspectArchive(
        archivePath: String,
        password: String? = nil,
        autoVaultUnlock: Bool = true
    ) async throws -> ArchiveInspectionResult {
        guard !archivePath.isEmpty, FileManager.default.fileExists(atPath: archivePath) else {
            throw ArchiveError.fileNotFound
        }
        
        let explicitPwd = password ?? ArchivePasswordStore.shared.getPassword(for: archivePath)
        
        let passwordFailure: ArchiveError
        do {
            // The facade owns retries. The reader must not consult the global vault as well.
            let entries = try await reader.inspect(archivePath: archivePath, password: explicitPwd, candidatePasswords: [])
            let treeNode = ArchiveComponentTreeBuilder.buildTree(from: entries)
            let securityReport = securityScanner.scanEntriesForReport(entries)
            return ArchiveInspectionResult(
                archivePath: archivePath,
                entries: entries,
                treeNode: treeNode,
                securityReport: securityReport,
                unlockedPassword: explicitPwd
            )
        } catch {
            let mapped = ArchiveError.from(error: error)
            guard mapped.isPasswordRelated else { throw mapped }
            // An explicit or already selected credential has its own failure contract.
            // Only a passwordless inspection may discover a credential from the vault.
            guard explicitPwd?.isEmpty != false else { throw mapped }
            passwordFailure = mapped
        }

        if autoVaultUnlock {
            let vaultEntries = passwordVault.getEntries()
            for entry in vaultEntries {
                do {
                    let entries = try await reader.inspect(
                        archivePath: archivePath, password: entry.password, candidatePasswords: []
                    )
                    passwordVault.recordUsage(id: entry.id)
                    let treeNode = ArchiveComponentTreeBuilder.buildTree(from: entries)
                    let securityReport = securityScanner.scanEntriesForReport(entries)
                    return ArchiveInspectionResult(
                        archivePath: archivePath,
                        entries: entries,
                        treeNode: treeNode,
                        securityReport: securityReport,
                        unlockedPassword: entry.password
                    )
                } catch {
                    let mapped = ArchiveError.from(error: error)
                    guard mapped.isPasswordRelated else { throw mapped }
                }
            }
        }
        
        throw passwordFailure
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

// MARK: - Selected-Entry Extraction on Extractor Types

// Defined in the TTZipCore module because the session loop (`extract(_:handling:progress:)`) and the
// staging registry live here; `ArchiveExtractor` itself sits in the lower TTZipCoreArchive module.

extension ArchiveExtractor {
    /// Extracts a single entry (a directory entry includes its descendants) directly into `destinationDir`.
    @discardableResult
    public func extractSingleFile(
        archivePath: String,
        entryPath: String,
        destinationDir: String,
        password: String?,
        handling: CollisionHandling = .policy(.keepBoth)
    ) async throws -> ExtractionOutcome {
        try await TTZipEngineFacade.shared.extractEntries(
            archivePath: archivePath,
            entryPaths: [entryPath],
            destinationDir: destinationDir,
            password: password,
            handling: handling
        )
    }
}

extension ArchiveSelectiveExtractor {
    /// Extracts the selected entries (a directory entry includes its descendants) into `destinationDir`,
    /// relative to their deepest common parent.
    @discardableResult
    public func extractSelected(
        archivePath: String,
        targetEntryPaths: Set<String>,
        destinationDir: String,
        password: String? = nil,
        handling: CollisionHandling = .policy(.keepBoth)
    ) async throws -> ExtractionOutcome {
        try await TTZipEngineFacade.shared.extractEntries(
            archivePath: archivePath,
            entryPaths: targetEntryPaths.sorted(),
            destinationDir: destinationDir,
            password: password,
            handling: handling
        )
    }
}

// MARK: - Extraction Request Helpers

private extension CollisionHandling {
    /// Policy recorded in the `ExtractionRequest`; interactive sessions resolve conflicts
    /// through the decision provider, so the request carries the non-destructive default.
    var requestPolicy: CollisionPolicy {
        switch self {
        case .policy(let policy):
            return policy
        case .interactive:
            return .keepBoth
        }
    }
}

private extension ExtractionOutcome {
    /// Outcome of a selection that names no entries: nothing is read or written.
    static var nothingSelected: ExtractionOutcome {
        ExtractionOutcome(status: .completed, extracted: 0, replaced: 0, keptBoth: 0, skipped: 0, failed: [])
    }
}
