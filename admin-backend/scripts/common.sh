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
    
    # 检查 Docker daemon 是否运行（使用 docker info 更可靠）
    if ! docker info &> /dev/null; then
        log_error "Docker daemon 未运行或无法连接"
        echo ""
        echo "请先手动启动 Docker:"
        echo ""
        if [[ "$OSTYPE" == "darwin"* ]]; then
            echo "  macOS: 打开 Applications 文件夹，启动 Docker Desktop 应用"
            echo "        或使用命令: open -a Docker"
            echo ""
            echo "  如果 Docker Desktop 已打开但仍报错，请完全重启："
            echo "    osascript -e 'quit app \"Docker\"'"
            echo "    sleep 5"
            echo "    open -a Docker"
            echo "    sleep 30  # 等待 Docker Engine 启动"
        elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
            echo "  Linux: sudo systemctl start docker"
        else
            echo "  Windows: 打开开始菜单，搜索并启动 Docker Desktop"
        fi
        echo ""
        echo "启动 Docker 后，请重新运行此脚本"
        echo ""
        echo "详细修复指南: cat DOCKER_CONNECTION_FIX.md"
        echo ""
        exit 1
    fi
    
    log_info "Docker daemon 正在运行"
    
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
    
    # 先检查 Docker daemon 是否可用
    if ! docker info > /dev/null 2>&1; then
        log_error "Docker daemon 不可用或已断开连接"
        echo ""
        echo "请先确保 Docker Desktop 正常运行："
        echo "  ./scripts/check-docker-stability.sh"
        echo ""
        return 1
    fi
    
    # 检查数据库容器是否已运行
    if docker ps 2>/dev/null | grep -q admin-backend-postgres; then
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

# 检查 PostgreSQL 数据库是否运行
# 参数: $1 - admin-backend 目录路径
check_postgres_db() {
    local ADMIN_BACKEND_DIR="$1"
    
    log_section "检查 PostgreSQL 数据库"
    
    cd "$ADMIN_BACKEND_DIR"
    
    # 先检查 Docker daemon 是否可用
    if ! docker info > /dev/null 2>&1; then
        log_error "Docker daemon 不可用或已断开连接"
        echo ""
        echo "Docker daemon 似乎不稳定，请尝试："
        echo ""
        echo "1. 完全重启 Docker Desktop："
        echo "   osascript -e 'quit app \"Docker\"'"
        echo "   sleep 5"
        echo "   open -a Docker"
        echo "   sleep 30"
        echo ""
        echo "2. 或查看详细修复指南："
        echo "   cat DOCKER_CONNECTION_FIX.md"
        echo ""
        return 1
    fi
    
    # 检查数据库容器是否已运行
    if docker ps 2>/dev/null | grep -q admin-backend-postgres; then
        log_info "PostgreSQL 容器已在运行"
        
        # 验证数据库是否就绪
        if docker exec admin-backend-postgres pg_isready -U postgres > /dev/null 2>&1; then
            log_info "PostgreSQL 已就绪"
            return 0
        else
            log_error "PostgreSQL 容器运行中但未就绪"
            echo ""
            echo "查看日志: docker logs admin-backend-postgres"
            return 1
        fi
    fi
    
    # 数据库未运行，报错并提示手动启动
    log_error "PostgreSQL 容器未运行"
    echo ""
    echo "请先手动启动 PostgreSQL 数据库:"
    echo ""
    echo "  cd admin-backend && docker-compose up -d postgres"
    echo ""
    echo "或使用专用脚本:"
    echo ""
    echo "  ./admin-backend/scripts/start-db.sh"
    echo ""
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
    
    # 获取使用该端口的进程
    local PIDS=$(lsof -ti :${PORT} 2>/dev/null || true)
    
    if [ -z "$PIDS" ]; then
        # 端口未被占用
        return 0
    fi
    
    # 检查是否是 Docker 相关进程
    for PID in $PIDS; do
        local PROCESS_NAME=$(ps -p $PID -o comm= 2>/dev/null || true)
        
        # 跳过 Docker 相关进程
        if echo "$PROCESS_NAME" | grep -qi "docker\|com.docker"; then
            log_warn "跳过 Docker 进程 (PID: $PID, 进程: $PROCESS_NAME)"
            continue
        fi
        
        # 杀死非 Docker 进程
        log_info "终止进程 PID: $PID ($PROCESS_NAME)"
        kill -9 $PID 2>/dev/null || true
    done
    
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
    
    # 将 PID 写入文件，供外部脚本读取
    echo "$BACKEND_PID" > /tmp/admin-backend.pid
    echo "$BACKEND_PID"
}

