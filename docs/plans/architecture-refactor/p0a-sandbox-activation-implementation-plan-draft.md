# P0-A 沙箱激活实现规划 — 研究草稿（#128 hand-off 材料）

> **Status**: 🟡 **Implementation planning draft** — 由 agent 基于 [`p0a-sandbox-activation-decision-research-draft.md`](./p0a-sandbox-activation-decision-research-draft.md) §10 决策矩阵和用户在会话中给出的"全部接受"反馈编写。
>
> ## ⚠️ 决策矩阵已变更（2026-XX-XX 会话）— 本草稿正文 §1-§N 部分已 SUPERSEDED
>
> 本草稿原基于 [`p0a-sandbox-activation-decision-research-draft.md`](./p0a-sandbox-activation-decision-research-draft.md) §10 旧矩阵（D0=B / D1=C / D2=E+C / D3=D Docker / D4=B / D5=WW / D6=A 含后门 / D7=A）撰写。
>
> 用户在新一轮 per-decision MCP gate 评审后给出**新矩阵**（详见研究草稿头部"⭐ 决策最终结果摘要"区块 + §附录 A）：
>
> - **D0 = C（ExecutionMode 枚举）** — 不再用 boolean `os_sandbox.enabled`；用 enum `{ Direct, OsSandbox, Docker }`
> - **D1 = B（enterprise 强制无降级）** — 比原 C 更严
> - **D2 = E + 友好错误子系统** — 直接全量 + 引导 UX
> - **D3 = D3-3 Fork codex-windows-sandbox（方案 X）** — Windows 不走 Docker，走用户隔离 fork（详见 [`p0a-sandbox-fork-codex-windows-sandbox-issue-draft.md`](./p0a-sandbox-fork-codex-windows-sandbox-issue-draft.md)）
> - **D4a = D / D4b = A** — 拆分 init 失败 vs runtime 拒绝两种语义
> - **D5 = E（WorkspaceWrite + 政策可覆盖）** — 与原一致
> - **D6 = A 简化版（去 always_on 后门）**
> - **D7 = A** — 与原一致
>
> **实施者注意**：
> 1. 本草稿 §3-§N（Phase 拆分、API 设计、测试计划）部分基于旧矩阵的 LOC 估算与代码示例需要按新矩阵重写
> 2. 在 [#127](https://github.com/Linnanli/xClaw/issues/127) 由人类落地为正式 ADR 之后，本草稿应被**整体重写或归档**
> 3. Windows 路径不再走 Docker，改 fork codex-windows-sandbox，需协调新 epic（见上述 issue draft）
> 4. Framework 实施按 A→C 渐进：W3（#128）= 档位 A ~200 LOC env+Builder fallback；W4 = 新 ADR 档位 C ~1500 LOC 纯 Builder
>
> ---
>
> **重要前置约束**：
> - ❌ 本文件**不是** [#128](https://github.com/Linnanli/xClaw/issues/128) PR
> - ❌ 本文件**不修改** Rust 代码
> - ❌ 在 [#127](https://github.com/Linnanli/xClaw/issues/127) ADR Status=Accepted 由人类签字之前，**任何 #128 PR 都不应开启**
> - ✅ 本文件仅是给 #128 实施者的拆分清单 + 测试计划 + 风险对账
>
> **Source**:
> - 决策草稿（用户已口头批准 8/8 — **已 SUPERSEDED**，新矩阵见决策草稿头部摘要）：[`p0a-sandbox-activation-decision-research-draft.md`](./p0a-sandbox-activation-decision-research-draft.md)
> - inventory：[`p0a-sandbox-activation-inventory.md`](./p0a-sandbox-activation-inventory.md)
> - **新增 Windows 实施 issue draft**：[`p0a-sandbox-fork-codex-windows-sandbox-issue-draft.md`](./p0a-sandbox-fork-codex-windows-sandbox-issue-draft.md)
> - 父：[#28](https://github.com/Linnanli/xClaw/issues/28)，决策红线：[#127](https://github.com/Linnanli/xClaw/issues/127)，实现：[#128](https://github.com/Linnanli/xClaw/issues/128)

---

## 0. 红线合规说明（**实施者必读**）

[#127](https://github.com/Linnanli/xClaw/issues/127) 带有 `adr-redline` label，按 [`AGENTS.md`](../../../AGENTS.md) §"不要碰的红线"：**agent 不得自动 promote 草稿到 Accepted ADR、不得关闭 #127、不得替人类签字**。

**正确顺序**：

1. 人类基于 [`p0a-sandbox-activation-decision-research-draft.md`](./p0a-sandbox-activation-decision-research-draft.md) 在 `docs/plans/architecture-refactor/adr-XXX-p0a-sandbox-activation-decision.md` 创建 ADR 文件，Status 改 Accepted，签 Authors
2. 人类关闭 [#127](https://github.com/Linnanli/xClaw/issues/127)（或留 PR 引用）
3. **此后**，[#128](https://github.com/Linnanli/xClaw/issues/128) 实施 PR 才能开启
4. #128 PR 按 [`AGENTS.md`](../../../AGENTS.md) §"GitHub PR 工作流" 拆 stacked，按本文件 §6 拆分粒度切片

**若 ADR 实质偏离决策草稿 §10 矩阵，本文件 §3–§7 全部作废，需重写。**

---

## 1. 实施清单概览（按依赖链排序）

| Step | 文件 | 改动类型 | 估计 LOC | 依赖 | TDD 顺序 |
|---|---|---|---|---|---|
| **S1** | [`config/os_sandbox.rs`](../../../desktop-client/ironclaw/src/config/) (NEW) | 新增 struct + resolve | ~120 | 无 | 单元测试先行 |
| **S2** | [`config/mod.rs`](../../../desktop-client/ironclaw/src/config/mod.rs) | 暴露 `os_sandbox: OsSandboxConfig` | ~10 | S1 | 集成测试 |
| **S3** | [`settings.rs`](../../../desktop-client/ironclaw/src/settings.rs)（or wherever Settings live） | 新增 `os_sandbox` 字段 | ~15 | S1 | toml 解析测试 |
| **S4** | [`tools/bootstrap.rs`](../../../desktop-client/ironclaw/src/tools/bootstrap.rs) `BootstrapContext` | 新增 3 个 `Option<...>` 字段 | ~10 | 无 | 编译测试 |
| **S5** | [`tools/registry.rs`](../../../desktop-client/ironclaw/src/tools/registry.rs) `register_dev_tools` | 按 D4-B 拆分 ShellTool 注册路径 | ~50 | S4 | 失败路径单元测试 |
| **S6** | [`app.rs`](../../../desktop-client/ironclaw/src/app.rs) boot 序列 | 在 `init_secrets` 后构造 `OsExecutor` + `start_network_proxy` | ~80 | S2/S3/S4 | 启动时序测试 |
| **S7** | [`enterprise_mode.rs`](../../../desktop-client/ironclaw/src/) (existing or new) | 暴露 `is_enterprise_mode()` 给 `OsSandboxConfig::resolve` | ~5 | 无（可能已存在） | 单元测试 |
| **S8** | E2E 测试 | sandbox-on / sandbox-off / proxy-allowlist 三类 | ~200 | S1–S7 | E2E 先红 |
| **S9** | 文档 [`desktop-client/docs/os-sandbox-activation.md`](../../../desktop-client/docs/) (NEW) | release notes + 用户 opt-in 引导 | ~150 | S1–S6 | n/a |

**总估计**：~640 LOC（不含测试，含测试约 1200 LOC），**建议拆 3–4 个 stacked PR**：

- PR-1：S1 + S2 + S3 + S4（纯配置 + struct，无行为变更）
- PR-2：S5（registry 路径拆分，行为变更）
- PR-3：S6 + S7（boot 序列接入）
- PR-4：S8 + S9（E2E 测试 + 文档）

---

## 2. 决策矩阵到代码的映射（每条决策 → 必需断言）

| 决策 | ADR 决策值（待签字） | 代码 / 测试断言 |
|---|---|---|
| **D0** B 独立命名空间 | `Settings.os_sandbox: OsSandboxConfig` | S1 单元：`OsSandboxConfig` 与 `SandboxModeConfig` 是不同类型 |
| **D1** C enterprise 强制 `true` | `OsSandboxConfig::resolve(settings, is_enterprise)` 在 enterprise=true 时强制 `enabled=true` | S1 失败路径：enterprise 模式 + env `OS_SANDBOX_ENABLED=false` → `enabled=true`（env 被忽略），日志 WARN |
| **D2** E+C rollout | enterprise 强制 + personal default `false` | S1 默认值断言；docs/release-notes（S9） |
| **D3** D Windows 走 Docker | `OsSandboxConfig::resolve` 在 `cfg!(target_os = "windows")` && enterprise 时返回 `Err(ConfigError::WindowsRequiresDockerMode)` | S1 失败路径单元：Windows + enterprise + `os_sandbox.enabled=true` → ConfigError |
| **D4** B 沙箱失败 ShellTool 不注册 | S5 `register_dev_tools` 在 `ctx.os_sandbox_executor.is_none() && is_enterprise` 时跳过 `ShellTool::new()` 注册 + `tracing::error!` | S5 失败路径：mock context, enterprise=true, executor=None → registry 中无 `ShellTool` |
| **D5** `WorkspaceWrite` 默认 | `OsSandboxConfig::default().policy = SandboxPolicy::WorkspaceWrite` | S1 默认值单元 |
| **D6** A 共开关 + `always_on` 后门 | `OsSandboxConfig.network_proxy.always_on: bool` 默认 `false`；S6 boot 时 if `enabled || network_proxy.always_on` → `start_network_proxy` | S6 集成：`enabled=false, always_on=true` → 仅 proxy 启动，无 sandbox |
| **D7** A 不迁移 | 无代码改动；旧 `SANDBOX_ENABLED` 仍只控 Docker | S3 集成：旧 `SANDBOX_ENABLED=true` env → `Settings.sandbox.enabled=true && Settings.os_sandbox.enabled=false` |

---

## 3. `OsSandboxConfig` struct（S1）— 字段草案

> 草案，最终以 ADR 为准。

```rust
// desktop-client/ironclaw/src/config/os_sandbox.rs
use crate::config::helpers::{parse_bool_env, parse_optional_env, parse_string_env};
use crate::error::ConfigError;
use crate::sandbox::SandboxPolicy;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct OsSandboxConfig {
    /// 是否启用 OS 沙箱（macOS Seatbelt / Linux Landlock+seccomp）。
    /// enterprise mode 下被强制 `true`，env 无法覆盖。
    pub enabled: bool,
    /// Sandbox 策略。enterprise mode 下 FullAccess 被强制降级到 WorkspaceWrite。
    pub policy: SandboxPolicy,
    /// 命令执行超时。
    pub timeout: Duration,
    /// 网络允许列表。
    pub network_allowlist: Vec<String>,
    /// HTTP forward proxy 监听端口（0 = 自动）。
    pub proxy_port: u16,
    /// proxy 子配置（含 `always_on` D6 后门）。
    pub network_proxy: NetworkProxySubConfig,
}

#[derive(Debug, Clone)]
pub struct NetworkProxySubConfig {
    /// 即使 `OsSandboxConfig.enabled=false` 也启用 proxy（D6 后门）。
    /// 默认 `false`，第一版不暴露给 UI。
    pub always_on: bool,
}

impl Default for OsSandboxConfig {
    fn default() -> Self {
        Self {
            enabled: false,                              // D1 personal default
            policy: SandboxPolicy::WorkspaceWrite,       // D5
            timeout: Duration::from_secs(120),
            network_allowlist: crate::sandbox::default_allowlist(),
            proxy_port: 0,
            network_proxy: NetworkProxySubConfig { always_on: false }, // D6
        }
    }
}

impl OsSandboxConfig {
    pub(crate) fn resolve(
        settings: &crate::settings::Settings,
        is_enterprise_mode: bool,
    ) -> Result<Self, ConfigError> {
        let env_enabled = parse_bool_env("OS_SANDBOX_ENABLED", false)?;

        // D3: Windows enterprise 必须走 Docker mode
        if is_enterprise_mode && cfg!(target_os = "windows") && (env_enabled || settings.os_sandbox.enabled) {
            return Err(ConfigError::InvalidValue {
                key: "OS_SANDBOX_ENABLED".to_string(),
                message: "Windows enterprise mode must use Docker sandbox; \
                          set SANDBOX_ENABLED=true (Docker mode) instead of \
                          OS_SANDBOX_ENABLED. See ADR-XXX §D3.".to_string(),
            });
        }

        // D1: enterprise 强制启用
        let enabled = if is_enterprise_mode {
            if !env_enabled {
                tracing::warn!(
                    "Enterprise mode forces OS_SANDBOX_ENABLED=true (was false); \
                     env override ignored per ADR-XXX D1."
                );
            }
            true
        } else {
            env_enabled
        };

        let policy_str = parse_string_env("OS_SANDBOX_POLICY", "workspace_write".to_string())?;
        let mut policy = parse_sandbox_policy(&policy_str)?;

        // D5 配套：enterprise mode 下 FullAccess 强制降级
        if is_enterprise_mode && policy == SandboxPolicy::FullAccess {
            tracing::error!(
                "Enterprise mode rejects SandboxPolicy::FullAccess; downgrading to WorkspaceWrite."
            );
            policy = SandboxPolicy::WorkspaceWrite;
        }

        Ok(Self {
            enabled,
            policy,
            timeout: Duration::from_secs(parse_optional_env("OS_SANDBOX_TIMEOUT_SECS", 120)?),
            network_allowlist: settings.os_sandbox.network_allowlist.clone(),
            proxy_port: parse_optional_env("OS_SANDBOX_PROXY_PORT", 0)?,
            network_proxy: NetworkProxySubConfig {
                always_on: parse_bool_env("OS_SANDBOX_PROXY_ALWAYS_ON", false)?,
            },
        })
    }
}
```

> **注**：`is_enterprise_mode: bool` 入参由调用方（[`config/mod.rs`](../../../desktop-client/ironclaw/src/config/mod.rs) 中 `Config::resolve`）传入，参考现有 `SandboxModeConfig::resolve` pattern。

---

## 4. `BootstrapContext` 改动（S4）

```rust
// desktop-client/ironclaw/src/tools/bootstrap.rs
#[derive(Default)]
pub struct BootstrapContext {
    pub mode: BootstrapMode,
    // ... 既有字段 ...

    // 新增（S4）：
    /// OS sandbox executor，None 表示沙箱未启用或启动失败。
    pub os_sandbox_executor: Option<Arc<OsExecutor>>,
    /// Network proxy handle，必须比 executor 长寿命。
    pub network_proxy_handle: Option<Arc<NetworkProxyHandle>>,
    /// 注入到 sandboxed shell 子进程的环境变量（HTTP_PROXY / HTTPS_PROXY / NO_PROXY）。
    pub proxy_env: Option<HashMap<String, String>>,
    /// 是否处于 enterprise mode（用于 D4 fail-closed 判断）。
    pub is_enterprise_mode: bool,
}
```

---

## 5. `register_dev_tools` 拆分（S5，**风险最高**）

```rust
// desktop-client/ironclaw/src/tools/registry.rs
fn register_dev_tools(&self, ctx: &BootstrapContext) {
    // D4-B: 沙箱不可用 + enterprise mode → ShellTool 不注册
    let shell_tool = match (ctx.is_enterprise_mode, &ctx.os_sandbox_executor) {
        (true, None) => {
            tracing::error!(
                target: "x_claw_agent::shell_tool",
                "Enterprise mode + OS sandbox unavailable: ShellTool will NOT be registered. \
                 Users will see this tool as missing per ADR-XXX D4-B."
            );
            None
        }
        (_, Some(executor)) => {
            let mut tool = ShellTool::new()
                .with_sandbox(executor.clone())
                .with_sandbox_policy(/* from config */);
            if let Some(env) = &ctx.proxy_env {
                tool = tool.with_extra_env(env.clone());
            }
            Some(tool)
        }
        (false, None) => {
            // Personal mode + 沙箱关闭 → fall-back direct（保留现有行为）
            Some(ShellTool::new())
        }
    };

    if let Some(tool) = shell_tool {
        self.register_sync(Arc::new(tool));
    }

    self.register_sync(Arc::new(ReadFileTool::new()));
    // ... 其他 dev tools 保持不变 ...
}
```

**风险点**：
- `ShellTool` 缺失会让现有 personal-mode E2E 测试断（如果它们假设 ShellTool 总在）→ **必须先全量跑 `cargo nextest run -p ironclaw`** 找出依赖项
- `register_dev_tools` 签名从 `&self` 变 `&self, ctx: &BootstrapContext`，调用方 [`bootstrap_tools`](../../../desktop-client/ironclaw/src/tools/bootstrap.rs) 必须同步修改

---

## 6. `app.rs` boot 序列（S6）

```rust
// desktop-client/ironclaw/src/app.rs (after init_secrets, before bootstrap_tools)
async fn init_os_sandbox(&mut self) -> Result<(), AppError> {
    let cfg = &self.config.os_sandbox;
    if !cfg.enabled && !cfg.network_proxy.always_on {
        return Ok(()); // 沙箱 + proxy 都关，零开销
    }

    // 启动 proxy（沙箱开启时必须；always_on 时单独启动）
    let mappings = crate::sandbox::default_credential_mappings();
    let secrets_store = self.secrets_store.clone()
        .ok_or(AppError::SecretsStoreUnavailable)?;
    let proxy_handle = match start_network_proxy(
        &cfg.to_sandbox_config(),
        mappings,
        secrets_store,
        self.config.owner_id.clone(),
    ).await {
        Ok(handle) => Arc::new(handle),
        Err(e) => {
            // D4-B 兜底：enterprise mode 下 proxy 启动失败 → 不构造 executor，
            //          register_dev_tools 走"ShellTool 不注册"分支
            if self.is_enterprise_mode() {
                tracing::error!("Enterprise mode: network proxy start failed: {e}; \
                                 ShellTool will be unavailable per ADR-XXX D4-B.");
                return Ok(()); // 不返回 Err，让 app 继续启动但 ShellTool 缺失
            }
            return Err(AppError::NetworkProxy(e));
        }
    };
    self.network_proxy_handle = Some(proxy_handle.clone());

    // 沙箱开启时构造 executor（仅当 enabled，always_on 单独不构造 executor）
    if cfg.enabled {
        let executor = match OsExecutor::new(cfg.timeout, /* allow_full_access */ false) {
            Ok(e) => Arc::new(e),
            Err(e) => {
                if self.is_enterprise_mode() {
                    tracing::error!("Enterprise mode: OsExecutor::new failed: {e}");
                    return Ok(());
                }
                return Err(AppError::Sandbox(e));
            }
        };
        self.os_sandbox_executor = Some(executor);
    }

    Ok(())
}
```

**`BootstrapContext` 装配**：

```rust
let ctx = BootstrapContext {
    // ... 既有字段 ...
    os_sandbox_executor: self.os_sandbox_executor.clone(),
    network_proxy_handle: self.network_proxy_handle.clone(),
    proxy_env: self.network_proxy_handle.as_ref()
        .map(|h| crate::sandbox::net_proxy::proxy_env_vars(h.addr())),
    is_enterprise_mode: self.is_enterprise_mode(),
};
tools.bootstrap_tools(&ctx).await?;
```

---

## 7. 测试矩阵（S8 + S5/S6 内嵌）

按 [`AGENTS.md`](../../../AGENTS.md) §"测试维度矩阵" 全维度覆盖：

### 7.1 单元测试（S1）

| 测试 | 断言 |
|---|---|
| `os_sandbox_default_disabled_personal` | `OsSandboxConfig::default().enabled == false` (D1) |
| `os_sandbox_default_policy_workspace_write` | `OsSandboxConfig::default().policy == WorkspaceWrite` (D5) |
| `os_sandbox_default_proxy_always_off` | `OsSandboxConfig::default().network_proxy.always_on == false` (D6) |
| `req_p0a_d1_enterprise_forces_enabled` | `resolve(settings_with_enabled_false, is_enterprise=true)` → `enabled=true` (D1-C) |
| `req_p0a_d1_enterprise_ignores_env_off` | env `OS_SANDBOX_ENABLED=false` + enterprise=true → `enabled=true` 且日志 WARN |
| `req_p0a_d3_windows_enterprise_rejects` | Windows + enterprise=true + `enabled=true` → `Err(ConfigError::InvalidValue)` |
| `req_p0a_d5_enterprise_downgrades_full_access` | enterprise=true + `policy=FullAccess` → `policy=WorkspaceWrite` 且日志 ERROR |
| `os_sandbox_disabled_proxy_off` | `enabled=false, always_on=false` → 解析成功且字段 false |

### 7.2 失败路径测试（S5）

| 测试 | 断言 |
|---|---|
| `req_p0a_d4b_enterprise_no_executor_skips_shell` | mock `BootstrapContext { is_enterprise_mode: true, os_sandbox_executor: None }` → `ToolRegistry::has("shell") == false` 且日志 ERROR |
| `req_p0a_d4b_personal_no_executor_falls_back` | mock `is_enterprise_mode: false, executor: None` → `ToolRegistry::has("shell") == true` 且 `ShellTool` 走 direct |
| `req_p0a_d4b_executor_present_uses_sandbox` | executor: Some → `ShellTool` 注册时 `with_sandbox` 被调用 |

### 7.3 集成测试（S6）

| 测试 | 断言 |
|---|---|
| `req_p0a_boot_sandbox_enabled_starts_proxy` | `os_sandbox.enabled=true` → boot 后 `network_proxy_handle.is_some()` |
| `req_p0a_boot_proxy_always_on_no_sandbox` | `enabled=false, always_on=true` (D6) → proxy started, executor=None |
| `req_p0a_boot_proxy_failure_enterprise_continues` | mock proxy failure + enterprise=true → app 启动成功但 ShellTool 不可用 |
| `req_p0a_boot_proxy_failure_personal_fails` | mock proxy failure + enterprise=false → app 启动失败 `AppError::NetworkProxy` |

### 7.4 E2E 测试（S8）

| 测试 | 断言 |
|---|---|
| `e2e_sandboxed_shell_blocks_disallowed_domain` | `os_sandbox.enabled=true` + `curl https://evil.com` → 失败 + `NetworkDenyReason` |
| `e2e_sandboxed_shell_allows_allowlisted_domain` | `enabled=true` + `curl https://github.com`（白名单）→ 成功 |
| `e2e_sandboxed_shell_blocks_outside_workspace_write` | `policy=WorkspaceWrite` + `echo > /tmp/foo` → 拒绝（seatbelt/landlock 报错） |
| `e2e_sandboxed_shell_allows_workspace_write` | `policy=WorkspaceWrite` + `echo > $WORKSPACE/foo` → 成功 |
| `e2e_sandbox_disabled_falls_back_personal` | personal + `enabled=false` → ShellTool 注册且走 direct（向后兼容） |

### 7.5 安全审计测试

| 测试 | 断言 |
|---|---|
| `audit_proxy_env_no_bearer_in_logs` | proxy_env 含 `Bearer xyz` → 日志中不出现 `xyz`（grep tracing output） |
| `audit_secrets_store_not_in_error_messages` | `SecretsStoreUnavailable` 错误信息不含 secrets 内容 |

### 7.6 启动时序测试（参考 [`engine_startup_tests.rs`](../../../desktop-client/src/engine_startup_tests.rs)）

| 测试 | 断言 |
|---|---|
| `startup_init_secrets_before_init_os_sandbox` | `init_secrets` 必须在 `init_os_sandbox` 之前完成（否则 `secrets_store=None` panic） |
| `startup_proxy_handle_outlives_executor` | `NetworkProxyHandle` 必须比 `OsExecutor` 长寿命（drop 顺序断言） |

### 7.7 契约测试

| 测试 | 断言 |
|---|---|
| `contract_d6_proxy_env_present_when_proxy_started` | `proxy_handle.is_some()` ↔ `proxy_env.is_some()` 等价（D6 不变量） |
| `contract_d4b_shell_visible_iff_executor_or_personal` | `ToolRegistry::has("shell")` ↔ (`executor.is_some()` ∨ `!is_enterprise_mode`)（D4-B 不变量） |

---

## 8. 与其他 in-flight issue 的协作

| Issue | 影响 | 协作动作 |
|---|---|---|
| [#84](https://github.com/Linnanli/xClaw/issues/84) P0-B tool visibility triple gate | D4-B "ShellTool 不注册" 是 #84 triple-gate 的一种实现 | #84 实施时引用 D4-B 测试，避免重复造轮子 |
| [#85](https://github.com/Linnanli/xClaw/issues/85) P0-D enterprise tool audit | D1-C enterprise 强制启用 → audit log 必须记 sandbox state | #85 schema 新增 `sandbox_state: enabled/disabled/forced` 字段 |
| [#91](https://github.com/Linnanli/xClaw/issues/91) P1 Windows sandbox | D3-D 把 #91 P1 必须项降级为可延后 | #91 issue body 应更新："enterprise Windows 走 Docker，本 issue 仅服务 personal mode" |
| [#92](https://github.com/Linnanli/xClaw/issues/92) P0-F attachment DLP | D6 `always_on` 后门给 #92 留接口 | #92 实施时通过 `OsSandboxConfig.network_proxy.always_on=true` 启用 proxy |
| [#94](https://github.com/Linnanli/xClaw/issues/94) P0-H tool execution surface parity | D4-B 把 4 个 tool surface 的 fail-closed 语义统一 | #94 引用 D4-B 测试 |
| [#95](https://github.com/Linnanli/xClaw/issues/95) P0-I enterprise tenant fail-closed | D1-C / D2-E enterprise 强制启用是 #95 的具体实例 | #95 admin policy schema 增加 `enforce_os_sandbox: bool` |

---

## 9. 风险登记 & 缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| **R1**：`register_dev_tools` 签名变更破坏 0 调用方 | 编译失败级 | 全量 `cargo check -p ironclaw --tests` 跑通 |
| **R2**：`OsExecutor::new` 在 macOS 不同 SDK 版本上行为差异 | E2E 不稳 | E2E 测试加 `#[cfg_attr(target_os = "macos", ignore = "需 SDK ≥ X")]` 标注 |
| **R3**：proxy 端口冲突（多 desktop 实例同时跑） | 启动失败 | `proxy_port: 0` 自动分配，已是默认 |
| **R4**：enterprise 模式 + Windows 用户安装 desktop-client → S1 报错 | 用户体验 | S9 文档明文提示 + installer 检测 + 友好错误信息 |
| **R5**：`secrets_store=None` 时启用沙箱 | 启动 panic | S6 已用 `?` propagate 到 `AppError::SecretsStoreUnavailable` |
| **R6**：`NetworkProxyHandle` drop 顺序错误导致 executor 用 dead proxy | 运行时 hang | 启动时序测试 `startup_proxy_handle_outlives_executor` |
| **R7**：现有 personal-mode E2E 假设 ShellTool 总在 → S5 后断 | 回归 | PR-2 前先跑 `cargo nextest run -p ironclaw -E 'test(shell)'` 列出所有依赖项 |
| **R8**：用户已设 `SANDBOX_ENABLED=false` 期待"无沙箱"，升级后发现 enterprise mode 仍强制启用 | 信任风险 | S9 release notes 必须用粗体强调"enterprise mode 强制覆盖" |

---

## 10. 验收清单（#128 PR 自查）

PR 开启前确保：

- [ ] [#127](https://github.com/Linnanli/xClaw/issues/127) ADR Status=Accepted（人类签字），并在 PR body 引用 ADR 路径
- [ ] 本文件 §2 的 8 条决策→代码映射全部有对应代码 + 测试
- [ ] 本文件 §7 的所有测试族（单元 / 失败 / 集成 / E2E / 安全审计 / 启动时序 / 契约）至少有 1 条存在
- [ ] `cargo nextest run -p ironclaw` 全绿（包括既有测试，特别是 personal-mode shell 路径）
- [ ] `cargo check -p ironclaw --tests` 0 错 0 警
- [ ] `python3.12 scripts/check_no_panics.py --base origin/xClaw` 通过
- [ ] `cargo fmt --all` 通过
- [ ] `cargo clippy --no-deps -p ironclaw --all-targets -- -D warnings` 通过
- [ ] [`AGENTS.md`](../../../AGENTS.md) §"Skills 强制使用规范" 三件套自查（`code-quality-audit` → `code-simplifier` → `code-review-expert`）
- [ ] PR 拆 stacked（按 §1 PR-1/2/3/4），每个 PR 独立 base 注明
- [ ] PR body 含 `Cross-cuts:` 三元声明（ADR-114 红线，本系列 PR 应为 `类A` — 不引入 `.ironclaw` 字面量）
- [ ] PR body 含 `Closes #128` 或分阶段 `Refs #128`（最后一个 PR 才 `Closes`）

---

## 11. 三层验证日志

按 [`AGENTS.md`](../../../AGENTS.md) §"任务启动 4 问"：

1. **新增文件**？✅ 是 → `semantic_search` 验证 `p0a-sandbox-activation-implementation*.md` 不存在
2. **否定性结论**？❌ 否 — 本文件全部基于 inventory + 决策草稿的既有结论
3. **跨项目对账**？❌ 否 — 仅在 desktop-client 内部
4. **架构对账文档**？⚠️ 弱是 — 是实施草稿，三层验证适用于决策草稿（§7 已完成），本文件继承其证据链

---

## 12. 后续会话 hand-off 提示

若本文件被新会话作为输入：

1. **先读** [`p0a-sandbox-activation-decision-research-draft.md`](./p0a-sandbox-activation-decision-research-draft.md)
2. **再读** [`p0a-sandbox-activation-inventory.md`](./p0a-sandbox-activation-inventory.md)
3. **检查** [#127](https://github.com/Linnanli/xClaw/issues/127) 是否已 Accepted（issue 是否 Closed + ADR 文件是否存在）
4. 若已 Accepted **且** ADR 决策与决策草稿 §10 一致 → 可按本文件 §1 逐 PR 实施
5. 若 Accepted **但** ADR 决策与草稿不一致 → **本文件 §3–§7 全部作废**，需重写 implementation plan
6. 若 #127 仍 OPEN → **不得开 #128 PR**，等待人类签字

---
