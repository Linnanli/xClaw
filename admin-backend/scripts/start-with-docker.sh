#!/bin/bash

# 使用 Docker 启动 PostgreSQL 并运行 admin-backend

set -e

echo "========================================="
echo "启动 Admin Backend (Docker PostgreSQL)"
echo "========================================="
echo ""

# 检查 Docker 是否安装
if ! command -v docker &> /dev/null; then
    echo "❌ 错误：未安装 Docker"
    echo ""
    echo "请安装 Docker："
    echo "  macOS: https://docs.docker.com/desktop/install/mac-install/"
    echo "  Linux: https://docs.docker.com/engine/install/"
    echo ""
    exit 1
fi

echo "✅ Docker 已安装"
echo ""

# 检查 Docker 是否运行
if ! docker info > /dev/null 2>&1; then
    echo "❌ 错误：Docker 未运行"
    echo ""
    echo "请启动 Docker Desktop"
    echo ""
    exit 1
fi

echo "✅ Docker 正在运行"
echo ""

# 启动 PostgreSQL
echo "启动 PostgreSQL 容器..."
cd "$(dirname "$0")/.."
docker-compose up -d

echo "等待 PostgreSQL 启动..."
sleep 5

# 检查 PostgreSQL 是否就绪
echo "检查 PostgreSQL 健康状态..."
for i in {1..30}; do
    if docker-compose exec -T postgres pg_isready -U postgres > /dev/null 2>&1; then
        echo "✅ PostgreSQL 已就绪"
        break
    fi
    if [ $i -eq 30 ]; then
        echo "❌ PostgreSQL 启动超时"
        docker-compose logs postgres
        exit 1
    fi
    echo "等待中... ($i/30)"
    sleep 1
done

echo ""

# 运行数据库迁移（如果需要）
echo "检查数据库表..."
TABLE_EXISTS=$(docker-compose exec -T postgres psql -U postgres -d ironclaw -tAc "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'users');")

if [ "$TABLE_EXISTS" = "f" ]; then
    echo "运行数据库迁移..."
    docker-compose exec -T postgres psql -U postgres -d ironclaw -f /docker-entrypoint-initdb.d/001_init.sql
    echo "✅ 数据库迁移完成"
else
    echo "✅ 数据库表已存在"
fi

echo ""

# 启动 admin-backend 服务
echo "启动 admin-backend 服务..."
cargo run &
BACKEND_PID=$!

echo "等待服务启动..."
sleep 10

# 检查服务是否启动
MAX_RETRIES=30
for i in $(seq 1 $MAX_RETRIES); do
    if curl -s http://localhost:3000/health > /dev/null 2>&1; then
        echo "✅ admin-backend 服务已启动"
        break
    fi
    if [ $i -eq $MAX_RETRIES ]; then
        echo "❌ admin-backend 服务启动失败"
        kill $BACKEND_PID 2>/dev/null || true
        docker-compose down
        exit 1
    fi
    echo "等待中... ($i/$MAX_RETRIES)"
    sleep 2
done

echo ""

# 创建测试用户
echo "创建测试用户..."
HTTP_CODE=$(curl -s -o /tmp/response.json -w "%{http_code}" -X POST http://localhost:3000/api/auth/register \
    -H "Content-Type: application/json" \
    -d '{
        "username": "admin",
        "email": "admin@example.com",
        "password": "admin123"
    }')

HTTP_BODY=$(cat /tmp/response.json)

if [ "$HTTP_CODE" = "201" ]; then
    echo "✅ 测试用户创建成功！"
elif echo "$HTTP_BODY" | grep -q "UserExists"; then
    echo "⚠️  用户已存在，可以直接登录"
else
    echo "⚠️  创建用户失败（可能已存在）"
    echo "响应：$HTTP_BODY"
fi

rm -f /tmp/response.json

echo ""
echo "========================================="
echo "Admin Backend 服务已就绪！"
echo "========================================="
echo ""
echo "服务地址：http://localhost:3000"
echo "健康检查：http://localhost:3000/health"
echo ""
echo "PostgreSQL 信息："
echo "  主机：localhost"
echo "  端口：5432"
echo "  用户：postgres"
echo "  密码：postgres"
echo "  数据库：ironclaw"
echo ""
echo "测试账号："
echo "  用户名：admin"
echo "  密码：admin123"
echo "  邮箱：admin@example.com"
echo ""
echo "管理命令："
echo "  查看日志：docker-compose logs -f postgres"
echo "  停止服务：docker-compose down"
echo "  重启服务：docker-compose restart"
echo "  连接数据库：docker-compose exec postgres psql -U postgres -d ironclaw"
echo ""
echo "按 Ctrl+C 停止服务"
echo ""

# 捕获中断信号
trap "echo ''; echo '停止服务...'; kill $BACKEND_PID 2>/dev/null || true; docker-compose down; echo '✅ 服务已停止'; exit 0" INT TERM

# 等待用户中断
wait $BACKEND_PID
