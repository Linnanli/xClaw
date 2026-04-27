# 43 — Network Proxy Port ADR (W3.2b)

> 决策：把 codex `network-proxy` 还是 `ironclaw-main/sandbox/proxy/*` port 到 `dasclaw_net_proxy`？
> 状态：**Final（已决议）**
> 上游依赖：W3.2a (PR #17) 删 ironclaw 内嵌 `sandbox/proxy/*` 死代码已完成
> 关联：[42-client-job-runtime-routes.md](./42-client-job-runtime-routes.md)、`crates/dasclaw_net_proxy`

---

## 0. TL;DR

**采纳 Tier B''+**：

1. **网络代理**：把 `ironclaw-main/src/sandbox/proxy/*` (1361 LOC) **整体复用**到 `crates/dasclaw_net_proxy`
2. **凭据注入**：复用 `ironclaw-main/src/secrets/*` 现有 KMS 架构（AES-256-GCM + HKDF-SHA256 + OS keychain master key）
3. **reasons 枚举**：增量定义强类型 `DenyReason`（~58 LOC），替换字符串拒绝原因
4. **Windows keychain**：补齐 `secrets/keychain.rs` 的 Windows DPAPI/Credential Manager 实现（~150 LOC）

**不**从 codex `network-proxy` (8876 LOC) port 任何代码（含 MITM/cert）。

DLP（深度流量审计）作为**服务器端 admin-backend gateway**未来工作（§7.1），不在 desktop-client 做。

---

## 1. 上下文

### 1.1 三种实现现状对比

| 实现 | 位置 | LOC | 状态 |
|---|---|---|---|
| **ironclaw-main** 网络代理 | `ironclaw-main/src/sandbox/proxy/` | 1361 | ✅ 上游生产代码，被 `SandboxManager` 使用 |
| **codex** 网络代理 | `codex-cli-main/codex-rs/network-proxy/` | 8876 | ✅ codex 生产代码（含 MITM/cert）|
| **dasclaw_net_proxy** | `crates/dasclaw_net_proxy/` | 24 | ⚠️ W1 占位 skeleton |
| **xClaw 内嵌 proxy** | ~~`desktop-client/ironclaw/src/sandbox/proxy/`~~ | ~~1360~~ | ❌ W3.2a 已删（PR #17）|

### 1.2 关键发现（三级分析得出）

1. xClaw 内嵌 proxy 是从 ironclaw-main 直接复制来的（文件名/行数字节级一致）。W3.2a 删它是因为 W3.1c 删 Docker `SandboxManager` 后，proxy 没人用变成死代码。
2. ironclaw-main 的代理本身就是 **forward-only + CONNECT 隧道**，**不带 MITM**。
3. **凭据注入**（API key 自动注入到 Bearer / X-Api-Key / Query string）是 ironclaw-main 独有的政企级亮点 (`ironclaw-main/src/sandbox/proxy/http.rs:240-330`)。

---

## 2. 政企级真实需求拆解

| 需求 | 必要性 | ironclaw-main | codex network-proxy | 真正需要 MITM？ |
|---|---|---|---|---|
| 合规审计：谁何时访问什么域名 | 🔴 必需 | ✅ | ✅ | 否 |
| 凭据隔离：API key 不暴露给沙箱进程 | 🔴 必需 | ✅★ | ✅ | 否 |
| 域名管控：allowlist | 🔴 必需 | ✅ | ✅ | 否 |
| **DLP（数据防泄漏）** | 🟡 重要 | ❌ | ✅（MITM）| **是** |
| 企业批量部署：1000 台员工电脑一键装 | 🔴 必需 | ✅ 简单 | ❌ CA 链问题 | — |
| 结构化审计日志（进 SIEM）| 🟡 重要 | ⚠️ 仅 tracing | ✅ state.rs | — |

---

## 3. 决策矩阵

| Tier | 范围 | LOC | 安全 | 凭据注入 | DLP | 部署 | 维护 | 总分 |
|---|---|---|---|---|---|---|---|---|
| A. codex full port | 8876 | 5 | ✅ | ✅ | ✅ MITM | 1（CA 灾难）| 2 | 13 |
| B. codex 子集 port | 3500 | 5 | ⚠️ 需重写凭据注入 | ❌ | ✅ | 3 | 17 |
| **B''. ironclaw-main 复用**（推荐）| **1361** | **5** | **✅★** | **❌（DLP 服务器侧）**| **5** | **5** | **20** |
| C. HTTP-only minimal | ~1500 | 4 | ✅ | ❌ | ❌ | 4 | 13 |
| D. 不做 | 0 | 5 | ❌ | ❌ | 1 | 5 | 11 |

---

## 4. 决策：Tier B''+

### 4.1 复用 ironclaw-main 网络代理（1361 LOC）

- 把 `ironclaw-main/src/sandbox/proxy/{allowlist,http,mod,policy}.rs` 4 文件整体复制到 `crates/dasclaw_net_proxy/src/`
- 重命名包：`crate::sandbox::proxy::*` → `dasclaw_net_proxy::*`
- 重命名错误类型：`crate::sandbox::error::SandboxError::ProxyError` → `dasclaw_net_proxy::NetProxyError`
- 解耦：移除对 `crate::secrets::CredentialMapping` 的硬编码依赖，改为 trait 注入
- 保留：HTTP forward / CONNECT 隧道 / DomainAllowlist / NetworkPolicyDecider / 凭据注入接口

### 4.2 凭据注入：复用 ironclaw secrets 模块

**不在 dasclaw_net_proxy 内部实现 keychain**，由 desktop-client 注入实现：

```rust
// dasclaw_net_proxy 仅定义 trait（保持解耦）
pub trait CredentialResolver: Send + Sync {
    async fn resolve(&self, name: &str) -> Option<String>;
}

// desktop-client/ironclaw 实现，桥接现有 secrets 模块
pub struct IronclawSecretsResolver {
    store: Arc<dyn SecretsStore + Send + Sync>,
    user_id: String,
}

impl CredentialResolver for IronclawSecretsResolver {
    async fn resolve(&self, name: &str) -> Option<String> {
        self.store.get_decrypted(&self.user_id, name).await
            .ok()
            .map(|s| s.expose().to_string())
    }
}
```

复用以下 ironclaw 既有能力：
- `secrets/crypto.rs` (373 LOC) — AES-256-GCM + HKDF-SHA256
- `secrets/store.rs` (1269 LOC) — Postgres/libsql/InMemory 三后端 + CAS 消费 + ACL
- `secrets/keychain.rs` (318 LOC) — macOS + Linux master key
- `secrets/types.rs::CredentialLocation` (5 种位置：Bearer/Basic/Header/QueryParam/UrlPath)
- `secrets/types.rs::CredentialMapping` (默认 `optional: false`，安全默认)
- `secrets/types.rs::any_exist` (启动安全门：表非空但 master key 换了 → 启动失败)

### 4.3 reasons 枚举（58 LOC 增量）

替换 `policy.rs` 中的字符串拒绝原因：

```rust
pub enum DenyReason {
    NotInAllowlist { domain: String, allowlist_count: usize },
    PolicyForbidden { policy: SandboxPolicy },
    CredentialMissing { mapping_name: String, host_pattern: String },
    InvalidMethod { method: String, allowed: Vec<String> },
    UpstreamUnreachable { error_kind: ErrorKind },
}
```

收益：审计 SQL `GROUP BY reason_type` / 前端按类型本地化 + 一键修复 / 测试断言精确化。

### 4.4 Windows keychain（150 LOC 增量）

补齐 `desktop-client/ironclaw/src/secrets/keychain.rs` 的 Windows 实现：

```rust
#[cfg(target_os = "windows")]
mod platform {
    use windows::Win32::Security::Credentials::{
        CredDeleteW, CredReadW, CredWriteW, CRED_TYPE_GENERIC, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
    };
    use super::*;

    pub async fn store_master_key(key: &[u8]) -> Result<(), SecretError> {
        // CredWriteW with CRED_TYPE_GENERIC + CRED_PERSIST_LOCAL_MACHINE
        // Data 由 DPAPI 自动加密绑定到当前 user
    }
    pub async fn get_master_key() -> Result<Vec<u8>, SecretError> { /* CredReadW */ }
    pub async fn delete_master_key() -> Result<(), SecretError> { /* CredDeleteW */ }
}
```

依赖：`windows = { version = "0.58", features = ["Win32_Security_Credentials"] }`

**为什么选 windows-rs 而非 keyring-rs**：
- 与现有 macOS (security-framework) / Linux (secret-service) 风格一致（每平台直接调原生 API）
- **不动现有 318 LOC 已稳定代码**（最小风险）
- windows-rs 是微软官方维护，质量与 security-framework 同级
- DPAPI 自动加密 + 绑定 user，比通用 keyring 抽象更精确

### 4.5 不做的事

- ❌ MITM / TLS 解密 / 自签 CA：详见 §1.2 的 8 项风险
- ❌ SOCKS5 支持：现阶段不需要，出现具体兼容问题再加（§7.2 TODO）
- ❌ 动态策略热更新：重启进程即可
- ❌ 透明代理：需 root 权限
- ❌ 从 codex `network-proxy` port 任何代码：避免重写已有可用实现

---

## 5. 实施分阶段

### W3.2b-1 — 复用 ironclaw-main 网络代理到 dasclaw_net_proxy（约 1400 LOC）
- 复制 `sandbox/proxy/` 4 个 .rs 文件
- 改包名/错误类型/解耦 secrets 模块（定义 trait）
- 删 W1 skeleton
- `cargo build` + `cargo nextest` 通过

### W3.2b-2 — reasons 枚举（约 60 LOC）
- 定义 `DenyReason` enum
- 替换 `policy.rs` 中的 `String` 拒绝原因
- 单元测试覆盖每个变体

### W3.2b-3 — Windows keychain 实现（约 150 LOC）
- `desktop-client/ironclaw/src/secrets/keychain.rs` 加 `#[cfg(target_os = "windows")] mod platform`
- 用 windows-rs 调 CredWriteW/CredReadW/CredDeleteW
- Cargo.toml 加 `windows = "0.58"` Windows-only 依赖
- 跨平台测试（macOS/Linux/Windows CI matrix）

### W3.2b-4 — desktop-client 集成（约 150 LOC）
- 实现 `IronclawSecretsResolver` 桥接 `dasclaw_net_proxy::CredentialResolver`
- ironclaw `os_executor` 启动 NetProxy 监听 localhost:某端口
- 沙箱进程环境变量注入 `HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY`
- 集成测试：启动代理 → 沙箱内 curl 通过它访问 allow 域名 / 拒非 allow / 凭据正确注入

### W3.2b-5 — 文档与 e2e（约 100 LOC）
- 写 `docs/plans/architecture-refactor/44-net-proxy-port-completion.md` 记录差异
- 端到端 e2e 测试（agent → 代理 → 真实 GitHub API）
- 性能基准：直连 vs 经代理的延迟

**总量**：约 1860 LOC（其中 1400 直接复用，460 新增），5 个 PR。

---

## 6. 已识别风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| ironclaw-main 4 文件依赖 `crate::secrets`，移植要重新设计抽象 | 高 | 中 | W3.2b-1 显式解耦：定义 `CredentialMapping` 中性 trait，让 desktop-client 注入实现 |
| ironclaw-main 用 hyper 0.x，dasclaw workspace 可能用别版本 | 中 | 中 | 第一步先 `cargo build` 验证依赖能 resolve |
| 缺 SOCKS5 → 某些工具不读 HTTPS_PROXY 出网绕过 | 低 | 中 | 监控审计日志缺口；出现具体工具失败再补 SOCKS5 |
| 缺 DLP body 解密 → 员工敏感数据可能外传 | 中 | 高 | 见 §7 服务器端方案 |

---

## 7. 未来工作（TODO）

### 7.1 [TODO] 服务器端 DLP Gateway（admin-backend）

**问题**：客户端 forward proxy 看不到 TLS body，无法做敏感数据外传检测。

**方案**：在 admin-backend 增加 DLP gateway 模块：

```
desktop agent → [本地 forward proxy: 元数据审计 + allowlist + 凭据注入]
                    │
                    └──► admin-backend DLP gateway → [TLS termination + body 扫描] → 真实外网
                          ↑ 服务器侧 CA（HSM 保护），不下发到客户端
```

**为什么放服务器端**：
- 1 台服务器 vs 1000 台员工电脑，CA 维护点收敛
- CA 私钥在服务器（带 HSM 保护）vs 1000 份私钥分散
- 企业网关产品（Zscaler / Palo Alto Prisma）都是这套架构
- 法规合规：审计日志统一收口

**触发条件**（任一即启动设计）：
- 客户明确要求"员工不能把代码外传"
- 法规要求（金融 PCI-DSS / 医疗 HIPAA / 等保 三级以上）
- 内部安全审计发现 forward-only 不足以满足合规

**前置依赖**：
- admin-backend HTTPS gateway 基础设施（已有？）
- DLP 规则引擎（正则 / 关键字 / 文件指纹 / ML 分类）
- CA 颁发与员工设备 trust store 推送（GPO for Windows / MDM for macOS / kickstart for Linux）

**预估**：W11+ 工作量，需独立 ADR + RFP（含商用方案对比 Zscaler/Symantec DLP 等）。

### 7.2 [TODO] SOCKS5 兜底（按需）

如果生产中观察到工具绕过 forward proxy 出网，再从 codex `network-proxy/socks5.rs` (616 LOC) 增量 port。

### 7.3 [TODO] 结构化审计日志

ironclaw-main 当前只有 tracing 文本日志。如需进 SIEM/Splunk，可增量 port codex `state.rs` (419 LOC) 提供结构化事件。

---

## 8. 决策

✅ **采纳 Tier B''+**：

1. 复用 ironclaw-main `sandbox/proxy/*` (1361 LOC) 到 `dasclaw_net_proxy`
2. 复用 ironclaw-main `secrets/*` 现有 KMS 架构（不重写）
3. 增量加 reasons 枚举（~58 LOC）
4. 增量补 Windows keychain（~150 LOC）

不从 codex network-proxy port 任何代码。

DLP / SOCKS5 / 结构化审计 列为未来 TODO（§7），按触发条件增量。
