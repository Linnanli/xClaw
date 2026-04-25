# 26 — 现有能力逐项审核矩阵

> **v0.1 (2026-04-25)** · 回应用户 "先审查一下哪些能力具体状态,逐个确认"
>
> 基于 LOC 统计 + TODO 密度 + 测试族分布 + handler 数量的硬数据审核。每项标注 [DB/API/UI/Desktop/Tests] 五个维度,你逐项确认状态。

---

## §0 代码规模总览 (硬数据)

| 层 | LOC | 备注 |
|----|-----|------|
| Admin Backend Rust 源码 (handlers + routes + models + middleware + scanner + policy) | **~16,429** | 含 routes.rs 5938 行 |
| Admin Backend migrations (28 个) | **1,024** | 001-028 |
| Admin Backend tests (43 个文件) | **12,652** | contract/failure/unit/integration/security_audit/reliability/regression |
| Admin UI pages (18 个) | **9,603** | React+TS |
| Admin UI components | **3,647** |   |
| Desktop Client Rust 源码 (root) | **12,736** | 20+ 模块 |
| Desktop DLP 模块 | **5,645** | 13 文件 |
| Desktop tests | **3,277** | 10 文件 |
| Desktop ironclaw bundled subdir | ~627,000 | codex/ironclaw bundle (不算自写) |
| **合计 (不含 ironclaw bundle)** | **~65,013 LOC** | 纯自写 |

**TODO/FIXME/unimplemented 密度**: admin handlers 共 5 处 (alerts 3 + knowledge_base 1 + departments 1), 其他 0 → **绝非占位代码**

**测试族类型分布**:
- admin: unit / failure / integration / contract / security_audit / reliability / regression / property
- desktop: integration / regression / reliability / requirements / property / e2e (qwen)

---

## §1 逐项能力审核矩阵

> 图例: ✅ 完整 / 🟡 部分 / 🔴 骨架/存疑 / ❌ 无

### §1.1 身份与权限 (AuthN/AuthZ)

| 能力 | DB schema | Backend API | Admin UI | Desktop 集成 | 测试 | MVP 可用? | 你的判定 |
|------|----------|-----------|---------|------------|-----|----------|--------|
| 用户管理 | ✅ 001_init | ✅ `/api/users` CRUD (routes 87-93) + import | ✅ users.tsx (601 LOC) | ✅ auth_token_manager (588 LOC) | ✅ user_edit_tests + auth_* (5 文件) | ✅ | [ ] |
| 角色 (Role) | ✅ 002_rbac | ✅ `/api/roles` CRUD + assign | ✅ 嵌在 users.tsx | 🟡 token 带 role claim | ✅ auth_property_tests | ✅ | [ ] |
| 权限 (Permission) | ✅ 002_rbac | ✅ `/api/permissions` + `/api/roles/{id}/permissions` | ✅ 嵌在 users.tsx | 🔴 Desktop 未强制检查 | ✅ | 🟡 **需补 Desktop 侧权限门** | [ ] |
| SSO/LDAP/OAuth2/OIDC | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ **真延后** | [ ] |
| JWT Token 刷新 | ✅ | ✅ `/api/auth/refresh` | ✅ login.tsx (275 LOC) | ✅ auth_token_manager | ✅ auth_regression_tests | ✅ | [ ] |

### §1.2 组织与配额

| 能力 | DB | API | UI | Desktop | 测试 | MVP? | 判定 |
|------|---|-----|---|--------|-----|------|-----|
| 部门树 (单根) | ✅ 012+014 (42 行) | ✅ handlers/departments.rs (326 LOC, 4 API) | ✅ departments.tsx (**1117 LOC, 最大 UI**) | ❌ (Desktop 不感知部门) | ✅ department_unit/integration/failure | ✅ | [ ] |
| 三级配额 (org/dept/user) | ✅ 015 | ✅ handlers/quota.rs (**864 LOC, 10 API**) | ✅ quota.tsx (344 LOC) | 🟡 data_reporter 上报 usage | ✅ quota_unit + quota_failure | ✅ | [ ] |
| 计费埋点 usage_records | ✅ 015 | ✅ report-usage / usage-records API | ✅ 部门/模型排行 | 🟡 data_reporter | ✅ | ✅ | [ ] |
| 模型单价 (分/千 Token) | ✅ 013+015 | ✅ 嵌在 quota.rs | ✅ model-configs.tsx (544 LOC) | ✅ model_switch | ✅ model_config_unit/contract/failure/security_audit | ✅ | [ ] |

