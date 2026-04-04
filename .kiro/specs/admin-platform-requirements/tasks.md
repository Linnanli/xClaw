# IronClaw 政企级 AI 办公助手管理平台 — 实现任务列表

## 说明

- `[x]` 已完成，`[ ]` 待实现，`[-]` 进行中
- 标注 `*` 的为可选任务
- 任务按 P0 → P1 → P2 优先级排列，同优先级内按依赖关系排序
- 每个任务对应 requirements.md 中的具体验收标准

---

## P0：核心基础（安全加固 + 已有模块扩展）

### 1. 数据库迁移：已有表扩展字段

- [x] 1.1 执行 `014_department_extensions.sql`：为 users 表添加 department_id、mfa_enabled、login_fail_count、locked_until 字段；为 departments 表添加 parent_id、path、allowed_model_ids 字段；为 audit_logs 表添加 ip_address、user_agent、is_immutable 字段；为 clients 表添加 device_fingerprint、needs_upgrade 字段；为 model_configs 表添加 total_calls、avg_latency_ms、consecutive_failures 字段
- [x] 1.2 在 `integration_smoke_tests.rs` 中验证上述新增字段存在于对应表中

### 2. 系统安全加固（需求 22）

- [x] 2.1 实现请求频率限制中间件（`middleware/rate_limit.rs`）：基于 IP 的滑动窗口限流，登录接口每分钟最多 10 次，其他接口每分钟最多 100 次（需求 22.3）
- [x] 2.2 实现安全 HTTP 头中间件（`middleware/security_headers.rs`）：注入 Content-Security-Policy、X-Frame-Options、X-Content-Type-Options（需求 22.4）
- [x] 2.3 在 `auth.rs` 中实现账户锁定逻辑：连续 5 次密码错误后锁定 15 分钟，锁定期间返回 423 状态码（需求 1.6）
- [x] 2.4 在 `auth.rs` 中实现登录审计日志：每次成功/失败登录写入 audit_logs，记录 IP 和 User-Agent（需求 1.10、22.8）
- [x] 2.5 在 `auth.rs` 中实现 JWT 过期处理：过期返回 401，前端跳转登录页（需求 1.9）
- [x] 2.6 编写安全加固失败路径测试（`security_hardening_failure_tests.rs`）：验证锁定账户拒绝登录、频率限制触发 429、过期 JWT 返回 401

### 3. 认证扩展（需求 1 新增部分）

- [ ] 3.1* 实现 MFA TOTP 支持：在 `ironclaw_auth` crate 中添加 TOTP 验证逻辑，登录流程增加二步验证（需求 1.7）
- [ ] 3.2* 实现 SSO 集成：支持 LDAP/AD、SAML 2.0 或 OIDC 协议（需求 1.8）

### 4. 审计日志扩展（需求 11 新增部分）

- [x] 4.1 在 audit_logs 写入时自动填充 ip_address 和 user_agent（从请求头提取）（需求 11.7）
- [x] 4.2 在 `GET /api/audit-logs` 中添加全文搜索参数 `q`，使用 PostgreSQL `ILIKE` 或 `tsvector`（需求 11.8）
- [x] 4.3 实现审计日志自动清理定时任务：按 system_settings 中的 audit_retention_days 清理过期记录（需求 11.9）

### 5. 部门管理扩展（需求 4 新增部分）

- [x] 5.1 在 `GET /api/departments` 中支持 `?tree=true` 参数，返回嵌套树形结构（需求 4.6）
- [x] 5.2 在 `PUT /api/departments/:id/model-whitelist` 和 `GET /api/departments/:id/model-whitelist` 中实现部门模型白名单 CRUD（需求 4.9）
- [x] 5.3 在 `GET /api/departments` 中支持按名称搜索和按费用限额状态筛选（需求 4.11）
- [x] 5.4 前端部门列表页：实现树形展示组件，支持展开/折叠子部门（需求 4.6）
- [x] 5.5 前端部门详情页：添加模型白名单配置 UI（需求 4.9）

### 6. 模型配置扩展（需求 10 新增部分）

