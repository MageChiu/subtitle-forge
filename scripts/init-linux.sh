#!/usr/bin/env bash
set -euo pipefail

echo "=========================================="
echo "  SubtitleForge - Linux 初始化脚本"
echo "=========================================="

echo ""
echo "[1/5] 安装系统依赖..."
if command -v apt-get &>/dev/null; then
    echo "  → 检测到 Debian/Ubuntu 系统..."
    sudo apt-get update
    sudo apt-get install -y \
        build-essential \
        pkg-config \
        libssl-dev \
        libffmpeg-dev \
        libavcodec-dev \
        libavformat-dev \
        libavutil-dev \
        libswresample-dev \
        libswscale-dev \
        libavdevice-dev \
        libavfilter-dev \
        libclang-dev \
        libgtk-3-dev \
        libwebkit2gtk-4.1-dev \
        libappindicator3-dev \
        librsvg2-dev \
        libsoup-3.0-dev \
        libjavascriptcoregtk-4.1-dev
    echo "  ✓ Debian/Ubuntu 依赖安装完成"
elif command -v dnf &>/dev/null; then
    echo "  → 检测到 Fedora/RHEL 系统..."
    sudo dnf install -y \
        gcc gcc-c++ make pkg-config \
        openssl-devel \
        ffmpeg-devel \
        libavcodec-devel libavformat-devel libavutil-devel \
        libswresample-devel libswscale-devel \
        clang-devel \
        gtk3-devel \
        webkit2gtk4.1-devel \
        libappindicator-gtk3-devel \
        librsvg2-devel \
        libsoup3-devel \
        javascriptcoregtk4.1-devel
    echo "  ✓ Fedora/RHEL 依赖安装完成"
elif command -v pacman &>/dev/null; then
    echo "  → 检测到 Arch Linux 系统..."
    sudo pacman -S --noconfirm \
        base-devel pkg-config \
        openssl \
        ffmpeg \
        clang \
        gtk3 \
        webkit2gtk-4.1 \
        libappindicator-gtk3 \
        librsvg \
        libsoup3
    echo "  ✓ Arch Linux 依赖安装完成"
else
    echo "  ⚠ 未检测到支持的包管理器 (apt/dnf/pacman)"
    echo "  请手动安装以下依赖:"
    echo "    - FFmpeg 开发库 (libavcodec, libavformat, libavutil, libswresample, libswscale)"
    echo "    - libclang-dev / clang-devel"
    echo "    - GTK3 开发库"
    echo "    - WebKit2GTK 4.1 开发库"
    echo "    - libsoup 3.0 开发库"
    echo "    - pkg-config, openssl 开发库"
fi

echo ""
echo "[2/5] 检查 Rust 工具链..."
if command -v cargo &>/dev/null; then
    echo "  ✓ Rust 已安装: $(rustc --version)"
else
    echo "  → 正在安装 Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    echo "  ✓ Rust 安装完成: $(rustc --version)"
fi

echo ""
echo "[3/5] 安装 Tauri CLI..."
if command -v cargo-tauri &>/dev/null; then
    echo "  ✓ Tauri CLI 已安装"
else
    echo "  → 正在安装 Tauri CLI..."
    cargo install tauri-cli --version "^2"
    echo "  ✓ Tauri CLI 安装完成"
fi

echo ""
echo "[4/5] 安装前端依赖..."
if command -v pnpm &>/dev/null; then
    echo "  ✓ pnpm 已安装"
else
    echo "  → 正在安装 pnpm..."
    npm install -g pnpm
    echo "  ✓ pnpm 安装完成"
fi
pnpm install

echo ""
echo "[5/5] 配置 bindgen..."
CARGO_CONFIG_DIR="src-tauri/.cargo"
mkdir -p "$CARGO_CONFIG_DIR"
cat > "$CARGO_CONFIG_DIR/config.toml" << EOF
[env]
BINDGEN_EXTRA_CLANG_ARGS = "-I/usr/include -I/usr/local/include"
EOF
echo "  ✓ 已写入 $CARGO_CONFIG_DIR/config.toml"

echo ""
echo "=========================================="
echo "  ✓ Linux 初始化完成！"
echo "=========================================="
echo ""
echo "  可用命令:"
echo "    pnpm tauri dev          启动开发服务器"
echo "    pnpm tauri build        构建生产版本"
echo ""
echo "  GPU 加速选项 (构建时指定):"
echo "    --features cuda         NVIDIA CUDA 加速"
echo "    --features vulkan       Vulkan 加速"
echo "    --features openblas     CPU BLAS 加速"
echo ""
