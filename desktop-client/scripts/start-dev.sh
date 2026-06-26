#!/bin/bash
# IronClaw Desktop Client — 开发模式启动脚本
#
# 用法:
#   ./desktop-client/scripts/start-dev.sh
#
# 配置来源（优先级从高到低）:
#   1. 显式环境变量（shell export）
#   2. Rust 启动链路读取管理端模型接口（/api/client-models）
#   3. 客户端默认值（DATABASE_BACKEND=libsql 等）
#
# ⚠️ 不自动读取全局 CLI env 或 desktop-client/.env — 客户端数据完全隔离
#
# 前置条件:
#   - LLM 配置至少通过以下任一方式提供:
#     a) 显式环境变量: LLM_BACKEND=anthropic ANTHROPIC_API_KEY=sk-ant-xxx ./scripts/start-dev.sh
#     b) Admin Backend 已启动，且 /api/client-models 返回可用于启动的默认模型

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
APP_DATA_DIR="${HOME}/Library/Application Support/ironclaw-desktop"

FRONTEND_PID=""

cleanup() {
    if [ -n "$FRONTEND_PID" ] && kill -0 "$FRONTEND_PID" 2>/dev/null; then
        echo ""
        echo "🛑 停止前端 dev server (PID $FRONTEND_PID)..."
        kill "$FRONTEND_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

echo "╔══════════════════════════════════════════════╗"
echo "║  IronClaw Desktop Client — 嵌入式模式       ║"
echo "╚══════════════════════════════════════════════╝"

# 显示配置来源
echo ""
echo "📋 配置来源:"
if [ -n "${LLM_BACKEND:-}" ]; then
    echo "   ✅ 显式环境变量 (LLM_BACKEND=${LLM_BACKEND})"
else
    echo "   ✅ Rust 启动链路将尝试读取管理端模型接口 (/api/client-models)"
fi
echo "   📁 数据目录: $APP_DATA_DIR"

# ── 检查前端依赖 ────────────────────────────────────────────────
if [ ! -d "$PROJECT_DIR/ui/node_modules" ]; then
    echo ""
    echo "📦 安装前端依赖..."
    (cd "$PROJECT_DIR/ui" && npm install)
fi

# ── 启动前端 dev server ────────────────────────────────────────
# 在脚本中显式启动，避免 Tauri beforeDevCommand 的 PATH 问题
# （nvm 等工具安装的 node/npm 在 Tauri 的 shell 中可能找不到）

# 清理可能残留的 5173 端口进程
if lsof -ti:5173 > /dev/null 2>&1; then
    echo ""
    echo "🧹 清理 5173 端口残留进程..."
    lsof -ti:5173 | xargs kill -9 2>/dev/null || true
    sleep 1
fi

echo ""
echo "🌐 启动前端 dev server..."
(cd "$PROJECT_DIR/ui" && npm run dev) &
FRONTEND_PID=$!

# 等待前端就绪（最多 30 秒）
echo "   等待 http://localhost:5173 就绪..."
READY=false
for i in $(seq 1 30); do
    if curl -sf http://localhost:5173 > /dev/null 2>&1; then
        echo "   ✅ 前端已就绪 (${i}s)"
        READY=true
        break
    fi
    if ! kill -0 "$FRONTEND_PID" 2>/dev/null; then
        echo "   ❌ 前端 dev server 意外退出"
        exit 1
    fi
    sleep 1
done

if [ "$READY" = false ]; then
    echo "   ❌ 前端 dev server 启动超时（30s）"
    exit 1
fi

# ── 启动 Tauri ────────────────────────────────────────────────
# tauri.conf.json 中的 beforeDevCommand 已移除，前端由本脚本管理
echo ""
echo "🚀 启动 Tauri..."
echo "   前端: http://localhost:5173"
echo "   引擎: IronClaw 嵌入式（无需外部服务）"
echo ""

cd "$PROJECT_DIR"
cargo tauri dev
