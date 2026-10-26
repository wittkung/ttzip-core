// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation

/// Persistent registry tracking destination roots with active extraction staging directories.
///
/// If the application is terminated mid-extraction (e.g. SIGKILL, power loss, crash),
/// recorded roots are swept on the next application launch to reclaim orphaned disk space.
public final class ExtractionStagingRegistry: @unchecked Sendable {
    public static let shared = ExtractionStagingRegistry()

    public static let userDefaultsKey = "TTZip_ActiveStagingRoots"

    private let userDefaults: UserDefaults
    private let lock = NSLock()

    public init(userDefaults: UserDefaults = .standard) {
        self.userDefaults = userDefaults
    }

    /// Registers an active destination root before extraction begins.
    public func register(destinationRoot: String) {
        guard !destinationRoot.isEmpty else { return }
        lock.lock()
        defer { lock.unlock() }

        var roots = Set(userDefaults.stringArray(forKey: Self.userDefaultsKey) ?? [])
        roots.insert(destinationRoot)
        userDefaults.set(Array(roots), forKey: Self.userDefaultsKey)
    }

    /// Unregisters an active destination root upon extraction completion or abort.
    public func unregister(destinationRoot: String) {
        guard !destinationRoot.isEmpty else { return }
        lock.lock()
        defer { lock.unlock() }

        var roots = Set(userDefaults.stringArray(forKey: Self.userDefaultsKey) ?? [])
        roots.remove(destinationRoot)
        userDefaults.set(Array(roots), forKey: Self.userDefaultsKey)
    }

    /// Returns the snapshot of currently registered active roots.
    public func registeredRoots() -> Set<String> {
        lock.lock()
        defer { lock.unlock() }
        return Set(userDefaults.stringArray(forKey: Self.userDefaultsKey) ?? [])
    }

    /// Sweeps orphaned staging directories in all recorded roots on app launch.
    ///
    /// Executes off the main thread to prevent startup latency.
    @discardableResult
    public func sweepOnLaunch() -> UInt32 {
        let rootsToSweep: Set<String>
        do {
            lock.lock()
            rootsToSweep = Set(userDefaults.stringArray(forKey: Self.userDefaultsKey) ?? [])
            lock.unlock()
        }

        var totalSwept: UInt32 = 0
        var remainingRoots: Set<String> = []

        let fm = FileManager.default
        for root in rootsToSweep {
            var isDir: ObjCBool = false
            if fm.fileExists(atPath: root, isDirectory: &isDir), isDir.boolValue {
                do {
                    let swept = try sweepStaleStaging(destinationRoot: root)
                    totalSwept += swept
                } catch {
                    // Non-fatal error during stale sweep; keep root for future retry
                    remainingRoots.insert(root)
                }
            }
        }

        lock.lock()
        userDefaults.set(Array(remainingRoots), forKey: Self.userDefaultsKey)
        lock.unlock()

        return totalSwept
    }
}
