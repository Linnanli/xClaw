#!/bin/bash

# 创建测试用户脚本
# 用户名：admin
# 密码：admin123

set -e

echo "创建测试用户..."

# 检查后端服务是否运行
echo "检查 admin-backend 服务..."
if ! curl -s http://localhost:3000/health > /dev/null 2>&1; then
    echo "❌ 错误：admin-backend 服务未运行"
    echo ""
    echo "请先启动服务："
    echo "  cd admin-backend"
    echo "  cargo run"
    echo ""
    exit 1
fi

echo "✅ admin-backend 服务正在运行"
echo ""

# 创建用户
echo "调用 API 创建用户..."
HTTP_CODE=$(curl -s -o /tmp/response.json -w "%{http_code}" -X POST http://localhost:3000/api/auth/register \
    -H "Content-Type: application/json" \
    -d '{
        "username": "admin",
        "email": "admin@example.com",
        "password": "admin123"
    }')

HTTP_BODY=$(cat /tmp/response.json)

echo "HTTP 状态码：$HTTP_CODE"
echo "响应体：$HTTP_BODY"
echo ""

# 检查是否成功
if [ "$HTTP_CODE" = "201" ]; then
    echo "✅ 测试用户创建成功！"
    echo ""
    echo "登录信息："
    echo "  用户名：admin"
    echo "  密码：admin123"
    echo "  邮箱：admin@example.com"
    echo ""
    echo "现在可以访问前端登录页面："
    echo "  http://localhost:5174"
elif echo "$HTTP_BODY" | grep -q "UserExists"; then
    echo "⚠️  用户已存在，可以直接登录"
    echo ""
    echo "登录信息："
    echo "  用户名：admin"
    echo "  密码：admin123"
    echo ""
    echo "现在可以访问前端登录页面："
    echo "  http://localhost:5174"
else
    echo "❌ 创建用户失败"
    echo "HTTP 状态码：$HTTP_CODE"
    echo "响应：$HTTP_BODY"
    exit 1
fi

# 清理临时文件
rm -f /tmp/response.json
