// swift-tools-version: 6.0
// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

import PackageDescription
import Foundation

let ttlogPackage: Package.Dependency = .package(url: "https://github.com/wittkung/ttlog.git", exact: "1.0.1")
let ttlogPackageName = "ttlog"

let coreSwiftSettings: [SwiftSetting] = [
    .enableUpcomingFeature("StrictConcurrency")
]

let archiveLinkerSettings: [LinkerSetting] = [
    .linkedLibrary("archive"),
    .linkedLibrary("bz2"),
    .linkedLibrary("lzma"),
    .linkedLibrary("iconv", .when(platforms: [.macOS, .iOS])),
    .linkedLibrary("c++", .when(platforms: [.macOS, .iOS])),
    .linkedLibrary("compression", .when(platforms: [.macOS, .iOS])),
    .linkedFramework("Security", .when(platforms: [.macOS, .iOS]))
]

let coreSourcesPath = "Sources/TTZipCore"

let typesSources = [
    "ArchiveProtocols.swift",
    "Features/Charset/TTZipTextEncodingService.swift",
    "TTZipImageModels.swift",
    "Localization",
    "Logging",
    "Platform",
    "Standards",
    "Types",
]

let cryptoSources = [
    "PasswordRecoveryEngine.swift",
    "PasswordVaultArchiveCredentials.swift",
    "PasswordVaultEntries.swift",
    "PasswordVaultManager+BiometricUnlock.swift",
    "PasswordVaultManager+VaultReset.swift",
    "PasswordVaultManager.swift",
    "PasswordVaultModels.swift",
    "TTZipPluginAuthService.swift",
    "Crypto",
    "Features/CryptoVault",
    "Security",
]

let archiveSources = [
    "ArchiveExtractor.swift",
    "ArchiveReader.swift",
    "ArchiveTreeNode+Builder.swift",
    "ArchiveTreeNode+Composite.swift",
    "ArchiveTreeNode+Search.swift",
    "ArchiveTreeNode.swift",
    "ArchiveWriter.swift",
    "HashCalculator.swift",
    "TTZipMmapStreamService.swift",
    "UniFfiMmapReader+ZeroCopy.swift",
    "Bridge",
    "Codecs",
    "Commands",
    "Concurrency",
    "Pipeline",
    "Split",
    "Tree",
    "VFS/VFSLz4CachePool.swift",
]

let facadeSources = [
    "CompetitorDetector.swift",
    "Facades",
    "FinderSyncHelper.swift",
    "PresetManager.swift",
    "QuickLook",
    "Services",
    "SystemServices.swift",
    "TTZipAppUpdateService.swift",
    "TTZipCoreExports.swift",
    "TTZipDocumentMetadataService.swift",
    "TTZipEbookReaderService.swift",
    "TTZipEngine.swift",
    "TTZipImageRenderingService.swift",
    "ToolchainInstaller.swift",
    "VFS/TTZipVfsSchemeHandler.swift",
]

