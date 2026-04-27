# Docker (ironclaw) vs OS-level (codex) 沙箱能力对比

**生成时间**：2026-04-27  
**分析方法**：AGENTS.md §"分析工具使用规范" 三级流程  
- L1 语义搜（`semantic_search`，本次因索引未建退化为多关键词 grep）  
- L2 符号引用（`vscode_listCodeUsages` + 直接 import 链分析）  
- L3 字面量（`grep_search` 跨 codex-cli-main + ironclaw）

## 1. 文件 / 代码量对照

### Docker (ironclaw) 沙箱模块
**位置**：`desktop-client/ironclaw/src/sandbox/`

| 文件 | LOC | 角色 |
|------|----:|------|
| `manager.rs` | 697 | SandboxManager 主入口，协调 proxy + container |
| `container.rs` | 635 | bollard Docker 容器创建/启停/清理 |
| `detect.rs` | 235 | Docker 守护进程探测 |
| `agent_executor.rs` | 281 | x_claw_agent::SandboxExecutor adapter (标注 dead path) |
| `config.rs` | 233 | SandboxPolicy 3 变体 + ResourceLimits |
| `error.rs` | 58 | 错误枚举 (含 `Docker(bollard::Error)`) |
| `proxy/allowlist.rs` | 335 | 域名 pattern 匹配 |
| `proxy/http.rs` | 556 | HTTP proxy server + **凭证注入** |
| `proxy/policy.rs` | 306 | NetworkPolicyDecider |
| `proxy/mod.rs` | 163 | facade |
| **合计** | **3499** | (不含 mod.rs / os_executor.rs) |

### OS-level (codex) 沙箱模块
**位置**：`codex-cli-main/codex-rs/`

