# 初始架构设置规范

## 为什么
为了实现 `governance/architecture.md` 中定义的治理架构，我们需要建立一个全功能的开发基础，而不仅仅是目录结构。这包括初始化项目配置文件、依赖管理、以及核心模块的代码骨架。这将确保开发团队可以直接开始业务逻辑的编写，而无需纠结于基础构建配置。

## 变更内容
- **项目根目录**：初始化为 Monorepo 结构（如果适用）或多语言混合项目根。
- **管理后台 (`admin-backend/`)**：初始化为 Node.js/TypeScript 服务集合（模拟微服务结构），包含基础的 HTTP 服务入口。
- **客户端终端 (`client/`)**：初始化为标准的 Tauri (Rust + Vue) 项目。
    - **UI 层**：建立 Vue 组件基本路由和页面骨架。
    - **安全微内核**：建立 Rust `kernel` 模块，并生成 `mod.rs` 及对应子模块的结构体定义。
- **执行环境 (`execution-env/`)**：初始化 Python 环境，用于 LLM 和 MCP 桥接。

## 影响
- **新文件**：`package.json`, `Cargo.toml`, `tsconfig.json`, `requirements.txt` 以及各个模块的入口文件（`main.rs`, `index.ts`, `main.py`）。
- **环境要求**：需要 Node.js, Rust (Cargo), Python 环境。

## 新增需求

### 需求：管理后台服务骨架
系统应在 `admin-backend/` 中初始化各子系统的服务入口：
- **技术栈**：Node.js + TypeScript (基础 Express 或类似结构)
- **组件**：
    - `audit-center/`：创建 `src/index.ts`，包含日志接收接口存根。
    - `sec-pipeline/`：创建 `src/index.ts`，包含扫描任务触发接口存根。
    - `iam/`：创建 `src/index.ts`，包含登录/鉴权接口存根。
    - `store-svr/`：创建 `src/index.ts`，包含插件列表接口存根。
    - `policy-engine/`：创建 `src/index.ts`，包含策略下发接口存根。

### 需求：客户端终端工程化
系统应初始化 `client/` 为可运行的 Tauri 应用：
- **技术栈**：Tauri v2 (Rust) + Vue (TypeScript) + Vite
- **UI 层 (`client/src/`)**：
    - 配置 Vue Router。
    - 创建 `pages/Login.vue`, `pages/Chat.vue`, `pages/Store.vue`, `components/ApprovalModal.vue` 及其基础布局。
- **安全微内核 (`client/src-tauri/src/kernel/`)**：
    - 创建 `mod.rs` 暴露内核模块。
    - 实现 `validator.rs`, `dlp_engine.rs`, `wasm_box.rs`, `audit_proxy.rs`, `sync_manager.rs` 的基础 `struct` 和 `impl`（空方法）。
    - 在 `main.rs` 中注册这些模块或命令。

### 需求：执行环境脚本骨架
系统应在 `execution-env/` 中初始化 Python 项目：
- **技术栈**：Python 3.10+
- **组件**：
    - `mcp-bridge/`：创建 `main.py`，模拟 MCP 协议处理循环。
    - `llm-gateway/`：创建 `server.py`，模拟 LLM 接口转发。
    - `openclaw-skills/`：创建标准插件目录结构和 `manifest.json` 模板。
