# 启动脚本对比说明

## 概览

项目中有两个启动脚本，分别用于不同的开发场景：

| 脚本 | 路径 | 用途 | 架构模式 |
|------|------|------|----------|
| `start-dev.sh` | `desktop-client/scripts/start-dev.sh` | 仅启动 Desktop Client（嵌入式模式） | IronClaw 嵌入式 |
| `start-all.sh` | `scripts/start-all.sh` | 启动完整开发环境（所有服务） | IronClaw 外部服务 + Admin Backend |

---

## 1. `start-dev.sh` — Desktop Client 嵌入式模式

### 架构

```
┌─────────────────────────────────────────┐
│         Tauri Desktop Client            │
│  ┌───────────────────────────────────┐  │
│  │  前端 (React)                     │  │
│  │  http://localhost:5173            │  │
│  └───────────────────────────────────┘  │
│              ▲                          │
│              │ Tauri IPC                │
│              ▼                          │
│  ┌───────────────────────────────────┐  │
│  │  IronClaw 引擎（嵌入式）          │  │
│  │  - Agent                          │  │
│  │  - LLM                            │  │
│  │  - Tools                          │  │
│  │  - Safety/DLP                     │  │
│  │  - Database (libSQL)              │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
```

### 启动的服务

- ✅ Desktop Client 前端（Vite Dev Server，端口 5173）
- ✅ Tauri 应用（内嵌 IronClaw 引擎）
- ❌ 不启动外部 IronClaw 服务器
- ❌ 不启动 Admin Backend
- ❌ 不启动 PostgreSQL 数据库

### 使用场景

- 开发 Desktop Client 功能
- 测试嵌入式 IronClaw 引擎
- 不需要管理后台
- 快速启动，单一进程

### 前置条件

```bash
# 1. 配置 LLM API Key
cp desktop-client/.env.example desktop-client/.env
# 编辑 .env，设置 LLM_BACKEND 和 LLM_API_KEY

# 2. 安装前端依赖
cd desktop-client/src-ui && npm install
```

### 启动命令

```bash
# 方式 1: 使用脚本（推荐）
./desktop-client/scripts/start-dev.sh

# 方式 2: 直接使用 Tauri CLI
cd desktop-client
cargo tauri dev

# 方式 3: 临时指定 API Key
LLM_BACKEND=anthropic LLM_API_KEY=sk-ant-xxx ./desktop-client/scripts/start-dev.sh
```

### 优点

- ✅ 启动快速（无需等待多个服务）
- ✅ 单一进程，易于调试
- ✅ 无需 Docker 和 PostgreSQL
- ✅ 真实的嵌入式架构（与生产环境一致）
- ✅ 本地数据库（libSQL），数据隔离

### 缺点

- ❌ 无法测试管理后台功能
- ❌ 无法测试配置同步和数据上报
- ❌ 无法测试多客户端协同

---

## 2. `start-all.sh` — 完整开发环境

### 架构

```
┌─────────────────────────────────────────┐
│         Tauri Desktop Client            │
│  ┌───────────────────────────────────┐  │
│  │  前端 (React)                     │  │
│  │  http://localhost:5173            │  │
│  └───────────────────────────────────┘  │
│              ▲                          │
│              │ HTTP API                 │
│              ▼                          │
└─────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────┐
│   IronClaw 服务器（外部进程）            │
│   http://localhost:38080                │
│   - Gateway API                         │
│   - Agent                               │
│   - LLM                                 │
│   - Tools                               │
└─────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────┐
│         Admin Backend                   │
│  ┌───────────────────────────────────┐  │
│  │  前端 (React)                     │  │
│  │  http://localhost:5174            │  │
│  └───────────────────────────────────┘  │
│              ▲                          │
│              │ HTTP API                 │
│              ▼                          │
│  ┌───────────────────────────────────┐  │
│  │  后端 (Rust)                      │  │
│  │  http://localhost:3000            │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────┐
│   PostgreSQL (Docker)                   │
│   localhost:5432                        │
└─────────────────────────────────────────┘
```

### 启动的服务

- ✅ Desktop Client 前端（端口 5173）
- ✅ Tauri 应用（连接外部 IronClaw 服务器）
- ✅ IronClaw 服务器（端口 38080）
- ✅ Admin Backend 后端（端口 3000）
- ✅ Admin Backend 前端（端口 5174）
- ✅ PostgreSQL 数据库（端口 5432，Docker 容器）

### 使用场景

- 开发管理后台功能
- 测试配置同步和数据上报
- 测试多客户端协同
- 完整的系统集成测试

### 前置条件

```bash
# 1. 启动 Docker（macOS: Docker Desktop）
# 2. 启动 PostgreSQL 数据库
cd admin-backend
./scripts/start-db.sh

# 3. 配置 LLM API Key
export ANTHROPIC_API_KEY="sk-ant-xxx"
# 或在 .env 或 ~/.ironclaw/.env 中配置
```

### 启动命令

```bash
# 方式 1: 并行启动（快速，但看不到编译进度）
./scripts/start-all.sh

# 方式 2: 串行启动（推荐首次运行，显示编译进度）
./scripts/start-all.sh --serial
# 或
./scripts/start-all.sh -s
```

