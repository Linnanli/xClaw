# 改造完成后：客户端与管理端能力全景图

> 生成日期：2026-03-21
> 基于架构：客户端本地 AI + 管理端集中管理
> 前置文档：`IRONCLAW_PACKAGING_AND_MODIFICATION_ANALYSIS.md`、`CLIENT_LOCAL_AI_ADMIN_CENTRAL_MGMT_ANALYSIS.md`

---

## 一、架构总览

```
┌──────────────────────────────────────────────────────────────────┐
│                     Admin-Backend（管理端）                        │
│                  统一管理控制台 · 策略中心 · 审计中心                │
│                                                                  │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐            │
│  │ 用户管理  │ │ 安全策略  │ │ 审计中心  │ │ 客户端管理│            │
│  │ RBAC     │ │ DLP 规则  │ │ 操作日志  │ │ 注册/心跳 │            │
│  └──────────┘ └────┬─────┘ └────▲─────┘ └──────────┘            │
│                    │策略下发     │审计上报                         │
└────────────────────┼────────────┼────────────────────────────────┘
                     │            │
              ═══════╪════════════╪═══════  网络边界
                     │            │
┌────────────────────┼────────────┼────────────────────────────────┐
│                    ▼            │                                 │
│  Desktop-Client（客户端）        │                                 │
│  ┌─────────────────────────────┴──┐                              │
│  │         Tauri 应用层            │                              │
│  │  策略同步 · 审计上报 · Sidecar管理│                              │
│  └──────────────┬─────────────────┘                              │
│                 │ localhost API                                   │
│  ┌──────────────▼─────────────────┐                              │
│  │    IronClaw Sidecar（AI 引擎）   │                              │
│  │  Agent · LLM · Tools · Safety  │                              │
│  │  Gateway API (:38080)          │                              │
│  └────────────────────────────────┘                              │
└──────────────────────────────────────────────────────────────────┘
```

---

## 二、Desktop-Client 能力清单

### 🧠 AI 核心能力（来自 IronClaw Sidecar）

| 能力 | 说明 | 来源 |
|------|------|------|
| AI 对话 | 多轮对话、上下文管理、线程管理 | IronClaw Agent |
| LLM 调用 | 多模型支持（OpenAI/Anthropic/Bedrock/本地） | IronClaw LLM |
| 工具执行 | 20+ 内置工具 + WASM 沙箱 + MCP 协议 | IronClaw Tools |
| 技能系统 | 动态加载 AI 提示词扩展 | IronClaw Skills |
| 扩展系统 | MCP Server / WASM Tool / Channel Relay | IronClaw Extensions |
| 例程自动化 | Cron 调度 / 事件触发 / Webhook | IronClaw Routines |
| 记忆系统 | 文件树 / 读写 / 语义搜索 | IronClaw Memory |
| 任务管理 | 后台任务 / 容器沙箱 / 任务调度 | IronClaw Jobs |
| SSE 实时流 | 流式响应 / 思考过程 / 状态事件 | IronClaw Gateway |

### 🔒 安全能力（IronClaw Safety + 管理端策略）

| 能力 | 说明 | 来源 |
|------|------|------|
| 提示注入防护 | 18 个 AhoCorasick 模式 + 4 个正则 | IronClaw Sanitizer（默认） |
| 安全策略检查 | 7 条默认规则（系统文件/注入/加密密钥等） | IronClaw Policy（默认） |
| 凭证泄露检测 | 16 个模式（API Key/Token/PEM 等） | IronClaw LeakDetector（默认） |
| 输出长度限制 | 防止超长输出 | IronClaw Validator |
| **管理端 DLP 规则** | 正则 + 关键词类型，动态下发 | **管理端 → 策略同步** |
| **管理端字典规则** | 关键词词库，动态下发 | **管理端 → 策略同步** |
| **管理端敏感操作规则** | 操作风险分类，动态下发 | **管理端 → 策略同步** |
| 用户输入 DLP 扫描 | 发送前扫描敏感信息 | Desktop-Client DLP |
| 出站请求检查 | HTTP 请求内容检查 | Desktop-Client DLP |
| 存储内容清理 | 本地存储脱敏 | Desktop-Client DLP |

