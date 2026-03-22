#!/bin/bash

# Admin Backend 启动脚本
# 启动 admin-backend 后端和前端服务（不含 Desktop Client）
#
# 用法:
#   ./admin-backend/scripts/start-admin.sh
#   或从项目根目录:
#   ./scripts/start-admin.sh
#
# 前置条件:
#   - Docker 已启动（PostgreSQL 容器）
#   - 如果数据库未运行，脚本会自动启动

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
ADMIN_BACKEND_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"

source "$SCRIPT_DIR/common.sh"

cleanup() {
    log_info "清理资源..."
    [ -n "${BACKEND_PID:-}" ] && kill $BACKEND_PID 2>/dev/null || true
    [ -n "${FRONTEND_PID:-}" ] && kill $FRONTEND_PID 2>/dev/null || true
    log_info "清理完成"
}
trap cleanup EXIT

# ============================================
# 检查依赖 & 清理端口
# ============================================

check_dev_dependencies
check_docker_dependencies

log_section "清理旧进程和端口"
clean_port 3000 "admin-backend"
clean_port 5174 "admin-frontend"

# ============================================
# 启动数据库
# ============================================

if ! start_postgres_db "$ADMIN_BACKEND_ROOT"; then
    exit 1
fi

# ============================================
# 启动后端
# ============================================

# start_admin_backend 通过 /tmp/admin-backend.pid 传递 PID
start_admin_backend "$ADMIN_BACKEND_ROOT"
BACKEND_PID=$(cat /tmp/admin-backend.pid 2>/dev/null || echo "")
if [ -z "$BACKEND_PID" ]; then
    exit 1
fi

# ============================================
# 启动前端
# ============================================

# start_admin_frontend 通过 /tmp/admin-frontend.pid 传递 PID
start_admin_frontend "$ADMIN_BACKEND_ROOT/frontend"
FRONTEND_PID=$(cat /tmp/admin-frontend.pid 2>/dev/null || echo "")
if [ -z "$FRONTEND_PID" ]; then
    exit 1
fi

# ============================================
# 启动完成
# ============================================

log_section "🎉 Admin Backend 启动完成"

echo ""
echo -e "${GREEN}数据库:${NC}  localhost:5432  (postgres/postgres, db: ironclaw)"
echo -e "${GREEN}后端:${NC}    http://localhost:3000  (PID: $BACKEND_PID)"
echo -e "${GREEN}前端:${NC}    http://localhost:5174  (PID: $FRONTEND_PID)"
echo ""
echo "测试账号: admin / admin123"
echo ""
echo "日志:"
echo "  tail -f /tmp/admin-backend.log"
echo "  tail -f /tmp/admin-frontend.log"
echo ""
echo "停止: Ctrl+C（数据库继续运行）"
echo "停止数据库: cd admin-backend && docker-compose down"
echo ""

if command -v open &> /dev/null; then
    open "http://localhost:5174" 2>/dev/null || true
fi

tail -f /dev/null
