#!/bin/bash

# Admin Backend 启动脚本
# 启动 admin-backend 后端和前端服务

set -e

# 获取脚本所在目录
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
ADMIN_BACKEND_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"

# 加载共享函数
source "$SCRIPT_DIR/common.sh"

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
    
    # 注意：不停止数据库容器，以便保留数据
    # 如需停止数据库，请手动运行: docker-compose down
    
    log_info "清理完成"
}

trap cleanup EXIT

# ============================================
# 检查依赖
# ============================================

check_dev_dependencies
check_docker_dependencies

# ============================================
# 清理旧进程和端口
# ============================================

log_section "清理旧进程和端口"

clean_port 3000 "admin-backend"
clean_port 5174 "admin-frontend"

log_info "所有端口已清理"

# ============================================
# 启动数据库
# ============================================

if ! start_postgres_db "$ADMIN_BACKEND_ROOT"; then
    exit 1
fi

# ============================================
# 启动后端
# ============================================

BACKEND_PID=$(start_admin_backend "$ADMIN_BACKEND_ROOT")
if [ -z "$BACKEND_PID" ]; then
    exit 1
fi

# ============================================
# 启动前端
# ============================================

FRONTEND_PID=$(start_admin_frontend "$ADMIN_BACKEND_ROOT/frontend")
if [ -z "$FRONTEND_PID" ]; then
    exit 1
fi

# ============================================
# 启动完成
# ============================================

log_section "启动完成"

echo ""
echo -e "${GREEN}✅ Admin Backend 所有服务已启动${NC}"
echo ""
echo "数据库服务:"
echo "  容器: admin-backend-postgres"
echo "  端口: localhost:5432"
echo "  用户: postgres"
echo "  密码: postgres"
echo "  数据库: ironclaw"
echo "  查看日志: docker logs admin-backend-postgres"
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
echo "停止服务:"
echo "  - 按 Ctrl+C 停止后端和前端"
echo "  - 停止数据库: cd admin-backend && docker-compose down"
echo ""

# 等待用户中断
wait
