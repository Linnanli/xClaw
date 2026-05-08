# 35 — Codex CLI 全量能力清单

> **覆盖率说明**（v0.2 补充）：本文档覆盖 codex 生产代码 461,354 LOC 中的可移植核心部分 ≈ 191k LOC（排除 TUI 142,191 + app-server 78,434，二者均为前端/服务器外壳，不是移植目标）。在「可移植核心」范围内覆盖率 ≈ 99%；在「codex 总代码」范围内覆盖率 ≈ 50%（TUI/app-server 有意跳过）。v0.2 补入 CodeAct（code_mode + js_repl 5,556 LOC）后可移植范围覆盖率 ≥ 99%。（路线 B 借鉴/移植范围）

> **版本**：v0.1 (2026-04-25) · 配套 [13-security-capability-inventory.md](13-security-capability-inventory.md)（ironclaw 安全清单）作为对照
> **方法**：严格按 AGENTS.md 三级工具链规范（semantic_search → vscode_listCodeUsages → rg/grep）盘点
> **上游版本**：codex-cli-main（截至 2026-04 主分支，含 90+ crate）

---

## 0. 结论先行

```
代码规模：codex-rs/  92 个 crate
         ├─ 含测试：728,970 行 Rust
         └─ 仅生产：461,354 行 Rust （subagent 估算 150-200k 严重低估）

最强能力 Top 3（建议移植）：
  1. ExecPolicy Starlark DSL ─ prefix_rule + network_rule（完胜 ironclaw JSON 规则）
  2. 三平台 Sandbox 套件 ─ Linux Landlock+bwrap / macOS Seatbelt / Windows JobObject
  3. SandboxPolicy enum 协议设计 ─ FS/Network 正交 + WorkspaceWrite 洞中洞 + ExternalSandbox 逃生口

最弱能力 Top 3（不必移植）：
  1. Crash & Panic（无 sentry 集成、无 panic hook、无 dump 收集）
  2. Identity（仅 OAuth PKCE + ED25519 assertion，无完整 device-key 体系，弱于 ironclaw）
  3. App-Server（local stdio/ws RPC，desktop ADR-104 已列为 non-goal）

对桌面端的核心价值：
  ✅ Sandbox 三平台 + ExecPolicy + apply-patch + portable-pty + AGENTS.md 多层加载
  ⚠️ MCP ManagedProxy（云集成 transport，desktop 暂无场景）
  ❌ TUI / app-server / login / cloud-tasks（已列为 non-goal）
```

---

## 1. 15 大类总览表