### 优点

- ✅ 完整的开发环境
- ✅ 可测试所有功能（包括管理后台）
- ✅ 可测试配置同步和数据上报
- ✅ 可测试多客户端协同
- ✅ 真实的生产环境模拟

### 缺点

- ❌ 启动慢（需要编译多个 Rust 项目）
- ❌ 需要 Docker 和 PostgreSQL
- ❌ 多进程管理复杂
- ❌ 资源占用高

---

## 启动模式对比

### 并行模式 vs 串行模式（仅 `start-all.sh`）

| 特性 | 并行模式 | 串行模式 |
|------|----------|----------|
| 启动速度 | 快（所有服务同时启动） | 慢（逐个启动） |
| 编译进度 | 不可见（后台编译） | 可见（实时显示） |
| 调试难度 | 高（日志混杂） | 低（逐个验证） |
| 推荐场景 | 日常开发（已编译过） | 首次启动、排查问题 |

---

## 快速选择指南

### 我应该用哪个脚本？

```
┌─────────────────────────────────────────┐
│  你需要测试管理后台功能吗？              │
└─────────────────────────────────────────┘
              │
      ┌───────┴───────┐
      │               │
     是              否
      │               │
      ▼               ▼
start-all.sh    start-dev.sh
（完整环境）    （嵌入式模式）
```

### 具体场景

| 场景 | 推荐脚本 | 原因 |
|------|----------|------|
| 开发聊天功能 | `start-dev.sh` | 嵌入式模式足够，启动快 |
| 开发 DLP 功能 | `start-dev.sh` | 嵌入式模式包含 SafetyBridge |
| 开发技能/扩展 | `start-dev.sh` | 嵌入式模式包含完整功能 |
| 测试配置同步 | `start-all.sh` | 需要 Admin Backend |
| 测试数据上报 | `start-all.sh` | 需要 Admin Backend |
| 开发管理后台 | `start-all.sh` | 需要完整环境 |
| 快速验证功能 | `start-dev.sh` | 启动快，单一进程 |
| 集成测试 | `start-all.sh` | 需要完整环境 |

---

## 环境变量配置

### `start-dev.sh` 配置

```bash
# desktop-client/.env
LLM_BACKEND=anthropic
LLM_API_KEY=sk-ant-xxx
# 可选
LLM_MODEL=claude-sonnet-4-20250514
DATABASE_URL=  # 留空使用默认 libSQL 路径
IRONCLAW_OWNER_ID=default
```

### `start-all.sh` 配置

```bash
# .env 或 ~/.ironclaw/.env
ANTHROPIC_API_KEY=sk-ant-xxx
# 或
OPENAI_API_KEY=sk-xxx
# 或
LLM_API_KEY=sk-xxx
LLM_BACKEND=anthropic
LLM_BASE_URL=  # 可选，仅 openai_compatible 需要
LLM_MODEL=  # 可选
```

---

## 停止服务

### `start-dev.sh`

```bash
# 按 Ctrl+C 停止 Tauri 应用
# 或关闭 Tauri 窗口
```

### `start-all.sh`

```bash
# 按 Ctrl+C 停止所有服务（数据库会继续运行）

# 停止数据库
cd admin-backend
docker-compose down
```

---

## 日志查看

### `start-dev.sh`

```bash
# Tauri 日志直接输出到终端
# 或查看 Tauri 日志文件（如果有）
```

### `start-all.sh`

```bash
# IronClaw 服务器
tail -f /tmp/ironclaw-server.log

# Desktop Client 前端
tail -f /tmp/desktop-frontend.log

# Tauri 客户端
tail -f /tmp/tauri.log

# Admin Backend 后端
tail -f /tmp/admin-backend.log

# Admin Backend 前端
tail -f /tmp/admin-frontend.log

# PostgreSQL
docker logs admin-backend-postgres
```

---

## 常见问题

### Q: 为什么 `start-all.sh` 启动这么慢？

A: 因为需要编译多个 Rust 项目（IronClaw、Tauri、Admin Backend），首次编译可能需要 5-10 分钟。后续启动会快很多（增量编译）。

### Q: 我可以同时运行两个脚本吗？

A: 不可以。两个脚本会争用相同的端口（5173、38080），导致冲突。

### Q: `start-dev.sh` 可以连接到 Admin Backend 吗？

A: 目前不可以。`start-dev.sh` 使用嵌入式模式，不连接外部服务。如果需要管理后台功能，请使用 `start-all.sh`。

### Q: 如何在嵌入式模式下使用管理端配置？

A: 嵌入式模式支持从本地缓存加载管理端配置（`~/Library/Application Support/ironclaw-desktop/admin_config.json`）。配置同步功能需要在运行时手动触发或通过 Admin Backend API 获取。

---

## 总结

- **日常开发 Desktop Client** → 使用 `start-dev.sh`（快速、简单）
- **开发管理后台或测试完整系统** → 使用 `start-all.sh`（完整、复杂）
- **首次启动** → 使用 `start-all.sh --serial`（显示编译进度）
- **快速验证** → 使用 `start-dev.sh`（单一进程，易于调试）
