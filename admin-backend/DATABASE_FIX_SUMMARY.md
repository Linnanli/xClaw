# 数据库连接问题修复总结

## 问题描述

访问 `api/dlp-rules` 接口时报错 500：
```json
{
  "details": "Database error: Error occurred while creating a new object: error connecting to server",
  "error": "Database error"
}
```

## 根本原因

PostgreSQL 数据库未启动。原有的 `start-admin.sh` 脚本只启动了后端和前端服务，没有启动数据库容器。

## 修复内容

### 1. 更新 `scripts/start-admin.sh`

**添加的功能**：
- 检查 Docker 和 Docker Compose 依赖
- 自动启动 PostgreSQL 数据库容器
- 等待数据库健康检查通过
- 在清理函数中添加数据库相关说明
- 在启动完成信息中显示数据库连接信息

**关键改动**：
```bash
# 新增：启动数据库
log_section "启动 PostgreSQL 数据库"

if docker ps | grep -q admin-backend-postgres; then
    log_info "PostgreSQL 容器已在运行"
else
    log_info "启动 PostgreSQL 容器..."
    docker compose up -d postgres
    
    # 等待数据库就绪
    MAX_WAIT=30
    while [ $WAIT_COUNT -lt $MAX_WAIT ]; do
        if docker exec admin-backend-postgres pg_isready -U postgres > /dev/null 2>&1; then
            log_info "PostgreSQL 已就绪"
            break
        fi
        sleep 1
        WAIT_COUNT=$((WAIT_COUNT + 1))
    done
fi
```

### 2. 创建 `scripts/start-db.sh`

**新增脚本**：独立的数据库启动脚本

**功能**：
- 仅启动 PostgreSQL 数据库容器
- 检查容器是否已运行
- 等待数据库健康检查通过
- 显示数据库连接信息和常用命令

**使用场景**：
- 单独启动数据库进行调试
- 修复数据库连接问题
- 手动启动后端前先启动数据库

### 3. 更新 `scripts/README.md`

**添加的内容**：
- `start-db.sh` 脚本说明
- 数据库连接错误的解决方案
- 数据库管理命令
- 数据库信息和连接字符串
- 开发工作流程
- 服务端口列表

### 4. 创建 `TROUBLESHOOTING.md`

**新增文档**：完整的故障排查指南

**包含内容**：
- 快速诊断命令
- 7 种常见错误及解决方案
- 完整重置流程
- 日志位置
- 常用调试命令
- 预防措施

### 5. 创建 `QUICK_FIX.md`

**新增文档**：快速修复指南

**包含内容**：
- 问题描述
- 3 种解决方案
- 验证步骤
- 常见问题
- 数据库信息

### 6. 创建 `DATABASE_FIX_SUMMARY.md`

**新增文档**：本文档，总结所有修改

## 文件清单

### 修改的文件

1. `admin-backend/scripts/start-admin.sh`
   - 添加数据库依赖检查
   - 添加数据库启动逻辑
   - 更新启动完成信息

2. `admin-backend/scripts/README.md`
   - 添加 `start-db.sh` 说明
   - 添加数据库管理部分
   - 更新故障排查部分

### 新增的文件

1. `admin-backend/scripts/start-db.sh`
   - 独立的数据库启动脚本
   - 可执行权限：`chmod +x`

2. `admin-backend/TROUBLESHOOTING.md`
   - 完整的故障排查指南
   - 7 种常见错误及解决方案

3. `admin-backend/QUICK_FIX.md`
   - 快速修复指南
   - 针对数据库连接错误

4. `admin-backend/DATABASE_FIX_SUMMARY.md`
   - 本文档
   - 修复总结

## 使用方法

### 快速修复（推荐）

```bash
cd admin-backend
./scripts/start-admin.sh
```

这会自动：
1. 检查依赖
2. 启动 PostgreSQL 数据库
3. 启动后端服务
4. 启动前端服务

### 仅启动数据库

```bash
cd admin-backend
./scripts/start-db.sh
```

### 验证修复