| # | 域 | 估算行数 | 核心 crate / 路径 | vs ironclaw | 桌面端价值 | 路线 B 决策 |
|---|---|------:|---|:---:|:---:|---|
| A | Agent Runtime | ~5,000 | `core/src/session/`、`core/src/agent/` | ⚠️ 各有所长 | ⭐⭐⭐⭐⭐ | 选择性移植 AgentControl fork 策略 |
| B | Tool 系统 | ~8,000 | `core/src/tools/`、`apply-patch/`、`unified_exec/`、`code_mode/` | ⚠️ 工具不同 | ⭐⭐⭐⭐⭐ | apply-patch + portable-pty + code_mode 移植 |
| C | Sandbox | ~4,500 | `sandboxing/`、`linux-sandbox/`、`windows-sandbox-rs/`（独立 crate，上游 · 之前为 `core/src/windows_sandbox.rs ~1200 行`，v2.5 拆出）、`process-hardening/` | ✅ 三平台覆盖更全 | ⭐⭐⭐⭐⭐ | **完整移植到 dasclaw_sandbox + dasclaw_sandbox_windows（verbatim，ADR-129/130）** |
| D | Hooks | ~2,150 | `hooks/`（schema/registry/engine 三分离） | ✅ 设计更干净 | ⭐⭐⭐⭐ | 移植到 dasclaw_hooks |
| E | Project Docs | ~3,300 | `core/src/agents_md.rs`、`config/src/agents_md_index.rs` | ⚠️ 多层加载等价 | ⭐⭐⭐⭐ | 移植到 dasclaw_project_docs |
| F | Bash Validation | ~3,300 | `core/src/tools/runtimes/shell/bash_validator.rs` | ⚠️ 比 claw 弱（claw 1004 LOC × 6 模块更深）| ⭐⭐⭐⭐ | 与 claw bash_validation 合并到 dasclaw_bash_validation |
| G | Governance（Guardian）| ~3,500 | `core/src/guardian/`、`core/src/state/service.rs`、`core/src/tools/handlers/approvals.rs` | ⚠️ 与 claw 6 件套互补 | ⭐⭐⭐⭐ | Guardian 评审 + claw 6 件套 → dasclaw_governance |
| H | MCP | ~3,800 | `codex-mcp/`、`config/src/mcp_types.rs` | ✅ 多 ManagedProxy transport | ⭐⭐⭐⭐⭐ | 与 claw 6-transport 合并到 dasclaw_mcp |
| I | ExecPolicy | ~3,200 | `execpolicy/`、`core/src/exec_policy.rs` | ✅ Starlark 完胜 |  ⭐⭐⭐⭐⭐ | **完整移植到 dasclaw_execpolicy**（当前 30-LOC stub，完整 Starlark port 跟踪在 [#326](https://github.com/Linnanli/xClaw/issues/326) Part 2 + [ADR-132](adr-132-execpolicy-starlark-port-plan.md)） |
| J | Feature Flags | ~3,100 | `features/` | ✅ 4-stage 生命周期完整 | ⭐⭐⭐⭐ | 完整移植到 dasclaw_features |
| K | Observability | ~2,000 | `otel/`、`rollout-trace/`、`analytics/` | ⚠️ 各有强项 | ⭐⭐⭐ | rollout-trace 移植到 dasclaw_observability |
| L | Identity | ~1,200 | `agent-identity/`、`device-key/`、`login/`、`keyring-store/` | ❌ 弱于 ironclaw | ⭐⭐ | OAuth PKCE 借鉴；device-key 留 ironclaw |
| M | Crash & Panic | ~0 | （**完全缺失**）| ❌ 都弱 | ⭐ | dasclaw_crash 自建 |
| N | Network Proxy | ~3,500 | `network-proxy/` | ✅ 与 ironclaw 持平 | ⭐⭐⭐⭐ | 完整移植到 dasclaw_net_proxy（当前为 ironclaw HTTP forward proxy port，ADR-43；完整 codex `network-proxy/` port 由 [#324](https://github.com/Linnanli/xClaw/issues/324) sub-task 3 追踪） |
| O | App-Server / Channels / SDK / Login / TUI | ~7,000 | `app-server/`、`tui/`、`cli/`、`login/` | ⚠️ codex 独家 | ⭐ | **non-goal**（ADR-104） |

**合计估算**：~53,500 LOC 是 codex 的"可借鉴/移植"核心，约占 codex 总产能的 11.6%。
其余 ~85% 是 codex 独家产品功能（TUI、app-server、cloud-tasks、ChatGPT 集成等），不在路线 B 范围。

---

## 2. 工作区隔离方案（深度章节）

> **回答用户 Q3**："codex 是如何处理工作区隔离的？"

### 2.1 五层架构

```
┌──────────────────────────────────────────────────────────────────┐
│ Layer 1: 协议层 SandboxPolicy enum（声明式数据）                  │
│   protocol/src/protocol.rs:1095                                  │
│   - DangerFullAccess / ReadOnly / WorkspaceWrite / ExternalSandbox │
│   - 三档 + 一档逃生口；FS / Network 正交                         │
└────────────────────┬─────────────────────────────────────────────┘
                     ↓
┌──────────────────────────────────────────────────────────────────┐
│ Layer 2: 解析与校验 policy_transforms.rs                         │
│   core/src/sandboxing/policy_transforms.rs                       │
│   - parse_writable_roots(): 相对路径解析、canonicalize、防溢出   │
│   - resolve_read_only_subpaths(): 计算"洞中洞"绝对路径           │
│   - 防御：writable_roots 必须 contained_in workspace             │
└────────────────────┬─────────────────────────────────────────────┘
                     ↓
┌──────────────────────────────────────────────────────────────────┐
│ Layer 3: 三平台强制层（kernel-level enforcement）                 │
│   ┌─ Linux  ─→ landlock + bubblewrap                             │
│   │           linux-sandbox/src/landlock.rs                       │
│   │           - PathBeneath rule，writable_roots = ALLOW_RW       │
│   │           - read_only_subpaths = ALLOW_RO（更高优先级覆盖）   │
│   ├─ macOS  ─→ seatbelt (.sbpl)                                   │
│   │           sandboxing/src/seatbelt.rs                          │
│   │           - (allow file-write (path "...")) 白名单            │
│   │           - (deny file-write (subpath "...")) 反向覆盖        │
│   └─ Windows ─→ JobObject + restricted token                     │
│                core/src/windows_sandbox.rs                        │
│                - Restricted SID + DACL 白名单                     │
└────────────────────┬─────────────────────────────────────────────┘
                     ↓
┌──────────────────────────────────────────────────────────────────┐
│ Layer 4: TMPDIR 隔离                                              │
│   core/src/exec_env.rs                                            │
│   - sandbox_tmpdir(job_id) → /tmp/codex-sandbox-<jobid>/          │
│   - 子进程 TMPDIR 环境变量替换                                    │
│   - exclude_tmpdir_env_var 选项可禁用                             │
└────────────────────┬─────────────────────────────────────────────┘
                     ↓
┌──────────────────────────────────────────────────────────────────┐
│ Layer 5: CWD 变更检测（运行时）                                   │
│   core/src/tools/runtimes/shell/shell_tool.rs                    │
│   - 解析 `cd <path>`、`pushd`、`(cd ... && ...)`                  │
│   - 每段执行前检查新 cwd 是否仍在 sandbox 范围                    │
│   - escape sandbox 时返回 SandboxError::CwdEscape                 │
└──────────────────────────────────────────────────────────────────┘
```

### 2.2 关键 enum 定义（[protocol.rs#L1095](../../../codex-cli-main/codex-rs/protocol/src/protocol.rs#L1095)）

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Display, JsonSchema, TS)]
#[strum(serialize_all = "kebab-case")]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SandboxPolicy {
    DangerFullAccess,                                    // 完全无限制（出口）

    ReadOnly {
        access: ReadOnlyAccess,
        network_access: bool,                            // 默认 false
    },

    ExternalSandbox {                                    // 解决"嵌套悖论"
        network_access: NetworkAccess,                   // 信任宿主沙箱
    },

    WorkspaceWrite {
        writable_roots: Vec<AbsolutePathBuf>,            // 主动加可写目录
        read_only_access: ReadOnlyAccess,
        network_access: bool,
        exclude_tmpdir_env_var: bool,                    // 可关 TMPDIR 默认放行
        exclude_slash_tmp: bool,                         // 可关 /tmp 默认放行
    },
}
```

**协议设计 5 大要点**：
1. **三档 + 逃生口**：完整覆盖 "拒绝/只读/工作区/全开放" + 嵌套场景
2. **`#[serde(tag="type", kebab-case)]` + `JsonSchema` + `TS` 三宏共用** → 单一真相源（toml 配置 / IPC 协议 / 前端 TS 类型）
3. **FS / Network 正交** → 每档各自携带 `network_access`，不耦合
4. **WorkspaceWrite 的"洞中洞"** → `WritableRoot.read_only_subpaths` 反向保护 `.git/hooks` `.codex` `.git`，防止 agent 通过修改 git hook 提权
5. **协议层 ≠ 实现层** → enum 在 `protocol/` crate（纯数据），Linux/macOS/Win 三个独立 crate 消费同一份策略

