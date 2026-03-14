# 需求文档 - 企业级 AI Agent 平台

## 简介

本文档定义了基于安全微内核的政企 AI Agent 平台的功能需求。该平台旨在为政企客户提供一个安全、可控、可审计的 AI Agent 执行环境，支持国密算法、离线工作、插件生态、DLP 脱敏和完整的审计追溯能力。

平台由三大核心模块组成：
- **管理后台**：内网部署的管控与审计中心
- **桌面客户端**：基于 Tauri 的用户交互终端
- **执行环境**：隔离的插件运行与模型推理空间

## 术语表

- **系统 (System)**: 指整个企业级 AI Agent 平台
- **管理后台 (Admin_Backend)**: 内网部署的管控与审计中心
- **桌面客户端 (Desktop_Client)**: 基于 Tauri 的用户交互终端
- **安全内核 (Security_Kernel)**: 桌面客户端中的核心安全模块
- **Skills**: 可执行的插件或工具，包括 WASM 插件和脚本
- **ClawHub**: 社区驱动的开源插件市场
- **DLP (Data_Loss_Prevention)**: 数据泄露防护，用于敏感信息脱敏
- **国密算法 (GM_Crypto)**: 中国商用密码算法，包括 SM2/SM3/SM4
- **MCP (Model_Context_Protocol)**: 用于沙箱与宿主通信的标准协议
- **审计日志 (Audit_Log)**: 记录用户操作和系统行为的日志
- **主密码 (Master_Password)**: 用户设置的本地认证密码
- **敏感操作 (Sensitive_Operation)**: 需要人工审批的高风险操作，如文件删除、外网访问
- **灰度发布 (Canary_Deployment)**: 按用户组分阶段推送插件更新的策略
- **CoT (Chain_of_Thought)**: 思考链，展示 AI 模型的推理过程
- **动态水印 (Dynamic_Watermark)**: 嵌入用户信息的半透明水印，用于审计溯源

## 需求

### 需求 1: 国密算法支持

**用户故事**: 作为政企客户，我希望系统支持国密算法栈，以满足合规要求

#### 验收标准

1. THE System SHALL 支持通过配置项在国密算法栈 (SM2/SM3/SM4) 和默认算法栈 (AES-256-GCM/Ed25519/SHA256) 之间切换
2. WHEN 配置为国密模式时，THE Security_Kernel SHALL 使用 SM2 进行数字签名验证
3. WHEN 配置为国密模式时，THE Security_Kernel SHALL 使用 SM4 进行本地数据加密
4. WHEN 配置为国密模式时，THE Security_Kernel SHALL 使用 SM3 进行审计日志摘要计算
5. THE System SHALL 通过统一的 CryptoProvider 接口抽象加密算法实现，确保算法切换不影响业务逻辑

### 需求 2: Skills 安全审核流水线

**用户故事**: 作为安全管理员，我希望所有插件在上架前经过自动化安全审核，以防止恶意代码注入

#### 验收标准

1. WHEN 开发者提交 Skill 时，THE Admin_Backend SHALL 执行 SCA 供应链扫描以检测依赖漏洞
2. WHEN 开发者提交 Skill 时，THE Admin_Backend SHALL 执行 AST 静态代码审计以检测安全风险
3. WHEN Skill 通过所有安全审核时，THE Admin_Backend SHALL 使用 SM2 私钥对插件进行数字签名
4. WHEN Skill 未通过安全审核时，THE Admin_Backend SHALL 拒绝上架并返回详细的审核报告
5. THE Admin_Backend SHALL 记录所有审核过程和结果到审计日志

### 需求 3: 企业 Skills 商店管理

**用户故事**: 作为企业管理员，我希望能够管理插件版本、配置权限策略并支持灰度发布

#### 验收标准

1. THE Admin_Backend SHALL 支持插件的版本管理，包括创建、更新和删除版本
2. THE Admin_Backend SHALL 支持为不同用户组配置插件访问权限
3. WHEN 管理员启动灰度发布时，THE Admin_Backend SHALL 按照配置的用户组比例分阶段推送插件更新
4. WHEN 插件错误率超过阈值时，THE Admin_Backend SHALL 支持管理员手动触发版本回滚
5. THE Admin_Backend SHALL 记录所有插件管理操作到审计日志

### 需求 4: ClawHub 插件市场集成

**用户故事**: 作为企业管理员，我希望能够从 ClawHub 社区市场选择性地引入插件，并进行二次审核

