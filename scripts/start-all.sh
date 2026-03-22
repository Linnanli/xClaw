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
# 3. Tauri 客户端（IronClaw 引擎嵌入式运行，无独立端口）
# 4. Admin Backend 后端（端口 3000）
# 5. Admin Backend 前端（端口 5174）
#
# 架构说明：
# - Desktop Client 使用嵌入式模式：IronClaw 引擎直接运行在 Tauri 进程内
# - 前端通过 Tauri IPC 与引擎通信，不再需要外部 IronClaw 服务器（端口 38080）
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
    
    # 杀死 Desktop Client 前端进程
    if [ ! -z "$DESKTOP_FRONTEND_PID" ]; then
        log_info "停止 Desktop Client 前端 (PID: $DESKTOP_FRONTEND_PID)..."
        kill $DESKTOP_FRONTEND_PID 2>/dev/null || true
    fi
    
    # 杀死 Tauri 进程（IronClaw 引擎随之停止）
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

# Desktop Client 的 LLM 配置由 main.rs 自行加载（desktop-client/.env + admin_config.json）。
# start-all.sh 只需要为 Admin Backend 和 IronClaw CLI 加载项目根 .env。
if [ -f .env ]; then
    log_info "从项目 .env 加载配置..."
    set -a
    source .env
    set +a
fi

# ⚠️ 不再从 ~/.ironclaw/.env 加载配置
# Desktop Client 使用独立的配置隔离机制（IRONCLAW_BASE_DIR 重定向）

# 检查是否有有效的 LLM 配置（Desktop Client 可通过 admin_config.json 获取，此处仅检查项目级配置）
APP_DATA_DIR="${HOME}/Library/Application Support/ironclaw-desktop"
HAS_ADMIN_CONFIG=false
[ -f "$APP_DATA_DIR/admin_config.json" ] && HAS_ADMIN_CONFIG=true

if [ -z "${ANTHROPIC_API_KEY:-}" ] && [ -z "${OPENAI_API_KEY:-}" ] && [ -z "${LLM_API_KEY:-}" ] && [ -z "${NEARAI_API_KEY:-}" ]; then
    if [ "$HAS_ADMIN_CONFIG" = true ]; then
        log_info "LLM 配置将由管理端下发 (admin_config.json)"
    else
        log_warn "未找到 LLM API 密钥（Desktop Client 可能无法正常工作）"
        echo ""
        echo "配置方式:"
        echo "  1. 编辑 desktop-client/.env 设置 LLM_BACKEND + LLM_API_KEY"
        echo "  2. 或等待管理端下发配置"
        echo "  3. 或设置环境变量: export LLM_API_KEY=\"sk-...\""
        echo ""
        echo "继续启动其他服务..."
    fi
else
    log_info "LLM 配置已检测"
fi

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
clean_port 3000 "Admin Backend"
clean_port 5174 "Admin Frontend"
# 注意: 不清理 5432 端口，因为这是 Docker 容器的端口
# 嵌入式模式下 38080 端口不再使用

log_info "所有应用端口已清理"

# ============================================
# 检查 PostgreSQL 数据库
# ============================================

if ! check_postgres_db "$PROJECT_ROOT/admin-backend"; then
    exit 1
fi

# ============================================
# Desktop Client 使用嵌入式模式，无需外部 IronClaw 服务器
# IronClaw 引擎直接运行在 Tauri 进程内，通过 Tauri IPC 通信
# ============================================

log_info "Desktop Client 使用嵌入式模式，跳过外部 IronClaw 服务器启动"

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

    log_info "启动 Tauri 客户端（IronClaw 引擎嵌入式运行）..."

    # 启动 Tauri 开发服务器
    # IronClaw 引擎在 Tauri 进程内启动，通过 Tauri IPC 与前端通信
    cargo tauri dev > /tmp/tauri.log 2>&1 &
    TAURI_PID=$!

    log_info "Tauri 客户端进程 PID: $TAURI_PID"
    log_info "等待 Tauri 客户端编译启动（首次编译可能需要几分钟）..."

    # 等待 Tauri 进程存活并完成初始编译
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

        # 检查超时
        CURRENT_TIME=$(date +%s)
        ELAPSED=$((CURRENT_TIME - TAURI_START_TIME))
        if [ $ELAPSED -gt $TAURI_TIMEOUT ]; then
            echo ""
            log_warn "Tauri 启动超时，继续启动其他服务"
            break
        fi

        # 嵌入式模式：通过日志判断引擎是否就绪
        if grep -q "AppState injected into Tauri" /tmp/tauri.log 2>/dev/null; then
            echo ""
            log_info "Tauri 客户端和 IronClaw 引擎已就绪"
            break
        fi

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
        log_info "Tauri 客户端运行中（IronClaw 引擎嵌入式）"
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
echo "   前端开发服务器:"
echo "     URL: http://localhost:5173"
echo "     PID: $DESKTOP_FRONTEND_PID"
echo "     日志: tail -f /tmp/desktop-frontend.log"
echo ""
if [ ! -z "$TAURI_PID" ] && kill -0 $TAURI_PID 2>/dev/null; then
    echo "   Tauri 客户端（IronClaw 嵌入式引擎）:"
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
echo "   Desktop Client 前端 (5173) → Tauri IPC → IronClaw 引擎（嵌入式）"
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
