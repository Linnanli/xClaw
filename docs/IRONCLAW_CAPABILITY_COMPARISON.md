# IronClaw 与 Admin-Backend / Desktop-Client 功能对比分析

> 生成日期：2026-03-21
> 目的：明确 IronClaw 主项目与 Admin-Backend、Desktop-Client 之间的功能边界，避免重复实现，指导架构决策。

## 一、IronClaw Skills 系统 vs Admin-Backend 技能管理

### IronClaw Skills 的真实能力

- 基于 `SKILL.md` 文件的提示词扩展系统（YAML 前置元数据 + Markdown 提示词）
- 技能存储在文件系统中（`~/.ironclaw/skills/` 或 `workspace/skills/`），不是数据库
- 数据字段：name、description、version、trust（信任级别）、source（来源）、keywords
- 管理 API：
  - `GET /api/skills` — 列出已安装技能
  - `POST /api/skills/search` — 搜索技能（本地 + ClawHub 注册表）
  - `POST /api/skills/install` — 安装技能（需 `X-Confirm-Action: true` 头）
  - `DELETE /api/skills/{name}` — 移除技能
- 信任模型：
  - `Installed`（注册表/外部技能）— 仅读权限工具
  - `Trusted`（用户本地技能）— 完全工具访问权限
- 安全特性：技能名称验证、XML 属性转义、内容转义、行尾规范化

### Admin-Backend 技能管理的现状

- 数据库表 `skills`（id、name、description、version、author、enabled）
- 前端只有列表展示，无 CRUD 操作
- 当前代理到 IronClaw Gateway 的 `/api/skills`，Gateway 不可用时回退本地数据库

### 结论：概念不完全匹配，但代理方案可行

- IronClaw 的 Skills 是"AI 提示词扩展"，不是传统意义的"技能模块管理"
- Admin-Backend 代理 IronClaw 的 skills API 是合理的，可以展示当前安装的 AI 技能
- Admin-Backend 的本地 `skills` 数据库表是冗余的，建议删除
- 前端应适配 IronClaw 的字段（trust、source、keywords）

---

## 二、IronClaw Extensions 系统 vs Admin-Backend 插件管理

### IronClaw Extensions 的真实能力

- 4 种扩展类型：
  - `McpServer` — 托管 MCP 服务器，HTTP 传输，OAuth 2.1 认证
  - `WasmTool` — 沙箱 WASM 模块，文件基础，能力认证
  - `WasmChannel` — WASM 通道模块，热激活支持
  - `ChannelRelay` — 外部通道（Slack 等）通过 channel-relay 服务
- 完整的生命周期管理：发现 → 安装 → 认证 → 激活
- 管理 API：
  - `GET /api/extensions` — 列出已安装扩展
  - `GET /api/extensions/tools` — 列出工具
  - `GET /api/extensions/registry` — 列出注册表条目
  - `POST /api/extensions/install` — 安装扩展
  - `POST /api/extensions/{name}/activate` — 激活扩展
  - `POST /api/extensions/{name}/remove` — 移除扩展
  - `GET /api/extensions/{name}/setup` — 获取设置表单
  - `POST /api/extensions/{name}/setup` — 提交设置
- 数据字段：name、display_name、kind、description、authenticated、active、tools、needs_setup、has_auth、activation_status、version

### Admin-Backend 插件管理的现状

- 数据库表 `plugins`（id、name、description、version、author、enabled）
- 前端只有列表展示
- 当前代理到 IronClaw Gateway 的 `/api/extensions`，字段映射为 plugins 格式

### 结论：代理方案合理，但字段映射需要优化

- IronClaw 的 Extensions 比 Admin-Backend 的 "plugins" 概念更丰富
- 当前的字段映射丢失了 kind、authenticated、active、tools 等关键信息
- Admin-Backend 的本地 `plugins` 数据库表是冗余的，建议删除
- 前端应该展示更多 IronClaw 扩展的真实信息（类型、认证状态、激活状态、工具列表）

---

## 三、IronClaw Settings vs Admin-Backend 系统配置

### IronClaw Settings 的真实能力

- 通用 key-value 存储（`settings` 表），按 user_id 隔离
- API：
  - `GET /api/settings` — 列出所有设置
  - `GET /api/settings/{key}` — 获取单个设置
  - `PUT /api/settings/{key}` — 设置值
  - `DELETE /api/settings/{key}` — 删除设置
  - `GET /api/settings/export` — 导出所有设置
  - `POST /api/settings/import` — 导入设置
- 存储的是 IronClaw 运行时配置（LLM 模型、通道配置等）
- 配置优先级：环境变量 > 数据库 > 默认值

### Admin-Backend 系统配置的现状

- 专用 `system_settings` 表（key-value + JSONB）
- 存储内容：
  - DLP 配置（dlp_enabled、dlp_scan_timeout_ms、dlp_fail_open）
  - 审计配置（audit_retention_days、audit_enabled）
  - 客户端配置（client_heartbeat_interval_s、client_offline_threshold_s）
  - 策略同步配置（policy_sync_interval_s、policy_auto_push）