# 启动 Admin Frontend 前端
# 参数: $1 - admin-backend/ui 目录路径
# PID 通过 /tmp/admin-frontend.pid 文件传递，不通过 stdout（避免彩色日志污染捕获）
start_admin_frontend() {
    local FRONTEND_DIR="$1"
    
    log_section "启动 Admin Frontend 前端"
    
    cd "$FRONTEND_DIR"
    
    # 检查依赖
    if [ ! -d "node_modules" ]; then
        log_info "安装前端依赖..."
        npm install
    fi
    
    log_info "启动前端 dev server..."
    npm run dev > /tmp/admin-frontend.log 2>&1 &
    local FRONTEND_PID=$!
    
    # 将 PID 写入文件（不通过 stdout，避免被调用方的命令替换捕获）
    echo "$FRONTEND_PID" > /tmp/admin-frontend.pid
    
    log_info "前端进程 PID: $FRONTEND_PID"
    log_info "等待 http://localhost:5174 就绪（最多 60 秒）..."
    
    local READY=false
    for i in $(seq 1 60); do
        # 检查进程是否还活着
        if ! kill -0 "$FRONTEND_PID" 2>/dev/null; then
            log_error "前端 dev server 意外退出，查看日志: tail -30 /tmp/admin-frontend.log"
            return 1
        fi
        # 检查端口是否就绪
        if curl -sf http://localhost:5174 > /dev/null 2>&1; then
            log_info "Admin Frontend 已就绪 (${i}s)"
            READY=true
            break
        fi
        sleep 1
    done
    
    if [ "$READY" = false ]; then
        log_error "Admin Frontend 启动超时（60s），查看日志: tail -30 /tmp/admin-frontend.log"
        return 1
    fi
    
    log_info "Admin Frontend 已启动: http://localhost:5174"
}


# ============================================
# 串行启动模式函数
# ============================================

