# 快速启动指南

## 一键启动所有服务 🚀

```bash
./scripts/start-all.sh
```

这会启动：
- ✅ PostgreSQL 数据库（端口 5432）
- ✅ Desktop Client 前端（端口 5173）
- ✅ Tauri 客户端（内嵌后端，端口 38080）
- ✅ Admin Backend 后端（端口 3000）
- ✅ Admin Backend 前端（端口 5174）

## 访问地址

| 服务 | 地址 | 说明 |
|------|------|------|
| Desktop Client | Tauri 窗口自动打开 | 桌面客户端 |
| Admin Backend | http://localhost:5174 | 管理后台 |
| Desktop Client 前端 | http://localhost:5173 | 开发服务器 |
| Admin Backend API | http://localhost:3000 | 后端 API |
| Tauri 内嵌后端 | http://localhost:38080 | 内嵌服务 |
| PostgreSQL | localhost:5432 | 数据库 |

## 停止服务

**停止前端和后端**:
```bash
# 按 Ctrl+C
```

**停止数据库**:
```bash
cd admin-backend
docker-compose down
```

## 其他启动方式

### 仅启动 Admin Backend

```bash
cd admin-backend
./scripts/start-admin.sh
```

### 仅启动数据库

```bash
cd admin-backend
./scripts/start-db.sh
```

## 常见问题

### 数据库连接错误

**错误**: `Database error: error connecting to server`

**解决**: 数据库未启动，运行：
```bash
./scripts/start-all.sh
```

### 端口被占用

**解决**: 脚本会自动清理端口，如果仍有问题：
```bash
# 清理所有端口
lsof -i :3000 | grep -v COMMAND | awk '{print $2}' | xargs kill -9
lsof -i :5173 | grep -v COMMAND | awk '{print $2}' | xargs kill -9
lsof -i :5174 | grep -v COMMAND | awk '{print $2}' | xargs kill -9
lsof -i :5432 | grep -v COMMAND | awk '{print $2}' | xargs kill -9
lsof -i :38080 | grep -v COMMAND | awk '{print $2}' | xargs kill -9
```

### 查看日志

```bash
# Desktop Client 前端
tail -f /tmp/desktop-frontend.log

# Tauri 客户端
tail -f /tmp/tauri.log

# Admin Backend 后端
tail -f /tmp/admin-backend.log

# Admin Backend 前端
tail -f /tmp/admin-frontend.log

# 数据库
docker logs -f admin-backend-postgres
```

## 详细文档

- [START_ALL_UPDATE.md](START_ALL_UPDATE.md) - start-all.sh 更新说明
- [admin-backend/QUICK_FIX.md](admin-backend/QUICK_FIX.md) - 快速修复指南
- [admin-backend/TROUBLESHOOTING.md](admin-backend/TROUBLESHOOTING.md) - 完整故障排查
- [admin-backend/scripts/README.md](admin-backend/scripts/README.md) - 脚本使用说明
