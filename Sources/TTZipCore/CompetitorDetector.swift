// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import Foundation
#if canImport(AppKit)
import AppKit
#endif

/// Value type representing detected third-party archive applications and CLI tools.
public struct CompetitorTool: Identifiable, Sendable {
    public var id: String { toolId }
    public let toolId: String
    public let name: String
    public let bundleIdentifier: String?
    public let isInstalled: Bool
    public let appPath: String?
    public let cliExecutablePath: String?
    public let iconSystemName: String
    public let description: String
    
    public init(
        toolId: String,
        name: String,
        bundleIdentifier: String? = nil,
        isInstalled: Bool,
        appPath: String? = nil,
        cliExecutablePath: String? = nil,
        iconSystemName: String = "shippingbox.fill",
        description: String = ""
    ) {
        self.toolId = toolId
        self.name = name
        self.bundleIdentifier = bundleIdentifier
        self.isInstalled = isInstalled
        self.appPath = appPath
        self.cliExecutablePath = cliExecutablePath
        self.iconSystemName = iconSystemName
        self.description = description
    }
}

/// Measured performance score against detected toolchains.
public struct CompetitorRealScore: Identifiable, Sendable {
    public var id: String { tool.id }
    public let tool: CompetitorTool
    public let measuredElapsedSeconds: Double
    public let measuredThroughputMBs: Double
    public let relativeSpeedupVsNative: Double
    
    public init(
        tool: CompetitorTool,
        measuredElapsedSeconds: Double,
        measuredThroughputMBs: Double,
        relativeSpeedupVsNative: Double
    ) {
        self.tool = tool
        self.measuredElapsedSeconds = measuredElapsedSeconds
        self.measuredThroughputMBs = measuredThroughputMBs
        self.relativeSpeedupVsNative = relativeSpeedupVsNative
    }
}

/// Detection service identifying installed archiving applications and CLI binaries across macOS.
public enum CompetitorDetector {
    /// Detects all supported third-party archiving tools and CLI engines installed on host system.
    public static func detectAllCompetitors() -> [CompetitorTool] {
        let sevenZipCli = findExecutable(names: ["7zz", "7z"])
        return detectSystemAndDesktopCompetitors(sevenZipCli: sevenZipCli)
            + detectCliEngineCompetitors()
            + detectBenchmarkAndSpecializedCompetitors()
    }

