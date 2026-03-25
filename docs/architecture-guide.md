# 架构指南

本文档包含项目架构的详细说明和代码示例。  
核心规则见 `AGENTS.md`，本文档是其补充。

---

## 项目架构关系

```
主项目 (src/)
├── 核心功能实现
├── CLI 命令 (src/cli/)
├── Web Gateway API (src/channels/web/handlers/)
└── 共享 Crates (crates/)

Desktop Client (desktop-client/)
├── Tauri 应用
├── 调用 Web Gateway API
└── 依赖共享 Crates

Admin Backend (admin-backend/)
├── 管理后台应用
├── 依赖主项目 crate
└── 依赖共享 Crates
```

---

## Desktop Client 功能实现流程

```
1. 需求分析
   ↓
2. 检查 src/channels/web/handlers/ 是否有对应 API
   ├─ 有 → 创建 Tauri 命令包装器 → 完成
   └─ 无 ↓
3. 检查 crates/ 是否有可复用模块
   ├─ 有 → 依赖该 crate → 完成
   └─ 无 ↓
4. 检查主项目 src/ 是否有相关功能
   ├─ 有 → 考虑提取为共享 crate 或添加 Web API
   └─ 无 → 评估是否为 Desktop Client 特有功能
       ├─ 是 → 独立实现
       └─ 否 → 在主项目中实现，然后复用
```

## Admin Backend 功能实现流程

```
1. 需求分析
   ↓
2. 检查主项目 src/ 是否有对应功能
   ├─ 有 → 通过 ironclaw crate 依赖 → 完成
   └─ 无 ↓
3. 检查 crates/ 是否有可复用模块
   ├─ 有 → 依赖该 crate → 完成
   └─ 无 ↓
4. 评估是否需要共享
   ├─ 需要 → 创建共享 crate
   └─ 不需要 → 独立实现
```

---

## 代码复用示例

### Desktop Client 复用 Web API ✅

```rust
#[tauri::command]
pub async fn get_logs(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<LogEntry>, String> {
    let url = format!("{}/api/logs", state.gateway_url);
    let response = state.http_client
        .get(&url)
        .query(&[("limit", limit.unwrap_or(200))])
        .send().await.map_err(|e| e.to_string())?;
    response.json().await.map_err(|e| e.to_string())
}
```

### Admin Backend 复用主项目功能 ✅

```rust
use ironclaw::agent::Agent;
use ironclaw::db::Database;

pub async fn create_agent(db: &Database) -> Result<Agent, Error> {
    Agent::new(db).await
}
```

### 重复实现 ❌

```rust
// 错误：重新实现密码哈希（ironclaw_auth 已有）
pub fn hash_password(password: &str) -> String { ... }

// 错误：重新实现日志查询（Web API 已有）
pub async fn query_logs() -> Vec<LogEntry> { ... }
```

---

## 共享 Crate 创建规范

参考 `crates/ironclaw_auth/` 的模式：

```
crates/your_crate/
├── Cargo.toml
├── src/
│   ├── lib.rs       # 公共 API
│   ├── error.rs     # 错误类型
│   └── ...
└── tests/           # 集成测试（可选）
```

关键要求：
- 共享 crate 不应依赖应用层代码
- API 变更需同步更新所有使用方
- 应用层通过 `impl From<crate::Error> for AppError` 映射错误类型
- 更新 workspace `Cargo.toml`

---

## 可复用的 Web Gateway API 端点

实现 Desktop Client 功能前必须检查以下 handler 是否已存在于 `src/channels/web/handlers/`：

- Memory: `memory_tree_handler`, `memory_list_handler`, `memory_read_handler`, `memory_write_handler`, `memory_search_handler`
- Chat: `chat_send_handler`, `chat_history_handler`, `chat_threads_handler`, `chat_new_thread_handler`, `chat_events_handler`
- Jobs: `jobs_list_handler`, `jobs_detail_handler`, `jobs_cancel_handler`, `jobs_restart_handler`, `jobs_events_handler`
- Extensions: `extensions_list_handler`, `extensions_install_handler`, `extensions_uninstall_handler`
- Skills: `skills_list_handler`, `skills_install_handler`, `skills_uninstall_handler`
- Routines: `routines_list_handler`, `routines_create_handler`, `routines_delete_handler`, `routines_trigger_handler`
- Logs: `logs_events_handler`, `logs_level_handler`
- Approval: `approve_operation_handler`, `deny_operation_handler`

---

## Tauri 命令注册机制

### 集中注册表宏

`desktop-client/src/lib.rs` 中定义 `all_tauri_commands!()` 宏，`main.rs` 和测试共用同一份注册表：

```rust
#[macro_export]
macro_rules! all_tauri_commands {
    () => {
        tauri::generate_handler![
            desktop_client::ipc::send_chat_message,
            // ... 所有命令
        ]
    };
}
```

### 契约测试原理

```
普通单元测试路径：
  测试代码 → 直接调用 Rust 函数 → 绕过 IPC 路由

真实运行时路径：
  前端 invoke('cmd') → Tauri IPC → invoke_handler 路由表 → Rust 函数

契约测试路径：
  测试代码 → Tauri 测试运行时 IPC → invoke_handler 路由表 → Rust 函数
```

参考：`desktop-client/tests/tauri_command_contract_tests.rs`
