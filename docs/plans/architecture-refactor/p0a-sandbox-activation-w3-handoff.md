# P0-A 沙箱激活 — W3 收尾与 W4 启动 hand-off

- **Date**: 2026-05-06
- **Status**: ✅ W3 stage A landed
- **决策依据**: [ADR-121](./adr-121-p0a-sandbox-activation-decision.md) v1.1（Accepted, 2026-05-06）

---

## 1. W3（档位 A）已完成

| 项 | 状态 | 引用 |
| --- | --- | --- |
| ADR 落地 | ✅ Accepted | [ADR-121](./adr-121-p0a-sandbox-activation-decision.md) |
| 红线 issue | ✅ Closed (2026-05-06) | [#127](https://github.com/Linnanli/xClaw/issues/127) |
| 实施 issue | ✅ Closed (2026-05-06) | [#128](https://github.com/Linnanli/xClaw/issues/128) |
| 主 PR | ✅ Merged squash `8a13c815` | [#242](https://github.com/Linnanli/xClaw/pull/242) |
| 测试 fixture 跟进 | ✅ Merged | [#243](https://github.com/Linnanli/xClaw/pull/243) → [#244](https://github.com/Linnanli/xClaw/issues/244) |
| 用户文档 | ✅ Landed | [`desktop-client/docs/sandbox-activation.md`](../../../desktop-client/docs/sandbox-activation.md) |

W3 实际交付范围（与 ADR-121 §3 档位 A 对齐）：

- `desktop-client/ironclaw/src/sandbox/{config.rs,net_proxy.rs}`：`ExecutionMode` 枚举（Direct/OsSandbox/Docker）+ env 解析
- `desktop-client/ironclaw/src/config/sandbox.rs`：Builder fallback、政策文件优先级
- `desktop-client/ironclaw/src/{app.rs, tools/{bootstrap.rs, registry.rs}}`：启动期注入与 D6 简化版双开关（已去除 `always_on` 后门）
- D2 友好错误骨架（init 失败提示 / runtime 拒绝消息）

---

## 2. W4 启动前置（待人类）

W4 = 档位 C（纯 Builder + 政策文件加载格式 + 三平台路径全打通），ADR-121 §3.3 已声明**需新 ADR 配套**。启动 W4 前需完成：

1. **新 ADR 草拟**（覆盖档位 C 的 API 形态、政策文件 schema、迁移路径），承接 ADR-121 v1.1。
2. **issue 拆分**：将 W4 拆为可独立 PR 的 stage（如 stage C-1 Builder 重写 / stage C-2 政策文件加载 / stage C-3 三平台收敛）。
3. **Windows 路径解耦**：Windows 隔离走方案 X，由 epic [#241](https://github.com/Linnanli/xClaw/issues/241) 推进，**不阻塞** W4 档位 C 的 macOS/Linux 主线。

## 3. 仍开放的依赖

- [#241](https://github.com/Linnanli/xClaw/issues/241) — Fork codex-windows-sandbox epic（5 Phase / blocked-by-ADR-121 已解除，可启动 Phase 1）
- D2 友好错误子系统的 UX/i18n 完整规范（ADR-121 §3.3 列为后续 ADR 候选）

## 4. 历史归档

- [`p0a-sandbox-activation-decision-research-draft.md`](./p0a-sandbox-activation-decision-research-draft.md) — D0–D7 研究底稿（含 §附录 A codex-windows-sandbox 实地考察）
- [`p0a-sandbox-activation-implementation-plan-draft.md`](./p0a-sandbox-activation-implementation-plan-draft.md) — **SUPERSEDED**，仅供历史溯源；W4 不应据此实施
- [`p0a-sandbox-fork-codex-windows-sandbox-issue-draft.md`](./p0a-sandbox-fork-codex-windows-sandbox-issue-draft.md) — 已发布为 [#241](https://github.com/Linnanli/xClaw/issues/241)
