# 任务列表

- [ ] 任务 1：初始化项目结构
  - [ ] 创建 `admin-backend/`, `client/`, `execution-env/` 目录
  - [ ] 将 `admin-backend/` 初始化为子模块（创建仓库，添加子模块）

- [ ] 任务 2：设置管理后台结构（在子模块中）
  - [ ] 创建 `sec-pipeline/`（供应链扫描、静态代码审计、签名机）
  - [ ] 创建 `iam/`（LDAP/SSO/UKey）
  - [ ] 创建 `store-svr/`（技能商店）
  - [ ] 创建 `policy-engine/`（RBAC权限、DLP词库）
  - [ ] 创建 `audit-center/`（日志同步、对账引擎、审计仓库）

- [ ] 任务 3：设置客户端终端结构（Tauri）
  - [ ] 创建 `client/src-tauri/`（Rust 后端）
  - [ ] 创建 `client/src/`（前端 UI）
  - [ ] 创建 UI 模块：`login/`, `chat/`, `store/`, `approval/`
  - [ ] 创建安全微内核模块：`validator/`, `dlp-engine/`, `wasm-box/`, `audit-proxy/`, `sync-manager/`

- [ ] 任务 4：设置执行环境结构
  - [ ] 创建 `execution-env/mcp-bridge/`
  - [ ] 创建 `execution-env/llm-gateway/`（模型代理、本地LLM）
  - [ ] 创建 `execution-env/openclaw-skills/`

# 任务依赖关系
- 任务 2 依赖于任务 1
- 任务 3、4 可以与任务 2 并行运行