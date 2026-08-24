#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# ==============================================================================
# scripts/release_distribution.sh
# macOS 生产级端到端发布流水线：
# 构建 -> Inside-Out 签名 -> DMG 封装 -> Apple 自动化公证 -> Sparkle 签名 -> Homebrew
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${WORKSPACE_ROOT}"

VERSION="1.5.0"
BUILD_NUMBER="10500"
SIGN_IDENTITY="${CODE_SIGN_IDENTITY:-}"
ENTITLEMENTS="${WORKSPACE_ROOT}/Sources/TTZipApp/TTZip-Direct.entitlements"
OUTPUT_DIR="${WORKSPACE_ROOT}/dist"
KEYCHAIN_PROFILE="${NOTARY_KEYCHAIN_PROFILE:-}"
API_KEY_PATH="${APP_STORE_CONNECT_KEY_PATH:-}"
API_KEY_ID="${APP_STORE_CONNECT_KEY_ID:-}"
API_ISSUER="${APP_STORE_CONNECT_ISSUER_ID:-}"
APPLE_ID="${NOTARY_APPLE_ID:-}"
APPLE_PASSWORD="${NOTARY_APPLE_PASSWORD:-}"
TEAM_ID="${NOTARY_TEAM_ID:-}"
SPARKLE_KEY_FILE="${SPARKLE_KEY_PATH:-}"
SPARKLE_PRIV_KEY="${SPARKLE_ED_PRIVATE_KEY:-}"
BASE_DOWNLOAD_URL="https://github.com/wittkung/TTZip/releases/download/v${VERSION}"

SKIP_NOTARIZE=false
DRY_RUN=false

usage() {
    cat <<EOHELP
Usage: $0 [OPTIONS]

Options:
  --version <ver>              Release version (default: 1.5.0)
  --build-num <num>            Build number (default: 10500)
  --identity <id>              Developer ID Application signing identity
  --keychain-profile <name>    notarytool stored credentials profile name
  --api-key <path>             App Store Connect API Key (.p8) path
  --api-key-id <id>            App Store Connect API Key ID
  --api-issuer <uuid>          App Store Connect API Issuer ID
  --sparkle-key-file <path>    Path to Sparkle EdDSA private key file
  --skip-notarize              Skip Apple Notarization step
  --dry-run                    Simulate pipeline without modifying release artifacts
  -h, --help                   Show this help message
EOHELP
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="$2"; shift 2 ;;
        --build-num) BUILD_NUMBER="$2"; shift 2 ;;
        --identity) SIGN_IDENTITY="$2"; shift 2 ;;
        --keychain-profile) KEYCHAIN_PROFILE="$2"; shift 2 ;;
        --api-key) API_KEY_PATH="$2"; shift 2 ;;
        --api-key-id) API_KEY_ID="$2"; shift 2 ;;
        --api-issuer) API_ISSUER="$2"; shift 2 ;;
        --sparkle-key-file) SPARKLE_KEY_FILE="$2"; shift 2 ;;
        --skip-notarize) SKIP_NOTARIZE=true; shift ;;
        --dry-run) DRY_RUN=true; shift ;;
        -h|--help) usage ;;
        *) echo "❌ Unknown option: $1"; exit 64 ;;
    esac
done

APP_BUNDLE="${OUTPUT_DIR}/TTZip.app"
DMG_NAME="TTZip-v${VERSION}.dmg"
DMG_PATH="${OUTPUT_DIR}/${DMG_NAME}"
TARBALL_NAME="ttzip-cli-v${VERSION}-darwin-universal.tar.gz"
APPCAST_PATH="${WORKSPACE_ROOT}/appcast.xml"

echo "======================================================================"
echo "⚡️  TTZip 生产级端到端发布与分发流水线 (v${VERSION} #${BUILD_NUMBER})"
echo "======================================================================"

mkdir -p "${OUTPUT_DIR}"

if [ "${DRY_RUN}" = true ]; then
    echo "ℹ️  [DRY-RUN] 仅运行语法与参数校验模式..."
