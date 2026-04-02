# IronClaw 政企级 AI 办公助手管理平台 — 需求规格文档

## 简介

IronClaw 是一个面向政企级场景的 AI 办公助手管理平台（Admin Backend），为 openClaw 式桌面客户端提供统一的安全防护、工作内容记录、告警通知、组织权限、AI 助手管控和合规审计能力。本文档基于现有已实现的功能模块进行扩展，覆盖政企级安全合规的完整需求。

文档中每条需求标注 `[已实现]` 或 `[新增]`，以区分现有功能和待开发功能。

> **文档范围说明**：本文档同时覆盖 Admin Backend（管理后台）和 Desktop Client（桌面客户端）两侧的验收标准。对于涉及客户端消费的功能，在对应需求下设有独立的"**客户端验收标准**"小节，描述客户端侧的实现状态和行为要求。这样 fullstack-audit skill 在分析任意需求时，能在同一文档中找到完整的配置方（Admin Backend）和消费方（Desktop Client）链路。

## 术语表

- **Admin_Platform**：IronClaw 管理后台 Web 应用，供管理员使用
- **Desktop_Client**：IronClaw 桌面端应用，供普通员工使用
- **Admin**：拥有管理平台访问权限的用户
- **Employee**：使用桌面客户端的普通用户
- **DLP_Engine**：数据防泄漏扫描引擎，执行规则匹配和内容检测
- **Audit_Service**：审计日志服务，记录所有管理操作和用户行为
- **Alert_Service**：告警服务，负责安全事件检测、告警规则匹配和通知分发
- **Policy_Service**：策略服务，管理安全策略的版本控制、签名和下发
- **Client_Manager**：客户端管理服务，管理终端注册、心跳和在线状态
- **Watermark_Service**：水印服务，为 AI 对话输出和导出文件添加追踪水印
- **Compliance_Service**：合规服务，生成合规报告和执行数据分类分级
- **Quota_Service**：配额服务，管理 AI 模型使用配额和费用控制
- **Knowledge_Base_Service**：知识库服务，管理企业私有知识供 AI 助手检索增强
- **Conversation_Service**：对话审计服务，记录、检索和审计 AI 对话内容
- **Auth_Service**：认证授权服务，处理登录、令牌管理和权限校验
- **Model_Config_Service**：模型配置服务，管理多提供商 AI 模型的接入和调度
- **Notification_Channel**：通知渠道，包括邮件、企业微信、钉钉、飞书 Webhook 等
- **RBAC**：基于角色的访问控制
- **SSO**：单点登录（LDAP/AD、SAML、OIDC）
- **MFA**：多因素认证（TOTP、短信验证码）
- **Device_Fingerprint**：设备指纹，用于唯一标识和追踪终端设备

## 需求


---

### 需求 1：认证与登录 `[已实现 + 扩展]`

**用户故事：** 作为管理员，我希望通过安全的认证方式登录管理平台，以确保只有授权人员能访问管理功能。

#### 验收标准

1. `[已实现]` WHEN Admin 提交有效的用户名和密码, THE Auth_Service SHALL 签发 JWT 令牌并跳转至仪表盘页面
2. `[已实现]` WHEN Admin 提交无效的凭据, THE Auth_Service SHALL 返回明确的错误提示且不泄露用户是否存在
3. `[已实现]` WHEN Admin 勾选"记住我"选项, THE Admin_Platform SHALL 在本地存储中保存用户名以便下次自动填充
4. `[已实现]` WHILE Admin 已通过认证, THE Admin_Platform SHALL 允许访问受保护的路由
5. `[已实现]` WHILE Admin 未通过认证, THE Admin_Platform SHALL 将所有受保护路由的请求重定向至登录页面
6. `[新增]` WHEN Admin 连续输入错误密码达到 5 次, THE Auth_Service SHALL 锁定该账户 15 分钟并记录告警事件
7. `[新增]` WHERE 组织启用了 MFA, THE Auth_Service SHALL 在密码验证通过后要求输入 TOTP 验证码或短信验证码
8. `[新增]` WHERE 组织启用了 SSO, THE Auth_Service SHALL 支持通过 LDAP/AD、SAML 2.0 或 OIDC 协议完成单点登录
9. `[新增]` WHEN JWT 令牌过期, THE Auth_Service SHALL 拒绝请求并返回 401 状态码，Admin_Platform SHALL 跳转至登录页面
10. `[新增]` THE Auth_Service SHALL 在每次成功和失败的登录尝试后生成审计日志条目

---

### 需求 2：仪表盘 `[已实现 + 扩展]`

**用户故事：** 作为管理员，我希望在仪表盘上看到系统运行的关键指标和趋势，以便快速掌握整体安全态势。

#### 验收标准

1. `[已实现]` THE Admin_Platform SHALL 在仪表盘页面展示总用户数、在线客户端数、今日 DLP 拦截数和今日敏感操作数四项核心统计
2. `[已实现]` THE Admin_Platform SHALL 在仪表盘页面展示近 7 天的用户活动趋势折线图和 DLP 拦截趋势折线图
3. `[已实现]` THE Admin_Platform SHALL 在仪表盘页面展示最近操作日志列表，包含用户、操作类型、详情和时间
4. `[已实现]` WHEN Admin 点击刷新按钮, THE Admin_Platform SHALL 重新加载所有仪表盘数据
5. `[新增]` THE Admin_Platform SHALL 在仪表盘页面展示今日 AI 对话总数和 Token 消耗总量统计
6. `[新增]` THE Admin_Platform SHALL 在仪表盘页面展示未处理告警数量，并以醒目颜色标识高危告警
7. `[新增]` THE Admin_Platform SHALL 在仪表盘页面展示系统健康状态指标，包括 API 响应延迟和数据库连接状态
8. `[已实现]` THE Admin_Platform SHALL 在仪表盘页面提供"费用统计" Tab，展示今日/本月费用消耗、月度预算使用率进度条、部门费用消耗排行和模型调用量排行。原独立配额管理页面已合并至此

---

### 需求 3：用户管理 `[已实现 + 扩展]`

**用户故事：** 作为管理员，我希望管理平台用户的账户、角色和权限，以实现细粒度的访问控制。

#### 验收标准

1. `[已实现]` THE Admin_Platform SHALL 展示用户列表，包含用户名、邮箱、角色、状态和创建时间
2. `[已实现]` WHEN Admin 创建新用户, THE Admin_Platform SHALL 要求填写用户名、邮箱和初始角色
3. `[已实现]` WHEN Admin 修改用户信息, THE Admin_Platform SHALL 允许更新用户名、邮箱、角色和启用/禁用状态
4. `[已实现]` WHEN Admin 删除用户, THE Admin_Platform SHALL 要求二次确认后执行删除
5. `[已实现]` THE Admin_Platform SHALL 展示角色列表，包含角色名称、描述、关联权限数和关联用户数
6. `[已实现]` WHEN Admin 创建或编辑角色, THE Admin_Platform SHALL 允许配置角色名称、描述和关联权限
7. `[已实现]` THE Admin_Platform SHALL 展示权限列表，包含权限名称、描述、资源和操作类型
8. `[新增]` WHEN Admin 批量导入用户, THE Admin_Platform SHALL 支持通过 CSV 文件批量创建用户账户
9. `[新增]` WHERE 组织启用了 LDAP/AD 同步, THE Admin_Platform SHALL 支持从目录服务自动同步用户和组织架构
10. `[新增]` THE Admin_Platform SHALL 支持为用户分配所属部门，一个用户仅属于一个主部门

