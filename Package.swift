// swift-tools-version: 6.0
// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import PackageDescription
import Foundation

let ttlogDependencyResolution: (dependency: Package.Dependency, packageName: String) = {
    // Tier 1: Explicit environment variable override
    if let envPath = ProcessInfo.processInfo.environment["TTLOG_PATH"],
       FileManager.default.fileExists(atPath: "\(envPath)/Package.swift") {
        return (.package(path: envPath), "TTLog")
    }

    // Tier 2: Standard peer workspace probe (e.g. ../../../infra/ttlog or ../../infra/ttlog)
    if ProcessInfo.processInfo.environment["TTZIP_USE_REMOTE_TTLOG"] != "1" {
        let candidates = ["../../../infra/ttlog", "../../infra/ttlog"]
        for relPath in candidates {
            let manifestURL = URL(fileURLWithPath: #filePath)
                .deletingLastPathComponent()
                .appendingPathComponent("\(relPath)/Package.swift")
                .standardized
            if FileManager.default.fileExists(atPath: manifestURL.path) {
                return (.package(path: relPath), "TTLog")
            }
        }
    }

    // Tier 3: Remote GitHub repository fallback for external machines and CI
    return (.package(url: "https://github.com/wittkung/ttlog.git", branch: "main"), "TTLog")
}()

let ttlogPackage = ttlogDependencyResolution.dependency
let ttlogPackageName = ttlogDependencyResolution.packageName

let coreSwiftSettings: [SwiftSetting] = [
    .enableUpcomingFeature("StrictConcurrency")
]

let package = Package(
    name: "TTZipCore",
    platforms: [
        .macOS(.v14),
        .iOS(.v17)
    ],
    products: [
        .library(
            name: "TTZipCore",
            type: .dynamic,
            targets: ["TTZipCore"]
        ),
        .library(
            name: "CTTZipBridge",
            targets: ["CTTZipBridge"]
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
        .target(
            name: "CTTZipBridge",
            dependencies: ["TTZipVendor"],
            path: "Sources/CTTZipBridge",
            publicHeadersPath: "include",
            cSettings: [
                .headerSearchPath("include")
            ],
            linkerSettings: [
                .linkedLibrary("archive"),
                .linkedLibrary("bz2"),
                .linkedLibrary("lzma"),
                .linkedLibrary("iconv"),
                .linkedLibrary("c++"),
                .linkedLibrary("compression"),
                .linkedFramework("Security")
            ]
        ),
        .target(
            name: "TTZipCore",
            dependencies: [
                "CTTZipBridge",
                "TTZipVendor",
                .product(name: "TTLogKit", package: ttlogPackageName)
            ],
            path: "Sources/TTZipCore",
            swiftSettings: coreSwiftSettings
        ),
        .executableTarget(
            name: "TTZipBench",
            dependencies: [
                "TTZipCore",
                "CTTZipBridge"
            ],
            path: "Sources/TTZipBench",
            swiftSettings: coreSwiftSettings
        ),
        .testTarget(
            name: "TTZipTests",
            dependencies: [
                "TTZipCore",
                "CTTZipBridge"
            ],
            path: "Tests/TTZipTests",
            resources: [
                .copy("Fixtures")
            ],
            swiftSettings: coreSwiftSettings
        ),
        .testTarget(
            name: "TTZipCoreTests",
            dependencies: [
                "TTZipCore"
            ],
            path: "Tests/TTZipCoreTests",
            swiftSettings: coreSwiftSettings
        )
    ]
)