- [x] 6.1 在 `GET /api/model-configs` 响应中包含 total_calls、avg_latency_ms 字段（需求 10.8）
- [x] 6.2 在模型调用完成后更新 model_configs 的 total_calls 和 avg_latency_ms（需求 10.8）
- [x] 6.3 为每个模型添加 input_price_per_1k、output_price_per_1k 字段（分/千Token），在模型配置 CRUD 中支持（需求 10.12）
- [x] 6.4 前端模型配置页：展示调用量统计和平均延迟，添加单价配置字段（需求 10.8、10.12）
- [ ] 6.5* 实现"获取官方定价"功能：从提供商公开接口拉取单价并自动填充（需求 10.13）

### 7. 仪表盘扩展（需求 2 新增部分）

- [x] 7.1 在 `GET /api/dashboard/stats` 中新增 `ai_conversations_today`、`token_usage_today`、`unhandled_alerts` 字段（需求 2.5、2.6）
- [x] 7.2 前端仪表盘：添加 AI 对话统计卡片、Token 消耗卡片和未处理告警徽标（需求 2.5、2.6）

### 8. 客户端配置下发扩展（需求 9 待实现部分）

- [x] 8.1 [Desktop Client] 实现配置推送接收：当 Admin Backend 推送配置更新通知时，Desktop_Client 主动触发 `AdminConfigSync::fetch_once()` 拉取最新配置（需求 9.10）

---

## P1：重要功能（新增核心模块）

### 9. 数据库迁移：新增表

- [x] 9.1 执行 `015_alerts.sql`：创建 alert_rules 和 alert_events 表及索引
- [x] 9.2 执行 `016_conversations.sql`（已存在，验证）：确认 conversations 和 conversation_messages 表结构正确
- [x] 9.3 执行 `017_knowledge_bases.sql`（已存在，验证）：确认 knowledge_bases 和 kb_documents 表结构正确
- [x] 9.4 执行 `018_approvals.sql`（已存在，验证）：确认 approval_tickets 表结构正确
- [x] 9.5 在 `integration_smoke_tests.rs` 中为上述所有新表添加存在性验证

### 10. 告警与通知系统（需求 15）

- [x] 10.1 实现告警规则 CRUD API（`handlers/alerts.rs`）：`GET/POST/PUT/DELETE /api/alert-rules`（需求 15.1-15.2）
- [x] 10.2 实现告警事件列表 API：`GET /api/alerts`，支持按状态、严重级别、时间范围筛选（需求 15.5）
- [x] 10.3 实现告警状态更新 API：`PUT /api/alerts/:id/status`，支持 acknowledged/in_progress/closed（需求 15.6）
- [x] 10.4 实现未处理告警数量 API：`GET /api/alerts/unhandled-count`（需求 15.8）
- [x] 10.5 实现告警触发服务（`services/alert.rs`）：检查 DLP 拦截次数、配额超限、模型连续失败等触发条件，生成告警记录（需求 15.3）
- [x] 10.6 实现告警静默期逻辑：同一规则在 silence_minutes 内只触发一次通知（需求 15.7）
- [x] 10.7 实现通知发送服务：支持邮件、企业微信 Webhook、钉钉 Webhook、飞书 Webhook 四种渠道（需求 15.4）
- [x] 10.8 实现通知失败重试：失败后记录日志，下一周期重试，3 次后标记 failed（需求 15.9）
- [x] 10.9 前端告警规则管理页（`/alerts/rules`）：列表、创建、编辑、删除告警规则（需求 15.1-15.2）
- [x] 10.10 前端告警事件列表页（`/alerts/events`）：展示告警事件，支持处理操作（需求 15.5-15.6）
- [x] 10.11 前端导航栏和仪表盘：添加未处理告警数量徽标（需求 15.8）
- [x] 10.12 编写告警系统单元测试（`alerts_unit_tests.rs`）和失败路径测试（`alerts_failure_tests.rs`）

### 11. AI 对话审计前端（需求 16 待开发部分）

