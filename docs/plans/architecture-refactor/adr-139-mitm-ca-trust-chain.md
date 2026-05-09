# ADR-139: MITM CA trust chain management for `dasclaw_net_proxy`

- **Status**: 🟡 **Decision: pending — proposes per-user keychain injection (Option A) over system-store injection (Option B/C)**（research-only ADR；implementation **out of scope** for this PR）
- **Date**: 2026-05-09
- **Approver**: pending nally sign-off
- **Authors**: GitHub Copilot agent
- **Tracker**: #350 — ADR-139 MITM CA trust chain management
- **Related**:
  - [ADR-137](adr-137-net-proxy-port-plan.md) §4 Q5 — net-proxy port plan，遗留 CA 信任链问题
  - [ADR-129](adr-129-sandbox-windows-windows-crate-adoption.md) §1.3 — verbatim port red-line（CA 生成红线归属 codex）
  - codex 上游：`codex-cli-main/codex-rs/network-proxy/src/certs.rs`（已 verbatim port → [`crates/dasclaw_net_proxy/src/certs.rs`](../../../crates/dasclaw_net_proxy/src/certs.rs)，PR #355）
  - 已落地基础：`ManagedMitmCa::load_or_create()` + `$CODEX_HOME/proxy/{ca.pem,ca.key}` 生成路径
  - tracking issue：#324 sub-task 3 完成 → 解锁本 ADR

---

## 1. Context

### 1.1 现状 — verbatim port 后的 CA 生成已就位

PR #355 落地后，`dasclaw_net_proxy` 已具备完整 MITM 能力：

| 能力 | 实现位置 | 备注 |
|---|---|---|
| CA 自动生成 | [`certs.rs::load_or_create_ca`](../../../crates/dasclaw_net_proxy/src/certs.rs) | ECDSA P-256 + SHA-256，10 年有效期 |
| CA 持久化 | `$CODEX_HOME/proxy/ca.pem` + `ca.key` | 文件权限 0600（仅 Linux/macOS）|
| 主机证书签发 | `tls_acceptor_data_for_host(host)` | 每 host 即时签发，不持久化 |
| Issuer 缓存 | `ManagedMitmCa { issuer: Issuer<'static, KeyPair> }` | 进程级单例 |

**未实现**：把 CA 注入操作系统/浏览器/CLI 工具的信任链，导致：

- HTTPS 客户端（`reqwest` / `curl` / `git` / `npm` / Chrome / Safari / Firefox）**全部**因 CA 不在信任链而拒绝 TLS 握手
- desktop-client 启动后 MITM 拦截立即失效，沙箱进程的 HTTPS 流量直通或失败
- 当前 `crates/dasclaw_net_proxy/tests/` 里 `mitm_tests.rs` 用的是测试客户端显式加载 CA pem，不代表生产路径

### 1.2 三平台信任链注入面

| 平台 | 系统级 store | 用户级 store | CLI 工具 | 浏览器特殊处理 |
|---|---|---|---|---|
| **macOS** | `/Library/Keychains/System.keychain`（需 root + UI prompt） | `~/Library/Keychains/login.keychain-db`（per-user，无需 root） | curl/git 走 SecureTransport→读 Keychain；Python `requests` 默认走 certifi（需额外注入） | Safari/Chrome 读 Keychain；**Firefox 用独立 NSS DB**（`~/Library/Application Support/Firefox/Profiles/*/cert9.db`） |
| **Windows** | `LocalMachine\Root`（需 admin）| `CurrentUser\Root`（per-user，无需 admin） | curl.exe/git.exe 默认走 schannel→读 CertStore；npm/Python 同 macOS | Edge/Chrome 读 CertStore；**Firefox 同 NSS DB** |
| **Linux** | `/etc/ssl/certs/`（需 root + `update-ca-certificates`/`update-ca-trust`） | 无统一约定；通常需要 NSS shared DB `~/.pki/nssdb`（per-user） | curl/git 读 `SSL_CERT_FILE` 环境变量或 system bundle；Python certifi 同 | Chrome 读 NSS DB；Firefox 同 NSS DB |

### 1.3 codex 上游处理

codex 上游 `codex-cli-main/codex-rs/network-proxy/` **不**实现信任链注入：

- CA 生成在 `$CODEX_HOME/proxy/ca.pem`
- 子进程通过 `SSL_CERT_FILE=$CODEX_HOME/proxy/ca.pem` 环境变量读取（见 codex `core/src/exec.rs` 注入逻辑）
- **不**写系统/用户 store，也**不**触碰浏览器 NSS DB

