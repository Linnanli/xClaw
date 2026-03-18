#!/bin/bash

# DLP 实时测试脚本
# 用于诊断 DLP 功能是否正常工作

set -e

echo "🔍 DLP 实时测试"
echo "==============="
echo ""

# 检查后端是否运行
echo "1️⃣ 检查后端服务..."
if ! curl -s http://localhost:3000/health > /dev/null 2>&1; then
    echo "❌ 后端服务未运行"
    echo "   请先启动后端: cargo run -- run --cli-only --no-onboard"
    exit 1
fi
echo "✅ 后端服务正常"
echo ""

# 检查前端是否运行
echo "2️⃣ 检查前端服务..."
if ! curl -s http://localhost:5173 > /dev/null 2>&1; then
    echo "❌ 前端服务未运行"
    echo "   请先启动前端: cd desktop-client/src-ui && npm run dev"
    exit 1
fi
echo "✅ 前端服务正常"
echo ""

# 打开浏览器控制台测试
echo "3️⃣ 打开浏览器进行测试..."
echo ""
echo "请在浏览器控制台中运行以下命令："
echo ""
echo "// 测试 DLP 扫描"
echo "const { invoke } = window.__TAURI__.core;"
echo "await invoke('scan_user_input', { content: '我的身份证号是 330326199408015618' })"
echo ""
echo "预期结果："
echo "{"
echo "  had_sensitive_data: true,"
echo "  sanitized_content: '我的身份证号是 330************618',"
echo "  was_blocked: false,"
echo "  sanitization_stats: {"
echo "    total_matches: 1,"
echo "    redacted_count: 1,"
echo "    blocked_count: 0,"
echo "    warned_count: 0"
echo "  }"
echo "}"
echo ""
echo "如果看到错误，请检查："
echo "1. Tauri 应用是否正确启动"
echo "2. DLP 命令是否正确注册"
echo "3. 浏览器控制台是否有错误日志"
echo ""
