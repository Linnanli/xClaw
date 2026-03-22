# 需求文档：Admin Backend 待开发功能

## 简介

本文档描述 Admin Backend（管理后台）中尚未实现的七项核心功能的需求。这些功能涵盖仪表盘真实数据接入、技能/插件启用禁用管理、用户编辑、客户端配置下发管理界面、审计日志导出、客户端强制下线与策略推送，以及部门管理。

当前代码库中，后端（Rust/Axum）和前端（React/TypeScript/Ant Design）均存在对应的占位代码或 TODO 注释，本文档明确定义每项功能的完整需求，供开发者直接按照文档实现。

---

## 词汇表

- **Admin_Backend**：本项目的管理后台服务，包含 Rust 后端和 React 前端。
- **Dashboard**：仪表盘页面（`/dashboard`），展示系统整体运行状态。
- **Gateway**：主项目 IronClaw 的 Web Gateway 服务，提供技能和插件的管理 API。
- **Skill**：技能，由 Gateway 管理的 AI 能力单元，存储于 `skills` 表。
- **Plugin**：插件（扩展），由 Gateway 管理的功能扩展单元，存储于 `plugins` 表。
- **Client**：已注册的桌面客户端，存储于 `registered_clients` 表。
- **AuditLog**：审计日志，存储于 `audit_logs` 表，记录所有管理操作。
- **ClientConfig**：客户端配置，存储于 `client_configs` 表，包含 LLM 参数和功能开关。
- **PolicyVersion**：策略版本号，存储于 `registered_clients.policy_version` 字段，客户端通过心跳检测版本变化后拉取最新策略。
- **Stats_API**：仪表盘统计数据接口，路径为 `/api/dashboard/stats`。
- **Activity_API**：仪表盘活动日志接口，路径为 `/api/dashboard/activity`。
- **Department**：部门，组织架构单元，存储于 `departments` 表，每个用户可归属一个部门。
- **TokenQuota**：Token 限额，每个部门可配置的每日 AI Token 使用上限，通过 `token_quota_enabled` 开关控制是否启用。

---

## 需求

### 需求 1：仪表盘真实数据接入

**用户故事：** 作为管理员，我希望仪表盘展示真实的系统统计数据和近期操作日志，以便我能准确了解系统当前运行状态，而不是看到硬编码的假数据。

#### 验收标准

1. THE Admin_Backend SHALL 提供 `GET /api/dashboard/stats` 接口，返回以下字段：
   - `total_users`：系统中的用户总数（整数，≥ 0）
   - `online_clients`：当前在线客户端数量（整数，≥ 0）
   - `dlp_blocked_today`：今日 DLP 拦截次数（整数，≥ 0）
   - `sensitive_ops_today`：今日敏感操作次数（整数，≥ 0）

2. THE Admin_Backend SHALL 提供 `GET /api/dashboard/activity` 接口，返回最近 20 条审计日志，每条包含：
   - `id`：日志 ID
   - `username`：操作人用户名（可为 null，表示系统操作）
   - `action`：操作类型
   - `details`：操作详情
   - `created_at`：操作时间（ISO 8601 格式）

3. THE Admin_Backend SHALL 提供 `GET /api/dashboard/trends` 接口，返回近 7 天的每日统计数据，包含：
   - `user_activity`：数组，每项包含 `date`（YYYY-MM-DD）和 `count`（当日活跃用户数）
   - `dlp_blocks`：数组，每项包含 `date`（YYYY-MM-DD）和 `count`（当日 DLP 拦截次数）

4. WHEN 前端 Dashboard 页面加载时，THE Dashboard SHALL 调用 `GET /api/dashboard/stats` 获取统计数据，替换原有的 `setTimeout` 硬编码逻辑。

5. WHEN 前端 Dashboard 页面加载时，THE Dashboard SHALL 调用 `GET /api/dashboard/activity` 获取最近操作日志，替换原有的硬编码日志数据。

6. WHEN 前端 Dashboard 页面加载时，THE Dashboard SHALL 调用 `GET /api/dashboard/trends` 获取趋势图数据，替换原有的硬编码趋势数据。

7. IF `GET /api/dashboard/stats` 请求失败，THEN THE Dashboard SHALL 显示错误提示，并将统计数字显示为 `--`，而不是崩溃或显示 0。

8. WHILE 数据加载中，THE Dashboard SHALL 在统计卡片和图表区域显示加载状态（Skeleton 或 Spin）。

