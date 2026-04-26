@echo off
chcp 65001 >nul 2>&1
setlocal enabledelayedexpansion

for %%I in ("%~dp0..") do set "REPO_ROOT=%%~fI"
set "VCPKG_ROOT=%USERPROFILE%\vcpkg"
set "VCPKG_TRIPLET=x64-windows"
set "FFMPEG_PORT=ffmpeg[avcodec,avdevice,avfilter,avformat,swresample,swscale]"
set "LLVM_ROOT=C:\Program Files\LLVM"

echo ==========================================
echo   SubtitleForge - Windows 初始化脚本
echo ==========================================

echo.
echo [1/6] 检查包管理器...

where choco >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    echo   ✓ Chocolatey 已安装
) else (
    echo   → 正在安装 Chocolatey...
    powershell -NoProfile -ExecutionPolicy Bypass -Command "Set-ExecutionPolicy Bypass -Scope Process -Force; [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072; iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))"
    echo   ✓ Chocolatey 安装完成
)

echo.
echo [2/6] 安装系统依赖...
echo   → 安装 Visual C++ 工具链、Git、CMake、LLVM...
choco install -y visualstudio2022buildtools --package-parameters "--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --passive"
if errorlevel 1 goto :error
choco install -y git cmake llvm
if errorlevel 1 goto :error
call refreshenv >nul 2>&1

echo.
echo [3/6] 检查 Rust 工具链...
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
rustup target add x86_64-pc-windows-msvc
if errorlevel 1 goto :error

echo.
echo [4/6] 准备 Node.js / pnpm / Tauri CLI...
where npm >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    echo   ✓ Node.js / npm 已安装
) else (
    echo   → 正在安装 Node.js LTS...
    choco install -y nodejs-lts
    if errorlevel 1 goto :error
    call refreshenv >nul 2>&1
)

where pnpm >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    echo   ✓ pnpm 已安装
) else (
    echo   → 正在安装 pnpm...
    call npm install -g pnpm
    if errorlevel 1 goto :error
    echo   ✓ pnpm 安装完成
)

where cargo-tauri >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    echo   ✓ Tauri CLI 已安装
) else (
    echo   → 正在安装 Tauri CLI...
    cargo install tauri-cli --version "^2" --locked
    if errorlevel 1 goto :error
    echo   ✓ Tauri CLI 安装完成
)

echo.
echo [5/6] 初始化 vcpkg 并安装 FFmpeg...
if exist "%VCPKG_ROOT%\.git" (
    echo   ✓ 复用现有 vcpkg: %VCPKG_ROOT%
) else (
    echo   → 克隆 vcpkg 到 %VCPKG_ROOT%...
    git clone --depth 1 https://github.com/microsoft/vcpkg.git "%VCPKG_ROOT%"
    if errorlevel 1 goto :error
)

if exist "%VCPKG_ROOT%\vcpkg.exe" (
    echo   ✓ vcpkg 已完成引导
) else (
    echo   → 正在引导 vcpkg...
    call "%VCPKG_ROOT%\bootstrap-vcpkg.bat" -disableMetrics
    if errorlevel 1 goto :error
)

echo.
echo   → 安装 FFmpeg 动态库 (%VCPKG_TRIPLET%)...
call "%VCPKG_ROOT%\vcpkg.exe" install "%FFMPEG_PORT%:%VCPKG_TRIPLET%" --clean-after-build
if errorlevel 1 goto :error

echo.
echo [6/6] 写入当前用户环境变量并安装前端依赖...
if exist "%REPO_ROOT%\src-tauri\.cargo\config.toml" (
    echo   → 检测到遗留的 src-tauri\.cargo\config.toml，Windows 将改用环境变量方案
    del /f /q "%REPO_ROOT%\src-tauri\.cargo\config.toml"
    if errorlevel 1 goto :error
)
set "PATH=%LLVM_ROOT%\bin;%PATH%"
set "VCPKG_ROOT=%VCPKG_ROOT%"
set "VCPKG_DEFAULT_TRIPLET=%VCPKG_TRIPLET%"
set "VCPKGRS_DYNAMIC=1"
set "FFMPEG_DIR=%VCPKG_ROOT%\installed\%VCPKG_TRIPLET%"
set "BINDGEN_EXTRA_CLANG_ARGS=-I%VCPKG_ROOT%\installed\%VCPKG_TRIPLET%\include"
set "LIBCLANG_PATH=%LLVM_ROOT%\bin"

setx VCPKG_ROOT "%VCPKG_ROOT%" >nul
setx VCPKG_DEFAULT_TRIPLET "%VCPKG_TRIPLET%" >nul
setx VCPKGRS_DYNAMIC "1" >nul
setx FFMPEG_DIR "%VCPKG_ROOT%\installed\%VCPKG_TRIPLET%" >nul
setx BINDGEN_EXTRA_CLANG_ARGS "-I%VCPKG_ROOT%\installed\%VCPKG_TRIPLET%\include" >nul
setx LIBCLANG_PATH "%LLVM_ROOT%\bin" >nul

echo   ✓ 已写入用户环境变量:
echo     VCPKG_ROOT=%VCPKG_ROOT%
echo     VCPKG_DEFAULT_TRIPLET=%VCPKG_TRIPLET%
echo     VCPKGRS_DYNAMIC=1
echo     FFMPEG_DIR=%VCPKG_ROOT%\installed\%VCPKG_TRIPLET%
echo     BINDGEN_EXTRA_CLANG_ARGS=-I%VCPKG_ROOT%\installed\%VCPKG_TRIPLET%\include
echo     LIBCLANG_PATH=%LLVM_ROOT%\bin

pushd "%REPO_ROOT%"
call pnpm install
if errorlevel 1 (
    popd
    goto :error
)
popd

echo.
echo ==========================================
echo   ✓ Windows 初始化完成！
echo ==========================================
echo.
echo   可用命令:
echo     pnpm tauri dev          启动开发服务器
echo     pnpm tauri build        构建生产版本
echo.
echo   说明:
echo     - Windows 本地环境不提交 src-tauri\.cargo\config.toml
echo     - Windows 构建和 CI 现在统一使用 vcpkg 的 FFmpeg 动态库
echo     - 初始化脚本会删除遗留的 Cargo 本地配置，避免覆盖环境变量
echo     - build.rs 会在构建时自动复制 FFmpeg 运行时 DLL 到 exe 旁边
echo     - 默认构建只需要 FFmpeg 这一组 DLL
echo     - 若启用 --features offline-translate / cuda / openblas，需额外检查对应运行时
echo.
echo   GPU 加速选项 (构建时指定):
echo     --features cuda         NVIDIA CUDA 加速
echo     --features openblas     CPU BLAS 加速
echo.

endlocal
pause
exit /b 0

:error
echo.
echo ==========================================
echo   x Windows 初始化失败
echo ==========================================
echo   请检查上面的错误输出后重试。
endlocal
pause
exit /b 1
