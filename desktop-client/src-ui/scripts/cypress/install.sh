#!/bin/bash

# Cypress 安装脚本 - 使用官方镜像（最可靠）
# 如果官方镜像速度慢，可以修改 .npmrc 使用国内镜像

set -e

echo "🚀 开始安装 Cypress..."
echo ""

# 检查 npm 是否已安装
if ! command -v npm &> /dev/null; then
    echo "❌ npm 未找到，请先安装 Node.js"
    exit 1
fi

echo "✅ npm 已安装"
echo ""

# 使用官方镜像（最可靠）
export CYPRESS_DOWNLOAD_MIRROR=https://download.cypress.io

echo "📦 使用镜像源: $CYPRESS_DOWNLOAD_MIRROR"
echo ""
echo "💡 提示："
echo "   - 如果下载速度慢，可以编辑 .npmrc 文件切换到国内镜像"
echo "   - 国内镜像可能版本不全，建议先尝试官方镜像"
echo ""

# 安装 Cypress
echo "⏳ 正在安装 Cypress（这可能需要几分钟）..."
npm install cypress --save-dev

echo ""
echo "✅ Cypress 安装完成！"
echo ""
echo "📝 可用命令："
echo "  npm run cypress:open    - 打开 Cypress UI"
echo "  npm run cypress:run     - 运行所有测试"
echo "  npm run cypress:run:headed - 有界面地运行测试"
echo ""
