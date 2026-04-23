#!/usr/bin/env bash
set -euo pipefail

echo "=========================================="
echo "  SubtitleForge - macOS 初始化脚本"
echo "=========================================="

echo ""
echo "[1/6] 检查 Xcode Command Line Tools..."
if xcode-select -p &>/dev/null; then
    echo "  ✓ Xcode Command Line Tools 已安装"
else
    echo "  → 正在安装 Xcode Command Line Tools..."
    xcode-select --install
    echo "  ⚠ 请完成安装后重新运行此脚本"
    exit 1
fi

echo ""
echo "[2/6] 检查 Homebrew..."
if command -v brew &>/dev/null; then
    echo "  ✓ Homebrew 已安装"
else
    echo "  → 正在安装 Homebrew..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    echo "  ✓ Homebrew 安装完成"
fi

echo ""
echo "[3/6] 安装系统依赖..."
brew install ffmpeg pkg-config llvm

echo ""
echo "[4/6] 检查 Rust 工具链..."
if command -v cargo &>/dev/null; then
    echo "  ✓ Rust 已安装: $(rustc --version)"
else
    echo "  → 正在安装 Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    echo "  ✓ Rust 安装完成: $(rustc --version)"
fi

echo ""
echo "[5/6] 安装 Tauri CLI..."
if command -v cargo-tauri &>/dev/null; then
    echo "  ✓ Tauri CLI 已安装"
else
    echo "  → 正在安装 Tauri CLI..."
    cargo install tauri-cli --version "^2"
    echo "  ✓ Tauri CLI 安装完成"
fi

echo ""
echo "[6/6] 安装前端依赖..."
if command -v pnpm &>/dev/null; then
    echo "  ✓ pnpm 已安装"
else
    echo "  → 正在安装 pnpm..."
    npm install -g pnpm
    echo "  ✓ pnpm 安装完成"
fi
pnpm install

echo ""
echo "=========================================="
echo "  配置 bindgen 的 macOS SDK 路径..."
echo "=========================================="
SDK_PATH=$(xcrun --show-sdk-path 2>/dev/null || echo "/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk")
CARGO_CONFIG_DIR="src-tauri/.cargo"
mkdir -p "$CARGO_CONFIG_DIR"
cat > "$CARGO_CONFIG_DIR/config.toml" << EOF
[env]
BINDGEN_EXTRA_CLANG_ARGS = "-isysroot $SDK_PATH"
EOF
echo "  ✓ 已写入 $CARGO_CONFIG_DIR/config.toml (SDK: $SDK_PATH)"

echo ""
echo "=========================================="
echo "  ✓ macOS 初始化完成！"
echo "=========================================="
echo ""
echo "  可用命令:"
echo "    pnpm tauri dev          启动开发服务器"
echo "    pnpm tauri build        构建生产版本"
echo ""
echo "  GPU 加速选项 (构建时指定):"
echo "    --features metal        Apple Metal 加速 (推荐)"
echo "    --features coreml       Apple CoreML 加速"
echo ""