#### 验收标准

1. THE Admin_Backend SHALL 支持手动刷新 ClawHub 插件清单，仅获取元数据而不自动下载插件内容
2. WHEN 管理员选择特定插件时，THE Admin_Backend SHALL 下载插件二进制或脚本内容到企业内部存储
3. WHEN ClawHub 插件下载完成时，THE Admin_Backend SHALL 将其存入待审核池
4. WHEN ClawHub 插件通过企业审核时，THE Admin_Backend SHALL 使用企业 SM2 私钥进行二次签名
5. THE Desktop_Client SHALL 仅展示经过企业二次签名且状态为已审核的插件

### 需求 5: 身份认证与会话管理

**用户故事**: 作为用户，我希望通过安全的方式登录系统并保持会话安全

#### 验收标准

1. WHEN 用户首次启动应用时，THE Desktop_Client SHALL 要求用户设置主密码
2. WHEN 用户设置主密码时，THE Desktop_Client SHALL 将派生密钥存储在操作系统 Keychain 中
3. WHEN 用户启动应用时，THE Desktop_Client SHALL 要求输入主密码进行解锁
4. WHEN 用户无操作超过 30 分钟时，THE Desktop_Client SHALL 自动锁定并要求重新输入主密码
5. WHEN 用户执行敏感操作时，THE Desktop_Client SHALL 要求二次验证主密码
6. THE Desktop_Client SHALL 使用 JWT 作为访问令牌，有效期为 1 小时
7. THE Desktop_Client SHALL 使用 Refresh Token 进行令牌续期，有效期为 7 天

### 需求 6: DLP 数据脱敏

**用户故事**: 作为合规管理员，我希望系统能够自动检测并脱敏敏感信息，防止数据泄露

#### 验收标准

1. THE Admin_Backend SHALL 支持配置和分发 DLP 脱敏词库
2. WHEN 用户输入包含敏感信息时，THE Security_Kernel SHALL 根据 DLP 词库实时过滤敏感内容
3. WHEN 模型推理请求包含敏感信息时，THE Security_Kernel SHALL 在发送前进行脱敏处理
4. WHEN DLP 词库更新时，THE Desktop_Client SHALL 自动同步最新词库到本地
5. WHILE 系统处于离线状态时，THE Security_Kernel SHALL 使用本地缓存的 DLP 词库进行脱敏

### 需求 7: 敏感操作物理拦截

**用户故事**: 作为用户，我希望在执行高风险操作前能够进行人工确认，避免误操作

#### 验收标准

1. WHEN AI Agent 尝试执行敏感操作时，THE Desktop_Client SHALL 暂停执行并在对话流中展示授权按钮
2. THE Desktop_Client SHALL 在对话卡片中清晰展示操作类型、影响范围和风险等级
3. WHEN 用户通过点击授权按钮确认操作时，THE Desktop_Client SHALL 继续执行并记录审批结果到审计日志
4. WHEN 用户通过点击授权按钮拒绝操作时，THE Desktop_Client SHALL 取消执行并记录拒绝原因到审计日志
5. THE Admin_Backend SHALL 支持配置哪些操作类型需要人工审批

### 需求 8: WASM 插件隔离执行

**用户故事**: 作为系统架构师，我希望可信插件能够在隔离环境中安全执行，防止恶意代码影响宿主系统

#### 验收标准

1. WHEN 用户加载 WASM 插件时，THE Security_Kernel SHALL 验证插件的 SM2 数字签名
2. WHEN 签名验证通过时，THE Security_Kernel SHALL 将插件加载到 Wasmtime 运行时沙箱中
3. WHEN 签名验证失败时，THE Security_Kernel SHALL 拒绝加载并记录安全事件到审计日志
4. WHILE WASM 插件运行时，THE Security_Kernel SHALL 限制其访问的系统资源，包括文件系统、网络和内存
5. WHEN WASM 插件尝试访问受限资源时，THE Security_Kernel SHALL 拦截请求并触发审批流程

### 需求 9: MCP 协议网桥

**用户故事**: 作为系统架构师，我希望支持无法编译为 WASM 的脚本插件，通过标准协议与宿主通信

#### 验收标准

