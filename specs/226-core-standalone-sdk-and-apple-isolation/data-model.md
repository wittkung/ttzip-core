# Data Model & Distribution Manifests: TTZipCore SDK

**Feature**: `226-core-standalone-sdk-and-apple-isolation`  
**Date**: 2026-08-26  
**Status**: COMPLETE

---

## 1. Binary Artifact Structure

```
TTZipVendor.xcframework/
├── Info.plist
└── macos-arm64_x86_64/
    ├── Headers/
    │   ├── CTTZipBridge.h
    │   └── ttzip_engineFFI.h
    └── libTTZipVendor.a (Universal Static Mach-O Archive: arm64 + x86_64)
```

### Manifest Metadata (Info.plist)
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>AvailableLibraries</key>
    <array>
        <dict>
            <key>HeadersPath</key>
            <string>Headers</string>
            <key>LibraryIdentifier</key>
            <string>macos-arm64_x86_64</string>
            <key>LibraryPath</key>
            <string>libTTZipVendor.a</string>
            <key>SupportedArchitectures</key>
            <array>
                <string>arm64</string>
                <string>x86_64</string>
            </array>
            <key>SupportedPlatform</key>
            <string>macos</string>
        </dict>
    </array>
    <key>CFBundlePackageType</key>
    <string>XFWK</string>
    <key>XCFrameworkFormatVersion</key>
    <string>1.0</string>
</dict>
</plist>
```

---

## 2. Swift Package Manifest Data Contracts

### `core/Package.swift` Contract
```swift
// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "TTZipCore",
    platforms: [.macOS(.v14), .iOS(.v17)],
    products: [
        .library(name: "TTZipCore", targets: ["TTZipCore"]),
        .library(name: "CTTZipBridge", targets: ["CTTZipBridge"])
    ],
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
            linkerSettings: [
                .linkedLibrary("archive"),
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
            swiftSettings: [.enableUpcomingFeature("StrictConcurrency")]
        )
    ]
)
```

### `apple/Package.swift` Contract
```swift
// swift-tools-version: 6.0
import PackageDescription
import Foundation

let isLocalCoreAvailable: Bool = {
    if ProcessInfo.processInfo.environment["TTZIP_USE_REMOTE_CORE"] == "1" { return false }
    let localManifest = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .appendingPathComponent("../core/Package.swift")
        .standardized
    return FileManager.default.fileExists(atPath: localManifest.path)
}()

let coreDependency: Package.Dependency = isLocalCoreAvailable
    ? .package(path: "../core")
    : .package(url: "https://github.com/wittkung/ttzip-core.git", branch: "main")

let corePackageName = isLocalCoreAvailable ? "core" : "ttzip-core"

let package = Package(
    name: "TTZipApp",
    platforms: [.macOS(.v14), .iOS(.v17)],
    products: [
        .executable(name: "TTZipApp", targets: ["TTZipApp"]),
        .library(name: "TTZipPluginKit", targets: ["TTZipPluginKit"]),
        .library(name: "TTZipQuickLook", type: .dynamic, targets: ["TTZipQuickLook"]),
        .library(name: "TTZipFinderSync", type: .dynamic, targets: ["TTZipFinderSync"])
    ],
    dependencies: [
        coreDependency,
        .package(url: "https://github.com/sparkle-project/Sparkle", exact: "2.6.0")
    ],
    targets: [
        .target(name: "TTZipPluginKit", path: "Sources/TTZipPluginKit"),
        .executableTarget(
            name: "TTZipApp",
            dependencies: [
                .product(name: "TTZipCore", package: corePackageName),
                .product(name: "Sparkle", package: "Sparkle"),
                "TTZipPluginKit"
            ],
            path: "Sources/TTZipApp"
        )
    ]
)
```