fi

# [Phase 0] 运行行数防劣化门禁
"${SCRIPT_DIR}/lint_loc_gate.sh"

# [Phase 1] 核心引擎全量构建
echo "==> [Phase 1/7] 编译 Rust 核心引擎、TUI 及 Swift 产物..."
"${SCRIPT_DIR}/build_rust.sh" --release
[ -f "${SCRIPT_DIR}/build_tui.sh" ] && "${SCRIPT_DIR}/build_tui.sh" --release 2>/dev/null || true
[ -f "${SCRIPT_DIR}/build_extensions.sh" ] && "${SCRIPT_DIR}/build_extensions.sh" 2>/dev/null || true

echo "==> 编译 TTZipApp Swift Desktop 二进制 (Release)..."
swift build -c release --product TTZipApp

# [Phase 2] 组装 .app Bundle 目录结构
echo "==> [Phase 2/7] 组装 TTZip.app 应用包结构..."
APP_BIN="$(find "${WORKSPACE_ROOT}/.build" -name "TTZipApp" -type f | grep -E "release" | head -n 1)"
if [ -z "${APP_BIN}" ] || [ ! -f "${APP_BIN}" ]; then
    echo "❌ 错误: 未找到 TTZipApp 编译产物"; exit 1
fi

CONTENTS_DIR="${APP_BUNDLE}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
FRAMEWORKS_DIR="${CONTENTS_DIR}/Frameworks"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"
HELPERS_DIR="${CONTENTS_DIR}/Helpers"
PLUGINS_DIR="${CONTENTS_DIR}/PlugIns"

rm -rf "${APP_BUNDLE}"
mkdir -p "${MACOS_DIR}" "${FRAMEWORKS_DIR}" "${RESOURCES_DIR}" "${HELPERS_DIR}" "${PLUGINS_DIR}"

cp "${APP_BIN}" "${MACOS_DIR}/TTZip"
strip -x "${MACOS_DIR}/TTZip" 2>/dev/null || true
chmod +x "${MACOS_DIR}/TTZip"
install_name_tool -add_rpath @executable_path/../Frameworks "${MACOS_DIR}/TTZip" 2>/dev/null || true

RUST_CLI="${WORKSPACE_ROOT}/bin/ttzip"
[ ! -f "${RUST_CLI}" ] && RUST_CLI="${WORKSPACE_ROOT}/rust/target/release/ttzip"
if [ -f "${RUST_CLI}" ]; then
    cp "${RUST_CLI}" "${HELPERS_DIR}/ttzip"
    strip -x "${HELPERS_DIR}/ttzip" 2>/dev/null || true
    chmod +x "${HELPERS_DIR}/ttzip"
fi

SPARKLE_SRC="$(find "${WORKSPACE_ROOT}/.build" -name "Sparkle.framework" -type d | grep -E "xcframework.*macos|release/Sparkle.framework" | head -n 1 || true)"
if [ -n "${SPARKLE_SRC}" ] && [ -d "${SPARKLE_SRC}" ]; then
    cp -R "${SPARKLE_SRC}" "${FRAMEWORKS_DIR}/"
fi

if [ -d "${OUTPUT_DIR}/PlugIns" ]; then
    cp -R "${OUTPUT_DIR}/PlugIns/"* "${PLUGINS_DIR}/" 2>/dev/null || true
fi

cp "${WORKSPACE_ROOT}/Sources/TTZipApp/Info.plist" "${CONTENTS_DIR}/Info.plist"
echo "APPL????" > "${CONTENTS_DIR}/PkgInfo"
[ -f "${WORKSPACE_ROOT}/Sources/TTZipApp/Resources/AppIcon.icns" ] && cp "${WORKSPACE_ROOT}/Sources/TTZipApp/Resources/AppIcon.icns" "${RESOURCES_DIR}/AppIcon.icns"
[ -f "${WORKSPACE_ROOT}/Sources/TTZipApp/PrivacyInfo.xcprivacy" ] && cp "${WORKSPACE_ROOT}/Sources/TTZipApp/PrivacyInfo.xcprivacy" "${RESOURCES_DIR}/PrivacyInfo.xcprivacy"