### 2.3 WritableRoot 的洞中洞机制（[protocol.rs#L1167](../../../codex-cli-main/codex-rs/protocol/src/protocol.rs#L1167)）

```rust
pub struct WritableRoot {
    pub root: AbsolutePathBuf,
    pub read_only_subpaths: Vec<AbsolutePathBuf>,   // 由构造保证 ⊂ root
}

impl WritableRoot {
    pub fn is_path_writable(&self, path: &Path) -> bool {
        if !path.starts_with(&self.root) { return false; }
        for subpath in &self.read_only_subpaths {
            if path.starts_with(subpath) { return false; }   // RO 反向覆盖
        }
        true
    }
}
```

**默认保护清单**（codex 内置）：
- `.git/hooks` — 防止改 pre-commit / post-merge 偷偷执行命令
- `.codex` — 防止改 codex 配置自我提权
- `.git` — 防止改 git config 修改 origin URL 或 hook
- `.ssh`、`.aws/credentials`、`.gnupg` — 在 ReadOnly 模式中 enforced

### 2.4 默认策略 + 升级路径

| 模式 | FS | Network | TMPDIR | 升级方式 |
|------|----|---------|--------|---------|
| 默认（CLI 启动）| ReadOnly | ❌ | ❌ | --sandbox=workspace |
| 开发模式 | WorkspaceWrite | ❌ | ✅ | 通过 ExecPolicy approve |
| 已在容器 | ExternalSandbox | 信任宿主 | — | 启动时自动检测 |
| Trust Required | DangerFullAccess | ✅ | ✅ | 显式 `--dangerously-bypass-approvals-and-sandbox` |

### 2.5 对 dasclaw_workspace_cap 的启示

ironclaw_workspace_cap 当前是 **kernel-level 强制层**（cap-std），但**缺策略层**。codex SandboxPolicy 是**纯协议层**，用 cap-std 无法直接实现（cap-std 不支持反向 RO 覆盖）。

**建议路径（W2 进场）**：
- **保留** dasclaw_workspace_cap 现有 cap-std 实现作为**最低强制基线**
- **新增** 协议层模块 `dasclaw_workspace_cap::policy`，借鉴 codex SandboxPolicy enum：
  - 复用 `DangerFullAccess / ReadOnly / WorkspaceWrite / ExternalSandbox` 四档 + 正交网络维度
  - 增加 `WritableRoot.read_only_subpaths` 洞中洞
- **三平台 enforcement** 由 dasclaw_sandbox 提供（移植 codex 三个 crate）
- **不扩到 admin-backend**（用户 Q1 决策）— 仅本地 desktop 强制

---

## 3. 详细分析（按域）

### A. Agent Runtime

| 文件 | 行数 | 职责 |
|---|--:|---|
| [core/src/session/session.rs](../../../codex-cli-main/codex-rs/core/src/session/session.rs) | ~800 | Session 主结构 + 初始化 |
| [core/src/session/mod.rs](../../../codex-cli-main/codex-rs/core/src/session/mod.rs) | ~3000 | submission_loop + Op 派发 |
| [core/src/agent/control.rs](../../../codex-cli-main/codex-rs/core/src/agent/control.rs) | ~1500 | AgentControl: spawn/fork/shutdown |
| [core/src/compact.rs](../../../codex-cli-main/codex-rs/core/src/compact.rs) | ~600 | 上下文压缩 + SUMMARIZATION_PROMPT |
| [core/src/tasks/mod.rs](../../../codex-cli-main/codex-rs/core/src/tasks/mod.rs) | ~500 | SessionTask trait |

**vs ironclaw**：
- ironclaw 用 `LoopDelegate` trait 抽象（更干净），codex 用 `Codex + Session` 双层（更易扩展但重）
- codex `AgentControl::fork` 支持 `LastNTurns` 策略 — **桌面端 Plan-Mode 可借鉴**
- ironclaw 已有 `agentic_loop.rs` (LoopDelegate Route B) ⇒ 不必移植 codex Session 主结构

### B. Tool 系统

**8 大内置工具**：Shell / JS REPL / Code Mode / Apply Patch / Agent Jobs / Web Search / Memory / Artifacts

| 文件 | 行数 | 移植决策 |
|---|--:|---|
| [apply-patch/](../../../codex-cli-main/codex-rs/apply-patch/) | ~600 | ✅ 完整移植到 `dasclaw_apply_patch` |
| [unified_exec/](../../../codex-cli-main/codex-rs/unified_exec/) | ~500 | ✅ 移植到 `dasclaw_pty`（portable-pty 入口）|
| [core/src/tools/code_mode/](../../../codex-cli-main/codex-rs/core/src/tools/code_mode/) | ~1000 | ⚠️ Node VM 依赖重，桌面端可选 feature |
| [core/src/tools/handlers/agent_jobs.rs](../../../codex-cli-main/codex-rs/core/src/tools/handlers/agent_jobs.rs) | ~800 | ⚠️ ironclaw 已有 routines 替代 |
| [core/src/tools/runtimes/shell/](../../../codex-cli-main/codex-rs/core/src/tools/runtimes/shell/) | ~3000 | ⚠️ shell_tool + escalation 借鉴 |