---

### 需求 4：部门与组织架构管理 `[已实现 + 扩展]`

**用户故事：** 作为管理员，我希望管理组织的部门架构和费用配额，以实现按部门的资源和成本管控。

#### 验收标准

1. `[已实现]` THE Admin_Platform SHALL 展示部门列表，包含部门名称、描述、成员数、费用限额状态和创建时间。部门详情页统计卡片展示成员数、每日消耗（分）、每日配额（分）和模型白名单数量
2. `[已实现]` WHEN Admin 创建部门, THE Admin_Platform SHALL 要求填写部门名称，并允许配置描述和每日费用限额（单位：分）
3. `[已实现]` WHEN Admin 编辑部门, THE Admin_Platform SHALL 允许修改名称、描述和费用限额配置
4. `[已实现]` WHEN Admin 删除部门, THE Admin_Platform SHALL 检查部门下是否有成员或子部门，有成员或子部门时禁止删除并提示原因；删除时自动清理该部门的费用配额配置和模型白名单，但保留历史费用消耗记录（department_id 置空）
5. `[已实现]` WHERE 部门启用了费用限额, THE Quota_Service SHALL 根据每次 AI 请求的实际费用（基于模型单价×Token 消耗量）累计，达到每日限额时拒绝后续请求。所有限额统一使用费用（分）作为单位，配置存储在 quota_configs 表中
6. `[新增]` THE Admin_Platform SHALL 支持树形组织架构展示，允许部门嵌套形成多级层级。子部门独立配置费用限额和模型白名单，不继承父部门的配置
7. `[新增]` WHEN 部门的费用消耗达到限额的 80%, THE Alert_Service SHALL 向部门管理员发送预警通知
8. `[新增]` THE Admin_Platform SHALL 展示各部门的费用消耗排行和趋势图
9. `[新增]` THE Admin_Platform SHALL 支持为部门配置可用模型白名单，限定该部门成员可使用的 AI 模型范围
10. `[新增]` WHEN Desktop_Client 发起 AI 对话, THE Client_Manager SHALL 根据用户所属部门的模型白名单返回可用模型列表，未配置白名单的部门默认可使用所有已启用模型
11. `[新增]` THE Admin_Platform SHALL 支持按部门名称搜索和按费用限额状态（已启用/未启用）筛选部门列表

> **客户端侧实现说明**：部门模型白名单的消费方是 Desktop_Client。客户端通过 `GET /api/client-models?user_id={id}` 拉取经白名单过滤后的可用模型列表。

#### 客户端验收标准

12. `[已实现]` WHEN Desktop_Client 启动或用户切换模型, THE Desktop_Client SHALL 调用 `get_available_models` 命令，优先从 `GET /api/client-models?user_id={owner_id}` 拉取后台下发的可用模型列表；owner_id 非合法 UUID 时不传 user_id 参数，后端返回所有已启用模型
13. `[已实现]` WHEN Admin Backend 不可用, THE Desktop_Client SHALL 降级查询本地 LLM provider 的 `list_models()`，并将当前活跃模型始终包含在列表中
14. `[已实现]` THE Desktop_Client SHALL 支持本地自定义模型（`create_custom_model` / `update_custom_model` / `delete_custom_model`），自定义模型与后台下发模型合并展示，来源标记为 `custom`
15. `[已实现]` THE Desktop_Client SHALL 对本地自定义模型的 API Key 不回显（展示为 `****`），防止 Key 泄露

---

### 需求 5：DLP 数据防泄漏 `[已实现 + 扩展]`

**用户故事：** 作为安全管理员，我希望配置和管理 DLP 规则，以防止敏感数据通过 AI 对话泄露。

> **架构说明**：DLP 扫描在客户端执行（输入扫描在发送前，输出扫描在 LLM 返回后），后台下发规则配置。客户端通过 `SafetyBridge`（链式处理：SafetyLayer 密钥检测 → DLP PII 格式保留脱敏）执行扫描，支持从 Admin Backend 热更新规则（`POST /api/dlp-rules`）。

#### 验收标准

1. `[已实现]` THE Admin_Platform SHALL 展示 DLP 规则列表，包含规则名称、模式、严重级别、类别、类型和启用状态
2. `[已实现]` WHEN Admin 创建 DLP 规则, THE Admin_Platform SHALL 支持正则表达式、关键词和字典三种规则类型
3. `[已实现]` WHEN Admin 编辑 DLP 规则, THE Admin_Platform SHALL 允许修改名称、模式、替换文本、严重级别、类别和启用状态
4. `[已实现]` THE Admin_Platform SHALL 提供 DLP 规则测试功能，允许 Admin 输入测试文本验证规则匹配效果
5. `[已实现]` WHEN Admin 批量操作 DLP 规则, THE Admin_Platform SHALL 支持批量启用、批量禁用和批量删除
6. `[已实现]` THE Admin_Platform SHALL 支持 DLP 规则的导入和导出功能
7. `[已实现]` THE Admin_Platform SHALL 展示敏感词典列表，支持创建、编辑和删除词典及其关键词
8. `[已实现]` IF DLP_Engine 扫描超时, THEN THE DLP_Engine SHALL 根据系统配置的故障模式（开放或安全）决定是否放行消息
9. `[已实现]` THE Desktop_Client SHALL 在用户输入发送前执行 DLP 扫描（`SafetyBridge::scan_user_input`），检测到密钥时故障安全拒绝，检测到 PII 时格式保留脱敏（如 `330***618`）
10. `[已实现]` THE Desktop_Client SHALL 在 LLM 返回后执行防提示词攻击扫描（`SafetyBridge::scan_tool_output`），包含截断、注入检测和密钥清理
11. `[已实现]` WHEN Admin 发布新 DLP 规则, THE Desktop_Client SHALL 通过 `sync_dlp_rules_from_admin` 命令从 `/api/dlp-rules` 拉取规则并热更新 DLP 检测器，无需重启客户端
12. `[新增]` WHEN DLP_Engine 检测到严重级别为 critical 的违规, THE Alert_Service SHALL 立即生成高优先级告警
13. `[新增]` THE Admin_Platform SHALL 展示 DLP 拦截事件的详细记录，包含触发规则、原始内容摘要、用户和时间
14. `[未来需求]` THE DLP_Engine SHOULD 考虑使用成熟的 DLP 库以支持文件扫描和更高性能的大规模文本扫描

> **客户端侧实现说明**：DLP 扫描链路：`SafetyBridge::scan_user_input`（密钥检测 → PII 脱敏）→ 发送消息 → LLM 返回 → `SafetyBridge::scan_tool_output`（截断/注入检测/密钥清理）。引擎启动时自动执行一次 DLP 规则同步（`do_sync_dlp_rules`），同步失败时使用内置规则，不阻塞启动。