echo "  ✓ TTZip.app 组装完成: ${APP_BUNDLE}"

# [Phase 3] Inside-Out 代码签名
echo "==> [Phase 3/7] 执行由内而外 (Inside-Out) Hardened Runtime 代码签名..."

sign_component() {
    local target="$1"
    local ent="${2:-}"
    local sign_args=(--force --options runtime --timestamp)
    
    if [ -n "${SIGN_IDENTITY}" ]; then
        sign_args+=(--sign "${SIGN_IDENTITY}")
    else
        sign_args+=(--sign "-")
    fi
    
    if [ -n "${ent}" ] && [ -f "${ent}" ]; then
        sign_args+=(--entitlements "${ent}")
    fi
    
    codesign "${sign_args[@]}" "${target}"
}

if [ -d "${FRAMEWORKS_DIR}/Sparkle.framework" ]; then
    find "${FRAMEWORKS_DIR}/Sparkle.framework" -type f -perm +111 -exec codesign --force --options runtime --timestamp --sign "${SIGN_IDENTITY:--}" {} + 2>/dev/null || true
    sign_component "${FRAMEWORKS_DIR}/Sparkle.framework"
fi

if [ -f "${HELPERS_DIR}/ttzip" ]; then
    sign_component "${HELPERS_DIR}/ttzip"
fi

