@echo off
setlocal enabledelayedexpansion
title 构建应用
color 0A

echo ================================================
echo        Windows网络配置工具构建
echo ================================================
echo.
echo 正在构建前端...
call npm run build
if errorlevel 1 (
    echo [错误] 前端构建失败！
    exit /b %errorlevel%
)

echo.
echo 正在构建Rust应用...
cd src-tauri
cargo build --release
if errorlevel 1 (
    echo [错误] 后端构建失败！
    cd ..
    exit /b %errorlevel%
)
cd ..

echo.
echo 构建完成！
echo 可执行文件位于: src-tauri\target\release\tauri-vue-20250419.exe
echo.
pause
