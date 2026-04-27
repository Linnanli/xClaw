# 43 — Network Proxy Port ADR (W3.2b)

> 决策：把 codex `network-proxy` (8876 LOC) 中的哪些部分 port 到 `dasclaw_net_proxy`？
> 状态：**Draft，待 review**
> 上游依赖：W3.2a (PR #17) 删 `sandbox/proxy/*` 死代码已完成
> 关联：[42-client-job-runtime-routes.md](./42-client-job-runtime-routes.md)、`crates/dasclaw_net_proxy`

---

## 0. TL;DR

**推荐路线 = Tier B（Audit-only HTTP/SOCKS5 forward proxy + 静态 allowlist）**：从 codex 取 ~3500 LOC，**不做** MITM 解密 / 不做证书自签 / 不做动态策略热更新。

| 组件 | codex LOC | port? | 理由 |
|---|---|---|---|
| `lib.rs` (公共导出) | 62 | ✅ 重写 | 适配 dasclaw 命名 |
| `config.rs` (env/CLI 解析) | 869 | ⚠️ 子集 ~300 | 只保留代理监听/上游/allowlist；不要 MITM/cert 配置 |
| `http_proxy.rs` (HTTP CONNECT 转发) | 1319 | ✅ 全 | 核心 |
| `socks5.rs` (SOCKS5 转发) | 616 | ✅ 全 | 让无 HTTP_PROXY 支持的工具也能走代理 |
| `network_policy.rs` (allowlist 决策) | 898 | ⚠️ 子集 ~400 | 只保留静态 allowlist + audit log；删 dynamic policy update |
| `policy.rs` (policy 类型/解析) | 465 | ⚠️ 子集 ~200 | 同上 |
| `proxy.rs` (主代理状态机) | 1182 | ✅ 全 | 核心 |
| `runtime.rs` (异步 runtime/connection mgmt) | 1776 | ✅ 全 | 核心 |
| `state.rs` (审计/计数) | 419 | ✅ 全 | 安全可观测性必需 |
| `upstream.rs` (上游连接) | 190 | ✅ 全 | 核心 |
| `reasons.rs` (拒绝理由枚举) | 8 | ✅ 全 | 微不足道 |
| `responses.rs` (HTTP 错误页) | 118 | ✅ 全 | UX |
| `certs.rs` (CA 自签证书生成) | 344 | ❌ 不 port | MITM 专用，安全审计风险高 |
| `mitm.rs` (TLS 解密/重签) | 482 | ❌ 不 port | 同上 |
| `mitm_tests.rs` | 128 | ❌ 不 port | 同上 |

**Port 总量：约 3500 LOC（占 codex 40%）。**

---

## 1. 上下文

### 1.1 现状

- **codex `network-proxy`**：8876 LOC，14 文件，rama 框架。功能：HTTP/SOCKS5 转发 + MITM 解密 + 自签 CA + 静态/动态 allowlist + 审计计数。被 `codex-rs/core/src/session/mod.rs:71-73` 等多处生产使用。
- **ironclaw `sandbox/proxy/*`**：1360 LOC，已在 W3.2a (PR #17) 删除（dead code，0 外部消费者）。
- **dasclaw_net_proxy**：W1 占位 24 行 skeleton，只有 `NetProxy` trait stub。

### 1.2 真实需求来源

ironclaw 的网络代理用于哪些场景？
1. **Tool 网络访问审计**：`ReadOnly`/`WorkspaceWrite` 沙箱策略下，工具默认 OS 层拦截网络（Seatbelt/Landlock/Restricted Token）。如果用户允许特定 allowlist，工具应通过本地代理出网，所有请求落审计日志。
2. **凭据注入隔离**：Agent 自己的 HTTP 调用（如 search、fetch tool）不应把 API key 暴露给被沙箱的工具进程。代理可代为注入（旧 ironclaw `CredentialResolver` 的初衷）。
3. **企业合规**：审计原文（host/port/method/timestamps），便于事后合规审查。

**不需要的能力**：
- MITM 解密 TLS（合规高风险，且对 OpenAI/Anthropic API 等已知密钥流量没有审计价值——只看到加密 bytes）
- 动态策略热更新（admin-backend 推送可走重启或独立配置 channel）
- 自签 CA 链（增加安装复杂度 + 安全攻击面）

---

## 2. 决策矩阵

| Tier | 范围 | LOC | 启动时间 | 安全审计风险 | UX 复杂度 |
|---|---|---|---|---|---|
| A. Full port | 全部 8876 LOC | 8876 | 高（CA 安装、证书管理）| 高（MITM）| 高（用户必装 CA）|
| **B. Audit-only forward**（推荐）| 去 MITM/cert | ~3500 | 中 | 低 | 低 |
| C. Minimal HTTP allowlist | 只 HTTP/CONNECT + allowlist | ~1500 | 低 | 极低 | 极低 |
| D. 不做 | 直接走 OS 层 block-all | 0 | 0 | 0 | 用户痛点：无法审计授权出网 |

**评分**（满分 5）：

| Tier | 安全 | 审计 | 维护成本 | 用户 UX | 总分 |
|---|---|---|---|---|---|
| A | 4 | 5 | 2 | 2 | 13 |
| **B** | **5** | **5** | **3** | **4** | **17** |
| C | 5 | 3 | 5 | 4 | 17 |
| D | 5 | 1 | 5 | 2 | 13 |

B 与 C 同分。**选 B 的理由**：SOCKS5 让 git/curl/wget 等不支持 `HTTPS_PROXY` 的工具也能受控，C 会暴露这个口子。多花 ~2000 LOC 一次性投入换长期工具兼容性，账划得来。

---

## 3. 拒绝路线的依据

### A. Full port — 拒绝
- MITM 自签 CA 在 macOS 需写入 keychain，Windows 写入 root CA store，Linux 各发行版路径不一。安装失败率 >20%（codex 用户社区反馈）。
- TLS 解密对 LLM API 流量审计价值低（要看的就是 host/method/timestamp，不是 body）。
- 攻击面：CA 私钥泄漏 = 中间人能伪造任何站点。

### C. Minimal HTTP allowlist — 拒绝
- 无 SOCKS5 → git clone/curl 类工具如果不支持 `HTTPS_PROXY`（或者用户配错），出网就完全绕过审计。
- 现代 agent 工具链普遍要 git/cargo/npm/pip 出网，全部加 `HTTPS_PROXY` 配置太脆弱。

### D. 不做 — 拒绝
- 用户场景 1（审计授权出网）无法满足。
- 等同于"要么完全断网要么完全开放"，不符合 ReadOnly+allowlist 策略的 IronClaw 价值主张。

---

## 4. 实施分阶段

### W3.2b-1 — 项目结构与依赖（约 200 LOC）
- 在 `crates/dasclaw_net_proxy/Cargo.toml` 加依赖：`tokio`, `hyper`, `rama`, `tracing`, `thiserror`, `serde` 等（参考 codex 版本）
- 拆分 src/ 目录骨架：`config.rs`, `http_proxy.rs`, `socks5.rs`, `policy.rs`, `state.rs`, `runtime.rs`, `reasons.rs`, `responses.rs`, `upstream.rs`
- 删 `lib.rs` 中的 W1 skeleton，写新公共 API（`NetProxyServer`, `Builder`, `AuditEvent`, `AllowlistDecider`）

### W3.2b-2 — 端口 forward 核心（约 2500 LOC）
- 复制 codex `http_proxy.rs` + `socks5.rs` + `runtime.rs` + `proxy.rs` + `upstream.rs`
- 改命名：`codex_*` → `dasclaw_*`，去掉 codex 内部依赖
- 留接口可注入 `AllowlistDecider` trait，但默认实现先简单（静态白名单 + audit log）

### W3.2b-3 — 端口 policy 子集（约 600 LOC）
- 复制 `network_policy.rs` 中**静态 allowlist + reasons**部分
- 复制 `policy.rs` 中类型定义
- 删 dynamic policy update / hot reload

### W3.2b-4 — 端口 state/audit（约 400 LOC）
- 复制 `state.rs` 全部
- 留 hook：`AuditSink` trait 让 desktop-client 接 sqlite/log file

### W3.2b-5 — 集成测试 + 文档（约 200 LOC）
- 端到端测试：启动 NetProxy → curl 通过它访问 allow 域名 → block 非 allow 域名 → 审计日志包含两条
- 写 `docs/plans/architecture-refactor/44-net-proxy-port-completion.md` 记录差异

### W3.2b-6 — desktop-client 集成（约 100 LOC）
- 在 ironclaw 启动 NetProxy 监听 localhost:某端口
- ReadOnly/WorkspaceWrite 沙箱注入 `HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY` 环境变量
- 写集成测试

---

## 5. 已识别风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| codex 的 rama 版本与 dasclaw workspace 冲突 | 中 | 高 | 第一步先验证依赖能 resolve，不行就降级用 hyper 直连 |
| codex 内部用了 codex-only 的 logging/error 类型 | 高 | 中 | 明确改写到 dasclaw 标准（thiserror + tracing）|
| 8876 → 3500 LOC 删除可能误删依赖 | 中 | 中 | 分模块独立 PR，每个 PR 跑 cargo build + nextest |
| 用户不愿配置 HTTPS_PROXY | 低 | 低 | 默认通过 `dasclaw_exec` 注入环境变量；用户可关闭 |
| MITM 缺失影响某些审计需求 | 低 | 低 | 文档明确：要审计 body，用 OpenTelemetry on agent side，不在 proxy 层 |

---

## 6. 不做什么（明确边界）

- **不**移植 MITM/cert 模块：永远走 CONNECT 隧道，TLS 流量原样转发
- **不**支持透明代理（iptables/pf 拦截）：用户必须显式配 `HTTPS_PROXY`
- **不**做应用层 LLM 流量解析：那是 admin-backend 的 DLP gateway 的事
- **不**支持热重载策略：重启进程，简单可靠

---

## 7. 决策

✅ **采纳 Tier B**。

下一步：等用户 review 此 ADR，确认后开 W3.2b-1 PR（依赖 + 骨架）。