### 🖥️ 客户端本地能力（Desktop-Client 独有）

| 能力 | 说明 | 实现 |
|------|------|------|
| 主密码保护 | 本地加密存储，Argon2 哈希 | `auth.rs` |
| 会话管理 | 自动锁定、会话超时 | `auth_token_manager.rs` |
| 离线模式 | 网络断开时本地功能可用 | `offline_mode.rs` |
| 本地配置管理 | 加密存储配置 | `config_manager.rs` |
| 本地审计日志 | 操作记录本地存储 | `storage.rs` |
| 环境一致性检查 | 检测运行环境问题 | `environment_checker.rs` |
| Token 自动刷新 | JWT token 过期自动续期 | `token_refresh_service.rs` |
| 动态水印 | 防截图水印 | `DynamicWatermark.tsx` |

### 🔄 与管理端交互能力（改造后新增）

| 能力 | 说明 | 数据流向 |
|------|------|---------|
| **策略同步** | 定时拉取管理端最新 DLP 规则 | 管理端 → 客户端 |
| **审计上报** | DLP 事件、安全违规上报管理端 | 客户端 → 管理端 |
| **客户端注册** | 启动时向管理端注册 | 客户端 → 管理端 |
| **心跳上报** | 定时上报在线状态 | 客户端 → 管理端 |
| **Sidecar 管理** | 启动/停止/重启 IronClaw 进程 | 本地 |
| **配置注入** | 将管理端地址、规则注入 Sidecar | 本地 |

### 📱 前端 UI 页面

| 页面/Tab | 功能 | 对应 Tauri 命令 |
|----------|------|----------------|
| ChatTab | AI 对话、SSE 流式响应 | `send_chat_message`, `subscribe_chat_events` |
| MemoryTab | 记忆树浏览、读写、搜索 | `get_memory_tree`, `read_memory`, `write_memory` |
| JobsTab | 后台任务列表、详情、取消 | `get_jobs`, `get_job_detail`, `cancel_job` |
| ExtensionsTab | 扩展列表、安装、卸载 | `get_installed_extensions`, `install_extension` |
| SkillsTab | 技能列表、安装、卸载 | `get_installed_skills`, `install_skill` |
| RoutinesTab | 例程列表、触发、启停 | `get_routines`, `trigger_routine` |
| LogsTab | 日志查看、搜索、导出 | `get_logs`, `search_logs`, `export_logs` |
| SettingsTab | 配置管理 | `store_config`, `get_config` |
| DLP 状态指示器 | DLP 扫描状态显示 | `get_dlp_config`, `get_dlp_statistics` |
| 连接状态 | IronClaw 连接状态 | `ConnectionStatus.tsx` |
| 密码登录/设置 | 主密码认证 | `setup_master_password`, `unlock_app` |

---

## 三、Admin-Backend 能力清单

### 👥 用户与权限管理

| 能力 | 说明 | API |
|------|------|-----|
| 用户注册/登录 | JWT 认证（access + refresh token） | `POST /auth/register`, `POST /auth/login` |
| 用户 CRUD | 创建、查看、删除用户 | `GET/POST/DELETE /users` |
| 角色管理 | 角色 CRUD、权限分配 | `GET/POST/PUT/DELETE /roles` |
| 权限管理 | 权限列表、角色权限分配 | `GET /permissions`, `POST /roles/:id/permissions` |
| 用户角色分配 | 给用户分配角色 | `POST /users/:id/roles` |

### 🛡️ 安全策略管理（DLP）

