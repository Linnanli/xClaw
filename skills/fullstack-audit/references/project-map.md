# IronClaw 项目结构映射

本文件记录需求模块与项目文件的对应关系，帮助快速定位检查目标。

## 核心文件路径

### 需求与设计文档
- 需求规格：`.kiro/specs/admin-platform-requirements/requirements.md`
- 技术设计：`.kiro/specs/admin-platform-requirements/design.md`
- 设计图：`admin-backend/ui/design/backend-design.pen`

### 后台服务端（Rust + Axum）
- 入口：`admin-backend/src/main.rs`
- 路由注册：`admin-backend/src/routes.rs`（`create_router` 函数）
- Handler：`admin-backend/src/handlers/mod.rs`（策略查询）+ `admin-backend/src/handlers/departments.rs`（部门扩展）+ `admin-backend/src/routes.rs`（其余 handler，TODO: 逐步迁移到 handlers/）
- 数据模型：`admin-backend/src/models.rs`
- 数据库层：`admin-backend/src/db.rs`
- 认证：`admin-backend/src/auth.rs`
- 错误类型：`admin-backend/src/error.rs`
- 策略管理：`admin-backend/src/policy_management.rs`
- 服务层：`admin-backend/src/services/`（新增模块）
- 迁移文件：`admin-backend/migrations/`
- AppState：`admin-backend/src/lib.rs`

### 后台前端（React + Tailwind + shadcn/ui）

> ⚠️ `admin-backend/ui/` 是当前实际运行的前端（shadcn + tailwind + zustand）。
> `admin-backend/frontend/` 是旧版（Ant Design），已废弃。所有前端修改必须在 `ui/` 目录下进行。

- 路由：`admin-backend/ui/src/router.tsx`
- 页面组件：`admin-backend/ui/src/pages/`
- 公共组件：`admin-backend/ui/src/components/`（shadcn/ui）
- API 客户端：`admin-backend/ui/src/lib/api.ts`
- 状态管理：`admin-backend/ui/src/stores/`（zustand）
- 类型定义：`admin-backend/ui/src/types/`
- 工具函数：`admin-backend/ui/src/lib/`
- 表单辅助：`admin-backend/ui/src/components/ui/form-helpers.tsx`
- 弹窗组件：`admin-backend/ui/src/components/ui/dialog.tsx`（shadcn Dialog）

### 桌面客户端（Tauri）
- Rust 源码：`desktop-client/src/`
- 前端 UI：`desktop-client/src-ui/`
- 命令注册：`desktop-client/src/lib.rs`（`all_tauri_commands!()` 宏）
- Tauri 配置：`desktop-client/tauri.conf.json`

### 测试
- 后端测试：`admin-backend/tests/`
- 冒烟测试：`admin-backend/tests/integration_smoke_tests.rs`
- 前端测试：`admin-backend/ui/src/**/*.test.ts(x)`
- E2E 测试：`admin-backend/ui/cypress/`
- 客户端测试：`desktop-client/tests/`
- 客户端契约测试：`desktop-client/tests/tauri_command_contract_tests.rs`

### 共享 Crate
- 认证：`crates/ironclaw_auth/`
- 主项目 Web Gateway：`src/channels/web/handlers/`

## 需求 → 文件映射

### 已实现模块

| 需求 | 后台 API 路由 | 前端页面 | 迁移文件 |
|------|-------------|---------|---------|
| 1. 认证登录 | `/api/auth/*` | `Login.tsx` | 001 |
| 2. 仪表盘 | `/api/dashboard/*` | `Dashboard.tsx` | — |
| 3. 用户管理 | `/api/users/*`, `/api/roles/*`, `/api/permissions` | `Users/` | 001, 002 |
| 4. 部门管理 | `/api/departments/*` | `Departments/` | 012 |
| 5. DLP | `/api/dlp-rules/*`, `/api/dictionaries/*` | `Security/` | 001, 003, 005, 006 |
| 6. 敏感操作 | `/api/sensitive-operations/*` | `Security/` | 001, 007 |
| 7. 策略版本 | `/api/policy-changes` | `Security/` | 008 |
| 8. 客户端管理 | `/api/clients/*` | `ClientList.tsx` | 009 |
| 9. 客户端配置 | `/api/client-config` | `ClientConfig.tsx` | 011 |
| 10. 模型配置 | `/api/model-configs/*` | `ModelConfigs.tsx` | 013 |
| 11. 审计日志 | `/api/audit-logs/*` | `AuditLog.tsx` | 001 |
| 12. 统计报表 | `/api/reports/*` | `Reports.tsx` | — |
| 13. 系统设置 | `/api/settings` | `Settings.tsx` | 010 |
| 14. 扩展管理 | `/api/skills/*`, `/api/plugins/*` | `Extensions/` | 010 |

### 新增模块（待开发）

| 需求 | 计划 API 路由 | 计划前端页面 | 计划迁移文件 |
|------|-------------|------------|------------|
| 15. 告警通知 | `/api/alert-rules/*`, `/api/alerts/*` | `AlertRuleList`, `AlertEventList` | 015 |
| 16. 对话审计 | `/api/conversations/*` | `ConversationList`, `ConversationDetail` | 016 |
| 17. 知识库 | `/api/knowledge-bases/*` | `KnowledgeBaseList`, `KnowledgeBaseDetail` | 017 |
| 18. Token 配额 | `/api/quota/*` | `QuotaOverview` | 018 |
| 19. 合规分级 | `/api/compliance/*` | `ComplianceOverview` | 019 |
| 20. 水印追踪 | `/api/watermark/*` | `WatermarkConfig` | 020 |
| 21. 审批流 | `/api/approvals/*` | `ApprovalList` | 021 |
| 22. 安全加固 | 中间件层 | — | 022 |

### 已有模块扩展字段

| 需求 | 扩展内容 | 计划迁移 |
|------|---------|---------|
| 1. 认证 | MFA、SSO、账户锁定 | 014 |
| 2. 仪表盘 | AI 对话统计、告警徽标 | — |
| 3. 用户管理 | 批量导入、部门关联 | 014 |
| 4. 部门管理 | 树形架构、Token 消耗排行 | 014 |
| 5. DLP | 数据分级关联 | 014 |
| 8. 客户端 | 设备指纹、版本管控 | 014 |
| 10. 模型配置 | 调用统计 | 014 |
| 11. 审计日志 | IP/UA、全文搜索 | 014 |
| 13. 系统设置 | 告警/安全/水印配置 | — |

## 客户端接入判断

以下验收标准涉及桌面客户端，开发时需要检查客户端是否接入：

- 需求 4（部门管理）：客户端根据用户所属部门的模型白名单获取可用模型列表
- 需求 5（DLP）：客户端发送消息时触发 DLP 扫描
- 需求 8（客户端管理）：心跳、策略推送、版本管控
- 需求 9（客户端配置）：配置下发和拉取
- 需求 16（对话审计）：客户端对话数据上报
- 需求 18（Token 配额）：客户端请求时配额检查
- 需求 20（水印）：客户端导出时嵌入水印
- 需求 21（审批流）：客户端触发敏感操作时创建审批

其他需求主要是管理后台内部功能，不直接涉及客户端。