### 结论：不重叠，各自独立

- IronClaw 的 Settings 是 AI 代理的运行时配置
- Admin-Backend 的 Settings 是安全管理平台的配置
- 两者服务不同的目的，不需要合并

---

## 四、IronClaw Safety vs Admin-Backend DLP 管理

### IronClaw Safety（`ironclaw_safety` crate）的真实能力

- 硬编码的安全规则（7 条默认策略规则）：
  1. `system_file_access` — 阻止系统文件访问（Critical/Block）
  2. `crypto_private_key` — 检测加密私钥（Critical/Block）
  3. `sql_pattern` — SQL 注入警告（Medium/Warn）
  4. `shell_injection` — Shell 命令注入阻止（Critical/Block）
  5. `excessive_urls` — 过多 URL 警告（Low/Warn）
  6. `encoded_exploit` — 编码攻击清理（High/Sanitize）
  7. `obfuscated_string` — 混淆字符串警告（Medium/Warn）
- 提示注入检测和清理（Sanitizer）
- 凭证泄露检测（credential_detect、leak_detector）
- 输出长度限制
- **不支持动态规则管理**（规则在代码中硬编码）
- **不支持关键词词库**
- **不支持敏感操作管理**

### Admin-Backend DLP 管理的现状

- 完整的 DLP 规则 CRUD（正则 + 关键词类型）
- 字典管理（关键词词库 CRUD）
- 敏感操作管理（CRUD + 风险级别 + 审批者角色）
- 批量操作（启用/禁用/删除）
- 导入/导出
- 策略版本控制
- 策略变更记录和审计

### 结论：互补关系，不重叠

- IronClaw Safety 是**运行时安全层**（硬编码规则，防注入/防泄露）
- Admin-Backend DLP 是**管理层**（动态规则配置，面向管理员）
- 两者是"管理面"和"执行面"的关系
- 未来应实现"策略下发"机制：Admin-Backend 配置的规则 → 推送到 IronClaw 客户端的 Safety 层执行

---

## 五、IronClaw 认证 vs Admin-Backend 认证

### IronClaw 认证

- 单用户模式（owner_id）
- Gateway 使用 token 认证（启动时生成，命令行输出）
- 无用户管理、无角色权限
- 无注册/登录流程

### Admin-Backend 认证

- 多用户管理（注册/登录）
- JWT 认证（access token + refresh token）
- RBAC（角色 + 权限 + 用户角色分配）
- 共享 `ironclaw_auth` crate（密码哈希 Argon2 + JWT 生成/验证）

### 结论：不重叠

- IronClaw 是单用户 AI 代理
- Admin-Backend 是多用户管理平台
- 共享 `ironclaw_auth` crate 是正确的复用方式

---

## 六、其他功能对比

### 审计日志

| 维度 | IronClaw | Admin-Backend |
|------|----------|---------------|
| 存储 | 数据库（conversations/messages） | 专用 audit_logs 表 |
| 粒度 | 对话级别 | 操作级别（用户操作、策略变更） |
| 客户端上报 | 无 | 支持（`POST /api/audit-logs/report`） |
| 策略变更追踪 | 无 | 完整（policy_change_records 表） |
| 多用户 | 单用户 | 多用户 |

**结论**：部分重叠，但 Admin-Backend 的审计系统更完整，面向不同场景。

### 客户端管理

- IronClaw：无客户端管理概念
- Admin-Backend：完整的客户端注册、心跳、在线状态、统计

**结论**：不重叠，Admin-Backend 独有。

### 策略版本控制

- IronClaw：无策略版本概念
- Admin-Backend：完整的策略变更记录、版本号管理、变更统计

**结论**：不重叠，Admin-Backend 独有。

---

## 七、功能重叠总结表

| 功能领域 | IronClaw 能力 | Admin-Backend 能力 | 重叠程度 | 建议 |
|---------|-------------|-------------------|---------|------|
| 技能管理 | 完整（文件系统 + API） | 仅列表展示（代理） | ⚠️ 代理可行 | 删除本地 skills 表，优化字段映射 |
| 扩展/插件 | 完整（发现/安装/激活） | 仅列表展示（代理） | ⚠️ 代理可行 | 删除本地 plugins 表，展示更多扩展信息 |
| 系统配置 | 通用 key-value（AI 运行时） | 专用安全管理配置 | ✅ 不重叠 | 各自独立，无需合并 |
| DLP 规则 | 硬编码安全规则（7 条） | 动态 CRUD 管理 | ✅ 互补 | 管理面 vs 执行面，未来实现策略下发 |
| 字典管理 | 无 | 完整 CRUD | ✅ 不重叠 | Admin-Backend 独有 |
| 敏感操作 | 无 | 完整 CRUD | ✅ 不重叠 | Admin-Backend 独有 |
| 审计日志 | 基础（对话级别） | 完整审计系统 | ⚠️ 部分重叠 | Admin-Backend 更完整，面向管理场景 |
| 用户管理 | 无（单用户） | 完整 RBAC | ✅ 不重叠 | Admin-Backend 独有 |
| 客户端管理 | 无 | 完整管理 | ✅ 不重叠 | Admin-Backend 独有 |
| 策略版本 | 无 | 完整版本控制 | ✅ 不重叠 | Admin-Backend 独有 |

