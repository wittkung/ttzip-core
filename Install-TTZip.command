#!/usr/bin/env bash
# SPDX-License-Identifier: LicenseRef-TTZip-Source-Available-1.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine for macOS.
#
# Double-clickable macOS installation script (GUI App + CLI tools)

set -e

# Crucial: cd to script's directory regardless of how Finder launches it
cd "$(dirname "$0")"
REPO_ROOT="$(pwd)"

# Colors & Formatting
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
GOLD='\033[38;5;220m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

clear
echo -e "${GOLD}"
echo "  ████████╗████████╗███████╗██╗██████╗ "
echo "  ╚══██╔══╝╚══██╔══╝╚══███╔╝██║██╔══██╗"
echo "     ██║      ██║     ███╔╝ ██║██████╔╝"
echo "     ██║      ██║    ███╔╝  ██║██╔═══╝ "
echo "     ██║      ██║   ███████╗██║██║     "
echo "     ╚═╝      ╚═╝   ╚══════╝╚═╝╚═╝     "
echo -e "${CYAN}  Native High-Performance Archiver for macOS${NC}"
echo -e "${BOLD}  [双击一键编译并覆盖安装 Release 最新版]${NC}"
echo "========================================================"

# 1. Check Toolchain
echo -e "\n${BLUE}==> [1/5] 检查编译环境...${NC}"
if ! command -v swift >/dev/null 2>&1; then
    echo -e "${RED}[错误] 未检测到 Swift 编译器，请确保已安装 Xcode 16+ 或 Command Line Tools。${NC}"
    echo "按任意键退出..."
    read -n 1 -s -r
    exit 1
fi

ARCH="$(uname -m)"
OS_VER="$(sw_vers -productVersion 2>/dev/null || echo "macOS")"
echo "  • 架构: ${ARCH} (Apple Silicon / Intel 兼容)"
echo "  • 系统: macOS ${OS_VER}"
echo "  • 源码路径: ${REPO_ROOT}"

# 2. Terminate Old Running Instances
echo -e "\n${BLUE}==> [2/5] 清理旧版运行进程...${NC}"
if pgrep -x "TTZip" >/dev/null 2>&1; then
    echo -e "${YELLOW}  • 检测到正在运行的 TTZip，正在平滑关闭...${NC}"
    pkill -x "TTZip" >/dev/null 2>&1 || true
    sleep 0.8
fi

# 3. Compile Release Targets & Bundle App
echo -e "\n${BLUE}==> [3/5] 开始编译最新 Release 版本 (Swift 6.0 + 硬件加速)...${NC}"
if [ -f "${REPO_ROOT}/../apple/scripts/bundle_app.sh" ]; then
    echo "  --> 正在调用 apple/scripts/bundle_app.sh 构建桌面端..."
    "${REPO_ROOT}/../apple/scripts/bundle_app.sh" --release
    APP_BUNDLE="${REPO_ROOT}/../apple/dist/TTZip.app"
else
    echo "  --> 正在编译 TTZipCore..."
    swift build -c release
    APP_BUNDLE="${REPO_ROOT}/dist/TTZip.app"
fi

# 4. Install TTZip.app to /Applications
echo -e "\n${BLUE}==> [4/5] 覆盖安装 TTZip.app 到 /Applications...${NC}"
TARGET_APP="/Applications/TTZip.app"

if [ -d "${APP_BUNDLE}" ]; then
    echo "  --> 覆盖安装至 ${TARGET_APP}..."
    if [ -d "${TARGET_APP}" ]; then
        if [ -w "/Applications" ]; then
            rm -rf "${TARGET_APP}"
        else
            echo -e "${YELLOW}  [需要权限] 请输入密码以覆盖 /Applications/TTZip.app:${NC}"
            sudo rm -rf "${TARGET_APP}"
        fi
    fi

    if [ -w "/Applications" ]; then
        cp -R "${APP_BUNDLE}" "/Applications/"
    else
        sudo cp -R "${APP_BUNDLE}" "/Applications/"
    fi

    # Re-register LaunchServices
    if [ -x "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister" ]; then
        /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "${TARGET_APP}" 2>/dev/null || true
    fi
    echo -e "${GREEN}  ✓ GUI 桌面版安装成功: ${TARGET_APP}${NC}"
fi

# 5. Install CLI Tools to System PATH
echo -e "\n${BLUE}==> [5/5] 安装 CLI 命令行工具 (ttzip / ttzip-cli)...${NC}"

CLI_TARGET_DIR="/usr/local/bin"
if [ -w "/opt/homebrew/bin" ] && [[ ":$PATH:" == *":/opt/homebrew/bin:"* ]]; then
    CLI_TARGET_DIR="/opt/homebrew/bin"
fi

echo "  --> 目标目录: ${CLI_TARGET_DIR}"

install_cli() {
    local target_dir="$1"
    if [ -w "${target_dir}" ]; then
        cp "${CLI_SRC}" "${target_dir}/ttzip-cli"
        strip -x "${target_dir}/ttzip-cli" 2>/dev/null || true
        chmod +x "${target_dir}/ttzip-cli"
        ln -sf "${target_dir}/ttzip-cli" "${target_dir}/ttzip"
    else
        echo -e "${YELLOW}  [需要权限] 请输入密码以安装命令行工具到 ${target_dir}:${NC}"
        sudo mkdir -p "${target_dir}"
        sudo cp "${CLI_SRC}" "${target_dir}/ttzip-cli"
        sudo strip -x "${target_dir}/ttzip-cli" 2>/dev/null || true
        sudo chmod +x "${target_dir}/ttzip-cli"
        sudo ln -sf "${target_dir}/ttzip-cli" "${target_dir}/ttzip"
    fi
}

install_cli "${CLI_TARGET_DIR}"

# If on Apple Silicon with both /opt/homebrew/bin and /usr/local/bin, link both if possible
if [ "${CLI_TARGET_DIR}" = "/opt/homebrew/bin" ] && [ -d "/usr/local/bin" ] && [ -w "/usr/local/bin" ]; then
    cp "${CLI_SRC}" "/usr/local/bin/ttzip-cli" 2>/dev/null || true
    strip -x "/usr/local/bin/ttzip-cli" 2>/dev/null || true
    ln -sf "/usr/local/bin/ttzip-cli" "/usr/local/bin/ttzip" 2>/dev/null || true
fi

echo -e "${GREEN}  ✓ CLI 命令行工具已安装至: ${CLI_TARGET_DIR}/ttzip & ttzip-cli${NC}"

# Summary & Launch
echo ""
echo "========================================================"
echo -e "${GREEN}${BOLD}🎉 TTZip 最新 Release 版本已成功安装并覆盖！${NC}"
echo "========================================================"
echo -e "  • 桌面端应用 : ${BOLD}/Applications/TTZip.app${NC}"
echo -e "  • 命令行工具 : ${BOLD}${CLI_TARGET_DIR}/ttzip${NC}"
echo ""
echo "--> 正在启动最新版 TTZip.app..."
open -a TTZip || open "${TARGET_APP}"

echo ""
echo -e "${CYAN}安装完毕！按任意键关闭此终端窗口...${NC}"
read -n 1 -s -r
exit 0
