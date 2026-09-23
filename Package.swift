// swift-tools-version: 6.0
// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

import PackageDescription
import Foundation

let ttlogPackage: Package.Dependency = .package(url: "https://github.com/wittkung/ttlog.git", exact: "0.1.0")
let ttlogPackageName = "TTLog"

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
                .linkedLibrary("iconv", .when(platforms: [.macOS, .iOS])),
                .linkedLibrary("c++", .when(platforms: [.macOS, .iOS])),
                .linkedLibrary("compression", .when(platforms: [.macOS, .iOS])),
                .linkedFramework("Security", .when(platforms: [.macOS, .iOS])),
                .unsafeFlags(["-Xlinker", "-w"])
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
    ],
    swiftLanguageModes: [.v6]
)