#### 正确性属性

- **非负性不变量**：对于 Stats_API 返回的所有数值字段，其值必须 ≥ 0。
- **趋势数据长度**：`GET /api/dashboard/trends` 返回的 `user_activity` 和 `dlp_blocks` 数组长度必须 ≤ 7（近 7 天，若某天无数据则可省略该天）。
- **活动日志排序**：`GET /api/dashboard/activity` 返回的日志必须按 `created_at` 降序排列（最新的在前）。

---

### 需求 2：技能/插件启用禁用管理

**用户故事：** 作为管理员，我希望能够在管理后台直接启用或禁用已安装的技能和插件，以便在不重启系统的情况下控制 AI 能力的可用范围。

#### 验收标准

1. THE Admin_Backend SHALL 提供 `POST /api/skills/{id}/enable` 接口，将指定技能的 `enabled` 字段设置为 `true`，并将操作代理到 Gateway 的对应接口。

2. THE Admin_Backend SHALL 提供 `POST /api/skills/{id}/disable` 接口，将指定技能的 `enabled` 字段设置为 `false`，并将操作代理到 Gateway 的对应接口。

3. THE Admin_Backend SHALL 提供 `POST /api/plugins/{id}/enable` 接口，将指定插件的 `enabled` 字段设置为 `true`，并将操作代理到 Gateway 的对应接口。

4. THE Admin_Backend SHALL 提供 `POST /api/plugins/{id}/disable` 接口，将指定插件的 `enabled` 字段设置为 `false`，并将操作代理到 Gateway 的对应接口。

5. IF Gateway 不可用，THEN THE Admin_Backend SHALL 直接更新本地数据库中的 `enabled` 字段，并返回成功响应，同时在响应中包含 `"gateway_synced": false` 标记。

6. IF 指定的技能或插件 ID 不存在，THEN THE Admin_Backend SHALL 返回 HTTP 404 状态码和描述性错误信息。

7. WHEN 技能或插件的启用/禁用状态发生变更时，THE Admin_Backend SHALL 向 `audit_logs` 表写入一条审计记录，包含操作人、操作类型（`enable_skill`/`disable_skill`/`enable_plugin`/`disable_plugin`）和目标名称。

8. THE SkillList 前端页面 SHALL 在技能列表的每行操作列中显示一个启用/禁用开关（Switch 组件），开关状态与 `enabled` 字段同步。

9. THE PluginList 前端页面 SHALL 在插件列表的每行操作列中显示一个启用/禁用开关（Switch 组件），开关状态与 `enabled` 字段同步。

10. WHEN 用户切换开关时，THE SkillList SHALL 调用对应的启用或禁用接口，并在操作完成后刷新列表数据。

11. WHEN 用户切换开关时，THE PluginList SHALL 调用对应的启用或禁用接口，并在操作完成后刷新列表数据。

12. IF 启用/禁用操作失败，THEN THE SkillList 和 THE PluginList SHALL 显示错误提示，并将开关恢复到操作前的状态。

#### 正确性属性

- **幂等性**：对同一技能连续调用两次 `enable` 接口，第二次调用的结果与第一次相同（`enabled: true`），不产生错误。
- **状态翻转一致性**：调用 `enable` 后再调用 `disable`，最终 `enabled` 字段为 `false`；调用 `disable` 后再调用 `enable`，最终 `enabled` 字段为 `true`。

---

### 需求 3：用户编辑功能

**用户故事：** 作为管理员，我希望能够修改用户的邮箱地址和重置用户密码，以便在用户信息变更或账号安全问题时进行及时处理。

#### 验收标准

1. THE Admin_Backend SHALL 提供 `PUT /api/users/{id}` 接口，支持以下可选字段：
   - `email`：新邮箱地址（字符串，需符合邮箱格式）
   - `password`：新密码（字符串，长度 8-128 字符）
   - `department_id`：所属部门 ID（UUID 或 null，用于更新用户所属部门）

2. IF `PUT /api/users/{id}` 请求中的 `email` 字段不符合邮箱格式，THEN THE Admin_Backend SHALL 返回 HTTP 400 状态码和字段级错误信息。

3. IF `PUT /api/users/{id}` 请求中的 `password` 字段长度不在 8-128 字符范围内，THEN THE Admin_Backend SHALL 返回 HTTP 400 状态码和字段级错误信息。

