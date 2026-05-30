# Admin Backend 启动脚本说明

## 脚本架构

本项目的启动脚本采用**共享函数库**架构，所有脚本都复用 `common.sh` 中的核心函数。

### 架构图

```
common.sh (共享函数库)
    ↑
    │ source
    │
    ├─── start-db.sh          ✅ 自动获得所有改进
    ├─── start-admin.sh       ✅ 自动获得所有改进
    └─── ../scripts/start-all.sh  ✅ 自动获得所有改进
```

### 优势

1. **一次改进，全部受益** - 修改 `common.sh` 中的函数，所有脚本自动获得改进
2. **行为一致** - 所有脚本使用相同的 Docker 检查、日志格式、错误处理
3. **易于维护** - 核心逻辑集中管理，减少代码重复

### 最近改进

✅ **Docker Engine 智能等待** (2026-03-20)
- 自动检测 Docker Desktop 是否运行
- 自动等待 Docker Engine 就绪（最多 30 秒）
- 自动启动 Docker Desktop（如果未运行）
- 所有脚本自动支持此改进

详细架构说明: [ARCHITECTURE.md](./ARCHITECTURE.md)

---

## 脚本概述

### `start-admin.sh` - Admin Backend 完整启动脚本

**用途**: 启动 Admin Backend 管理后台的所有服务

**启动的服务**:
- PostgreSQL 数据库（端口 5432）
- Admin Backend 后端服务（端口 3000）
- Admin Frontend 前端服务（端口 5174）

**使用场景**:
- 仅开发和测试 Admin Backend 功能
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
- 数据库: localhost:5432
- 测试账号: admin / admin123

---

### `start-db.sh` - 数据库启动脚本

**用途**: 仅启动 PostgreSQL 数据库容器

**启动的服务**:
- PostgreSQL 数据库（端口 5432）

**使用场景**:
- 单独启动数据库进行调试
- 修复数据库连接问题
- 手动启动后端前先启动数据库

**启动方式**:
```bash
cd admin-backend
./scripts/start-db.sh
```

**数据库信息**:
- 容器名: admin-backend-postgres
- 端口: localhost:5432
- 用户: postgres
- 密码: postgres
- 数据库: ironclaw
- 连接字符串: postgresql://postgres:postgres@localhost:5432/ironclaw

---

### `../scripts/start-all.sh` - 完整开发环境启动脚本 ⭐

**用途**: 启动所有开发服务（Desktop Client + Admin Backend）

**启动的服务**:
- PostgreSQL 数据库（端口 5432）
- Desktop Client 前端（端口 5173）
- Tauri 客户端（内嵌 IronClaw 核心服务，端口 38080）
- Admin Backend 后端（端口 3000）
- Admin Backend 前端（端口 5174）

**使用场景**:
- 完整的开发环境
- 同时开发 Desktop Client 和 Admin Backend
- 测试两个系统的集成

**启动方式**:
```bash
cd <project-root>
./scripts/start-all.sh
```

**访问地址**:
- Desktop Client 前端: http://localhost:5173
- Desktop Client 后端: http://localhost:38080
- Admin Backend 前端: http://localhost:5174
- Admin Backend 后端: http://localhost:3000
- PostgreSQL 数据库: localhost:5432

**注意**: 
- 此脚本会自动启动 PostgreSQL 数据库
- 无需单独运行 `start-db.sh` 或 `start-admin.sh`
- 一键启动所有服务

---

## 快速诊断

如果遇到问题，首先运行诊断脚本:

```bash
cd admin-backend
./scripts/diagnose.sh
```

诊断脚本会检查:
- Docker 是否安装和运行 ⭐
- PostgreSQL 容器状态
- 后端服务状态 (端口 3000)
- 前端服务状态 (端口 5174)
- Rust 和 Node.js 环境
- 日志文件位置

---

## 常见问题解决

### 问题 0: Docker 未运行 🔴

**错误信息**:
```
Cannot connect to the Docker daemon
❌ PostgreSQL 启动超时
```

**原因**: Docker Desktop 应用未启动

**解决方案**:
- **macOS**: 打开 Applications 文件夹，启动 Docker 应用
- **Windows**: 打开开始菜单，搜索并启动 Docker Desktop
- **Linux**: `sudo systemctl start docker`

**验证**:
```bash
docker ps  # 应该能看到容器列表
```

---

### 问题 1: 数据库连接错误 ⚠️

**错误信息**:
```
Database error: Error occurred while creating a new object: error connecting to server
```

**原因**: PostgreSQL 数据库未启动

**解决方案**:
```bash
# 方案 1: 使用完整启动脚本（推荐）
cd admin-backend
./scripts/start-admin.sh

# 方案 2: 单独启动数据库
cd admin-backend
./scripts/start-db.sh

# 方案 3: 手动启动数据库
cd admin-backend
docker-compose up -d postgres
```

