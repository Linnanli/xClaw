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
| **UI 交互层 (UI Layer)** | 身份认证 (SSO/UKey)、对话渲染、CoT 展示、动态水印。 | React/Vue, Tauri, CSS | **P0** |
| **人工审批弹窗 (Approval)** | 敏感操作（如文件删除、外网访问）的物理拦截确认。 | Tauri Dialog, Rust | **P0** |
| **安全内核 (Security Kernel)** | **核心能力集**：SM2 验签、DLP 脱敏、WASM 隔离、SM4 本地库。 | Rust (IronClaw Core) | **P0** |
| **同步管理器 (SyncManager)** | 离线日志缓存、连线后断点续传上报。 | Rust (Tokio) | P1 |

### C. 执行环境 (Execution Environment)
插件运行与模型推理的隔离空间。

| 子模块 | 功能描述 | 技术栈 | 优先级 |
| :--- | :--- | :--- | :--- |
| **WASM 运行时 (WASM Box)** | 资源隔离、API 挂钩、运行时权限受控。 | Wasmtime (Rust) | **P0** |
| **MCP 网桥 (Bridge)** | 兼容 MCP 协议，驱动本地或远程工具。 | Rust, MCP SDK | **P0** |
| **内网推理网关 (LLM Gateway)** | 模型路由、请求脱敏、私有化模型接入。 | Rust, DeepSeek/Qwen | P1 |

---

## 2. 用户交互流程 (User Interaction Flow)

以下是模块间串联的典型场景：

1.  **Skills 上架**：开发者提交 Skill → **SecOps** 扫描 → **Signer** 签名 → **Store** 发布。
2.  **初始化与策略同步**：用户通过 **LoginUI** 登录 → **IAM** 验证 → **PolicyEngine** 同步 RBAC 权限与 DLP 词库到本地 **DLPEngine**。
3.  **插件加载**：用户请求 Skill → **Validator** 使用 SM2 公钥验证签名 → 成功后载入 **WASM_Box**。
4.  **安全推理**：用户输入 → **DLPEngine** 本地脱敏 → **WASM_Box** 处理逻辑 → **ModelProxy** 加密推理。
5.  **受控执行 (物理拦截)**：WASM 触发敏感操作 → **ApprovalUI** 弹窗 → 用户点击“确认” → **MCP_Bridge** 执行。
6.  **审计闭环**：操作记录经 **AuditProxy** → **SM3 签名存证** → **LocalDB (SM4)** → 连线后 **SyncManager** 增量同步至后端。

---

## 3. Core IronClaw 缺失能力补充 (Gaps to Fill)

要完全实现流程图中的功能，目前的 `src/` 代码需要补充以下核心能力：

1.  **国密算法栈 (China Crypto Standard)**：
    *   目前使用 Ed25519/SHA256，需替换或补充 **SM2** (签名/验签)、**SM3** (哈希/审计)、**SM4** (静态加密)。
    *   对应文件：`src/channels/wasm/signature.rs`, `src/safety/` 等。

2.  **DLP 词库热更新与匹配逻辑**：
    *   现有的 `leak_detector.rs` 较简单，需增加从后端同步策略并实时更新的能力。
    *   对应文件：`src/config/safety.rs`, `src/safety/policy.rs`。

3.  **本地加密存储层 (Encrypted Local Storage)**：
    *   目前的配置和历史记录是明文或简单存储。需实现基于 SM4 的透明存储层。
    *   对应文件：`src/db/libsql/`, `src/history/store.rs`。

4.  **物理拦截信号机制 (Physical Interception Signal)**：
    *   现有的工具审批是异步逻辑，需与前端 Tauri 弹窗建立强一致性的阻塞/唤醒机制。
    *   对应文件：`src/agent/agentic_loop.rs`, `src/agent/session.rs`。

5.  **离线审计代理 (Offline Audit Proxy)**：
    *   实现离线状态下的日志签名与暂存，防止日志被本地篡改。
    *   对应文件：`src/observability/log.rs`。

---

## 4. 总结与建议

*   **P0 阶段**应聚焦于 **国密安全内核** 与 **物理拦截机制** 的开发。
*   **P1 阶段**完善 **DLP 自动化策略同步** 与 **离线审计上报**。
*   **管理后台**可优先实现 **签名机** 与 **策略分发**，作为 MVP 版本的基础。
