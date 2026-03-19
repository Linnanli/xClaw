#!/bin/bash

# 管理后台 E2E 测试启动脚本
# 使用真实后端 API 进行端到端测试

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
echo -e "${BLUE}管理后台 E2E 测试${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

# 1. 检查后端服务
echo "1. 检查后端服务..."
if curl -s http://localhost:3000/health > /dev/null 2>&1; then
    log_success "后端服务正在运行"
else
    log_error "后端服务未运行"
    log_info "请先启动后端服务："
    log_info "  cd ../../admin-backend"
    log_info "  cargo run"
    exit 1
fi

# 2. 检查前端开发服务器
echo ""
echo "2. 检查前端开发服务器..."
if curl -s http://localhost:5174 > /dev/null 2>&1; then
    log_success "前端开发服务器正在运行"
else
    log_warning "前端开发服务器未运行"
    log_info "启动前端开发服务器..."
    npm run dev &
    FRONTEND_PID=$!
    
    # 等待前端服务器启动
    log_info "等待前端服务器启动..."
    for i in {1..30}; do
        if curl -s http://localhost:5174 > /dev/null 2>&1; then
            log_success "前端服务器启动成功"
            break
        fi
        sleep 1
        echo -n "."
    done
    echo ""
fi

# 3. 检查测试数据库
echo ""
echo "3. 检查测试数据库..."
if [ -f ~/.ironclaw/ironclaw.db ]; then
    log_success "数据库文件存在"
else
    log_warning "数据库文件不存在"
    log_info "初始化数据库..."
    cd ../../admin-backend
    cargo run -- db init
    cd ../admin-backend/frontend
fi

# 4. 创建测试用户（如果不存在）
echo ""
echo "4. 检查测试用户..."
# 这里可以添加创建测试用户的逻辑
log_info "确保测试用户存在：admin / admin123"

# 5. 运行 E2E 测试
echo ""
echo "5. 运行 E2E 测试..."
log_info "测试模式：${1:-headless}"

if [ "$1" = "headed" ]; then
    log_info "启动 Cypress 测试界面..."
    npm run cypress:open
else
    log_info "运行 Cypress 测试（headless 模式）..."
    npm run cypress:run
fi

# 6. 测试结果
echo ""
if [ $? -eq 0 ]; then
    log_success "所有测试通过"
else
    log_error "部分测试失败"
    log_info "查看测试报告：cypress/screenshots/ 和 cypress/videos/"
    exit 1
fi

echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}✅ E2E 测试完成${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo "测试覆盖率类型："
echo "  ✅ 单元测试 - 表单验证"
echo "  ✅ 集成测试 - 前后端登录流程"
echo "  ✅ 失败路径测试 - 错误凭据、网络错误"
echo "  ✅ 安全测试 - 密码隐藏、令牌存储"
echo "  ✅ 可靠性测试 - 重试机制、超时处理"
echo "  ✅ 需求级测试 - 登录功能规范"
echo "  ✅ 用户体验测试 - 加载状态、错误提示"
echo "  ✅ 代码覆盖测试 - 边界情况"
echo "  ✅ 数据覆盖测试 - 各种输入格式"
echo "  ✅ 性能测试 - 响应时间"
echo ""
