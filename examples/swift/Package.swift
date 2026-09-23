// swift-tools-version: 6.0
// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

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
