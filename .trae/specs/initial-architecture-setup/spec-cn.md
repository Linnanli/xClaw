# 初始架构设置规范

## 为什么
为了实现 `governance/architecture.md` 中定义的治理架构，将关注点分离到不同的模块中，以提高安全性、可维护性和可扩展性。管理后台需要严格分离（独立仓库）以进行安全审计和访问控制。

## 变更内容
- 创建反映架构图的模块化项目结构
- **管理后台**：在 `admin-backend/` 中初始化为 git 子模块
- **客户端终端**：在 `client/` 中设置 Tauri 应用程序结构
- **执行环境**：在 `execution-env/` 中创建服务结构
- **安全微内核**：在 `client/src-tauri/src/kernel/` 中实现核心 Rust 模块

## 影响
- **新目录**：`client/`, `admin-backend/`, `execution-env/`
- **新配置**：`.gitmodules` 用于管理后台
- **影响的规范**：无（初始设置）

## 新增需求

### 需求：管理后台子模块
系统应将管理后台代码组织在单独的 git 仓库中，并将其作为子模块包含在 `admin-backend/` 中
- **组件**：
    - `audit-center/`：日志同步、对账引擎、审计仓库
    - `sec-pipeline/`：供应链扫描、静态代码审计、国密签名机
    - `iam/`：身份访问管理
    - `store-svr/`：技能商店服务器
    - `policy-engine/`：策略管理

### 需求：客户端终端结构
系统应将客户端终端组织为位于 `client/` 的 Tauri 应用程序
- **UI 层 (`client/src/`)**：
    - `login/`：登录界面
    - `chat/`：聊天界面
    - `store/`：商店界面
    - `approval/`：审批弹窗
- **安全微内核 (`client/src-tauri/src/kernel/`)**：
    - `validator/`：SM2 签名验证器
    - `dlp-engine/`：数据防泄漏引擎
    - `wasm-box/`：WASM 运行时沙箱
    - `audit-proxy/`：离线审计代理
    - `sync-manager/`：断点续传管理器

### 需求：执行环境结构
系统应将执行环境组织在 `execution-env/` 中
- **组件**：
    - `mcp-bridge/`：MCP 协议网桥
    - `llm-gateway/`：模型代理和本地 LLM 接口
    - `openclaw-skills/`：生态系统插件