# 启动脚本架构说明

## 概述

本项目的启动脚本采用了**共享函数库**的架构设计，所有脚本都复用 `common.sh` 中的核心函数，确保行为一致性和易维护性。

---

## 架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                         common.sh                               │
│                      (共享函数库)                                │
│                                                                 │
│  ✅ check_docker_dependencies()  - Docker 检查和智能等待       │
│  ✅ check_dev_dependencies()     - Rust/Node.js 检查           │
│  ✅ start_postgres_db()          - PostgreSQL 启动             │
│  ✅ start_admin_backend()        - Admin Backend 启动          │
│  ✅ start_admin_frontend()       - Admin Frontend 启动         │
│  ✅ clean_port()                 - 端口清理                    │
│  ✅ log_*()                      - 日志函数                    │
│  ✅ show_database_info()         - 数据库信息显示              │
└─────────────────────────────────────────────────────────────────┘
                              ▲
                              │ source
                              │
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
        │                     │                     │
┌───────▼────────┐   ┌────────▼────────┐   ┌───────▼────────┐
│  start-db.sh   │   │ start-admin.sh  │   │ start-all.sh   │
│                │   │                 │   │                │
│ 仅启动数据库    │   │ 启动 Admin      │   │ 启动所有服务    │
│                │   │ Backend 全部    │   │                │
└────────────────┘   └─────────────────┘   └────────────────┘
```

---

## 脚本关系

### 1. `common.sh` - 共享函数库 (核心)

**位置**: `admin-backend/scripts/common.sh`

**作用**: 提供所有启动脚本共用的函数

**核心函数**:

#### Docker 相关
- `check_docker_dependencies()` - 检查 Docker 并智能等待 Engine 启动
  - ✅ 自动检测 Docker Desktop 是否运行
  - ✅ 自动等待 Docker Engine 就绪（最多 30 秒）
  - ✅ 自动启动 Docker Desktop（如果未运行）
  - ✅ 提供清晰的进度提示

- `start_postgres_db(dir)` - 启动 PostgreSQL 数据库
  - ✅ 检查容器是否已运行
  - ✅ 启动容器并等待就绪
  - ✅ 健康检查验证

#### 服务启动
- `start_admin_backend(dir)` - 启动 Admin Backend 后端
  - ✅ 智能等待编译完成
  - ✅ 端口监听检测
  - ✅ 健康检查验证

- `start_admin_frontend(dir)` - 启动 Admin Frontend 前端
  - ✅ 自动安装依赖
  - ✅ 进程状态检查

#### 工具函数
- `check_dev_dependencies()` - 检查 Rust/Node.js
- `clean_port(port, name)` - 清理端口
- `show_database_info()` - 显示数据库信息
- `log_info()`, `log_error()`, `log_warn()`, `log_section()` - 日志函数

---

### 2. `start-db.sh` - 数据库启动脚本

**位置**: `admin-backend/scripts/start-db.sh`

**用途**: 仅启动 PostgreSQL 数据库

**依赖**: `common.sh`

**使用的函数**:
```bash
source "$SCRIPT_DIR/common.sh"

check_docker_dependencies    # ✅ 包含 Docker Engine 智能等待
start_postgres_db           # ✅ 启动数据库
show_database_info          # ✅ 显示信息
```

**使用场景**:
- 单独启动数据库进行调试
- 修复数据库连接问题
- 手动启动后端前先启动数据库

---

### 3. `start-admin.sh` - Admin Backend 完整启动脚本

**位置**: `admin-backend/scripts/start-admin.sh`

**用途**: 启动 Admin Backend 的所有服务

**依赖**: `common.sh`

**使用的函数**:
```bash
source "$SCRIPT_DIR/common.sh"

check_dev_dependencies       # ✅ 检查 Rust/Node.js
check_docker_dependencies    # ✅ 包含 Docker Engine 智能等待
clean_port                   # ✅ 清理端口
start_postgres_db           # ✅ 启动数据库
start_admin_backend         # ✅ 启动后端
start_admin_frontend        # ✅ 启动前端
```

**启动的服务**:
1. PostgreSQL 数据库（端口 5432）
2. Admin Backend 后端（端口 3000）
3. Admin Frontend 前端（端口 5174）

---

### 4. `start-all.sh` - 完整开发环境启动脚本

**位置**: `scripts/start-all.sh` (项目根目录)

**用途**: 启动所有开发服务

**依赖**: `admin-backend/scripts/common.sh`

**使用的函数**:
```bash
source "$PROJECT_ROOT/admin-backend/scripts/common.sh"

