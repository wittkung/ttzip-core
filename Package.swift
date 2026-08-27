// swift-tools-version: 6.0
// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import PackageDescription

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
    dependencies: [],
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
                "TTZipVendor"
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
        )
    ]
)