### §1.3 策略与合规

| 能力 | DB | API | UI | Desktop | 测试 | MVP? | 判定 |
|------|---|-----|---|--------|-----|------|-----|
| DLP 规则 (输入) | ✅ 003+005+006 | ✅ /api/dlp-rules CRUD + 批量 + 导入导出 | ✅ 嵌在 security.tsx (**922 LOC**) | ✅ dlp/detector+patterns (2 文件 核心) | ✅ **DLP 专项 8 测试族** (change/code/data/integration/policy_sync/reliability/requirements/security) | ✅ | [ ] |
| DLP 输出拦截 | 🟡 规则数据模型够 | 🟡 需确认是否覆盖响应侧 | 🟡 同上 | 🟡 sanitizer.rs 有但需验证覆盖 | ✅ | 🟡 **Q5 Top 1 要补验证** | [ ] |
| DLP 字典 | ✅ 006 | ✅ /api/dlp-dictionaries CRUD | ✅ | 🟡 Desktop 是否拉取字典 | ✅ | 🟡 | [ ] |
| 敏感操作策略 | ✅ 007 | ✅ /api/sensitive-operations CRUD | ✅ | ✅ managed_policy + safety_bridge | ✅ safety_bridge_tests (900 LOC!) | ✅ | [ ] |
| 策略变更记录 | ✅ 008 | ✅ /api/policy-changes + stats | ✅ 嵌在 security.tsx | ✅ policy_sync | ✅ policy_sync_property_tests | ✅ | [ ] |
| 合规管理 | ✅ 019 | ✅ handlers/compliance.rs (308, 5 API) | ✅ compliance.tsx (407) | ❌ Desktop 未对接 | ✅ compliance_unit/failure | 🟡 **合规 UI 有,Desktop 未对接** | [ ] |
| 安全字段 | ✅ 020 | 🟡 嵌在多处 | ✅ security.tsx (922) | ✅ | ✅ security_hardening_failure | ✅ | [ ] |
| 等保 2 合规落地 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ **真延后 Q10 答复** | [ ] |
| 密评合规 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ **真延后** | [ ] |
| 凭证 host 边界注入 (Q5 Top 2) | 🟡 auth_token 有但非 host 注入 | 🟡 | 🟡 | 🔴 **不是 host 注入哲学** | 🟡 | 🔴 **真要新建** | [ ] |

### §1.4 审计与观测

| 能力 | DB | API | UI | Desktop | 测试 | MVP? | 判定 |
|------|---|-----|---|--------|-----|------|-----|
| 审计日志 (audit_logs) | ✅ 001+004+007 | ✅ /api/audit-logs GET/POST/export | ✅ audit-logs.tsx (544 LOC) | ✅ data_reporter 上报 | ✅ audit_integrity + audit_log_export | ✅ | [ ] |
| 客户端事件 | ✅ 011 | ✅ /api/client-events | ✅ | ✅ data_reporter_tests (606 LOC) | ✅ client_config_reports 5 文件 | ✅ | [ ] |
| 对话跟踪 | ✅ 017 | ✅ handlers/conversations.rs (352, 4 API) | ✅ conversations.tsx (297) | ✅ conversation_tracker (491 LOC) | ✅ conversations_unit/failure | ✅ | [ ] |
| 附件存储 | ✅ 026 | 🟡 handler 未独立暴露 | 🟡 嵌在 conversations | 🟡 | 🟡 | 🟡 **需确认** | [ ] |
| 告警中心 | ✅ 016 (72 行) | ✅ handlers/alerts.rs (**902 LOC, 11 API**) | ✅ alerts.tsx (445) | 🟡 未直接拉 alerts | ✅ alerts_unit/failure/contract | ✅ | [ ] |
| 审计 WORM / 不可篡改 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ **真延后** | [ ] |
| Prometheus/Grafana 集成 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ **真延后** | [ ] |

### §1.5 客户端与策略下发