    private static func detectSystemAndDesktopCompetitors(sevenZipCli: String?) -> [CompetitorTool] {
        let dittoPath = findExecutable(names: ["ditto"], extraPaths: ["/usr/bin/ditto"])
        let dittoApp = checkAppInstalled(
            bundleId: "com.apple.archiveutility",
            defaultPath: "/System/Library/CoreServices/Applications/Archive Utility.app"
        )
        let kekaInstalled = checkAppInstalled(bundleId: "com.aone.keka", defaultPath: "/Applications/Keka.app")
        let kekaBase = kekaInstalled.path ?? "/Applications/Keka.app"
        let kekaCli = findExecutable(
            names: ["keka7zz", "keka7z", "kekaexec"],
            extraPaths: [
                kekaBase + "/Contents/MacOS/keka7zz",
                kekaBase + "/Contents/MacOS/keka7z",
                kekaBase + "/Contents/Resources/kekaexec"
            ]
        ) ?? sevenZipCli

        let bzInstalled = checkAppInstalled(bundleId: "com.macitbetter.betterzip", defaultPath: "/Applications/BetterZip.app")
        let bzBase = bzInstalled.path ?? "/Applications/BetterZip.app"
        let bzCli = findExecutable(
            names: ["betterzip", "7za"],
            extraPaths: [bzBase + "/Contents/Helpers/7za", bzBase + "/Contents/Resources/betterzip"]
        )

        let mzInstalled = checkAppInstalled(bundleId: "com.awehunt.maczip", defaultPath: "/Applications/MacZip.app")
        let mzBase = mzInstalled.path ?? "/Applications/MacZip.app"

        return [
            CompetitorTool(
                toolId: "native_ditto",
                name: "macOS Native (ditto)",
                bundleIdentifier: "com.apple.archiveutility",
                isInstalled: dittoPath != nil,
                appPath: dittoApp.path,
                cliExecutablePath: dittoPath,
                iconSystemName: "applelogo",
                description: "macOS default Archive Utility and ditto compression tool"
            ),
            CompetitorTool(
                toolId: "7zip_cli",
                name: "7-Zip (7zz CLI)",
                bundleIdentifier: nil,
                isInstalled: sevenZipCli != nil,
                appPath: nil,
                cliExecutablePath: sevenZipCli,
                iconSystemName: "terminal.fill",
                description: "Official 7-Zip ARM64 multi-threaded CLI engine"
            ),
            CompetitorTool(
                toolId: "keka",
                name: "Keka",
                bundleIdentifier: "com.aone.keka",
                isInstalled: kekaInstalled.installed || kekaCli != nil,
                appPath: kekaInstalled.path,
                cliExecutablePath: kekaCli,
                iconSystemName: "archivebox.circle.fill",
                description: "macOS open-source archiver powered by p7zip"
            ),
            CompetitorTool(
                toolId: "betterzip",
                name: "BetterZip",
                bundleIdentifier: "com.macitbetter.betterzip",
                isInstalled: bzInstalled.installed || bzCli != nil,
                appPath: bzInstalled.path,
                cliExecutablePath: bzCli ?? sevenZipCli,
                iconSystemName: "slider.horizontal.3",
                description: "macOS archive manager"
            ),
            CompetitorTool(
                toolId: "maczip",
                name: "MacZip (eZip)",
                bundleIdentifier: "com.awehunt.maczip",
                isInstalled: mzInstalled.installed,
                appPath: mzInstalled.path,
                cliExecutablePath: findExecutable(names: ["MacZip"], extraPaths: [mzBase + "/Contents/MacOS/MacZip"]),
                iconSystemName: "folder.badge.gearshape",
                description: "macOS archiver utility"
            )
        ]
    }

    private static func detectCliEngineCompetitors() -> [CompetitorTool] {
        let pigzCli = findExecutable(names: ["pigz"])
        let zstdCli = findExecutable(names: ["zstd"])
        let libdeflateCli = findExecutable(names: ["libdeflate-gzip"])
        let zpaqfranzCli = findExecutable(names: ["zpaqfranz", "zpaq"])
        let lzfseCli = findExecutable(names: ["lzfse"])
        return [
            CompetitorTool(
                toolId: "pigz_cli",
                name: "Parallel pigz",
                bundleIdentifier: nil,
                isInstalled: pigzCli != nil,
                appPath: nil,
                cliExecutablePath: pigzCli,
                iconSystemName: "cpu.fill",
                description: "Parallel multi-threaded GZIP implementation"
            ),
            CompetitorTool(
                toolId: "zstd_cli",
                name: "Zstandard zstd",
                bundleIdentifier: nil,
                isInstalled: zstdCli != nil,
                appPath: nil,
                cliExecutablePath: zstdCli,
                iconSystemName: "bolt.fill",
                description: "Meta Zstandard real-time compression algorithm CLI"
            ),
            CompetitorTool(
                toolId: "libdeflate_gzip",
                name: "libdeflate-gzip",
                bundleIdentifier: nil,
                isInstalled: libdeflateCli != nil,
                appPath: nil,
                cliExecutablePath: libdeflateCli,
                iconSystemName: "flame.fill",
                description: "High-throughput libdeflate GZIP implementation"
            ),
            CompetitorTool(
                toolId: "zpaq_franz",
                name: "ZPAQ Franz",
                bundleIdentifier: nil,
                isInstalled: zpaqfranzCli != nil,
                appPath: nil,
                cliExecutablePath: zpaqfranzCli,
                iconSystemName: "diamond.fill",
                description: "Context-mixing archiving and backup engine"
            ),
            CompetitorTool(
                toolId: "lzfse_cli",
                name: "Apple LZFSE CLI",
                bundleIdentifier: nil,
                isInstalled: lzfseCli != nil,
                appPath: nil,
                cliExecutablePath: lzfseCli,
                iconSystemName: "applelogo",
                description: "Apple LZFSE compression reference CLI"
            )
        ]
    }