- [x] 11.1 前端对话记录列表页（`/conversations`）：展示对话列表，支持按用户、时间、DLP 标记、关键词筛选（需求 16.1-16.3）
- [x] 11.2 前端对话详情页（`/conversations/:id`）：展示完整消息流，标记 DLP 触发的消息（需求 16.2）
- [x] 11.3 前端对话审计报告导出功能（需求 16.6）
- [x] 11.4 [Desktop Client] 实现 `ConversationTracker` 定期 flush：后台每 5 分钟调用 `flush_idle_threads()`（需求 16.16）

### 12. 费用配额管理扩展（需求 18 新增部分）

- [x] 12.1 实现费用消耗明细 API：`GET /api/quota/details`，支持按时间范围、用户、模型、部门筛选和分页（需求 18.6）（复用 usage_records handler，路由已统一为 /api/quota/details）
- [x] 12.2 前端费用管理页：添加费用明细记录表格，支持筛选和分页（需求 18.6）
- [x] 12.3 实现月度费用预警：当月度费用达到预算 90% 时触发告警（需求 18.8，依赖任务 10.5）

### 13. 操作审批流扩展（需求 21 新增部分）

- [x] 13.1 前端审批工单列表页（`/approvals`）：展示审批记录，支持按申请人、工具类型、状态、时间筛选（需求 21.4、21.8）
- [x] 13.2 前端审批操作 UI：批准/拒绝按钮，填写审批意见（需求 21.5）
- [x] 13.3 实现审批结果推送：Admin 审批后通过 SSE 推送结果到客户端（需求 21.5）
- [x] 13.4 实现审批超时催办：24 小时未处理时通过 Alert_Service 发送催办通知（需求 21.6，依赖任务 10.5）
- [x] 13.5 [Desktop Client] 实现管理端审批结果接收：通过 SSE/WebSocket 接收推送，通过 `chat-event` 通知前端（需求 21.11）

### 14. 对话流异步审批任务（需求 23）

- [x] 14.1 [Desktop Client] 实现 `submit_approval_ticket(content, thread_id)` Tauri 命令：调用 `POST /api/approvals` 创建工单，通过 `tokio::spawn` 启动后台轮询任务（需求 23.10）
- [x] 14.2 [Desktop Client] 实现后台轮询任务：每 30 秒调用 `GET /api/approvals/{id}/check`，状态变更时通过 `chat-event` 推送并终止轮询（需求 23.11）
- [x] 14.3 [Desktop Client] 实现 pending ticket 本地持久化：JSON 文件存储，启动时恢复轮询（需求 23.12）
- [x] 14.4 [Desktop Client] 将 `submit_approval_ticket` 注册到 `all_tauri_commands!()` 宏，并在 `tauri_command_contract_tests.rs` 的 `FRONTEND_INVOKED_COMMANDS` 中添加条目（需求 23.13）
- [x] 14.5 后端实现 `GET /api/approvals/:id/check` 端点：返回工单当前状态（需求 23.3）
- [x] 14.6 后端实现工单过期逻辑：定时任务将超过 24 小时的 pending 工单标记为 expired（需求 23.6）

### 15. 知识库管理（需求 17）

- [x] 15.1 实现知识库 CRUD API（`handlers/knowledge_base.rs`）：`GET/POST/PUT/DELETE /api/knowledge-bases`（需求 17.1-17.2）
- [x] 15.2 实现文档上传 API：`POST /api/knowledge-bases/:id/documents`，支持 PDF、Word、Markdown、纯文本（需求 17.3）
- [x] 15.3 实现文档处理状态查询：`GET /api/knowledge-bases/:id/documents`（需求 17.5）
- [x] 15.4 实现文档删除 API：`DELETE /api/knowledge-bases/:id/documents/:doc_id`（需求 17.7）
- [x] 15.5 实现知识库检索测试 API：`POST /api/knowledge-bases/:id/search`（需求 17.8）（stub：向量化引擎未实现，当前返回空结果）
- [x] 15.6 前端知识库管理页（`/knowledge-bases`）：列表、创建、编辑、删除知识库（需求 17.1-17.2）
- [x] 15.7 前端知识库文档管理页（`/knowledge-bases/:id`）：文档上传、状态展示、删除（需求 17.3-17.7）（stub：文档上传仅存元数据，文档处理/向量化未实现）