| 能力 | 说明 | API |
|------|------|-----|
| DLP 规则 CRUD | 正则 + 关键词类型规则管理 | `GET/POST/PUT/DELETE /dlp-rules` |
| 批量操作 | 批量启用/禁用/删除 | `POST /dlp-rules/batch-*` |
| 导入/导出 | JSON 格式规则导入导出 | `GET /dlp-rules/export`, `POST /dlp-rules/import` |
| 字典管理 | 关键词词库 CRUD | `GET/POST/PUT/DELETE /dictionaries` |
| 敏感操作管理 | 操作风险分类、审批者角色 | `GET/POST/PUT/DELETE /sensitive-operations` |
| 策略版本控制 | 变更记录、版本号、时间线 | `GET /policy-changes` |
| 策略变更统计 | 按类型/时间统计变更 | `GET /policy-changes/stats` |
| **策略下发** | 将规则推送到客户端执行 | **改造后新增** |

### 📊 审计与监控

| 能力 | 说明 | API |
|------|------|-----|
| 审计日志查询 | 按用户/操作/时间筛选 | `GET /audit-logs` |
| 客户端审计上报 | 接收客户端安全事件 | `POST /audit-logs/report` |
| 客户端管理 | 注册、心跳、在线状态 | `GET /clients`, `GET /clients/:id` |
| 客户端统计 | 在线/离线/总数统计 | `GET /clients/stats` |
| 统计报表 | DLP 拦截统计、分布图表 | Reports 页面 |

### 🔌 扩展管理（代理 IronClaw）

| 能力 | 说明 | API |
|------|------|-----|
| 技能列表 | 代理 IronClaw Skills API | `GET /skills` |
| 插件列表 | 代理 IronClaw Extensions API | `GET /plugins` |
| 本地回退 | Gateway 不可用时读本地 DB | 自动降级 |

### ⚙️ 系统配置

| 能力 | 说明 | API |
|------|------|-----|
| DLP 配置 | 启用/超时/失败策略 | `GET/PUT /settings` |
| 审计配置 | 保留天数、启用开关 | `GET/PUT /settings` |
| 客户端配置 | 心跳间隔、离线阈值 | `GET/PUT /settings` |
| 策略同步配置 | 同步间隔、自动推送 | `GET/PUT /settings` |

### 📱 前端 UI 页面

| 页面 | 路由 | 功能 |
|------|------|------|
| Dashboard | `/dashboard` | 总览仪表盘 |
| UserList | `/users/list` | 用户列表管理 |
| RoleList | `/users/roles` | 角色管理 |
| PermissionList | `/users/permissions` | 权限管理 |
| DlpRuleList | `/security/dlp-rules` | DLP 规则管理 |
| DlpDictionaryList | `/security/dictionaries` | 字典管理 |
| SensitiveOpList | `/security/sensitive-ops` | 敏感操作管理 |
| PolicyVersionList | `/security/policy-versions` | 策略版本时间线 |
| AuditLog | `/audit` | 审计日志查询 |
| ClientList | `/clients` | 客户端管理 |
| SkillList | `/extensions/skills` | 技能列表（代理） |
| PluginList | `/extensions/plugins` | 插件列表（代理） |
| Settings | `/settings` | 系统配置 |
| Reports | `/reports` | 统计报表 |

---

## 四、能力归属矩阵

### 按功能域划分

| 功能域 | Desktop-Client | Admin-Backend | 共享 |
|--------|:-------------:|:-------------:|:----:|
| **AI 对话** | ✅ 执行 | ❌ | — |
| **LLM 调用** | ✅ 执行 | ❌ | — |
| **工具执行** | ✅ 执行 | ❌ | — |
| **技能管理** | ✅ 使用 | ✅ 查看（代理） | — |
| **扩展管理** | ✅ 使用 | ✅ 查看（代理） | — |
| **例程管理** | ✅ 使用 | ❌ | — |
| **记忆系统** | ✅ 使用 | ❌ | — |
| **任务管理** | ✅ 使用 | ❌ | — |
| **DLP 规则配置** | ❌ | ✅ 管理 | — |
| **DLP 规则执行** | ✅ 执行 | ❌ | — |
| **字典管理** | ❌ | ✅ 管理 | — |
| **敏感操作管理** | ❌ | ✅ 管理 | — |
| **策略版本控制** | ❌ | ✅ 管理 | — |
| **策略同步** | ✅ 拉取 | ✅ 下发 | 双向 |
| **审计上报** | ✅ 上报 | ✅ 接收/查询 | 双向 |
| **用户管理** | ❌ | ✅ 管理 | — |
| **RBAC 权限** | ❌ | ✅ 管理 | — |
| **客户端管理** | ✅ 注册/心跳 | ✅ 查看/管理 | 双向 |
| **统计报表** | ❌ | ✅ 查看 | — |
| **主密码保护** | ✅ 本地 | ❌ | — |
| **离线模式** | ✅ 本地 | ❌ | — |
| **认证** | 主密码 | JWT + RBAC | `ironclaw_auth` |
| **安全基础** | IronClaw Safety | — | `ironclaw_safety` |