#### 客户端验收标准

15. `[已实现]` THE Desktop_Client 引擎启动后 2 秒内 SHALL 自动调用 `do_sync_dlp_rules` 从 Admin Backend 同步 DLP 规则；同步失败时使用内置规则，不阻塞引擎启动
16. `[已实现]` THE Desktop_Client SHALL 支持三种规则类型的转换：`regex`（直接使用 pattern）、`keyword`（从 rule_config.keywords 构建正则）、`dictionary`（从 pattern 逗号分隔关键字构建正则）；无效正则跳过并记录警告
17. `[已实现]` WHEN DLP 规则 severity 为 `critical`, THE Desktop_Client SHALL 将对应 LeakPattern 的 action 设为 `Block`（故障安全拒绝）；其他级别设为 `Redact`（格式保留脱敏）
18. `[已实现]` THE Desktop_Client SHALL 通过 `DataReporter` 将 DLP 事件（`ClientReport::DlpEvent`）上报到 Admin Backend，仅上报统计信息（规则名称、是否拦截），不上报原始内容
19. `[已实现]` WHEN DLP 扫描检测到密钥泄露, THE Desktop_Client SHALL 采用 Fail-Safe 策略拒绝发送，不降级为允许发送

---

### 需求 6：敏感操作管理 `[已实现]`

**用户故事：** 作为安全管理员，我希望定义和管理敏感操作类型，以对高风险操作实施审批控制。

#### 验收标准

1. `[已实现]` THE Admin_Platform SHALL 展示敏感操作列表，包含名称、操作类型、风险等级、是否需要审批和启用状态
2. `[已实现]` WHEN Admin 创建敏感操作, THE Admin_Platform SHALL 要求指定操作类型（文件操作、系统命令、网络访问、数据导出、配置变更）和风险等级
3. `[已实现]` WHERE 敏感操作配置为需要审批, THE Admin_Platform SHALL 允许指定审批角色列表
4. `[已实现]` WHEN Admin 编辑敏感操作, THE Admin_Platform SHALL 允许修改名称、类型、风险等级、审批要求和启用状态

---

### 需求 7：策略版本管理 `[已实现 + 扩展]`

**用户故事：** 作为安全管理员，我希望管理安全策略的版本和变更记录，以实现策略的可追溯性和可回滚性。

#### 验收标准

1. `[已实现]` THE Admin_Platform SHALL 展示策略版本列表，包含版本号、变更日志、签名状态和创建时间
2. `[已实现]` THE Admin_Platform SHALL 展示策略变更记录，包含变更的规则、变更类型、变更前后值、操作人和时间
3. `[已实现]` THE Admin_Platform SHALL 展示策略变更统计，包含按变更类型和规则类型的分布以及近 7 天趋势
4. `[新增]` WHEN Admin 发布新策略版本, THE Policy_Service SHALL 对策略内容进行数字签名以防篡改
5. `[新增]` WHEN Admin 选择回滚策略, THE Policy_Service SHALL 恢复到指定历史版本并生成新的版本记录
6. `[新增]` WHEN 新策略版本发布, THE Policy_Service SHALL 根据系统配置决定是否自动推送至所有在线客户端


---

### 需求 8：客户端管理 `[已实现 + 扩展]`

**用户故事：** 作为管理员，我希望监控和管理所有已注册的桌面客户端，以确保终端设备的安全合规。

#### 验收标准

1. `[已实现]` THE Admin_Platform SHALL 展示客户端列表，包含客户端名称、用户、在线状态、版本、操作系统、IP 地址、策略版本和最后活跃时间
2. `[已实现]` THE Admin_Platform SHALL 展示客户端统计卡片，包含总数、在线数、离线数和版本分布
3. `[已实现]` THE Admin_Platform SHALL 支持按用户名、客户端名、IP 搜索以及按在线状态和操作系统筛选客户端
4. `[已实现]` WHEN Admin 点击"推送策略", THE Client_Manager SHALL 向指定客户端推送最新策略版本
5. `[已实现]` WHEN Admin 点击"批量推送策略", THE Client_Manager SHALL 向所有在线客户端推送最新策略版本
6. `[已实现]` WHEN Admin 点击"强制下线", THE Client_Manager SHALL 断开指定客户端的连接
7. `[已实现]` WHEN Admin 点击"删除", THE Client_Manager SHALL 在二次确认后删除客户端注册记录
8. `[新增]` THE Client_Manager SHALL 基于心跳间隔和离线判定阈值自动更新客户端在线状态
9. `[新增]` WHEN Desktop_Client 版本低于管理员设定的最低版本, THE Client_Manager SHALL 标记该客户端为"需要升级"并通知用户
10. `[新增]` THE Admin_Platform SHALL 展示客户端的设备指纹信息，包含硬件标识、操作系统版本和安全补丁级别

> **客户端侧实现说明**：Desktop_Client 通过 `AdminConfigSync` 模块（`GET /api/client-config`）拉取配置，每 5 分钟定时刷新，支持离线缓存（`admin_config.json`）。客户端 token 由 `AuthTokenManager` 管理，64 位十六进制格式，通过 `get_auth_token` 命令暴露给前端。

#### 客户端验收标准

11. `[已实现]` THE Desktop_Client SHALL 通过 `AdminConfigSync` 在启动时从 `GET /api/client-config` 拉取配置，并注入为环境变量（LLM_BACKEND、LLM_API_KEY、LLM_MODEL、LLM_BASE_URL、SAFETY_ENABLED 等）；API Key 注入时不写入日志
12. `[已实现]` WHEN Admin Backend 不可用, THE Desktop_Client SHALL 从本地缓存文件（`admin_config.json`）加载上次成功拉取的配置，支持离线启动
13. `[已实现]` THE Desktop_Client SHALL 每 5 分钟定时刷新配置；刷新失败时静默重试，不影响客户端运行
14. `[已实现]` THE Desktop_Client SHALL 通过 `get_auth_token` 命令向前端提供 64 位十六进制客户端 token，用于与 Admin Backend 的认证通信

---

### 需求 9：客户端配置下发 `[已实现 + 扩展]`

**用户故事：** 作为管理员，我希望集中配置桌面客户端的运行参数，以统一管控所有终端的行为。

#### 验收标准

1. `[已实现]` THE Admin_Platform SHALL 展示客户端配置页面，包含 LLM 后端配置、功能开关和费用控制
2. `[已实现]` WHEN Admin 修改 LLM 后端配置, THE Admin_Platform SHALL 允许设置后端类型、API Key、模型名称和 Base URL
3. `[已实现]` THE Admin_Platform SHALL 提供安全防护、技能系统和扩展系统的功能开关
4. `[已实现]` THE Admin_Platform SHALL 允许设置每日费用限制（单位：分）
5. `[已实现]` WHEN Admin 保存配置, THE Admin_Platform SHALL 递增配置版本号并展示当前版本
6. `[新增]` THE Admin_Platform SHALL 支持按部门或用户组下发差异化的客户端配置
7. `[新增]` WHEN 客户端配置更新, THE Client_Manager SHALL 通过心跳机制通知在线客户端拉取最新配置

