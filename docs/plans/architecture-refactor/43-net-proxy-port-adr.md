# 43 — Network Proxy Port ADR (W3.2b)

> 决策：把 codex `network-proxy` 还是 `ironclaw-main/sandbox/proxy/*` port 到 `dasclaw_net_proxy`？
> 状态：**Final（已决议）**
> 上游依赖：W3.2a (PR #17) 删 ironclaw 内嵌 `sandbox/proxy/*` 死代码已完成
> 关联：[42-client-job-runtime-routes.md](./42-client-job-runtime-routes.md)、`crates/dasclaw_net_proxy`

---

## 0. TL;DR

**采纳 Tier B''**：把 `ironclaw-main/src/sandbox/proxy/*` (1361 LOC) **整体复用**到 `crates/dasclaw_net_proxy`，**不**从 codex `network-proxy` port 任何代码。

DLP（深度流量审计）作为**服务器端 admin-backend gateway**未来工作，不在 desktop-client 做。

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

## 4. 决策：Tier B''

### 4.1 复用 ironclaw-main 网络代理

- 把 `ironclaw-main/src/sandbox/proxy/{allowlist,http,mod,policy}.rs` 4 文件 1361 LOC **整体复制**到 `crates/dasclaw_net_proxy/src/`
- 重命名包：`crate::sandbox::proxy::*` → `dasclaw_net_proxy::*`
- 重命名错误类型：`crate::sandbox::error::SandboxError::ProxyError` → `dasclaw_net_proxy::NetProxyError`
- 解耦：移除对 `crate::secrets::CredentialMapping` 的硬编码依赖，改为 trait 注入
- 保留：HTTP forward / CONNECT 隧道 / DomainAllowlist / NetworkPolicyDecider / **凭据注入**（核心政企卖点）

### 4.2 desktop-client 集成

- ironclaw `sandbox/os_executor.rs` 启动 sandboxed 进程时，注入 `HTTPS_PROXY` / `HTTP_PROXY` / `ALL_PROXY` 环境变量指向本地 `dasclaw_net_proxy`
- ReadOnly / WorkspaceWrite 策略下启用代理；FullAccess 直接出网

### 4.3 不做的事

- ❌ MITM / TLS 解密 / 自签 CA：技术上有效，部署上是灾难（CA 私钥泄漏 = 任意站点伪造）
- ❌ SOCKS5 支持：现阶段不需要（git/curl/cargo/npm/pip 全支持 `HTTPS_PROXY`），出现兼容问题再加
- ❌ 动态策略热更新：重启进程即可
- ❌ 透明代理：需 root 权限，不符合企业部署模型
- ❌ 从 codex `network-proxy` port 任何代码：避免重写已有可用实现

---

## 5. 实施分阶段

### W3.2b-1 — 复用 ironclaw-main 4 文件到 dasclaw_net_proxy（约 1400 LOC）
- 复制 4 个 .rs 文件
- 改包名/错误类型/解耦 secrets 模块
- 删 W1 skeleton
- `cargo build` + `cargo nextest` 通过

### W3.2b-2 — desktop-client 集成（约 100 LOC）
- ironclaw `os_executor` 启动 NetProxy 监听 localhost:某端口
- 沙箱进程环境变量注入
- 集成测试：启动代理 → 沙箱内 curl 通过它访问 allow 域名 / 拒非 allow

### W3.2b-3 — 文档与回归测试（约 100 LOC）
- 写 `docs/plans/architecture-refactor/44-net-proxy-port-completion.md` 记录差异
- 端到端 e2e 测试
- 性能基准：对比直连 vs 经代理的延迟

**总量**：约 1600 LOC（其中 1400 是直接复用），3 个 PR。

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

✅ **采纳 Tier B''**：复用 ironclaw-main 现状 (1361 LOC) 到 `dasclaw_net_proxy`，不从 codex port。

DLP / SOCKS5 / 结构化审计 列为未来 TODO，按触发条件增量。
