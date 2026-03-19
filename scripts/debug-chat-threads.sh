#!/bin/bash
# 诊断 /api/chat/threads 接口问题

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

log_error() {
    echo -e "${RED}❌ $1${NC}"
}

log_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

log_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}诊断 /api/chat/threads 接口${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

# 1. 检查后端进程
echo "1. 检查后端进程..."
if ps aux | grep -q "[i]ronclaw.*run"; then
    log_success "后端进程正在运行"
    PID=$(ps aux | grep "[i]ronclaw.*run" | awk '{print $2}')
    log_info "进程 PID: $PID"
else
    log_error "后端进程未运行"
    log_info "启动命令：./scripts/start-backend-only.sh"
    exit 1
fi

# 2. 检查端口
echo ""
echo "2. 检查端口 3000..."
if lsof -i :3000 > /dev/null 2>&1; then
    log_success "端口 3000 正在监听"
else
    log_error "端口 3000 未监听"
    log_info "后端可能正在编译中，请等待..."
    exit 1
fi

# 3. 检查健康端点
echo ""
echo "3. 检查健康端点..."
HEALTH_RESPONSE=$(curl -s http://localhost:3000/api/health 2>&1)
if echo "$HEALTH_RESPONSE" | grep -q "healthy"; then
    log_success "健康检查通过"
else
    log_error "健康检查失败"
    log_info "响应：$HEALTH_RESPONSE"
    exit 1
fi

# 4. 检查数据库
echo ""
echo "4. 检查数据库..."
if [ -f ~/.ironclaw/ironclaw.db ]; then
    log_success "数据库文件存在"
    COUNT=$(sqlite3 ~/.ironclaw/ironclaw.db "SELECT COUNT(*) FROM conversations" 2>/dev/null || echo "0")
    log_info "对话数量：$COUNT"
else
    log_error "数据库文件不存在"
    log_info "初始化命令：cd ironclaw && cargo run -- db init"
    exit 1
fi

# 5. 获取认证令牌
echo ""
echo "5. 获取认证令牌..."
TOKEN=$(sqlite3 ~/.ironclaw/ironclaw.db "SELECT value FROM settings WHERE key = 'channels.gateway_auth_token'" 2>/dev/null | tr -d '"' | tr -d '\n' | tr -d '\r')

if [ -z "$TOKEN" ]; then
    log_error "未找到认证令牌"
    log_info "尝试从后端日志获取..."
    
    if [ -f /tmp/backend.log ]; then
        TOKEN_FROM_LOG=$(grep "gateway.*http" /tmp/backend.log | tail -1 | sed 's/.*token=//' | tr -d '\n' | tr -d '\r')
        if [ ! -z "$TOKEN_FROM_LOG" ]; then
            TOKEN="$TOKEN_FROM_LOG"
            log_success "从日志获取到令牌"
        else
            log_error "日志中也未找到令牌"
            log_info "检查后端日志：tail -f /tmp/backend.log"
            exit 1
        fi
    else
        log_error "后端日志文件不存在"
        exit 1
    fi
else
    log_success "认证令牌已获取"
fi

log_info "令牌前缀：${TOKEN:0:20}..."

# 6. 测试 API
echo ""
echo "6. 测试 /api/chat/threads 接口..."

# 使用临时文件存储响应
TEMP_RESPONSE=$(mktemp)
HTTP_CODE=$(curl -s -w "%{http_code}" -o "$TEMP_RESPONSE" \
    -H "Authorization: Bearer $TOKEN" \
    http://localhost:3000/api/chat/threads)

BODY=$(cat "$TEMP_RESPONSE")
rm "$TEMP_RESPONSE"

if [ "$HTTP_CODE" = "200" ]; then
    log_success "API 调用成功"
    echo ""
    echo -e "${BLUE}响应内容：${NC}"
    echo "$BODY" | jq . 2>/dev/null || echo "$BODY"
else
    log_error "API 调用失败"
    log_info "HTTP 状态码：$HTTP_CODE"
    echo ""
    echo -e "${BLUE}响应内容：${NC}"
    echo "$BODY" | jq . 2>/dev/null || echo "$BODY"
    
    echo ""
    echo -e "${YELLOW}可能的原因：${NC}"
    case "$HTTP_CODE" in
        401)
            echo "  - 认证令牌无效或已过期"
            echo "  - 解决方案：重新启动后端服务"
            ;;
        403)
            echo "  - 权限不足"
            echo "  - 解决方案：检查用户权限配置"
            ;;
        500)
            echo "  - 服务器内部错误"
            echo "  - 解决方案：检查后端日志 (tail -f /tmp/backend.log)"
            ;;
        503)
            echo "  - 服务不可用（Session Manager 未初始化）"
            echo "  - 解决方案：确保使用 --no-onboard 启动参数"
            ;;
        *)
            echo "  - 未知错误"
            echo "  - 解决方案：检查后端日志"
            ;;
    esac
    
    exit 1
fi

echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}✅ 诊断完成 - 所有检查通过${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo "如果仍有问题，请查看："
echo "  - 后端日志：tail -f /tmp/backend.log"
echo "  - 诊断文档：docs/CHAT_THREADS_API_DEBUG.md"
echo ""
