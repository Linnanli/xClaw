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
| **企业 Skills 商店 (Store)** | 插件版本控制、灰度发布、权限策略配置。支持手动刷新 ClawHub 清单并按需下载插件内容（物理入库隔离）。 | Rust (Axum), PostgreSQL | P1 |
| **身份管理 (IAM)** | JWT/Session 令牌管理、Token 刷新、会话过期控制。MVP 阶段支持本地主密码，P2 阶段支持 SSO/UKey。 | Rust, JWT, OAuth2 | **P0** |
| **策略引擎 (Policy Engine)** | DLP 脱敏词库分发、敏感操作拦截规则配置。MVP 阶段不实现 RBAC。 | Rust | **P0** |
| **行为审计中心 (Audit)** | 日志同步、SM3 摘要审计仓存证、违规行为对账。 | Rust, ClickHouse/TimescaleDB | P1 |

### B. 客户端 (Client Terminal - Tauri App)
基于 Tauri 框架的桌面端，是用户交互的核心，集成安全微内核。

| 子模块 | 功能描述 | 技术栈 | 优先级 |
| :--- | :--- | :--- | :--- |
| **UI 交互层 (UI Layer)** | 身份认证 (MVP: 本地主密码; P2: SSO/UKey)、对话渲染、CoT 展示、动态水印（Canvas 实现，嵌入用户名/ID）。 | Vue 3, Tauri, TypeScript | **P0** |
| **人工审批 (Approval)** | 敏感操作（如文件删除、外网访问）的对话流内物理拦截确认。 | Tauri, Vue 3, Rust | **P0** |
| **安全内核 (Security Kernel)** | **核心能力集**：SM2 验签、DLP 脱敏、WASM 隔离、SM4 本地库。 | Rust (IronClaw Core) | **P0** |
| **同步管理器 (SyncManager)** | 离线日志缓存、连线后断点续传上报。 | Rust (Tokio) | P1 |

### C. 执行环境 (Execution Environment)
插件运行与模型推理的隔离空间。

| 子模块 | 功能描述 | 技术栈 | 优先级 |
| :--- | :--- | :--- | :--- |
| **ClawHub 沙箱运行时** | 为 ClawHub 不可信插件提供强隔离环境，通过 MCP 协议与宿主通信。 | Wasmtime + MCP Bridge | P1 |
| **MCP 协议网桥 (MCP Bridge)** | 沙箱与宿主的通信层，负责工具路由、调用日志记录、敏感操作拦截。 | Rust, MCP Protocol | P1 |
| **ClawHub 插件市场 (Skills Hub)** | 社区贡献的开源插件市场，支持插件发现、版本管理、安全审核。 | Rust, WASM | P2 |
| **内网推理网关 (LLM Gateway)** | 模型路由、请求脱敏、私有化模型接入。 | Rust, DeepSeek/Qwen | P1 |

---

## 2. 用户交互流程 (User Interaction Flow)

以下是模块间串联的典型场景：

1.  **Skills 上架**：开发者提交 Skill → **SecOps** 扫描 → **Signer** 签名 → **Store** 发布。
2.  **初始化与策略同步**：用户通过 **LoginUI** 登录 → **IAM** 验证 → **PolicyEngine** 同步 DLP 词库到本地 **DLPEngine**。
3.  **插件加载**：
    - **ClawHub 插件**：**Validator** 验证签名 → 加载到沙箱运行时 → 通过 **MCP_Bridge** 与宿主通信。
4.  **安全推理**：用户输入 → **DLPEngine** 本地脱敏 → **WASM_Box** 处理逻辑 → **ModelProxy** 加密推理。
6.  **审计闭环**：操作记录经 **AuditProxy** → **SM3 签名存证** → **LocalDB (SM4)** → 连线后 **SyncManager** 增量同步至后端。

---

## 3. 离线能力矩阵 (Offline Capability Matrix)

系统设计支持离线工作模式，以下是各功能模块的离线能力说明：

| 功能模块 | 离线可用 | 说明 |
| :--- | :---: | :--- |
| 身份认证 | ✅ | 使用本地主密码解锁，无需联网验证 |
| 插件加载 | ✅ | 已下载的插件可离线验证签名并加载 |
| DLP 脱敏 | ✅ | 使用本地缓存的 DLP 词库进行实时过滤 |
| 模型推理 | ✅ | 使用本地部署的 LLM（如 DeepSeek/Qwen） |
| 工具执行 | ✅ | 本地工具（文件操作、计算等）可正常执行 |
| 外网工具 | ❌ | 需要网络连接的工具（API 调用、搜索等）不可用 |
| 插件下载 | ❌ | 需要连接企业 Skills 商店 |
| 策略同步 | ❌ | 使用上次同步的本地缓存策略 |
| 审计上报 | 🔄 | 离线时本地加密存储，连线后自动同步 |

