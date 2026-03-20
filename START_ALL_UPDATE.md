# start-all.sh 更新说明

## 更新内容

`scripts/start-all.sh` 已更新，现在包含了 PostgreSQL 数据库的自动启动功能。

## 新增功能

### 1. 自动启动 PostgreSQL 数据库

**功能**:
- 检查 Docker 和 Docker Compose 依赖
- 自动启动 PostgreSQL 数据库容器
- 等待数据库健康检查通过
- 如果数据库已运行，跳过启动步骤

**实现**:
```bash
# 检查数据库容器是否已运行
if docker ps | grep -q admin-backend-postgres; then
    log_info "PostgreSQL 容器已在运行"
else
    log_info "启动 PostgreSQL 容器..."
    docker compose up -d postgres
    
    # 等待数据库就绪
    # ...
fi
```

### 2. 更新清理函数

**功能**:
- 停止所有服务时保留数据库容器
- 数据库数据持久化保存

**说明**:
```bash
# 注意：不停止数据库容器，以便保留数据
# 如需停止数据库，请手动运行: cd admin-backend && docker-compose down
```

### 3. 更新端口清理

**新增**:
- 清理 5432 端口（PostgreSQL）

### 4. 更新启动信息

**新增显示**:
- 数据库服务信息
- 数据库连接字符串
- 数据库日志查看命令

## 启动的服务

现在 `start-all.sh` 启动以下服务：

1. **PostgreSQL 数据库**（端口 5432）
2. **Desktop Client 前端**（端口 5173）
3. **Tauri 客户端**（内嵌 IronClaw 核心服务，端口 38080）
4. **Admin Backend 后端**（端口 3000）
5. **Admin Backend 前端**（端口 5174）

## 使用方法

### 启动所有服务

```bash
cd <project-root>
./scripts/start-all.sh
```

这会自动：
1. 检查所有依赖（Rust、Node.js、Docker）
2. 清理旧进程和端口
3. 启动 PostgreSQL 数据库
4. 启动 Desktop Client 前端
5. 启动 Tauri 客户端
6. 启动 Admin Backend 后端
7. 启动 Admin Backend 前端

### 停止服务

**停止前端和后端**:
```bash
# 按 Ctrl+C
```

**停止数据库**:
```bash
cd admin-backend
docker-compose down
```

**停止所有服务（包括数据库）**:
```bash
# 按 Ctrl+C 停止前端和后端
cd admin-backend
docker-compose down
```

## 服务架构

```
┌─────────────────────────────────────────────────────────────┐
│                    完整开发环境                              │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  Desktop Client                                              │
│  ├─ 前端 (5173) ──────────┐                                 │
│  └─ Tauri ────────────────┼─→ 内嵌后端 (38080)             │
│                            │                                 │
│  Admin Backend                                               │
│  ├─ 前端 (5174) ──────────┤                                 │
│  ├─ 后端 (3000) ──────────┼─→ PostgreSQL (5432)            │
│  └─ 数据库 (5432) ────────┘                                 │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## 与其他脚本的关系

### start-all.sh vs start-admin.sh

| 特性 | start-all.sh | start-admin.sh |
|------|-------------|----------------|
| PostgreSQL 数据库 | ✅ | ✅ |
| Desktop Client 前端 | ✅ | ❌ |
| Tauri 客户端 | ✅ | ❌ |
| Admin Backend 后端 | ✅ | ✅ |
| Admin Backend 前端 | ✅ | ✅ |
| 使用场景 | 完整开发环境 | 仅 Admin Backend |

### start-all.sh vs start-db.sh

| 特性 | start-all.sh | start-db.sh |
|------|-------------|-------------|
| PostgreSQL 数据库 | ✅ | ✅ |
| 其他服务 | ✅ | ❌ |
| 使用场景 | 完整开发环境 | 仅数据库 |

## 推荐使用场景

### 使用 start-all.sh

**适用于**:
- 完整的开发环境
- 同时开发 Desktop Client 和 Admin Backend
- 测试两个系统的集成
- 首次启动项目

**命令**:
```bash
./scripts/start-all.sh
```

### 使用 start-admin.sh

**适用于**:
- 仅开发 Admin Backend
- 不需要 Desktop Client
- 快速启动 Admin Backend

**命令**:
```bash
cd admin-backend
./scripts/start-admin.sh
```

### 使用 start-db.sh

**适用于**:
- 仅启动数据库
- 调试数据库问题
- 手动启动其他服务

**命令**:
```bash
cd admin-backend
./scripts/start-db.sh
```

## 常见问题

### Q: start-all.sh 会自动启动数据库吗？

A: 是的，现在 `start-all.sh` 会自动启动 PostgreSQL 数据库。

### Q: 我还需要单独运行 start-db.sh 吗？

A: 不需要。如果使用 `start-all.sh`，数据库会自动启动。

### Q: 停止服务时数据库会停止吗？

A: 不会。按 `Ctrl+C` 只会停止前端和后端服务，数据库容器会继续运行以保留数据。

### Q: 如何停止数据库？

A: 运行 `cd admin-backend && docker-compose down`

### Q: 数据库数据会丢失吗？

A: 不会。数据存储在 Docker 卷中，除非运行 `docker-compose down -v`。

### Q: 如果数据库已经在运行，会重启吗？

A: 不会。脚本会检测到数据库已运行并跳过启动步骤。

## 验证更新

### 1. 检查脚本版本

```bash
head -10 scripts/start-all.sh
```

应该看到：
```bash
# 此脚本启动所有开发服务：
# 1. PostgreSQL 数据库（端口 5432）
# 2. Desktop Client 前端（端口 5173）
# ...
```

### 2. 测试启动

```bash
./scripts/start-all.sh
```

应该看到：
```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
启动 PostgreSQL 数据库
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

✅ 启动 PostgreSQL 容器...
✅ 等待 PostgreSQL 启动...
✅ PostgreSQL 已就绪
```

### 3. 验证数据库

```bash
docker ps | grep admin-backend-postgres
docker exec admin-backend-postgres pg_isready -U postgres
```

## 相关文档

- [admin-backend/scripts/README.md](admin-backend/scripts/README.md) - 脚本使用说明
- [admin-backend/QUICK_FIX.md](admin-backend/QUICK_FIX.md) - 快速修复指南
- [admin-backend/TROUBLESHOOTING.md](admin-backend/TROUBLESHOOTING.md) - 故障排查
- [admin-backend/DATABASE_FIX_SUMMARY.md](admin-backend/DATABASE_FIX_SUMMARY.md) - 数据库修复总结

## 总结

✅ `start-all.sh` 现在包含数据库启动功能
✅ 无需单独运行 `start-db.sh` 或 `start-admin.sh`
✅ 一键启动所有开发服务
✅ 数据库数据持久化保存
✅ 智能检测数据库状态，避免重复启动