4. IF 指定的用户 ID 不存在，THEN THE Admin_Backend SHALL 返回 HTTP 404 状态码。

5. WHEN `PUT /api/users/{id}` 成功执行时，THE Admin_Backend SHALL 向 `audit_logs` 表写入一条审计记录，操作类型为 `update_user`，详情中注明修改了哪些字段（邮箱/密码），不记录密码明文。

6. WHEN `PUT /api/users/{id}` 修改密码时，THE Admin_Backend SHALL 使用 Argon2 算法对新密码进行哈希后存储，不存储明文密码。

7. THE UserList 前端页面 SHALL 在用户列表的每行操作列中增加"编辑"按钮（EditOutlined 图标）。

8. WHEN 用户点击"编辑"按钮时，THE UserList SHALL 弹出编辑表单模态框，包含以下字段：
   - 邮箱（Input，预填当前邮箱值）
   - 新密码（Password Input，可选，留空表示不修改密码）
   - 确认密码（Password Input，与新密码一致时才允许提交）
   - 所属部门（Select 组件，选项从 `GET /api/departments` 接口获取，允许选择"无部门"即 null）

9. WHEN 编辑表单提交成功时，THE UserList SHALL 关闭模态框并刷新用户列表。

10. IF 编辑表单提交失败，THEN THE UserList SHALL 在模态框内显示后端返回的错误信息，不关闭模态框。

#### 正确性属性

- **密码哈希不可逆**：`PUT /api/users/{id}` 修改密码后，数据库中存储的 `password_hash` 字段不等于明文密码，且可通过 Argon2 验证新密码。
- **审计日志完整性**：每次成功的 `PUT /api/users/{id}` 调用后，`audit_logs` 表中必须存在一条对应的 `update_user` 记录。

---

### 需求 4：客户端配置下发管理界面

**用户故事：** 作为管理员，我希望通过管理后台界面查看和修改全局客户端配置（包括 LLM 参数和功能开关），以便统一管控所有桌面客户端的行为，而不需要直接操作数据库。

#### 验收标准

1. THE Admin_Backend SHALL 提供 `PUT /api/client-config` 接口，支持更新全局默认配置（`client_id IS NULL` 的记录），可更新字段包括：
   - `llm_backend`：LLM 后端标识（字符串）
   - `llm_api_key`：LLM API Key（字符串）
   - `llm_model`：LLM 模型名称（字符串）
   - `llm_base_url`：LLM Base URL（字符串，需为合法 URL 格式或空字符串）
   - `safety_enabled`：安全功能开关（布尔值）
   - `skills_enabled`：技能功能开关（布尔值）
   - `extensions_enabled`：扩展功能开关（布尔值）
   - `max_cost_per_day_cents`：每日最大费用限制（整数，单位分，≥ 0）

2. WHEN `PUT /api/client-config` 成功执行时，THE Admin_Backend SHALL 将 `config_version` 字段自增 1。

3. WHEN `PUT /api/client-config` 成功执行时，THE Admin_Backend SHALL 向 `audit_logs` 表写入一条审计记录，操作类型为 `update_client_config`，详情中列出变更的字段名，不记录 `llm_api_key` 的值。

4. IF `PUT /api/client-config` 请求中的 `max_cost_per_day_cents` 字段值小于 0，THEN THE Admin_Backend SHALL 返回 HTTP 400 状态码和错误信息。

5. IF `PUT /api/client-config` 请求中的 `llm_base_url` 字段不为空且不是合法 URL，THEN THE Admin_Backend SHALL 返回 HTTP 400 状态码和错误信息。

6. THE Admin_Backend SHALL 在 `GET /api/client-config` 响应中，将 `llm_api_key` 字段脱敏处理：若值非空，则返回前 4 位字符加 `****` 的格式（例如 `sk-a****`），不返回完整值。

7. THE Admin_Backend SHALL 新增前端路由 `/client-config`，对应新建的 `ClientConfig` 页面组件。

8. THE ClientConfig 页面 SHALL 在页面加载时调用 `GET /api/client-config` 获取当前全局配置，并将各字段填充到对应的表单控件中。

9. THE ClientConfig 页面 SHALL 包含以下表单控件：
   - LLM 后端（Select 或 Input）
   - LLM API Key（Password Input，显示脱敏值，修改时可输入新值）
   - LLM 模型名称（Input）
   - LLM Base URL（Input）
   - 安全功能开关（Switch）
   - 技能功能开关（Switch）
   - 扩展功能开关（Switch）
   - 每日最大费用限制（InputNumber，单位：分）

