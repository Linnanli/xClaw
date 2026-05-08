# ADR-135: codex `sandboxing` crate adoption evaluation (research-only)

- **Status**: 🟡 **Decision: adopt-verbatim-but-blocked** (research-only ADR per [#324](https://github.com/Linnanli/xClaw/issues/324) sub-task 2; implementation is **out of scope** for this PR)
- **Date**: 2026-05-08
- **Approver**: pending nally sign-off
- **Authors**: GitHub Copilot agent
- **Tracker**: [#324](https://github.com/Linnanli/xClaw/issues/324) sub-task 2 — kernel-level WritableRoot 强制（landlock V3 / sbpl / Win ACL）
- **Related**:
  - [ADR-129](adr-129-sandbox-windows-windows-crate-adoption.md) §1.3 — verbatim port red line
  - [ADR-132](adr-132-execpolicy-starlark-port-plan.md) — execpolicy port 计划（同一 verbatim 模式）
  - [ADR-133](adr-133-shell-command-adoption-eval.md) — shell-command 评估（同一研究模式）
  - [ADR-002](adr-002-sandbox-backend-layered-strategy.md) — 多层 sandbox 策略（已落地）
  - [ADR-121](adr-121-p0a-sandbox-activation-decision.md) — 现有 dasclaw_sandbox 激活
  - [doc 32 §W5](32-execution-plan.md) — 落地 wave
  - codex 上游：`codex-cli-main/codex-rs/sandboxing/`（12 文件 + 3 sbpl，5,251 LOC）
  - dasclaw 现状：[`crates/dasclaw_sandbox/`](../../../crates/dasclaw_sandbox/)
  - 已落地姊妹 verbatim：PR #341 `dasclaw_execpolicy` / PR #342 `dasclaw_shell_command` / PR #343 `dasclaw_process_hardening`

---

## 1. Context

### 1.1 上游事实（已三层验证）

`codex-cli-main/codex-rs/sandboxing/` 是 codex 的统一 sandbox 决策中枢，位置: `vscode_listCodeUsages` on `policy_transforms::SandboxPolicyResolver` → `rg "use codex_sandboxing"` → `semantic_search "manager landlock seatbelt"`。

| 文件 | LOC | 角色 |
|---|---|---|
| `lib.rs` | 47 | 出口 |
| `manager.rs` | 313 | **核心** — `SandboxManager` 管理 platform backend 选择 |
| `policy_transforms.rs` | 655 | 把 `SandboxPolicy` 转换成各 backend 的 native 表达 |
| `landlock.rs` | 117 | Linux landlock V3 ruleset 构造（仅当 kernel ≥ 6.7） |
| `bwrap.rs` | 136 | bubblewrap 命令行编排（fallback path on older Linux） |
| `seatbelt.rs` | 723 | macOS sbpl 策略生成（`sandbox-exec`） |
| `seatbelt_base_policy.sbpl` | 122 | 底座 sbpl（read-only / writable-root） |
| `seatbelt_network_policy.sbpl` | 35 | 网络部分 sbpl（与 `codex-network-proxy` 协同） |
| `restricted_read_only_platform_defaults.sbpl` | 199 | macOS readonly 平台默认规则 |
| `*_tests.rs` × 5 | 2,904 | 内联测试（manager / landlock / bwrap / seatbelt / policy_transforms） |

公共 API 真理来源：`codex-cli-main/codex-rs/sandboxing/src/lib.rs:1-47`。

### 1.2 dasclaw 现状（已三层验证）

[`crates/dasclaw_sandbox/`](../../../crates/dasclaw_sandbox/) 已存在多平台 sandbox 实现，但**职责更窄**：

| 模块 | 职责 | 来源 |
|---|---|---|
| `linux/` | seccompiler 系统调用过滤 | claw-code W2.3 port |
| `macos/` | `sandbox-exec` 命令行调用（自家实现） | dasclaw 原生 |
| `windows/` | Windows Job Object 资源限制（CPU/内存）+ launcher | ADR-131 落地 |
| `rlimit.rs` | POSIX `setrlimit` 包装 | claw-code |
| `launcher_ipc.rs` | spawn-time IPC 协议 | dasclaw 原生 |
| `proxy.rs` | network proxy 句柄（占位） | ADR-002 占位 |

**关键差异（已三层验证）**：
- dasclaw_sandbox 的 macOS 路径调用 `sandbox-exec` 但**不自己生成 sbpl**——使用静态硬编码 + 占位
- dasclaw_sandbox 的 Linux 路径**完全没有 landlock**——只有 seccomp（按 ADR §W2.3 决议，把文件系统限制留给 `ironclaw_workspace_cap` 进程内 cap-std）
- dasclaw_sandbox 的 Windows 路径**只有资源限制，没有 ACL DENY**——文件系统强制由 `ironclaw_workspace_cap` 兜底

> 这就是 #324 sub-task 2 的痛点：现行 `WritableRoot` 是**进程内自律式**（cap-std 拦 syscall）而非**内核强制**（kernel 拒绝 syscall）。逃逸窗口存在于"恶意代码绕过 cap-std 直接发 raw syscall"。

### 1.3 codex `sandboxing` crate 的 transitive 依赖图

```
codex-sandboxing
├── codex-network-proxy           ← #324 sub-task 3 NOT YET PORTED（XL effort，独立 ADR）
├── codex-protocol                ← #342 仅切了 ParsedCommand (31 LOC)；本 ADR 需要 16 个新类型
│   ├── permissions::FileSystemSandboxPolicy
│   ├── permissions::NetworkSandboxPolicy
│   ├── permissions::FileSystemPath / FileSystemAccessMode / FileSystemSandboxEntry / FileSystemSandboxKind
│   ├── permissions::FileSystemSpecialPath / ReadDenyMatcher
│   ├── protocol::SandboxPolicy / NetworkAccess
│   ├── models::AdditionalPermissionProfile / FileSystemPermissions / NetworkPermissions
│   ├── config_types::WindowsSandboxLevel
│   └── error::CodexErr
├── codex-utils-absolute-path     ✅ 已落地为 dasclaw_absolute_path（#341 / #342）
├── dunce                         ✅ workspace 已有
├── libc / regex-lite / serde_json / tracing / url / which   ✅ workspace 已有
└── (dev) anyhow / async-trait / pretty_assertions / tempfile / tokio
```

**两个红色依赖**意味着 sub-task 2 不能孤立落地。

### 1.4 ADR-002 与本 ADR 的关系

ADR-002 决议"分层 sandbox 策略"——内核层（landlock/sbpl/Job Object）+ 进程层（cap-std）+ 应用层（policy validation）。当前 dasclaw_sandbox 只完成**进程层 + Windows 应用层资源限制**，**内核层 readonly/writable-root 仍是开口**。

ADR-132 (execpolicy) 解决了"应用层 policy"语言；ADR-133 (shell-command) 解决了"应用层命令解析"；本 ADR 是 ADR-002 三层架构的**最后一层补完**。

---

## 2. Decision options

### 2A. **Verbatim port whole crate**（推荐路径，与 ADR-132 / ADR-133 一致）

把 `codex-cli-main/codex-rs/sandboxing/` 整个 vendor 进 `crates/dasclaw_sandboxing/`，5,251 LOC 字节级 verbatim：

| 项 | 处理 |
|---|---|
| `lib.rs` / `manager.rs` / `policy_transforms.rs` | use-path swap: `codex_protocol` → `dasclaw_protocol`、`codex_network_proxy` → `dasclaw_net_proxy`、`codex_utils_absolute_path` → `dasclaw_absolute_path` |
| `landlock.rs` / `bwrap.rs` | use-path swap |
| `seatbelt.rs` | use-path swap |
| `*.sbpl` (122+35+199 = 356 LOC) | **零改动**（声明式 SBPL，不含 codex_ 标识） |
| `*_tests.rs` (2,904 LOC) | use-path swap + verbatim |
| `Cargo.toml` | 包名 + lib 名 swap，依赖名 swap |

**Drift guard**: `scripts/check_codex_sandboxing_drift.py`（约 12 PAIRS + `_sort_use_blocks` 归一化），与 ADR-132 / ADR-133 同款。

**优点**：
- 与已建立的 verbatim 范式 100% 一致，未来 codex 升级 landlock V3 / sbpl 规则时机械同步
- macOS sbpl 文件不动，避免人工写错 SBPL 引入逃逸
- 测试 2,904 LOC 一次得到（覆盖 manager / landlock / bwrap / seatbelt / policy_transforms 五个模块）

**缺点 / 阻塞**：
- **硬阻塞 #324 sub-task 3**（codex-network-proxy port，effort:XL）——sandboxing 把 network policy 委托给 net-proxy，在 net-proxy 落地前 sandboxing crate 编译不通过
- **硬阻塞 dasclaw_protocol 扩展**——需要新增至少 16 个类型（~1500-2000 LOC slice 自 codex-protocol/src/{permissions,models,protocol,config_types,error}.rs），见 #342 留下的 dasclaw_parsed_command 切片先例
- 与现有 dasclaw_sandbox 共存 → 需要适配器层（dasclaw_sandbox 调 dasclaw_sandboxing 还是反过来？见 §3）

### 2B. **Selective port**（只移植 landlock + seatbelt 模块）

只把 `landlock.rs` (117 LOC) + `seatbelt.rs` (723 LOC) + 三个 `.sbpl` 文件搬过来，policy_transforms / manager / bwrap 用 dasclaw 自家实现包装。

**优点**：避开 net-proxy 阻塞（landlock + seatbelt 不直接依赖 net-proxy，是 manager 层依赖）。

**缺点（致命）**：
- **违反 ADR-129 §1.3 verbatim 红线** — landlock/seatbelt 的 input 类型来自 policy_transforms 转换结果；切断 transforms 等于重写公共 API，drift guard 无法对齐
- 仍然要 port `permissions::FileSystemSandboxPolicy`（policy_transforms 的输入），跟 2A 工作量没省多少
- macOS sbpl 单独搬过来要自己写"如何拼装" → 很容易出 sbpl 安全 bug
- ❌ **拒绝**

### 2C. **Don't port — 在 dasclaw_sandbox 内自家实现 landlock / sbpl / Win ACL**

照 #324 sub-task 2 issue 文本字面要求："新增 dasclaw_sandbox/src/{landlock_v3,sbpl_writable_root,win_acl_deny}.rs"。

**优点**：零依赖、零阻塞、立即落地。

**缺点（致命）**：
- ❌ 违反 AGENTS.md "禁止补丁式代码"——把内核安全代码混进非 verbatim crate，跟 codex 上游漂移即"我们写一份，他们写一份"，长期发散
- ❌ macOS SBPL 是 Apple 私有 DSL，上游 codex 1,279 行（base + network + readonly + seatbelt.rs）已经踩过所有的坑；自家实现 SBPL 意味着自家做 macOS 安全审计
- ❌ landlock V3 ruleset 构造在上游已对齐 kernel 6.7-6.10 的 ABI 变化；自家实现等于再踩一次 kernel ABI 兼容
- ❌ 拒绝

### 2D. **Defer — 暂不补内核层，记入风险登记**

明确把 #324 sub-task 2 推到 W6 或 W7，等 net-proxy 与 protocol 扩展先落地。

**优点**：诚实声明范围；不撒谎说 W5 完成 #324。

**缺点**：
- security-critical 缺口持续存在（cap-std 绕过窗口）
- 但这是**事实**——没有 sub-task 3 + protocol 扩展，sub-task 2 物理上不可能落地

---

## 3. Recommendation

**采纳 2A（verbatim port whole crate），但分三波落地，最终 wave 不是 W5：**

### Wave-A（解阻塞，可并行启动）
1. **PR-A1**: `dasclaw_protocol` 扩展 — 切 codex-protocol 的 `permissions/` + `models/` + `protocol/SandboxPolicy` + `config_types::WindowsSandboxLevel` + `error::CodexErr`，预估 1,500-2,000 LOC verbatim slice + drift guard。模式参照 #342 的 `dasclaw_parsed_command`（极小切片）但范围更大，必要时合并切片成单一 `crates/dasclaw_protocol/`。需要独立 ADR-136。
2. **PR-A2**: `dasclaw_net_proxy` 完整 verbatim port — #324 sub-task 3，effort:XL，需要独立 ADR-137（注意现存 `crates/dasclaw_net_proxy/` 是占位 stub，不是 codex port）。

### Wave-B（执行 sub-task 2 主体）
3. **PR-B1**: `crates/dasclaw_sandboxing/` verbatim port — 5,251 LOC + drift guard，遵循本 ADR 的 use-path swap 矩阵。

### Wave-C（接入 dasclaw 主流水）
4. **PR-C1**: 在 `dasclaw_sandbox::launcher_ipc` 接入 `dasclaw_sandboxing::SandboxManager` —— 把现行进程内 cap-std 调用改为"先调内核 backend，再用 cap-std 兜底"。这是**真正的"内核层强制"接入点**。
5. **PR-C2**: 退役 `dasclaw_sandbox/{linux,macos,windows}/` 中与 sandboxing crate 重叠的部分（保留 `windows/job_object`、`rlimit`、`launcher_ipc`，移除冗余 `sandbox-exec` 调用）。

每个 PR 单独 review，单 issue 一目标，不跨阶段。

### 3.1 use-path swap 矩阵

| 上游 | 本仓库 |
|---|---|
| `codex_protocol::permissions::*` | `dasclaw_protocol::permissions::*` |
| `codex_protocol::protocol::SandboxPolicy / NetworkAccess` | `dasclaw_protocol::protocol::SandboxPolicy / NetworkAccess` |
| `codex_protocol::models::*` | `dasclaw_protocol::models::*` |
| `codex_protocol::config_types::WindowsSandboxLevel` | `dasclaw_protocol::config_types::WindowsSandboxLevel` |
| `codex_protocol::error::CodexErr` | `dasclaw_protocol::error::CodexErr` |
| `codex_network_proxy::*` | `dasclaw_net_proxy::*` |
| `codex_utils_absolute_path::*` | `dasclaw_absolute_path::*` |

### 3.2 .sbpl 文件处理

三个 `.sbpl` 文件**byte-identical** 拷贝（drift guard 简单 SHA256，无需 use-block normalize），原因：
- SBPL 是 Apple Sandbox Profile Language，不含 Rust use 路径
- 上游对宿主路径的引用（`/usr/bin/codex` 等）通过 codex `seatbelt.rs` 的运行时字符串替换注入，**SBPL 模板里没有 codex 字面量**
- 已 grep 确认：`codex-cli-main/codex-rs/sandboxing/src/*.sbpl` 0 处 `codex` 字面（仅 darwin 系统路径 + 通用占位符 `(BWRAP_SOCK_FD)` 等）

### 3.3 与现有 dasclaw_sandbox 的关系

**保留双层**：
- `dasclaw_sandboxing`（**新**）：内核 backend 装配（landlock V3 / sbpl / 调 sandbox-exec）
- `dasclaw_sandbox`（**现有**）：spawn 编排（launcher_ipc）+ 资源限制（job_object / rlimit）+ cap-std 兜底

这与 ADR-002 三层架构兼容（内核层落到 dasclaw_sandboxing；进程层在 dasclaw_sandbox 保留；应用层在 dasclaw_execpolicy / dasclaw_bash_validation）。

---

## 4. Out of scope (本 ADR PR)

❌ **不写一行 Rust 代码**——本 PR 仅落 ADR-135 文档与决策。

❌ 不开 PR-A1 / A2 / B1 / C1 / C2 实现——每个独立 issue + 独立 PR + 独立 ADR（A1 → ADR-136；A2 → ADR-137）。

❌ 不修改任何现有 `dasclaw_sandbox/` 文件。

❌ 不动 `Cargo.toml` workspace members。

---

## 5. Validation (本 ADR PR)

```bash
# 文档规范
markdownlint docs/plans/architecture-refactor/adr-135-*.md   # 可选

# 红线（doc-only PR 全部应 OK 或 skipped）
python3.12 scripts/check_no_panics.py --base origin/xClaw   # OK (no .rs changed)
python3   scripts/check_no_new_ironclaw_literal.py --base origin/xClaw   # OK
cargo fmt --all -- --check   # OK (no .rs changed)
```

无 cargo check / clippy / nextest 跑（无代码改动）。

---

## 6. Open questions（留给 reviewer）

1. **Q1 — `dasclaw_protocol` 切片粒度**：是按 ADR-136 把整个 `codex-protocol/src/` 1:1 verbatim port？还是只 slice "sandboxing 需要的 16 个类型"？前者更 ADR-129-friendly，后者更小。倾向：**完整 port 整个 codex-protocol**（W6 范围；与 #342 的 ParsedCommand 31 LOC 切片合流到同一 crate）。
2. **Q2 — `dasclaw_net_proxy` 现存占位**：[`crates/dasclaw_net_proxy/`](../../../crates/dasclaw_net_proxy/) 当前 LOC 是多少？是 stub 还是已有实现？需要 PR-A2 启动前查清。本 ADR 暂记为"placeholder"。
3. **Q3 — Wave 归属**：本路径明显跨 W5+W6+W7。是否把 #324 sub-task 2 重新 milestone 到 W6 / W7？建议在 PR review 决定。
4. **Q4 — Sub-task 2 issue 文本修订**：issue 写"新增 dasclaw_sandbox/src/{landlock_v3,…}.rs"——本 ADR 推翻这个写法（同 PR #343 process-hardening 的偏差理由）。是否需要 issue body 编辑澄清？建议本 ADR 合并后由 reviewer 直接 update issue body。

---

## 7. Decision log

- **2026-05-08**: 起草，研究 codex sandboxing crate 与 dasclaw 现状对比，提出三波落地路径。等待 nally sign-off。
