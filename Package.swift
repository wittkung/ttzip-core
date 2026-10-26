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
            name: "TTZipCore",
            dependencies: [
                "TTZipVendor",
                .product(name: "TTLogKit", package: ttlogPackageName)
            ],
            path: "Sources/TTZipCore",
            swiftSettings: coreSwiftSettings,
            linkerSettings: [
                .linkedLibrary("archive"),
                .linkedLibrary("bz2"),
                .linkedLibrary("lzma"),
                .linkedLibrary("iconv", .when(platforms: [.macOS, .iOS])),
                .linkedLibrary("c++", .when(platforms: [.macOS, .iOS])),
                .linkedLibrary("compression", .when(platforms: [.macOS, .iOS])),
                .linkedFramework("Security", .when(platforms: [.macOS, .iOS]))
            ]
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
