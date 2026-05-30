#!/bin/bash
# DLP E2E 测试启动脚本

set -e

echo "🚀 Starting DLP E2E Tests"
echo ""

# 检查依赖
echo "📦 Checking dependencies..."
if ! command -v cargo &> /dev/null; then
    echo "❌ cargo not found. Please install Rust."
    exit 1
fi

if ! command -v node &> /dev/null; then
    echo "❌ node not found. Please install Node.js."
    exit 1
fi

echo "✅ Dependencies OK"
echo ""

# 第一步：启动后端服务
echo "🔧 Starting backend service..."
cd "$(dirname "$0")/../../.."
cargo run -- run --cli-only --no-onboard &
BACKEND_PID=$!
echo "   Backend PID: $BACKEND_PID"

# 等待后端启动
echo "⏳ Waiting for backend to start..."
sleep 5

# 检查后端是否启动成功
if ! curl -s http://localhost:3000/health > /dev/null 2>&1; then
    echo "⚠️  Backend health check failed, but continuing..."
fi

echo "✅ Backend started"
echo ""

# 第二步：启动前端开发服务器
echo "🎨 Starting frontend dev server..."
cd desktop-client/ui
npm run dev &
FRONTEND_PID=$!
echo "   Frontend PID: $FRONTEND_PID"

# 等待前端启动
echo "⏳ Waiting for frontend to start..."
sleep 10

# 检查前端是否启动成功
if ! curl -s http://localhost:5173 > /dev/null 2>&1; then
    echo "⚠️  Frontend health check failed, but continuing..."
fi

echo "✅ Frontend started"
echo ""

# 第三步：运行 Cypress E2E 测试
echo "🧪 Running Cypress E2E tests..."
npx cypress run --spec "cypress/e2e/dlp_integration.cy.js"
TEST_EXIT_CODE=$?

echo ""
echo "📊 Test Results:"
if [ $TEST_EXIT_CODE -eq 0 ]; then
    echo "✅ All tests passed!"
else
    echo "❌ Some tests failed (exit code: $TEST_EXIT_CODE)"
fi

# 清理：停止后端和前端
echo ""
echo "🧹 Cleaning up..."
kill $BACKEND_PID 2>/dev/null || true
kill $FRONTEND_PID 2>/dev/null || true

echo "✅ Cleanup complete"
echo ""

exit $TEST_EXIT_CODE