10. THE ClientConfig 页面 SHALL 包含"保存配置"按钮，点击后调用 `PUT /api/client-config` 提交表单数据。

11. WHEN 保存配置成功时，THE ClientConfig 页面 SHALL 显示成功提示，并重新加载配置以显示最新的 `config_version`。

12. THE MainLayout 左侧导航菜单 SHALL 在"系统配置"分组下增加"客户端配置"菜单项，点击后跳转到 `/client-config` 路由。

#### 正确性属性

- **配置往返一致性（Round-Trip）**：调用 `PUT /api/client-config` 保存一组配置后，立即调用 `GET /api/client-config`，返回的非敏感字段值（除 `llm_api_key` 外）必须与保存时提交的值完全一致。
- **版本单调递增**：每次成功调用 `PUT /api/client-config` 后，`config_version` 必须比上一次的值大 1。
- **API Key 脱敏不可逆**：`GET /api/client-config` 返回的 `llm_api_key` 字段不包含完整的原始 API Key 值（若原始值长度 > 4）。

---

### 需求 5：审计日志导出功能

**用户故事：** 作为管理员，我希望能够将审计日志导出为 CSV 文件，以便进行离线分析、合规审查或归档存储。

#### 验收标准

1. THE Admin_Backend SHALL 提供 `GET /api/audit-logs/export` 接口，支持以下查询参数（均为可选）：
   - `start_time`：开始时间（ISO 8601 格式）
   - `end_time`：结束时间（ISO 8601 格式）
   - `action`：操作类型过滤（精确匹配）
   - `username`：操作人用户名过滤（模糊匹配）

2. THE Admin_Backend SHALL 在 `GET /api/audit-logs/export` 响应中设置以下 HTTP 头：
   - `Content-Type: text/csv; charset=utf-8`
   - `Content-Disposition: attachment; filename="audit-logs-{YYYY-MM-DD}.csv"`

3. THE Admin_Backend SHALL 在导出的 CSV 文件中包含 UTF-8 BOM（`\uFEFF`），以确保 Microsoft Excel 正确识别中文字符。

4. THE Admin_Backend SHALL 在导出的 CSV 文件中包含以下列（第一行为表头）：
   - `时间`、`操作人`、`操作类型`、`详情`、`日志ID`

5. IF `start_time` 或 `end_time` 参数格式不合法，THEN THE Admin_Backend SHALL 返回 HTTP 400 状态码和错误信息。

6. IF 筛选条件下没有匹配的日志，THEN THE Admin_Backend SHALL 返回仅包含表头行的 CSV 文件（HTTP 200），不返回 404。

7. THE AuditLog 前端页面 SHALL 将现有的"导出 CSV"按钮改为调用 `GET /api/audit-logs/export` 接口，并将当前页面的筛选条件（时间范围、操作类型、用户名搜索）作为查询参数传递。

8. WHEN 用户点击"导出 CSV"按钮时，THE AuditLog 页面 SHALL 触发浏览器文件下载，文件名格式为 `audit-logs-{YYYY-MM-DD}.csv`。

9. WHEN 导出请求正在进行时，THE AuditLog 页面 SHALL 将"导出 CSV"按钮设置为加载状态（loading），防止重复点击。

10. IF 导出请求失败，THEN THE AuditLog 页面 SHALL 显示错误提示信息。

#### 正确性属性

- **导出与查询结果一致性（Metamorphic Property）**：使用相同的筛选条件分别调用 `GET /api/audit-logs`（分页查询，取总数）和 `GET /api/audit-logs/export`，导出 CSV 的数据行数（不含表头）必须等于查询接口返回的 `total` 字段值。
- **CSV 可解析性**：导出的 CSV 文件必须能被标准 CSV 解析器正确解析，即所有包含逗号或换行符的字段值必须用双引号包裹，双引号字符必须转义为 `""`。

---

### 需求 6：客户端强制下线与策略推送

**用户故事：** 作为管理员，我希望能够主动将指定客户端标记为离线状态，并向单个或全部在线客户端推送最新安全策略，以便在安全事件发生时快速响应，确保所有客户端使用最新的策略配置。

#### 验收标准