**数据同步冲突解决策略**：
- 审计日志：采用追加模式，无冲突
- 策略更新：服务端优先，连线后强制覆盖本地缓存
- 插件版本：服务端版本号更高时提示用户更新

---

## 4. Core IronClaw 缺失能力补充 (Gaps to Fill)

### 4.1 国密算法栈与多算法切换 (Crypto Agility)
为了满足政企客户的合规要求，同时保持系统灵活性，系统需支持“国密/默认”双算法栈切换。

- **配置项控制**：引入 `CRYPTO_STANDARD` 环境变量。
  - `Default` (默认)：使用 AES-256-GCM, Ed25519, SHA256。
  - `ChinaCrypto`：使用 SM4, SM2, SM3。
- **Provider 模式架构**：
  - 定义统一的 `CryptoProvider` Trait，抽象非对称验签、对称加解密和摘要哈希。
  - 现有逻辑通过注入不同的 Provider 实现算法切换，不删除原有算法代码。

### 4.2 DLP 词库热更新与匹配逻辑
- 现有的 `leak_detector.rs` 较简单，需增加从后端同步策略并实时更新的能力。
- 对应文件：`src/config/safety.rs`, `src/safety/policy.rs`。

### 4.3 本地加密存储层 (Encrypted Local Storage)
- 目前的配置和历史记录是明文或简单存储。需实现基于 **SM4/AES** 的透明存储层。
- 对应文件：`src/db/libsql/`, `src/history/store.rs`。

### 4.4 离线审计代理 (Offline Audit Proxy)
- 实现离线状态下的日志签名与暂存，防止日志被本地篡改。
- 对应文件：`src/observability/log.rs`。

### 4.5 ClawHub 外挂脚本沙箱与 MCP 网桥实现 (Script Sandbox & MCP Bridge)

#### 4.5.1 技术选型：Wasmtime (WASM) + MCP (Scripts)

**为什么引入 MCP？**
- **兼容性**：ClawHub 社区中存在大量无法直接编译为 WASM 的 Python/Node.js 脚本。
- **隔离性**：对于此类不可信脚本，通过外挂沙箱（如 Docker 或轻量级进程隔离）运行，并使用标准 MCP 协议与宿主通信。
- **WASM 插件**：可信或已通过审核的 WASM 插件直接运行在宿主 WASM 运行时中，不经过 MCP。

**架构设计**：
```
[ClawHub 插件市场]
  /            \
[WASM 插件]    [Python/非 WASM 脚本]
    ↓               ↓
[WASM 运行时]    [外挂沙箱运行时]
    ↓               ↓ (MCP 协议)
[宿主导出函数] ←── [MCP Bridge]
    ↓               ↓
[Tool Implementation (系统资源/工具路由)]
```

#### 4.5.2 ClawHub 插件

| 特性 | ClawHub 插件 |
|------|-------------|
| **信任级别** | 不可信（社区贡献） |
| **隔离方式** | MCP 协议通信 |
| **权限控制** | 通过 MCP Bridge 控制 |
| **性能开销** | 低（MCP 序列化开销） |
| **审计日志** | MCP Bridge 记录 |

#### 4.5.3 MCP Bridge 核心功能

- **MCP 协议通信**：
  - 作为 MCP Client 与外挂沙箱（MCP Server）建立标准协议通信。
  - 支持同步/异步调用模式。
- **工具路由**：
  - 根据脚本名称路由到对应的沙箱环境（Python/Node.js 等）。
  - 支持工具版本管理和降级策略。
- **权限与拦截**：
  - 检测外网访问等敏感操作，触发审批流程。
- **调用日志**：
  - 记录外挂脚本的调用参数、返回值、执行时间。
- **对应文件**：`src/tools/mcp/`, `src/sandbox/`。

### 4.6 ClawHub 插件市场管理 (ClawHub Ecosystem)

ClawHub 是社区驱动的开源插件市场。系统通过管理后台进行可控接入。

- **清单刷新与按需下载**：
  - 管理后台提供“手动刷新”按钮，仅从 ClawHub 获取最新的插件清单（Metadata）。
  - 管理员根据业务需求，点击特定插件的“下载”按钮，才真正将插件二进制/脚本内容拉取到企业内部存储。
  - 数据库中对 ClawHub 插件清单与已下载/审核插件进行逻辑隔离。
- **安全审核流**：
  - **初次拉取**：存入待审核池。
  - **审核通过**：使用企业 SM2 私钥进行二次签名，正式上架到企业 Store。
- **客户端展示逻辑**：
  - 客户端仅拉取并展示经过企业二次签名的、状态为“已审核”的插件。
  - 用户在客户端无法直接发现或安装未经审核的社区插件。
- **运行机制**：
  - **WASM 插件**：直接加载到宿主 WASM 运行时，无需 MCP 转换。
  - **非 WASM 脚本**：通过 MCP Bridge 启动对应的沙箱环境运行。
