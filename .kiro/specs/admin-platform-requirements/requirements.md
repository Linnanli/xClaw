# IronClaw 政企级 AI 办公助手管理平台 — 需求规格文档

## 简介

IronClaw 是一个面向政企级场景的 AI 办公助手管理平台（Admin Backend），为 openClaw 式桌面客户端提供统一的安全防护、工作内容记录、告警通知、组织权限、AI 助手管控和合规审计能力。本文档基于现有已实现的功能模块进行扩展，覆盖政企级安全合规的完整需求。

文档中每条需求标注 `[已实现]` 或 `[新增]`，以区分现有功能和待开发功能。

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

---

### 需求 5：DLP 数据防泄漏 `[已实现 + 扩展]`

**用户故事：** 作为安全管理员，我希望配置和管理 DLP 规则，以防止敏感数据通过 AI 对话泄露。

#### 验收标准

1. `[已实现]` THE Admin_Platform SHALL 展示 DLP 规则列表，包含规则名称、模式、严重级别、类别、类型和启用状态
2. `[已实现]` WHEN Admin 创建 DLP 规则, THE Admin_Platform SHALL 支持正则表达式、关键词和字典三种规则类型
3. `[已实现]` WHEN Admin 编辑 DLP 规则, THE Admin_Platform SHALL 允许修改名称、模式、替换文本、严重级别、类别和启用状态
4. `[已实现]` THE Admin_Platform SHALL 提供 DLP 规则测试功能，允许 Admin 输入测试文本验证规则匹配效果
5. `[已实现]` WHEN Admin 批量操作 DLP 规则, THE Admin_Platform SHALL 支持批量启用、批量禁用和批量删除
6. `[已实现]` THE Admin_Platform SHALL 支持 DLP 规则的导入和导出功能
7. `[已实现]` THE Admin_Platform SHALL 展示敏感词典列表，支持创建、编辑和删除词典及其关键词
8. `[已实现]` IF DLP_Engine 扫描超时, THEN THE DLP_Engine SHALL 根据系统配置的故障模式（开放或安全）决定是否放行消息
9. `[新增]` THE DLP_Engine SHALL 支持对 AI 对话的输入和输出双向扫描
10. `[新增]` WHEN DLP_Engine 检测到严重级别为 critical 的违规, THE Alert_Service SHALL 立即生成高优先级告警
11. `[新增]` THE Admin_Platform SHALL 展示 DLP 拦截事件的详细记录，包含触发规则、原始内容摘要、用户和时间

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

### 需求 15：告警与通知系统 `[新增]`

**用户故事：** 作为安全管理员，我希望系统能自动检测安全事件并通过多渠道发送告警通知，以便及时响应安全威胁。

#### 验收标准

1. `[新增]` THE Admin_Platform SHALL 展示告警规则列表，包含规则名称、触发条件、严重级别、通知渠道和启用状态
2. `[新增]` WHEN Admin 创建告警规则, THE Admin_Platform SHALL 允许配置触发条件（DLP 拦截次数阈值、异常登录、配额超限、模型服务异常等）、严重级别和通知渠道
3. `[新增]` WHEN 安全事件匹配告警规则的触发条件, THE Alert_Service SHALL 生成告警记录并通过配置的 Notification_Channel 发送通知
4. `[新增]` THE Alert_Service SHALL 支持邮件、企业微信 Webhook、钉钉 Webhook 和飞书 Webhook 四种通知渠道
5. `[新增]` THE Admin_Platform SHALL 展示告警事件列表，包含告警时间、规则名称、严重级别、触发详情和处理状态
6. `[新增]` WHEN Admin 处理告警事件, THE Admin_Platform SHALL 允许标记为"已确认"、"处理中"或"已关闭"，并记录处理备注
7. `[新增]` THE Alert_Service SHALL 对同一规则在配置的静默期内仅发送一次通知，避免告警风暴
8. `[新增]` THE Admin_Platform SHALL 在仪表盘和导航栏展示未处理告警的数量徽标
9. `[新增]` IF Alert_Service 发送通知失败, THEN THE Alert_Service SHALL 记录发送失败日志并在下一个周期重试，重试 3 次后标记为发送失败

---

### 需求 16：AI 对话审计 `[新增]`

**用户故事：** 作为安全管理员，我希望审计所有员工与 AI 助手的对话记录，以确保 AI 使用符合企业安全策略。

#### 验收标准

1. `[新增]` THE Admin_Platform SHALL 展示对话记录列表，包含用户、对话主题、消息数、Token 消耗、开始时间和 DLP 标记状态
2. `[新增]` WHEN Admin 点击对话记录, THE Conversation_Service SHALL 展示完整的对话消息流，包含用户输入和 AI 回复
3. `[新增]` THE Admin_Platform SHALL 支持按用户、时间范围、DLP 标记状态和关键词搜索对话记录
4. `[新增]` WHEN DLP_Engine 在对话中检测到敏感内容, THE Conversation_Service SHALL 在对话记录上标记 DLP 告警标签
5. `[新增]` THE Conversation_Service SHALL 记录每条消息的 Token 消耗量和使用的模型信息
6. `[新增]` THE Admin_Platform SHALL 支持导出指定时间范围的对话审计报告
7. `[新增]` THE Conversation_Service SHALL 按系统配置的保留策略自动归档或清理过期对话记录

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

**用户故事：** 作为安全管理员，我希望为 AI 助手的输出内容添加追踪水印，以便在数据泄露时追溯来源。

#### 验收标准

1. `[新增]` WHERE 水印功能已启用, THE Watermark_Service SHALL 为 AI 对话导出的文件添加包含用户标识和时间戳的隐式水印
2. `[新增]` THE Admin_Platform SHALL 提供水印配置页面，允许设置水印内容模板（支持用户名、部门、时间等变量）
3. `[新增]` THE Watermark_Service SHALL 支持文本水印和图片水印两种形式
4. `[新增]` WHEN Admin 提供疑似泄露的文件, THE Watermark_Service SHALL 提取水印信息以追溯文件来源用户和导出时间
5. `[新增]` THE Admin_Platform SHALL 展示水印提取记录，包含提取时间、文件信息和追溯结果

---

### 需求 21：操作审批流 `[新增]`

**用户故事：** 作为管理员，我希望对高风险操作实施审批流程，以确保关键操作经过授权。

#### 验收标准

1. `[新增]` WHEN Employee 触发配置为需要审批的敏感操作, THE Admin_Platform SHALL 创建审批工单并通知指定审批角色
2. `[新增]` THE Admin_Platform SHALL 展示待审批工单列表，包含申请人、操作类型、申请时间和当前状态
3. `[新增]` WHEN Admin 审批工单, THE Admin_Platform SHALL 允许选择"批准"或"拒绝"并填写审批意见
4. `[新增]` WHEN 工单被批准, THE Admin_Platform SHALL 允许申请人在有效期内执行该操作
5. `[新增]` WHEN 工单被拒绝, THE Admin_Platform SHALL 通知申请人并记录拒绝原因
6. `[新增]` IF 审批工单在 24 小时内未处理, THEN THE Alert_Service SHALL 向审批人发送催办通知
7. `[新增]` THE Audit_Service SHALL 记录审批流程的完整生命周期，包含申请、审批和执行环节

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

