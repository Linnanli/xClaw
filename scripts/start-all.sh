#!/bin/bash

# 整合启动脚本 - 启动后端和前端
# 
# 此脚本解决了以下问题:
# 1. 后端 REPL 模式阻塞 HTTP 服务器 - 使用 stdin 重定向解决
# 2. 旧进程仍在运行导致启动失败 - 清理旧进程和 PID 文件
# 3. 认证令牌过期 - 每次启动生成新令牌
# 4. 环境变量未设置 - 检查并设置所有必需的环境变量

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}✅ $1${NC}"
}

log_error() {
    echo -e "${RED}❌ $1${NC}"
}

log_warn() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

log_section() {
    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
}

cleanup() {
    log_info "清理资源..."
    
    # 杀死后端进程
    if [ ! -z "$BACKEND_PID" ]; then
        log_info "停止后端服务 (PID: $BACKEND_PID)..."
        kill $BACKEND_PID 2>/dev/null || true
    fi
    
    # 杀死前端进程
    if [ ! -z "$FRONTEND_PID" ]; then
        log_info "停止前端服务 (PID: $FRONTEND_PID)..."
        kill $FRONTEND_PID 2>/dev/null || true
    fi
    
    # 杀死 Tauri 进程
    if [ ! -z "$TAURI_PID" ]; then
        log_info "停止 Tauri 客户端 (PID: $TAURI_PID)..."
        kill $TAURI_PID 2>/dev/null || true
    fi
    
    log_info "清理完成"
}

trap cleanup EXIT

# ============================================
# 检查依赖
# ============================================

log_section "检查依赖"

if ! command -v cargo &> /dev/null; then
    log_error "Rust/Cargo 未找到，请先安装 Rust"
    exit 1
fi
log_info "Rust/Cargo 已安装"

if ! command -v node &> /dev/null; then
    log_error "Node.js 未找到，请先安装 Node.js"
    exit 1
fi
log_info "Node.js 已安装"

if ! command -v npm &> /dev/null; then
    log_error "npm 未找到，请先安装 npm"
    exit 1
fi
log_info "npm 已安装"

# ============================================
# 检查环境变量
# ============================================

log_section "检查环境变量"

# 先从项目 .env 加载配置（优先级更高）
if [ -f .env ]; then
    log_info "从项目 .env 加载配置..."
    set -a
    source .env
    set +a
fi

