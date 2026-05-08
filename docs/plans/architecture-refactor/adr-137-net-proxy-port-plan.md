# ADR-137: dasclaw_net_proxy verbatim port plan (research-only)

- **Status**: 🟡 **Decision: pending — proposes hybrid retire + verbatim adoption**（research-only ADR；implementation **out of scope** for this PR）
- **Date**: 2026-05-08
- **Approver**: pending nally sign-off
- **Authors**: GitHub Copilot agent
- **Tracker**: #324 sub-task 3 — dasclaw_net_proxy verbatim port
- **Related**:
  - [ADR-129](adr-129-sandbox-windows-windows-crate-adoption.md) §1.3 — verbatim port red-line
  - [ADR-135](adr-135-sandboxing-crate-adoption-eval.md) §1.4, §6 Q3 — Wave-A 阻塞前置之一
  - [ADR-136](adr-136-protocol-expansion-plan.md) — 姊妹 protocol 切片 ADR
  - codex 上游：`codex-cli-main/codex-rs/network-proxy/`（15 文件，8,876 LOC）
  - dasclaw 现状：[`crates/dasclaw_net_proxy/`](../../../crates/dasclaw_net_proxy/) **divergent impl**（8 文件，1,766 LOC，源自 ironclaw-main `src/sandbox/proxy/*`）
  - 已落地姊妹 verbatim：PR #341 / PR #342 / PR #343 / PR #346

---

## 1. Context

### 1.1 现状 — `dasclaw_net_proxy` **不是** stub，而是 divergent impl

| 维度 | 现行 dasclaw | codex 上游 |
|---|---|---|
| 来源 | ironclaw-main `src/sandbox/proxy/*` | codex-cli-main `codex-rs/network-proxy/` |
| 文件数 | 8 | 15 |
| LOC | 1,766 | 8,876 |
| 文件清单 | allowlist / builder / error / http / lib / policy / reasons / types | certs / config / http_proxy / lib / mitm / mitm_tests / network_policy / policy / proxy / reasons / responses / runtime / socks5 / state / upstream |
| HTTP 实现 | `hyper` 1.5 + `reqwest` 0.12 | `rama-*` 系（=0.3.0-alpha.4，7 个 sub-crate） |
| MITM TLS | 无 | `certs.rs` (344 LOC) + `mitm.rs` (482 LOC) — 自签 CA 拦截 HTTPS |
| SOCKS5 | 无 | `socks5.rs` (616 LOC) |
| Unix socket policy | 无 | `network_policy.rs` (898 LOC) 完整覆盖 |
| Hot reload | 无 | `runtime.rs` (1,776 LOC) `ConfigReloader` |
| Audit metadata | 无 | `state.rs::NetworkProxyAuditMetadata` |
| 公共 API 表面 | 14 类型（`HttpProxy` / `NetworkProxyBuilder` / `ProxyMode` / `CredentialResolver` / `CredentialMapping` 等）| ~50 类型（`NetworkProxy` / `NetworkProxyBuilder` / `NetworkPolicyDecider` / `BlockedRequestObserver` / `ConfigReloader` 等）|

### 1.2 唯一消费者

```
desktop-client/ironclaw/src/sandbox/net_proxy.rs
  use dasclaw_net_proxy::{
      CredentialLocation, CredentialMapping, CredentialResolver,
      HttpProxy, NetworkProxyBuilder, ProxyMode,
  };
```

**1 个 .rs 文件 + 1 处 Cargo.toml 依赖 + 2 处文档注释引用**（`crates/dasclaw_sandbox/src/macos/mod.rs:14` / `desktop-client/ironclaw/src/sandbox/mod.rs:27,33`）。范围极小。

### 1.3 codex transitive 依赖

```
codex-network-proxy
├── codex-utils-absolute-path     ✅ 已 verbatim port → dasclaw_absolute_path (PR #341)
├── codex-utils-home-dir          ❌ 未 port，~50 LOC，行为简单
├── codex-utils-rustls-provider   ❌ 未 port，~30 LOC
├── rama-core / rama-http / rama-http-backend / rama-net /
├── rama-socks5 / rama-tcp / rama-tls-rustls / rama-unix
│      上游 pin =0.3.0-alpha.4（7 个外部 crate）
└── chrono / time / clap / async-trait / globset / url / tracing
```

