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
            name: "TTZipCore",
            type: .dynamic,
            targets: ["TTZipCore"]
        ),
        .library(
            name: "TTZipEpub",
            type: .dynamic,
            targets: ["TTZipEpub"]
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
            swiftSettings: coreSwiftSettings
        ),
        .target(
            name: "TTZipCore",
            dependencies: [
                "TTZipArchiveContract",
                "TTZipOperationContract",
                "TTZipCoreBindings",
                "TTZipVendor",
                .product(name: "TTLogKit", package: ttlogPackageName)
            ],
            path: "Sources/TTZipCore",
            exclude: ["Generated"],
            swiftSettings: coreSwiftSettings,
            linkerSettings: archiveLinkerSettings
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
