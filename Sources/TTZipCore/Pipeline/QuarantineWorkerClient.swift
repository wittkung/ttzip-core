// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import TTZipCoreBindings
import TTZipCoreTypes
import Foundation
#if canImport(Darwin)
import Darwin
#endif

/// Errors that can occur during quarantined archive processing in an isolated sandbox worker.
public enum QuarantineWorkerError: LocalizedError, Sendable {
    case fileNotFound(URL)
    case workerNotFound
    case workerCrashed(exitCode: Int32, reason: String)
    case zipSlipDetected(String)
    case communicationFailure(String)
    case executionFailed(code: Int, message: String)

    public var errorDescription: String? {
        switch self {
        case .fileNotFound(let url):
            return "Archive file not found at: \(url.path)"
        case .workerNotFound:
            return "Sandboxed ttarchive-worker executable could not be located"
        case .workerCrashed(let code, let reason):
            return "Quarantine worker terminated unexpectedly (code: \(code)): \(reason)"
        case .zipSlipDetected(let path):
            return "Zip Slip directory traversal vulnerability detected and blocked: \(path)"
        case .communicationFailure(let msg):
            return "IPC communication error with quarantine worker: \(msg)"
        case .executionFailed(let code, let msg):
            return "Quarantine extraction failed (\(code)): \(msg)"
        }
    }
}

/// Client managing isolated out-of-process execution for quarantined and untrusted archives.
public final class QuarantineWorkerClient: Sendable {
    public static let shared = QuarantineWorkerClient()

    private init() {}

    /// Checks whether an archive file contains the macOS `com.apple.quarantine` extended attribute.
    public func isQuarantined(url: URL) -> Bool {
        let path = url.path
        let attrName = "com.apple.quarantine"
        let size = getxattr(path, attrName, nil, 0, 0, 0)
        return size > 0
    }

    /// Determines whether the specified file should be routed to the isolated out-of-process worker.
    public func shouldRouteToSandboxWorker(url: URL) -> Bool {
        return isQuarantined(url: url)
    }

    /// Resolves the file URL of the sandboxed `ttarchive-worker` helper executable.
    public func locateWorkerExecutable() -> URL? {
        // 1. Check main bundle Resources/Helpers
        if let bundleURL = Bundle.main.url(forResource: "ttarchive-worker", withExtension: nil) {
            return bundleURL
        }
        if let helperURL = Bundle.main.bundleURL
            .appendingPathComponent("Contents/Helpers/ttarchive-worker", isDirectory: false) as URL?,
           FileManager.default.fileExists(atPath: helperURL.path) {
            return helperURL
        }

        // 2. Check development build roots
        let cwd = FileManager.default.currentDirectoryPath
        let devCandidates = [
            URL(fileURLWithPath: cwd).appendingPathComponent("bazel-bin/products/ttzip/workers/ttarchive-worker/ttarchive-worker"),
            URL(fileURLWithPath: cwd).appendingPathComponent("products/ttzip/workers/ttarchive-worker/target/release/ttarchive-worker"),
            URL(fileURLWithPath: cwd).appendingPathComponent("products/ttzip/workers/ttarchive-worker/target/debug/ttarchive-worker"),
            URL(fileURLWithPath: cwd).appendingPathComponent("products/ttzip/workers/ttarchive-worker/ttarchive-worker"),
        ]
        for url in devCandidates {
            if FileManager.default.isExecutableFile(atPath: url.path) {
                return url
            }
        }
        return nil
    }

