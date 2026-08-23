#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.
# Automated Physical Repository Splitter for ttzip-core and ttzip-apple.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
PARENT_DIR="$(cd "${REPO_ROOT}/.." && pwd)"

CORE_DIR="${PARENT_DIR}/ttzip-core"
APPLE_DIR="${PARENT_DIR}/ttzip-apple"

echo "======================================================================"
echo "⚡️ TTZip Deterministic Two-Repository Physical Splitter"
echo "======================================================================"
echo "Source Monorepo:  ${REPO_ROOT}"
echo "Target Core Repo: ${CORE_DIR}"
echo "Target App Repo:  ${APPLE_DIR}"
echo ""

# -----------------------------------------------------------------------------
# 1. Prepare Repository A: ttzip-core
# -----------------------------------------------------------------------------
echo "[1/4] Preparing ttzip-core (BSD-3-Clause OR Apache-2.0)..."
rm -rf "${CORE_DIR}"
mkdir -p "${CORE_DIR}"

# Write .gitignore for ttzip-core
cat << 'EOF' > "${CORE_DIR}/.gitignore"
.DS_Store
.build/
target/
*.xcuserstate
*.xcworkspace
xcuserdata/
*.swp
*.orig
EOF

# Copy Core directories and files excluding target/ and .build/
mkdir -p "${CORE_DIR}/rust"
rsync -a --exclude 'target' --exclude '.build' "${REPO_ROOT}/rust/" "${CORE_DIR}/rust/"

mkdir -p "${CORE_DIR}/Sources"
rsync -a "${REPO_ROOT}/Sources/CTTZipBridge" "${CORE_DIR}/Sources/"
rsync -a "${REPO_ROOT}/Sources/TTZipCore" "${CORE_DIR}/Sources/"
if [ -d "${REPO_ROOT}/Sources/TTZipBench" ]; then
    rsync -a "${REPO_ROOT}/Sources/TTZipBench" "${CORE_DIR}/Sources/"
fi

mkdir -p "${CORE_DIR}/Tests"
if [ -d "${REPO_ROOT}/Tests/TTZipTests" ]; then
    rsync -a "${REPO_ROOT}/Tests/TTZipTests" "${CORE_DIR}/Tests/"
fi
if [ -d "${REPO_ROOT}/Tests/fixtures" ]; then
    rsync -a "${REPO_ROOT}/Tests/fixtures" "${CORE_DIR}/Tests/"
fi

mkdir -p "${CORE_DIR}/Vendor"
rsync -a "${REPO_ROOT}/Vendor/TTZipVendor.xcframework" "${CORE_DIR}/Vendor/"

mkdir -p "${CORE_DIR}/scripts"
cp "${REPO_ROOT}/scripts/build_rust.sh" "${CORE_DIR}/scripts/"
cp "${REPO_ROOT}/scripts/install_local_git_hooks.sh" "${CORE_DIR}/scripts/"
cp "${REPO_ROOT}/scripts/lint_loc_gate.sh" "${CORE_DIR}/scripts/"
cp "${REPO_ROOT}/scripts/lint_loc_gate.py" "${CORE_DIR}/scripts/"
cp "${REPO_ROOT}/scripts/run_local_ci_gate.sh" "${CORE_DIR}/scripts/"
cp "${REPO_ROOT}/scripts/run_rust_tests.sh" "${CORE_DIR}/scripts/"
cp "${REPO_ROOT}/scripts/audit_licenses.py" "${CORE_DIR}/scripts/"
cp "${REPO_ROOT}/scripts/generate_acknowledgements.py" "${CORE_DIR}/scripts/"

cp "${REPO_ROOT}/LICENSE-BSD" "${CORE_DIR}/"
cp "${REPO_ROOT}/LICENSE-APACHE" "${CORE_DIR}/"
cp "${REPO_ROOT}/NOTICE" "${CORE_DIR}/"
cp "${REPO_ROOT}/ACKNOWLEDGEMENTS.md" "${CORE_DIR}/"
mkdir -p "${CORE_DIR}/docs"
if [ -f "${REPO_ROOT}/docs/THIRD_PARTY_LICENSES.md" ]; then
    cp "${REPO_ROOT}/docs/THIRD_PARTY_LICENSES.md" "${CORE_DIR}/docs/"
