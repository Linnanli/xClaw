# 24 — MVP 范围与延后能力清单 (Pivot α 决策落地)

> **v0.1 (2026-04-25)** · 基于用户 2026-04-25 三项确认:
> - ✅ 14 改名 "技术参考清单"
> - ✅ 收敛到 3 crate + POC 级 MVP
> - ✅ Route B 22 crate 存档为 v2+ 愿景冻结

---

## §1 MVP 范围 (2026-12 交付目标)

### 1.1 新建 (3 个 crate)

| Crate | 用途 | 对应 Q5 | 预估 |
|-------|-----|--------|-----|
| `dasclaw_dlp_engine` | 双向 DLP 规则引擎 (输入+输出) | Top 1 (#6) | 2 月 |
| `dasclaw_cred_boundary` | 凭证 host 边界注入 | Top 2 (#10) | 1.5 月 |
| `dasclaw_sandbox_kernel` | 三平台沙箱 facade (Linux/Mac/Win) | Top 3 (#5) | 2 月 |

### 1.2 复用 (零改动 / 小补丁)

| 现有组件 | 复用方式 | MVP 集成动作 |
|---------|---------|------------|
| `admin-backend` 审计模块 | 现状继续用 | 写补丁让 DLP/sandbox 事件入审计流 |
| `claw-code/rust` LLM providers | 加国产 LLM 配置 (智谱/通义/豆包/DeepSeek/Qwen) | 配置文件 + 简单适配 |
| `claw-code` 5 个 MVP skill (file/search/web/code/kb) | 全部复用 | 接 dlp_engine 做敏感词过滤 |
| `ironclaw` / codex runtime | 当 backend,不 port | 通过现有接口调用 |
| `desktop-client` Tauri 客户端 | 现状继续 | 接新 crate 暴露的命令 |

### 1.3 主线 demo (2 条)

依据 Q3 答 "辅助编程 + 内部数据分析":

1. **Code review demo**: 上传一段含密钥的代码 → DLP 拦截 → 凭证从 host 注入 → sandbox 内执行
2. **HR 薪酬分析 demo**: 上传 Excel → DLP 检测 PII → 沙箱内跑分析 → 审计完整记录

---

## §2 延后到 v2+ 的能力清单

### 2.1 架构级延后 (原 Route B 22 crate 中的 19 个)

| Wave | Crate / 能力 | 原计划 | 延后理由 |
|------|------------|-------|---------|
| W3 | Codex 全套 port (78 crate → dasclaw_*) | 重做底层 | 4 资深人月预算根本不够 |
| W4 | Runtime 统一 (合并 ironclaw + claw-code) | 单一 runtime | 现有两套并存可工作 |
| W5 | Observer Chain 重构 (admin-backend 审计重设计) | 统一可观测 | 现有审计够 POC 用 |
| W6 | SkillsHub 治理引擎 | 商店级 | MVP 5 skill 写死即可 |
| W7 | 沙箱深度增强 (port codex landlock/seatbelt 完整代码) | 深度 | facade 层够 POC |
| W8 | 多租户 / 组织树 / 部门权限 / 配额 | 大客户必须 | Q8 D/E 客户延 v2 |
| W9 | 国密硬件 (TPM/SE/CryptoCard/USB Key SKF) | 硬件加密 | Q12 答 C 软实现 |
| W10 | 等保 / 密评 合规落地 | 认证就绪 | Q10 答先设计不做 |
| W_Q3 | 可视化 Agent 编辑器 | 无代码搭 Agent | 用户技术水平 C 但非 MVP 必需 |

### 2.2 功能级延后

**部署 / 基础设施**:
- 高可用 / 多副本 / 异地容灾
- Kubernetes Operator
- 灰度发布 / 蓝绿部署
- 监控告警体系 (Prometheus/Grafana 集成)

**多租户 / 企业**:
- 组织树管理 (1 万人企业必须)
- 部门权限模型
- 资源配额 (每用户 token 上限 / 每部门 Agent 数)
- SSO / LDAP / OAuth2 / OIDC 集成
- 审批工作流引擎

**计费 / 商业化**:
- Seat 按用户计费埋点
- LLM token 量精确计量
- Agent 调用次数计量
- API 调用计费
- SaaS 化所需的多租户隔离

**安全合规**:
- 国密硬件: TPM 2.0 / HSM / USB Key (SKF 接口)
- 审计不可篡改 (区块链 / WORM 存储)
- 等保 2.0 三级 / 四级合规落地
- 密评 (GM/T 0054) 认证
- GDPR / HIPAA / SOC2

**国产化适配**:
- 麒麟桌面 / 统信 UOS 客户端 (Q11 答靠后)
- 国产 CPU (鲲鹏/飞腾/龙芯) 服务端
- 国产数据库 (达梦/人大金仓/OceanBase) 后端

**生态**:
- SkillsHub 商店 (上架 / 审核 / 分发)
- WASM capability opt-in 插件运行时
- ISV 开发者门户
- Skill 数字签名与认证

**用户体验**:
- Web 客户端 (目前只有 Tauri 桌面)
- 移动端 (iOS / Android)
- 浏览器插件
- IDE 集成 (VSCode / JetBrains)

### 2.3 工程基础设施延后

| 项目 | 原计划 (18) | MVP 处理 | 延后理由 |
|------|-----------|---------|---------|
| ADR 体系 | 全套 (10+ ADR) | 缩到 3 个核心 ADR | 1 人维护不来 |
| Fuzz CI | 持续 fuzz 集成 | 不做 | 基础设施成本高 |
| 100% 测试覆盖 | 安全模块 100% | 70-80% | port 代码不现实 |
| 性能 baseline | 全链路 P50/P99 | DLP 单点 P50 | 全链路要客户场景 |
| 跨 crate 文档自动化 | rustdoc + mdbook | rustdoc 即可 | 1 人没精力 |
| benchmark 矩阵 | criterion + flamegraph | 关键路径 1-2 个 | M3 末再考虑 |

---

## §3 不延后但要降级的能力

| 能力 | 原 18 目标 | MVP 实际 | 降级幅度 |
|------|----------|---------|---------|
| DLP 准确率 | 90% | **75-80%** | -10% |
| 端到端 P50 延迟 | < 2s | **< 3-4s** | +50-100% |
| 沙箱启动 | < 100ms | **< 300ms** | +200% |
| 国产 LLM 适配数 | 6+ | **3-4** | -33% |
| 测试覆盖 (新 crate) | 90% | **70-80%** | -10-20% |
| 测试覆盖 (复用代码) | 80% | **现状不强求** | 不动 |

---

## §4 v2+ 路线图 (愿景存档)

按推测的客户拓展节奏排:

### v2 (2027 H1) — "可生产"
- 多租户 / 组织树 / 配额 (W8)
- SSO 集成
- 高可用部署
- 等保 3 级落地 (W10 部分)
- DLP 准确率提到 90%
- 沙箱深度增强 (W7 完整 port)

### v3 (2027 H2) — "国产化"
- 麒麟 / 统信适配
- 国密硬件 (TPM / HSM, W9 部分)
- 国产 CPU / 数据库
- 国产 LLM 增加到 8+

### v4 (2028) — "生态"
- SkillsHub 商店 (W6)
- WASM 插件 (W6 配套)
- 可视化 Agent 编辑器 (W_Q3)
- ISV 门户

### v5+ (2028+) — "Route B 完整"
- Codex 全套 port (W3)
- Runtime 统一 (W4)
- Observer Chain (W5)
- 22 个 dasclaw_* crate 完整体系

---

## §5 即将执行的文档动作 (你刚拍板后)

1. ✅ **本文档 (24)** 落盘 commit
2. **14 改名** → `14-technical-reference.md` + 删除"对标"章节
3. **15 升 v1.2**: 锁所有 19 答案为产品事实 + MVP 定位 = "POC 级标杆验证"
4. **18 大改 v2.0**: 13 Wave → 4 Wave (W0 骨架 / W1 DLP / W2 凭证 / W3 沙箱) + 砍掉延后 Wave + 8 月时间表
5. **20 重评 Red Flag**: 1 人 + AI 视角下 7 项重新打分
6. **21 升 v1.1**: 新增场景 D (独狼 + AI),废止场景 A/B/C
7. **17 附录 C**: 标记 19 个 crate 为"v2+ 冻结愿景"
8. **新增 25-real-competitor-parity.md**: 对标智谱/Dify/M365 (Q5 Top 3 维度)

预估 1-2 轮对话内完成所有文档变更,然后进 C (3 ADR) → A (W0 编码)。

---

## 变更历史

- **v0.1 (2026-04-25)**: 用户三项确认后落地的 MVP 范围与延后能力首版清单。