> **客户端侧实现说明**：`AdminClientConfig` 包含 LLM 配置、安全策略开关、功能开关、费用限制和水印配置，通过 `inject_to_env()` 注入环境变量后由 IronClaw 引擎读取。

#### 客户端验收标准

8. `[已实现]` THE Desktop_Client SHALL 从 `AdminClientConfig` 中读取水印配置（`watermark_enabled`、`watermark_template`、`watermark_font_size`、`watermark_opacity`、`watermark_position`、`watermark_color`），并通过 `get_watermark_config` 命令暴露给前端
9. `[已实现]` THE Desktop_Client SHALL 从 `AdminClientConfig` 中读取功能开关（`safety_enabled`、`skills_enabled`、`extensions_enabled`）并注入为环境变量，IronClaw 引擎据此决定是否启用对应功能
10. `[待实现]` WHEN Admin Backend 推送配置更新通知, THE Desktop_Client SHALL 主动触发一次 `AdminConfigSync::fetch_once()` 拉取最新配置，而非等待下一个 5 分钟定时周期

---

### 需求 10：模型配置管理 `[已实现 + 扩展]`

**用户故事：** 作为管理员，我希望管理多个 AI 模型提供商的接入配置，以灵活调度不同场景的模型调用。

#### 验收标准

1. `[已实现]` THE Admin_Platform SHALL 展示模型配置列表，包含模型名称、模型 ID、提供商、API Endpoint、能力标签、启用状态和默认标记
2. `[已实现]` WHEN Admin 添加模型配置, THE Admin_Platform SHALL 支持选择提供商（DeepSeek、Moonshot、Qwen、Zhipu、MiniMax、Xiaomi、Volcengine、Ollama、OpenAI、Anthropic、Custom）
3. `[已实现]` WHEN Admin 选择提供商和 API 格式, THE Admin_Platform SHALL 自动填充对应的 API Base URL
4. `[已实现]` THE Admin_Platform SHALL 支持 OpenAI 兼容和 Anthropic 兼容两种 API 格式
5. `[已实现]` WHEN Admin 点击"测试连接", THE Model_Config_Service SHALL 使用提供的凭据验证 API 连通性
6. `[已实现]` THE Admin_Platform SHALL 允许设置默认模型，且同一时间仅有一个模型为默认
7. `[已实现]` THE Admin_Platform SHALL 对 API Key 进行脱敏展示，仅显示前 4 位和后 4 位
8. `[新增]` THE Admin_Platform SHALL 展示各模型的调用量统计和平均响应延迟
9. `[新增]` WHEN 模型 API 连续调用失败达到阈值, THE Alert_Service SHALL 生成模型服务异常告警
10. `[新增]` THE Admin_Platform SHALL 支持为不同部门或角色配置可用模型白名单
11. `[新增]` WHEN Desktop_Client 请求可用模型列表, THE Model_Config_Service SHALL 根据用户所属部门的模型白名单过滤并返回该用户可用的模型配置（脱敏后），未配置白名单的部门返回所有已启用模型
12. `[新增]` THE Admin_Platform SHALL 允许为每个模型配置输入单价和输出单价（单位：分/千Token），用于费用计算
13. `[新增]` WHEN Admin 点击"获取官方定价", THE Model_Config_Service SHALL 尝试从对应提供商的公开定价接口拉取当前模型的单价并自动填充，拉取失败时提示手动输入

> **客户端侧实现说明**：模型配置的消费方是 Desktop_Client。客户端通过 `get_available_models` 命令拉取模型列表，支持后台下发、本地自定义和内置兜底三个来源。`test_model_connection` 命令用于验证自定义模型的 API 连通性。

#### 客户端验收标准

14. `[已实现]` THE Desktop_Client SHALL 通过 `test_model_connection` 命令向指定 API Base URL 发送最小化 chat completion 请求（max_tokens=5），验证自定义模型的 API 连通性，返回成功/失败状态和 HTTP 状态码
15. `[已实现]` THE Desktop_Client SHALL 支持运行时模型切换（`ModelSwitchProvider`），同 provider 内切换通过 `model_override` 注入，跨 provider 切换通过 `replace_inner()` 替换底层 provider；切换时清除 model_override

---

### 需求 11：审计日志 `[已实现 + 扩展]`

**用户故事：** 作为安全管理员，我希望查看和导出所有管理操作的审计日志，以满足合规审计要求。

#### 验收标准

1. `[已实现]` THE Admin_Platform SHALL 展示审计日志列表，包含时间、操作人、操作类型、详情
2. `[已实现]` THE Admin_Platform SHALL 支持按操作人、操作类型和日期范围筛选审计日志
3. `[已实现]` WHEN Admin 点击日志条目的"详情"按钮, THE Admin_Platform SHALL 在弹窗中展示完整的日志信息
4. `[已实现]` WHEN Admin 点击"导出 CSV", THE Admin_Platform SHALL 按当前筛选条件导出审计日志文件
5. `[已实现]` THE Admin_Platform SHALL 支持分页浏览审计日志，每页可选 20、50 或 100 条
6. `[已实现]` THE Audit_Service SHALL 为 DLP 规则、字典、用户、角色、权限的创建、更新、删除操作自动生成审计日志
7. `[新增]` THE Audit_Service SHALL 记录每条审计日志的来源 IP 地址和 User-Agent
8. `[新增]` THE Admin_Platform SHALL 支持审计日志的全文搜索功能
9. `[新增]` THE Audit_Service SHALL 按系统配置的保留天数自动清理过期审计日志
10. `[新增]` THE Audit_Service SHALL 确保审计日志一旦写入不可修改或删除（仅支持自动过期清理）

---

### 需求 12：统计报表 `[已实现 + 扩展]`

**用户故事：** 作为管理员，我希望查看系统各维度的统计报表，以了解安全态势和资源使用情况。

#### 验收标准

1. `[已实现]` THE Admin_Platform SHALL 展示概览统计，包含 DLP 规则数、敏感操作数、字典数、客户端数、策略变更数和近 7 天变更数
2. `[已实现]` THE Admin_Platform SHALL 展示 DLP 规则按严重级别的分布图
3. `[已实现]` THE Admin_Platform SHALL 展示敏感操作按风险等级的分布图
4. `[已实现]` THE Admin_Platform SHALL 展示策略变更按类型（创建、修改、删除、启用、禁用、导入）的分布统计
5. `[新增]` THE Admin_Platform SHALL 展示 AI 对话量的日/周/月趋势报表
6. `[新增]` THE Admin_Platform SHALL 展示各模型的 Token 消耗量和费用统计报表
7. `[新增]` THE Admin_Platform SHALL 展示按部门维度的 AI 使用量排行报表
8. `[新增]` THE Admin_Platform SHALL 支持将报表导出为 PDF 或 Excel 格式

---

### 需求 13：系统设置 `[已实现 + 扩展]`

**用户故事：** 作为管理员，我希望配置系统级参数，以调整平台的安全策略和运行行为。

#### 验收标准

