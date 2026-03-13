# 任务列表

- [ ] 任务 1：初始化项目根目录和管理后台
  - [ ] 初始化 Git 仓库和根目录 `.gitignore`
  - [ ] 初始化 `admin-backend/package.json` (使用 pnpm workspace 或 npm init)
  - [ ] 为每个子模块 (`audit-center`, `sec-pipeline`, `iam`, `store-svr`, `policy-engine`) 创建基础目录结构：`src/index.ts`, `tsconfig.json`
  - [ ] 在 `admin-backend/` 下创建基础的 Express/Fastify 服务入口代码（Hello World 级别，确认环境可用）

- [ ] 任务 2：初始化客户端终端 (Tauri App)
  - [ ] 使用 `create-tauri-app` 或手动初始化 `client/` 目录 (Vue + TypeScript + Vite)
  - [ ] 配置 `client/src-tauri/Cargo.toml` 添加必要依赖 (如 `serde`, `tauri-plugin-*`)
  - [ ] **UI 层实现**：
    - [ ] 安装 `vue-router`
    - [ ] 创建页面组件：`src/pages/Login.vue`, `src/pages/Chat.vue`, `src/pages/Store.vue`
    - [ ] 创建组件：`src/components/ApprovalModal.vue`
    - [ ] 配置 `src/App.vue` 路由
  - [ ] **Rust 内核层实现**：
    - [ ] 创建 `client/src-tauri/src/kernel/mod.rs`
    - [ ] 创建子模块文件：`validator.rs`, `dlp_engine.rs`, `wasm_box.rs`, `audit_proxy.rs`, `sync_manager.rs`
    - [ ] 在上述 Rust 文件中定义基础 `pub struct` 和 `impl` (例如 `pub fn new() -> Self`)
    - [ ] 在 `main.rs` 中引入 `kernel` 模块并打印启动日志确认加载

- [ ] 任务 3：初始化执行环境 (Python)
  - [ ] 在 `execution-env/` 下创建虚拟环境说明 (README.md) 或 `requirements.txt`
  - [ ] 创建 `execution-env/mcp-bridge/main.py` (包含基础的主循环骨架)
  - [ ] 创建 `execution-env/llm-gateway/server.py` (包含基础的 HTTP/WebSocket 服务骨架)
  - [ ] 创建 `execution-env/openclaw-skills/examples/manifest.json` (插件示例配置)

# 任务依赖关系
- 任务 2 和 任务 3 依赖于 任务 1 的根目录初始化
- UI 层和 Rust 内核层可以并行开发
