# Admin Backend 启动脚本说明

## 脚本概述

### `start-admin.sh` - Admin Backend 专用启动脚本

**用途**: 启动 Admin Backend 管理后台

**启动的服务**:
- Admin Backend 后端服务（端口 3000）
- Admin Frontend 前端服务（端口 5174）

**使用场景**:
- 开发和测试 Admin Backend 功能
- 管理用户、角色、权限
- 配置 DLP 规则
- 查看审计日志

**启动方式**:
```bash
cd admin-backend
./scripts/start-admin.sh
```

**访问地址**:
- 前端: http://localhost:5174
- 后端 API: http://localhost:3000
- 测试账号: admin / admin123

---

### `../scripts/start-all.sh` - Desktop Client 启动脚本

**用途**: 启动 Desktop Client 桌面客户端

**启动的服务**:
- Ironclaw 主项目后端（端口 3000）
- Desktop Client 前端（端口 5173）
- Tauri 桌面应用

**使用场景**:
- 开发和测试 Desktop Client 功能
- 与 AI Agent 交互
- 使用聊天、记忆、任务等功能

**启动方式**:
```bash
cd <project-root>
./scripts/start-all.sh
```

**访问地址**:
- 前端: http://localhost:5173
- 后端 API: http://localhost:3000
- Tauri 应用: 自动启动

---

## 端口冲突说明

### 问题

两个项目都使用 3000 端口作为后端 API 端口，因此**不能同时运行**。

### 解决方案

#### 方案 1: 分时运行（推荐）

根据需要选择启动哪个项目：

**开发 Admin Backend 时**:
```bash
# 停止所有服务
pkill -f "ironclaw.*run"
pkill -f "admin-backend"
lsof -i :3000 | grep -v COMMAND | awk '{print $2}' | xargs kill -9

# 启动 Admin Backend
cd admin-backend
./scripts/start-admin.sh
```

**开发 Desktop Client 时**:
```bash
# 停止所有服务
pkill -f "ironclaw.*run"
pkill -f "admin-backend"
lsof -i :3000 | grep -v COMMAND | awk '{print $2}' | xargs kill -9

# 启动 Desktop Client
cd <project-root>
./scripts/start-all.sh
```

#### 方案 2: 修改端口（不推荐）

如果确实需要同时运行，可以修改 Admin Backend 的端口：

1. 修改 `admin-backend/.env`:
   ```env
   SERVER_PORT=3001
   ```

2. 修改 `admin-backend/frontend/.env`:
   ```env
   VITE_API_BASE_URL=http://localhost:3001
   ```

3. 重启服务

**注意**: 修改端口后需要更新所有相关配置和文档。

---

## 快速参考

### 检查端口占用

```bash
# 检查 3000 端口
lsof -i :3000

# 检查 5173 端口（Desktop Client 前端）
lsof -i :5173

# 检查 5174 端口（Admin Frontend）
lsof -i :5174
```

### 停止服务

```bash
# 停止 Admin Backend
pkill -f "admin-backend"

# 停止 Ironclaw
pkill -f "ironclaw.*run"

# 强制清理 3000 端口
lsof -i :3000 | grep -v COMMAND | awk '{print $2}' | xargs kill -9
```

### 查看日志

```bash
# Admin Backend 日志
tail -f /tmp/admin-backend.log

# Admin Frontend 日志
tail -f /tmp/admin-frontend.log

# Ironclaw 日志
tail -f /tmp/backend.log
```

---

## 常见问题

### Q: 为什么不能同时运行两个项目？

A: 两个项目都使用 3000 端口作为后端 API 端口，端口冲突导致无法同时运行。

### Q: 如何知道当前运行的是哪个项目？

A: 检查 3000 端口的进程名：
```bash
lsof -i :3000 | grep LISTEN
```
- 如果显示 `admin-backend`，则是 Admin Backend
- 如果显示 `ironclaw`，则是 Desktop Client

### Q: 启动脚本失败怎么办？

A: 按以下步骤排查：
1. 检查日志文件（`/tmp/admin-backend.log` 或 `/tmp/backend.log`）
2. 确认端口未被占用（`lsof -i :3000`）
3. 确认依赖已安装（`cargo --version`, `node --version`）
4. 尝试手动启动服务进行调试

### Q: Admin Backend 需要 ironclaw 服务吗？

A: **不需要**。Admin Backend 是独立的管理后台应用，有自己的后端服务和数据库。它不依赖 ironclaw 主项目的运行时服务。

### Q: 什么时候使用 start-all.sh？

A: 仅当你需要开发或测试 Desktop Client 功能时使用。如果只是开发 Admin Backend，使用 `admin-backend/scripts/start-admin.sh`。

---

## 架构说明

```
项目结构:
├── ironclaw/                    # 主项目（子模块）
│   └── 后端服务（端口 3000）
├── desktop-client/              # 桌面客户端
│   ├── src-ui/                  # 前端（端口 5173）
│   └── src-tauri/               # Tauri 应用
├── admin-backend/               # 管理后台（独立）
│   ├── src/                     # 后端服务（端口 3000）
│   ├── frontend/                # 前端（端口 5174）
│   └── scripts/
│       └── start-admin.sh       # Admin Backend 启动脚本
└── scripts/
    └── start-all.sh             # Desktop Client 启动脚本
```

**关键点**:
- Admin Backend 和 Desktop Client 是两个独立的应用
- 它们共享相同的后端端口（3000），因此不能同时运行
- Admin Backend 用于管理和配置
- Desktop Client 用于与 AI Agent 交互

---

## 总结

- **开发 Admin Backend**: 使用 `admin-backend/scripts/start-admin.sh`
- **开发 Desktop Client**: 使用 `scripts/start-all.sh`
- **不能同时运行**: 端口冲突
- **Admin Backend 不需要 ironclaw 服务**: 完全独立
