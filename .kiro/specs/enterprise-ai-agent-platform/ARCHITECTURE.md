# 企业级 AI Agent 平台 - 架构设计

## 系统概览

```
┌─────────────────────────────────────────────────────────────────┐
│                     用户交互层                                   │
├─────────────────────────────────────────────────────────────────┤
│  桌面客户端 (Tauri)          │  Web 前端 (HTML/CSS/JS)          │
│  - 聊天界面 (本地+离线)      │  - 聊天界面 (云端+实时)          │
│  - 内存管理 (本地)           │  - 内存管理 (云端)               │
│  - 插件管理                  │  - 扩展管理                       │
│  - 本地认证                  │  - 日程管理                       │
│  - 离线模式                  │  - 设置管理                       │
│  - DLP 集成                  │  - OAuth 集成                     │
└─────────────────────────────────────────────────────────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     API 网关层                                   │
├─────────────────────────────────────────────────────────────────┤
│  src/channels/web/ (Axum + Tokio)                               │
│  - 聊天 API (发送、接收、历史)                                   │
│  - 内存 API (读、写、搜索)                                       │
│  - 扩展 API (列表、安装、激活)                                   │
│  - 日程 API (创建、触发、运行)                                   │
│  - 设置 API (获取、设置、导出、导入)                             │
│  - WebSocket 支持                                                │
│  - Server-Sent Events                                            │
│  - OAuth 集成                                                    │
│  - 统一认证中间件 (JWT)                                          │
└─────────────────────────────────────────────────────────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     管理后台层                                   │
├─────────────────────────────────────────────────────────────────┤
│  admin-backend/ (Axum + Tokio)                                  │
│  - 用户管理                                                      │
│  - 审计日志                                                      │
│  - DLP 规则管理                                                  │
│  - 敏感操作规则                                                  │
│  - 统一认证系统 (JWT)                                            │
└─────────────────────────────────────────────────────────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     核心业务层                                   │
├─────────────────────────────────────────────────────────────────┤
│  src/ (Rust)                                                     │
│  - Agent 引擎                                                    │
│  - LLM 集成                                                      │
│  - 工具执行                                                      │
│  - 安全沙箱                                                      │
│  - DLP 引擎                                                      │
│  - 加密管理                                                      │
└─────────────────────────────────────────────────────────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     数据存储层                                   │
├─────────────────────────────────────────────────────────────────┤
│  PostgreSQL (主数据库)                                           │
│  - 用户数据                                                      │
│  - 审计日志                                                      │
│  - 规则配置                                                      │
│  - 技能版本                                                      │
│                                                                  │
│  libSQL (本地数据库)                                             │
│  - 桌面客户端本地存储                                            │
│  - 离线缓存                                                      │
└─────────────────────────────────────────────────────────────────┘
```

## 服务职责划分

### 1. 桌面客户端 (desktop-client/)

**技术栈**: Tauri + Rust + HTML/CSS/JavaScript

**职责**:
- 聊天界面（本地 + 离线支持）
  - 发送消息
  - 接收消息
  - 聊天历史
  - 线程管理
- 内存管理（本地）
  - 读取
  - 写入
  - 搜索
- 插件管理
  - 安装、启用、禁用、更新
- 本地用户认证（主密码）
- 离线模式支持
- DLP 规则本地应用
- 本地数据加密存储
- 审计日志记录

**数据库**: libSQL (本地加密)

**通信**: Tauri IPC + HTTP API

---

### 2. Web 网关 (src/channels/web/)

**技术栈**: Axum + Tokio + PostgreSQL

**职责**:
- 聊天接口（云端 + 实时）
  - 发送消息
  - 接收消息
  - 聊天历史
  - 线程管理
- 内存管理（云端）
  - 读取
  - 写入
  - 搜索
- 扩展管理
  - 列表、安装、激活、移除
- 日程管理
  - 创建、触发、运行、删除
- 设置管理
  - 获取、设置、导出、导入
- WebSocket 支持（实时通信）
- Server-Sent Events（事件流）
- OAuth 集成（第三方认证）
- 统一认证中间件（JWT 验证）

**数据库**: PostgreSQL

**认证**: 共享 `src/auth/` 模块

**前端**: 静态文件 (HTML/CSS/JavaScript)

---

### 3. 管理后台 (admin-backend/)

**技术栈**: Axum + Tokio + PostgreSQL

**职责**:
- 用户管理
  - 注册
  - 登录
  - 令牌刷新
  - 用户信息查询
- 审计日志管理
  - 查询审计日志
  - 日志分析
- DLP 规则管理
  - 获取 DLP 规则
  - 创建/更新规则
  - 规则版本控制
- 敏感操作规则管理
  - 获取敏感操作规则
  - 创建/更新规则
- 系统监控
  - 健康检查
  - 性能指标

**数据库**: PostgreSQL

**认证**: 共享 `src/auth/` 模块

**访问控制**: 仅限管理员

---

### 4. 核心业务层 (src/)

**技术栈**: Rust

**职责**:
- Agent 引擎
- LLM 集成
- 工具执行
- 安全沙箱（WASM）
- DLP 引擎
- 加密管理
- 数据库操作

---

## 认证系统架构

### 共享认证模块 (src/auth/)

```
src/auth/
├── mod.rs          # AuthManager (统一接口)
├── jwt.rs          # JWT 令牌管理
└── password.rs     # 密码哈希管理
```

**功能**:
- JWT 令牌生成和验证
- 密码哈希（Argon2）
- 访问令牌（1小时）
- 刷新令牌（7天）

**使用方式**:

```rust
// 在 src/channels/web/ 中使用
use crate::auth::AuthManager;

let auth = AuthManager::new(jwt_secret);
let token = auth.generate_access_token(user_id)?;
let claims = auth.verify_token(&token)?;
```

```rust
// 在 admin-backend/ 中使用
use ironclaw::auth::AuthManager;

let auth = AuthManager::new(jwt_secret);
let hash = auth.hash_password(password)?;
auth.verify_password(password, &hash)?;
```

---

## 数据库设计

### PostgreSQL 表结构

```sql
-- 用户表
users (id, username, email, password_hash, created_at, updated_at)

-- 技能表
skills (id, name, description, version, author, created_at, updated_at)

-- 技能版本表
skill_versions (id, skill_id, version, changelog, created_at)

-- 审计日志表
audit_logs (id, user_id, action, details, created_at)

-- DLP 规则表
dlp_rules (id, pattern, replacement, severity, created_at)

-- 敏感操作规则表
sensitive_operation_rules (id, operation_type, requires_approval, created_at)

-- 金丝雀部署表
canary_deployments (id, skill_id, version, percentage, status, created_at, updated_at)
```

### libSQL 表结构（桌面客户端）

```sql
-- 本地配置
local_config (key, value, encrypted, created_at, updated_at)

-- 本地审计日志
local_audit_logs (id, action, details, created_at)

-- 缓存 DLP 规则
cached_dlp_rules (id, pattern, replacement, severity, created_at)

-- 下载的技能
downloaded_skills (id, name, version, metadata, created_at)
```

---

## API 端点设计

### Web 网关 (src/channels/web/)

```
聊天接口:
  POST   /api/chat/send              # 发送消息
  GET    /api/chat/history           # 获取历史
  GET    /api/chat/threads           # 获取线程列表
  POST   /api/chat/threads           # 创建新线程
  GET    /api/chat/events            # SSE 事件流
  WebSocket /api/chat/ws             # WebSocket

内存管理:
  GET    /api/memory/tree            # 树形视图
  GET    /api/memory/list            # 列表视图
  GET    /api/memory/read            # 读取
  POST   /api/memory/write           # 写入
  GET    /api/memory/search          # 搜索

扩展管理:
  GET    /api/extensions/list        # 列表
  POST   /api/extensions/install     # 安装
  POST   /api/extensions/activate    # 激活
  DELETE /api/extensions/:id         # 移除

日程管理:
  GET    /api/routines/list          # 列表
  POST   /api/routines/trigger       # 触发
  GET    /api/routines/runs          # 运行记录

设置管理:
  GET    /api/settings/list          # 列表
  GET    /api/settings/:key          # 获取
  POST   /api/settings/:key          # 设置
  DELETE /api/settings/:key          # 删除
```

### 管理后台 (admin-backend/)

```
认证:
  POST   /api/auth/register          # 注册
  POST   /api/auth/login             # 登录
  POST   /api/auth/refresh           # 刷新令牌

用户管理:
  GET    /api/users/:id              # 获取用户信息

审计日志:
  GET    /api/audit-logs             # 获取审计日志

规则管理:
  GET    /api/dlp-rules              # 获取 DLP 规则
  GET    /api/sensitive-operations   # 获取敏感操作规则

系统:
  GET    /health                     # 健康检查
```

---

## 部署架构

### 开发环境

```
localhost:3000  - Web 网关 (src/channels/web/)
localhost:3001  - 管理后台 (admin-backend/)
localhost:5432  - PostgreSQL
```

### 生产环境

```
api.example.com         - Web 网关 (负载均衡)
admin.example.com       - 管理后台 (受限访问)
db.example.com          - PostgreSQL (主从复制)
桌面客户端              - 本地运行 (Tauri)
```

---

## 安全考虑

### 认证

- ✅ JWT 令牌（访问 + 刷新）
- ✅ Argon2 密码哈希
- ✅ 令牌过期时间
- ✅ 刷新令牌轮换

### 授权

- ✅ 基于角色的访问控制 (RBAC)
- ✅ 管理后台仅限管理员
- ✅ 用户只能访问自己的数据

### 数据保护

- ✅ 传输层加密 (HTTPS/TLS)
- ✅ 数据库加密 (PostgreSQL)
- ✅ 本地存储加密 (libSQL + AES-256-GCM)
- ✅ DLP 数据脱敏

### 审计

- ✅ 所有操作记录在审计日志
- ✅ 敏感操作需要批准
- ✅ 审计日志加密存储

---

## 扩展性考虑

### 水平扩展

- Web 网关可以部署多个实例（无状态）
- 管理后台可以部署多个实例（无状态）
- PostgreSQL 使用主从复制
- 使用负载均衡器分发流量

### 垂直扩展

- 增加服务器资源
- 优化数据库查询
- 使用缓存（Redis）

### 微服务化

- 将管理后台分离为独立服务
- 将 Agent 引擎分离为独立服务
- 使用消息队列（RabbitMQ/Kafka）

---

## 技术栈总结

| 组件 | 技术 | 版本 |
|------|------|------|
| 桌面客户端 | Tauri + Rust | 2.x |
| Web 框架 | Axum | 0.8 |
| 异步运行时 | Tokio | 1.x |
| 数据库 | PostgreSQL | 14+ |
| 本地数据库 | libSQL | 0.6+ |
| 认证 | JWT + Argon2 | - |
| 加密 | AES-256-GCM | - |
| 前端 | HTML/CSS/JavaScript | - |

---

**最后更新**: 2026-03-15
**版本**: 1.0
**状态**: 架构设计完成

