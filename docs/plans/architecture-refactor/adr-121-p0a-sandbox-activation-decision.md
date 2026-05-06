# ADR-121: P0-A 沙箱默认激活与 Windows 隔离方案

- **Status**: � **Accepted**（#127 红线已关闭，#128 已随 PR #242 合入 xClaw）
- **Date**: 2026-05-06
- **Approver**: nally
- **Authors**: nally
- **Issue**: [#127](https://github.com/Linnanli/xClaw/issues/127) — `[P0-A] Sandbox activation decision (adr-redline)`
- **Closes**: #127（已人工签字后关闭）
- **Source drafts**:
  - [`p0a-sandbox-activation-decision-research-draft.md`](p0a-sandbox-activation-decision-research-draft.md) — D0–D7 研究底稿（含 §附录 A 实地考察）
  - [`p0a-sandbox-activation-implementation-plan-draft.md`](p0a-sandbox-activation-implementation-plan-draft.md) — #128 实施 hand-off（部分 SUPERSEDED，待本 ADR Accepted 后整体重写）
  - [`p0a-sandbox-fork-codex-windows-sandbox-issue-draft.md`](p0a-sandbox-fork-codex-windows-sandbox-issue-draft.md) — Windows 方案 X (fork codex-windows-sandbox) epic issue 草稿
- **Related**:
  - [ADR-002](adr-002-sandbox-backend-layered-strategy.md) — 沙箱后端分层策略（macOS Seatbelt / Linux Landlock+seccomp / Windows = 本 ADR 决定）
  - [ADR-114](adr-114-dasclaw-rebrand.md) — 历史 ironclaw 命名空间 → dasclaw 命名迁移（本 ADR 不引入新的 ironclaw-prefix 字面量；CI grep guard 会拦）

## 修订记 (Revisions)

- **v1.0 (2026-05-XX)** — 初稿（Proposed）。在 #127 D0–D7 per-decision 评审完成后落地为正式 ADR。
- **v1.1 (2026-05-06)** — Status → Accepted。#128 (W3 档位 A) 随 PR #242 合入 xClaw（squash commit `8a13c815`）；#127 红线已关闭。W4 档位 C 与 Windows 路径 (epic #241) 仍待后续 ADR 覆盖。

---

## 1. Context

### 1.1 问题

`desktop-client/ironclaw` 当前默认运行在**无沙箱**模式，仅靠 `dasclaw_governance` 的 permission 层做策略拦截。这与 P0 安全基线冲突：

- 政企招标硬约束（等保/分级保护合规要求"技术沙箱"，permission-only 不达标）
- 工具调用一旦绕过 permission 层（bug / mis-config / prompt injection），裸跑在用户主进程权限下，可读写整盘
- 三平台策略不对齐：macOS 已接 Seatbelt scaffold、Linux 有 Landlock 模块，Windows 长期空缺

### 1.2 客群定位（决定方案的关键约束）

**Case A — 政企级本地办公助手（90% 场景，本 ADR 默认场景）**：

- 用户期待 agent 无缝读写本地 Word/Excel/PowerPoint/PDF/code/SSH key/JDK/Oracle client/PowerShell 脚本
- 私有云沙箱 / 容器化沙箱 / VM 沙箱**破坏 Case A**（用户文件不在沙箱里）
- 约束：沙箱必须 *本地*、*透明读 workspace*、*白名单写 workspace*、*默认拒绝其他*
- AV/域控/第三方工具的兼容性**优先于** AppContainer 提供的内核级强隔离

Case B（开发者编程助手）/ Case C（研究/数据分析）等场景在本 ADR 中作为次要 cases 处理，沙箱策略可由用户在 settings 中切换更宽松/更严格的预设。

### 1.3 事实底盘（三层验证已完成）

详见 [研究底稿](p0a-sandbox-activation-decision-research-draft.md) §1–§9 与 §附录 A。关键发现：

1. **codex 已有 Windows 沙箱独立实现**（先前认知错误已修正）：`codex-cli-main/codex-rs/windows-sandbox-rs/` 独立 crate，~5000 LOC，~30 个 .rs 模块，Apache-2.0 license
2. **codex 不用 AppContainer**：用专用 Windows 用户隔离（CodexSandboxUsers 组下 offline+online 两用户）+ Job Object + Restricted Token + 按用户 SID Firewall + Win32 alternate desktop + DPAPI 加密 + ConPTY + UAC setup helper
3. **AppContainer 在政企客群有 5 类已知坑**：CreateProcess 慢/失败、AV DLL 注入失败、minifilter ACL bug（含 BSOD 风险）、网络分类失败、LowIL+AC 双重严格冲突 — 这是 codex 刻意避开 AppContainer 的根本原因
4. **Windows 兼容下限 = Win10 1809 (Build 17763)**（ConPTY 要求），政企覆盖 99%+（Win10 LTSC 2019/2021 / Win11 22H2+ / Server 2019/2022）

---

## 2. Decisions（D0–D7 矩阵）

| ID | 主题 | 选择 | 一句话理由 |
|---|---|---|---|
| **D0** | 配置形态 | **C — `ExecutionMode` 枚举** | `enum ExecutionMode { Direct, OsSandbox, Docker }`；`Direct` = dev-only，不暴露给终端用户；替代原 boolean `os_sandbox.enabled` |
| **D1** | 默认开关 | **B — enterprise 强制无降级 fail-CLOSED** | 政企招标硬约束；不允许任何 runtime fallback 到 `Direct` |
| **D2** | 启用粒度 | **E + 友好错误子系统** | 直接全量激活 + 启动失败 / runtime 拒绝走专用 UX 子系统（不是 panic / 不是 silent fallback） |
| **D3** | Windows 后端 | **D3-3 — fork codex-windows-sandbox（方案 X）** | 否决其他 6 个候选；fork Apache-2.0 现成 5000 LOC 代码进 monorepo，比 AppContainer 政企兼容性显著更好 |
| **D4a** | init 失败语义 | **D — 启动 + setup 引导** | Windows 首次部署 100% 命中（必须运行 UAC setup helper 创建沙箱用户），需有友好首次引导 UX |
| **D4b** | runtime 拒绝语义 | **A — 友好错误，不自动 escalate** | 沙箱内 syscall 被拒不应静默升权或 fallback；展示用户可读错误 + 由用户决定提交 approval workflow |
| **D5** | 默认 SandboxPolicy | **E — `WorkspaceWrite` 默认 + 政策文件可覆盖** | `DangerFullAccess` 仍需 `SANDBOX_ALLOW_FULL_ACCESS=true` 双重 opt-in |
| **D6** | proxy 与 sandbox 关系 | **A 简化版（无 always_on 后门）** | proxy 与 sandbox 共享 enable 开关；**去除原矩阵的 `always_on=true` 后门**（曾被用作 fallback 通道，现禁掉） |
| **D7** | 旧行为迁移 | **A — 不迁移直接切换** | 三层验证确认生产代码 0 调用方使用旧的"无沙箱默认"路径，无须保留兼容层 |

> **新旧矩阵差异**：原 8/8 矩阵（D0=B / D1=C / D2=E+C / D3=D Docker / D4=B / D5=WW / D6=A 含后门 / D7=A）已 **SUPERSEDED**。差异点详见研究底稿头部"⭐ 决策最终结果摘要"区块。

### 2.1 Framework 实施节奏（A → C 渐进）

| 阶段 | 范围 | 大致 LOC | 落地 issue |
|---|---|---|---|
| **W3 / 档位 A** | env 变量 + Builder fallback，最小可激活路径 | ~200 LOC | [#128](https://github.com/Linnanli/xClaw/issues/128) |
| **W4 / 档位 C** | 纯 Builder + 政策文件加载 + 三平台路径全打通 | ~1500 LOC | 待新 ADR（不在本 ADR 范围） |
| **方案 X Phase 1–5** | fork codex-windows-sandbox → brand 改名 → adapter → UAC setup UX → 真机矩阵 CI | 详见 [issue draft](p0a-sandbox-fork-codex-windows-sandbox-issue-draft.md) | 新 epic issue（待人类发布） |

---

## 3. Consequences

### 3.1 正面

- 政企招标合规线（"技术沙箱"硬约束）补齐
- 三平台策略对齐（macOS Seatbelt / Linux Landlock+seccomp / Windows fork codex-windows-sandbox）
- 5 类 AppContainer 坑（AV / 域 / minifilter / 网络分类 / LowIL+AC）规避
- 失败语义清晰（D4a init 失败 vs D4b runtime 拒绝）

### 3.2 代价 / 风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| Windows 首次部署 UAC 弹窗 | 政企部署 IT 操作步骤 +1 | 在 changelog / 部署文档明确说明，提供 setup.exe 一次性引导 |
| Linux RHEL 7/8 存量（kernel < 5.13 无 Landlock + bwrap 未预装） | ~50% 命中率 init 失败 | D4a 友好错误 + 部署文档列出 kernel/bwrap 要求 |
| fork codex-windows-sandbox 维护负担 | ~5000 LOC 进 monorepo | Phase 1 先 vendor 不改；Phase 2-5 渐进改名 + adapter |
| Apache-2.0 attribution | 必须保留 license / NOTICE / 修改声明 | Phase 1 vendor 时同步落地 |
| 旧 always_on=true 后门移除 | 任何依赖此后门的脚本/部署会失败 | 三层验证已确认 0 caller，无须迁移期 |

### 3.3 后续 ADR 可能触发

- **W4 档位 C ADR**：纯 Builder + 政策文件加载格式
- **方案 X 各 Phase 子 ADR**（如有重大设计变更）
- 友好错误子系统 UX/i18n 的具体规范（D2 + D4a/D4b）

---

## 4. Implementation Hooks

- [#128](https://github.com/Linnanli/xClaw/issues/128) — W3 档位 A 实施（已随 **PR #242** 合入 xClaw，squash commit `8a13c815`）
- [#241](https://github.com/Linnanli/xClaw/issues/241) — 方案 X epic：fork codex-windows-sandbox（W4 启动）
- 实施草稿 SUPERSEDED 重写：[`p0a-sandbox-activation-implementation-plan-draft.md`](p0a-sandbox-activation-implementation-plan-draft.md) 头部已标注，需在 W4 档位 C ADR 启动前整体重写

---

## 5. Acceptance Checklist（已完成）

- [x] D0–D7 9 项决策与研究底稿 §10 + 头部摘要一致
- [x] 政企客群 Case A 定位与三平台策略对齐
- [x] Windows 方案 X 选型理由已覆盖 6 个否决候选
- [x] 风险矩阵已含 5 项主要风险与缓解
- [x] 实施 issue ([#128](https://github.com/Linnanli/xClaw/issues/128)) 与方案 X epic ([#241](https://github.com/Linnanli/xClaw/issues/241)) 关系已说明
- [x] 旧行为迁移 (D7=A) 三层验证证据已链接到底稿
- [x] 签字人填 Authors / Approver / Date

---

> _本 ADR 已于 2026-05-06 转 Accepted；#127 已关闭，#128 已随 PR #242 合并。_