check_dev_dependencies       # ✅ 检查 Rust/Node.js
check_docker_dependencies    # ✅ 包含 Docker Engine 智能等待
clean_port                   # ✅ 清理端口
start_postgres_db           # ✅ 启动数据库
start_admin_backend         # ✅ 启动 Admin Backend
start_admin_frontend        # ✅ 启动 Admin Frontend
```

**启动的服务**:
1. PostgreSQL 数据库（端口 5432）
2. Desktop Client 前端（端口 5173）
3. Tauri 客户端（内嵌 IronClaw 核心服务，端口 38080）
4. Admin Backend 后端（端口 3000）
5. Admin Backend 前端（端口 5174）

---

## 改进的自动传播

### Docker Engine 智能等待改进

**改进位置**: `common.sh` 中的 `check_docker_dependencies()` 函数

**改进内容**:
- ✅ 自动检测 Docker Desktop 是否运行
- ✅ 自动等待 Docker Engine 就绪（最多 30 秒）
- ✅ 自动启动 Docker Desktop（如果未运行）
- ✅ 提供清晰的进度提示

**自动受益的脚本**:
1. ✅ `start-db.sh` - 自动支持
2. ✅ `start-admin.sh` - 自动支持
3. ✅ `start-all.sh` - 自动支持

**无需修改**: 所有脚本都通过 `source common.sh` 自动获得改进！

---

## 优势

### 1. 代码复用 ✅

**问题**: 重复代码导致维护困难

**解决**: 共享函数库

**效果**:
- 一次修改，所有脚本受益
- 减少代码重复
- 降低维护成本

### 2. 行为一致性 ✅

**问题**: 不同脚本行为不一致

**解决**: 使用相同的函数

**效果**:
- Docker 检查逻辑一致
- 日志格式统一
- 错误处理一致

### 3. 易于扩展 ✅

**问题**: 添加新功能需要修改多个脚本

**解决**: 在 `common.sh` 中添加新函数

**效果**:
- 新功能自动传播到所有脚本
- 减少重复工作
- 降低出错概率

### 4. 易于测试 ✅

**问题**: 难以测试脚本行为

**解决**: 函数化设计

**效果**:
- 可以单独测试每个函数
- 易于模拟和 mock
- 提高测试覆盖率

---

## 最佳实践

### 添加新功能

如果需要添加新的启动逻辑：

1. **在 `common.sh` 中添加函数**
   ```bash
   # 新功能函数
   start_new_service() {
       local SERVICE_DIR="$1"
       log_section "启动新服务"
       # 实现逻辑
   }
   ```

2. **在需要的脚本中调用**
   ```bash
   source "$SCRIPT_DIR/common.sh"
   start_new_service "$PROJECT_ROOT/new-service"
   ```

3. **所有脚本自动受益**
   - 无需修改其他脚本
   - 行为自动一致

### 修改现有功能

如果需要修改现有逻辑：

1. **只修改 `common.sh` 中的函数**
   ```bash
   # 改进 Docker 检查逻辑
   check_docker_dependencies() {
       # 新的实现
   }
   ```

2. **所有脚本自动获得改进**
   - `start-db.sh` ✅
   - `start-admin.sh` ✅
   - `start-all.sh` ✅

### 添加新脚本

如果需要添加新的启动脚本：

1. **加载共享函数库**
   ```bash
   #!/bin/bash
   SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
   source "$SCRIPT_DIR/common.sh"
   ```

2. **使用现有函数**
   ```bash
   check_docker_dependencies
   start_postgres_db "$PROJECT_ROOT/admin-backend"
   ```

3. **自动获得所有改进**
   - Docker 智能等待 ✅
   - 统一的日志格式 ✅
   - 一致的错误处理 ✅

---

## 文件位置

```
项目结构:
├── scripts/
│   └── start-all.sh              # 完整开发环境启动脚本
│                                 # ✅ 使用 common.sh
└── admin-backend/
    └── scripts/
        ├── common.sh             # 共享函数库 (核心)
        ├── start-db.sh           # 数据库启动脚本
        │                         # ✅ 使用 common.sh
        ├── start-admin.sh        # Admin Backend 启动脚本
        │                         # ✅ 使用 common.sh
        └── diagnose.sh           # 诊断脚本
```

---

## 依赖关系

```
common.sh (核心)
    ↑
    │ source
    │
    ├─── start-db.sh
    │
    ├─── start-admin.sh
    │
    └─── start-all.sh (通过 ../admin-backend/scripts/common.sh)
```

---

## 测试验证

### 验证 Docker 智能等待

```bash
# 1. 退出 Docker Desktop
osascript -e 'tell application "Docker" to quit'

# 2. 运行任意启动脚本
./scripts/start-all.sh

# 预期结果:
# ✅ 自动检测 Docker Desktop 未运行
# ✅ 自动启动 Docker Desktop
# ✅ 自动等待 Docker Engine 就绪
# ✅ 继续启动其他服务
```

### 验证函数复用

```bash
# 1. 修改 common.sh 中的日志格式
# 2. 运行所有脚本
./admin-backend/scripts/start-db.sh
./admin-backend/scripts/start-admin.sh
./scripts/start-all.sh

# 预期结果:
# ✅ 所有脚本使用相同的日志格式
```

---

## 总结

**架构优势**:
- ✅ 代码复用 - 一次修改，所有脚本受益
- ✅ 行为一致 - 使用相同的函数
- ✅ 易于扩展 - 添加新功能简单
- ✅ 易于维护 - 集中管理核心逻辑

**Docker 改进自动传播**:
- ✅ `start-db.sh` - 自动支持 Docker Engine 智能等待
- ✅ `start-admin.sh` - 自动支持 Docker Engine 智能等待
- ✅ `start-all.sh` - 自动支持 Docker Engine 智能等待

**无需额外工作**:
- ❌ 不需要修改每个脚本
- ❌ 不需要重复实现逻辑
- ❌ 不需要担心行为不一致

**一次改进，全部受益！** 🎉

---

**文档创建时间**: 2026-03-20  
**作者**: Kiro AI Assistant
