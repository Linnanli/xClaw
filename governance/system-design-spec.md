# IronClaw 系统设计说明文档 (System Design Specification)

本文档基于 [architecture.md](file:///c:/codes/ironclaw/governance/architecture.md) 流程图，详细描述各模块功能、技术栈、开发优先级以及 Core IronClaw 的能力补充要求。

---

## 1. 模块功能说明 (Module Breakdown)

### A. 管理后台 (Admin Backend - 管控与审计中心)
部署于企业内网，负责全生命周期的 Skills 管理与安全合规。

| 子模块 | 功能描述 | 技术栈 | 优先级 |
| :--- | :--- | :--- | :--- |
| **Skills 安全审核 (SecOps)** | SCA 依赖扫描、AST 静态代码审计。确保插件无后门。 | Rust/Python, Semgrep, Trivy | P1 |
| **国密签名机 (Signer)** | 使用 SM2 私钥对审核通过的 Skills 进行数字签名。 | Rust, GMSSL/Tassl | **P0** |
| **企业 Skills 商店 (Store)** | 插件版本控制、灰度发布、权限策略配置。 | Rust (Axum), PostgreSQL | P1 |
| **策略引擎 (Policy Engine)** | RBAC 权限管理、DLP 脱敏词库分发。 | Rust, Casbin | **P0** |
| **行为审计中心 (Audit)** | 日志同步、SM3 摘要审计仓存证、违规行为对账。 | Rust, ClickHouse/TimescaleDB | P1 |

### B. 客户端 (Client Terminal - Tauri App)
基于 Tauri 框架的桌面端，是用户交互的核心，集成安全微内核。

| 子模块 | 功能描述 | 技术栈 | 优先级 |
| :--- | :--- | :--- | :--- |
| **UI 交互层 (UI Layer)** | 身份认证 (MVP: 本地主密码; P2: SSO/UKey)、对话渲染、CoT 展示、动态水印。 | Vue, Tauri, CSS | **P0** |
| **人工审批 (Approval)** | 敏感操作（如文件删除、外网访问）的对话流内物理拦截确认。 | Tauri, React, Rust | **P0** |
| **安全内核 (Security Kernel)** | **核心能力集**：SM2 验签、DLP 脱敏、WASM 隔离、SM4 本地库。 | Rust (IronClaw Core) | **P0** |
| **同步管理器 (SyncManager)** | 离线日志缓存、连线后断点续传上报。 | Rust (Tokio) | P1 |

### C. 执行环境 (Execution Environment)
插件运行与模型推理的隔离空间。

| 子模块 | 功能描述 | 技术栈 | 优先级 |
| :--- | :--- | :--- | :--- |
| **WASM 运行时 (WASM Box)** | 资源隔离、API 挂钩、运行时权限受控。 | Wasmtime (Rust) | **P0** |
| **内网推理网关 (LLM Gateway)** | 模型路由、请求脱敏、私有化模型接入。 | Rust, DeepSeek/Qwen | P1 |

---

## 2. 用户交互流程 (User Interaction Flow)

以下是模块间串联的典型场景：

1.  **Skills 上架**：开发者提交 Skill → **SecOps** 扫描 → **Signer** 签名 → **Store** 发布。
2.  **初始化与策略同步**：用户通过 **LoginUI** 登录 → **IAM** 验证 → **PolicyEngine** 同步 RBAC 权限与 DLP 词库到本地 **DLPEngine**。
3.  **插件加载**：用户请求 Skill → **Validator** 使用 SM2 公钥验证签名 → 成功后载入 **WASM_Box**。
4.  **安全推理**：用户输入 → **DLPEngine** 本地脱敏 → **WASM_Box** 处理逻辑 → **ModelProxy** 加密推理。
5.  **受控执行 (物理拦截)**：WASM 触发敏感操作 → **对话流渲染审批卡片** → 用户点击“确认” → **MCP_Bridge** 执行。
6.  **审计闭环**：操作记录经 **AuditProxy** → **SM3 签名存证** → **LocalDB (SM4)** → 连线后 **SyncManager** 增量同步至后端。

---

## 3. Core IronClaw 缺失能力补充 (Gaps to Fill)

### 3.1 国密算法栈与多算法切换 (Crypto Agility)
为了满足政企客户的合规要求，同时保持系统灵活性，系统需支持“国密/默认”双算法栈切换。

- **配置项控制**：引入 `CRYPTO_STANDARD` 环境变量。
  - `Default` (默认)：使用 AES-256-GCM, Ed25519, SHA256。
  - `ChinaCrypto`：使用 SM4, SM2, SM3。
- **Provider 模式架构**：
  - 定义统一的 `CryptoProvider` Trait，抽象非对称验签、对称加解密和摘要哈希。
  - 现有逻辑通过注入不同的 Provider 实现算法切换，不删除原有算法代码。

### 3.2 DLP 词库热更新与匹配逻辑
- 现有的 `leak_detector.rs` 较简单，需增加从后端同步策略并实时更新的能力。
- 对应文件：`src/config/safety.rs`, `src/safety/policy.rs`。

### 3.3 本地加密存储层 (Encrypted Local Storage)
- 目前的配置和历史记录是明文或简单存储。需实现基于 **SM4/AES** 的透明存储层。
- 对应文件：`src/db/libsql/`, `src/history/store.rs`。

### 3.4 物理拦截信号机制 (Physical Interception Signal)
- 现有的工具审批是异步逻辑，需与前端 Tauri 对话流建立强一致性的阻塞/唤醒机制。
- 对应文件：`src/agent/agentic_loop.rs`, `src/agent/session.rs`。

### 3.5 离线审计代理 (Offline Audit Proxy)
- 实现离线状态下的日志签名与暂存，防止日志被本地篡改。
- 对应文件：`src/observability/log.rs`。

### 3.6 MVP 身份认证与安全加固 (MVP Auth & Hardening)
考虑到 SSO/UKey 较高的实现复杂度，系统将首先实现一个基于“本地主密码”的 MVP 认证方案。

- **认证逻辑**：
  - 用户首次启动时设置“主密码”。
  - 主密码派生的密钥通过 `keyring` 存储在 **OS Keychain** (Windows Credential Manager / macOS Keychain)。
  - 应用每次启动时需输入主密码（或调用系统生物识别如 Windows Hello/TouchID）进行解锁。
- **存储加固**：
  - 基于 [3.3 本地加密存储层](#33-本地加密存储层-encrypted-local-storage) 的能力，使用主密码派生的密钥对 **libSQL** 进行全库加密。
- **CoT 展示**：
  - 纯前端渲染逻辑。在 UI 层增加 `Thinking` 状态组件，用于展示大模型的中间思考链路，增强交互透明度。

---

## 4. 总结与建议

*   **P0 阶段**应聚焦于 **国密安全内核** 与 **物理拦截机制** 的开发。
*   **P1 阶段**完善 **DLP 自动化策略同步** 与 **离线审计上报**。
*   **管理后台**可优先实现 **签名机** 与 **策略分发**，作为 MVP 版本的基础。