1. `[已实现]` THE Admin_Platform SHALL 提供 DLP 配置项：启用开关、扫描超时时间（100-30000ms）和故障开放模式开关
2. `[已实现]` THE Admin_Platform SHALL 提供审计配置项：启用开关和日志保留天数（7-365 天）
3. `[已实现]` THE Admin_Platform SHALL 提供客户端配置项：心跳间隔（10-300 秒）和离线判定阈值（30-600 秒）
4. `[已实现]` THE Admin_Platform SHALL 提供策略同步配置项：同步间隔（60-3600 秒）和自动推送开关
5. `[已实现]` WHEN Admin 点击"恢复默认", THE Admin_Platform SHALL 将所有配置项重置为系统默认值
6. `[已实现]` WHEN Admin 点击"保存配置", THE Admin_Platform SHALL 持久化所有配置项并展示成功提示
7. `[新增]` THE Admin_Platform SHALL 提供告警配置项：告警通知渠道选择和各渠道的连接参数
8. `[新增]` THE Admin_Platform SHALL 提供安全配置项：密码复杂度策略、会话超时时间和登录失败锁定阈值
9. `[新增]` THE Admin_Platform SHALL 提供水印配置项：水印启用开关、水印内容模板和水印样式


---

### 需求 14：扩展管理（技能与插件） `[已实现 + 扩展]`

**用户故事：** 作为管理员，我希望管理 AI 助手可用的技能和插件，以控制助手的能力范围。

#### 验收标准

1. `[已实现]` THE Admin_Platform SHALL 展示技能列表，包含名称、描述、版本、作者、启用状态和更新时间
2. `[已实现]` WHEN Admin 切换技能的启用状态, THE Admin_Platform SHALL 调用后端 API 启用或禁用该技能
3. `[已实现]` THE Admin_Platform SHALL 展示插件列表，包含名称、描述、版本、作者、启用状态和更新时间
4. `[已实现]` WHEN Admin 切换插件的启用状态, THE Admin_Platform SHALL 调用后端 API 启用或禁用该插件
5. `[新增]` THE Admin_Platform SHALL 支持按部门或角色配置可用技能和插件的白名单
6. `[新增]` WHEN 技能或插件执行涉及敏感操作, THE DLP_Engine SHALL 对技能和插件的输入输出执行 DLP 扫描
7. `[新增]` THE Admin_Platform SHALL 展示各技能和插件的调用频次统计

---

### 需求 15：告警与通知系统 `[已实现 + 扩展]`

**用户故事：** 作为安全管理员，我希望系统能自动检测安全事件并通过多渠道发送告警通知，以便及时响应安全威胁。

> **架构说明**：告警规则在后台配置，包含触发条件（如 DLP 拦截次数阈值、费用超限百分比等）、严重级别和通知渠道。告警触发时生成告警记录并通过配置的 Notification_Channel 发送通知。费用预警的接收方先使用系统管理员。

#### 验收标准

1. `[已实现]` THE Admin_Platform SHALL 展示告警规则列表，包含规则名称、触发条件、严重级别、通知渠道和启用状态（后端 API 已实现，前端 UI 待开发）
2. `[已实现]` WHEN Admin 创建告警规则, THE Admin_Platform SHALL 允许配置触发条件（DLP 拦截次数阈值、异常登录、配额超限百分比、模型服务异常等）、严重级别和通知渠道。告警触发阈值在新建规则时可配置，不是硬编码
3. `[已实现]` WHEN 安全事件匹配告警规则的触发条件, THE Alert_Service SHALL 生成告警记录并通过配置的 Notification_Channel 发送通知（后端逻辑已实现）
4. `[新增]` THE Alert_Service SHALL 支持邮件、企业微信 Webhook、钉钉 Webhook 和飞书 Webhook 四种通知渠道
5. `[已实现]` THE Admin_Platform SHALL 展示告警事件列表，包含告警时间、规则名称、严重级别、触发详情和处理状态（后端 API 已实现，前端 UI 待开发）
6. `[新增]` WHEN Admin 处理告警事件, THE Admin_Platform SHALL 允许标记为"已确认"、"处理中"或"已关闭"，并记录处理备注
7. `[新增]` THE Alert_Service SHALL 对同一规则在配置的静默期内仅发送一次通知，避免告警风暴
8. `[新增]` THE Admin_Platform SHALL 在仪表盘和导航栏展示未处理告警的数量徽标
9. `[新增]` IF Alert_Service 发送通知失败, THEN THE Alert_Service SHALL 记录发送失败日志并在下一个周期重试，重试 3 次后标记为发送失败
10. `[新增]` WHEN 月度费用达到预算的 90%, THE Alert_Service SHALL 向系统管理员发送费用预警通知

---

### 需求 16：AI 对话审计 `[部分实现 + 扩展]`

**用户故事：** 作为安全管理员，我希望审计所有员工与 AI 助手的对话记录，以确保 AI 使用符合企业安全策略。

> **架构决策**：IronClaw 采用客户端直连 LLM API 模式，对话内容不经过后端。因此对话数据通过客户端批量上报机制（复用已有的 `POST /api/client-reports`，新增 `report_type = "conversation"`）写入后端。客户端本地先存对话，定期（或心跳时）批量上报，网络中断时不丢数据。后端接收后解析 JSON payload 写入 conversations + conversation_messages 表。
>
> **当前状态**：后端已实现 conversations 表结构（migration 017）和基础 handler，客户端已有 `DataReporter` 框架但缺少 `ClientReport::Conversation` 类型和对话上报逻辑。

#### 验收标准

1. `[已实现]` THE Admin_Platform SHALL 展示对话记录列表，包含用户、对话主题、消息数、Token 消耗、开始时间和 DLP 标记状态（后端 API 已实现，前端 UI 待开发）
2. `[已实现]` WHEN Admin 点击对话记录, THE Conversation_Service SHALL 展示完整的对话消息流，包含用户输入和 AI 回复（后端 API 已实现，前端 UI 待开发）
3. `[已实现]` THE Admin_Platform SHALL 支持按用户、时间范围、DLP 标记状态和关键词搜索对话记录（后端 API 已实现，前端 UI 待开发）
4. `[新增]` WHEN DLP_Engine 在对话中检测到敏感内容, THE Conversation_Service SHALL 在对话记录上标记 DLP 告警标签
5. `[新增]` THE Conversation_Service SHALL 记录每条消息的 Token 消耗量和使用的模型信息
6. `[新增]` THE Admin_Platform SHALL 支持导出指定时间范围的对话审计报告
7. `[新增]` THE Conversation_Service SHALL 按系统配置的保留策略自动归档或清理过期对话记录
8. `[待实现]` WHEN Desktop_Client 完成一轮对话, THE Desktop_Client SHALL 将对话摘要和消息记录通过 `POST /api/client-reports`（report_type="conversation"）批量上报至后端，使用 client_conversation_id 做幂等去重。需要在 `DataReporter` 中新增 `ClientReport::Conversation` 类型，并在对话结束时调用 `reporter.enqueue()`
9. `[已实现]` THE Conversation_Service SHALL 解析 client-reports 中 report_type="conversation" 的 payload，写入 conversations 和 conversation_messages 表（后端 handler 已实现）
10. `[已实现]` THE Admin_Platform SHALL 在对话列表 API 中仅返回对话摘要（主题、消息数、Token 消耗），不返回完整消息内容；完整消息仅在详情 API 中返回