| 能力 | DB | API | UI | Desktop | 测试 | MVP? | 判定 |
|------|---|-----|---|--------|-----|------|-----|
| 客户端注册与管理 | ✅ 009+022 | ✅ /api/clients (含 stats/disconnect/push-policy/push-policy-all) | ✅ clients.tsx (224) | ✅ admin_sync (374) + tests (644) | ✅ client_operations_tests | ✅ | [ ] |
| 客户端配置上报 | ✅ 011 | ✅ | ✅ | ✅ data_reporter | ✅ client_config_reports 5 文件 | ✅ | [ ] |
| 企业策略同步 | ✅ | ✅ | ✅ | ✅ **enterprise_policy_sync (901 LOC)** + managed_policy (421) + policy_sync (122) | ✅ managed_skills_sync_tests | ✅ **这是核心优势** | [ ] |
| 实时推送 (SSE/WebSocket) | 🟡 tauri_channel 实现 SSE | 🟡 | 🟡 | ✅ tauri_channel (844 LOC) + tests (482) + vercel_ui_protocol (417) | ✅ tauri_channel_tests | ✅ | [ ] |
| 模型白名单/切换 | ✅ 013 | ✅ | ✅ | ✅ model_switch (128) + tests (305) | ✅ model_whitelist_integration | ✅ | [ ] |

### §1.6 沙箱与工具链

| 能力 | DB | API | UI | Desktop | 测试 | MVP? | 判定 |
|------|---|-----|---|--------|-----|------|-----|
| Code 工具设置 | ✅ 027 (43 行) | ✅ code_tools.rs (403, 11 API) | ✅ 嵌 settings? | 🟡 engine 有 skill 但 code_tools 是否 wire? | 🟡 | 🟡 **需确认 wiring** | [ ] |
| LSP 服务器设置 | ✅ 028 | 🟡 需查 | 🟡 | 🟡 | 🟡 | 🟡 | [ ] |
| Skills/Extensions 管理 | ✅ 010+023 (186 行) | ✅ extensions.rs (**2193 LOC, 19 API**) | ✅ extensions.tsx (**2106 LOC**) | ✅ engine (1036) + engine_skills_tests | ✅ extensions **7 测试族** (contract/failure/integration/regression/reliability/security_audit/unit) | ✅ **最成熟** | [ ] |
| 扫描结果 | ✅ 025 | ✅ scanner.rs (479) | 🟡 嵌 security? | 🟡 | ✅ scanner_unit_tests | 🟡 | [ ] |
| 沙箱 kernel (Q5 Top 3) | N/A | N/A | N/A | 🟡 **分散在 codex landlock/seatbelt + ironclaw channel + safety_bridge** | ✅ safety_bridge_tests (900) | 🟡 **需 facade 抽取** | [ ] |
| WASM 插件运行时 | ❌ | ❌ | ❌ | 🟡 desktop-client/ironclaw/src/channels/wasm/ 有壳 | ❌ | 🔴 **v4 延后** | [ ] |
| SkillsHub 商店 (上架/审核/分发) | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ **真延后** | [ ] |
| Skill 数字签名 | ❌ | 🟡 ed25519_dalek 库已引 (routes.rs) | ❌ | ❌ | ❌ | 🔴 **骨架,延后** | [ ] |

### §1.7 UX 能力

| 能力 | 状态 | 判定 |
|------|-----|------|
| Admin Web UI (18 页) | ✅ 成熟 | [ ] |
| Desktop Tauri 客户端 | ✅ Rust + React (src-ui) | [ ] |
| Web 客户端 (纯浏览器) | ❌ | [ ] |
| 移动端 (iOS/Android) | ❌ | [ ] |
| IDE 集成 (VSCode/JetBrains) | ❌ | [ ] |
| 浏览器插件 | ❌ | [ ] |
| 国产麒麟/统信桌面 | ❌ | [ ] |
| 水印 | ✅ watermark.tsx (160) | [ ] |

### §1.8 基础设施

| 能力 | 状态 | 判定 |
|------|-----|------|
| Rate Limit | ✅ middleware/rate_limit.rs (172) | [ ] |
| Security Headers | ✅ middleware/security_headers.rs (69) | [ ] |
| 仪表盘 | ✅ dashboard.tsx (258) + handlers/reports.rs (189) + dashboard_unit_tests | [ ] |
| K8s/高可用/灰度 | ❌ | [ ] |
| 容灾备份 | ❌ | [ ] |
| 国密硬件 (TPM/HSM/USB Key) | ❌ | [ ] |
| 国产 CPU/OS/DB | ❌ | [ ] |

---

## §2 真正需要澄清的灰色项 (你拍板)