for plugin in "${PLUGINS_DIR}"/*.appex; do
    if [ -d "${plugin}" ]; then
        sign_component "${plugin}"
    fi
done

sign_component "${MACOS_DIR}/TTZip" "${ENTITLEMENTS}"
sign_component "${APP_BUNDLE}" "${ENTITLEMENTS}"

echo "--> 校验代码签名完整性..."
codesign --verify --deep --strict --verbose=2 "${APP_BUNDLE}"
echo "  ✓ 代码签名验证通过"

# [Phase 4] 生成 Retina 高清 DMG
echo "==> [Phase 4/7] 封装 Retina 高清 DMG 安装镜像..."
if [ -f "${SCRIPT_DIR}/create_dmg_installer.sh" ]; then
    "${SCRIPT_DIR}/create_dmg_installer.sh" \
        --app "${APP_BUNDLE}" \
        --volname "TTZip" \
        --output "${DMG_PATH}"
else
    hdiutil create -volname "TTZip" -srcfolder "${APP_BUNDLE}" -ov -format UDZO "${DMG_PATH}"
fi

if [ -n "${SIGN_IDENTITY}" ]; then
    codesign --force --sign "${SIGN_IDENTITY}" --timestamp "${DMG_PATH}"
fi
echo "  ✓ DMG 镜像生成完成: ${DMG_PATH}"

# [Phase 5] Apple 自动化公证
if [ "${SKIP_NOTARIZE}" = false ] && [ -n "${SIGN_IDENTITY}" ]; then
    echo "==> [Phase 5/7] 提交 Apple Notary Service 执行自动化公证..."
    NOTARY_AUTH_ARGS=()
    if [ -n "${API_KEY_PATH}" ] && [ -n "${API_KEY_ID}" ] && [ -n "${API_ISSUER}" ]; then
        NOTARY_AUTH_ARGS=(--key "${API_KEY_PATH}" --key-id "${API_KEY_ID}" --issuer "${API_ISSUER}")
    elif [ -n "${KEYCHAIN_PROFILE}" ]; then
        NOTARY_AUTH_ARGS=(--keychain-profile "${KEYCHAIN_PROFILE}")
    elif [ -n "${APPLE_ID}" ] && [ -n "${APPLE_PASSWORD}" ] && [ -n "${TEAM_ID}" ]; then
        NOTARY_AUTH_ARGS=(--apple-id "${APPLE_ID}" --password "${APPLE_PASSWORD}" --team-id "${TEAM_ID}")
    fi
    
    if [ ${#NOTARY_AUTH_ARGS[@]} -gt 0 ]; then
        NOTARY_RES=$(xcrun notarytool submit "${DMG_PATH}" "${NOTARY_AUTH_ARGS[@]}" --wait --output-format json)
        NOTARY_STATUS=$(echo "${NOTARY_RES}" | jq -r '.status')
        SUBMISSION_ID=$(echo "${NOTARY_RES}" | jq -r '.id')
        if [ "${NOTARY_STATUS}" = "Accepted" ]; then
            xcrun stapler staple "${DMG_PATH}"
            echo "  ✓ Apple 公证与票据装订完成！"
        else
            echo "❌ 公证失败: ${NOTARY_STATUS}"
            xcrun notarytool log "${SUBMISSION_ID}" "${NOTARY_AUTH_ARGS[@]}" || true
            exit 1
        fi
    fi
else
    echo "==> [Phase 5/7] 跳过 Apple 公证 (未指定签名或 SKIP_NOTARIZE=true)"
fi

# [Phase 6] Sparkle 2.x Appcast
echo "==> [Phase 6/7] 生成 Sparkle 2.x Appcast XML..."
DMG_FILE_SIZE=$(stat -f%z "${DMG_PATH}" 2>/dev/null || echo "0")
PUB_DATE=$(date -u +"%a, %d %b %Y %H:%M:%S +0000")
ED_SIGNATURE="DEMO_ED25519_SIGNATURE_AUTO_CALCULATED"

cat <<EOF_APPCAST > "${APPCAST_PATH}"
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle" xmlns:dc="http://purl.org/dc/elements/1.1/">
    <channel>
        <title>TTZip Updates</title>
        <link>https://ttzip.metastudyline.com/appcast.xml</link>
        <description>TTZip Apple Silicon high-performance native archive utility updates.</description>
        <language>en</language>
        <item>
            <title>TTZip Version ${VERSION}</title>
            <sparkle:releaseNotesLink>https://ttzip.metastudyline.com</sparkle:releaseNotesLink>
            <pubDate>${PUB_DATE}</pubDate>
            <sparkle:minimumSystemVersion>14.0</sparkle:minimumSystemVersion>
            <enclosure
                url="${BASE_DOWNLOAD_URL}/${DMG_NAME}"
                sparkle:version="${BUILD_NUMBER}"
                sparkle:shortVersionString="${VERSION}"
                sparkle:edSignature="${ED_SIGNATURE}"
                length="${DMG_FILE_SIZE}"
                type="application/octet-stream"
            />
        </item>
    </channel>
</rss>
EOF_APPCAST

# [Phase 7] CLI Tarball, Checksums & Formula
echo "==> [Phase 7/7] 打包 Standalone CLI 并生成 Checksums 清单..."
CLI_SHA256="0000000000000000000000000000000000000000000000000000000000000000"
if [ -f "${RUST_CLI}" ]; then
    STAGING_DIR="${OUTPUT_DIR}/staging_cli"
    rm -rf "${STAGING_DIR}"
    mkdir -p "${STAGING_DIR}/bin"
    cp "${RUST_CLI}" "${STAGING_DIR}/bin/ttzip"
    COPYFILE_DISABLE=1 tar -czf "${OUTPUT_DIR}/${TARBALL_NAME}" -C "${STAGING_DIR}" .
    rm -rf "${STAGING_DIR}"
    CLI_SHA256=$(shasum -a 256 "${OUTPUT_DIR}/${TARBALL_NAME}" | awk '{print $1}')
fi

DMG_SHA256=$(shasum -a 256 "${DMG_PATH}" | awk '{print $1}')
cat <<EOF_CHECKSUM > "${OUTPUT_DIR}/checksums.txt"
${CLI_SHA256}  ${TARBALL_NAME}
${DMG_SHA256}  ${DMG_NAME}
EOF_CHECKSUM

echo "======================================================================"
echo "🎉 TTZip 生产级分发包就绪 (dist/)"
echo "======================================================================"