> **客户端侧实现说明（已完成，需更新状态）**：`ClientReport::Conversation` 类型已在 `data_reporter.rs` 中实现，`ConversationTracker` 已实现按 thread_id 分组的消息缓冲、超时自动 flush（30 分钟）、DLP 标记传递和 Token 回填（`update_last_assistant_tokens`）。上报时使用 `{user_id}-{thread_id}` 作为 `client_conversation_id` 做幂等去重。

#### 客户端验收标准

11. `[已实现]` THE Desktop_Client SHALL 通过 `ConversationTracker` 按 thread_id 分组缓冲对话消息，记录用户消息（`record_user_message`）和 assistant 回复（`record_assistant_message`），Token 信息通过 `update_last_assistant_tokens` 在 TurnCost 事件到达后回填
12. `[已实现]` WHEN 对话 thread 超过 30 分钟无新消息, THE Desktop_Client SHALL 自动将该 thread 的消息缓冲通过 `DataReporter::enqueue(ClientReport::Conversation)` 加入上报队列
13. `[已实现]` WHEN 用户显式切换 thread 或关闭应用, THE Desktop_Client SHALL 调用 `finish_thread()` 立即 flush 当前 thread 的消息缓冲
14. `[已实现]` THE Desktop_Client SHALL 通过 `DataReporter` 每 30 秒批量上报队列中的事件到 `POST /api/client-reports`；上报失败时事件放回队列头部，下次重试；队列超过 10000 条时丢弃最旧的 10% 事件
15. `[已实现]` THE Desktop_Client SHALL 在对话上报中传递 DLP 标记状态（`dlp_flagged`），当任意消息触发 DLP 时整个对话标记为 `dlp_flagged=true`
16. `[待实现]` THE Desktop_Client SHALL 在 `ConversationTracker` 中集成 `DataReporter` 的定期 flush 调用，确保后台每 5 分钟检查一次空闲 thread 并触发 `flush_idle_threads()`

---

### 需求 17：知识库管理 `[新增]`

**用户故事：** 作为管理员，我希望管理企业私有知识库，以增强 AI 助手基于企业知识的回答能力。

#### 验收标准

1. `[新增]` THE Admin_Platform SHALL 展示知识库列表，包含知识库名称、描述、文档数量、最后更新时间和启用状态
2. `[新增]` WHEN Admin 创建知识库, THE Knowledge_Base_Service SHALL 创建独立的向量索引空间
3. `[新增]` WHEN Admin 上传文档至知识库, THE Knowledge_Base_Service SHALL 支持 PDF、Word、Markdown 和纯文本格式
4. `[新增]` WHEN 文档上传完成, THE Knowledge_Base_Service SHALL 自动执行文档切片、向量化和索引构建
5. `[新增]` THE Admin_Platform SHALL 展示文档处理状态（待处理、处理中、已完成、失败）
6. `[新增]` THE Admin_Platform SHALL 支持为知识库配置访问权限，限定可使用该知识库的部门或角色
7. `[新增]` WHEN Admin 删除知识库中的文档, THE Knowledge_Base_Service SHALL 同步清理对应的向量索引数据
8. `[新增]` THE Admin_Platform SHALL 提供知识库检索测试功能，允许 Admin 输入查询验证检索效果

---

### 需求 18：费用配额管理 `[已实现 + 扩展]`

**用户故事：** 作为管理员，我希望管理 AI 模型的使用费用配额，以控制企业的 AI 使用成本。

> 设计决策：费用配额管理恢复为独立菜单页面（`/quota`），不再作为仪表盘的 Tab。原因：随着费用明细记录、筛选、分页等功能的加入，复杂度已超出仪表盘 Tab 能承载的范围。仪表盘保留轻量的费用摘要卡片（今日/本月费用），点击可跳转至费用管理页面。配额配置（每日限额、月度预算）统一在部门管理页面的根部门配额卡片中设置。所有限额统一使用费用（分）作为单位，不再使用 Token 数量。

#### 验收标准

1. `[已实现]` THE Admin_Platform SHALL 在独立的费用管理页面中展示全局费用概览，包含今日费用、本月费用、本月预算和预算使用率、部门费用消耗排行和模型调用量排行
2. `[已实现]` THE Quota_Service SHALL 支持按部门层级设置每日费用限额（单位：分），配置统一存储在 quota_configs 表中
3. `[已实现]` WHEN Desktop_Client 发起 AI 请求, THE Quota_Service SHALL 先执行预检：查询用户所属部门的当日已消耗费用，若已超过限额则直接拒绝请求并返回超额提示
4. `[已实现]` WHEN AI 请求完成后, THE Quota_Service SHALL 根据模型响应中的 usage（input_tokens、output_tokens）乘以该模型配置的单价，计算本次实际费用并写入 usage_records 表
5. `[已实现]` WHEN 部门的费用消耗达到部门限额, THE Quota_Service SHALL 拒绝该部门所有用户的后续 AI 请求
6. `[新增]` THE Admin_Platform SHALL 在费用管理页面中展示费用消耗的明细记录表格，包含用户、模型、Token 数量、费用和时间，支持按时间范围、用户、模型、部门筛选和分页
7. `[已实现]` THE Admin_Platform SHALL 在费用管理页面中展示按部门和模型维度的费用消耗排行
8. `[新增]` WHEN 月度费用达到预算的 90%, THE Alert_Service SHALL 向管理员发送费用预警通知
9. `[已实现]` THE Quota_Service SHALL 使用时间窗口查询（`WHERE created_at >= today_start`）计算当日消耗，而非定时清零计数器，以避免零点前后并发请求导致的数据不一致
10. `[已实现]` IF Quota_Service 预检接口不可用（数据库故障、网络超时）, THEN THE Quota_Service SHALL 采用 Fail-Safe 策略拒绝请求，而非放行（政企场景安全优先）
11. `[已实现]` WHERE 模型未配置单价（input_price 或 output_price 为 NULL）, THE Quota_Service SHALL 按 0 计费（免费），Admin_Platform SHALL 在模型列表中标记"未定价"警告
12. `[已实现]` WHEN Desktop_Client 请求可用模型列表, THE Model_Config_Service SHALL 根据用户所属部门的模型白名单过滤并返回该用户可用的模型配置（脱敏后），未配置白名单的部门返回所有已启用模型
13. `[已实现]` WHEN Quota_Service 执行预检, THE Quota_Service SHALL 同时检查用户直属部门限额和根部门（公司级）限额，任一超额即拒绝请求。不递归检查中间层级部门。根部门消耗通过全表当日 SUM(usage_records WHERE created_at >= today) 计算（利用已有时间索引），不使用递归 CTE。直属部门即根部门时只检查一次
14. `[已实现]` THE Admin_Platform SHALL 在根部门（parent_id 为 NULL）的配额卡片中默认展示所有子部门限额的累加值，并提供"自定义限额"切换按钮和月度预算设置。开启自定义后管理员可手动设置公司级限额（可低于累加值，用于更严格的总量控制）。自定义限额和月度预算存入 quota_configs 表
15. `[已实现]` THE Admin_Platform SHALL 在部门详情页的统计卡片中展示"每日消耗"和"每日配额"（单位：分），替代原有的 Token 配额展示

