#!/bin/bash
# IronClaw Desktop Client — 开发模式启动脚本
#
# 用法:
#   ./desktop-client/scripts/start-dev.sh
#
# 配置来源（优先级从高到低）:
#   1. 管理端下发缓存（~/Library/Application Support/ironclaw-desktop/admin_config.json）
#   2. 显式环境变量（shell export）
#   3. desktop-client/.env
#   4. 客户端默认值（DATABASE_BACKEND=libsql 等）
#
# ⚠️ 不使用 ~/.ironclaw/.env — 客户端数据完全隔离
#
# 前置条件:
#   - LLM 配置至少通过以下任一方式提供:
#     a) desktop-client/.env 中设置 LLM_BACKEND + LLM_API_KEY
#     b) 管理端下发 admin_config.json
#     c) 显式环境变量: LLM_BACKEND=anthropic LLM_API_KEY=sk-xxx ./scripts/start-dev.sh

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

# ── 检查 LLM 配置来源 ─────────────────────────────────────────
HAS_ENV_FILE=false
HAS_ADMIN_CONFIG=false
HAS_ENV_VAR=false

[ -f "$PROJECT_DIR/.env" ] && HAS_ENV_FILE=true
[ -f "$APP_DATA_DIR/admin_config.json" ] && HAS_ADMIN_CONFIG=true
[ -n "${LLM_API_KEY:-}" ] && HAS_ENV_VAR=true

if [ "$HAS_ENV_FILE" = false ] && [ "$HAS_ADMIN_CONFIG" = false ] && [ "$HAS_ENV_VAR" = false ]; then
    echo ""
    echo "⚠️  未找到任何 LLM 配置来源"
    echo ""
    echo "请通过以下任一方式配置:"
    echo ""
    echo "  方式 1: 创建 .env 文件"
    echo "    cp desktop-client/.env.example desktop-client/.env"
    echo "    # 编辑 .env，设置 LLM_BACKEND 和 LLM_API_KEY"
    echo ""
    echo "  方式 2: 命令行传入"
    echo "    LLM_BACKEND=anthropic LLM_API_KEY=sk-ant-xxx $0"
    echo ""
    echo "  方式 3: 等待管理端下发配置"
    echo "    配置缓存路径: $APP_DATA_DIR/admin_config.json"
    echo ""
    exit 1
fi

# 显示配置来源
echo ""
echo "📋 配置来源:"
[ "$HAS_ADMIN_CONFIG" = true ] && echo "   ✅ 管理端下发 (admin_config.json)"
[ "$HAS_ENV_VAR" = true ]      && echo "   ✅ 环境变量 (LLM_API_KEY)"
[ "$HAS_ENV_FILE" = true ]     && echo "   ✅ 本地配置 (desktop-client/.env)"
echo "   📁 数据目录: $APP_DATA_DIR"

# ── 检查前端依赖 ────────────────────────────────────────────────
if [ ! -d "$PROJECT_DIR/src-ui/node_modules" ]; then
    echo ""
    echo "📦 安装前端依赖..."
    (cd "$PROJECT_DIR/src-ui" && npm install)
fi

# ── 启动前端 dev server ────────────────────────────────────────
# 在脚本中显式启动，避免 Tauri beforeDevCommand 的 PATH 问题
# （nvm 等工具安装的 node/npm 在 Tauri 的 shell 中可能找不到）
echo ""
echo "🌐 启动前端 dev server..."
(cd "$PROJECT_DIR/src-ui" && npm run dev) &
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