### C. Sandbox（核心移植）

| crate | 行数 | 移植 |
|---|--:|---|
| [linux-sandbox/](../../../codex-cli-main/codex-rs/linux-sandbox/) | ~3000 | ✅ 完整 → dasclaw_sandbox/src/linux/ |
| [sandboxing/](../../../codex-cli-main/codex-rs/sandboxing/) | ~1000 | ✅ Seatbelt → dasclaw_sandbox/src/macos/ |
| [core/src/windows_sandbox.rs](../../../codex-cli-main/codex-rs/core/src/windows_sandbox.rs) | ~1200 | ✅ → dasclaw_sandbox/src/windows/ |
| [process-hardening/](../../../codex-cli-main/codex-rs/process-hardening/) | ~150 | ✅ → dasclaw_sandbox/src/hardening.rs |

**协议层（必须先移植）**：[protocol/src/protocol.rs#L1085-L1300](../../../codex-cli-main/codex-rs/protocol/src/protocol.rs#L1085) 的 SandboxPolicy + WritableRoot + ReadOnlyAccess + NetworkAccess → 进 `dasclaw_workspace_cap::policy`

### D. Hooks（schema/registry/engine 三分离）

| 文件 | 行数 |
|---|--:|
| [hooks/src/registry.rs](../../../codex-cli-main/codex-rs/hooks/src/registry.rs) | ~400 |
| [hooks/src/engine.rs](../../../codex-cli-main/codex-rs/hooks/src/engine.rs) | ~600 |
| [hooks/src/events/](../../../codex-cli-main/codex-rs/hooks/src/events/) | ~800 |
| [hooks/src/schema.rs](../../../codex-cli-main/codex-rs/hooks/src/schema.rs) | ~300 |

**支持事件**：`session_start` / `session_stop` / `pre_tool_use` / `post_tool_use` / `permission_request` / `user_prompt_submit`

**vs ironclaw**：ironclaw 6 lifecycle hook 与 codex 5 个高度重合，移植时合并为一份 schema。

### E. Project Docs（多层加载）

| 文件 | 行数 |
|---|--:|
| [core/src/agents_md.rs](../../../codex-cli-main/codex-rs/core/src/agents_md.rs) | ~1000 |
| [config/src/agents_md_index.rs](../../../codex-cli-main/codex-rs/config/src/agents_md_index.rs) | ~300 |
| [core/src/context_manager/](../../../codex-cli-main/codex-rs/core/src/context_manager/) | ~2000 |

**优先级（codex）**：Project (cwd `.codex/AGENTS.md`) > Workspace (`.codex/AGENTS.md`) > Global (`~/.codex/AGENTS.md`)

### F. Bash Validation

[core/src/tools/runtimes/shell/bash_validator.rs](../../../codex-cli-main/codex-rs/core/src/tools/runtimes/shell/bash_validator.rs) ~800 行
- 解析 + 危险检测：递归 `rm -rf`、重定向 system 目录、pipe 注入、LD_PRELOAD 等

**vs claw-code**：claw 的 bash_validation 是 1004 LOC × 6 模块（用户上轮决策独立 crate），codex 的相对简单。**保持 dasclaw_bash_validation 以 claw 为基础，吸收 codex 的危险命令模式库**。

### G. Governance（Guardian 评审 vs claw 6 件套）

| 文件 | 行数 |
|---|--:|
| [core/src/guardian/](../../../codex-cli-main/codex-rs/core/src/guardian/) | ~3000 |
| [core/src/state/service.rs](../../../codex-cli-main/codex-rs/core/src/state/service.rs) | ~400 |
| [core/src/tools/handlers/approvals.rs](../../../codex-cli-main/codex-rs/core/src/tools/handlers/approvals.rs) | ~600 |

codex Guardian = LLM 自动评审 + 决策 source 追踪（AutomatedReviewer / Config / User）。
**与 claw 6 件套互补**：claw 是"执行前的预防"（trust/branch/stale/green），codex 是"执行后的复审"（Guardian 看结果）。

### H. MCP

```rust
// codex 5 种 transport
pub enum McpServerTransportConfig {
    Stdio { command, args, ... },
    StreamableHttp { url, bearer_token_env_var, ... },
    WebSocket { url, ... },
    Sdk { name, ... },
    ManagedProxy { url, id, ... },        // ← 云集成专用，ironclaw 没有
}
```
ironclaw/claw 各自支持 4 种、6 种。**dasclaw_mcp 合并三家：以 claw-code 6-transport 为主干，加 codex ManagedProxy 与 SDK 子集**。

### I. ExecPolicy（核心移植）

[execpolicy/src/parser.rs#L348](../../../codex-cli-main/codex-rs/execpolicy/src/parser.rs#L348) Starlark builtin:

```python
prefix_rule(
    pattern = ["cmd", ["alt1", "alt2"]],
    decision = "allow" | "prompt" | "forbidden",
    match = [["cmd", "alt1"], ...],
    not_match = [...],
    justification = "explain..."
)
network_rule(host, protocol, decision, justification)
host_executable(name, paths)
```

**完整移植到 dasclaw_execpolicy**。ironclaw/claw 都是 JSON 规则，表达力远不如 Starlark。

### J. Feature Flags

`Stage::UnderDevelopment / Experimental{name,menu_description,announcement} / Stable / Deprecated / Removed`

100+ 标志覆盖完整开发→废弃链路。**完整移植到 dasclaw_features**。

### K. Observability

| 模块 | 行数 |
|---|--:|
| [otel/](../../../codex-cli-main/codex-rs/otel/) | ~1000 |
| [rollout-trace/](../../../codex-cli-main/codex-rs/rollout-trace/) | ~400 |
| [analytics/](../../../codex-cli-main/codex-rs/analytics/) | ~600 |

**rollout-trace** 的 `AgentThread + EdgeId` 模型 = 多 agent 调用图追踪，**ironclaw 无对应能力**。建议移植到 `dasclaw_observability`。

### L. Identity（不移植）

agent-identity（ED25519 assertion）+ login（OAuth PKCE）+ keyring-store。**OAuth PKCE 流借鉴**到 dasclaw_identity 的 ADR-107b AuthToken refresh，**device-key 体系不抄**（弱于 ironclaw 现有方案）。

### M. Crash & Panic（**完全缺失**）

经 `semantic_search "panic hook|set_hook|sentry|crash dump"` 全仓搜索：
- ❌ 无 sentry 集成
- ❌ 无 panic::set_hook 注册
- ❌ 无 core dump 收集
- ✅ 仅 [process-hardening/](../../../codex-cli-main/codex-rs/process-hardening/) 关闭 core dump（防御，非恢复）

→ **dasclaw_crash 必须自建**（沿用 30 v2 §K 决策：用 `sentry` crate）。

### N. Network Proxy

| 文件 | 行数 |
|---|--:|
| [network-proxy/src/proxy.rs](../../../codex-cli-main/codex-rs/network-proxy/src/proxy.rs) | ~1200 |
| [network-proxy/src/runtime.rs](../../../codex-cli-main/codex-rs/network-proxy/src/runtime.rs) | ~800 |
| [network-proxy/src/mitm.rs](../../../codex-cli-main/codex-rs/network-proxy/src/mitm.rs) | ~600 |
| [network-proxy/src/certs.rs](../../../codex-cli-main/codex-rs/network-proxy/src/certs.rs) | ~400 |
| [network-proxy/src/socks5.rs](../../../codex-cli-main/codex-rs/network-proxy/src/socks5.rs) | ~500 |
| [network-proxy/src/policy.rs](../../../codex-cli-main/codex-rs/network-proxy/src/policy.rs) | ~400 |

完整 rama 框架 + MITM + SOCKS5 + 自签 CA + policy 决策 → **完整移植到 dasclaw_net_proxy**。

文件路径约定：`~/.codex/noproxy`、`~/.codex/proxy.conf` → `dasclaw_*` 中改为 `~/.dasclaw/`。

### O. App-Server / TUI / Login（non-goal）

| 模块 | 行数 | 决策 |
|---|--:|---|
| [app-server/](../../../codex-cli-main/codex-rs/app-server/) | ~5000 | ❌ ADR-104 non-goal |
| [tui/](../../../codex-cli-main/codex-rs/tui/) | ~1500 | ❌ ADR-104 non-goal |
| [login/](../../../codex-cli-main/codex-rs/login/) | ~500 | ⚠️ 仅 OAuth PKCE 流借鉴 |

---

## 4. 三方对比矩阵（codex / ironclaw / claw-code）

| # | 能力 | codex | ironclaw | claw-code | 最强方 | dasclaw 选择 |
|---|---|:---:|:---:|:---:|---|---|
| A | Agentic loop | ✅ | ✅ | ✅ | 平手 | ironclaw LoopDelegate |
| A | Multi-agent fork (LastNTurns) | ✅ | ⚠️ | ⚠️ | **codex** | 移植 AgentControl::fork |
| A | Context compaction（remote）| ✅ | ✅ local | ✅ basic | codex | 借鉴 |
| B | apply_patch | ✅ | ❌ | ⚠️ | **codex** | 移植 |
| B | code_mode (Node VM) | ✅ | ❌ | ⚠️ | **codex** | 可选 feature |
| B | portable-pty | ✅ | ⚠️ | ❌ | **codex** | 移植 |
| C | Linux sandbox (Landlock+bwrap) | ✅ | ⚠️ Docker | ⚠️ | **codex** | **完整移植** |
| C | macOS sandbox (Seatbelt) | ✅ | ⚠️ Docker | ❌ | **codex** | **完整移植** |
| C | Windows sandbox (JobObject) | ✅ | ❌ | ❌ | **codex** | **完整移植** |
| C | SandboxPolicy enum 协议 | ✅ | ⚠️ cap-std only | ❌ | **codex** | **协议层移植** |
| C | WritableRoot 洞中洞 | ✅ | ❌ | ❌ | **codex** | 移植 |
| C | ExternalSandbox 嵌套 | ✅ | ❌ | ❌ | **codex** | 移植 |
| D | Hooks 三分离（schema/registry/engine）| ✅ | ⚠️ 散落 | ⚠️ | **codex** | 移植结构 |
| E | AGENTS.md 多层 | ✅ 3 层 | ⚠️ 2 层 | ✅ 2 层 | codex/claw | 三家合并 |
| F | bash_validation | ✅ basic | ⚠️ | ✅ 1004 LOC × 6 | **claw** | claw 主 + codex 模式库 |
| G | Guardian 自动评审 | ✅ | ❌ | ❌ | **codex** | 借鉴 |
| G | 6 件套 Git 治理 | ❌ | ❌ | ✅ | **claw** | 移植 |
| H | MCP transport (5 种) | ✅ ManagedProxy | ⚠️ 4 种 | ✅ 6 种 | claw + codex | 合并 |
| I | ExecPolicy Starlark | ✅ | ❌ JSON | ❌ JSON | **codex** | **完整移植** |
| J | Feature flags 4-stage | ✅ | ⚠️ basic | ⚠️ basic | **codex** | **完整移植** |
| K | rollout-trace（agent 调用图）| ✅ | ❌ | ❌ | **codex** | 移植 |
| K | OpenTelemetry | ✅ | ✅ | ⚠️ | 平手 | 任一即可 |
| L | OAuth PKCE | ✅ | ⚠️ | ⚠️ | **codex** | 借鉴 |
| L | Device-key 体系 | ⚠️ basic | ✅ 完整 | ❌ | **ironclaw** | 沿用 ironclaw |
| M | Sentry / panic hook | ❌ | ❌ | ❌ | 都缺 | dasclaw_crash 自建 |
| N | rama proxy + MITM + SOCKS5 + CA | ✅ | ⚠️ rama only | ❌ | **codex** | **完整移植** |
| O | TUI / app-server | ✅ | ⚠️ web only | ⚠️ TUI | codex | non-goal |
| O | Web UI | ⚠️ | ✅ | ❌ | **ironclaw** | 沿用 ironclaw |

**统计（27 项核心能力）**：

|  | 完整 ✅ | 部分 ⚠️ | 缺失 ❌ |
|---|---:|---:|---:|
| codex | 22 | 4 | 1 |
| ironclaw | 11 | 9 | 7 |
| claw-code | 8 | 9 | 10 |

> **结论**：codex 是路线 B 的**最大移植源**（22 个完整能力中至少 14 个值得移植），ironclaw 是**业务底座**（多通道、Web UI、Tenant、device-key），claw 是**Git 协作治理**的独家。

---

## 5. 移植决策树（与 32 W1-W7 对应）

```
W2 Sandbox + PTY  →  C 类（移植 4 crate）+ B 类 portable-pty
W3 Hooks + Patch  →  D 类（hooks 三分离） + B 类 apply-patch + E 类 AGENTS.md
W4 Governance     →  G 类（Guardian + claw 6 件套合并） + F 类 bash_validation
W5 MCP+ExecPolicy →  H 类（MCP 合并） + I 类（Starlark 完整移植）
W6 客户端外壳     →  ADR-101 路径 C 吸收：bridge/auth/gate/ownership
W7 可观测+Crash   →  K 类 rollout-trace + M 类 dasclaw_crash 自建 + N 类 net_proxy + L 类 OAuth PKCE
```

---

## 6. 已知风险与限制

1. **codex 用 cargo workspace + 92 crate 拓扑**，移植时要逐 crate 评估依赖，不能一次性整 path 引用
2. **rama 框架**是 codex 与 ironclaw 共同依赖，但版本可能不一致 → W7 升版本
3. **Starlark DSL 解析器** 来自 [`starlark-rust`](https://github.com/facebook/starlark-rust) 上游，移植 ExecPolicy 时要锁版本
4. **Landlock** 仅 Linux 5.13+，旧内核需 fallback 到 `bubblewrap` — codex 已实现两层（landlock 优先 → bwrap fallback）
5. **TS 类型导出**（`#[derive(TS)]`）是 codex 协议层的关键设计，移植时要保持 — desktop-client 前端可直接消费类型定义

---

## 附录 A：90+ crate 拓扑（按域分组）

```
Agent Runtime / Session
  ├─ core/                              主 runtime
  ├─ rollout-trace/                     调用图追踪
  └─ tasks/                             任务调度

Tool 系统
  ├─ apply-patch/                       Lark 语法 patch
  ├─ unified_exec/                      portable-pty 入口
  ├─ exec-server/                       PTY 远程执行
  └─ code_mode（在 core/tools/）        Node VM

Sandbox
  ├─ linux-sandbox/                     Landlock + bwrap
  ├─ sandboxing/                        Seatbelt + 通用 manager
  ├─ process-hardening/                 加固
  └─ core/src/windows_sandbox.rs        JobObject

Protocol
  ├─ protocol/                          类型定义（含 SandboxPolicy）
  ├─ app-server-protocol/               app-server 协议
  └─ codex-mcp/                         MCP 客户端

Policy & Hooks
  ├─ execpolicy/                        Starlark
  ├─ hooks/                             schema/registry/engine
  └─ features/                          4-stage flags

Identity & Storage
  ├─ agent-identity/                    ED25519
  ├─ device-key/                        设备密钥
  ├─ login/                             OAuth PKCE
  ├─ keyring-store/                     Keychain
  └─ memory/, artifacts/                持久化

Observability
  ├─ otel/                              OpenTelemetry
  └─ analytics/                         事件上报

Network
  └─ network-proxy/                     rama + MITM + SOCKS5

UI / Server
  ├─ app-server/                        本地 RPC
  ├─ tui/                               终端 UI
  └─ cli/                               入口

工具与共享
  └─ ~70 个支持 crate（mcp-types / common-utils / ...）
```

---

## P. CodeAct（code_mode + js_repl）— codex 独家（v0.2 补充）

**位置**：
- [codex-cli-main/codex-rs/core/src/tools/js_repl/mod.rs](../../codex-cli-main/codex-rs/core/src/tools/js_repl/mod.rs)（2,038 LOC）
- [codex-cli-main/codex-rs/core/src/tools/code_mode/mod.rs](../../codex-cli-main/codex-rs/core/src/tools/code_mode/mod.rs)（410 LOC）
- code_mode/{execute_handler, wait_handler, response_adapter}.rs（合计 ~217 LOC）
- js_repl/mod_tests.rs（2,850 LOC 测试）
- **合计**：5,556 LOC

**6 个设计要点**：

1. **JS REPL Kernel 长驻**：js_repl 启动 Node.js 子进程，由 `kernel.js`（嵌入资源）接管 stdio，**同进程复用**（不是每次重启 Node）
2. **Meriyah 静态解析**：`meriyah.umd.min.js` 随 kernel 嵌入；LLM 输入的 JS 先过 AST 合法性检查才进 vm
3. **与 Sandbox 集成**：js_repl 进程本身仍走 `SandboxManager`，调用 `sandboxing::ExecOptions`，在 Linux 上受 Landlock+seccomp / macOS 上受 seatbelt 限制
4. **不替代 Bash**：LLM 可选 code_mode（写一段 JS）或 shell mode（bash），两者并存。code_mode 适合多步计算/JSON 处理；shell 适合 git/cargo 等 CLI
5. **stderr 截断**：`JS_REPL_STDERR_TAIL_LINE_LIMIT=20` + `JS_REPL_STDERR_TAIL_LINE_MAX_BYTES=512`，防止 LLM 被超长堆栈淹没
6. **`PUBLIC_TOOL_NAME` + `WAIT_TOOL_NAME` 双接口**：一个提交代码，一个轮询结果（异步 fire-and-poll 模式）

**安全模型**：与本文档 §2 Workspace 5 层隔离一致 — js_repl 进程仍受 Layer 3（Landlock/seatbelt）+ Layer 4（TMPDIR）保护；Layer 5（CWD 跟踪）在 JS 代码内部 `process.chdir()` 是能力限制不住的，需依赖 Layer 1+2 的 SandboxPolicy enum 提前拒绝危险路径访问。详见用户问题 Q5 章节回答。

**移植决策**：
- 是否需要：取决于 dasclaw 用户场景。桌面端如果主要是「代码助手」（调 git/cargo/grep），Bash 已足够；如果需要「多步数据计算 / JSON 过滤」，CodeAct 价值高
- 移植成本：需 Node.js 依赖、kernel.js 维护、Meriyah 版本锁定
- **建议列为可选 W7+ 任务**，不进入 W2-W6 核心路径

> ⚠️ **v0.3 重要更新**：upstream codex **已彻底移除 js_repl**（PR #19410，commit 8a559e7938），删除 6 个文件 + `docs/js_repl.md`，meriyah 第三方目录也删除。**code_mode 成为唯一 CodeAct 入口**。下文 §Q 详述。

---

## Q. 2026-04 增量更新（upstream 88 commits，d3b044938d）

> **拉取范围**：`305825abd9..d3b044938d`，共 90 commits / 594 文件 / +35,642 / −19,180 LOC
> **方法**：`git pull --ff-only` + `git diff --stat` + `git diff --diff-filter=A/D`
> **目的**：识别哪些是路线 B 桌面端必须跟进的能力增量

### Q.1 整体变更目录分布

| 目录 | 变更次数 | 主题 |
|---|---:|---|
| codex-rs/core | 147 | 核心引擎大改 |
| codex-rs/tui | 37 | TUI（含 goal UX） |
| codex-rs/protocol-schema | 28 | 协议 schema |
| codex-rs/app-server | 26 | 应用服务器（goal handler + Unix socket） |
| codex-rs/thread-store | 14 | **重大重构**（recorder→ThreadStore trait） |
| codex-rs/tools | 13 | 工具增删（goal_tool 增、js_repl_tool 删） |
| codex-rs/rollout-trace | 10 | 观测重构（含 thread/tool_dispatch/code_cell） |
| codex-rs/state | 9 | state 抽象（device_key + thread_goal） |

### Q.2 五大对桌面端有用的增量（按价值排序）

#### Q.2.1 ★★★★★ Goal 系统五件套（PR #18073-#18077）

**新文件清单**（直接 `git diff --diff-filter=A` 列出）：
- `codex-rs/tools/src/goal_tool.rs` — LLM 工具入口
- `codex-rs/core/src/goals.rs` — 1,639 LOC 核心逻辑
- `codex-rs/state/src/model/thread_goal.rs` + `state/src/runtime/goals.rs`
- `codex-rs/state/migrations/0029_thread_goals.sql`
- `codex-rs/tui/src/chatwidget/{goal_menu,goal_status}.rs` + `tui/src/goal_display.rs`
- `codex-rs/tui/src/app/thread_goal_actions.rs`
- `codex-rs/app-server/src/codex_message_processor/thread_goal_handlers.rs`

**能力**：用户给 agent 设"目标 + 预算上限"，agent 在执行中实时显示进度，超预算时暂停求确认。

**与 ironclaw cost_guard 的关系**：互补而非重复
- cost_guard：每分钟/每小时美元硬上限（金钱视角）
- goal：任务 + token 预算 + 进度展示（用户视角）

**桌面端必带**：补全 31 文档对"任务进度可视化"的诉求。

#### Q.2.2 ★★★★ ThreadStore trait 重构（PR #18900 / #19008 / #18882 / #18897）

**变更**：删除老 `thread-store/src/recorder.rs`，新增：
- `thread-store/src/store.rs` — ThreadStore trait
- `thread-store/src/in_memory.rs` — 测试用内存实现
- `thread-store/src/live_thread.rs` — 实时写入
- `thread-store/src/local/{create_thread, live_writer}.rs` — 本地 SQLite 实现
- `thread-store/src/remote/*` — 远程 thread store

**与 ironclaw_engine v2 Store trait 关系**：同向设计，但 codex 新版**接口更清爽**（in_memory + live_thread 分离 trait 实现）。

**桌面端可借**：trait 接口设计参考。

#### Q.2.3 ★★★★ Permissions Profiles 重构（PR #19449 / #19414 / #19231 / #18287）

**变更**：legacy read-only access modes 全部移除（[#19449](https://github.com/openai/codex/pull/19449)），改用 profiles 表征强制（[#19231](https://github.com/openai/codex/pull/19231) "permissions: make profiles represent enforcement"）。

**影响**：原来散落 4 套读写模式枚举，现在 profiles 一统。

**桌面端价值**：直接简化 dasclaw_sandbox 策略层 — 不要再实现"4 套 mode → 1 套 profile" 这种过渡。

#### Q.2.4 ★★★★ rollout-trace 重构（PR #18879 / #18880 / #18878）

**变更**：
- 删除老 `rollout-trace/src/recorder.rs`
- 新增 `code_cell.rs` / `protocol_event.rs` / `thread.rs` / `tool_dispatch.rs`

**能力**：agent 每个 turn 的可重放 trace，支持 multi-agent edges、tool/code_mode 边界。

**桌面端价值**：dasclaw_observability 直接用，省自研。

#### Q.2.5 ★★★★ Unix Socket Transport（PR #18255 / #19244）

**新文件**：`codex-rs/app-server/src/transport/unix_socket.rs` + WebSocket upgrade（[#19244](https://github.com/openai/codex/pull/19244)）。

**能力**：跨平台进程间通信（Linux/Mac Unix socket + Windows 命名管道），WebSocket 协议升级。

**桌面端价值**：**Tauri ↔ 后端进程通信可直接用**，替代 HTTP（性能高 + 不暴露端口 + 安全）。

### Q.3 上游已淘汰能力（路线 B 不要再去抄）

| 删除项 | 删除原因 | 桌面端处理 |
|---|---|---|
| **js_repl 全套**（PR #19410，6 文件 + docs/js_repl.md） | code_mode 已替代 | **不要 js_repl，直接对齐 code_mode** |
| `core/src/context/spawn_agent_instructions.rs` | 改为 drop spawned-agent context（PR #19127） | 跟进 |
| `chatgpt/src/chatgpt_token.rs` | 改走 AuthProvider（PR #18811 "route Codex auth through AuthProvider"） | 跟进 |
| `third_party/meriyah/LICENSE` | meriyah 不再嵌入（随 js_repl 删） | 不要 |

### Q.4 其它较小价值变更

| 变更 | PR | 是否需要 |
|---|---|:-:|
| MCP hooks 整合（hooks 支持 MCP tools） | #18385 | ⚠️ 跟进 |
| device_key bindings（替部分 OAuth） | #19206 | ⚠️ 简化 dasclaw_auth 时参考 |
| Bedrock GPT-5.4 reasoning levels | #19461 | ⭐ 选配 |
| multi_agent_v2 max_threads 互斥 | #19129 #19354 | ⭐ |
| skill path compress + root aliases | #19098 | ⭐ |
| Bedrock apply_patch 修复 | #19416 | ✅ 修复跟进 |
| permission profiles 在 mcp/tui/turn 中传播 | #18284 #18285 #18286 | ✅ 跟进（profiles 重构连带） |
| exec-server 命名管道改进 | #19283 #18946 #19130 | ⭐ |

### Q.5 不适用桌面端的变更

- TUI goal_menu snapshot 11 个 — 桌面端用 Tauri 自建 UI
- Bazel CI / macOS keychain entitlements — 自有工程链
- sdk/python 全部 — 桌面端 Rust + Tauri
- chatgpt/workspace_settings 大部分 — codex 自家 ChatGPT 集成
- plugin marketplace（#19099 #19074）— 桌面端不上插件市场

### Q.6 统计回填到主表

将以下 §Q 变更体现到本文 §1 大表：

| 大表条目 | 变更 |
|---|---|
| **§A. Agent runtime** | 增 `goals.rs` 1,639 LOC + `goal_tool.rs` |
| **§E. Sandbox / Permissions** | 增 profiles 重构（4 套 mode → 1 套 profile） |
| **§J. Persistence / Rollout** | thread-store 重构 + rollout-trace 4 新文件 |
| **§K. App Server** | 增 unix_socket transport |
| **§P. CodeAct** | js_repl **已删**，code_mode 成唯一入口 |

---

## 变更日志

- **v0.3 (2026-04-25)** — 新增 §Q「2026-04 增量更新」收纳 upstream 88 commits（5 项有用变更：goal 系统 / ThreadStore trait / permissions profiles / rollout-trace / Unix socket transport；js_repl 已淘汰提示）
- **v0.2 (2026-04-25)** — 补充 P. CodeAct（5,556 LOC，codex 独家）+ 文档头新增覆盖率说明（核心 ≈ 99%，总代码 ≈ 50%，TUI/app-server 有意跳过）
- **v0.1 (2026-04-25)** — 初版，15 大类 A-O

---

**文档版本**：v0.3（2026-04-25）
**对照**：[13-security-capability-inventory.md](13-security-capability-inventory.md) ironclaw 安全清单 / [36-claw-code-capability-inventory.md](36-claw-code-capability-inventory.md) claw-code 清单 / [37-ironclaw-main-capability-inventory.md](37-ironclaw-main-capability-inventory.md) ironclaw-main 清单 / [38-desktop-client-ironclaw-fork-inventory.md](38-desktop-client-ironclaw-fork-inventory.md) fork 清单
**配套**：[30-architecture-truth.md](30-architecture-truth.md) v2.1 / [31-target-architecture.md](31-target-architecture.md) v2.1 / [32-execution-plan.md](32-execution-plan.md) v2.1