# 再从 ~/.ironclaw/.env 加载配置（作为后备，只设置未定义的变量）
if [ -f ~/.ironclaw/.env ]; then
    log_info "从 ~/.ironclaw/.env 加载后备配置..."
    # 只加载未设置的变量
    while IFS='=' read -r key value; do
        # 跳过注释和空行
        [[ $key =~ ^[[:space:]]*# ]] && continue
        [[ -z $key ]] && continue
        
        # 移除引号
        value=$(echo "$value" | sed 's/^"//;s/"$//')
        
        # 只有当变量未设置时才设置
        if [ -z "${!key}" ]; then
            export "$key"="$value"
        fi
    done < ~/.ironclaw/.env
fi

# 检查是否有有效的 LLM 配置
if [ -z "$ANTHROPIC_API_KEY" ] && [ -z "$OPENAI_API_KEY" ] && [ -z "$LLM_API_KEY" ] && [ -z "$NEARAI_API_KEY" ]; then
    log_error "未找到任何 LLM API 密钥配置"
    echo ""
    echo "请先设置以下之一:"
    echo "  export ANTHROPIC_API_KEY=\"sk-ant-...\""
    echo "  export OPENAI_API_KEY=\"sk-...\""
    echo "  export LLM_API_KEY=\"sk-...\" (for Qwen/OpenAI-compatible)"
    echo "  export NEARAI_API_KEY=\"...\" (for NEAR AI)"
    echo ""
    exit 1
fi

log_info "LLM 配置已检测"

# ============================================
# 设置环境变量（仅当未设置时）
# ============================================

log_section "设置环境变量"

log_info "使用配置:"
log_info "✅ LLM_BACKEND=$LLM_BACKEND"
if [ ! -z "$LLM_BASE_URL" ]; then
    log_info "✅ LLM_BASE_URL=$LLM_BASE_URL"
fi
if [ ! -z "$LLM_MODEL" ]; then
    log_info "✅ LLM_MODEL=$LLM_MODEL"
fi
if [ ! -z "$ANTHROPIC_MODEL" ]; then
    log_info "✅ ANTHROPIC_MODEL=$ANTHROPIC_MODEL"
fi
log_info "✅ LLM API 密钥已设置"

# ============================================
# 清理旧进程和端口
# ============================================

log_section "清理旧进程和端口"

log_info "停止旧的后端进程..."
pkill -f "cargo run" || true
sleep 1

log_info "清理 PID 文件..."
rm -f ~/.ironclaw/ironclaw.pid

log_info "清理 3000 端口..."
lsof -i :3000 | grep -v COMMAND | awk '{print $2}' | xargs kill -9 2>/dev/null || true
sleep 1

log_info "清理 5173 端口..."
lsof -i :5173 | grep -v COMMAND | awk '{print $2}' | xargs kill -9 2>/dev/null || true
sleep 1

log_info "清理 8080 端口..."
lsof -i :8080 | grep -v COMMAND | awk '{print $2}' | xargs kill -9 2>/dev/null || true
sleep 1

log_info "所有端口已清理"

# ============================================
# 启动后端
# ============================================

log_section "启动后端服务"

log_info "启动后端..."
# 关键修复: 在后台启动后端，避免 REPL 阻塞
# 使用 exec 和 stdin 重定向确保进程不会等待输入
(exec cargo run -- run --no-onboard < /dev/null > /tmp/backend.log 2>&1) &
BACKEND_PID=$!

log_info "后端进程 PID: $BACKEND_PID"

log_info "等待后端编译和启动（这可能需要几分钟）..."

# 智能等待：检查编译是否完成并且服务器启动
COMPILE_TIMEOUT=600  # 10 分钟超时
COMPILE_CHECK_INTERVAL=5
COMPILE_START_TIME=$(date +%s)

echo -n "编译进度: "
while true; do
    # 检查进程是否还在运行
    if ! kill -0 $BACKEND_PID 2>/dev/null; then
        echo ""
        log_error "后端进程已退出，检查编译错误"
        echo ""
        echo "查看后端日志:"
        echo "  tail -50 /tmp/backend.log"
        exit 1
    fi
    
    # 检查健康端点是否响应
    if curl -s http://localhost:3000/api/health > /dev/null 2>&1; then
        echo ""
        log_info "后端编译完成并启动成功"
        break
    fi
    
    # 检查超时
    CURRENT_TIME=$(date +%s)
    ELAPSED=$((CURRENT_TIME - COMPILE_START_TIME))
    if [ $ELAPSED -gt $COMPILE_TIMEOUT ]; then
        echo ""
        log_error "后端编译超时（${COMPILE_TIMEOUT}秒）"
        echo ""
        echo "查看后端日志:"
        echo "  tail -100 /tmp/backend.log"
        exit 1
    fi
    
    # 显示进度
    echo -n "."
    sleep $COMPILE_CHECK_INTERVAL
done

# 验证后端健康状态（快速检查，因为我们已经确认启动成功）
log_info "验证后端健康状态..."
if curl -s http://localhost:3000/api/health > /dev/null 2>&1; then
    log_info "后端健康检查通过"
else
    log_error "后端健康检查失败（这不应该发生）"
    echo ""
    echo "查看后端日志:"
    echo "  tail -100 /tmp/backend.log"
    exit 1
fi

# 获取并显示认证令牌
log_info "获取认证令牌..."
GATEWAY_URL=$(grep "gateway.*http" /tmp/backend.log | tail -1 | sed 's/.*http/http/')
if [ ! -z "$GATEWAY_URL" ]; then
    log_info "网关 URL: $GATEWAY_URL"
fi

# ============================================
# 提取并导出认证令牌
# ============================================

log_section "提取认证令牌"

# 从后端日志中提取令牌
GATEWAY_URL=$(grep "gateway.*http" /tmp/backend.log | tail -1 | sed 's/.*http/http/')
if [ ! -z "$GATEWAY_URL" ]; then
    # 从 URL 中提取令牌
    export GATEWAY_AUTH_TOKEN=$(echo "$GATEWAY_URL" | sed 's/.*token=//')
    log_info "认证令牌已提取并导出到环境变量"
    log_info "GATEWAY_AUTH_TOKEN=${GATEWAY_AUTH_TOKEN:0:20}..."
else
    log_error "无法从后端日志中提取令牌"
    exit 1
fi

# ============================================
# 启动前端
# ============================================

log_section "启动前端服务"

# 获取脚本所在目录的父目录（项目根目录）
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"

cd "$PROJECT_ROOT/desktop-client/src-ui"

# 检查依赖
if [ ! -d "node_modules" ]; then
    log_info "安装前端依赖..."
    npm install
fi

log_info "启动前端..."
npm run dev &
FRONTEND_PID=$!

log_info "前端进程 PID: $FRONTEND_PID"

log_info "等待前端启动..."
sleep 5

# 检查前端是否运行
if ! kill -0 $FRONTEND_PID 2>/dev/null; then
    log_error "前端启动失败"
    exit 1
fi

log_info "前端已启动"

# ============================================
# 启动 Tauri 客户端
# ============================================

log_section "启动 Tauri 客户端"

# 回到项目根目录
cd "$PROJECT_ROOT/desktop-client"

log_info "启动 Tauri 客户端..."

# 启动 Tauri 开发服务器
# 注意: Tauri 会自动连接到前端开发服务器 (http://localhost:5173)
cargo tauri dev &
TAURI_PID=$!

log_info "Tauri 客户端进程 PID: $TAURI_PID"

log_info "等待 Tauri 客户端启动..."
sleep 10

# 检查 Tauri 是否运行
if ! kill -0 $TAURI_PID 2>/dev/null; then
    log_warn "Tauri 客户端启动失败或已关闭"
else
    log_info "Tauri 客户端已启动"
fi

log_section "启动完成"

echo ""
echo -e "${GREEN}✅ 所有服务已启动${NC}"
echo ""
echo "后端服务:"
echo "  URL: http://localhost:3000"
echo "  PID: $BACKEND_PID"
echo "  日志: tail -f /tmp/backend.log"
echo ""
echo "前端服务:"
echo "  开发服务器: http://localhost:5173"
echo "  PID: $FRONTEND_PID"
echo ""
if [ ! -z "$TAURI_PID" ] && kill -0 $TAURI_PID 2>/dev/null; then
    echo "Tauri 客户端:"
    echo "  PID: $TAURI_PID"
    echo "  状态: 运行中"
    echo "  说明: Tauri 会自动连接到前端开发服务器"
    echo ""
fi

# 提取令牌
GATEWAY_URL=$(grep "gateway.*http" /tmp/backend.log | tail -1 | sed 's/.*http/http/')
if [ ! -z "$GATEWAY_URL" ]; then
    # 从 URL 中提取令牌
    TOKEN=$(echo "$GATEWAY_URL" | sed 's/.*token=//' | sed 's/$//')
    FRONTEND_URL="http://localhost:5173?token=$TOKEN"
    echo "前端 URL（带令牌）:"
    echo "  $FRONTEND_URL"
    echo ""
    echo "💡 提示: 使用上面的 URL 访问前端，令牌会自动保存到本地存储"
    echo ""
    
    # 尝试自动打开浏览器
    if command -v open &> /dev/null; then
        echo "🌐 正在打开浏览器..."
        open "$FRONTEND_URL" 2>/dev/null || true
    fi
else
    echo "前端 URL: http://localhost:5173"
    echo ""
    echo "⚠️  无法获取令牌，请手动从后端日志中获取"
fi

echo ""
echo "停止服务: 按 Ctrl+C"
echo ""

# 等待用户中断
wait