| Crate / 文件 | LOC | 角色 |
|------|----:|------|
| `sandboxing/src/{lib,landlock,seatbelt,policy_transforms}.rs` | ~1500 | 跨平台沙箱抽象 + 策略转换 |
| `linux-sandbox/src/proxy_routing.rs` | ~770 | HTTP_PROXY/NPM_CONFIG_HTTP_PROXY 等多环境路由 |
| `linux-sandbox/` 其他 | ~5000+ | landlock + seccomp + namespace |
| `windows-sandbox-rs/` | ~3000+ | Windows Restricted Token |
| `network-proxy/src/certs.rs` | 344 | TLS CA 证书生成 |
| `network-proxy/src/config.rs` | 869 | NetworkMode / NetworkProxyConfig / NetworkDomainPermissions |
| `network-proxy/src/http_proxy.rs` | 1319 | HTTP proxy server |
| `network-proxy/src/mitm.rs` | 482 | **HTTPS MITM 拦截/解密** |
| `network-proxy/src/network_policy.rs` | 898 | NetworkPolicyDecider + 决策管道 |
| `network-proxy/src/policy.rs` | 465 | host normalization |
| `network-proxy/src/proxy.rs` | 1182 | 主 NetworkProxy 结构 |
| `network-proxy/src/runtime.rs` | 1776 | 运行时管理 (热加载) |
| `network-proxy/src/socks5.rs` | 616 | **SOCKS5 代理** |
| `network-proxy/src/state.rs` | 419 | 状态管理 |
| `network-proxy/src/upstream.rs` | 190 | 上游 proxy 链 |
| `network-proxy/合计` | **8876** | (6.5x ironclaw proxy/*) |
| `process-hardening/src/lib.rs` | ~120 | setrlimit(RLIMIT_CORE) 防 core dump |

## 2. 能力维度对照表（实证后修正版）

| 能力维度 | Docker (ironclaw) | OS-level (codex) | OS 能否替换 Docker |
|---------|:--------:|:--------:|:--------:|
| **进程隔离** | ✅ Docker container | ✅ Seatbelt / Landlock+seccomp / Restricted Token | ✅ |
| **FS read/write 控制** (root 级) | ✅ mount ro/rw | ✅ Landlock path rules | ✅ |
| **FS 洞中洞** (writable_root 内 read-only 子路径) | ✅ kernel 强制 | ⚠️ 用户态 `is_path_writable` 决策（kernel 暂不强制） | ⚠️ **降级** |
| **timeout** | ✅ | ✅ (`tokio::time::timeout`) | ✅ |
| **RLIMIT_CORE 防 dump** | ❌ (容器隔离即足够) | ✅ `process-hardening` crate | N/A |
| **Memory 限额** | ✅ 2GB 默认 | ❌ **未实现** | ❌ **缺失** |
| **CPU 限额** | ✅ 1024 shares | ❌ **未实现** | ❌ **缺失** |
| **HTTP proxy** | ✅ | ✅ | ✅ |
| **HTTPS MITM 拦截** | ❌ | ✅ `mitm.rs` (482 LOC) | ✅ **升级** |
| **SOCKS5 代理** | ❌ | ✅ `socks5.rs` (616 LOC) | ✅ **升级** |
| **Domain allowlist** | ✅ pattern | ✅ pattern + 三态 (Allow/Deny/Default) | ✅ |
| **Multi-tool proxy env** (NPM/YARN/BUNDLE/DOCKER 等 ~10 种) | ❌ 仅 http_proxy/https_proxy | ✅ 完整覆盖 | ✅ **升级** |
| **TLS CA 证书生成** | ❌ | ✅ `certs.rs` | ✅ **升级** |
| **Unix socket 权限** | ❌ | ✅ `NetworkUnixSocketPermissions` | ✅ **升级** |
| **配置热加载** | ❌ | ✅ `ConfigReloader` / `runtime.rs` | ✅ **升级** |
| **凭证注入** (按域名注入 API key 到上游请求) | ✅ `CredentialResolver` trait | ❌ **未实现** | ❌ **缺失** |
| **网络请求审计/日志** | ✅ | ✅ | ✅ |

## 3. 真正的 gap（codex 未覆盖 ironclaw 的能力）

只有 **2 项**：

### Gap 1: Memory / CPU 资源限额
- **影响**：恶意或失控的子进程能消耗主机全部资源
- **缓解空间**：
  - macOS：Seatbelt 不原生支持 cgroup-style limit（需配合 `launchd plist` 或外部 wrapper）
  - Linux：可加 cgroup v2，需在 `dasclaw_sandbox` 实现
  - 通用：`setrlimit(RLIMIT_AS, RLIMIT_CPU)` 可实现进程级限额（codex 已有 `setrlimit(RLIMIT_CORE)` 模式可借鉴）
- **优先级**：P1（生产可用前必须补）

### Gap 2: 凭证注入（按域名注入 secret 到上游请求）
- **架构差异**：
  - **ironclaw 模型**：secret 存 host，container 内不持有；proxy 在请求阶段按域名解析并注入 `Authorization`/header
  - **codex 模型**：secret 通过环境变量传入子进程；proxy 不注入 secret
- **影响**：
  - codex 模型下，敏感凭证可能出现在 child 进程的环境变量、日志、core dump
  - ironclaw 模型对 supply chain 攻击（恶意脚本 grep `process.env`）更鲁棒
- **缓解空间**：
  - 可在 codex `network-proxy` 之上加一层 hook：解析目标域名 → 查 host secret store → 注入 header
  - 或保留 ironclaw `proxy/http.rs` 的 `CredentialResolver` 部分逻辑，与 codex `network-proxy` 拼接
- **优先级**：P0（这是 ironclaw vs codex 的核心安全卖点）

## 4. 反向：codex 比 Docker 强的地方（5 项）

| # | 能力 | 价值 |
|---|------|------|
| 1 | HTTPS MITM | 可审计加密流量（防 secret 走 HTTPS 出网） |
| 2 | SOCKS5 | 兼容更多客户端（git clone via SSH-over-SOCKS5） |
| 3 | Multi-tool proxy env | npm/yarn/bundle/docker 各自独立配置，更精细 |
| 4 | TLS CA 自动生成 | MITM 必备，开箱即用 |
| 5 | 配置热加载 | 不重启即可调整白名单 |

## 5. W3.1c 路径决策（基于实证）

### A. 推荐：双层渐进
**W3.1c**（本轮）：
- 删 `manager.rs` / `container.rs` / `detect.rs` / `agent_executor.rs`（~1850 LOC）
- 删 `bollard` 依赖
- **保留** `proxy/*` (1361 LOC)
- 让 `OsExecutor` 启动命令前注入 `http_proxy=http://127.0.0.1:PORT`

**W3.2**（后续）：
- 评估迁移到 `codex_network_proxy`（多 5 倍能力但 8800 LOC vs 1361 LOC）
- 把 ironclaw `CredentialResolver` 适配到 codex proxy 之上

**W3.3**（后续）：
- 实现 Memory/CPU 限额（Linux cgroup v2 + 通用 setrlimit）

### B. 激进：直接全替换
- 删 `manager.rs` + `container.rs` + `detect.rs` + `agent_executor.rs` + **整个 `proxy/*`**
- 接入 `codex_network_proxy`（需要在 ironclaw 里加 path 依赖）
- 在 codex proxy 上写一个 credential injection hook layer
- 风险：跨大模块同步重构，回归点多

### C. 保守：暂不删 Docker
- W3.1c 不动 Docker，只把 OsExecutor 作为 fast path
- ShellTool 持有 `Either<OsExecutor, SandboxManager>`
- 用户配置选择
- 维护成本翻倍，长期不可持续

## 6. 推荐组合

**推荐 A（双层渐进）**，理由：
1. W3.1c 风险最低（只删容器层，proxy 层不动，credential injection 不丢）
2. 立即收获：删除 ~1850 LOC + bollard 依赖（攻击面变小）
3. 不锁死后续路径（W3.2 仍可评估是否迁 codex network-proxy）
4. 关键 gap (Memory/CPU 限额) 留到 W3.3 单独处理，避免 W3.1c 包揽过多

## 7. 工具链使用记录（按 AGENTS.md 规范）

| 工具 | 用途 | 命中数 |
|------|------|-------:|
| L1 `semantic_search` × 3 | 凭证注入 / 资源限制 / 网络白名单 概念搜 | 0（索引未建） |
| L1 `grep_search`（多关键词正则） × 3 | 替代 L1 语义搜 | 95+ |
| L2 import 链 / `pub use` 反查 | 定位 codex `network-proxy` crate | 1 个 8876 LOC crate |
| L3 `grep_search` 字面量 × 3 | mitm/socks5/credential 实现细节 | 30+ |
| `read_file` 关键模块头部 | manager.rs / container.rs / proxy/mod.rs / dasclaw_sandbox/lib.rs / network-proxy/lib.rs | 5 个 |

**关键纠错点**：第一轮分析（仅看 ironclaw + dasclaw_sandbox）误判"codex 无 proxy"。L3 grep 找到 `linux-sandbox/proxy_routing.rs` + `seatbelt_tests.rs` 引用 `codex_network_proxy::*` 后才发现 codex 自带 8876 LOC 的完整 proxy 系统。这印证 AGENTS.md 强调的"否定性结论必须给出 L1+L3 双证据"。