### 按数据流向划分

```
管理端 ──策略下发──→ 客户端
  │                    │
  │  DLP 规则           │  接收规则
  │  字典规则           │  注入 IronClaw Safety
  │  敏感操作规则       │  本地执行
  │                    │
  │                    │
管理端 ←──审计上报──── 客户端
  │                    │
  │  接收事件           │  DLP 拦截事件
  │  存储审计日志       │  安全违规事件
  │  生成报表           │  用户操作审计
  │                    │
  │                    │
管理端 ←──心跳上报──── 客户端
  │                    │
  │  更新在线状态       │  定时心跳
  │  客户端统计         │  客户端注册
```

---

## 五、共享 Crate 使用情况

| Crate | Desktop-Client | Admin-Backend | IronClaw |
|-------|:--------------:|:-------------:|:--------:|
| `ironclaw_auth` | ✅ 密码哈希 | ✅ JWT + 密码哈希 | ❌ |
| `ironclaw_safety` | ✅ 通过 Sidecar | ❌ | ✅ 核心使用 |

---

## 六、改造前后对比

### 改造前

```
Desktop-Client                    Admin-Backend
┌──────────────┐                 ┌──────────────┐
│ 本地 AI 对话  │                 │ 用户管理      │
│ 本地 DLP     │  ← 无连接 →     │ DLP 规则管理  │
│ 本地存储     │                 │ 审计日志      │
└──────────────┘                 └──────────────┘
   各自独立，数据孤岛
```

**问题**：
- 客户端 DLP 规则硬编码，管理员无法远程更新
- 客户端安全事件无法上报，管理员看不到
- 管理端配置的规则无法到达客户端执行
- 客户端在线状态管理端不知道

### 改造后

```
Desktop-Client                    Admin-Backend
┌──────────────┐   策略下发      ┌──────────────┐
│ 本地 AI 对话  │ ←────────────── │ DLP 规则管理  │
│ IronClaw     │                 │ 字典管理      │
│ Safety 执行  │   审计上报      │ 敏感操作管理  │
│ (默认+动态)  │ ──────────────→ │ 审计日志中心  │
│              │   心跳/注册     │ 客户端管理    │
│ 策略同步服务  │ ──────────────→ │ 统计报表      │
└──────────────┘                 └──────────────┘
   数据互通，集中管控
```

**收益**：
- 管理员可远程更新所有客户端的 DLP 规则
- 安全事件集中审计，生成合规报表
- 客户端在线状态实时可见
- AI 能力仍在本地执行，数据不出客户端

---

## 七、能力边界原则

| 原则 | 说明 |
|------|------|
| **AI 在客户端** | Agent、LLM、Tools、Memory 等 AI 能力全部在客户端本地执行 |
| **管理在管理端** | 用户、权限、规则配置、审计查询等管理能力在管理端 |
| **策略单向下发** | 管理端制定规则 → 客户端执行规则，客户端不能修改规则 |
| **审计单向上报** | 客户端产生事件 → 上报管理端，管理端不能修改客户端日志 |
| **数据不出客户端** | 用户的 AI 对话内容、记忆数据留在客户端本地，不上传管理端 |
| **安全事件可上报** | DLP 拦截统计、违规事件摘要可上报（不含原始敏感内容） |

---

## 八、Tauri 命令完整清单（100+）

### 认证与会话（6 个）
`check_setup_status` · `setup_master_password` · `unlock_app` · `get_session_info` · `lock_app` · `update_session_activity`