    /// Extracts an untrusted archive out-of-process into destination directory using Single-Root-FD Jail posix_spawn.
    public func extractQuarantinedArchive(
        url: URL,
        destinationDirectory: URL
    ) async throws -> UInt64 {
        guard FileManager.default.fileExists(atPath: url.path) else {
            throw QuarantineWorkerError.fileNotFound(url)
        }
        guard let workerURL = locateWorkerExecutable() else {
            throw QuarantineWorkerError.workerNotFound
        }

        return try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                do {
                    let totalBytes = try Self.performPosixSpawnExtraction(
                        archiveURL: url,
                        destinationDirectory: destinationDirectory,
                        workerURL: workerURL
                    )
                    continuation.resume(returning: totalBytes)
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    // MARK: - Private Single-Root-FD Jail & Public Bookmark IPC Helpers

    /// Creates a public Foundation URL bookmark with implicit security scope (App Store Review Guideline 2.5.1 compliant)
    /// so the `com.apple.security.inherit` child worker can resolve and access the Powerbox-authorized file or directory.
    private static func createImplicitSecurityScopedBookmarkBase64(for url: URL) -> String? {
        if let data = try? url.bookmarkData(
            options: .minimalBookmark,
            includingResourceValuesForKeys: nil,
            relativeTo: nil
        ), !data.isEmpty {
            return data.base64EncodedString()
        }
        if let data = try? url.bookmarkData(
            options: [],
            includingResourceValuesForKeys: nil,
            relativeTo: nil
        ), !data.isEmpty {
            return data.base64EncodedString()
        }
        return nil
    }

    /// Creates base64-encoded implicit security-scoped bookmarks for the source archive and destination directory.
    private static func createWorkerSecurityBookmarks(
        archiveURL: URL,
        destinationDirectory: URL
    ) -> (archiveBookmark: String?, destinationBookmark: String?) {
        let archiveBookmark = createImplicitSecurityScopedBookmarkBase64(for: archiveURL)
        let destinationBookmark = createImplicitSecurityScopedBookmarkBase64(for: destinationDirectory)
        return (archiveBookmark, destinationBookmark)
    }

    /// Executes extraction inside ttarchive-worker via Darwin `posix_spawn` with a jailed root descriptor.
    private static func performPosixSpawnExtraction(
        archiveURL: URL,
        destinationDirectory: URL,
        workerURL: URL
    ) throws -> UInt64 {
        try ensureDestinationDirectoryExists(destinationDirectory)

        let archiveFd = try openCloexecDescriptor(
            path: archiveURL.path,
            flags: O_RDONLY,
            label: "archive file"
        )
        defer { Darwin.close(archiveFd) }

        let dirFd = try openCloexecDescriptor(
            path: destinationDirectory.path,
            flags: O_RDONLY | O_DIRECTORY,
            label: "destination directory"
        )
        defer { Darwin.close(dirFd) }

        let bookmarks = createWorkerSecurityBookmarks(
            archiveURL: archiveURL,
            destinationDirectory: destinationDirectory
        )
        return try executeSpawnedWorker(
            workerURL: workerURL,
            archiveURL: archiveURL,
            archiveFd: archiveFd,
            dirFd: dirFd,
            bookmarks: bookmarks
        )
    }

    private static func ensureDestinationDirectoryExists(_ destinationDirectory: URL) throws {
        guard !FileManager.default.fileExists(atPath: destinationDirectory.path) else { return }
        do {
            try FileManager.default.createDirectory(at: destinationDirectory, withIntermediateDirectories: true)
        } catch {
            throw QuarantineWorkerError.executionFailed(
                code: -1,
                message: "Failed to create destination directory: \(error.localizedDescription)"
            )
        }
    }

    private static func openCloexecDescriptor(
        path: String,
        flags: Int32,
        label: String
    ) throws -> Int32 {
        let rawFd = Darwin.open(path, flags)
        guard rawFd >= 0 else {
            let errCode = errno
            throw QuarantineWorkerError.executionFailed(
                code: Int(errCode),
                message: "Failed to open \(label) descriptor: \(String(cString: strerror(errCode)))"
            )
        }
        let dupFd = Darwin.fcntl(rawFd, F_DUPFD_CLOEXEC, 10)
        Darwin.close(rawFd)
        guard dupFd >= 0 else {
            let errCode = errno
            throw QuarantineWorkerError.executionFailed(
                code: Int(errCode),
                message: "Failed to duplicate \(label) descriptor: \(String(cString: strerror(errCode)))"
            )
        }
        return dupFd
    }

    private static func executeSpawnedWorker(
        workerURL: URL,
        archiveURL: URL,
        archiveFd: Int32,
        dirFd: Int32,
        bookmarks: (archiveBookmark: String?, destinationBookmark: String?)
    ) throws -> UInt64 {
        var outPipe: [Int32] = [0, 0]
        guard Darwin.pipe(&outPipe) == 0 else {
            let errCode = errno
            throw QuarantineWorkerError.communicationFailure("Failed to create stdout pipe: \(String(cString: strerror(errCode)))")
        }

        let childArchiveFd: Int32 = 4
        let childTargetFd: Int32 = 5

        var fileActions: posix_spawn_file_actions_t? = nil
        let actionRc = configureFileActions(
            fileActions: &fileActions,
            outPipe: outPipe,
            archiveFd: archiveFd,
            childArchiveFd: childArchiveFd,
            dirFd: dirFd,
            childTargetFd: childTargetFd
        )
        guard actionRc == 0 else {
            closePipePair(outPipe)
            throw QuarantineWorkerError.communicationFailure("Failed to configure posix_spawn file actions: \(actionRc)")
        }
        defer { posix_spawn_file_actions_destroy(&fileActions) }

        var attr: posix_spawnattr_t? = nil
        let attrRc = configureSpawnAttributes(attr: &attr)
        guard attrRc == 0 else {
            closePipePair(outPipe)
            throw QuarantineWorkerError.communicationFailure("Failed to configure posix_spawn attributes: \(attrRc)")
        }
        defer { posix_spawnattr_destroy(&attr) }

        let pid: pid_t
        do {
            pid = try spawnWorkerProcess(
                workerURL: workerURL,
                archiveURL: archiveURL,
                childArchiveFd: childArchiveFd,
                childTargetFd: childTargetFd,
                archiveBookmark: bookmarks.archiveBookmark,
                destinationBookmark: bookmarks.destinationBookmark,
                fileActions: &fileActions,
                attr: &attr
            )
        } catch {
            closePipePair(outPipe)
            throw error
        }

        Darwin.close(outPipe[1])
        let outData = readDescriptorToEnd(fd: outPipe[0])
        Darwin.close(outPipe[0])

        var status: Int32 = 0
        waitpid(pid, &status, 0)
        return try parseWorkerOutput(data: outData, waitStatus: status)
    }

    private static func closePipePair(_ pipePair: [Int32]) {
        Darwin.close(pipePair[0])
        Darwin.close(pipePair[1])
    }

    /// Configures Darwin `posix_spawn_file_actions_t` descriptors for Single-Root-FD Jail isolation.
    private static func configureFileActions(
        fileActions: inout posix_spawn_file_actions_t?,
        outPipe: [Int32],
        archiveFd: Int32,
        childArchiveFd: Int32,
        dirFd: Int32,
        childTargetFd: Int32
    ) -> Int32 {
        var rc = posix_spawn_file_actions_init(&fileActions)
        guard rc == 0 else { return rc }

        rc = posix_spawn_file_actions_addopen(&fileActions, STDIN_FILENO, "/dev/null", O_RDONLY, 0)
        guard rc == 0 else { return rc }

        rc = posix_spawn_file_actions_adddup2(&fileActions, outPipe[1], STDOUT_FILENO)
        guard rc == 0 else { return rc }

        rc = posix_spawn_file_actions_addclose(&fileActions, outPipe[0])
        guard rc == 0 else { return rc }

        rc = posix_spawn_file_actions_adddup2(&fileActions, archiveFd, childArchiveFd)
        guard rc == 0 else { return rc }

        rc = posix_spawn_file_actions_adddup2(&fileActions, dirFd, childTargetFd)
        return rc
    }

    /// Configures `posix_spawnattr_t` with `POSIX_SPAWN_CLOEXEC_DEFAULT`.
    private static func configureSpawnAttributes(attr: inout posix_spawnattr_t?) -> Int32 {
        let rc = posix_spawnattr_init(&attr)
        guard rc == 0 else { return rc }
        let flagRc = posix_spawnattr_setflags(&attr, Int16(POSIX_SPAWN_CLOEXEC_DEFAULT))
        if flagRc != 0 {
            posix_spawnattr_destroy(&attr)
            return flagRc
        }
        return 0
    }

    /// Spawns worker subprocess using `posix_spawn` with one-shot CLI arguments.
    private static func spawnWorkerProcess(
        workerURL: URL,
        archiveURL: URL,
        childArchiveFd: Int32,
        childTargetFd: Int32,
        archiveBookmark: String?,
        destinationBookmark: String?,
        fileActions: inout posix_spawn_file_actions_t?,
        attr: inout posix_spawnattr_t?
    ) throws -> pid_t {
        var pid: pid_t = 0
        let path = workerURL.path
        var args = [
            path,
            "extract",
            "--archive-fd",
            "\(childArchiveFd)",
            "--target-dir-fd",
            "\(childTargetFd)",
        ]
        if let archiveBookmark = archiveBookmark, !archiveBookmark.isEmpty {
            args.append("--archive-bookmark")
            args.append(archiveBookmark)
        }
        if let destinationBookmark = destinationBookmark, !destinationBookmark.isEmpty {
            args.append("--dest-bookmark")
            args.append(destinationBookmark)
        }
        args.append(archiveURL.path)

        let cArgs: [UnsafeMutablePointer<CChar>?] = args.map { strdup($0) } + [nil]
        defer {
            for ptr in cArgs {
                if let ptr = ptr { free(ptr) }
            }
        }

        let spawnRet = posix_spawn(&pid, path, &fileActions, &attr, cArgs, nil)
        guard spawnRet == 0 else {
            let errStr = String(cString: strerror(spawnRet))
            throw QuarantineWorkerError.communicationFailure("posix_spawn failed (code \(spawnRet)): \(errStr)")
        }
        return pid
    }

    /// Reads all bytes from specified file descriptor until EOF is reached.
    private static func readDescriptorToEnd(fd: Int32) -> Data {
        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        while true {
            let bytesRead = Darwin.read(fd, &buffer, buffer.count)
            if bytesRead <= 0 {
                break
            }
            data.append(buffer, count: bytesRead)
        }
        return data
    }

    /// Parses worker NDJSON lines and extracts completion bytes or throws domain errors.
    private static func parseWorkerOutput(data: Data, waitStatus: Int32) throws -> UInt64 {
        guard let outputStr = String(data: data, encoding: .utf8) else {
            throw QuarantineWorkerError.communicationFailure("Worker output could not be decoded as UTF-8")
        }

        var completedBytes: UInt64?
        for line in outputStr.split(separator: "\n") {
            if let bytes = try parseWorkerEventLine(String(line)) {
                completedBytes = bytes
                break
            }
        }

        if waitStatus != 0 {
            let exitCode = (waitStatus & 0x7f) == 0 ? ((waitStatus >> 8) & 0xff) : (waitStatus & 0x7f)
            throw QuarantineWorkerError.workerCrashed(
                exitCode: exitCode,
                reason: "Worker process terminated with status \(waitStatus)"
            )
        }

        if let bytes = completedBytes {
            return bytes
        }
        throw QuarantineWorkerError.communicationFailure("No completed event received from worker")
    }

    private static func parseWorkerEventLine(_ rawLine: String) throws -> UInt64? {
        let trimmed = rawLine.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty,
              let lineData = trimmed.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: lineData) as? [String: Any] else {
            return nil
        }

        let eventType = (json["type"] as? String) ?? (json["event"] as? String)
        if eventType == "completed" || eventType == "ExtractCompleted" {
            let payload = json["payload"] as? [String: Any]
            let number = (payload?["total_bytes"] as? NSNumber) ?? (json["total_bytes"] as? NSNumber)
            return number?.uint64Value ?? 0
        }
        if eventType == "error" || eventType == "Error" {
            throw parseWorkerErrorEvent(json)
        }
        return nil
    }

    private static func parseWorkerErrorEvent(_ json: [String: Any]) -> QuarantineWorkerError {
        let payload = json["payload"] as? [String: Any]
        let codeNumber = (payload?["code"] as? NSNumber) ?? (json["code"] as? NSNumber)
        let code = codeNumber?.intValue ?? -1
        let msg = (payload?["message"] as? String) ?? (json["message"] as? String) ?? "Unknown error"
        if isZipSlipSecurityFailure(code: code, message: msg) {
            return .zipSlipDetected(msg)
        }
        return .executionFailed(code: code, message: msg)
    }

    private static let zipSlipKeywords: [String] = [
        "zip slip", "zipslip", "symlink", "security", "traversal"
    ]

    private static func isZipSlipSecurityFailure(code: Int, message: String) -> Bool {
        if code == -30 || code == -403 {
            return true
        }
        let lowerMsg = message.lowercased()
        return zipSlipKeywords.contains { lowerMsg.contains($0) }
    }
}