    private static func detectBenchmarkAndSpecializedCompetitors() -> [CompetitorTool] {
        let unarCli = findExecutable(names: ["unar"])
        let unarApp = checkAppInstalled(
            bundleId: "com.macpaw.site.theunarchiver",
            defaultPath: "/Applications/The Unarchiver.app"
        )
        let lrzipCli = findExecutable(names: ["lrzip"])
        let turboBenchCli = findExecutable(names: ["turbobench", "TurboBench"])
        let lzbenchCli = findExecutable(names: ["lzbench"])
        return [
            CompetitorTool(
                toolId: "unar_cli",
                name: "The Unarchiver (unar)",
                bundleIdentifier: "com.macpaw.site.theunarchiver",
                isInstalled: unarCli != nil,
                appPath: unarApp.path,
                cliExecutablePath: unarCli,
                iconSystemName: "doc.zipper",
                description: "Universal archive extraction CLI"
            ),
            CompetitorTool(
                toolId: "lrzip_cli",
                name: "Long Range ZIP (lrzip)",
                bundleIdentifier: nil,
                isInstalled: lrzipCli != nil,
                appPath: nil,
                cliExecutablePath: lrzipCli,
                iconSystemName: "square.stack.3d.down.right.fill",
                description: "Extended long-distance matching compression tool"
            ),
            CompetitorTool(
                toolId: "turbobench_cli",
                name: "TurboBench (powturbo)",
                bundleIdentifier: nil,
                isInstalled: turboBenchCli != nil,
                appPath: nil,
                cliExecutablePath: turboBenchCli,
                iconSystemName: "gauge.with.needle.fill",
                description: "In-memory compression benchmark suite"
            ),
            CompetitorTool(
                toolId: "lzbench_cli",
                name: "lzbench (inikep)",
                bundleIdentifier: nil,
                isInstalled: lzbenchCli != nil,
                appPath: nil,
                cliExecutablePath: lzbenchCli,
                iconSystemName: "speedometer",
                description: "In-memory real-time data compression benchmark tool"
            )
        ]
    }
    
    /// Filters detected tools returning only those present on host system.
    public static func detectOnlyInstalledCompetitors() -> [CompetitorTool] {
        return detectAllCompetitors().filter { $0.isInstalled }
    }
    
    // MARK: - Private Dynamic Probing Helpers
    
    public static func checkAppInstalled(bundleId: String, defaultPath: String) -> (installed: Bool, path: String?) {
        let fm = FileManager.default
        if fm.fileExists(atPath: defaultPath) {
            return (true, defaultPath)
        }
        
        #if canImport(AppKit)
        if let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleId) {
            return (true, url.path)
        }
        #endif
        
        return (false, nil)
    }
    
    public static func findExecutable(names: [String], extraPaths: [String] = []) -> String? {
        let fm = FileManager.default

        for p in extraPaths {
            if fm.isExecutableFile(atPath: p) { return p }
        }

        #if MAS_BUILD
        let searchDirs = ["/usr/bin", "/bin"]
        #else
        let pathEnv = ProcessInfo.processInfo.environment["PATH"] ?? "/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:/opt/homebrew/bin"
        var searchDirs: [String] = []
        for dir in pathEnv.split(separator: ":").map({ String($0) }) + ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"] {
            if !searchDirs.contains(dir) {
                searchDirs.append(dir)
            }
        }
        #endif

        for name in names {
            for dir in searchDirs {
                let fullPath = (dir as NSString).appendingPathComponent(name)
                if fm.isExecutableFile(atPath: fullPath) {
                    return fullPath
                }
            }
        }

        return nil
    }
}
