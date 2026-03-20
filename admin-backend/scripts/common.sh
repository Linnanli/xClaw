#!/bin/bash

# 共享函数库
# 被 start-admin.sh、start-db.sh 和 start-all.sh 使用

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# 日志函数
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

# 检查 Docker 依赖
check_docker_dependencies() {
    log_section "检查 Docker 依赖"
    
    if ! command -v docker &> /dev/null; then
        log_error "Docker 未找到，请先安装 Docker"
        exit 1
    fi
    log_info "Docker 已安装"
    
    if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
        log_error "Docker Compose 未找到，请先安装 Docker Compose"
        exit 1
    fi
    log_info "Docker Compose 已安装"
}

# 启动 PostgreSQL 数据库
# 参数: $1 - admin-backend 目录路径
start_postgres_db() {
    local ADMIN_BACKEND_DIR="$1"
    
    log_section "启动 PostgreSQL 数据库"
    
    cd "$ADMIN_BACKEND_DIR"
    
    # 检查数据库容器是否已运行
    if docker ps | grep -q admin-backend-postgres; then
        log_info "PostgreSQL 容器已在运行"
        return 0
    fi
    
    log_info "启动 PostgreSQL 容器..."
    
    # 使用 docker compose 或 docker-compose
    if docker compose version &> /dev/null; then
        docker compose up -d postgres
    else
        docker-compose up -d postgres
    fi
    
    log_info "等待 PostgreSQL 启动..."
    
    # 等待数据库健康检查通过
    local MAX_WAIT=30
    local WAIT_COUNT=0
    echo -n "数据库启动进度: "
    while [ $WAIT_COUNT -lt $MAX_WAIT ]; do
        if docker exec admin-backend-postgres pg_isready -U postgres > /dev/null 2>&1; then
            echo ""
            log_info "PostgreSQL 已就绪"
            return 0
        fi
        echo -n "."
        sleep 1
        WAIT_COUNT=$((WAIT_COUNT + 1))
    done
    
    echo ""
    log_error "PostgreSQL 启动超时"
    echo ""
    echo "查看日志: docker logs admin-backend-postgres"
    return 1
}

# 显示数据库信息
show_database_info() {
    echo ""
    echo -e "${GREEN}✅ PostgreSQL 数据库已启动${NC}"
    echo ""
    echo "数据库信息:"
    echo "  容器: admin-backend-postgres"
    echo "  端口: localhost:5432"
    echo "  用户: postgres"
    echo "  密码: postgres"
    echo "  数据库: ironclaw"
    echo ""
    echo "连接字符串:"
    echo "  postgresql://postgres:postgres@localhost:5432/ironclaw"
    echo ""
    echo "常用命令:"
    echo "  查看日志: docker logs admin-backend-postgres"
    echo "  查看状态: docker ps | grep admin-backend-postgres"
    echo "  停止数据库: cd admin-backend && docker-compose down"
    echo "  重启数据库: cd admin-backend && docker-compose restart postgres"
    echo "  连接数据库: docker exec -it admin-backend-postgres psql -U postgres -d ironclaw"
    echo ""
}

# 检查 Rust 和 Node.js 依赖
check_dev_dependencies() {
    log_section "检查开发依赖"
    
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
}

# 清理端口
# 参数: $1 - 端口号, $2 - 服务名称
clean_port() {
    local PORT=$1
    local SERVICE_NAME=$2
    
    log_info "清理 ${PORT} 端口（${SERVICE_NAME}）..."
    lsof -i :${PORT} | grep -v COMMAND | awk '{print $2}' | xargs kill -9 2>/dev/null || true
    sleep 1
}

# 启动 Admin Backend 后端
# 参数: $1 - admin-backend 目录路径
start_admin_backend() {
    local ADMIN_BACKEND_DIR="$1"
    
    log_section "启动 Admin Backend 后端"
    
    cd "$ADMIN_BACKEND_DIR"
    
    log_info "启动后端..."
    cargo run > /tmp/admin-backend.log 2>&1 &
    local BACKEND_PID=$!
    
    log_info "后端进程 PID: $BACKEND_PID"
    
    log_info "等待后端编译和启动（这可能需要几分钟）..."
    
    # 智能等待：检查编译是否完成并且服务器启动
    local COMPILE_TIMEOUT=300  # 5 分钟超时
    local COMPILE_CHECK_INTERVAL=3
    local COMPILE_START_TIME=$(date +%s)
    
    echo -n "编译进度: "
    while true; do
        # 检查进程是否还在运行
        if ! kill -0 $BACKEND_PID 2>/dev/null; then
            echo ""
            log_error "后端进程已退出，检查编译错误"
            echo ""
            echo "查看后端日志:"
            echo "  tail -50 /tmp/admin-backend.log"
            return 1
        fi
        
        # 检查端口是否监听
        if lsof -i :3000 | grep -q LISTEN; then
            echo ""
            log_info "后端编译完成并启动成功"
            break
        fi
        
        # 检查超时
        local CURRENT_TIME=$(date +%s)
        local ELAPSED=$((CURRENT_TIME - COMPILE_START_TIME))
        if [ $ELAPSED -gt $COMPILE_TIMEOUT ]; then
            echo ""
            log_error "后端编译超时（${COMPILE_TIMEOUT}秒）"
            echo ""
            echo "查看后端日志:"
            echo "  tail -100 /tmp/admin-backend.log"
            return 1
        fi
        
        # 显示进度
        echo -n "."
        sleep $COMPILE_CHECK_INTERVAL
    done
    
    # 验证后端是否响应
    log_info "验证后端服务..."
    sleep 2
    if curl -s http://localhost:3000/health > /dev/null 2>&1; then
        log_info "后端服务验证通过"
    else
        log_warn "后端服务可能未完全启动，但端口已监听"
    fi
    
    echo "$BACKEND_PID"
}

# 启动 Admin Frontend 前端
# 参数: $1 - admin-backend/frontend 目录路径
start_admin_frontend() {
    local FRONTEND_DIR="$1"
    
    log_section "启动 Admin Frontend 前端"
    
    cd "$FRONTEND_DIR"
    
    # 检查依赖
    if [ ! -d "node_modules" ]; then
        log_info "安装前端依赖..."
        npm install
    fi
    
    log_info "启动前端..."
    npm run dev > /tmp/admin-frontend.log 2>&1 &
    local FRONTEND_PID=$!
    
    log_info "前端进程 PID: $FRONTEND_PID"
    
    log_info "等待前端启动..."
    sleep 5
    
    # 检查前端是否运行
    if ! kill -0 $FRONTEND_PID 2>/dev/null; then
        log_error "前端启动失败"
        echo ""
        echo "查看前端日志:"
        echo "  tail -50 /tmp/admin-frontend.log"
        return 1
    fi
    
    log_info "前端已启动"
    
    echo "$FRONTEND_PID"
}
