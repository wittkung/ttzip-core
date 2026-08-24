// swift-tools-version: 6.0
// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.

import PackageDescription

let package = Package(
    name: "TTZipSwiftExample",
    platforms: [
        .macOS(.v14)
    ],
    dependencies: [
        .package(name: "TTZip", path: "../../")
    ],
    targets: [
        .executableTarget(
            name: "TTZipSwiftExample",
            dependencies: [
                .product(name: "TTZipCore", package: "TTZip")
            ],
            path: "Sources"
        )
    ]
)