`rama-*` 系是 codex 选用的 **新一代 HTTP 中间件 stack**（替代 hyper/tower），现行 dasclaw 用的是 `hyper` 1.5。verbatim port 等于换栈。

### 1.4 ADR-135 阻塞依赖

ADR-135 §1.4 标记 sandboxing crate 全 port 阻塞在 sub-task 3：sandboxing 内部 `network` 模块 → `codex-network-proxy::NetworkPolicyDecision` /  `NetworkDecisionSource` 类型直接出现在 `dasclaw_protocol` 期望吸收的 `protocol.rs` 5,238 LOC 中（见 ADR-136 §1.2 transitive deps）。换言之：**ADR-136 step C1.5（protocol.rs 文件级 verbatim slice）也阻塞在 sub-task 3**，除非接受补丁式 stub 类型（违反 ADR-129 §1.3）。

---

## 2. Decision options

### 2A. **Full retire + verbatim replace**（最 ADR-129-friendly）

完全删除现行 1,766 LOC divergent impl，把 codex `network-proxy/` 8,876 LOC 整体 vendor 进 `crates/dasclaw_net_proxy/`，并 verbatim port 2 个 transitive utils（home-dir、rustls-provider）。引入 7 个 `rama-*` workspace deps。

| 项 | 处理 |
|---|---|
| 删除 | 现行 8 文件 1,766 LOC |
| 新增 | 15 文件 8,876 LOC verbatim |
| 新增 crate | `dasclaw_utils_home_dir`（~50 LOC）+ `dasclaw_utils_rustls_provider`（~30 LOC）|
| 工作区新增依赖 | `rama-core` / `rama-http` / `rama-http-backend` / `rama-net` / `rama-socks5` / `rama-tcp` / `rama-tls-rustls` / `rama-unix` 共 7 个外部 crate（pin =0.3.0-alpha.4）|
| 调用方 | `desktop-client/ironclaw/src/sandbox/net_proxy.rs` **必改**：API 表面发生重大变化（`HttpProxy`→`NetworkProxy`、`NetworkProxyBuilder` 签名改、`CredentialResolver` trait 改）|

**优点**：
- 100% 满足 ADR-129 §1.3 verbatim 红线
- 解锁 ADR-135 sandboxing port + ADR-136 step C1.5 protocol.rs 切片
- 获得上游 MITM TLS / SOCKS5 / hot-reload / Unix socket policy / audit metadata 全套能力
- drift guard 单一字节比对即可

**缺点**：
- 极大 PR — 8,876 LOC + 7 个 rama-* alpha 依赖
- `rama-*` 处于 alpha；上游 codex 锁死 `=0.3.0-alpha.4`，本仓 deny.toml / supply chain 审查需要补充
- ironclaw 现行的 secret-resolver 桥接逻辑必须重写（trait 签名改）
- 分支冲突面大 — `dasclaw_protocol` Wave-A（ADR-136）跟本 ADR 在 protocol.rs 与 network_policy.rs 处会重叠

### 2B. **Adapter wrap — 保留现行 1,766 LOC，重命名为 `dasclaw_net_proxy_legacy`，新建 `dasclaw_net_proxy = codex-network-proxy verbatim`，写 adapter 让 ironclaw 继续用 legacy API**

**否决** — 违反 ADR-129 §1.3：保留 divergent impl 即等于"我们一份 codex 一份长期发散"，ADR-129 立法目的就是禁止这种状态。adapter 层属于补丁式代码。

### 2C. **In-place verbatim replace + ironclaw shim**（推荐）