# 启动 Tauri 并等待编译完成（串行模式）
# 参数: $1 - desktop-client 目录路径
start_tauri_serial() {
    local DESKTOP_CLIENT_DIR="$1"
    
    log_section "启动 Tauri 客户端（串行模式 - 实时显示编译进度）"
    
    # ── 第一步：启动前端 dev server ──────────────────────────────
    log_info "启动前端 dev server..."
    (cd "$DESKTOP_CLIENT_DIR/src-ui" && npm run dev) > /tmp/desktop-frontend.log 2>&1 &
    local FRONTEND_PID=$!

    log_info "等待前端 http://localhost:5173 就绪..."
    local FRONTEND_READY=false
    for i in $(seq 1 30); do
        if curl -sf http://localhost:5173 > /dev/null 2>&1; then
            log_info "前端已就绪 (${i}s)"
            FRONTEND_READY=true
            break
        fi
        if ! kill -0 "$FRONTEND_PID" 2>/dev/null; then
            log_error "前端 dev server 意外退出，查看日志: tail -20 /tmp/desktop-frontend.log"
            return 1
        fi
        sleep 1
    done

    if [ "$FRONTEND_READY" = false ]; then
        log_error "前端 dev server 启动超时（30s）"
        kill "$FRONTEND_PID" 2>/dev/null || true
        return 1
    fi

    # ── 第二步：启动 Tauri ────────────────────────────────────────
    cd "$DESKTOP_CLIENT_DIR"
    
    log_info "启动 Tauri 客户端（IronClaw 引擎嵌入式运行）..."
    echo ""
    
    cargo tauri dev > /tmp/tauri.log 2>&1 &
    local TAURI_PID=$!
    
    # 将 PID 写入文件，供外部脚本读取（避免 stdout 捕获污染）
    echo "$TAURI_PID" > /tmp/tauri.pid
    
    # 同时记录前端 PID，供 cleanup 使用
    echo "$FRONTEND_PID" > /tmp/desktop-frontend.pid
    
    log_info "Tauri 客户端进程 PID: $TAURI_PID"
    log_info "正在编译，实时显示进度..."
    echo ""
    
    # 等待编译开始
    sleep 2
    
    # 实时显示编译进度
    local TIMEOUT=600  # 10 分钟超时
    local START_TIME=$(date +%s)
    local LAST_LINE=""
    
    while true; do
        # 嵌入式模式：通过日志判断引擎是否就绪（最可靠，不依赖进程 PID）
        if grep -q "Agent ironclaw ready and listening" /tmp/tauri.log 2>/dev/null; then
            echo ""
            log_info "✅ Tauri 客户端和 IronClaw 引擎已就绪"
            echo "$TAURI_PID"
            return 0
        fi

        # 检查编译是否失败
        if grep -qE "^error\[" /tmp/tauri.log 2>/dev/null; then
            echo ""
            log_error "Tauri 编译失败，查看日志: tail -50 /tmp/tauri.log"
            return 1
        fi
        
        # 显示最新的编译进度
        local NEW_LINE=$(tail -1 /tmp/tauri.log 2>/dev/null)
        if [ "$NEW_LINE" != "$LAST_LINE" ]; then
            if echo "$NEW_LINE" | grep -q "Compiling\|Building\|Finished"; then
                echo "$NEW_LINE"
                LAST_LINE="$NEW_LINE"
            fi
        fi
        
        # 检查超时
        local CURRENT_TIME=$(date +%s)
        local ELAPSED=$((CURRENT_TIME - START_TIME))
        if [ $ELAPSED -gt $TIMEOUT ]; then
            echo ""
            log_error "Tauri 启动超时（${TIMEOUT}秒）"
            return 1
        fi
        
        sleep 1
    done
}

# 启动 Admin Backend 并等待编译完成（串行模式）
# 参数: $1 - admin-backend 目录路径
start_admin_backend_serial() {
    local ADMIN_BACKEND_DIR="$1"
    
    log_section "启动 Admin Backend 后端（串行模式 - 实时显示编译进度）"
    
    cd "$ADMIN_BACKEND_DIR"
    
    log_info "启动后端..."
    echo ""
    
    cargo run > /tmp/admin-backend.log 2>&1 &
    local BACKEND_PID=$!
    
    # 将 PID 写入文件，供外部脚本读取
    echo "$BACKEND_PID" > /tmp/admin-backend.pid
    
    log_info "后端进程 PID: $BACKEND_PID"
    log_info "正在编译，实时显示进度..."
    echo ""
    
    # 等待编译开始
    sleep 2
    
    # 实时显示编译进度
    local TIMEOUT=300  # 5 分钟超时
    local START_TIME=$(date +%s)
    local LAST_LINE=""
    
    while true; do
        # 检查进程是否还在运行
        if ! kill -0 $BACKEND_PID 2>/dev/null; then
            echo ""
            log_error "后端进程已退出，检查编译错误"
            return 1
        fi
        
        # 检查端口是否监听
        if lsof -i :3000 | grep -q LISTEN; then
            echo ""
            log_info "✅ 后端编译完成并启动成功"
            echo "$BACKEND_PID"
            return 0
        fi
        
        # 显示最新的编译进度
        local NEW_LINE=$(tail -1 /tmp/admin-backend.log 2>/dev/null)
        if [ "$NEW_LINE" != "$LAST_LINE" ]; then
            if echo "$NEW_LINE" | grep -q "Compiling\|Building\|Finished"; then
                echo "$NEW_LINE"
                LAST_LINE="$NEW_LINE"
            fi
        fi
        
        # 检查超时
        local CURRENT_TIME=$(date +%s)
        local ELAPSED=$((CURRENT_TIME - START_TIME))
        if [ $ELAPSED -gt $TIMEOUT ]; then
            echo ""
            log_error "后端编译超时（${TIMEOUT}秒）"
            return 1
        fi
        
        sleep 1
    done
}
