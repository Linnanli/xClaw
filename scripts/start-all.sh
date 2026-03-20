#!/bin/bash

# 完整开发环境启动脚本
# 
# 前置条件：
# 1. Docker 必须已手动启动（macOS: Docker Desktop, Linux: systemctl start docker）
# 2. PostgreSQL 数据库容器必须已手动启动（运行 ./admin-backend/scripts/start-db.sh）
# 
# 此脚本启动所有开发服务：
# 1. 检查 PostgreSQL 数据库（端口 5432）- 如未运行则报错
# 2. Desktop Client 前端（端口 5173）
# 3. Tauri 客户端（内嵌 IronClaw 核心服务，端口 38080）
# 4. Admin Backend 后端（端口 3000）
# 5. Admin Backend 前端（端口 5174）
#
# 启动模式：
# - 默认：并行启动（快速，但看不到编译进度）
#   ./scripts/start-all.sh
# 
# - 串行：逐个启动服务，实时显示编译进度（推荐首次运行使用）
#   ./scripts/start-all.sh --serial
#   或
#   ./scripts/start-all.sh -s

set -e

# 获取项目根目录
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"

# 检查启动模式
SERIAL_MODE=false
if [ "$1" = "--serial" ] || [ "$1" = "-s" ]; then
    SERIAL_MODE=true
fi

# 加载 Admin Backend 的共享函数
source "$PROJECT_ROOT/admin-backend/scripts/common.sh"

cleanup() {
    log_info "清理资源..."
    
    # 杀死 IronClaw 服务器进程
    if [ ! -z "$IRONCLAW_SERVER_PID" ]; then
        log_info "停止 IronClaw 服务器 (PID: $IRONCLAW_SERVER_PID)..."
        kill $IRONCLAW_SERVER_PID 2>/dev/null || true
    fi
    
    # 杀死 Desktop Client 前端进程
    if [ ! -z "$DESKTOP_FRONTEND_PID" ]; then
        log_info "停止 Desktop Client 前端 (PID: $DESKTOP_FRONTEND_PID)..."
        kill $DESKTOP_FRONTEND_PID 2>/dev/null || true
    fi
    
    # 杀死 Tauri 进程
    if [ ! -z "$TAURI_PID" ]; then
        log_info "停止 Tauri 客户端 (PID: $TAURI_PID)..."
        kill $TAURI_PID 2>/dev/null || true
    fi
    
    # 杀死 Admin Backend 后端进程
    if [ ! -z "$ADMIN_BACKEND_PID" ]; then
        log_info "停止 Admin Backend 后端 (PID: $ADMIN_BACKEND_PID)..."
        kill $ADMIN_BACKEND_PID 2>/dev/null || true
    fi
    
    # 杀死 Admin Backend 前端进程
    if [ ! -z "$ADMIN_FRONTEND_PID" ]; then
        log_info "停止 Admin Backend 前端 (PID: $ADMIN_FRONTEND_PID)..."
        kill $ADMIN_FRONTEND_PID 2>/dev/null || true
    fi
    
    # 注意：不停止数据库容器，以便保留数据
    # 如需停止数据库，请手动运行: cd admin-backend && docker-compose down
    
    log_info "清理完成"
}

trap cleanup EXIT

# ============================================
# 检查依赖
# ============================================

check_dev_dependencies
check_docker_dependencies

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
# 设置环境变量
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

clean_port 5173 "Desktop Client 前端"
clean_port 38080 "Tauri 内嵌后端"
clean_port 3000 "Admin Backend"
clean_port 5174 "Admin Frontend"
# 注意: 不清理 5432 端口，因为这是 Docker 容器的端口
# 清理容器端口可能导致 Docker daemon 不稳定

log_info "所有应用端口已清理"

# ============================================
# 检查 PostgreSQL 数据库
# ============================================

if ! check_postgres_db "$PROJECT_ROOT/admin-backend"; then
    exit 1
fi

# ============================================
# 启动外部 IronClaw 服务器（用于 Desktop Client）
# ============================================

log_section "启动外部 IronClaw 服务器"

cd "$PROJECT_ROOT"

log_info "启动 IronClaw 服务器（端口 38080）..."

# 设置环境变量
export GATEWAY_PORT=38080
export GATEWAY_HOST=127.0.0.1
export GATEWAY_ENABLED=true

# 启动 IronClaw 服务器
cargo run --manifest-path ironclaw/Cargo.toml -- run --no-onboard > /tmp/ironclaw-server.log 2>&1 &
IRONCLAW_SERVER_PID=$!

log_info "IronClaw 服务器进程 PID: $IRONCLAW_SERVER_PID"

log_info "等待 IronClaw 服务器启动..."

# 等待服务器启动（最多 60 秒）
IRONCLAW_TIMEOUT=60
IRONCLAW_CHECK_INTERVAL=2
IRONCLAW_START_TIME=$(date +%s)

