# 25 — 现有代码盘点 · 修正 24 延后清单

> **v0.1 (2026-04-25)** · 回应用户 "你说延后的功能部分已有初版了" 的挑战
>
> 用 semantic_search + grep 扫了 admin-backend 和 desktop-client 实际代码,发现 **24 号文档对"延后能力"的定性严重错误**。大量我列为"v2+ 延后"的能力已有 v0.x 可运行初版。

---

## §1 发现摘要

| 类别 | 24 原判定 | 实际情况 | 差异 |
|------|---------|---------|-----|
| RBAC 角色权限 | 延后 v2 | ✅ 已有 `/api/roles` `/api/permissions` 完整 CRUD | 错判 |
| 部门管理 | 延后 v2 | ✅ migration 012/014 + handlers/departments.rs (4 API) | 错判 |
| 配额 / 计费埋点 | 延后 v2 | ✅ migration 015 三级配额 + usage_records + 10 quota API | **严重错判** |
| 审批流 | 延后 v2 | ✅ migration 018 + handlers/approvals.rs (6 API) + desktop `approval_polling` | 错判 |
| 告警中心 | 延后 v2 | ✅ migration 016 + handlers/alerts.rs (11 API) | 错判 |
| 对话管理 / 审计 | MVP 复用 | ✅ migration 017 + conversations.rs (4 API) + audit-logs API | 定性对 |
| 合规管理 | 延后 v2 | ✅ migration 019 + handlers/compliance.rs (5 API) + UI 页面 | 错判 |
| 知识库 | 延后 v2 | ✅ migration 021 + handlers/knowledge_base.rs (8 API) | 错判 |
| 客户端管理 | 延后 v2 | ✅ migration 009/022 + clients API + push-policy + disconnect | 错判 |
| DLP 双向 | 新建 3 crate 之一 | ✅ Admin: migration 003/005/006 + CRUD; Desktop: dlp/* 8 个文件 + 测试 | **需要合并不是新建** |
| 敏感操作策略 | 延后 | ✅ migration 007 + API | 错判 |
| 策略变更记录 | 延后 | ✅ migration 008 + API + 变更统计 | 错判 |
| 扩展/Skills 管理 | 部分有 | ✅ migration 010/023 + extensions.rs (19 API) | 定性对但严重低估规模 |
| 扫描结果 | 未提 | ✅ migration 025 + scanner 模块 | 遗漏 |
| 附件管理 | 未提 | ✅ migration 026 | 遗漏 |
| Code 工具设置 | 未提 | ✅ migration 027 + code_tools.rs (11 API) | 遗漏 |
| LSP 服务器设置 | 未提 | ✅ migration 028 | 遗漏 |
| 模型管理 + 定价 | 未提 | ✅ migration 013 + 015 输入/输出单价 | 遗漏 |
| 企业策略同步 | 未提 | ✅ desktop `enterprise_policy_sync` + `policy_sync` + `managed_policy` | 遗漏 |
| 水印 | 未提 | ✅ UI `watermark.tsx` 页面 | 遗漏 |
| 安全管理 UI | 未提 | ✅ UI `security.tsx` | 遗漏 |

### 1.1 Admin Backend 规模统计

- **28 个 migration** (从 001_init 到 028_lsp_server)
- **11 个 handler 模块** (alerts/approvals/code_tools/compliance/conversations/departments/extensions/knowledge_base/quota/reports/mod)
- **handler API 总数 ≈ 85 个** (+ routes.rs 内联的 users/roles/dlp/audit ≈ 30+ 个) = **约 120+ HTTP API**
- **18 个管理 UI 页面** (alerts / approvals / audit-logs / clients / compliance / conversations / dashboard / departments / extensions / knowledge-bases / login / model-configs / quota / reports / security / settings / users / watermark)

### 1.2 Desktop Client 规模统计

- **20+ 个 Rust 模块** (admin_sync / approval_polling / auth_token_manager / conversation_tracker / data_reporter / dlp (8 文件) / dlp_integration / engine / enterprise_policy_sync / managed_policy / model_switch / platform_utils / policy_sync / safety_bridge / tauri_channel / vercel_ui_protocol / workspace_dir)
- **DLP 模块已有 13 个文件** (detector / patterns / sanitizer + 8 个专项测试族: change_coverage / code_coverage / data_coverage / integration / policy_sync / reliability / requirements / security)
- **Tauri IPC 通道**: tauri_channel + vercel_ui_protocol (SSE / UI 协议)

---

## §2 真正未建的能力 (实际 v2+ 延后)

经验证确实缺失:

| 能力 | 状态 | 优先级 |
|------|-----|-------|
| SSO / LDAP / OAuth2 / OIDC | ❌ 未建 (OAuth 字样仅在 codex/claw-code MCP 外部配置中) | v2 |
| 审计 WORM / 区块链 / 不可篡改存储 | ❌ 未建 | v3 |
| 国密硬件 (TPM/HSM/USB Key SKF) | ❌ 未建 | v3 |
| 国产 CPU/OS/DB 适配 (鲲鹏/飞腾/麒麟/统信/达梦) | ❌ 未建 | v3 |
| Kubernetes Operator | ❌ 未建 | v2 |
| 高可用 / 多副本 / 异地容灾 | ❌ 未建 | v2 |
| Web 客户端 (纯浏览器无 Tauri) | ❌ 未建 | v2-v3 |
| 移动端 (iOS / Android) | ❌ 未建 | v4+ |
| 浏览器插件 / IDE 集成 | ❌ 未建 | v4+ |
| SkillsHub 商店 (extensions 有管理无"商店") | ❌ 未建 | v4 |
| WASM capability opt-in 插件运行时 | ❌ 未建 (仅 wasm channel 壳) | v4 |
| ISV 开发者门户 | ❌ 未建 | v4+ |
| Skill 数字签名与认证 | ❌ 未建 | v4 |
| 灰度发布 / 蓝绿部署 | ❌ 未建 | v2 |
| Prometheus/Grafana 监控告警 | ❌ 未建 | v2 |

---

## §3 真正需要做的是 "收口 + 增强",不是 "新建 3 crate"

### 3.1 DLP: 不是新建 `dasclaw_dlp_engine`

**现状**:
- admin-backend 有完整 DLP 规则 CRUD + 字典管理 + 批量操作 + 导入导出 (10+ API)
- desktop-client 有 detector/patterns/sanitizer + 8 测试族
- migration 003/005/006 三套迭代

**真实任务**: **"DLP 能力收口与增强"**
- 提取共享规则引擎到 `dasclaw_dlp_engine` crate (从 desktop-client 抽出 + 与 admin-backend 规则格式统一)
- 补齐 Q5 Top 1 诉求: 双向 (当前可能偏输出侧,需核验输入侧覆盖)
- 对齐 Q5 诉求的准确率 75-80%

### 3.2 凭证 Host 边界注入: 确实需要新建

- desktop-client 已有 `auth_token_manager` / `enterprise_policy_sync` / `safety_bridge` 但**不是 host 边界注入哲学**
- 需要新建 `dasclaw_cred_boundary` 实现: 凭证**仅存 host 侧**,通过 sandbox 运行时注入,被调用代码拿不到原值

### 3.3 沙箱 Kernel: 不是新建而是抽取

**现状**:
- codex-rs 有 landlock-sandbox / seatbelt-sandbox / execpolicy
- ironclaw 有 sandbox channel
- claw-code/rust 有安全相关 crate

**真实任务**: **"沙箱 facade 抽取 + 三平台统一"**
- 新建 `dasclaw_sandbox_kernel` 做 facade,不新做底层
- 聚合 codex 三平台能力 + 暴露统一 API

---

## §4 修正后的 MVP 工作量估算

### 4.1 真实工作分解

| 任务 | 工作类型 | 估算 (1 人 + AI) |
|------|--------|---------------|
| W0 骨架: 3 crate 空壳 + CI + ADR | 架构 | 3 周 |
| W1 DLP 收口: 从 desktop-client 抽 + 与 admin-backend 对齐 + 双向增强 | 收口+增强 | 1.5 月 |
| W2 凭证 host 注入: 真正新建 | 纯新建 | 1.5-2 月 |
| W3 沙箱 facade: 抽取 codex 三平台 | facade+集成 | 1.5-2 月 |
| W4 国产 LLM 适配补全 (claw-code providers 上叠加) | 配置 | 0.5 月 |
| W5 MVP 两条 demo E2E (code review + HR 薪酬) | 集成测试 | 1 月 |
| W6 POC 打磨 + 标杆客户预对接 | 打磨 | 1 月 |
| **缓冲 (1 人项目必须)** | 缓冲 | 1 月 |
| **总计** |   | **8-9 月** |

### 4.2 更现实的 MVP 定位

**不是**"从零做 POC",**而是**:
- ✅ "把已有 v0.x 系统的 Q5 Top 3 差异化能力从 MVP 级升到 POC 级可演示"
- ✅ "统一 admin ↔ desktop 的 DLP 规则格式与执行语义"
- ✅ "补齐凭证 host 边界注入这个真正缺失的核心能力"
- ✅ "抽取沙箱 kernel facade 替代散落在 codex/ironclaw/claw-code 的 ad-hoc 实现"

### 4.3 这比我之前估算**更乐观**

原 23 号评估: 3 新 crate × 2 月 ≈ 5.5-6 人月,8 月紧张

新评估: 1.5-2 月/crate(含收口) + 2 月集成 + 1 月缓冲 = **约 7-8 月可达 POC**,留 1 月应对标杆客户对接

---

## §5 修正 24 延后清单

### 5.1 从 "延后" 改为 "现状保持 / 小补丁"

- RBAC / 部门 / 配额 / 审批 / 告警 / 审计 / 合规 / 知识库 / 客户端管理 / 模型管理 / 策略管理 / 扩展管理 / 敏感操作 / 扫描 / Code 工具 / LSP / 水印 / 安全管理
- 共 **18 项** 从 v2+ 延后列表中**移除**
- MVP 改为 "保持现有能力正常工作 + 必要小补丁"

### 5.2 真实延后清单 (v2+) 收敛到 15 项

仅保留 §2 列出的 15 项真正未建能力。

### 5.3 需要重写的文档

| 文档 | 修正动作 |
|------|---------|
| 24-mvp-scope-and-deferred.md | v0.2: 大改延后清单,从"40+ 项延后"收敛到"15 项" |
| 23-ai-assisted-dev-reality-check.md | v0.2: 工作量从"新建"改为"收口+增强",略乐观 |
| 22-product-constraints-impact.md | v0.2: 强调 "底子比想象厚",Pivot α 更轻量 |
| 18-execution-roadmap.md | v2.0: Wave 数量从 13 → 6 (W0骨架/W1 DLP 收口/W2 cred/W3 sandbox/W4 demo/W5 打磨) |

---

## §6 结论

**你的质疑是对的,我之前低估了现有代码底子。**

**关键更正**:
1. MVP 不是 "新建 3 crate 做 POC", **而是 "在已有 v0.x admin + desktop 基础上收口 Q5 Top 3 差异化 + 新建少量真正缺失的核心模块"**
2. 延后清单从 40+ 项 → 收敛到 15 项真实缺失能力
3. 工作量从 "极度紧张" → "紧张但可达"
4. DLP 不是从零,是**统一 admin ↔ desktop 的实现与规则格式**
5. 沙箱不是新建,是 **facade 抽取**
6. 唯一真正新建的是 **`dasclaw_cred_boundary`** (凭证 host 边界注入)

**下一步 (等你拍板)**:
1. 确认接受 §3 修正的工作定性?
2. 开始执行 §5.3 的 4 份文档修正 (24/23/22/18)?
3. 然后进 14 改名 + C (3 ADR) → A (W0 编码)?

---

## 变更历史

- **v0.1 (2026-04-25)**: 用户挑战触发的现有代码盘点与 24 延后清单修正。