---

### 需求 19：数据分类分级与合规 `[新增]`

**用户故事：** 作为合规管理员，我希望对企业数据进行分类分级管理，以满足政企场景的数据安全合规要求。

#### 验收标准

1. `[新增]` THE Admin_Platform SHALL 支持定义数据分类分级体系，包含公开、内部、机密和绝密四个默认级别
2. `[新增]` THE Compliance_Service SHALL 支持为 DLP 规则关联数据分级标签，标识该规则保护的数据级别
3. `[新增]` THE Admin_Platform SHALL 展示合规概览仪表盘，包含各级别数据的 DLP 规则覆盖率和拦截统计
4. `[新增]` THE Compliance_Service SHALL 支持生成合规审计报告，包含指定时间范围内的安全事件汇总、DLP 拦截统计和策略变更记录
5. `[新增]` THE Admin_Platform SHALL 支持导出合规审计报告为 PDF 格式
6. `[新增]` THE Compliance_Service SHALL 支持配置数据保留策略，按数据级别设定不同的保留期限

---

### 需求 20：水印与追踪 `[新增]`

**用户故事：** 作为安全管理员，我希望为 AI 助手的输出内容添加可见水印，以威慑数据泄露行为。

> 设计决策：简化为可见水印配置，不实现隐写术水印。水印配置集成在系统设置中，客户端通过 `/api/client-config` 拉取水印配置，导出文件时渲染可见水印（用户名 + 部门 + 时间）。隐式水印提取和追溯功能暂不实现。

#### 验收标准

1. `[新增]` THE Admin_Platform SHALL 在系统设置中提供水印配置区域，包含启用开关、内容模板（支持 {username}、{department}、{datetime} 变量）、字体大小、透明度和位置
2. `[新增]` WHEN 水印功能已启用, THE Desktop_Client SHALL 在 AI 对话导出的文件上渲染包含用户标识和时间戳的可见水印
3. `[新增]` THE Admin_Platform SHALL 在水印管理页面展示当前水印配置和预览效果

> **客户端侧实现说明**：水印配置通过 `AdminClientConfig` 下发，客户端通过 `get_watermark_config` 命令读取并暴露给前端。前端在导出文件时根据配置渲染可见水印。

#### 客户端验收标准

4. `[已实现]` THE Desktop_Client SHALL 通过 `get_watermark_config` 命令从 `GET /api/settings` 拉取水印配置，返回 `watermark_enabled`、`watermark_template`、`watermark_font_size`、`watermark_opacity`、`watermark_position`、`watermark_color` 字段；Admin Backend 不可用时返回默认值（禁用状态）
5. `[待实现]` WHEN `watermark_enabled=true`, THE Desktop_Client 前端 SHALL 在导出对话内容时将水印模板中的 `{username}`、`{department}`、`{datetime}` 变量替换为当前用户信息，并渲染为可见水印叠加在导出文件上

---

### 需求 21：操作审批流 `[部分实现 + 扩展]`

**用户故事：** 作为管理员，我希望对高风险操作实施审批流程，以确保关键操作经过授权。

> **架构说明**：IronClaw 的审批机制基于消息系统。当工具需要审批时，Agent 发送 `ApprovalNeeded` 事件到前端，用户通过 `!approve <request_id>` 或 `!deny <request_id>` 消息响应，Agent 的 SubmissionParser 解析后执行审批操作。审批流在对话中进行，等待审批时任务保持运行中状态，审批通过后任务自动继续执行。
>
> **当前状态**：客户端已实现审批命令（`ic_approve_tool`, `ic_deny_tool`）和 `ApprovalNeeded` 事件，后端已有 approvals 表结构（migration 018）和基础 handler。需要补充：审批工单持久化、超时催办、审批历史查询等管理端功能。

#### 验收标准

1. `[已实现]` WHEN Desktop_Client 中的工具需要审批, THE Agent SHALL 发送 `ApprovalNeeded` 事件到前端，包含 request_id、tool_name 和 description
2. `[已实现]` WHEN Employee 在对话中批准或拒绝操作, THE Desktop_Client SHALL 通过 `ic_approve_tool` 或 `ic_deny_tool` 命令发送格式化的审批消息（`!approve <request_id>` 或 `!deny <request_id>`）到 Agent
3. `[已实现]` WHEN 审批消息到达 Agent, THE Agent 的 SubmissionParser SHALL 解析审批指令并执行对应操作，审批通过后任务自动继续执行
4. `[已实现]` THE Admin_Platform SHALL 展示审批记录列表，包含申请人、工具名称、申请时间、审批状态和审批人（后端 API 已实现，前端 UI 待开发）
5. `[新增]` WHEN Admin 在管理端审批工单, THE Admin_Platform SHALL 允许选择"批准"或"拒绝"并填写审批意见，审批结果通过 WebSocket/SSE 推送到客户端
6. `[新增]` IF 审批工单在 24 小时内未处理, THEN THE Alert_Service SHALL 向审批人发送催办通知
7. `[新增]` THE Audit_Service SHALL 记录审批流程的完整生命周期，包含申请、审批和执行环节
8. `[新增]` THE Admin_Platform SHALL 支持按申请人、工具类型、审批状态和时间范围筛选审批记录

> **客户端侧实现说明**：`ic_approve_tool` 和 `ic_deny_tool` 通过向 Agent 发送 `!approve <request_id>` / `!deny <request_id>` 格式消息实现审批，消息经 `IncomingMessage` 路由到 Agent 的 SubmissionParser 处理。两个命令均已注册到 `all_tauri_commands!()` 宏。

#### 客户端验收标准

9. `[已实现]` THE Desktop_Client SHALL 通过 `ic_approve_tool(request_id, thread_id)` 命令向 Agent 发送 `!approve {request_id}` 格式的审批消息，消息携带正确的 thread_id 和 owner_id
10. `[已实现]` THE Desktop_Client SHALL 通过 `ic_deny_tool(request_id, thread_id)` 命令向 Agent 发送 `!deny {request_id}` 格式的拒绝消息
11. `[待实现]` WHEN Admin 在管理端通过 SSE/WebSocket 推送审批结果, THE Desktop_Client SHALL 接收推送并通过 `chat-event` 通知前端更新审批状态，无需用户手动刷新

---

### 需求 23：对话流异步审批任务 `[新增]`

**用户故事：** 作为员工，我希望在对话中生成报告或执行高风险操作后，能够一键提交审批，审批过程在后台独立进行，不阻塞我继续使用对话功能；作为管理员，我希望在管理平台审批后，系统自动完成后续操作并通知员工。

> **架构说明**：本需求与需求 21（操作审批流）的区别在于：需求 21 的审批是对话内即时确认（用户就在聊天界面等待），本需求的审批是异步的（管理员可能数小时后才处理）。实现上，点击确认按钮后在 Desktop Client 创建一个独立的后台轮询任务（`tokio::spawn`），不依赖 IronClaw 的 Job 系统，不阻塞对话线程。