---

## 八、优化建议

### 短期优化

1. **Skills API 优化**
   - 删除 Admin-Backend 本地 `skills` 数据库表（冗余）
   - 前端适配 IronClaw 字段：展示 trust（信任级别）、source（来源）、keywords（关键词）
   - 保留 Gateway 不可用时的空列表回退

2. **Plugins/Extensions API 优化**
   - 删除 Admin-Backend 本地 `plugins` 数据库表（冗余）
   - 优化字段映射：保留 kind（扩展类型）、authenticated（认证状态）、active（激活状态）、tools（工具列表）
   - 前端展示扩展类型标签（MCP Server / WASM Tool / WASM Channel / Channel Relay）

3. **前端展示优化**
   - SkillList 页面：展示信任级别标签、来源标签、关键词标签
   - PluginList 页面：展示扩展类型、认证状态、激活状态、工具数量

### 中期规划

4. **策略下发机制**
   - Admin-Backend 配置的 DLP 规则 → 通过 API 推送到 IronClaw 客户端
   - 实现 IronClaw Safety 层的动态规则加载
   - 策略版本同步和一致性验证

5. **审计日志集成**
   - Admin-Backend 可以通过 IronClaw Gateway API 获取 AI 代理的操作日志
   - 统一审计视图：管理操作 + AI 代理操作

### 长期规划

6. **统一管理面**
   - Admin-Backend 作为所有 IronClaw 实例的集中管理平台
   - 多实例管理：多个 IronClaw 客户端的统一配置和监控
   - 策略合规性报告

---

## 九、架构关系图

```
┌─────────────────────────────────────────────────────────┐
│                    Admin-Backend                         │
│  （管理面：多用户、RBAC、DLP 规则管理、审计、策略版本）      │
│                                                         │
│  独有能力：                                               │
│  ├── 用户管理 + RBAC                                     │
│  ├── DLP 规则 CRUD + 字典管理                             │
│  ├── 敏感操作管理                                         │
│  ├── 客户端管理                                           │
│  ├── 策略版本控制                                         │
│  ├── 审计日志系统                                         │
│  └── 系统配置（安全管理相关）                               │
│                                                         │
│  代理能力（来自 IronClaw Gateway）：                       │
│  ├── GET /api/skills → IronClaw /api/skills              │
│  └── GET /api/plugins → IronClaw /api/extensions         │
└──────────────────────┬──────────────────────────────────┘
                       │ HTTP 代理
                       ▼
┌─────────────────────────────────────────────────────────┐
│              IronClaw Web Gateway (:38080)                │
│  （执行面：AI 代理、聊天、工具、扩展、技能）                 │
│                                                         │
│  核心能力：                                               │
│  ├── 聊天 API（发送/历史/线程/SSE）                        │
│  ├── 内存 API（树/列表/读写/搜索）                         │
│  ├── 任务 API（列表/详情/取消/重启）                       │
│  ├── 技能管理（列表/搜索/安装/移除）                       │
│  ├── 扩展管理（列表/安装/激活/移除/设置）                   │
│  ├── 例程管理（列表/触发/启停/删除）                       │
│  ├── 日志 API（事件流/级别控制）                           │
│  ├── 设置 API（CRUD/导入/导出）                           │
│  └── Safety 层（硬编码安全规则）                           │
│                                                         │
│  共享 Crate：                                             │
│  ├── ironclaw_auth（密码哈希 + JWT）                      │
│  └── ironclaw_safety（注入防护 + 策略检查）                │
└─────────────────────────────────────────────────────────┘
```

---

## 十、参考文件

| 文件 | 说明 |
|------|------|
| `ironclaw/src/channels/web/server.rs` | IronClaw Web Gateway 路由定义 |
| `ironclaw/src/channels/web/types.rs` | IronClaw API 请求/响应类型 |
| `ironclaw/src/channels/web/handlers/skills.rs` | IronClaw Skills API 处理器 |
| `ironclaw/src/channels/web/handlers/extensions.rs` | IronClaw Extensions API 处理器 |
| `ironclaw/src/skills/mod.rs` | IronClaw Skills 系统核心 |
| `ironclaw/src/extensions/mod.rs` | IronClaw Extensions 系统核心 |
| `ironclaw/crates/ironclaw_safety/src/lib.rs` | IronClaw Safety 层 |
| `ironclaw/crates/ironclaw_safety/src/policy.rs` | IronClaw 安全策略规则 |
| `ironclaw/src/config/mod.rs` | IronClaw 配置系统 |
| `ironclaw/src/db/mod.rs` | IronClaw 数据库接口（含 SettingsStore） |
| `crates/ironclaw_auth/src/lib.rs` | 共享认证 crate |
| `admin-backend/src/routes.rs` | Admin-Backend API 路由 |
| `admin-backend/src/models.rs` | Admin-Backend 数据模型 |
