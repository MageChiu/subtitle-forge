@echo off
chcp 65001 >nul 2>&1
setlocal enabledelayedexpansion

echo ==========================================
echo   SubtitleForge - Windows 初始化脚本
echo ==========================================

echo.
echo [1/5] 检查系统依赖...

where choco >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    echo   ✓ Chocolatey 已安装
) else (
    echo   → 正在安装 Chocolatey...
    powershell -NoProfile -ExecutionPolicy Bypass -Command "Set-ExecutionPolicy Bypass -Scope Process -Force; [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072; iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))"
    echo   ✓ Chocolatey 安装完成
)

echo.
echo   → 安装构建工具和 FFmpeg...
choco install -y visualstudio2022buildtools --package-parameters "--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --passive"
choco install -y ffmpeg pkgconfiglite

echo.
echo [2/5] 检查 Rust 工具链...
where cargo >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    echo   ✓ Rust 已安装
    rustc --version
) else (
    echo   → 正在安装 Rust...
    powershell -NoProfile -ExecutionPolicy Bypass -Command "Invoke-WebRequest -Uri https://win.rustup.rs/x86_64 -OutFile rustup-init.exe; ./rustup-init.exe -y"
    del rustup-init.exe 2>nul
    call "%USERPROFILE%\.cargo\env.bat"
    echo   ✓ Rust 安装完成
    rustc --version
)

echo.
echo [3/5] 安装 Tauri CLI...
where cargo-tauri >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    echo   ✓ Tauri CLI 已安装
) else (
    echo   → 正在安装 Tauri CLI...
    cargo install tauri-cli --version "^2"
    echo   ✓ Tauri CLI 安装完成
)

echo.
echo [4/5] 安装前端依赖...
where pnpm >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    echo   ✓ pnpm 已安装
) else (
    echo   → 正在安装 pnpm...
    npm install -g pnpm
    echo   ✓ pnpm 安装完成
)
call pnpm install

echo.
echo [5/5] 配置 bindgen...
set "CARGO_CONFIG_DIR=src-tauri\.cargo"
if not exist "%CARGO_CONFIG_DIR%" mkdir "%CARGO_CONFIG_DIR%"
if exist "%CARGO_CONFIG_DIR%\config.toml" (
    echo   ✓ %CARGO_CONFIG_DIR%\config.toml 已存在，跳过
) else (
    (
        echo [env]
        echo BINDGEN_EXTRA_CLANG_ARGS = "-I C:\Program Files\FFmpeg\include"
    ) > "%CARGO_CONFIG_DIR%\config.toml"
    echo   ✓ 已写入 %CARGO_CONFIG_DIR%\config.toml
)

echo.
echo ==========================================
echo   ✓ Windows 初始化完成！
echo ==========================================
echo.
echo   可用命令:
echo     pnpm tauri dev          启动开发服务器
echo     pnpm tauri build        构建生产版本
echo.
echo   GPU 加速选项 (构建时指定):
echo     --features cuda         NVIDIA CUDA 加速
echo     --features openblas     CPU BLAS 加速
echo.

endlocal
pause
