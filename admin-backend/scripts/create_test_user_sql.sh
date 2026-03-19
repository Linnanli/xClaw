#!/bin/bash

# 使用 SQL 直接创建测试用户
# 用户名：admin
# 密码：admin123

set -e

echo "使用 SQL 创建测试用户..."

# 检查数据库文件
DB_FILE="admin.db"

if [ ! -f "$DB_FILE" ]; then
    echo "数据库文件不存在，将在首次运行时创建"
fi

# 生成 UUID
USER_ID=$(uuidgen | tr '[:upper:]' '[:lower:]')

# 生成密码哈希（使用 Argon2）
# 注意：这里使用预先计算的哈希值
# 密码：admin123
# 使用 Argon2id 算法
PASSWORD_HASH='$argon2id$v=19$m=19456,t=2,p=1$aGVsbG93b3JsZA$+r0d29hqEB0yasKr55ZgICsQGSkl0v0kgwhd+U3wyRo'

# 当前时间
TIMESTAMP=$(date -u +"%Y-%m-%d %H:%M:%S")

# 创建 SQL 文件
cat > /tmp/create_user.sql <<EOF
INSERT INTO users (id, username, email, password_hash, created_at, updated_at)
VALUES ('$USER_ID', 'admin', 'admin@example.com', '$PASSWORD_HASH', '$TIMESTAMP', '$TIMESTAMP')
ON CONFLICT (username) DO NOTHING;
EOF

echo "SQL 文件已创建：/tmp/create_user.sql"
echo ""

# 执行 SQL
if command -v sqlite3 &> /dev/null; then
    echo "使用 sqlite3 执行 SQL..."
    sqlite3 "$DB_FILE" < /tmp/create_user.sql
    
    # 检查是否成功
    RESULT=$(sqlite3 "$DB_FILE" "SELECT username FROM users WHERE username='admin';")
    
    if [ "$RESULT" = "admin" ]; then
        echo "✅ 测试用户创建成功！"
        echo ""
        echo "登录信息："
        echo "  用户名：admin"
        echo "  密码：admin123"
        echo "  邮箱：admin@example.com"
        echo ""
        echo "现在可以访问前端登录页面："
        echo "  http://localhost:5174"
    else
        echo "⚠️  用户可能已存在或创建失败"
        echo "请检查数据库：sqlite3 $DB_FILE"
    fi
else
    echo "❌ 错误：未找到 sqlite3 命令"
    echo "请安装 sqlite3：brew install sqlite"
    exit 1
fi

# 清理临时文件
rm /tmp/create_user.sql
