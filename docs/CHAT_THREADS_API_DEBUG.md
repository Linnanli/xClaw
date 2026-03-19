# /api/chat/threads 接口报错诊断指南

## 问题描述

访问 `/api/chat/threads` 接口时报错。

## 可能的原因

### 1. 后端服务未启动

**症状**：
- 连接被拒绝（Connection refused）
- 无法访问 `http://localhost:3000`

**解决方案**：
```bash
# 启动主项目后端服务
./scripts/start-backend-only.sh

# 或者手动启动
cd ironclaw
cargo run -- run --no-onboard
```

**验证**：
```bash
# 检查健康端点
curl http://localhost:3000/api/health

# 应该返回：{"status":"ok"}
```

---

### 2. 数据库未配置或连接失败

**症状**：
- 500 Internal Server Error
- 日志中显示数据库连接错误

**解决方案**：

检查数据库配置：
```bash
# 查看 .env 文件
cat .env

# 或查看 ~/.ironclaw/.env
cat ~/.ironclaw/.env
```

确保有以下配置：
```bash
# libSQL 数据库（推荐）
DATABASE_URL=file:~/.ironclaw/ironclaw.db

# 或 PostgreSQL
DATABASE_URL=postgresql://user:password@localhost/ironclaw
```

初始化数据库：
```bash
cd ironclaw
cargo run -- db init
```

---

### 3. 认证令牌缺失或无效

**症状**：
- 401 Unauthorized
- 403 Forbidden

**解决方案**：

获取认证令牌：
```bash
# 方法 1：从后端日志获取
tail -f /tmp/backend.log | grep "gateway.*http"

# 方法 2：从数据库读取
sqlite3 ~/.ironclaw/ironclaw.db "SELECT value FROM settings WHERE key = 'channels.gateway_auth_token'"
```

使用令牌访问 API：
```bash
# 设置令牌
export TOKEN="your_token_here"

# 访问 API
curl -H "Authorization: Bearer $TOKEN" http://localhost:3000/api/chat/threads
```

---

### 4. Session Manager 未初始化

**症状**：
- 503 Service Unavailable
- 错误信息：`Session manager not available`

**原因**：
后端启动时未正确初始化 Session Manager。

**解决方案**：

检查后端启动参数：
```bash
# 确保使用正确的启动命令
cargo run -- run --no-onboard

# 不要使用 --cli-only 参数（这会禁用 Web Gateway）
```

检查后端日志：
```bash
tail -100 /tmp/backend.log | grep -i "session\|gateway"
```

---

### 5. Store（数据库存储）未初始化

**症状**：
- 返回空的线程列表
- 无法创建 assistant thread

**原因**：
后端启动时未连接到数据库存储。

**解决方案**：

检查数据库配置：
```bash
# 确保 DATABASE_URL 已设置
echo $DATABASE_URL

# 如果未设置，添加到 .env
echo 'DATABASE_URL=file:~/.ironclaw/ironclaw.db' >> .env
```

重新启动后端：
```bash
./scripts/start-backend-only.sh
```

---

## 完整的诊断流程

### 步骤 1：检查后端服务状态

```bash
# 检查进程是否运行
ps aux | grep ironclaw

# 检查端口是否监听
lsof -i :3000

# 检查健康端点
curl http://localhost:3000/api/health
```

### 步骤 2：检查后端日志

```bash
# 查看最近的日志
tail -100 /tmp/backend.log

# 实时查看日志
tail -f /tmp/backend.log
```

### 步骤 3：测试 API 端点

```bash
# 获取认证令牌
TOKEN=$(sqlite3 ~/.ironclaw/ironclaw.db "SELECT value FROM settings WHERE key = 'channels.gateway_auth_token'" | tr -d '"')

# 测试 threads 接口
curl -v \
  -H "Authorization: Bearer $TOKEN" \
  http://localhost:3000/api/chat/threads

# 查看详细的响应
curl -v \
  -H "Authorization: Bearer $TOKEN" \
  http://localhost:3000/api/chat/threads 2>&1 | grep -E "< HTTP|< Content-Type|{.*}"
```

### 步骤 4：检查数据库状态

```bash
# 检查数据库文件是否存在
ls -lh ~/.ironclaw/ironclaw.db

# 检查数据库表
sqlite3 ~/.ironclaw/ironclaw.db ".tables"

# 检查 conversations 表
sqlite3 ~/.ironclaw/ironclaw.db "SELECT COUNT(*) FROM conversations"
```

---

## 常见错误和解决方案

### 错误 1：Connection refused

```
curl: (7) Failed to connect to localhost port 3000: Connection refused
```

**原因**：后端服务未启动

**解决**：
```bash
./scripts/start-backend-only.sh
```

---

### 错误 2：401 Unauthorized