### 配置管理（5 个）
`store_config` · `get_config` · `get_app_init_info` · `get_app_config` · `get_network_config`

### AI 对话（7 个）
`get_threads` · `create_thread` · `send_message` · `get_messages` · `send_chat_message` · `subscribe_chat_events` · `unsubscribe_chat_events`

### 消息操作（4 个）
`edit_message` · `delete_message` · `search_messages` · `export_thread`

### 审批（2 个）
`approve_operation` · `deny_operation`

### 扩展管理（8 个）
`get_installed_extensions` · `get_available_extensions` · `install_extension` · `uninstall_extension` · `enable_extension` · `disable_extension` · `search_extensions` · `get_enabled_tools`

### 技能管理（6 个）
`get_available_skills` · `get_installed_skills` · `install_skill` · `uninstall_skill` · `enable_skill` · `disable_skill`

### 插件管理（8 个）
`get_installed_plugins` · `get_available_plugins` · `check_plugin_updates` · `install_plugin` · `uninstall_plugin` · `enable_plugin` · `disable_plugin` · `update_plugin`

### 例程管理（8 个）
`get_routines` · `create_routine` · `delete_routine` · `trigger_routine` · `enable_routine` · `disable_routine` · `pause_routine` · `get_routine_runs`

### 记忆系统（5 个）
`get_memory_tree` · `read_memory` · `write_memory` · `search_memory` · `delete_memory_local` · `is_memory_file_protected`

### 任务管理（4 个）
`get_jobs` · `get_job_detail` · `cancel_job` · `restart_job`

### 日志管理（5 个）
`get_logs` · `search_logs` · `filter_logs` · `export_logs` · `clear_logs`

### DLP 安全（8 个）
`scan_user_input` · `scan_outbound_request` · `sanitize_for_storage` · `check_http_request` · `get_dlp_config` · `update_dlp_config` · `get_dlp_statistics` · `sync_dlp_rules_from_admin`

### 离线模式（5 个）
`get_offline_state` · `enable_offline_mode` · `disable_offline_mode` · `get_offline_capabilities` · `can_perform_operation`

### 审计（2 个）
`log_audit_event` · `get_audit_logs`

### 其他（4 个）
`get_auth_token` · `refresh_auth_token` · `check_environment_consistency` · `upload_file`

---

## 九、Admin-Backend API 完整清单（40+）

### 认证（3 个）
`POST /auth/register` · `POST /auth/login` · `POST /auth/refresh`

### 用户管理（5 个）
`GET /users` · `POST /users` · `GET /users/:id` · `DELETE /users/:id` · `POST /users/:id/roles`

### 角色权限（7 个）
`GET /roles` · `POST /roles` · `GET /roles/:id` · `PUT /roles/:id` · `DELETE /roles/:id` · `POST /roles/:id/permissions` · `GET /permissions`

### DLP 规则（8 个）
`GET /dlp-rules` · `POST /dlp-rules` · `PUT /dlp-rules/:id` · `DELETE /dlp-rules/:id` · `POST /dlp-rules/batch-status` · `POST /dlp-rules/batch-delete` · `GET /dlp-rules/export` · `POST /dlp-rules/import`

### 字典管理（5 个）
`GET /dictionaries` · `POST /dictionaries` · `GET /dictionaries/:id` · `PUT /dictionaries/:id` · `DELETE /dictionaries/:id`

### 敏感操作（4 个）
`GET /sensitive-operations` · `POST /sensitive-operations` · `PUT /sensitive-operations/:id` · `DELETE /sensitive-operations/:id`

### 审计日志（2 个）
`GET /audit-logs` · `POST /audit-logs/report`

### 策略变更（2 个）
`GET /policy-changes` · `GET /policy-changes/stats`

### 客户端管理（4 个）
`GET /clients` · `GET /clients/:id` · `DELETE /clients/:id` · `GET /clients/stats`

### 扩展代理（2 个）
`GET /skills` · `GET /plugins`

### 系统配置（2 个）
`GET /settings` · `PUT /settings`

### 基础（1 个）
`GET /health`