1. THE Admin_Backend SHALL 提供 `POST /api/clients/{id}/disconnect` 接口，将指定客户端的 `online` 字段设置为 `false`，`last_activity` 字段更新为当前时间。

2. IF `POST /api/clients/{id}/disconnect` 指定的客户端 ID 不存在，THEN THE Admin_Backend SHALL 返回 HTTP 404 状态码。

3. WHEN `POST /api/clients/{id}/disconnect` 成功执行时，THE Admin_Backend SHALL 向 `audit_logs` 表写入一条审计记录，操作类型为 `disconnect_client`，详情中包含客户端 ID 和客户端名称。

4. THE Admin_Backend SHALL 提供 `POST /api/clients/{id}/push-policy` 接口，将指定客户端的 `policy_version` 字段更新为当前最新策略版本号（从 `policy_versions` 表获取最新版本）。

5. IF `POST /api/clients/{id}/push-policy` 指定的客户端 ID 不存在，THEN THE Admin_Backend SHALL 返回 HTTP 404 状态码。

6. WHEN `POST /api/clients/{id}/push-policy` 成功执行时，THE Admin_Backend SHALL 向 `audit_logs` 表写入一条审计记录，操作类型为 `push_policy`，详情中包含客户端 ID、客户端名称和推送的策略版本号。

7. THE Admin_Backend SHALL 提供 `POST /api/clients/push-policy-all` 接口，将所有 `online = true` 的客户端的 `policy_version` 字段更新为当前最新策略版本号，并返回实际更新的客户端数量。

8. WHEN `POST /api/clients/push-policy-all` 成功执行时，THE Admin_Backend SHALL 向 `audit_logs` 表写入一条审计记录，操作类型为 `push_policy_all`，详情中包含推送的策略版本号和更新的客户端数量。

9. THE ClientList 前端页面 SHALL 在客户端列表的每行操作列中增加以下按钮：
   - "强制下线"按钮（PoweroffOutlined 图标，危险样式），点击后弹出确认对话框
   - "推送策略"按钮（SyncOutlined 图标），点击后直接执行（无需二次确认）

10. THE ClientList 前端页面 SHALL 在页面顶部操作区增加"全部推送"按钮（BroadcastOutlined 图标），点击后弹出确认对话框，确认后调用 `POST /api/clients/push-policy-all`。

11. WHEN 强制下线操作成功时，THE ClientList 页面 SHALL 显示成功提示，并刷新客户端列表和统计数据。

12. WHEN 推送策略操作成功时，THE ClientList 页面 SHALL 显示成功提示（包含推送的版本号），并刷新客户端列表。

13. IF 强制下线或推送策略操作失败，THEN THE ClientList 页面 SHALL 显示后端返回的错误信息。

14. WHILE 强制下线或推送策略操作正在进行时，THE ClientList 页面 SHALL 将对应行的操作按钮设置为加载状态，防止重复点击。

#### 正确性属性

- **强制下线幂等性**：对同一客户端连续调用两次 `POST /api/clients/{id}/disconnect`，第二次调用不返回错误，客户端 `online` 字段保持为 `false`。
- **全量推送一致性**：调用 `POST /api/clients/push-policy-all` 后，所有 `online = true` 的客户端的 `policy_version` 字段必须等于当前最新策略版本号（调用前后查询结果一致）。
- **推送计数准确性**：`POST /api/clients/push-policy-all` 返回的 `updated_count` 字段值必须等于调用前 `online = true` 的客户端数量。

---

### 需求 7：部门管理

**用户故事：**
- 作为管理员，我希望能够创建、查看、编辑和删除部门，以便对组织架构进行管理。
- 作为管理员，我希望能够为每个部门设置每日 Token 使用限额，并通过开关控制是否启用该限额，以便控制各部门的 AI 使用成本。
- 作为管理员，我希望在创建或编辑用户时能够为其选择所属部门，以便将用户归属到正确的组织单元。

#### 验收标准

**后端 API：**

1. THE Admin_Backend SHALL 提供 `GET /api/departments` 接口，返回所有部门列表，每个部门包含：`id`、`name`、`description`、`token_quota_enabled`（布尔值）、`token_quota_per_day`（整数，单位 token，可为 null）、`member_count`（该部门的用户数量）、`created_at`、`updated_at`。