```json
{
  "error": "Unauthorized"
}
```

**原因**：认证令牌缺失或无效

**解决**：
```bash
# 获取正确的令牌
TOKEN=$(sqlite3 ~/.ironclaw/ironclaw.db "SELECT value FROM settings WHERE key = 'channels.gateway_auth_token'" | tr -d '"')

# 使用令牌访问
curl -H "Authorization: Bearer $TOKEN" http://localhost:3000/api/chat/threads
```

---

### 错误 3：503 Service Unavailable

```json
{
  "error": "Session manager not available"
}
```

**原因**：后端启动时未初始化 Session Manager

**解决**：
```bash
# 停止后端
pkill -f "ironclaw.*run"

# 重新启动（不要使用 --cli-only）
cd ironclaw
cargo run -- run --no-onboard
```

---

### 错误 4：500 Internal Server Error

```json
{
  "error": "Database error: ..."
}
```

**原因**：数据库连接失败或查询错误

**解决**：
```bash
# 检查数据库配置
echo $DATABASE_URL

# 初始化数据库
cd ironclaw
cargo run -- db init

# 重新启动后端
./scripts/start-backend-only.sh
```

---

## 快速修复脚本

创建一个快速诊断脚本：

```bash
#!/bin/bash
# 文件名：debug-chat-threads.sh

echo "=== 诊断 /api/chat/threads 接口 ==="
echo ""

# 1. 检查后端进程
echo "1. 检查后端进程..."
if ps aux | grep -q "[i]ronclaw.*run"; then
    echo "✅ 后端进程正在运行"
else
    echo "❌ 后端进程未运行"
    echo "   启动命令：./scripts/start-backend-only.sh"
    exit 1
fi

# 2. 检查端口
echo ""
echo "2. 检查端口 3000..."
if lsof -i :3000 > /dev/null 2>&1; then
    echo "✅ 端口 3000 正在监听"
else
    echo "❌ 端口 3000 未监听"
    exit 1
fi

# 3. 检查健康端点
echo ""
echo "3. 检查健康端点..."
if curl -s http://localhost:3000/api/health | grep -q "ok"; then
    echo "✅ 健康检查通过"
else
    echo "❌ 健康检查失败"
    exit 1
fi

# 4. 检查数据库
echo ""
echo "4. 检查数据库..."
if [ -f ~/.ironclaw/ironclaw.db ]; then
    echo "✅ 数据库文件存在"
    COUNT=$(sqlite3 ~/.ironclaw/ironclaw.db "SELECT COUNT(*) FROM conversations" 2>/dev/null || echo "0")
    echo "   对话数量：$COUNT"
else
    echo "❌ 数据库文件不存在"
    echo "   初始化命令：cd ironclaw && cargo run -- db init"
    exit 1
fi

# 5. 获取认证令牌
echo ""
echo "5. 获取认证令牌..."
TOKEN=$(sqlite3 ~/.ironclaw/ironclaw.db "SELECT value FROM settings WHERE key = 'channels.gateway_auth_token'" 2>/dev/null | tr -d '"')
if [ -z "$TOKEN" ]; then
    echo "❌ 未找到认证令牌"
    echo "   检查后端日志：tail -f /tmp/backend.log"
    exit 1
else
    echo "✅ 认证令牌已获取"
    echo "   令牌前缀：${TOKEN:0:20}..."
fi

# 6. 测试 API
echo ""
echo "6. 测试 /api/chat/threads 接口..."
RESPONSE=$(curl -s -w "\n%{http_code}" -H "Authorization: Bearer $TOKEN" http://localhost:3000/api/chat/threads)
HTTP_CODE=$(echo "$RESPONSE" | tail -1)
BODY=$(echo "$RESPONSE" | head -n -1)

if [ "$HTTP_CODE" = "200" ]; then
    echo "✅ API 调用成功"
    echo ""
    echo "响应内容："
    echo "$BODY" | jq . 2>/dev/null || echo "$BODY"
else
    echo "❌ API 调用失败"
    echo "   HTTP 状态码：$HTTP_CODE"
    echo "   响应内容：$BODY"
    exit 1
fi

echo ""
echo "=== 诊断完成 ==="
```

使用方法：
```bash
chmod +x debug-chat-threads.sh
./debug-chat-threads.sh
```

---

## 总结

最常见的问题和解决方案：

1. **后端未启动** → 运行 `./scripts/start-backend-only.sh`
2. **认证令牌缺失** → 从数据库或日志获取令牌
3. **数据库未初始化** → 运行 `cargo run -- db init`
4. **Session Manager 未初始化** → 确保使用 `--no-onboard` 而不是 `--cli-only`

如果以上方法都无法解决问题，请提供：
- 后端日志（`/tmp/backend.log`）
- 错误的完整响应
- 使用的启动命令