- **对应文件**：`src/skills/clawhub.rs`, `src/tools/wasm/`, `src/tools/mcp/`。

### 4.7 动态水印实现 (Dynamic Watermark)
在 UI 层实现不可见或半透明水印，用于审计溯源。

- **实现方案**：
  - 使用 Canvas API 在对话界面叠加水印层。
  - 水印内容：用户名 + ID。
- **渲染策略**：
  - 透明度：10-20%，不影响用户阅读。
  - 位置：随机平铺或对角线重复。
- **防篡改**：
  - 水印内容经 SM3 哈希后存入审计日志，用于截图溯源。
- **对应文件**：前端 `ui/components/Watermark.vue`。

### 4.8 灰度发布策略 (Canary Deployment)
企业 Skills 商店支持按用户组分阶段推送插件更新。

- **发布阶段**：
  - **Alpha**：内部测试组（5% 用户）。
  - **Beta**：早期采用者（20% 用户）。
  - **GA**：全量发布（100% 用户）。
- **回滚机制**：
  - 监控插件错误率，管理员可手动触发回滚。
- **用户分组**：
  - 基于部门、角色等维度分组。
- **对应文件**：`src/store/canary.rs`, 后端 API `/api/skills/deploy`。

### 4.9 IAM 详细设计 (Identity & Access Management)
IAM 模块负责用户身份验证和会话管理。

- **令牌管理**：
  - 使用 JWT 作为访问令牌（有效期 1 小时）。
  - Refresh Token 用于续期（有效期 7 天）。
- **会话过期控制**：
  - 无操作 30 分钟后自动锁定，需重新输入主密码。
  - 敏感操作（如删除数据）需二次验证。
- **MVP 实现**：
  - 本地主密码 + OS Keychain 存储。
  - 单用户模式，无多租户隔离。
  - P2 阶段集成企业 SSO（SAML/OAuth2）和 UKey 硬件认证。
- **对应文件**：`src/auth/iam.rs`, `src/auth/session.rs`。

**注**：MVP 阶段不实现多设备登录和并发会话管理，后期根据需求扩展。

### 4.10 MVP 身份认证与安全加固 (MVP Auth & Hardening)
考虑到 SSO/UKey 较高的实现复杂度，系统将首先实现一个基于“本地主密码”的 MVP 认证方案。

- **认证逻辑**：
  - 用户首次启动时设置“主密码”。
  - 主密码派生的密钥通过 `keyring` 存储在 **OS Keychain** (Windows Credential Manager / macOS Keychain)。
  - 应用每次启动时需输入主密码（或调用系统生物识别如 Windows Hello/TouchID）进行解锁。
- **存储加固**：
  - 基于 [4.3 本地加密存储层](#43-本地加密存储层-encrypted-local-storage) 的能力，使用主密码派生的密钥对 **libSQL** 进行全库加密。
- **CoT 展示**：
  - 纯前端渲染逻辑。在 UI 层增加 `Thinking` 状态组件，用于展示大模型的中间思考链路，增强交互透明度。

---

---

## 5. 技术栈统一说明 (Tech Stack Alignment)

为确保开发效率和代码一致性，明确各层技术选型：

| 层级 | 技术栈 | 说明 |
| :--- | :--- | :--- |
| **前端 UI** | Vue 3 + TypeScript + Vite | 统一使用 Vue 3 Composition API，避免 React/Vue 混用 |
| **桌面框架** | Tauri 2.x | Rust 后端 + Web 前端的混合架构 |
| **后端核心** | Rust (Tokio + Axum) | 异步运行时 + Web 框架 |
| **数据库** | PostgreSQL (管理后台) + libSQL (客户端) | 服务端关系型数据库 + 客户端嵌入式数据库 |
| **WASM 运行时** | Wasmtime | 支持 WASI 和资源限制 |
| **国密算法** | GMSSL/Tassl (C FFI) | 通过 Rust FFI 调用 |
| **审计存储** | ClickHouse / TimescaleDB | 时序数据库，支持高吞吐写入 |

---

## 6. 总结与建议

*   **P0 阶段**应聚焦于 **国密安全内核**、**物理拦截机制** 和 **IAM 基础认证** 的开发。
*   **P1 阶段**完善 **DLP 自动化策略同步**、**离线审计上报**、**ClawHub 沙箱与 MCP Bridge**、**灰度发布**。
*   **P2 阶段**引入 **RBAC 权限系统**、**多租户隔离**、**ClawHub 插件市场**。
*   **管理后台**可优先实现 **签名机** 与 **策略分发**，作为 MVP 版本的基础。
*   **前端统一使用 Vue 3**，避免技术栈分裂导致的维护成本增加。

### MVP 阶段简化策略
- **无 RBAC**：所有用户共享相同权限，AI 操作限定在指定目录。
- **单用户模式**：不考虑多租户和多设备并发登录。