fi

# Write standalone Core Package.swift
cat << 'EOF' > "${CORE_DIR}/Package.swift"
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
EOF

# Initialize git in ttzip-core
(
    cd "${CORE_DIR}"
    git init -b main >/dev/null 2>&1 || git init >/dev/null 2>&1
    git config user.name "Witt Kung"
    git config user.email "witt.w.kung@gmail.com"
    git add .
    git commit -m "feat(core): initialize autonomous ttzip-core open-source engine & SDK" >/dev/null 2>&1 || true
    chmod +x scripts/*.sh
    ./scripts/install_local_git_hooks.sh >/dev/null 2>&1 || true
)
echo "✅ ttzip-core initialized successfully."

# -----------------------------------------------------------------------------
# 2. Prepare Repository B: ttzip-apple
# -----------------------------------------------------------------------------
echo "[2/4] Preparing ttzip-apple (GPL-3.0-or-later)..."
rm -rf "${APPLE_DIR}"
mkdir -p "${APPLE_DIR}"

# Write .gitignore for ttzip-apple
cat << 'EOF' > "${APPLE_DIR}/.gitignore"
.DS_Store
.build/
*.xcuserstate
*.xcworkspace
xcuserdata/
*.swp
*.orig
EOF

mkdir -p "${APPLE_DIR}/Sources"
rsync -a "${REPO_ROOT}/Sources/TTZipApp" "${APPLE_DIR}/Sources/"
rsync -a "${REPO_ROOT}/Sources/TTZipQuickLook" "${APPLE_DIR}/Sources/"
rsync -a "${REPO_ROOT}/Sources/TTZipFinderSync" "${APPLE_DIR}/Sources/"

mkdir -p "${APPLE_DIR}/Tests"
if [ -d "${REPO_ROOT}/Tests/TTZipAppTests" ]; then
    rsync -a "${REPO_ROOT}/Tests/TTZipAppTests" "${APPLE_DIR}/Tests/"
fi
if [ -d "${REPO_ROOT}/Tests/fixtures" ]; then
    rsync -a "${REPO_ROOT}/Tests/fixtures" "${APPLE_DIR}/Tests/"
fi

if [ -d "${REPO_ROOT}/Resources" ]; then
    rsync -a "${REPO_ROOT}/Resources/" "${APPLE_DIR}/Resources/"
fi

mkdir -p "${APPLE_DIR}/scripts"
cp "${REPO_ROOT}/scripts/install_local_git_hooks.sh" "${APPLE_DIR}/scripts/"
cp "${REPO_ROOT}/scripts/lint_loc_gate.sh" "${APPLE_DIR}/scripts/"
cp "${REPO_ROOT}/scripts/lint_loc_gate.py" "${APPLE_DIR}/scripts/"

cp "${REPO_ROOT}/LICENSE-GPL" "${APPLE_DIR}/LICENSE"
cp "${REPO_ROOT}/NOTICE" "${APPLE_DIR}/"

# Write standalone Apple Client Package.swift (referencing local core)
cat << 'EOF' > "${APPLE_DIR}/Package.swift"
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
        .package(path: "../ttzip-core"),
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
EOF

# Initialize git in ttzip-apple
(
    cd "${APPLE_DIR}"
    git init -b main >/dev/null 2>&1 || git init >/dev/null 2>&1
    git config user.name "Witt Kung"
    git config user.email "witt.w.kung@gmail.com"
    git add .
    git commit -m "feat(apple): initialize autonomous ttzip-apple client application" >/dev/null 2>&1 || true
    chmod +x scripts/*.sh
    ./scripts/install_local_git_hooks.sh >/dev/null 2>&1 || true
)
echo "✅ ttzip-apple initialized successfully."

echo ""
echo "======================================================================"
echo "🎉 Physical repository split completed!"
echo "   - ttzip-core:  ${CORE_DIR}"
echo "   - ttzip-apple: ${APPLE_DIR}"
echo "======================================================================"