1. THE System SHALL 支持通过 MCP 协议与外挂沙箱中的脚本插件通信
2. WHEN 用户调用脚本插件时，THE Security_Kernel SHALL 通过 MCP Bridge 路由请求到对应的沙箱环境
3. THE MCP_Bridge SHALL 记录所有脚本插件的调用参数、返回值和执行时间到审计日志
4. WHEN 脚本插件尝试执行敏感操作时，THE MCP_Bridge SHALL 拦截请求并触发审批流程
5. THE MCP_Bridge SHALL 支持同步和异步调用模式

### 需求 10: 本地加密存储

**用户故事**: 作为用户，我希望本地存储的配置和历史记录被加密保护，防止数据泄露

#### 验收标准

1. THE Desktop_Client SHALL 使用主密码派生的密钥对 libSQL 数据库进行全库加密
2. WHEN 用户输入主密码解锁时，THE Desktop_Client SHALL 使用派生密钥解密数据库
3. THE Desktop_Client SHALL 使用 SM4 或 AES-256-GCM 加密本地配置文件
4. THE Desktop_Client SHALL 使用 SM4 或 AES-256-GCM 加密临时审计日志
5. WHEN 加密或解密失败时，THE Desktop_Client SHALL 记录错误并拒绝访问数据

### 需求 11: 离线审计与同步

**用户故事**: 作为合规管理员，我希望系统在离线状态下也能记录审计日志，并在连线后自动上报

#### 验收标准

1. WHILE 系统处于离线状态时，THE Desktop_Client SHALL 将审计日志加密存储到本地数据库
2. THE Desktop_Client SHALL 使用 SM3 或 SHA256 对每条审计日志计算摘要，防止本地篡改
3. WHEN 系统恢复网络连接时，THE Desktop_Client SHALL 自动检测并启动日志同步流程
4. WHEN 日志同步时，THE Desktop_Client SHALL 使用断点续传机制，仅上传未同步的日志
5. WHEN 日志同步完成时，THE Admin_Backend SHALL 验证日志摘要并存储到审计仓库
6. WHEN 日志摘要验证失败时，THE Admin_Backend SHALL 记录安全事件并通知管理员

### 需求 12: 安全推理网关

**用户故事**: 作为企业用户，我希望所有发往模型（无论内网私有化还是外部 API）的请求都经过统一的安全网关，以保护数据隐私和审计合规。

#### 验收标准

1. THE System SHALL 支持配置统一的推理网关地址，作为所有模型请求的强制出口。
2. WHEN 用户发起推理请求时，THE Desktop_Client SHALL 将请求路由到网关，由网关决定转发至外部 API 或内网私有化模型。
3. THE System SHALL 确保所有通过网关的请求都已执行 DLP 脱敏处理。
4. THE System SHALL 支持在网关层实现多租户配额管理和统一 API Key 注入。
5. WHILE 系统处于离线状态时，THE Desktop_Client SHALL 通过网关（或本地代理）路由至本地部署的 LLM。

### 需求 13: 动态水印

**用户故事**: 作为合规管理员，我希望在用户界面上嵌入动态水印，用于截图溯源和审计追踪

#### 验收标准

1. THE Desktop_Client SHALL 在对话界面上叠加包含用户名和用户 ID 的半透明水印
2. THE Desktop_Client SHALL 使用 Canvas API 实现水印渲染，透明度为 10-20%
3. THE Desktop_Client SHALL 随机平铺或对角线重复水印，防止被轻易裁剪
4. THE Desktop_Client SHALL 确保水印不影响用户正常阅读和操作

### 需求 14: 思考链展示

**用户故事**: 作为用户，我希望能够看到 AI 模型的推理过程，增强交互透明度

#### 验收标准

1. WHEN AI 模型生成思考链时，THE Desktop_Client SHALL 在对话界面中展示思考过程
2. THE Desktop_Client SHALL 使用可折叠的 UI 组件展示思考链，避免干扰主对话流
3. THE Desktop_Client SHALL 支持用户展开或折叠思考链内容
4. THE Desktop_Client SHALL 使用不同的视觉样式区分思考链和最终回复
5. THE Desktop_Client SHALL 记录用户是否查看了思考链到审计日志

### 需求 15: 离线工作模式

**用户故事**: 作为用户，我希望在没有网络连接的情况下也能使用系统的核心功能

#### 验收标准