#### 验收标准

1. `[新增]` WHEN Agent 在对话中生成需要审批才能发送的内容（如报告、敏感文件）, THE Desktop_Client SHALL 在消息气泡旁显示"提交审批"确认按钮，而非自动发送
2. `[新增]` WHEN Employee 点击"提交审批"按钮, THE Desktop_Client SHALL 立即向 Admin_Platform 创建审批工单（`POST /api/approvals`），并在后台启动独立轮询任务，对话流不阻塞、不进入等待状态
3. `[新增]` WHILE 审批工单处于 pending 状态, THE Desktop_Client 后台任务 SHALL 每 30 秒轮询一次 `GET /api/approvals/{id}/check`，直到状态变更或超时
4. `[新增]` WHEN Admin 在管理平台批准审批工单, THE Desktop_Client 后台任务 SHALL 检测到 `status=approved` 后自动执行发送操作，并通过 `chat-event` 通知前端任务完成
5. `[新增]` WHEN Admin 在管理平台拒绝审批工单, THE Desktop_Client 后台任务 SHALL 检测到 `status=rejected` 后终止任务，并通过 `chat-event` 通知前端审批被拒绝（含拒绝原因）
6. `[新增]` IF 审批工单在 24 小时内未处理（`status=expired`）, THE Desktop_Client 后台任务 SHALL 终止轮询并通过 `chat-event` 通知前端工单已过期
7. `[新增]` THE Desktop_Client SHALL 在对话界面显示审批任务的实时状态（待审批 / 已批准 / 已拒绝 / 已过期），状态更新不刷新整个对话历史
8. `[新增]` WHEN Desktop_Client 重启时, THE Desktop_Client SHALL 恢复所有未完成的审批轮询任务（通过本地持久化 pending ticket_id 列表实现）
9. `[新增]` THE Admin_Platform SHALL 在审批工单列表中显示来源标记（"对话流提交"），以区分手动创建的工单和对话流自动提交的工单

> **客户端侧实现说明**：本需求的后台轮询任务通过 `tokio::spawn` 创建独立异步任务，不依赖 IronClaw Job 系统，不阻塞对话线程。轮询结果通过 Tauri 的 `app_handle.emit("chat-event", ...)` 推送到前端。重启恢复通过本地持久化 pending ticket_id 列表实现。

#### 客户端验收标准

10. `[待实现]` THE Desktop_Client SHALL 实现 `submit_approval_ticket(content, thread_id)` Tauri 命令，调用 `POST /api/approvals` 创建工单，并通过 `tokio::spawn` 启动独立后台轮询任务，返回 ticket_id 给前端
11. `[待实现]` THE Desktop_Client 后台轮询任务 SHALL 每 30 秒调用 `GET /api/approvals/{id}/check`，检测到 `status` 变更时通过 `chat-event` 推送结果并终止轮询；网络错误时继续重试，不终止任务
12. `[待实现]` THE Desktop_Client SHALL 在本地持久化 pending ticket_id 列表（JSON 文件），应用启动时读取列表并为每个 pending ticket 重新启动轮询任务
13. `[待实现]` THE Desktop_Client SHALL 将 `submit_approval_ticket` 命令注册到 `all_tauri_commands!()` 宏，并在 `tauri_command_contract_tests.rs` 的 `FRONTEND_INVOKED_COMMANDS` 中添加对应条目

---

### 需求 22：系统安全加固 `[新增]`

**用户故事：** 作为安全管理员，我希望平台自身具备完善的安全防护能力，以抵御常见的安全威胁。

#### 验收标准

1. `[新增]` THE Auth_Service SHALL 对所有 API 请求验证 JWT 令牌的有效性和权限范围
2. `[新增]` THE Admin_Platform SHALL 对所有用户输入执行参数校验和 XSS 防护
3. `[新增]` THE Auth_Service SHALL 实施基于 IP 的请求频率限制，防止暴力破解和 API 滥用
4. `[新增]` THE Admin_Platform SHALL 在所有 API 响应中设置安全 HTTP 头（Content-Security-Policy、X-Frame-Options、X-Content-Type-Options）
5. `[新增]` THE Auth_Service SHALL 确保密码使用 bcrypt 或 argon2 算法进行哈希存储，禁止明文存储
6. `[新增]` THE Admin_Platform SHALL 确保 API Key、密码等敏感字段在日志、错误响应和网络传输中不以明文形式出现
7. `[新增]` THE Auth_Service SHALL 支持管理员配置会话超时时间，超时后自动注销
8. `[新增]` THE Audit_Service SHALL 记录所有认证失败事件，包含来源 IP、尝试的用户名和失败原因

---

## 附录：客户端直连架构的安全边界 `[低优先级 - 未来改进]`

### 背景

IronClaw 采用客户端直连 LLM API 模式（后台下发配置，客户端直连 LLM，客户端批量上报数据），这种架构在降低后端负载和提升响应速度的同时，也引入了一些安全边界问题。

### 已知安全边界

1. **API Key 暴露风险**
   - 现状：LLM API Key 通过客户端配置下发，存储在客户端本地
   - 风险：恶意用户可能提取 API Key 用于非授权调用
   - 缓解措施：使用客户端加密存储、定期轮换 API Key、监控异常调用模式

2. **配额绕过风险**
   - 现状：配额预检在客户端执行，客户端可能被篡改绕过检查
   - 风险：恶意用户可能绕过配额限制无限调用 LLM
   - 缓解措施：后端通过上报数据事后审计、异常检测告警、账户封禁机制

3. **DLP 规则绕过风险**
   - 现状：DLP 扫描在客户端执行，客户端可能被篡改禁用 DLP
   - 风险：恶意用户可能绕过 DLP 检查发送敏感数据
   - 缓解措施：后端通过对话审计事后检测、DLP 事件上报监控、异常行为告警

4. **对话内容完整性风险**
   - 现状：对话内容由客户端批量上报，客户端可能篡改或不上报
   - 风险：审计日志可能不完整或被篡改
   - 缓解措施：客户端签名上报数据、后端验证签名、异常检测（如长时间未上报）

### 未来改进方向（低优先级）

1. **混合架构**：关键操作（如高价值模型调用、敏感数据处理）通过后端代理，普通操作保持客户端直连
2. **零知识证明**：客户端生成操作证明，后端验证而不需要看到原始数据
3. **可信执行环境（TEE）**：在支持的平台上使用 TEE 保护 API Key 和 DLP 规则
4. **区块链审计**：将关键审计日志写入不可篡改的区块链

### 当前优先级

这些安全边界问题在政企场景中需要关注，但考虑到：
- 客户端部署在企业内网，物理隔离降低了攻击面
- 员工设备通常有 MDM 管理，篡改难度较高
- 事后审计和异常检测可以发现大部分违规行为

因此将这些改进标记为**低优先级**，优先完成核心功能和已知高优先级需求。在产品成熟后，根据客户反馈和实际安全事件再决定是否投入资源改进。

