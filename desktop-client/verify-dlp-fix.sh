#!/bin/bash

# DLP 修复验证脚本
# 自动验证 DLP 功能是否正常工作

set -e

echo "🔍 DLP 修复验证"
echo "=============="
echo ""

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 检查函数
check_step() {
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✅ $1${NC}"
        return 0
    else
        echo -e "${RED}❌ $1${NC}"
        return 1
    fi
}

# 1. 检查代码修改
echo "1️⃣ 检查代码修改..."

# 检查 useAiChat.ts 是否移除了降级处理
if grep -q "allowing message to proceed (degraded mode)" desktop-client/src-ui/src/app/hooks/useAiChat.ts; then
    echo -e "${RED}❌ useAiChat.ts 仍包含降级处理逻辑${NC}"
    echo "   请确保已移除 'allowing message to proceed (degraded mode)' 相关代码"
    exit 1
else
    echo -e "${GREEN}✅ useAiChat.ts 已移除降级处理逻辑${NC}"
fi

# 检查是否添加了函数类型验证
if grep -q "typeof scanUserInput !== 'function'" desktop-client/src-ui/src/app/hooks/useAiChat.ts; then
    echo -e "${GREEN}✅ useAiChat.ts 已添加函数类型验证${NC}"
else
    echo -e "${YELLOW}⚠️  useAiChat.ts 未添加函数类型验证${NC}"
fi

# 检查 main.tsx 是否导入诊断工具
if grep -q "dlpDiagnostics" desktop-client/src-ui/src/main.tsx; then
    echo -e "${GREEN}✅ main.tsx 已导入诊断工具${NC}"
else
    echo -e "${YELLOW}⚠️  main.tsx 未导入诊断工具${NC}"
fi

echo ""

# 2. 编译检查
echo "2️⃣ 编译检查..."

cd desktop-client
if cargo build 2>&1 | grep -q "^error"; then
    echo -e "${RED}❌ 编译失败${NC}"
    exit 1
else
    echo -e "${GREEN}✅ 编译成功${NC}"
fi

cd ..
echo ""

# 3. 检查 Tauri 命令注册
echo "3️⃣ 检查 Tauri 命令注册..."

if grep -q "scan_user_input," desktop-client/src/main.rs; then
    echo -e "${GREEN}✅ scan_user_input 命令已注册${NC}"
else
    echo -e "${RED}❌ scan_user_input 命令未注册${NC}"
    exit 1
fi

if grep -q "get_dlp_statistics," desktop-client/src/main.rs; then
    echo -e "${GREEN}✅ get_dlp_statistics 命令已注册${NC}"
else
    echo -e "${RED}❌ get_dlp_statistics 命令未注册${NC}"
    exit 1
fi

echo ""

# 4. 检查 DLP 集成初始化
echo "4️⃣ 检查 DLP 集成初始化..."

if grep -q "dlp_integration: Arc<Mutex<DlpIntegration>>" desktop-client/src/commands.rs; then
    echo -e "${GREEN}✅ CommandState 包含 dlp_integration 字段${NC}"
else
    echo -e "${RED}❌ CommandState 缺少 dlp_integration 字段${NC}"
    exit 1
fi

if grep -q "DlpIntegration::with_default_config" desktop-client/src/commands.rs; then
    echo -e "${GREEN}✅ DLP 集成已初始化${NC}"
else
    echo -e "${RED}❌ DLP 集成未初始化${NC}"
    exit 1
fi

echo ""

# 5. 运行单元测试
echo "5️⃣ 运行 DLP 单元测试..."

cd desktop-client
if cargo test --lib dlp 2>&1 | grep -q "test result: ok"; then
    echo -e "${GREEN}✅ DLP 单元测试通过${NC}"
else
    echo -e "${RED}❌ DLP 单元测试失败${NC}"
    cargo test --lib dlp 2>&1 | tail -20
    exit 1
fi

cd ..
echo ""

# 总结
echo "═══════════════════════════════════════"
echo -e "${GREEN}✅ 所有检查通过！${NC}"
echo "═══════════════════════════════════════"
echo ""
echo "下一步："
echo "1. 启动后端: cd desktop-client && cargo run"
echo "2. 启动前端: cd desktop-client/src-ui && npm run dev"
echo "3. 打开浏览器: http://localhost:5173"
echo "4. 打开控制台: F12 或 Cmd+Option+I"
echo "5. 运行诊断: await window.runDlpDiagnostics()"
echo "6. 测试聊天: 输入 '我的身份证号是 330326199408015618'"
echo ""
echo "预期结果："
echo "- 控制台显示: ⚠️  Sensitive data detected, using sanitized content"
echo "- Toast 显示: 已脱敏 1 处敏感信息"
echo "- 消息显示: 330************618"
echo ""
