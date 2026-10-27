# Data Model & Manifest Specifications: Repository Topology

**Feature**: `216-repository-split-and-independent-release-pipeline`  
**Status**: `SPECIFIED`  

---

## 1. Repository A (`ttzip-core`) `Package.swift` Manifest Schema

```swift
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
        .library(
            name: "TTZipCore",
            targets: ["TTZipCore"]
        ),
        .library(
            name: "CTTZipBridge",
            targets: ["CTTZipBridge"]
        )
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
        .testTarget(
            name: "TTZipCoreTests",
            dependencies: ["TTZipCore"],
            path: "Tests/TTZipCoreTests"
        )
    ]
)
```

---

## 2. Repository B (`ttzip-apple`) `Package.swift` Manifest Schema

```swift
// swift-tools-version: 6.0
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>

import PackageDescription

let package = Package(
    name: "TTZipApp",
    platforms: [
        .macOS(.v14),
        .iOS(.v17)
    ],
    products: [
        .executable(name: "TTZipApp", targets: ["TTZipApp"]),
        .library(name: "TTZipQuickLook", type: .dynamic, targets: ["TTZipQuickLook"]),
        .library(name: "TTZipFinderSync", type: .dynamic, targets: ["TTZipFinderSync"])
    ],
    dependencies: [
        .package(url: "https://github.com/wittkung/ttzip-core.git", from: "1.0.0"),
        .package(url: "https://github.com/sparkle-project/Sparkle", exact: "2.6.0")
    ],
    targets: [
        .executableTarget(
            name: "TTZipApp",
            dependencies: [
                .product(name: "TTZipCore", package: "ttzip-core"),
                .product(name: "Sparkle", package: "Sparkle")
            ],
            path: "Sources/TTZipApp"
        ),
        .target(
            name: "TTZipQuickLook",
            dependencies: [
                .product(name: "TTZipCore", package: "ttzip-core")
            ],
            path: "Sources/TTZipQuickLook"
        ),
        .target(
            name: "TTZipFinderSync",
            dependencies: [
                .product(name: "TTZipCore", package: "ttzip-core")
            ],
            path: "Sources/TTZipFinderSync"
        ),
        .testTarget(
            name: "TTZipAppTests",
            dependencies: ["TTZipApp"],
            path: "Tests/TTZipAppTests"
        )
    ]
)
```
