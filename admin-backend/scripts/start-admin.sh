#!/bin/bash

# Admin Backend 启动脚本
# 启动 admin-backend 后端和前端服务

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
# 清理旧进程和端口
# ============================================

log_section "清理旧进程和端口"

log_info "清理 3000 端口（admin-backend）..."
lsof -i :3000 | grep -v COMMAND | awk '{print $2}' | xargs kill -9 2>/dev/null || true
sleep 1

log_info "清理 5174 端口（admin-frontend）..."
lsof -i :5174 | grep -v COMMAND | awk '{print $2}' | xargs kill -9 2>/dev/null || true
sleep 1

log_info "所有端口已清理"

# ============================================
# 获取脚本所在目录
# ============================================

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
ADMIN_BACKEND_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"

# ============================================
# 启动后端
# ============================================

log_section "启动 Admin Backend 服务"

cd "$ADMIN_BACKEND_ROOT"

log_info "启动后端..."
cargo run > /tmp/admin-backend.log 2>&1 &
BACKEND_PID=$!

log_info "后端进程 PID: $BACKEND_PID"

log_info "等待后端编译和启动（这可能需要几分钟）..."

# 智能等待：检查编译是否完成并且服务器启动
COMPILE_TIMEOUT=300  # 5 分钟超时
COMPILE_CHECK_INTERVAL=3
COMPILE_START_TIME=$(date +%s)

echo -n "编译进度: "
while true; do
    # 检查进程是否还在运行
    if ! kill -0 $BACKEND_PID 2>/dev/null; then
        echo ""
        log_error "后端进程已退出，检查编译错误"
        echo ""
        echo "查看后端日志:"
        echo "  tail -50 /tmp/admin-backend.log"
        exit 1
    fi
    
    # 检查端口是否监听
    if lsof -i :3000 | grep -q LISTEN; then
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
        echo "  tail -100 /tmp/admin-backend.log"
        exit 1
    fi
    
    # 显示进度
    echo -n "."
    sleep $COMPILE_CHECK_INTERVAL
done

# 验证后端是否响应
log_info "验证后端服务..."
sleep 2
if curl -s http://localhost:3000/api/auth/login -X POST \
    -H "Content-Type: application/json" \
    -d '{"username":"test","password":"test"}' > /dev/null 2>&1; then
    log_info "后端服务验证通过"
else
    log_warn "后端服务可能未完全启动，但端口已监听"
fi

# ============================================
# 启动前端
# ============================================

log_section "启动 Admin Frontend 服务"

cd "$ADMIN_BACKEND_ROOT/frontend"

# 检查依赖
if [ ! -d "node_modules" ]; then
    log_info "安装前端依赖..."
    npm install
fi

log_info "启动前端..."
npm run dev > /tmp/admin-frontend.log 2>&1 &
FRONTEND_PID=$!

log_info "前端进程 PID: $FRONTEND_PID"

log_info "等待前端启动..."
sleep 5

# 检查前端是否运行
if ! kill -0 $FRONTEND_PID 2>/dev/null; then
    log_error "前端启动失败"
    echo ""
    echo "查看前端日志:"
    echo "  tail -50 /tmp/admin-frontend.log"
    exit 1
fi

log_info "前端已启动"

# ============================================
# 启动完成
# ============================================

log_section "启动完成"

echo ""
echo -e "${GREEN}✅ Admin Backend 所有服务已启动${NC}"
echo ""
echo "后端服务:"
echo "  URL: http://localhost:3000"
echo "  PID: $BACKEND_PID"
echo "  日志: tail -f /tmp/admin-backend.log"
echo ""
echo "前端服务:"
echo "  URL: http://localhost:5174"
echo "  PID: $FRONTEND_PID"
echo "  日志: tail -f /tmp/admin-frontend.log"
echo ""
echo "测试账号:"
echo "  用户名: admin"
echo "  密码: admin123"
echo ""
echo "💡 提示: 打开浏览器访问 http://localhost:5174"
echo ""

# 尝试自动打开浏览器
if command -v open &> /dev/null; then
    echo "🌐 正在打开浏览器..."
    open "http://localhost:5174" 2>/dev/null || true
fi

echo ""
echo "停止服务: 按 Ctrl+C"
echo ""

# 等待用户中断
wait