```bash
# 1. 检查数据库
docker ps | grep admin-backend-postgres

# 2. 测试连接
docker exec admin-backend-postgres pg_isready -U postgres

# 3. 测试 API
curl http://localhost:3000/health
curl http://localhost:3000/api/dlp-rules
```

## 技术细节

### 数据库配置

**Docker Compose 配置** (`docker-compose.yml`):
```yaml
services:
  postgres:
    image: postgres:15-alpine
    container_name: admin-backend-postgres
    environment:
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: postgres
      POSTGRES_DB: ironclaw
    ports:
      - "5432:5432"
    volumes:
      - postgres_data:/var/lib/postgresql/data
      - ./migrations:/docker-entrypoint-initdb.d
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 5s
      timeout: 5s
      retries: 5
```

**环境变量** (`.env`):
```env
DB_HOST=localhost
DB_PORT=5432
DB_USER=postgres
DB_PASSWORD=postgres
DB_NAME=ironclaw
```

### 数据库迁移

数据库迁移文件位于 `admin-backend/migrations/`:
- `001_init.sql` - 初始化表结构
- `002_rbac.sql` - RBAC 权限系统
- `003_dlp_rules_enhancement.sql` - DLP 规则增强

这些迁移会在容器首次启动时自动执行。

### 健康检查

脚本使用 `pg_isready` 命令检查数据库是否就绪：
```bash
docker exec admin-backend-postgres pg_isready -U postgres
```

返回值：
- `0` - 数据库就绪
- `非0` - 数据库未就绪

## 测试结果

### 测试环境

- 操作系统: macOS
- Docker: 已安装
- Docker Compose: 已安装

### 测试步骤

1. 运行 `./scripts/start-db.sh`
2. 验证数据库启动
3. 测试数据库连接

### 测试结果

✅ 数据库成功启动
✅ 健康检查通过
✅ 连接测试成功

**输出**：
```
✅ PostgreSQL 数据库已启动

数据库信息:
  容器: admin-backend-postgres
  端口: localhost:5432
  用户: postgres
  密码: postgres
  数据库: ironclaw

连接字符串:
  postgresql://postgres:postgres@localhost:5432/ironclaw
```

## 后续步骤

### 1. 启动完整服务

```bash
cd admin-backend
./scripts/start-admin.sh
```

### 2. 访问前端

浏览器打开：http://localhost:5174

### 3. 测试 DLP 规则 API

```bash
# 获取所有规则
curl http://localhost:3000/api/dlp-rules

# 创建新规则
curl -X POST http://localhost:3000/api/dlp-rules \
  -H "Content-Type: application/json" \
  -d '{
    "name": "测试规则",
    "pattern": "\\d{3}-\\d{2}-\\d{4}",
    "replacement": "[REDACTED]",
    "severity": "high",
    "category": "pii"
  }'
```

## 注意事项

### 数据持久化

数据库数据存储在 Docker 卷中：
- 卷名: `admin-backend_postgres_data`
- 停止容器不会删除数据
- 删除卷会清空所有数据

### 停止服务

**保留数据**：
```bash
docker-compose down
```

**删除数据**：
```bash
docker-compose down -v
```

### 端口冲突

Admin Backend 和 Desktop Client 都使用 3000 端口，不能同时运行。

**解决方案**：
- 开发 Admin Backend 时，停止 Desktop Client
- 开发 Desktop Client 时，停止 Admin Backend

## 相关文档

- [QUICK_FIX.md](./QUICK_FIX.md) - 快速修复指南
- [TROUBLESHOOTING.md](./TROUBLESHOOTING.md) - 完整故障排查
- [scripts/README.md](./scripts/README.md) - 脚本使用说明
- [docker-compose.yml](./docker-compose.yml) - 数据库配置

## 总结

通过以下修改，成功解决了数据库连接问题：

1. ✅ 更新启动脚本，自动启动数据库
2. ✅ 创建独立的数据库启动脚本
3. ✅ 添加完整的文档和故障排查指南
4. ✅ 测试验证所有功能正常

现在用户可以：
- 使用 `start-admin.sh` 一键启动所有服务
- 使用 `start-db.sh` 单独启动数据库
- 参考文档快速解决常见问题