**含义**：codex 选择"环境变量 + 进程级注入"路径，规避了三平台 CRUD 复杂度。

---

## 2. Decision options

### 2A. **Per-user keychain + env-var fallback**（推荐）

启动 desktop-client 时：

1. 调用 `ManagedMitmCa::load_or_create()`（已有）拿到 `$CODEX_HOME/proxy/ca.pem`
2. **可选用户授权步骤**：在首次启动 wizard 询问用户 "Trust dasclaw MITM CA for HTTPS inspection?"
3. 用户同意 → 写入**用户级** keychain（macOS `login.keychain-db` / Windows `CurrentUser\Root` / Linux `~/.pki/nssdb`）
4. 用户拒绝或写入失败 → 回落到**子进程 env var**：`SSL_CERT_FILE` / `NODE_EXTRA_CA_CERTS` / `REQUESTS_CA_BUNDLE` / `GIT_SSL_CAINFO` 注入
5. 用户卸载或主动撤销 → 提供 `dasclaw cert revoke` CLI 子命令，从 keychain 清除并删除 `$CODEX_HOME/proxy/`

| 项 | 处理 |
|---|---|
| 系统权限要求 | **不需要 root/admin**（per-user store 写入） |
| Crate 选型 | macOS `security-framework = "3"`（per-user keychain API）；Windows `windows = "0.59"` features `["Win32_Security_Cryptography_Catalog","Win32_Security_Cryptography"]`；Linux `nss-sys` 或 fork-exec `certutil -d sql:$HOME/.pki/nssdb -A` |
| Firefox NSS | **out of scope** for v1 — 用户需手动导入；ADR-140 跟踪 |
| Fallback env-var | `SSL_CERT_FILE=$CODEX_HOME/proxy/ca.pem` + `NODE_EXTRA_CA_CERTS` + `REQUESTS_CA_BUNDLE` + `GIT_SSL_CAINFO` 写入沙箱子进程 environment |
| 撤销 | `dasclaw cert revoke` 子命令；清空 keychain entry + 删 `$CODEX_HOME/proxy/` 文件 |
| 轮换 | CA 默认 10 年有效；`dasclaw cert rotate` 重新生成 + 重新注入；启动时若 CA 剩余 < 30 天自动 prompt 轮换 |
| 用户体验 | 首次 wizard 一次授权；后续无 prompt |
| 卸载清理 | desktop-client uninstaller 调用 `dasclaw cert revoke` |

**优点**：
- 不需要管理员权限，CI/容器/无 sudo 环境可用
- 失败回退 env-var 保底
- 撤销路径清晰，用户可控

**缺点**：
- 三平台 NSS 处理割裂（Linux 必须 NSS DB；macOS/Windows 走原生 API）
- Firefox 仍需手动导入（NSS profile 路径每个 profile 一份）
- Python certifi bundle 需要 env var 路径（keychain 注入对 certifi 无效）

### 2B. **System-store injection**（最强但侵入大）

写入系统级 store：macOS `System.keychain` / Windows `LocalMachine\Root` / Linux `/etc/ssl/certs/`。

| 项 | 处理 |
|---|---|
| 权限 | **必须 root/admin**；UI 弹 sudo prompt；CI/容器场景失败 |
| 兼容性 | 所有 OS 内置工具自动信任，无需 env var |
| 撤销 | uninstaller 必须 root，否则残留信任 |

**否决理由**：CI/devcontainer/restricted-user 场景全部失效；用户安装 desktop-client 不应被强制要 sudo。

### 2C. **Env-var-only**（最保守）

完全不碰 keychain，只在沙箱子进程 environment 注入 `SSL_CERT_FILE` 等变量。

| 项 | 处理 |
|---|---|
| 权限 | 零 |
| 兼容性 | curl/git/Python/Node 全部 OK；**浏览器不行**（用户用 Chrome 访问 sandbox 暴露的 HTTPS 端口仍报警告） |
| 撤销 | 删 `$CODEX_HOME/proxy/` 即可 |

**否决理由**：desktop-client 是 Tauri 应用，会嵌入 webview 访问 sandbox 服务；webview/外部浏览器无法读 env var，必须信任链注入。

---

## 3. Decision (proposed)

**采用 2A**：per-user keychain + env-var fallback；Firefox/NSS 手动导入文档化为 v1 已知限制，ADR-140 跟踪自动化。

理由：
- **零特权**满足 CI / devcontainer / restricted-user 场景
- **env-var fallback** 兜底主流 CLI 工具栈
- **撤销/轮换** 由 `dasclaw cert {revoke,rotate}` 子命令统一管理，对应 codex 哲学（用户掌控）
- **Firefox** 用户群体在企业/dev 场景占少数，文档说明可接受