操作上等同 2A 全替换，但**不写 adapter crate**。改为：
1. **Step P0**：先在 `desktop-client/ironclaw/src/sandbox/net_proxy.rs` 中**保留** `IronclawSecretsResolver` 桥接对象（这是 desktop-client 持有 `SecretsStore` 的唯一去处，无法上游化），改写它实现 codex 的新 `CredentialResolver` trait 而非现行的
2. **Step P1**：删现行 `crates/dasclaw_net_proxy/` 8 文件 + 替换为 codex `network-proxy/` 15 文件 verbatim
3. **Step P2**：更新 `crates/dasclaw_net_proxy/Cargo.toml` 用 codex 一致 deps（rama-* 全集 + 2 个 utils）
4. **Step P3**：更新 ironclaw consumer：`HttpProxy`→`NetworkProxy`、`NetworkProxyBuilder` 签名、`CredentialResolver` trait 适配（这是非补丁式重写，因为 trait 边界客观变化）
5. **Step P4**：起 drift guard `scripts/check_codex_net_proxy_drift.py`（13 PAIRS use-block normalization，参照 #342 模式）+ CI roll-up `code-style.yml`

**优点**：
- 与 2A 同样满足 ADR-129 §1.3
- 没有"adapter crate"补丁层
- ironclaw 改动是**真重写**而非补丁——上游 trait 签名变了，调用方必须跟着变，这是预期内的代价
- step P1-P4 可拆为 5 个 PR（按下面 §3.2）

**缺点**：
- 与 2A 同样的依赖蛋糕（rama-* 7 crate + 2 utils crate）
- ironclaw consumer 重写需要 careful test (`tests/cypress/` + ironclaw integration tests)

### 2D. **Defer entire #324 sub-task 3**（最小动作）

不动现行 dasclaw_net_proxy；**ADR-135 sandboxing port 永久阻塞**（直到本 ADR 被解决）；ADR-136 step C1.5 也阻塞 → 合在一起意味着 #324 epic 在 W7+ 才能闭环。

**优点**：诚实声明范围
**缺点**：sandboxing kernel WritableRoot enforcement gap 持续存在；ironclaw 现行 net_proxy 缺 MITM/SOCKS5/Unix socket policy

---

## 3. Recommendation

### 3.1 采纳 **2C — in-place verbatim replace + ironclaw shim**

理由：
- ADR-129 §1.3 强约束——必须 verbatim
- 没有合理的"切片"路径——protocol.rs 5,238 LOC 引用 `codex_network_proxy::NetworkPolicyDecision` 等核心类型，无法 stub 化
- adapter crate 是补丁式代码，被 §2B 否决
- defer 等于阻塞 sandboxing 端到端落地，违反 #324 epic 立项目的

### 3.2 PR 拆分（共 5 个）

| 序号 | 标题 | 主体 | 风险 | 依赖 |
|---|---|---|---|---|
| **PR-N1** | `feat(deps): add rama-* alpha + utils-home-dir + utils-rustls-provider workspace deps` | `Cargo.toml` workspace deps + `deny.toml` 例外 + 2 个新 vendored utils crate（`dasclaw_utils_home_dir` ~50 LOC、`dasclaw_utils_rustls_provider` ~30 LOC，verbatim from `codex-cli-main/codex-rs/utils-*`）+ 各 crate drift guard | 中 — `rama-*` alpha supply-chain 审查 | 无 |
| **PR-N2** | `feat(net-proxy): retire ironclaw-derived impl, prepare for verbatim port` | 删现行 `crates/dasclaw_net_proxy/src/{allowlist,builder,error,http,policy,reasons,types}.rs`；保留 `lib.rs` 临时空壳但标 `#[deprecated]`；ironclaw consumer 暂改用 `pub` 私有 inline 模块（**仅过渡，1 commit 内回滚**） | 高 — break ironclaw build；本 PR 必须与 PR-N3 stack | PR-N1 |
| **PR-N3** | `feat(net-proxy): #324 sub-task 3 — verbatim port codex-network-proxy` | 把 codex 15 文件 verbatim 落到 `crates/dasclaw_net_proxy/src/`；use-path swap `codex_*` → `dasclaw_*`；起 `scripts/check_codex_net_proxy_drift.py`；CI roll-up | 高 — 8,876 LOC + 7 rama-* deps；CI 可能跑挂 | PR-N1 + PR-N2 |
| **PR-N4** | `refactor(ironclaw): adapt to dasclaw_net_proxy v2 API` | 重写 `desktop-client/ironclaw/src/sandbox/net_proxy.rs` 适配 `NetworkProxy` / `NetworkProxyBuilder` / 新 `CredentialResolver` trait；更新 `to_proxy_mappings`；保留 `IronclawSecretsResolver` 但改实现 | 中-高 — ironclaw e2e 测试必跑 | PR-N3 |
| **PR-N5** | `chore(net-proxy): drop transition cruft from PR-N2` | 删 PR-N2 的 `#[deprecated]` 空壳；清理 dead code；docs 更新 | 低 | PR-N4 |