**验证数据库是否运行**:
```bash
# 检查容器状态
docker ps | grep admin-backend-postgres

# 测试数据库连接
docker exec admin-backend-postgres pg_isready -U postgres
```

---

### 问题 2: 端口冲突

**问题**: Admin Backend 和 Desktop Client 都使用 3000 端口，因此**不能同时运行**。

**解决方案**: 分时运行（推荐）

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

---

### 问题 3: 后端编译失败

**查看日志**:
```bash
tail -f /tmp/admin-backend.log
```

**常见原因**:
- 依赖未安装: `cargo build`
- 数据库未启动: 先运行 `./scripts/start-db.sh`
- 端口被占用: 清理 3000 端口

---

### 问题 4: 前端启动失败

**查看日志**:
```bash
tail -f /tmp/admin-frontend.log
```

**常见原因**:
- 依赖未安装: `cd frontend && npm install`
- 端口被占用: 清理 5174 端口

---

## 数据库管理

### 连接数据库

```bash
# 使用 psql 连接
docker exec -it admin-backend-postgres psql -U postgres -d ironclaw

# 或使用任何 PostgreSQL 客户端
# Host: localhost
# Port: 5432
# User: postgres
# Password: postgres
# Database: ironclaw
```

### 查看数据库日志

```bash
docker logs admin-backend-postgres
docker logs -f admin-backend-postgres  # 实时查看
```

### 停止数据库

```bash
cd admin-backend
docker-compose down
```

### 重启数据库

```bash
cd admin-backend
docker-compose restart postgres
```

### 清理数据库数据

```bash
cd admin-backend
docker-compose down -v  # 删除数据卷
```

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

# 检查 5432 端口（PostgreSQL）
lsof -i :5432
```

### 停止服务

```bash
# 停止 Admin Backend
pkill -f "admin-backend"

# 停止 Ironclaw
pkill -f "ironclaw.*run"

# 停止数据库
cd admin-backend && docker-compose down

# 强制清理 3000 端口
lsof -i :3000 | grep -v COMMAND | awk '{print $2}' | xargs kill -9
```

### 查看日志

```bash
# Admin Backend 日志
tail -f /tmp/admin-backend.log

# Admin Frontend 日志
tail -f /tmp/admin-frontend.log

# 数据库日志
docker logs -f admin-backend-postgres

# Ironclaw 日志
tail -f /tmp/backend.log
```

---

## 开发工作流

### 首次启动

```bash
# 1. 启动所有服务
cd admin-backend
./scripts/start-admin.sh

# 2. 等待编译完成（可能需要几分钟）

# 3. 浏览器自动打开 http://localhost:5174
```

### 日常开发

```bash
# 1. 启动数据库（如果未运行）
./scripts/start-db.sh

# 2. 启动后端（在一个终端）
cd admin-backend
cargo run

# 3. 启动前端（在另一个终端）
cd admin-backend/ui
npm run dev
```

---

## 服务端口

| 服务 | 端口 | 说明 |
|------|------|------|
| PostgreSQL | 5432 | 数据库 |
| Admin Backend | 3000 | 后端 API |
| Admin Frontend | 5174 | 前端开发服务器 |
| Desktop Client Backend | 3000 | 后端 API（与 Admin Backend 冲突） |
| Desktop Client Frontend | 5173 | 前端开发服务器 |

---

## 架构说明

```
项目结构:
├── ironclaw/                    # 主项目（子模块）
│   └── 后端服务（端口 3000）
├── desktop-client/              # 桌面客户端
│   ├── ui/                  # 前端（端口 5173）
│   └── src-tauri/               # Tauri 应用
├── admin-backend/               # 管理后台（独立）
│   ├── src/                     # 后端服务（端口 3000）
│   ├── frontend/                # 前端（端口 5174）
│   ├── migrations/              # 数据库迁移
│   ├── docker-compose.yml       # 数据库配置
│   └── scripts/
│       ├── start-admin.sh       # 完整启动脚本
│       └── start-db.sh          # 数据库启动脚本
└── scripts/
    └── start-all.sh             # Desktop Client 启动脚本
```

**关键点**:
- Admin Backend 和 Desktop Client 是两个独立的应用
- 它们共享相同的后端端口（3000），因此不能同时运行
- Admin Backend 需要 PostgreSQL 数据库
- Admin Backend 用于管理和配置
- Desktop Client 用于与 AI Agent 交互

---

## 总结

- **开发 Admin Backend**: 使用 `admin-backend/scripts/start-admin.sh`
- **仅启动数据库**: 使用 `admin-backend/scripts/start-db.sh`
- **开发 Desktop Client**: 使用 `scripts/start-all.sh`
- **不能同时运行**: 端口冲突
- **Admin Backend 需要数据库**: 必须先启动 PostgreSQL
- **数据库连接错误**: 使用 `start-db.sh` 启动数据库
