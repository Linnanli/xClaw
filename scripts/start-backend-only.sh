#!/bin/bash

# 仅启动后端服务的简化脚本

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

log_section() {
    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
}

cleanup() {
    log_info "清理资源..."
    if [ ! -z "$BACKEND_PID" ]; then
        log_info "停止后端服务 (PID: $BACKEND_PID)..."
        kill $BACKEND_PID 2>/dev/null || true
    fi
    log_info "清理完成"
}

trap cleanup EXIT

# ============================================
# 检查环境变量
# ============================================

log_section "检查环境变量"

# 从项目 .env 加载配置
if [ -f .env ]; then
    log_info "从项目 .env 加载配置..."
    set -a
    source .env
    set +a
fi

# 从 ~/.ironclaw/.env 加载后备配置
if [ -f ~/.ironclaw/.env ]; then
    log_info "从 ~/.ironclaw/.env 加载后备配置..."
    while IFS='=' read -r key value; do
        [[ $key =~ ^[[:space:]]*# ]] && continue
        [[ -z $key ]] && continue
        value=$(echo "$value" | sed 's/^"//;s/"$//')
        if [ -z "${!key}" ]; then
            export "$key"="$value"
        fi
    done < ~/.ironclaw/.env
fi

# 检查 LLM 配置
if [ -z "$ANTHROPIC_API_KEY" ] && [ -z "$OPENAI_API_KEY" ] && [ -z "$LLM_API_KEY" ] && [ -z "$NEARAI_API_KEY" ]; then
    log_error "未找到任何 LLM API 密钥配置"
    exit 1
fi

log_info "LLM 配置已检测"

# ============================================
# 清理旧进程和端口
# ============================================

log_section "清理旧进程和端口"

log_info "停止旧的后端进程..."
pkill -f "ironclaw.*run" || true
sleep 1

log_info "清理 3000 端口..."
lsof -i :3000 | grep -v COMMAND | awk '{print $2}' | xargs kill -9 2>/dev/null || true
sleep 1

log_info "所有端口已清理"

# ============================================
# 启动后端
# ============================================

log_section "启动后端服务"

log_info "启动后端..."
(cd ironclaw && exec cargo run -- run --no-onboard < /dev/null > /tmp/backend.log 2>&1) &
BACKEND_PID=$!

log_info "后端进程 PID: $BACKEND_PID"
log_info "等待后端编译和启动（首次编译可能需要 5-10 分钟）..."

# 智能等待
COMPILE_TIMEOUT=600
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
    
    echo -n "."
    sleep $COMPILE_CHECK_INTERVAL
done

# 验证后端健康状态
log_info "验证后端健康状态..."
if curl -s http://localhost:3000/api/health > /dev/null 2>&1; then
    log_info "后端健康检查通过"
else
    log_error "后端健康检查失败"
    exit 1
fi

# 获取并显示认证令牌
log_info "获取认证令牌..."
GATEWAY_URL=$(grep "gateway.*http" /tmp/backend.log | tail -1 | sed 's/.*http/http/')
if [ ! -z "$GATEWAY_URL" ]; then
    log_info "网关 URL: $GATEWAY_URL"
    TOKEN=$(echo "$GATEWAY_URL" | sed 's/.*token=//')
    export GATEWAY_AUTH_TOKEN="$TOKEN"
    log_info "认证令牌: ${TOKEN:0:20}..."
fi

log_section "启动完成"

echo ""
echo -e "${GREEN}✅ 后端服务已启动${NC}"
echo ""
echo "后端服务:"
echo "  URL: http://localhost:3000"
echo "  PID: $BACKEND_PID"
echo "  日志: tail -f /tmp/backend.log"
echo ""
echo "停止服务: 按 Ctrl+C"
echo ""

# 等待用户中断
wait
