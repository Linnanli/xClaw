# [Epic] Fork codex-windows-sandbox into dasclaw monorepo (Windows OS sandbox 实施路径)

> **Status**: 🟡 Issue draft（待人类创建为 GitHub issue 后归档为 `docs/plans/architecture-refactor/issue-XXX-fork-codex-windows-sandbox.md`）
>
> **Type**: epic（多 PR 拆分）
>
> **Blocks**: [#128](https://github.com/Linnanli/xClaw/issues/128) 的 Windows 路径
>
> **Depends on**: [#127](https://github.com/Linnanli/xClaw/issues/127) ADR Accepted 且 D3 = D3-3（方案 X / fork codex-windows-sandbox）
>
> **Cross-cuts**: [#91](https://github.com/Linnanli/xClaw/issues/91)（Windows 计划）
>
> **Source**: [`p0a-sandbox-activation-decision-research-draft.md` 附录 A](./p0a-sandbox-activation-decision-research-draft.md#附录-a--codex-windows-sandbox-实地考察事实修正)

---

## 背景

P0-A 沙箱激活决策（[#127](https://github.com/Linnanli/xClaw/issues/127)）的 D3（Windows 平台行为）经实地考察后修正为 **D3-3：fork upstream `codex-windows-sandbox` 进 monorepo**，原因详见研究草稿附录 A。

upstream 实现位置：[`codex-cli-main/codex-rs/windows-sandbox-rs/`](../../../codex-cli-main/codex-rs/windows-sandbox-rs/)

- License：Apache 2.0（可 fork / vendor）
- 规模：~30 个 `.rs` 模块，估计 5000+ LOC
- 策略：专用本地 Windows 用户隔离 + Job Object + Restricted Token + 按用户 SID Firewall + Win32 alternate desktop + DPAPI + ConPTY + UAC setup helper
- 兼容：Windows 10 1809+ / Server 2019+（受 ConPTY 卡死下限）
- 生产验证：OpenAI Codex CLI 在用

---

## 目标

把 codex-windows-sandbox 整合进 dasclaw monorepo，作为 dasclaw 的 Windows OS sandbox 实施路径，为 [#128](https://github.com/Linnanli/xClaw/issues/128) Windows 平台激活提供执行后端。

## 非目标

- ❌ 本 epic 不修改 macOS Seatbelt / Linux Landlock 路径（沿用现有 `crates/dasclaw_sandbox` 实现）
- ❌ 本 epic 不引入 AppContainer 路线（D3 已选 D3-3 用户隔离路线）
- ❌ 本 epic 不调整 Docker sandbox（[`SandboxModeConfig`](../../../desktop-client/ironclaw/src/config/sandbox.rs) 保持现状）
- ❌ 本 epic 不修改 [#127](https://github.com/Linnanli/xClaw/issues/127) 的 ADR 文本（ADR 由人类签字）

---

## 拆分计划（建议 stacked PR）

### Phase 1 — Vendor & Build（PR-1）

- 把 `codex-cli-main/codex-rs/windows-sandbox-rs/` 的源码（含 build.rs / Cargo.toml / manifest）vendor 进 `crates/dasclaw_sandbox_windows/`
- 添加 `LICENSE-APACHE-2.0` + `NOTICE`，标注 "derived from openai/codex (codex-cli-main, Apache-2.0)"
- workspace `Cargo.toml` 注册新 crate（仅 `cfg(target_os = "windows")` 编译）
- 解决依赖：上游依赖 `codex_protocol` / `codex_utils_pty` / `codex_app_server_protocol` 等
  - 决策：① 同步 fork 必要的上游 utility crate（最小集），或 ② 用 ironclaw 等价替代（如 `dasclaw_pty` 替代 `codex_utils_pty`）
  - 推荐 Phase 1 先选 ①（最小集 fork）保 build 通，Phase 4 再考虑替换以减小重复
- **不**改名、**不**改协议，目标是 `cargo check -p dasclaw_sandbox_windows --target x86_64-pc-windows-msvc` 通过
- 验证命令：CI Windows runner 跑 `cargo check`

### Phase 2 — Brand & Naming（PR-2，base = PR-1）

- 字符串改名：
  - `CodexSandboxUsers` → `DasclawSandboxUsers`
  - `codex-windows-sandbox-setup` → `dasclaw-windows-sandbox-setup`
  - `codex-command-runner` → `dasclaw-command-runner`
  - `codex_home` 路径变量 → `dasclaw_home`（与现有 `desktop-client/ironclaw` 命名空间对齐）
- manifest 标记 / log 文件名 / DPAPI key namespace 等都要改
- 建立"上游字符串映射表"作为后续 cherry-pick 时的 sed 脚本
- 验证：Phase 1 的 `cargo check` 仍通过 + 本地 Win10 VM smoke test（创建用户、删除用户、跑一条 echo 命令）

### Phase 3 — Adapter to dasclaw_sandbox API（PR-3，base = PR-2）

- 在 `crates/dasclaw_sandbox/src/windows/` 加一层 adapter，把 dasclaw 的 `SandboxPolicy` / `SandboxType::WindowsRestrictedToken` enum 映射到 fork 的 `windows_sandbox::SandboxPolicy`
- 把 fork 的 `SandboxType::WindowsRestrictedToken` 实现从 `NotImplemented stage:4` 改为真实调用 fork 的 `run_windows_sandbox_capture` API
- 把 [`OsExecutor::new`](../../../desktop-client/ironclaw/src/sandbox/) 的 Windows 路径接到 fork
- 验证：[`crates/dasclaw_sandbox`](../../../crates/dasclaw_sandbox) 的现有测试在 Windows runner 上通过

### Phase 4 — UAC Setup UX & 政企部署文档（PR-4，base = PR-3）

- desktop-client 启动时检测 `sandbox_setup_is_complete()`：
  - 已 setup → 直接进入正常流程
  - 未 setup → 弹窗引导（非政企常规终端用户场景下走"友好错误"，引导联系 IT 跑 setup.exe）
- 政企部署文档：`desktop-client/docs/windows-sandbox-setup-guide.md`，包含：
  - IT 一次性 setup 步骤（双击 setup.exe / 命令行 silent install）
  - 域控部署脚本（GPO / SCCM / Intune 推送 setup.exe）
  - 卸载流程（清理 `DasclawSandboxUsers` 组、清理 user profile、清理 firewall 规则）
  - 真机矩阵测试报告
- 验证：Win10 1809 / Win10 21H2 / Win11 / Server 2019 真机矩阵 + Defender / 360 / 瑞星 三家 AV smoke test

### Phase 5 — 真机矩阵 CI（PR-5，base = PR-4）

- 评估 GitHub Actions Windows runner 是否支持 elevation（创建用户）
- 如不支持，配置自托管 runner（政企内部 Win10 LTSC 2019 VM）跑 sandbox e2e
- CI 测试覆盖：setup → run sandboxed echo → run sandboxed PowerShell → 卸载

---

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 上游 dependency 链很深（codex_protocol 等） | Phase 1 选最小集 fork，Phase 4 评估替换为 dasclaw 等价 crate |
| 上游 cherry-pick 与本地改名冲突 | Phase 2 建字符串映射表 + sed 脚本自动化 |
| AV 误报（创建本地用户被 AV 拦截） | Phase 4 真机矩阵测试三家 AV，必要时申请代码签名证书 |
| 政企 IT 不愿意做一次性 setup | Phase 4 文档化静默 install + GPO 推送方案 |
| ConPTY 在 Win Server Core 不可用 | 政企客户端目标客户为 Win 客户端 LTSC 2019/2021 + Server 2019/2022 with Desktop Experience，Server Core 不在范围内 |
| Apache-2.0 license attribution 漏标 | Phase 1 + 每个 Phase 都加 license header 检查到 CI（参考现有 `scripts/check_no_panics.py` 模式） |
| upstream codex 后续重大重构（如改成 AppContainer） | 长期由维护者评估是否跟进；fork 版本可独立演进 |

---

## 验收 / DoD

- [ ] `crates/dasclaw_sandbox_windows` crate 编译通过（Windows target）
- [ ] dasclaw `SandboxType::WindowsRestrictedToken` 不再 `NotImplemented`
- [ ] `OsExecutor::new` 在 Windows 上能创建有效执行器
- [ ] 真机矩阵 4 种 Windows 版本（10 1809 / 10 21H2 / 11 / Server 2019）+ 3 家 AV（Defender / 360 / 瑞星）setup + run + uninstall smoke 全过
- [ ] 政企部署指南文档完整
- [ ] NOTICE / LICENSE attribution 完整
- [ ] [#128](https://github.com/Linnanli/xClaw/issues/128) Windows 路径解锁

---

## 参考

- [研究草稿 §附录 A](./p0a-sandbox-activation-decision-research-draft.md#附录-a--codex-windows-sandbox-实地考察事实修正)
- upstream 源码：[`codex-cli-main/codex-rs/windows-sandbox-rs/`](../../../codex-cli-main/codex-rs/windows-sandbox-rs/)
- upstream license：[`codex-cli-main/LICENSE`](../../../codex-cli-main/LICENSE)（Apache 2.0）
- 相关 ADR：[#127](https://github.com/Linnanli/xClaw/issues/127)
- Windows 计划：[#91](https://github.com/Linnanli/xClaw/issues/91)

---

## 标签建议

`epic`、`windows`、`sandbox`、`p0-a`、`blocked-by-adr`、`needs-human-review`
