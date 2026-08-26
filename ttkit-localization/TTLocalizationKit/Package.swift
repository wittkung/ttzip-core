// swift-tools-version: 6.0
// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTKit: High-performance cross-platform system utility foundation.

import PackageDescription

let package = Package(
    name: "TTLocalizationKit",
    platforms: [
        .macOS(.v14),
        .iOS(.v17)
    ],
    products: [
        .library(
            name: "TTLocalizationKit",
            targets: ["TTLocalizationCore", "TTLocalizationUI", "TTLocalizationAppKit", "TTLocalizationIPC"]
        ),
    ],
    targets: [
        .target(
            name: "TTLocalizationCore",
            swiftSettings: [
                .enableUpcomingFeature("StrictConcurrency")
            ]
        ),
        .target(
            name: "TTLocalizationUI",
            dependencies: ["TTLocalizationCore", "TTLocalizationIPC"],
            swiftSettings: [
                .enableUpcomingFeature("StrictConcurrency")
            ]
        ),
        .target(
            name: "TTLocalizationAppKit",
            dependencies: ["TTLocalizationCore"],
            swiftSettings: [
                .enableUpcomingFeature("StrictConcurrency")
            ]
        ),
        .target(
            name: "TTLocalizationIPC",
            dependencies: ["TTLocalizationCore"],
            swiftSettings: [
                .enableUpcomingFeature("StrictConcurrency")
            ]
        ),
        .testTarget(
            name: "TTLocalizationKitTests",
            dependencies: ["TTLocalizationCore", "TTLocalizationUI", "TTLocalizationAppKit", "TTLocalizationIPC"]
        ),
    ]
)