2. THE Admin_Backend SHALL 提供 `POST /api/departments` 接口，创建新部门，必填字段：`name`（2-100 字符）；可选字段：`description`（最长 500 字符）、`token_quota_enabled`（默认 false）、`token_quota_per_day`（整数，≥ 1，仅当 `token_quota_enabled` 为 true 时有效）。

3. THE Admin_Backend SHALL 提供 `PUT /api/departments/{id}` 接口，更新部门信息，支持更新 `name`、`description`、`token_quota_enabled`、`token_quota_per_day` 字段。

4. THE Admin_Backend SHALL 提供 `DELETE /api/departments/{id}` 接口，删除指定部门。

5. IF `DELETE /api/departments/{id}` 时该部门下仍有用户，THEN THE Admin_Backend SHALL 返回 HTTP 400 状态码和错误信息"该部门下还有用户，无法删除"。

6. IF `POST /api/departments` 或 `PUT /api/departments/{id}` 请求中的 `name` 字段与已有部门名称重复，THEN THE Admin_Backend SHALL 返回 HTTP 409 状态码和错误信息。

7. IF `token_quota_enabled` 为 true 且 `token_quota_per_day` 未提供或小于 1，THEN THE Admin_Backend SHALL 返回 HTTP 400 状态码和错误信息。

8. WHEN 部门创建、更新或删除时，THE Admin_Backend SHALL 向 `audit_logs` 表写入审计记录，操作类型分别为 `create_department`、`update_department`、`delete_department`。

**数据库：**

9. THE Admin_Backend SHALL 新增 `departments` 表，包含字段：`id`（UUID，主键）、`name`（VARCHAR(100)，唯一）、`description`（TEXT，可为 null）、`token_quota_enabled`（BOOLEAN，默认 false）、`token_quota_per_day`（INTEGER，可为 null）、`created_at`（TIMESTAMPTZ）、`updated_at`（TIMESTAMPTZ）。

10. THE Admin_Backend SHALL 在 `users` 表中新增 `department_id` 字段（UUID，外键引用 `departments.id`，可为 null，允许 SET NULL 级联）。

**用户编辑集成：**

11. THE Admin_Backend 的 `PUT /api/users/{id}` 接口 SHALL 支持 `department_id` 可选字段（UUID 或 null），用于更新用户所属部门。

12. THE Admin_Backend 的 `GET /api/users` 接口 SHALL 在每个用户对象中包含 `department`（对象，含 `id` 和 `name`，若未分配则为 null）。

**前端界面：**

13. THE Admin_Backend SHALL 新增前端路由 `/departments`，对应新建的 `DepartmentList` 页面组件。

14. THE DepartmentList 页面 SHALL 以表格形式展示所有部门，列包含：部门名称、描述、Token 限额状态（启用/禁用 Tag）、每日 Token 限额（若启用则显示数值，否则显示"未设置"）、成员数量、创建时间、操作列。

15. THE DepartmentList 页面 SHALL 提供"新建部门"按钮，点击后弹出创建表单模态框，包含：部门名称（必填）、描述（可选）、Token 限额开关（Switch）、每日 Token 限额（InputNumber，仅当开关开启时显示且必填）。

16. THE DepartmentList 页面 SHALL 在每行操作列提供"编辑"和"删除"按钮；点击"编辑"弹出编辑表单模态框（预填当前值）；点击"删除"弹出确认对话框。

17. WHEN Token 限额开关关闭时，THE DepartmentList 页面 SHALL 隐藏每日 Token 限额输入框。

18. THE UserList 前端页面（用户编辑模态框）SHALL 新增"所属部门"下拉选择框（Select 组件），选项从 `GET /api/departments` 接口获取，允许选择"无部门"（null）。

19. THE MainLayout 左侧导航菜单 SHALL 在"用户管理"分组下增加"部门管理"菜单项，点击后跳转到 `/departments` 路由。

#### 正确性属性

- **部门名称唯一性**：任意时刻，`departments` 表中不存在两条 `name` 字段相同的记录。
- **删除保护不变量**：若 `users` 表中存在 `department_id = X` 的记录，则 `DELETE /api/departments/X` 必须返回 HTTP 400，不得删除成功。
- **Token 限额一致性**：若 `token_quota_enabled = false`，则 `token_quota_per_day` 字段在 API 响应中应为 null（不论数据库中存储何值）。
- **成员计数准确性**：`GET /api/departments` 返回的每个部门的 `member_count` 必须等于 `users` 表中 `department_id` 等于该部门 ID 的记录数量。
