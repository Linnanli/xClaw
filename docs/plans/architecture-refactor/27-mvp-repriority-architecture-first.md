# 27 — MVP 重排: 架构优先 + 客户端基础 (合规延后)

> **v0.1 (2026-04-25)** · 回应用户 2026-04-25 方向:
> "我的想法是着重把架构搭建起来, 让客户端拥有基础能力, 合规这些能力都可以延后处理"

---

## §1 9 个灰色项深挖结果

| # | 项 | 深挖结论 | 影响 MVP? |
|---|-----|--------|----------|
| 1 | DLP 输出拦截 | ❌ **几乎无痕迹** — grep output/response 在 desktop dlp 仅 1 命中 (测试函数名),未做 LLM 响应侧拦截 | 🟡 Q5 Top 1 差异化需要,但可降级为"基础版 + 手动入口" |
| 2 | DLP 字典 Desktop 拉取 | ✅ **已支持** — desktop-client/src/ipc/dlp.rs:250/353/361 明确处理 `dictionary` 规则类型 | ✅ 不额外工作 |
| 3 | 合规 UI ↔ Desktop wiring | ❌ **Desktop 完全无 compliance 关键字** | 🟢 **用户决定延后** → v2 |
| 4 | 凭证 host 注入哲学 | ❌ 现状是 auth_token_manager.rs "客户端生成+存本地+作为 gateway auth" — **完全不是 host 边界注入** | 🟡 Q5 Top 2 核心,但严格版可降为"基础凭证隔离" |
| 5 | Code tools/LSP wiring | ❌ **Desktop 无 code_tool/lsp_server 字样** — admin 有配置 API 未被订阅 | 🟢 不影响 MVP 核心 demo |
| 6 | 扫描结果 UI | ✅ **已 wired** — extensions.tsx 把 scan-results 作为插件审核子功能 (多处 scanResultsEndpoint / uploadScanResult) | ✅ 不额外工作 |
| 7 | 沙箱 facade 定义 | 🟡 **散落三处**: codex landlock-sandbox / seatbelt-sandbox + ironclaw/src/channels/sandbox + safety_bridge — 需抽取 facade | 🔴 必做 |
| 8 | 附件存储 (026 migration 1 行) | ✅ **已用** — 给 conversation_messages 加 `attachments JSONB` 列 (不是独立表,是字段扩展) | ✅ 不额外工作 |
| 9 | Desktop RBAC 强制点 | ❌ **完全没有** — grep check_permission/require_permission/has_permission 零匹配 | 🟡 一般企业需要,但 POC 可不做 |

### §1.1 惊喜发现: Desktop IPC 层超完整

`desktop-client/src/ipc/*` 共 **9335 LOC, 26 个文件**:

| IPC 模块 | LOC | 说明 |
|---------|-----|------|
| skills | 1049 | Skill 注册/执行/管理 |
| chat | 762 | 对话流 |
| extensions | 635 | 扩展管理 |
| dlp | 447 | DLP 规则执行 (有字典支持) |
| routines | 441 | 自动化流程 |
| models | 440 | 模型切换 |
| jobs | 409 | 任务队列 |
| + memory / logs / threads / approval / plan_mode / file_ops / workspace / persistence | ~3150 | 完整 Agent 能力面 |

**这意味着客户端基础能力已相当完整**,架构焦点应是"收口已有能力 + 暴露清晰边界"而非"从零做基础功能"。

---

## §2 按用户方向重排优先级

### 2.1 新的重心

| 层 | 优先级 | 理由 |
|----|-------|------|
| **L1 架构骨架** | 🔴 Top 1 | 用户明确"着重架构搭建",是 8 月能否交付的地基 |
| **L2 客户端基础能力健壮性** | 🔴 Top 2 | 用户明确"客户端拥有基础能力",已有底子要走通 E2E |
| **L3 Q5 Top 3 差异化 (基础版)** | 🟡 Top 3 | DLP/凭证/沙箱 保基础版,不追求极致 |
| **L4 合规** | 🟢 延后 | 用户明确"可以延后处理" |
| **L5 大客户特性** (D/E 1 万人) | 🟢 延后 | POC 阶段不承诺 |

### 2.2 删除 / 降级 项

相较 26 号文档的 T1-T7 任务清单:

| 任务 | 原估 | 调整 |
|------|------|-----|
| T1 DLP **严格双向** + 准确率 80% | 1.5 月 | 🟡 **降级到 0.5 月**: 只做"输出侧基础拦截入口 + 现有字典复用",不追求指标 |
| T2 凭证 host **严格哲学** 注入 | 2 月 | 🟡 **降级到 0.8 月**: 做"凭证隔离接口"但不做完整 host 边界重构 |
| T3 沙箱 facade | 2 月 | ✅ **保留 1.5 月**: 架构骨架核心,不能砍 |
| T4 RBAC 强制点 | 0.5 月 | ❌ **砍** (POC 不需要) |
| T5 合规 UI↔Desktop | 0.5 月 | ❌ **砍** (v2) |
| T6 DLP 字典拉取 | 0.2 月 | ✅ **发现已支持** → 0 |
| T7 MVP 两条 demo E2E | 1 月 | ✅ 保留 |

### 2.3 新增 / 上调 项

| # | 任务 | 优先级 | 估算 |
|---|------|------|------|
| **A1** | **架构骨架搭建**: 3 crate + workspace 依赖图 + ADR × 3 + CI 模板 | 🔴 Top 1 | 1.5 月 |
| **A2** | **架构收口**: 抽取公共基础 (error 类型/trace/logging/config), 在 ironclaw/claw-code/admin-backend 间建立清晰边界 | 🔴 | 1 月 |
| **A3** | **客户端基础 E2E 健壮化**: 确保 chat/skill/extension/jobs/routines/models 6 条 IPC 流水线在 MVP 5 skill 下都走通 | 🔴 Top 2 | 1.5 月 |
| **A4** | **国产 LLM 适配**: 叠加 claw-code providers, 配置 3-4 家 (智谱/通义/DeepSeek) + 切换测试 | 🔴 | 0.5 月 |
| A5 | MVP 5 skill 验证 (文件/搜索/网页/代码/知识库) 全部可跑 | 🔴 | 0.5 月 |
| A6 | 两条主线 demo E2E (code review + HR 分析) | 🔴 | 1 月 |
| A7 (原 T3) | 沙箱 facade `dasclaw_sandbox_kernel` | 🟡 | 1.5 月 |
| A8 (原 T1 降级) | DLP 输出侧基础入口 + 现有字典复用 | 🟡 | 0.5 月 |
| A9 (原 T2 降级) | 凭证隔离基础接口 (非完整 host 边界) | 🟡 | 0.8 月 |
| A10 | 缓冲 / POC 标杆对接 | 🟡 | 1 月 |
| **总计** |   |   | **9.8 月** |

**仍超 8 月**。需进一步砍:

### 2.4 收敛到 8 月方案 (最终建议)

| # | 任务 | 必做? | 估算 |
|---|------|-----|------|
| A1 | 架构骨架 + 3 crate + ADR × 3 | 🔴 | 1.5 月 |
| A2 | 架构收口 (错误/日志/配置统一) | 🔴 | 1 月 |
| A3 | 客户端 E2E 健壮化 | 🔴 | 1.5 月 |
| A4 | 国产 LLM 适配 (3 家即可) | 🔴 | 0.3 月 |
| A5 | MVP 5 skill 验证 | 🔴 | 0.3 月 |
| A7 | 沙箱 facade (基础版) | 🔴 | 1.2 月 |
| A6 | 两条 demo E2E | 🔴 | 0.8 月 |
| A8 | DLP 输出侧基础 | 🟡 | 0.5 月 |
| A9 | 凭证隔离基础 | 🟡 | 0.5 月 |
| 缓冲 | POC 标杆对接 + bug | 🟡 | 0.5 月 |
| **总计** |   |   | **8.1 月** ✅ |

---

## §3 新的 MVP 定性

### 3.1 MVP = "架构就绪 + 客户端能跑 + 有安全基线"

不是:
- ❌ "国产政企合规版"
- ❌ "生产级 DLP"
- ❌ "完整多层沙箱"
- ❌ "凭证 host 边界注入严格哲学"

是:
- ✅ "架构骨架建立好,后续 v2-v5 能在此上扩展"
- ✅ "客户端基础能力全部走通 (chat/skill/5 个 MVP skill/国产 LLM)"
- ✅ "两条主线 demo 可演示"
- ✅ "安全基线存在 (DLP 基础/沙箱 facade/凭证隔离基础)"
- ✅ "合规/大客户/国密 明确 v2+ 承诺"

### 3.2 对外交付定位