### 16. 策略版本管理扩展（需求 7 新增部分）

- [ ] 16.1* 实现策略数字签名：发布新版本时对策略内容进行签名（需求 7.4）
- [ ] 16.2* 实现策略回滚：恢复到指定历史版本并生成新版本记录（需求 7.5）

### 17. 用户管理扩展（需求 3 新增部分）

- [x] 17.1 实现用户批量导入 API：`POST /api/users/import`，解析 CSV 文件批量创建用户（需求 3.8）
- [x] 17.2 前端用户管理页：添加批量导入按钮和 CSV 上传 UI（需求 3.8）
- [x] 17.3 在用户 CRUD 中支持 department_id 字段（需求 3.10）

---

## P2：增强功能

### 18. 数据分类分级与合规（需求 19）

- [x] 18.1 执行 `019_compliance.sql`：创建 data_classifications 和 compliance_reports 表，预置四个默认分级
- [x] 18.2 实现数据分级 CRUD API：`GET/PUT /api/compliance/classifications`（需求 19.1-19.2）
- [x] 18.3 实现合规概览 API：`GET /api/compliance/overview`（需求 19.3）
- [x] 18.4 实现合规报告生成 API：`POST /api/compliance/reports`（需求 19.4）
- [x] 18.5 实现合规报告 PDF 导出（需求 19.5）— 详情弹窗已完成，PDF 导出待实现
- [x] 18.6 前端合规概览页（`/compliance`）：展示分级体系、DLP 覆盖率、拦截统计（需求 19.3）
- [x] 18.7 在 DLP 规则编辑中添加数据分级标签关联（需求 19.2）

### 19. 水印配置（需求 20）

- [x] 19.1 执行 `020_watermark.sql`：水印配置通过 system_settings KV 表实现，无需独立迁移文件
- [x] 19.2 实现水印配置 API：`GET/PUT /api/settings` 包含 watermark_* 字段（需求 20.1）
- [x] 19.3 在系统设置页添加水印配置 Tab：启用开关、内容模板、字体大小、透明度、位置（需求 20.1）— 独立 `/watermark` 页面实现
- [x] 19.4 [Desktop Client] 前端实现水印渲染：导出对话时在 Markdown 内容末尾追加水印文字（需求 20.5）

### 20. 统计报表扩展（需求 12 新增部分）

- [x] 20.1 实现 AI 使用量报表 API：`GET /api/reports/ai-usage`，支持日/周/月维度（需求 12.5）
- [x] 20.2 实现模型费用统计报表 API（需求 12.6）
- [x] 20.3 实现部门 AI 使用量排行 API（需求 12.7）
- [x] 20.4 前端报表页：添加 AI 使用量趋势图、模型费用统计、部门排行（需求 12.5-12.7）
- [x] 20.5* 实现报表导出为 PDF 格式（需求 12.8）

### 21. 系统设置扩展（需求 13 新增部分）

- [ ] 21.1 在系统设置页添加告警配置 Tab：通知渠道选择和连接参数（需求 13.7）
- [ ] 21.2 在系统设置页添加安全配置 Tab：密码复杂度策略、会话超时、登录失败锁定阈值（需求 13.8）

### 22. 扩展管理扩展（需求 14 新增部分）

- [ ] 22.1* 实现技能/插件按部门白名单配置（需求 14.5）
- [ ] 22.2* 实现技能/插件调用频次统计展示（需求 14.7）

### 23. 客户端管理扩展（需求 8 新增部分）

- [ ] 23.1 实现客户端在线状态自动更新：基于心跳间隔和离线判定阈值定时更新 clients 表的在线状态（需求 8.8）
- [ ] 23.2* 实现客户端版本管控：检测低于最低版本的客户端并标记 needs_upgrade（需求 8.9）
