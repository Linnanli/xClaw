# 快速修复：数据库连接错误

## 问题

访问 `api/dlp-rules` 接口时报错 500：
```json
{
  "details": "Database error: Error occurred while creating a new object: error connecting to server",
  "error": "Database error"
}
```

## 原因

PostgreSQL 数据库未启动。

## 解决方案（3 种方法）

### 方法 1: 使用完整启动脚本（推荐）

```bash
cd admin-backend
./scripts/start-admin.sh
```

这会启动：
- PostgreSQL 数据库
- Admin Backend 后端
- Admin Frontend 前端

### 方法 2: 仅启动数据库

```bash
cd admin-backend
./scripts/start-db.sh
```

然后手动启动后端和前端：
```bash
# 终端 1: 启动后端
cd admin-backend
cargo run

# 终端 2: 启动前端
cd admin-backend/frontend
npm run dev
```

### 方法 3: 使用 Docker Compose

```bash
cd admin-backend
docker-compose up -d postgres
```

## 验证修复

### 1. 检查数据库是否运行

```bash
docker ps | grep admin-backend-postgres
```

应该看到类似输出：
```
CONTAINER ID   IMAGE                  STATUS         PORTS
abc123def456   postgres:15-alpine     Up 2 minutes   0.0.0.0:5432->5432/tcp
```

### 2. 测试数据库连接

```bash
docker exec admin-backend-postgres pg_isready -U postgres
```

应该看到：
```
/var/run/postgresql:5432 - accepting connections
```

### 3. 测试 API 端点

```bash
# 启动后端后测试
curl http://localhost:3000/health
```

应该返回：
```json
{"status":"ok"}
```

### 4. 测试 DLP 规则 API

```bash
curl http://localhost:3000/api/dlp-rules
```

应该返回 DLP 规则列表（可能为空）。

## 常见问题

### Q: 数据库启动后仍然报错？

A: 重启后端服务：
```bash
# 停止后端
pkill -f "admin-backend"

# 重新启动
cd admin-backend
cargo run
```

### Q: 端口 5432 被占用？

A: 停止其他 PostgreSQL 实例：
```bash
# 查看占用端口的进程
lsof -i :5432

# 停止 Docker 容器
docker-compose down
```

### Q: 数据库容器无法启动？

A: 查看日志：
```bash
docker logs admin-backend-postgres
```

重建容器：
```bash
docker-compose down
docker-compose up -d postgres
```

## 数据库信息

- **容器名**: admin-backend-postgres
- **端口**: localhost:5432
- **用户**: postgres
- **密码**: postgres
- **数据库**: ironclaw
- **连接字符串**: `postgresql://postgres:postgres@localhost:5432/ironclaw`

## 停止服务

### 停止后端和前端

按 `Ctrl+C` 或：
```bash
pkill -f "admin-backend"
```

### 停止数据库

```bash
cd admin-backend
docker-compose down
```

注意：这会保留数据。如需删除数据，使用：
```bash
docker-compose down -v
```

## 更多帮助

详细的故障排查指南：
- [TROUBLESHOOTING.md](./TROUBLESHOOTING.md)
- [scripts/README.md](./scripts/README.md)
