#!/bin/bash

# 端到端测试启动脚本
# 
# 这个脚本启动所有必要的服务并运行 E2E 测试

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 日志函数
log_info() {
    echo -e "${GREEN}✅ $1${NC}"
}

log_warn() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

log_error() {
    echo -e "${RED}❌ $1${NC}"
}

# 清理函数
cleanup() {
    log_info "Cleaning up..."
    
    # 杀死后端进程
    if [ ! -z "$BACKEND_PID" ]; then
        log_info "Stopping backend service (PID: $BACKEND_PID)..."
        kill $BACKEND_PID 2>/dev/null || true
    fi
    
    # 杀死前端进程
    if [ ! -z "$FRONTEND_PID" ]; then
        log_info "Stopping frontend dev server (PID: $FRONTEND_PID)..."
        kill $FRONTEND_PID 2>/dev/null || true
    fi
    
    # 杀死 Tauri 应用
    if [ ! -z "$TAURI_PID" ]; then
        log_info "Stopping Tauri app (PID: $TAURI_PID)..."
        kill $TAURI_PID 2>/dev/null || true
    fi
    
    log_info "Cleanup complete"
}

# 设置 trap 以确保清理
trap cleanup EXIT

# 检查依赖
check_dependencies() {
    log_info "Checking dependencies..."
    
    # 检查 Rust
    if ! command -v cargo &> /dev/null; then
        log_error "Rust/Cargo not found. Please install Rust."
        exit 1
    fi
    
    # 检查 Node.js
    if ! command -v node &> /dev/null; then
        log_error "Node.js not found. Please install Node.js."
        exit 1
    fi
    
    # 检查 npm
    if ! command -v npm &> /dev/null; then
        log_error "npm not found. Please install npm."
        exit 1
    fi
    
    log_info "All dependencies found"
}

# 启动后端服务
start_backend() {
    log_info "Starting backend service..."
    
    # 设置测试环境变量
    export ENVIRONMENT=testing
    export DATABASE_TYPE=sqlite
    export DATABASE_PATH=/tmp/test_ironclaw.db
    export LOG_LEVEL=debug
    
    # 启动后端
    cargo run -- run --cli-only --no-onboard &
    BACKEND_PID=$!
    
    log_info "Backend service started (PID: $BACKEND_PID)"
    
    # 等待后端启动
    log_info "Waiting for backend to start..."
    sleep 5
    
    # 检查后端是否运行
    if ! kill -0 $BACKEND_PID 2>/dev/null; then
        log_error "Backend failed to start"
        exit 1
    fi
    
    log_info "Backend is running"
}

# 启动前端开发服务器
start_frontend() {
    log_info "Starting frontend dev server..."
    
    cd desktop-client/ui
    
    # 安装依赖（如果需要）
    if [ ! -d "node_modules" ]; then
        log_info "Installing frontend dependencies..."
        npm install
    fi
    
    # 启动前端
    npm run dev &
    FRONTEND_PID=$!
    
    log_info "Frontend dev server started (PID: $FRONTEND_PID)"
    
    # 等待前端启动
    log_info "Waiting for frontend to start..."
    sleep 5
    
    # 检查前端是否运行
    if ! kill -0 $FRONTEND_PID 2>/dev/null; then
        log_error "Frontend failed to start"
        exit 1
    fi
    
    log_info "Frontend is running"
    
    cd ../..
}

# 启动 Tauri 应用
start_tauri() {
    log_info "Starting Tauri application..."
    
    cd desktop-client
    
    # 设置环境变量
    export ENVIRONMENT=testing
    
    # 启动 Tauri
    cargo tauri dev &
    TAURI_PID=$!
    
    log_info "Tauri application started (PID: $TAURI_PID)"
    
    # 等待 Tauri 启动
    log_info "Waiting for Tauri to start..."
    sleep 10
    
    cd ..
}

# 运行 Rust E2E 测试
run_rust_e2e_tests() {
    log_info "Running Rust E2E tests..."
    
    cd desktop-client
    
    # 运行 E2E 测试
    cargo test --test e2e_sse_browser_tests -- --nocapture --ignored
    
    if [ $? -eq 0 ]; then
        log_info "Rust E2E tests passed"
    else
        log_error "Rust E2E tests failed"
        return 1
    fi
    
    cd ..
}

# 运行 Cypress 测试
run_cypress_tests() {
    log_info "Running Cypress tests..."
    
    cd desktop-client
    
    # 检查 Cypress 是否安装
    if ! command -v npx &> /dev/null; then
        log_warn "npx not found, skipping Cypress tests"
        return 0
    fi
    
    # 运行 Cypress
    npx cypress run --spec "cypress/e2e/sse_connection.cy.js"
    
    if [ $? -eq 0 ]; then
        log_info "Cypress tests passed"
    else
        log_error "Cypress tests failed"
        return 1
    fi
    
    cd ..
}

# 主函数
main() {
    log_info "Starting E2E test suite..."
    
    # 检查依赖
    check_dependencies
    
    # 启动服务
    start_backend
    start_frontend
    
    # 可选：启动 Tauri 应用
    # start_tauri
    
    # 运行测试
    log_info "Running tests..."
    
    # 运行 Rust E2E 测试
    run_rust_e2e_tests
    RUST_RESULT=$?
    
    # 运行 Cypress 测试
    run_cypress_tests
    CYPRESS_RESULT=$?
    
    # 输出结果
    log_info "Test results:"
    
    if [ $RUST_RESULT -eq 0 ]; then
        log_info "Rust E2E tests: PASSED"
    else
        log_error "Rust E2E tests: FAILED"
    fi
    
    if [ $CYPRESS_RESULT -eq 0 ]; then
        log_info "Cypress tests: PASSED"
    else
        log_error "Cypress tests: FAILED"
    fi
    
    # 返回结果
    if [ $RUST_RESULT -eq 0 ] && [ $CYPRESS_RESULT -eq 0 ]; then
        log_info "All tests passed!"
        exit 0
    else
        log_error "Some tests failed"
        exit 1
    fi
}

# 运行主函数
main