我标 🟡 的那些,需要你确认:

1. **DLP 输出拦截 (Q5 Top 1 关键)**: 现有 sanitizer 是否真覆盖 LLM 响应侧?需我读 sanitizer.rs 验证吗?
2. **DLP 字典 Desktop 拉取**: Desktop dlp 是从 admin 拉字典,还是硬编码?
3. **合规 UI 与 Desktop 对接**: compliance 管理后台有 UI,但 Desktop 未见 wiring。这是当前遗漏还是已通过 managed_policy 代理?
4. **凭证 host 注入哲学**: auth_token_manager 是"客户端保管 token"还是"host 侧保管、沙箱内拿不到原值"?这决定 Q5 Top 2 工作量。
5. **Code 工具/LSP 设置 wiring**: 后台有 API,Desktop 是否真订阅并使用?
6. **扫描结果 UI**: scanner.rs 有 479 行但我没定位到对应 UI 页面。
7. **沙箱 facade 工作量**: 是把 ironclaw sandbox channel + codex landlock + safety_bridge 合到一起,还是另起炉灶?
8. **附件存储 (026 migration 仅 1 行)**: 是不是空架子?
9. **权限强制点**: Desktop 侧 RBAC 权限是否在关键命令处强制检查?

---

## §3 MVP 收敛后的 **真实任务清单**

基于审核结果,MVP 实际要做的 **不是 "新建 3 crate"**,而是:

### 🔴 必须做 (Q5 Top 3 兑现)

| # | 任务 | 性质 | 工作量 (1人+AI) |
|---|------|-----|--------------|
| T1 | DLP **输出侧拦截**验证 + 双向规则语义对齐 admin↔desktop | 收口 + 补齐 | 1.5 月 |
| T2 | 凭证 host 边界注入 (新建 `dasclaw_cred_boundary`) | **纯新建** | 2 月 |
| T3 | 沙箱 facade 抽取 (新建 `dasclaw_sandbox_kernel` 统一 landlock/seatbelt/process 三平台) | facade+集成 | 2 月 |

### 🟡 应该做 (POC 体验完整)

| # | 任务 | 性质 | 工作量 |
|---|------|-----|-------|
| T4 | Desktop 侧 RBAC 权限强制点补齐 | 补齐 | 0.5 月 |
| T5 | 合规 UI ↔ Desktop 对接 | 集成 | 0.5 月 |
| T6 | DLP 字典 Desktop 动态拉取 | 集成 | 0.2 月 |
| T7 | MVP 两条主线 demo E2E (Q3 辅助编程 + HR 分析) | 集成测试 | 1 月 |

### 🟢 不做 (v2+ 真延后,共 15 项,见 §1 各层 ❌ 标记)

SSO/LDAP · 审计 WORM · 国密硬件 · 国产化适配 · K8s · Web 客户端 · 移动端 · IDE 插件 · 浏览器插件 · SkillsHub 商店 · WASM 插件 · ISV 门户 · Skill 签名落地 · 灰度发布 · Prometheus/Grafana

**总计**: T1+T2+T3+T4+T5+T6+T7 ≈ 7.7 人月,加缓冲 1 月 → **8.7 月,Q4 末 MVP 仍可达成**

---

## §4 给用户的确认清单

请逐项给我反馈 (在本文档的"你的判定"列打勾 / 或直接回复):

### 4.1 要我先深入验证的灰色项 (§2 九个)

- [ ] DLP 输出拦截覆盖验证
- [ ] DLP 字典 Desktop 拉取机制
- [ ] 合规 UI↔Desktop wiring
- [ ] 凭证 host 注入现状
- [ ] Code tools/LSP wiring
- [ ] 扫描结果 UI
- [ ] 沙箱 facade 定义
- [ ] 附件存储真实性
- [ ] Desktop RBAC 强制点

### 4.2 是否接受 §3 的 7 项任务清单?

- [ ] T1-T7 都做 (8.7 月)
- [ ] 砍掉 T4-T6 只做 T1+T2+T3+T7 (6.5 月,更保险)
- [ ] 其他组合: _______

### 4.3 是否接受 §3 的 15 项真延后?

- [ ] 全接受
- [ ] 有哪项要提前: _______

---

## 变更历史

- **v0.1 (2026-04-25)**: 基于代码 LOC/TODO/测试族硬数据的逐项能力审核矩阵。
