@echo off
REM Cypress 安装脚本 - 使用官方镜像（最可靠）
REM 如果官方镜像速度慢，可以修改 .npmrc 使用国内镜像

setlocal enabledelayedexpansion

echo 🚀 开始安装 Cypress...
echo.

REM 检查 npm 是否已安装
where npm >nul 2>nul
if %ERRORLEVEL% NEQ 0 (
    echo ❌ npm 未找到，请先安装 Node.js
    exit /b 1
)

echo ✅ npm 已安装
echo.

REM 使用官方镜像（最可靠）
set CYPRESS_DOWNLOAD_MIRROR=https://download.cypress.io

echo 📦 使用镜像源: %CYPRESS_DOWNLOAD_MIRROR%
echo.
echo 💡 提示：
echo    - 如果下载速度慢，可以编辑 .npmrc 文件切换到国内镜像
echo    - 国内镜像可能版本不全，建议先尝试官方镜像
echo.

REM 安装 Cypress
echo ⏳ 正在安装 Cypress（这可能需要几分钟）...
call npm install cypress --save-dev

echo.
echo ✅ Cypress 安装完成！
echo.
echo 📝 可用命令：
echo   npm run cypress:open    - 打开 Cypress UI
echo   npm run cypress:run     - 运行所有测试
echo   npm run cypress:run:headed - 有界面地运行测试
echo.

endlocal