echo -n "IronClaw 启动进度: "
while true; do
    # 检查进程是否还在运行
    if ! kill -0 $IRONCLAW_SERVER_PID 2>/dev/null; then
        echo ""
        log_warn "IronClaw 服务器进程已退出"
        break
    fi
    
    # 检查服务器是否启动
    if curl -s http://localhost:38080/api/health > /dev/null 2>&1; then
        echo ""
        log_info "IronClaw 服务器已启动"
        break
    fi
    
    # 检查超时
    CURRENT_TIME=$(date +%s)
    ELAPSED=$((CURRENT_TIME - IRONCLAW_START_TIME))
    if [ $ELAPSED -gt $IRONCLAW_TIMEOUT ]; then
        echo ""
        log_warn "IronClaw 服务器启动超时"
        break
    fi
    
    # 显示进度
    echo -n "."
    sleep $IRONCLAW_CHECK_INTERVAL
done

# 检查服务器是否运行
if curl -s http://localhost:38080/api/health > /dev/null 2>&1; then
    log_info "IronClaw 服务器健康检查通过"
else
    log_warn "IronClaw 服务器可能未启动，请检查日志"
    echo ""
    echo "查看日志:"
    echo "  tail -50 /tmp/ironclaw-server.log"
fi

# ============================================
# 启动 Desktop Client 前端
# ============================================

log_section "启动 Desktop Client 前端"

cd "$PROJECT_ROOT/desktop-client/src-ui"

# 检查依赖
if [ ! -d "node_modules" ]; then
    log_info "安装 Desktop Client 前端依赖..."
    npm install
fi

log_info "启动 Desktop Client 前端（端口 5173）..."
npm run dev > /tmp/desktop-frontend.log 2>&1 &
DESKTOP_FRONTEND_PID=$!

log_info "Desktop Client 前端进程 PID: $DESKTOP_FRONTEND_PID"

log_info "等待 Desktop Client 前端启动..."
sleep 5

# 检查前端是否运行
if ! kill -0 $DESKTOP_FRONTEND_PID 2>/dev/null; then
    log_error "Desktop Client 前端启动失败"
    echo ""
    echo "查看日志:"
    echo "  tail -50 /tmp/desktop-frontend.log"
    exit 1
fi

log_info "Desktop Client 前端已启动"

# ============================================
# 启动 Tauri 客户端
# ============================================

if [ "$SERIAL_MODE" = true ]; then
    # 串行模式：等待 Tauri 完全启动后再继续
    TAURI_PID=$(start_tauri_serial "$PROJECT_ROOT/desktop-client")
    if [ -z "$TAURI_PID" ]; then
        log_error "Tauri 启动失败"
        exit 1
    fi
else
    # 并行模式：后台启动 Tauri
    log_section "启动 Tauri 客户端"

    cd "$PROJECT_ROOT/desktop-client"

    log_info "启动 Tauri 客户端（内嵌 IronClaw 核心服务）..."
    log_info "内嵌后端将在端口 38080 启动"

    # 启动 Tauri 开发服务器
    # 注意: 
    # 1. Tauri 会自动连接到前端开发服务器 (http://localhost:5173)
    # 2. Tauri 会自动启动内嵌的 IronClaw 核心服务（端口 38080）
    cargo tauri dev > /tmp/tauri.log 2>&1 &
    TAURI_PID=$!

    log_info "Tauri 客户端进程 PID: $TAURI_PID"

    log_info "等待 Tauri 客户端和内嵌后端启动（这可能需要几分钟）..."

    # 智能等待 Tauri 编译完成
    TAURI_TIMEOUT=600  # 10 分钟超时
    TAURI_CHECK_INTERVAL=3
    TAURI_START_TIME=$(date +%s)

    echo -n "Tauri 编译进度: "
    while true; do
        # 检查 Tauri 是否还在运行
        if ! kill -0 $TAURI_PID 2>/dev/null; then
            echo ""
            log_warn "Tauri 客户端进程已退出"
            break
        fi
        
        # 检查内嵌后端是否启动
        if curl -s http://localhost:38080/api/health > /dev/null 2>&1; then
            echo ""
            log_info "Tauri 客户端和内嵌后端已启动"
            break
        fi
        
        # 检查超时
        CURRENT_TIME=$(date +%s)
        ELAPSED=$((CURRENT_TIME - TAURI_START_TIME))
        if [ $ELAPSED -gt $TAURI_TIMEOUT ]; then
            echo ""
            log_warn "Tauri 启动超时，继续启动其他服务"
            break
        fi
        
        # 显示进度
        echo -n "."
        sleep $TAURI_CHECK_INTERVAL
    done

    # 检查 Tauri 是否运行
    if ! kill -0 $TAURI_PID 2>/dev/null; then
        log_warn "Tauri 客户端启动失败或已关闭"
        echo ""
        echo "查看日志:"
        echo "  tail -50 /tmp/tauri.log"
    else
        # 检查内嵌后端是否运行
        if curl -s http://localhost:38080/api/health > /dev/null 2>&1; then
            log_info "内嵌 IronClaw 核心服务已启动并健康"
        else
            log_warn "内嵌后端服务可能未启动，请检查 Tauri 日志"
        fi
    fi

    # 等待 Cargo 文件锁释放
    log_info "等待 Tauri 编译完成（确保 Admin Backend 可以编译）..."

    # 智能等待：检查 Cargo 锁文件
    MAX_WAIT=60
    WAIT_COUNT=0
    echo -n "等待进度: "

    while [ $WAIT_COUNT -lt $MAX_WAIT ]; do
        # 检查是否有其他 cargo 进程在运行
        if ! pgrep -f "cargo.*tauri" > /dev/null 2>&1; then
            echo ""
            log_info "Tauri 编译已完成，可以启动 Admin Backend"
            break
        fi
        
        echo -n "."
        sleep 1
        WAIT_COUNT=$((WAIT_COUNT + 1))
    done

    if [ $WAIT_COUNT -ge $MAX_WAIT ]; then
        echo ""
        log_warn "等待超时，继续启动 Admin Backend（可能会遇到 Cargo 锁冲突）"
    fi
