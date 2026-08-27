#!/usr/bin/env bash
# ============================================================
# TTZip macOS 重新构建并启动 (100% Pure Swift 6 + C/Rust FFI)
# ============================================================

# 自动创建 fnm 状态目录，消除本地多终端环境提示
mkdir -p "$HOME/.local/state/fnm_multishells" 2>/dev/null || true

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

echo "============================================================"
echo "  ⚡ 启动 TTZip macOS 全量重新构建与运行流水线..."
echo "  架构: C/Rust 极速内核 (TTZipCore) + Swift 6 原生 macOS App"
echo "============================================================"

if [ -f "./scripts/bundle_app.sh" ]; then
    ./scripts/bundle_app.sh --release --open "$@"
elif [ -f "../apple/scripts/bundle_app.sh" ]; then
    ../apple/scripts/bundle_app.sh --release --open "$@"
elif [ -f "./apple/scripts/bundle_app.sh" ]; then
    ./apple/scripts/bundle_app.sh --release --open "$@"
else
    echo "❌ 未找到 bundle_app.sh 构建脚本！"
    exit 1
fi

if [ -t 0 ]; then
    echo ""
    echo "💡 按任意键关闭此终端窗口..."
    read -n 1 -s -r 2>/dev/null || true
fi