1. WHILE 系统处于离线状态时，THE Desktop_Client SHALL 支持使用主密码进行身份认证
2. WHILE 系统处于离线状态时，THE Desktop_Client SHALL 支持加载已下载的插件并验证签名
3. WHILE 系统处于离线状态时，THE Desktop_Client SHALL 支持使用本地缓存的 DLP 词库进行脱敏
4. WHILE 系统处于离线状态时，THE Desktop_Client SHALL 支持使用本地部署的 LLM 进行推理
5. WHILE 系统处于离线状态时，THE Desktop_Client SHALL 支持执行本地工具，如文件操作和计算
6. WHILE 系统处于离线状态时，THE Desktop_Client SHALL 拒绝执行需要网络连接的工具，如 API 调用和搜索
7. WHILE 系统处于离线状态时，THE Desktop_Client SHALL 将审计日志加密存储到本地，待连线后自动同步

### 需求 16: 策略引擎

**用户故事**: 作为合规管理员，我希望能够集中配置和分发安全策略，确保所有客户端遵守统一规则

#### 验收标准

1. THE Admin_Backend SHALL 支持配置 DLP 脱敏词库，包括关键词、正则表达式和脱敏规则
2. THE Admin_Backend SHALL 支持配置敏感操作拦截规则，包括操作类型和风险等级
3. WHEN 策略更新时，THE Admin_Backend SHALL 将最新策略推送到所有在线的桌面客户端
4. WHEN 桌面客户端连线时，THE Desktop_Client SHALL 检查本地策略版本并自动同步最新策略
5. THE Admin_Backend SHALL 记录所有策略配置和分发操作到审计日志

### 需求 17: 行为审计中心

**用户故事**: 作为合规管理员，我希望能够查询和分析所有用户的操作行为，进行违规检测和审计追溯

#### 验收标准

1. THE Admin_Backend SHALL 接收并存储来自所有桌面客户端的审计日志
2. THE Admin_Backend SHALL 验证每条审计日志的 SM3 或 SHA256 摘要，确保日志未被篡改
3. THE Admin_Backend SHALL 支持按用户、时间范围、操作类型等维度查询审计日志
4. THE Admin_Backend SHALL 支持配置违规行为检测规则，自动标记可疑操作
5. WHEN 检测到违规行为时，THE Admin_Backend SHALL 生成告警并通知管理员
6. THE Admin_Backend SHALL 将审计日志存储到 ClickHouse 或 TimescaleDB 时序数据库

### 需求 18: 配置解析器与格式化器

**用户故事**: 作为开发者，我希望系统能够可靠地解析和格式化配置文件，确保配置的正确性

#### 验收标准

1. WHEN 系统加载配置文件时，THE System SHALL 解析配置文件并验证其格式和内容
2. WHEN 配置文件格式错误时，THE System SHALL 返回详细的错误信息，包括错误位置和原因
3. THE System SHALL 支持将配置对象格式化为标准的配置文件格式
4. FOR ALL 有效的配置对象，THE System SHALL 确保解析、格式化、再解析后得到等价的配置对象（往返属性）
5. THE System SHALL 记录所有配置解析错误到日志

### 需求 19: 插件版本管理

**用户故事**: 作为企业管理员，我希望能够管理插件的多个版本，支持版本回滚和升级

#### 验收标准

1. THE Admin_Backend SHALL 支持为每个插件存储多个版本
2. THE Admin_Backend SHALL 为每个插件版本记录版本号、发布时间、变更日志和签名
3. WHEN 管理员发布新版本时，THE Admin_Backend SHALL 验证版本号的唯一性
4. THE Admin_Backend SHALL 支持将插件回滚到历史版本
5. THE Desktop_Client SHALL 支持查看插件的版本历史和变更日志
6. WHEN 插件有新版本可用时，THE Desktop_Client SHALL 提示用户更新

### 需求 20: 灰度发布策略

**用户故事**: 作为企业管理员，我希望能够按用户组分阶段推送插件更新，降低发布风险

#### 验收标准

1. THE Admin_Backend SHALL 支持配置灰度发布策略，包括 Alpha、Beta 和 GA 三个阶段
2. THE Admin_Backend SHALL 支持为每个发布阶段配置目标用户组和推送比例
3. WHEN 管理员启动灰度发布时，THE Admin_Backend SHALL 按照配置的阶段顺序逐步推送更新
4. THE Admin_Backend SHALL 监控每个发布阶段的插件错误率和用户反馈
5. WHEN 错误率超过配置的阈值时，THE Admin_Backend SHALL 自动暂停发布并通知管理员
6. THE Admin_Backend SHALL 支持管理员手动推进到下一阶段或回滚到上一版本