| 客户类型 | MVP v1 承诺 |
|---------|-----------|
| POC 标杆客户 (1-2 家小规模) | ✅ 完整跑通两条主线 demo |
| 中型客户 (100-500 人) | 🟡 v2 (合规+多租户到位) |
| 大客户 D/E (1000+ 人) | 🟢 v2-v3 (组织树+国密+高可用) |

---

## §4 架构骨架规划 (A1 细化)

由于 A1 是 Top 1,详细展开:

### 4.1 三个 `dasclaw_*` crate

```
crates/
├── dasclaw_sandbox_kernel/      # 沙箱 facade (A7)
│   ├── src/
│   │   ├── lib.rs               # Sandbox trait + Executor
│   │   ├── linux/               # landlock 适配
│   │   ├── macos/               # seatbelt 适配
│   │   └── windows/             # Job Object 适配
│   └── Cargo.toml
│
├── dasclaw_dlp_engine/          # DLP 规则引擎 (A8 基础)
│   ├── src/
│   │   ├── lib.rs               # Rule + Engine + Verdict
│   │   ├── input.rs             # 输入侧 (已有逻辑搬入)
│   │   ├── output.rs            # 输出侧 (新建基础入口)
│   │   └── dictionary.rs        # 字典 (已有逻辑搬入)
│   └── Cargo.toml
│
└── dasclaw_cred_boundary/       # 凭证隔离基础 (A9 基础)
    ├── src/
    │   ├── lib.rs               # CredStore trait + Injector
    │   └── local.rs             # 本地 keychain 实现
    └── Cargo.toml
```

### 4.2 ADR × 3 (A1 配套)

| ADR | 标题 | 内容要点 |
|-----|-----|---------|
| ADR-001 | dasclaw_sandbox_kernel 层定位 | facade vs 新实现 / 三平台抽象 / 与 ironclaw channel 关系 |
| ADR-002 | dasclaw_dlp_engine 双向边界 | 输入 vs 输出 / 与 admin DLP 规则格式互通 / sanitizer vs detector 职责 |
| ADR-003 | dasclaw_cred_boundary 隔离等级 | POC 基础版 vs 严格 host 注入版 / 如何演进 |

### 4.3 Workspace 依赖图

```
admin-backend ────────┐
                       ├──► dasclaw_dlp_engine
desktop-client ───────┤
                       ├──► dasclaw_cred_boundary
                       └──► dasclaw_sandbox_kernel

claw-code/rust ──► (runtime)
ironclaw (channels) ──► 通过 dasclaw_sandbox_kernel 统一
```

---

## §5 给用户的最终确认

### 5.1 方向确认

- [ ] 同意"架构就绪 + 客户端能跑 + 安全基线"的 MVP 定性?
- [ ] 同意把 DLP/凭证 降级为"基础版"(不追求严格哲学)?
- [ ] 同意砍掉 RBAC 强制点 + 合规 Desktop 对接 + Code tools wiring?
- [ ] 同意任务清单 A1-A9 (共 8.1 月)?

### 5.2 任务优先级确认

- [ ] 🔴 A1 架构骨架 + 3 crate + ADR (1.5 月) — 最先开工
- [ ] 🔴 A2 架构收口 (1 月)
- [ ] 🔴 A3 客户端 E2E (1.5 月) — 可与 A1 并行
- [ ] 🔴 A4-A5 LLM + 5 skill (0.6 月)
- [ ] 🔴 A6 Demo E2E (0.8 月)
- [ ] 🔴 A7 沙箱 facade (1.2 月)
- [ ] 🟡 A8-A9 DLP + 凭证 基础 (1 月)
- [ ] 🟡 缓冲 0.5 月

### 5.3 执行入口

你确认后我会:
1. 归并 22/23/24/25/26 的有效结论 + 本 27 号 → 重写 **18-execution-roadmap.md v2.0**
2. 废弃 Route B 22 crate (作为 v2+ 愿景存档到附录)
3. 14 改名 → "14-technical-reference.md"
4. 15 升 v1.2 (锁答案 + 新 MVP 定性)
5. 21 升 v1.1 (新增场景 D 独狼 + AI)
6. 写 3 个 ADR (001-003)
7. 进 **A 代码阶段: A1 架构骨架开工**

---

## 变更历史

- **v0.1 (2026-04-25)**: 用户"架构优先 + 客户端基础 + 合规延后"方向的首版重排。