fi

# ============================================
# 启动 Admin Backend 后端
# ============================================

if [ "$SERIAL_MODE" = true ]; then
    # 串行模式：等待 Admin Backend 完全启动后再继续
    ADMIN_BACKEND_PID=$(start_admin_backend_serial "$PROJECT_ROOT/admin-backend")
    if [ -z "$ADMIN_BACKEND_PID" ]; then
        exit 1
    fi
else
    # 并行模式：后台启动 Admin Backend
    ADMIN_BACKEND_PID=$(start_admin_backend "$PROJECT_ROOT/admin-backend")
    if [ -z "$ADMIN_BACKEND_PID" ]; then
        exit 1
    fi
fi

# ============================================
# 启动 Admin Backend 前端
# ============================================

ADMIN_FRONTEND_PID=$(start_admin_frontend "$PROJECT_ROOT/admin-backend/frontend")
if [ -z "$ADMIN_FRONTEND_PID" ]; then
    exit 1
fi

# ============================================
# 启动完成
# ============================================

log_section "🎉 所有服务启动完成"

echo ""
echo -e "${CYAN}╔════════════════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║                         开发环境已就绪                                      ║${NC}"
echo -e "${CYAN}╚════════════════════════════════════════════════════════════════════════════╝${NC}"
echo ""

echo -e "${GREEN}📱 Desktop Client (桌面客户端)${NC}"
echo "   IronClaw 服务器:"
echo "     URL: http://localhost:38080"
echo "     PID: $IRONCLAW_SERVER_PID"
echo "     健康检查: http://localhost:38080/api/health"
echo "     日志: tail -f /tmp/ironclaw-server.log"
echo ""
echo "   前端开发服务器:"
echo "     URL: http://localhost:5173"
echo "     PID: $DESKTOP_FRONTEND_PID"
echo "     日志: tail -f /tmp/desktop-frontend.log"
echo ""
if [ ! -z "$TAURI_PID" ] && kill -0 $TAURI_PID 2>/dev/null; then
    echo "   Tauri 客户端:"
    echo "     PID: $TAURI_PID"
    echo "     状态: 运行中"
    echo "     日志: tail -f /tmp/tauri.log"
    echo ""
fi

echo -e "${GREEN}🔧 Admin Backend (管理后台)${NC}"
echo "   数据库服务:"
echo "     容器: admin-backend-postgres"
echo "     端口: localhost:5432"
echo "     用户: postgres"
echo "     密码: postgres"
echo "     数据库: ironclaw"
echo "     查看日志: docker logs admin-backend-postgres"
echo ""
echo "   后端服务:"
echo "     URL: http://localhost:3000"
echo "     PID: $ADMIN_BACKEND_PID"
echo "     日志: tail -f /tmp/admin-backend.log"
echo ""
echo "   前端服务:"
echo "     URL: http://localhost:5174"
echo "     PID: $ADMIN_FRONTEND_PID"
echo "     日志: tail -f /tmp/admin-frontend.log"
echo ""
echo "   测试账号:"
echo "     用户名: admin"
echo "     密码: admin123"
echo ""

echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo -e "${YELLOW}💡 快速访问${NC}"
echo "   Desktop Client: Tauri 窗口会自动打开"
echo "   Admin Backend:  http://localhost:5174"
echo ""
if [ "$SERIAL_MODE" = true ]; then
    echo -e "${YELLOW}🔄 启动模式${NC}"
    echo "   串行模式：已逐个启动服务并显示编译进度"
    echo "   下次可使用并行模式加快启动: ./scripts/start-all.sh"
    echo ""
fi
echo -e "${YELLOW}📊 服务架构${NC}"
echo "   Desktop Client 前端 (5173) → Tauri → IronClaw 服务器 (38080)"
echo "   Admin Backend 前端 (5174) → Admin Backend 后端 (3000) → PostgreSQL (5432)"
echo ""
echo -e "${YELLOW}🛑 停止服务${NC}"
echo "   按 Ctrl+C 停止所有服务（数据库会继续运行）"
echo "   停止数据库: cd admin-backend && docker-compose down"
echo ""
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

# 尝试自动打开 Admin Backend 浏览器
if command -v open &> /dev/null; then
    log_info "正在打开 Admin Backend 浏览器..."
    open "http://localhost:5174" 2>/dev/null || true
fi

# 等待用户中断
wait