let package = Package(
    name: "TTZipCore",
    platforms: [
        .macOS(.v14),
        .iOS(.v17)
    ],
    products: [
        .library(
            name: "TTZipArchiveContract",
            targets: ["TTZipArchiveContract"]
        ),
        .library(
            name: "TTZipOperationContract",
            targets: ["TTZipOperationContract"]
        ),
        .library(
            name: "TTZipArchiveRuntime",
            targets: ["TTZipCoreArchive"]
        ),
        .library(
            name: "TTZipCore",
            type: .dynamic,
            targets: ["TTZipCore"]
        ),
        .library(
            name: "TTZipEpub",
            type: .dynamic,
            targets: ["TTZipEpub"]
        ),
        .library(
            name: "TTZipArchiveWebKit",
            targets: ["TTZipWebKit"]
        ),
        .executable(
            name: "ttzip-bench",
            targets: ["TTZipBench"]
        )
    ],
    dependencies: [
        ttlogPackage
    ],
    targets: [
        .binaryTarget(
            name: "TTZipVendor",
            path: "Frameworks/TTZipVendor.xcframework"
        ),
        .binaryTarget(
            name: "TTZipEpubVendor",
            path: "Frameworks/TTZipEpubVendor.xcframework"
        ),
        .target(
            name: "CTTZipEpubBridge",
            dependencies: ["TTZipEpubVendor"],
            path: "Sources/CTTZipEpubBridge",
            linkerSettings: archiveLinkerSettings
        ),
        .target(
            name: "TTZipEpub",
            dependencies: ["CTTZipEpubBridge"],
            path: "Sources/TTZipEpub",
            swiftSettings: coreSwiftSettings
        ),
        .target(
            name: "TTZipArchiveContract",
            dependencies: [],
            path: "contracts/archive/Sources/TTZipArchiveContract",
            swiftSettings: coreSwiftSettings
        ),
        .target(
            name: "TTZipOperationContract",
            dependencies: [
                "TTZipArchiveContract"
            ],
            path: "contracts/operation/Sources/TTZipOperationContract",
            swiftSettings: coreSwiftSettings
        ),
        .target(
            name: "TTZipCoreBindings",
            dependencies: ["TTZipVendor"],
            path: "Sources/TTZipCore/Generated",
            exclude: ["ttzip_engineFFI.h", "ttzip_engineFFI.modulemap"],
            swiftSettings: coreSwiftSettings,
            linkerSettings: archiveLinkerSettings
        ),
        .target(
            name: "TTZipWebKit",
            path: "Sources/TTZipWebKit",
            swiftSettings: coreSwiftSettings
        ),
        .target(
            name: "TTZipCoreTypes",
            dependencies: [
                "TTZipArchiveContract",
                "TTZipOperationContract",
                "TTZipCoreBindings",
                .product(name: "TTLogKit", package: ttlogPackageName)
            ],
            path: coreSourcesPath,
            exclude: ["Generated"] + cryptoSources + archiveSources + facadeSources,
            sources: typesSources,
            swiftSettings: coreSwiftSettings
        ),
        .target(
            name: "TTZipCoreCrypto",
            dependencies: [
                "TTZipCoreBindings",
                "TTZipCoreTypes",
                .product(name: "TTLogKit", package: ttlogPackageName)
            ],
            path: coreSourcesPath,
            exclude: ["Generated"] + typesSources + archiveSources + facadeSources,
            sources: cryptoSources,
            swiftSettings: coreSwiftSettings
        ),
        .target(
            name: "TTZipCoreArchive",
            dependencies: [
                "TTZipArchiveContract",
                "TTZipCoreBindings",
                "TTZipCoreCrypto",
                "TTZipCoreTypes",
                .product(name: "TTLogKit", package: ttlogPackageName)
            ],
            path: coreSourcesPath,
            exclude: ["Generated"] + typesSources + cryptoSources + facadeSources,
            sources: archiveSources,
            swiftSettings: coreSwiftSettings
        ),
        .target(
            name: "TTZipCore",
            dependencies: [
                "TTZipArchiveContract",
                "TTZipOperationContract",
                "TTZipCoreArchive",
                "TTZipCoreBindings",
                "TTZipCoreCrypto",
                "TTZipCoreTypes",
                "TTZipWebKit",
                .product(name: "TTLogKit", package: ttlogPackageName)
            ],
            path: coreSourcesPath,
            exclude: ["Generated"] + typesSources + cryptoSources + archiveSources,
            sources: facadeSources,
            swiftSettings: coreSwiftSettings
        ),
        .executableTarget(
            name: "TTZipBench",
            dependencies: [
                "TTZipCore"
            ],
            path: "Sources/TTZipBench",
            swiftSettings: coreSwiftSettings
        ),
        .testTarget(
            name: "TTZipArchiveContractTests",
            dependencies: [
                "TTZipArchiveContract"
            ],
            path: "contracts/archive/Tests/TTZipArchiveContractTests",
            swiftSettings: coreSwiftSettings
        ),
        .testTarget(
            name: "TTZipOperationContractTests",
            dependencies: [
                "TTZipOperationContract",
                "TTZipArchiveContract"
            ],
            path: "contracts/operation/Tests/TTZipOperationContractTests",
            swiftSettings: coreSwiftSettings
        ),
        .testTarget(
            name: "TTZipTests",
            dependencies: [
                "TTZipCore"
            ],
            path: "Tests/TTZipTests",
            resources: [
                .copy("Fixtures")
            ],
            swiftSettings: coreSwiftSettings
        )
    ],
    swiftLanguageModes: [.v6]
)