### 3.3 推荐合并次序：N1 → N2+N3（atomic, stack） → N4 → N5

PR-N2 + PR-N3 必须 stack（PR-N2 break build，必须立刻接 N3 修好），考虑直接合并为 **PR-N23**（单 PR 同时删旧文件 + 加 verbatim 新文件 + 改一处 ironclaw use 路径以最小化 break window）— 但 PR 体量将达 8,876 + 1,766 LOC ≈ 10,600 LOC，evaluator 工作量大。本 ADR 倾向 atomic stack 即 **PR-N23**（合并 N2+N3）。

---

## 4. Open questions（留给 reviewer）

1. **Q1 — `rama-*` alpha 依赖审批**：deny.toml 现状是否允许 alpha 版本？需先扫一遍 supply-chain。如阻塞，回退选项是 vendored 静态副本 — 但成本极大。
2. **Q2 — PR-N2/N3 stack 还是合并**：合并为 PR-N23 减少 break window，但单 PR ~10,600 LOC；分开则需 stacked 标记 + reviewer 必须按序。倾向合并。
3. **Q3 — sub-task 3 可否在 sub-task 2（sandboxing）之前先落地**：本 ADR 观点是必须先落地（sandboxing 阻塞在 net_proxy 类型上）。如 reviewer 持反对意见，需调整 ADR-135 wave 划分。
4. **Q4 — ironclaw `IronclawSecretsResolver` API**：codex 新 `CredentialResolver` trait 的具体签名（`async fn resolve(&self, name: &str) -> ?`）尚未在本 ADR 中精确摘录；PR-N4 起步前必须 read codex 上游对应 trait 定义并锁死改写边界。
5. **Q5 — MITM CA 信任链管理**：codex 用自签 CA 拦截 HTTPS，落地后 desktop-client 必须把生成的根 CA 写入操作系统 keychain（macOS/Windows/Linux 各不同）。这是新功能，超出本 ADR 范围，但**不能不提**——需要 ADR-139 单独处理。

---

## 5. Out of scope (本 ADR PR)

❌ **不写一行 Rust 代码** — 本 PR 仅落 ADR-137 文档与决策。

❌ 不开 PR-N1 / PR-N23 / PR-N4 / PR-N5 任何实现 PR。

❌ 不修改 `Cargo.toml` workspace deps、`deny.toml`、任何 crate 源码。

❌ 不写 ADR-139 (MITM CA 信任链管理)。

---

## 6. Validation (本 ADR PR)

```bash
python3.12 scripts/check_no_panics.py --base origin/xClaw   # OK (no .rs changed)
python3   scripts/check_no_new_ironclaw_literal.py --base origin/xClaw   # OK
cargo fmt --all -- --check   # OK (no .rs changed)
```

---

## 7. Decision log

- **2026-05-08**: 起草，提出 2C in-place verbatim + ironclaw shim 方案，PR 拆分为 N1 / N23（合并 N2+N3）/ N4 / N5。等待 nally sign-off 决定 (a) 是否接受 rama-* alpha deps、(b) 是否允许 PR-N23 单 PR ≈10,600 LOC、(c) 是否需要 ADR-139 同步起草。
