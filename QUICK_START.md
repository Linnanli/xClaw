# 快速启动指南

## 前置条件 ⚠️

### 1. 手动启动 Docker

- **macOS**: 打开 Applications 文件夹，启动 Docker Desktop 应用
- **Linux**: `sudo systemctl start docker`
- **Windows**: 打开开始菜单，搜索并启动 Docker Desktop

确认 Docker 已启动:
```bash
docker ps
```

### 2. 手动启动 PostgreSQL 数据库

```bash
cd admin-backend
./scripts/start-db.sh
```

或使用 docker-compose:
```bash
cd admin-backend
docker-compose up -d postgres
```

## 一键启动所有服务 🚀

```bash
./scripts/start-all.sh
```

这会启动：
- ✅ 检查 PostgreSQL 数据库（端口 5432）- 如未运行则报错
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

### 创建独立 worktree

```bash
./scripts/add-worktree.sh audit
./scripts/add-worktree.sh --name ui-polish --branch feat/ui-polish --from main
./scripts/add-worktree.sh --name fix-login --no-open
```

这个脚本会：
- 在同级目录下创建 `../x-claw.worktrees/<名称>`
- 自动创建或复用对应 Git 分支
- 自动执行子模块同步与初始化，优先复用本地 `ironclaw` 仓库对象，避免新 worktree 里 `ironclaw` 为空或因本地未推送提交而拉取失败
- 默认用 VS Code 新窗口打开新 worktree；如不需要可加 `--no-open`

### 删除 worktree

```bash
./scripts/remove-worktree.sh audit
./scripts/remove-worktree.sh --name ui-polish --delete-branch
./scripts/remove-worktree.sh --path ../x-claw.worktrees/fix-login --force
```

## 常见问题

### Docker 未启动

**错误**: `Docker daemon 未运行`

**解决**: 手动启动 Docker
- **macOS**: 打开 Applications 文件夹，启动 Docker Desktop 应用
- **Linux**: `sudo systemctl start docker`
- **Windows**: 打开开始菜单，搜索并启动 Docker Desktop

启动后重新运行脚本：
```bash
./scripts/start-all.sh
```

### Docker 连接错误

**错误**: `Cannot connect to the Docker daemon at unix://.../.docker/run/docker.sock`

**原因**: Docker Desktop 应用运行中，但 Docker Engine 未启动

**快速修复**:
1. 点击菜单栏的 Docker 图标
2. 选择 "Quit Docker Desktop"
3. 等待 5-10 秒
4. 重新打开 Docker Desktop
5. 等待 Docker Engine 启动（菜单栏图标变为正常）
6. 验证: `docker ps`

**详细修复**: 查看 [DOCKER_CONNECTION_FIX.md](DOCKER_CONNECTION_FIX.md)

### 数据库未启动

**错误**: `PostgreSQL 容器未运行`

**解决**: 手动启动数据库
```bash
cd admin-backend
./scripts/start-db.sh
```

或使用 docker-compose:
```bash
cd admin-backend
docker-compose up -d postgres
```

启动后重新运行脚本：
```bash
./scripts/start-all.sh
```

### 数据库连接错误

**错误**: `Database error: error connecting to server`

**解决**: 数据库未启动或未就绪，运行：
```bash
cd admin-backend
./scripts/start-db.sh
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

### 启动相关
- [STARTUP_QUICK_REFERENCE.md](STARTUP_QUICK_REFERENCE.md) - 启动快速参考（推荐）
- [DESKTOP_CLIENT_QUICK_START.md](DESKTOP_CLIENT_QUICK_START.md) - Desktop Client 快速启动

### Docker 问题排查
- [FINAL_FIX_SUMMARY.md](FINAL_FIX_SUMMARY.md) - Docker 问题最终修复总结（必读）
- [DOCKER_DAEMON_CRASH_ANALYSIS.md](DOCKER_DAEMON_CRASH_ANALYSIS.md) - Docker daemon 崩溃问题分析
- [DOCKER_CONNECTION_FIX.md](DOCKER_CONNECTION_FIX.md) - Docker 连接问题详细修复指南

### Admin Backend
- [admin-backend/QUICK_FIX.md](admin-backend/QUICK_FIX.md) - 快速修复指南
- [admin-backend/TROUBLESHOOTING.md](admin-backend/TROUBLESHOOTING.md) - 完整故障排查
- [admin-backend/scripts/README.md](admin-backend/scripts/README.md) - 脚本使用说明
