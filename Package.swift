// swift-tools-version: 6.0
// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.

import PackageDescription

let package = Package(
    name: "TTZipCore",
    platforms: [
        .macOS(.v14),
        .iOS(.v17),
        .visionOS(.v1)
    ],
    products: [
        .library(name: "TTZipCore", targets: ["TTZipCore"]),
        .library(name: "CTTZipBridge", targets: ["CTTZipBridge"]),
        .executable(name: "ttzip-bench", targets: ["ttzip-bench"])
    ],
    dependencies: [],
    targets: [
        .binaryTarget(
            name: "TTZipVendor",
            path: "Vendor/TTZipVendor.xcframework"
        ),
        .target(
            name: "CTTZipBridge",
            dependencies: ["TTZipVendor"],
            path: "Sources/CTTZipBridge",
            publicHeadersPath: "include",
            cSettings: [
                .headerSearchPath("include"),
                .unsafeFlags(["-O3", "-fno-strict-aliasing"])
            ],
            linkerSettings: [
                .linkedLibrary("bz2"),
                .linkedLibrary("iconv"),
                .linkedLibrary("c++"),
                .linkedLibrary("compression"),
                .linkedFramework("Security")
            ]
        ),
        .target(
            name: "TTZipCore",
            dependencies: ["CTTZipBridge", "TTZipVendor"],
            path: "Sources/TTZipCore",
            swiftSettings: [
                .enableExperimentalFeature("StrictConcurrency")
            ]
        ),
        .executableTarget(
            name: "ttzip-bench",
            dependencies: ["TTZipCore", "CTTZipBridge"],
            path: "Sources/TTZipBench"
        ),
        .testTarget(
            name: "TTZipTests",
            dependencies: ["TTZipCore", "CTTZipBridge"],
            path: "Tests/TTZipTests",
            resources: [
                .copy("Fixtures")
            ]
        )
    ]
)
