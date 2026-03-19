#!/bin/bash

# 启动 admin-backend 服务并创建测试用户

set -e

echo "========================================="
echo "启动 Admin Backend 服务"
echo "========================================="
echo ""

# 检查 PostgreSQL 是否运行
echo "检查 PostgreSQL..."
if command -v pg_isready &> /dev/null; then
    if pg_isready -h localhost -p 5432 > /dev/null 2>&1; then
        echo "✅ PostgreSQL 正在运行"
    else
        echo "❌ PostgreSQL 未运行"
        echo ""
        echo "请启动 PostgreSQL："
        echo "  macOS: brew services start postgresql"
        echo "  Linux: sudo systemctl start postgresql"
        echo ""
        exit 1
    fi
else
    echo "⚠️  未找到 pg_isready 命令，跳过 PostgreSQL 检查"
fi

echo ""

# 检查数据库是否存在
echo "检查数据库..."
if command -v psql &> /dev/null; then
    if psql -h localhost -U postgres -lqt | cut -d \| -f 1 | grep -qw ironclaw; then
        echo "✅ 数据库 ironclaw 已存在"
    else
        echo "⚠️  数据库 ironclaw 不存在，正在创建..."
        createdb -h localhost -U postgres ironclaw || {
            echo "❌ 创建数据库失败"
            echo "请手动创建："
            echo "  createdb -h localhost -U postgres ironclaw"
            exit 1
        }
        echo "✅ 数据库创建成功"
    fi
fi

echo ""

# 运行数据库迁移
echo "运行数据库迁移..."
cd admin-backend
if [ -f "migrations/001_init.sql" ]; then
    psql -h localhost -U postgres -d ironclaw -f migrations/001_init.sql > /dev/null 2>&1 || {
        echo "⚠️  迁移可能已运行或失败（可忽略）"
    }
    echo "✅ 数据库迁移完成"
fi

echo ""

# 启动服务
echo "启动 admin-backend 服务..."
cargo run &
BACKEND_PID=$!

echo "等待服务启动..."
sleep 10

# 检查服务是否启动
if curl -s http://localhost:3000/health > /dev/null 2>&1; then
    echo "✅ admin-backend 服务已启动"
else
    echo "❌ admin-backend 服务启动失败"
    kill $BACKEND_PID 2>/dev/null || true
    exit 1
fi

echo ""

# 创建测试用户
echo "创建测试用户..."
./scripts/create_test_user.sh || {
    echo "⚠️  用户创建失败或已存在"
}

echo ""
echo "========================================="
echo "Admin Backend 服务已就绪！"
echo "========================================="
echo ""
echo "服务地址：http://localhost:3000"
echo "健康检查：http://localhost:3000/health"
echo ""
echo "测试账号："
echo "  用户名：admin"
echo "  密码：admin123"
echo ""
echo "按 Ctrl+C 停止服务"
echo ""

# 等待用户中断
wait $BACKEND_PID