---

## 4. Implementation plan（out of scope for this ADR）

### 4.1 Crate 拓扑

新增 `crates/dasclaw_cert_trust/`（**非** verbatim — 这是 dasclaw 原创代码，因 codex 上游不实现此层）：

```
dasclaw_cert_trust
├── Cargo.toml         # security-framework / windows / nss-sys 按 cfg 启用
├── src/
│   ├── lib.rs         # pub fn install_ca() / uninstall_ca() / rotate_ca()
│   ├── macos.rs       # security-framework SecKeychain*
│   ├── windows.rs     # windows::Win32::Security::Cryptography
│   ├── linux.rs       # certutil fork-exec or nss-sys binding
│   └── fallback.rs    # env-var injection helper
└── tests/             # 三平台 mock/integration 测试
```

### 4.2 三平台依赖

| 平台 | crate | 备选 |
|---|---|---|
| macOS | `security-framework = "3"` ([`apple/security-framework-rs`](https://github.com/kornelski/rust-security-framework)，已 verbatim 通过 codex `apple-codesign` 路径) | `keychain-sys` 直接 FFI |
| Windows | `windows = { version = "0.59", features = ["Win32_Security_Cryptography_Catalog"] }` | `wincrypt-sys` 低层 FFI |
| Linux | fork-exec `certutil -d sql:$HOME/.pki/nssdb -A -t "C,," -n dasclaw -i ca.pem`（需用户安装 `libnss3-tools`）| `nss-sys` Rust binding（实验性） |

### 4.3 CLI 表面

```sh
dasclaw cert install      # 写入 keychain（首次 wizard 自动调用）
dasclaw cert revoke       # 从 keychain 移除并删除 $CODEX_HOME/proxy/
dasclaw cert rotate       # 重新生成 CA + 重新注入
dasclaw cert status       # 显示 CA 指纹/到期时间/keychain 状态
dasclaw cert export       # 导出 ca.pem 给用户手动注入 Firefox/其他工具
```

### 4.4 测试矩阵

| 平台 | CI | 手动 e2e |
|---|---|---|
| macOS-latest | ✅ keychain mock + per-user keychain integration | curl HTTPS / Safari 访问 / Chrome 访问 |
| windows-latest | ✅ CertStore mock + CurrentUser\Root integration | curl.exe / Edge / Chrome |
| ubuntu-latest | ✅ certutil + NSS DB integration | curl / Chrome / Firefox 手动 |

### 4.5 子 issue 拆分（实施 PR）

后续单独 issue 拆 ≥3 个 PR：

1. `feat(cert-trust): scaffold crate + CLI subcommands`（S）
2. `feat(cert-trust): macOS + Windows keychain implementation`（M）
3. `feat(cert-trust): Linux NSS DB + uninstaller integration`（M）

---

## 5. Risks & mitigations

| 风险 | 缓解 |
|---|---|
| keychain API 平台差异 → bug 表面大 | 每平台独立 cfg-gated 模块 + 三平台 CI integration |
| 用户拒绝授权 → MITM 失效 | env-var fallback；wizard 文案明确说明非强制 |
| CA 私钥泄漏 → 全用户中间人风险 | `ca.key` 0600；keychain 走系统加密 store；ADR 强制 `dasclaw cert rotate` 周期提醒 |
| Firefox 用户体验差 | 文档化已知限制；提供 `dasclaw cert export` + 一键导入向导（ADR-140 跟踪）|
| 卸载残留 | uninstaller 调 `dasclaw cert revoke`；CI 验证幂等 |
| `certutil` 在最小化 Linux 缺失 | wizard 检测缺失时降级到 env-var only + 提示安装 `libnss3-tools` |

---

## 6. Out of scope

- ❌ Firefox NSS profile 自动注入 → ADR-140 单独追踪
- ❌ 实施 PR — 本 ADR 仅决策，子 issue 拆分见 §4.5
- ❌ 企业级 PKI（外部 CA / SCEP / EST）→ 未来 ADR
- ❌ 移动端（iOS/Android）信任链 → 不在 desktop-client 范围

---

## 7. Open questions

1. `dasclaw cert install` 是 desktop-client 启动 wizard 自动调用，还是要求用户显式跑？建议**自动 + 可拒绝**。
2. `nss-sys` Linux binding 实验性；是否接受 fork-exec `certutil` 作为 v1 兜底？建议接受。
3. CA 私钥是否考虑用 OS keychain 存储私钥（而不是 `ca.key` 文件）？建议 v2 优化。
