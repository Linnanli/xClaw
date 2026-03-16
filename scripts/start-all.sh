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

if [ -z "$LLM_API_KEY" ]; then
    log_error "LLM_API_KEY 环境变量未设置"
    echo ""
    echo "请先设置 Qwen API 密钥:"
    echo "  export LLM_API_KEY=\"sk-...\""
    echo ""
    echo "获取 API 密钥: https://dashscope.console.aliyun.com/api-key"
    exit 1
fi

log_info "LLM_API_KEY 已设置"

# ============================================
# 设置环境变量
# ============================================

log_section "设置环境变量"

export LLM_BACKEND="openai_compatible"
export LLM_BASE_URL="https://dashscope.aliyuncs.com/compatible-mode/v1"
export LLM_MODEL="qwen-max"

log_info "LLM_BACKEND=$LLM_BACKEND"
log_info "LLM_BASE_URL=$LLM_BASE_URL"
log_info "LLM_MODEL=$LLM_MODEL"
log_info "LLM_API_KEY=sk-..."

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
# 关键修复: 使用 < /dev/null 重定向 stdin 来禁用 REPL 交互模式
# 这样后端就不会等待用户输入，HTTP 服务器可以正常响应
nohup cargo run -- run --no-onboard < /dev/null > /tmp/backend.log 2>&1 &
BACKEND_PID=$!

log_info "后端进程 PID: $BACKEND_PID"

log_info "等待后端启动..."
sleep 15

# 检查后端是否运行
if ! kill -0 $BACKEND_PID 2>/dev/null; then
    log_error "后端启动失败"
    echo ""
    echo "查看后端日志:"
    echo "  tail -50 /tmp/backend.log"
    exit 1
fi

log_info "后端已启动"

# 检查后端健康状态
log_info "检查后端健康状态..."
if curl -s http://localhost:3000/api/health > /dev/null; then
    log_info "后端健康检查通过"
else
    log_error "后端健康检查失败"
    echo ""
    echo "查看后端日志:"
    echo "  tail -50 /tmp/backend.log"
    exit 1
fi

# 获取并显示认证令牌
log_info "获取认证令牌..."
GATEWAY_URL=$(grep "gateway.*http" /tmp/backend.log | tail -1 | sed 's/.*http/http/')
if [ ! -z "$GATEWAY_URL" ]; then
    log_info "网关 URL: $GATEWAY_URL"
fi

# ============================================
# 启动前端
# ============================================

log_section "启动前端服务"

cd desktop-client/src-ui

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
# 启动完成
# ============================================

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
echo "  URL: http://localhost:5173"
echo "  PID: $FRONTEND_PID"
echo ""
echo "打开浏览器访问: http://localhost:5173"
echo ""
echo "停止服务: 按 Ctrl+C"
echo ""

# 等待用户中断
wait
