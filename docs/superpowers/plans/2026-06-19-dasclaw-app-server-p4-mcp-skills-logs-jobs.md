# Dasclaw App Server P4 MCP Skills Logs Jobs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 P4 的 MCP、skills、logs、jobs 从“已有底座或声明”推进到 app-server 拥有的协议、健康状态和最小可用服务面。

**Architecture:** `dasclaw_app_server_protocol` 继续做 JSON-RPC wire/type owner；`dasclaw_app_server` 做本地 control-plane owner，并通过小而明确的 service trait 接入 MCP、skills、logs、jobs。MCP 与 skills 尽量对齐 Codex app-server v2 method/event shape；logs 与 jobs 是 Dasclaw native control-plane 能力，不伪装成 Codex 产品域。

**Tech Stack:** Rust 2024, serde JSON-RPC, `dasclaw_app_server_protocol`, `dasclaw_app_server`, `dasclaw_mcp`, `dasclaw_observability`, `dasclaw_runtime`, `cargo nextest`.

---

## Scope Check

P4 实际包含四个相对独立的 service owner：MCP、skills、logs、jobs。为了让每一步都能独立验证，本计划按 stacked slices 执行：

- P4-A：协议 DTO、capability helper、service trait seam。
- P4-B：logs source 接入 `log/entry` notification。
- P4-C：jobs host 的 native `jobs/list` / `jobs/read` control-plane。
- P4-D：skills registry 的 `skills/list` / `skills/config/write` / `skills/changed`。
- P4-E：MCP registry/status/reload 的最小 app-server service。
- P4-F：MCP tool-call/OAuth/resource-read 的有副作用路径，放在 P4 末尾，先由 fail-safe tests 锁边界；真实 session/OAuth owner 未接线前不广告 implemented capability。

不在本计划内：

- `fs/*`、`fs/changed`、standalone `command/exec*`：属于 P5。
- account、plugin marketplace、app list、feedback、external agent import、Windows sandbox、realtime audio：属于 P6/product decision。
- 直接依赖已废弃 `desktop-client`。它只能作为参考；当前目标代码是 `crates/*`、`desktop-app`、`dasclaw_app_server`。

## P4 Analysis

P4 不是“补几个 method 常量”。它要求 app-server 成为服务 owner：它要能说清服务是否 ready、能路由请求、能发事件、能失败时 fail-safe，而不是把 `protocol/schema` 里 declared future 的名字改成 implemented。

实施前证据归纳：

- MCP：`crates/dasclaw_mcp` 已有 `McpServerConfig`、OAuth helpers、`McpSessionManager`、`McpClient`、`create_client_from_config`、`McpToolExecutor`；app-server 仍显示 `ServiceName::Mcp` disabled，router 无 `mcpServer/*`。
- Skills：`crates/dasclaw_protocol/src/protocol.rs` 有 `SkillMetadata` / `SkillsListEntry` 等协议词汇，旧 `desktop-client/ironclaw/src/skills/*` 有参考 registry；app-server 仍显示 `ServiceName::Skills` disabled，当前不能直接依赖旧客户端。
- Logs：`crates/dasclaw_observability` 有 `Observer` / `ObserverEvent` / `LogObserver`；`dasclaw_app_server_protocol` 已有 `LogEntryEvent` 与 `log/entry` event schema；app-server 没有 log source。
- Jobs：`crates/dasclaw_runtime/src/job.rs` 有 `JobState` / `StateTransition`，`job_context.rs` 有 `JobContextCore`；app-server 仍显示 `ServiceName::Jobs` disabled，没有 job host/list/read owner。

优先顺序：

1. 先加 protocol DTO + service trait seam，让 Noop host 继续 honest disabled。
2. 先接 logs 和 jobs read-only/status，因为它们不需要外部网络或 OAuth。
3. 再接 skills list/config，因为涉及本地文件扫描和 enable/disable state。
4. 最后接 MCP status/tool-call/reload/OAuth/resource，因为它可能触发网络、进程和 token 边界。

## Startup 4 Questions

| Question | Answer | Required action |
|---|---|---|
| 是否新增模块 / crate / 文件？ | 是。本计划文件新增；实施会新增 app-server service modules。 | 已先做语义搜索；实施 commit message 必须写 `已检查 P4 service owner 是否已有，结论：...`。 |
| 结论是否包含否定语？ | 是。会判断当前 P4 仍未由 app-server 接线。 | 使用 Level 1 + LSP + `rg` 证据；测试必须证明 Noop host 不冒充 implemented。 |
| 是否跨项目对账？ | 是。涉及 `codex-cli-main` schema、`crates/*`、`desktop-app`、旧 `desktop-client` 参考。 | 只把 `desktop-client` 当参考，不作为新依赖。 |
| 是否写架构对账类文档？ | 是。这是从 gap matrix 派生的实施计划。 | 文件里保留 evidence vs decision，避免把 capability metadata 当实际服务。 |

Evidence gathered before this plan (implementation-start snapshot, not current completion status):

- Level 1 semantic search:
  - `native skills registry list skills config write skill metadata app server capability` 命中 `SkillMetadata`、`SkillsListEntry`、`SkillScope`。
  - `observability log observer log entry app server notification bus service health` 命中 `ObservabilityEvent`、`AuditLogHook`、`LogObserver` 相关节点。
  - `dasclaw_mcp McpToolExecutor McpServerConfig OAuthSession ResourceRead tool call progress startup status` 命中 `McpToolCallEndEvent`、`McpServerRefreshConfig`、MCP startup serialization tests。
  - `dasclaw_runtime job state job_context JobContext JobHandle JobStatus queued running completed failed` 的语义结果噪声较高；不作为 job 结论的唯一证据。
- Level 2 symbol layer:
  - 本机 `lsp-mcp execute_lsp` 可用；`workspace_symbols` 定位到 `McpServerRefreshConfig`、`SkillsListEntry`、`SkillMetadata`、`LogEntryEvent`、`JobContext`、`JobState`。
- Level 3 literal layer:
  - Codex schema: `ClientRequest.ts` 包含 `skills/list`、`skills/config/write`、`mcpServer/oauth/login`、`config/mcpServer/reload`、`mcpServerStatus/list`、`mcpServer/resource/read`、`mcpServer/tool/call`。
  - Codex schema: `ServerNotification.ts` 包含 `skills/changed`、`item/mcpToolCall/progress`、`mcpServer/oauthLogin/completed`、`mcpServer/startupStatus/updated`。
  - Dasclaw protocol: `CapabilityMatrix::phase_one()` 把 `logs`、`jobs`、`skills`、`mcp` 标为 `declared_future_capability`。
  - Dasclaw app-server: `service_health()` 明确返回 logs/jobs/skills/mcp disabled，`route_json_rpc()` 目前不路由 P4 methods。

## File Structure

Modify:

- `crates/dasclaw_app_server_protocol/src/lib.rs`
  - 增加 P4 method/event 常量。
  - 增加 skills、MCP、jobs 的 app-server wire DTO。
  - 增加 `P4ServiceAvailability` 与 `CapabilityMatrix::with_p4_services(...)`。
  - 增加 protocol serialization/capability tests。

- `crates/dasclaw_app_server/src/lib.rs`
  - 引入 P4 modules。
  - `AppServer` 持有 `P4Services`。
  - `route_json_rpc()` 路由 P4 methods。
  - `service_health()` 从 P4 service readiness 生成 logs/jobs/skills/mcp health。
  - `capabilities()` / `initialize()` 返回按 service readiness 计算的 capability matrix。

- `crates/dasclaw_app_server/Cargo.toml`
  - 增加 `dasclaw_mcp`、`dasclaw_observability`。
  - 若 P4-D 选择把 skills parser 抽到新 crate，再在对应 sub-PR 添加 `dasclaw_skills`。

Create:

- `crates/dasclaw_app_server/src/p4_services.rs`
  - P4 service trait、Noop implementations、test fakes。

- `crates/dasclaw_app_server/src/log_service.rs`
  - `dasclaw_observability::Observer` 到 app-server `LogEntryEvent` 的 bridge。

- `crates/dasclaw_app_server/src/job_service.rs`
  - Native job host DTO mapping 和 in-memory test host。

- `crates/dasclaw_app_server/src/skills_service.rs`
  - Skills list/config service。首版可以只接 service trait 与 deterministic in-memory/file-backed registry；不能依赖 `desktop-client`.

- `crates/dasclaw_app_server/src/mcp_service.rs`
  - MCP status/reload/tool-call/OAuth/resource service adapter。

Optional new crate for P4-D:

- `crates/dasclaw_skills/src/lib.rs`
  - 仅当 P4-D 需要复用到 CLI/runtime 之外时创建。创建前再次运行 semantic search，commit message 写明复用检查结论。

## Definition of Done

- `AppServer::new()` 默认仍把 P4 services 报告为 disabled/declared，不冒充 ready。
- 注入 ready P4 services 后，`capabilities/list` 和 `health/check` 同时显示对应 implemented/ready；MCP 需按 method/event availability 分开广告。
- `protocol/schema` 包含 P4 methods/events，并且 implemented capability methods 全部可路由；fail-safe 但未真正接线的 MCP effectful methods 不得被广告为 implemented。
- Logs：app-server 能把 structured event 发成 `log/entry`，并在进入 `NotificationBus` 前使用有界缓冲。
- Jobs：`jobs/list` / `jobs/read` 能返回 native job snapshot，非法 job id 返回 structured JSON-RPC error。
- Skills：`skills/list` 返回 Codex-shaped `SkillsListResponse`；`skills/config/write` 修改 enabled state 并发 `skills/changed`。
- MCP：真实 service 广告并实现 `mcpServerStatus/list`、`config/mcpServer/reload` 与 startup status event；`mcpServer/tool/call`、OAuth、resource-read 保持 fail-safe 且不泄露 token，但在 session/OAuth owner 接线前不作为 implemented capability 暴露。
- P5/P6 domains 仍在 compatibility opt-out 中，不因为 P4 合入而被误报 implemented。

## Task 1: Lock P4 Protocol Contracts

**Files:**
- Modify: `crates/dasclaw_app_server_protocol/src/lib.rs`

- [x] **Step 1: Write failing protocol tests**

Add these tests inside the existing `#[cfg(test)] mod tests` in `crates/dasclaw_app_server_protocol/src/lib.rs`:

```rust
#[test]
fn p4_phase_one_keeps_services_declared_until_owner_is_wired() {
    let matrix = CapabilityMatrix::phase_one();

    assert_eq!(matrix.logs.status, CapabilityStatus::Declared);
    assert_eq!(matrix.jobs.status, CapabilityStatus::Declared);
    assert_eq!(matrix.skills.status, CapabilityStatus::Declared);
    assert_eq!(matrix.mcp.status, CapabilityStatus::Declared);
}

#[test]
fn p4_capability_helper_advertises_only_ready_services() {
    let matrix = CapabilityMatrix::phase_one().with_p4_services(P4ServiceAvailability {
        logs: true,
        jobs: true,
        skills: false,
        mcp: P4McpServiceAvailability {
            status_list: true,
            reload: true,
            tool_call: true,
            resource_read: true,
            oauth_login: true,
            tool_call_progress_events: true,
            oauth_login_completed_events: true,
            startup_status_events: true,
        },
    });

    assert_eq!(matrix.logs.status, CapabilityStatus::Implemented);
    assert_eq!(matrix.logs.events, vec![event::LOG_ENTRY.to_string()]);
    assert_eq!(matrix.jobs.status, CapabilityStatus::Implemented);
    assert_eq!(
        matrix.jobs.methods,
        vec![method::JOBS_LIST.to_string(), method::JOBS_READ.to_string()]
    );
    assert_eq!(matrix.skills.status, CapabilityStatus::Declared);
    assert_eq!(matrix.mcp.status, CapabilityStatus::Implemented);
    assert_eq!(
        matrix.mcp.methods,
        vec![
            method::MCP_SERVER_OAUTH_LOGIN.to_string(),
            method::CONFIG_MCP_SERVER_RELOAD.to_string(),
            method::MCP_SERVER_STATUS_LIST.to_string(),
            method::MCP_SERVER_RESOURCE_READ.to_string(),
            method::MCP_SERVER_TOOL_CALL.to_string(),
        ]
    );
    assert_eq!(
        matrix.mcp.events,
        vec![
            event::ITEM_MCP_TOOL_CALL_PROGRESS.to_string(),
            event::MCP_SERVER_OAUTH_LOGIN_COMPLETED.to_string(),
            event::MCP_SERVER_STARTUP_STATUS_UPDATED.to_string(),
        ]
    );
}

#[test]
fn skills_list_response_serializes_codex_shape() {
    let response = SkillsListResponse {
        data: vec![SkillsListEntry {
            cwd: "/repo".into(),
            skills: vec![SkillMetadata {
                name: "review".into(),
                description: "Review local code".into(),
                short_description: Some("Code review".into()),
                interface: Some(SkillInterface {
                    display_name: Some("Review".into()),
                    short_description: Some("Code review".into()),
                    icon_small: None,
                    icon_large: None,
                    brand_color: None,
                    default_prompt: None,
                }),
                dependencies: Some(SkillDependencies { tools: vec![] }),
                path: "/repo/.codex/skills/review/SKILL.json".into(),
                scope: SkillScope::Repo,
                enabled: true,
            }],
            errors: vec![],
        }],
    };

    let value = serde_json::to_value(response).expect("skills/list response serializes");
    assert_eq!(value["data"][0]["cwd"], "/repo");
    assert_eq!(value["data"][0]["skills"][0]["name"], "review");
    assert_eq!(value["data"][0]["skills"][0]["shortDescription"], "Code review");
    assert_eq!(value["data"][0]["skills"][0]["scope"], "repo");
    assert_eq!(value["data"][0]["skills"][0]["enabled"], true);
}

#[test]
fn mcp_status_response_serializes_codex_shape() {
    let response = ListMcpServerStatusResponse {
        data: vec![McpServerStatus {
            name: "github".into(),
            tools: serde_json::json!({
                "list_issues": {
                    "name": "list_issues",
                    "description": "List issues",
                    "inputSchema": {"type": "object"}
                }
            }),
            resources: vec![],
            resource_templates: vec![],
            auth_status: McpAuthStatus::OAuth,
        }],
        next_cursor: None,
    };

    let value = serde_json::to_value(response).expect("mcp status response serializes");
    assert_eq!(value["data"][0]["name"], "github");
    assert_eq!(value["data"][0]["authStatus"], "oAuth");
    assert_eq!(value["data"][0]["tools"]["list_issues"]["name"], "list_issues");
    assert!(value["nextCursor"].is_null());
}

#[test]
fn native_job_list_response_serializes_snake_case_states() {
    let response = JobListResponse {
        data: vec![JobSnapshot {
            job_id: "job_1".into(),
            title: "Run audit".into(),
            description: "Audit repository".into(),
            state: JobSnapshotState::InProgress,
            created_at: "2026-06-19T00:00:00Z".into(),
            updated_at: Some("2026-06-19T00:01:00Z".into()),
            thread_id: Some("thread_1".into()),
        }],
        next_cursor: None,
    };

    let value = serde_json::to_value(response).expect("job list response serializes");
    assert_eq!(value["data"][0]["jobId"], "job_1");
    assert_eq!(value["data"][0]["state"], "in_progress");
}
```

- [x] **Step 2: Run the failing protocol tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol p4_phase_one_keeps_services_declared_until_owner_is_wired p4_capability_helper_advertises_only_ready_services skills_list_response_serializes_codex_shape mcp_status_response_serializes_codex_shape native_job_list_response_serializes_snake_case_states
```

Expected: fail to compile with missing P4 constants/types such as `P4ServiceAvailability`, `method::JOBS_LIST`, `SkillsListResponse`, and `ListMcpServerStatusResponse`.

- [x] **Step 3: Add protocol constants, DTOs, and helper**

Add these constants near the existing `method` and `event` modules:

```rust
pub mod method {
    // existing constants stay above
    pub const SKILLS_LIST: &str = "skills/list";
    pub const SKILLS_CONFIG_WRITE: &str = "skills/config/write";
    pub const MCP_SERVER_OAUTH_LOGIN: &str = "mcpServer/oauth/login";
    pub const CONFIG_MCP_SERVER_RELOAD: &str = "config/mcpServer/reload";
    pub const MCP_SERVER_STATUS_LIST: &str = "mcpServerStatus/list";
    pub const MCP_SERVER_RESOURCE_READ: &str = "mcpServer/resource/read";
    pub const MCP_SERVER_TOOL_CALL: &str = "mcpServer/tool/call";
    pub const JOBS_LIST: &str = "jobs/list";
    pub const JOBS_READ: &str = "jobs/read";
}

pub mod event {
    // existing constants stay above
    pub const SKILLS_CHANGED: &str = "skills/changed";
    pub const ITEM_MCP_TOOL_CALL_PROGRESS: &str = "item/mcpToolCall/progress";
    pub const MCP_SERVER_OAUTH_LOGIN_COMPLETED: &str = "mcpServer/oauthLogin/completed";
    pub const MCP_SERVER_STARTUP_STATUS_UPDATED: &str = "mcpServer/startupStatus/updated";
}
```

Add the P4 helper near `CapabilityMatrix`:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct P4ServiceAvailability {
    pub logs: bool,
    pub jobs: bool,
    pub skills: bool,
    pub mcp: P4McpServiceAvailability,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct P4McpServiceAvailability {
    pub status_list: bool,
    pub reload: bool,
    pub tool_call: bool,
    pub resource_read: bool,
    pub oauth_login: bool,
    pub tool_call_progress_events: bool,
    pub oauth_login_completed_events: bool,
    pub startup_status_events: bool,
}

impl P4McpServiceAvailability {
    #[must_use]
    pub fn methods(&self) -> Vec<&'static str> {
        let mut methods = Vec::new();
        if self.oauth_login {
            methods.push(method::MCP_SERVER_OAUTH_LOGIN);
        }
        if self.reload {
            methods.push(method::CONFIG_MCP_SERVER_RELOAD);
        }
        if self.status_list {
            methods.push(method::MCP_SERVER_STATUS_LIST);
        }
        if self.resource_read {
            methods.push(method::MCP_SERVER_RESOURCE_READ);
        }
        if self.tool_call {
            methods.push(method::MCP_SERVER_TOOL_CALL);
        }
        methods
    }

    #[must_use]
    pub fn events(&self) -> Vec<&'static str> {
        let mut events = Vec::new();
        if self.tool_call_progress_events {
            events.push(event::ITEM_MCP_TOOL_CALL_PROGRESS);
        }
        if self.oauth_login_completed_events {
            events.push(event::MCP_SERVER_OAUTH_LOGIN_COMPLETED);
        }
        if self.startup_status_events {
            events.push(event::MCP_SERVER_STARTUP_STATUS_UPDATED);
        }
        events
    }
}

impl CapabilityMatrix {
    #[must_use]
    pub fn with_p4_services(mut self, availability: P4ServiceAvailability) -> Self {
        if availability.logs {
            self.logs = Capability::implemented("logs", &[], &[event::LOG_ENTRY]);
        }
        if availability.jobs {
            self.jobs = Capability::implemented(
                "jobs",
                &[method::JOBS_LIST, method::JOBS_READ],
                &[],
            );
        }
        if availability.skills {
            self.skills = Capability::implemented(
                "skills",
                &[method::SKILLS_LIST, method::SKILLS_CONFIG_WRITE],
                &[event::SKILLS_CHANGED],
            );
        }
        let mcp_methods = availability.mcp.methods();
        let mcp_events = availability.mcp.events();
        if !mcp_methods.is_empty() || !mcp_events.is_empty() {
            self.mcp = Capability::implemented("mcp", &mcp_methods, &mcp_events);
        }
        self
    }
}
```

Add P4 DTOs below existing request/response structs:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsListParams {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cwds: Vec<String>,
    #[serde(default)]
    pub force_reload: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_cwd_extra_user_roots: Option<Vec<SkillsListExtraRootsForCwd>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsListExtraRootsForCwd {
    pub cwd: String,
    pub extra_user_roots: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsConfigWriteParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsConfigWriteResponse {
    pub changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillScope {
    User,
    Repo,
    System,
    Admin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface: Option<SkillInterface>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<SkillDependencies>,
    pub path: String,
    pub scope: SkillScope,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInterface {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_small: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_large: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brand_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_prompt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDependencies {
    pub tools: Vec<SkillToolDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillToolDependency {
    #[serde(rename = "type")]
    pub kind: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillErrorInfo {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsListEntry {
    pub cwd: String,
    pub skills: Vec<SkillMetadata>,
    pub errors: Vec<SkillErrorInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsListResponse {
    pub data: Vec<SkillsListEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsChangedNotification {}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListMcpServerStatusParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<McpServerStatusDetail>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpServerStatusDetail {
    #[serde(rename = "full")]
    Full,
    #[serde(rename = "toolsAndAuthOnly")]
    ToolsAndAuthOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpAuthStatus {
    #[serde(rename = "unsupported")]
    Unsupported,
    #[serde(rename = "notLoggedIn")]
    NotLoggedIn,
    #[serde(rename = "bearerToken")]
    BearerToken,
    #[serde(rename = "oAuth")]
    OAuth,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerStatus {
    pub name: String,
    pub tools: serde_json::Value,
    pub resources: Vec<serde_json::Value>,
    pub resource_templates: Vec<serde_json::Value>,
    pub auth_status: McpAuthStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListMcpServerStatusResponse {
    pub data: Vec<McpServerStatus>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerToolCallParams {
    pub thread_id: String,
    pub server: String,
    pub tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerToolCallResponse {
    pub result: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResourceReadParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    pub server: String,
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResourceReadResponse {
    pub contents: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerOauthLoginParams {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerOauthLoginResponse {
    pub authorization_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerOauthLoginCompletedNotification {
    pub name: String,
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerStatusUpdatedNotification {
    pub name: String,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolCallProgressNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobReadParams {
    pub job_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobSnapshotState {
    Pending,
    InProgress,
    Completed,
    Submitted,
    Accepted,
    Failed,
    Stuck,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSnapshot {
    pub job_id: String,
    pub title: String,
    pub description: String,
    pub state: JobSnapshotState,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobListResponse {
    pub data: Vec<JobSnapshot>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobReadResponse {
    pub job: JobSnapshot,
}
```

Extend `phase_one_methods()` with these entries:

```rust
MethodSchema::new(
    method::SKILLS_LIST,
    "skills",
    Some("SkillsListParams"),
    "SkillsListResponse",
    true,
),
MethodSchema::new(
    method::SKILLS_CONFIG_WRITE,
    "skills",
    Some("SkillsConfigWriteParams"),
    "SkillsConfigWriteResponse",
    true,
),
MethodSchema::new(
    method::MCP_SERVER_OAUTH_LOGIN,
    "mcp",
    Some("McpServerOauthLoginParams"),
    "McpServerOauthLoginResponse",
    true,
),
MethodSchema::new(
    method::CONFIG_MCP_SERVER_RELOAD,
    "mcp",
    None,
    "ListMcpServerStatusResponse",
    true,
),
MethodSchema::new(
    method::MCP_SERVER_STATUS_LIST,
    "mcp",
    Some("ListMcpServerStatusParams"),
    "ListMcpServerStatusResponse",
    true,
),
MethodSchema::new(
    method::MCP_SERVER_RESOURCE_READ,
    "mcp",
    Some("McpResourceReadParams"),
    "McpResourceReadResponse",
    true,
),
MethodSchema::new(
    method::MCP_SERVER_TOOL_CALL,
    "mcp",
    Some("McpServerToolCallParams"),
    "McpServerToolCallResponse",
    true,
),
MethodSchema::new(
    method::JOBS_LIST,
    "jobs",
    Some("JobListParams"),
    "JobListResponse",
    true,
),
MethodSchema::new(
    method::JOBS_READ,
    "jobs",
    Some("JobReadParams"),
    "JobReadResponse",
    true,
),
```

Extend `phase_one_events()` with these entries:

```rust
EventSchema::new(event::SKILLS_CHANGED, "skills", "SkillsChangedNotification"),
EventSchema::new(
    event::ITEM_MCP_TOOL_CALL_PROGRESS,
    "mcp",
    "McpToolCallProgressNotification",
),
EventSchema::new(
    event::MCP_SERVER_OAUTH_LOGIN_COMPLETED,
    "mcp",
    "McpServerOauthLoginCompletedNotification",
),
EventSchema::new(
    event::MCP_SERVER_STARTUP_STATUS_UPDATED,
    "mcp",
    "McpServerStatusUpdatedNotification",
),
```

Keep these schemas present even when capabilities are declared so clients can discover unsupported-but-known surface.

- [x] **Step 4: Run protocol tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol p4_phase_one_keeps_services_declared_until_owner_is_wired p4_capability_helper_advertises_only_ready_services skills_list_response_serializes_codex_shape mcp_status_response_serializes_codex_shape native_job_list_response_serializes_snake_case_states
```

Expected: all five tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/dasclaw_app_server_protocol/src/lib.rs
git commit -m "feat(app-server): define P4 protocol contracts" -m "已检查 P4 service owner 是否已有，结论：protocol 只有 declared future 与部分底座词汇，缺少 app-server P4 wire DTO/helper。"
```

## Task 2: Add P4 Service Seams And Honest Health

**Files:**
- Create: `crates/dasclaw_app_server/src/p4_services.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`

- [x] **Step 1: Write failing app-server tests**

Add these tests inside the existing `#[cfg(test)] mod tests` in `crates/dasclaw_app_server/src/lib.rs`:

```rust
#[test]
fn p4_noop_services_remain_disabled_and_declared() {
    let mut server = AppServer::new();

    let capabilities = server.capabilities();
    assert_eq!(
        capabilities.capabilities.logs.status,
        CapabilityStatus::Declared
    );
    assert_eq!(
        capabilities.capabilities.jobs.status,
        CapabilityStatus::Declared
    );
    assert_eq!(
        capabilities.capabilities.skills.status,
        CapabilityStatus::Declared
    );
    assert_eq!(
        capabilities.capabilities.mcp.status,
        CapabilityStatus::Declared
    );

    let health = server.health_check(HealthCheckParams {
        include_details: true,
    });
    assert!(health.services.iter().any(|service| {
        service.service == ServiceName::Logs && service.status == ServiceStatus::Disabled
    }));
    assert!(health.services.iter().any(|service| {
        service.service == ServiceName::Jobs && service.status == ServiceStatus::Disabled
    }));
    assert!(health.services.iter().any(|service| {
        service.service == ServiceName::Skills && service.status == ServiceStatus::Disabled
    }));
    assert!(health.services.iter().any(|service| {
        service.service == ServiceName::Mcp && service.status == ServiceStatus::Disabled
    }));
}

#[test]
fn p4_ready_services_drive_capabilities_and_health_together() {
    let services = p4_services::P4Services::for_tests(
        p4_services::TestLogService::ready(),
        p4_services::TestJobService::ready(vec![]),
        p4_services::TestSkillsService::ready(vec![]),
        p4_services::TestMcpService::ready(vec![]),
    );
    let mut server = AppServer::new().with_p4_services(services);

    let capabilities = server.capabilities();
    assert_eq!(
        capabilities.capabilities.logs.status,
        CapabilityStatus::Implemented
    );
    assert_eq!(
        capabilities.capabilities.jobs.status,
        CapabilityStatus::Implemented
    );
    assert_eq!(
        capabilities.capabilities.skills.status,
        CapabilityStatus::Implemented
    );
    assert_eq!(
        capabilities.capabilities.mcp.status,
        CapabilityStatus::Implemented
    );

    let health = server.health_check(HealthCheckParams {
        include_details: true,
    });
    for service_name in [
        ServiceName::Logs,
        ServiceName::Jobs,
        ServiceName::Skills,
        ServiceName::Mcp,
    ] {
        assert!(health.services.iter().any(|service| {
            service.service == service_name && service.status == ServiceStatus::Ready
        }));
    }
}
```

- [x] **Step 2: Run the failing tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server p4_noop_services_remain_disabled_and_declared p4_ready_services_drive_capabilities_and_health_together
```

Expected: fail to compile because `p4_services` and `AppServer::with_p4_services` do not exist.

- [x] **Step 3: Add service traits and test fakes**

Create `crates/dasclaw_app_server/src/p4_services.rs`:

```rust
use std::sync::Arc;

use dasclaw_app_server_protocol::{
    ErrorCode, JobListParams, JobListResponse, JobReadParams, JobReadResponse, ListMcpServerStatusParams,
    ListMcpServerStatusResponse, McpResourceReadParams, McpResourceReadResponse,
    McpServerOauthLoginParams, McpServerOauthLoginResponse, McpServerToolCallParams,
    McpServerToolCallResponse, P4ServiceAvailability, ServiceHealth, ServiceName,
    SkillsConfigWriteParams, SkillsConfigWriteResponse, SkillsListParams, SkillsListResponse,
};

use crate::AppServerError;

pub trait LogService: Send + Sync {
    fn health(&self) -> ServiceHealth;
    fn is_ready(&self) -> bool {
        matches!(
            self.health().status,
            dasclaw_app_server_protocol::ServiceStatus::Ready
        )
    }
}

pub trait JobService: Send + Sync {
    fn health(&self) -> ServiceHealth;
    fn list(&self, params: JobListParams) -> Result<JobListResponse, AppServerError>;
    fn read(&self, params: JobReadParams) -> Result<JobReadResponse, AppServerError>;
}

pub trait SkillsService: Send + Sync {
    fn health(&self) -> ServiceHealth;
    fn list(&self, params: SkillsListParams) -> Result<SkillsListResponse, AppServerError>;
    fn write_config(
        &self,
        params: SkillsConfigWriteParams,
    ) -> Result<SkillsConfigWriteResponse, AppServerError>;
}

pub trait McpService: Send + Sync {
    fn health(&self) -> ServiceHealth;
    fn list_status(
        &self,
        params: ListMcpServerStatusParams,
    ) -> Result<ListMcpServerStatusResponse, AppServerError>;
    fn reload(&self) -> Result<ListMcpServerStatusResponse, AppServerError>;
    fn call_tool(
        &self,
        params: McpServerToolCallParams,
    ) -> Result<McpServerToolCallResponse, AppServerError>;
    fn read_resource(
        &self,
        params: McpResourceReadParams,
    ) -> Result<McpResourceReadResponse, AppServerError>;
    fn oauth_login(
        &self,
        params: McpServerOauthLoginParams,
    ) -> Result<McpServerOauthLoginResponse, AppServerError>;
}

#[derive(Clone)]
pub struct P4Services {
    pub logs: Arc<dyn LogService>,
    pub jobs: Arc<dyn JobService>,
    pub skills: Arc<dyn SkillsService>,
    pub mcp: Arc<dyn McpService>,
}

impl Default for P4Services {
    fn default() -> Self {
        Self {
            logs: Arc::new(NoopLogService),
            jobs: Arc::new(NoopJobService),
            skills: Arc::new(NoopSkillsService),
            mcp: Arc::new(NoopMcpService),
        }
    }
}

impl P4Services {
    pub fn health(&self) -> Vec<ServiceHealth> {
        vec![
            self.logs.health(),
            self.jobs.health(),
            self.skills.health(),
            self.mcp.health(),
        ]
    }

    pub fn availability(&self) -> P4ServiceAvailability {
        P4ServiceAvailability {
            logs: self.logs.is_ready(),
            jobs: matches!(
                self.jobs.health().status,
                dasclaw_app_server_protocol::ServiceStatus::Ready
            ),
            skills: matches!(
                self.skills.health().status,
                dasclaw_app_server_protocol::ServiceStatus::Ready
            ),
            mcp: if self.mcp.is_ready() {
                self.mcp.availability()
            } else {
                dasclaw_app_server_protocol::P4McpServiceAvailability::default()
            },
        }
    }

    #[cfg(test)]
    pub fn for_tests(
        logs: impl LogService + 'static,
        jobs: impl JobService + 'static,
        skills: impl SkillsService + 'static,
        mcp: impl McpService + 'static,
    ) -> Self {
        Self {
            logs: Arc::new(logs),
            jobs: Arc::new(jobs),
            skills: Arc::new(skills),
            mcp: Arc::new(mcp),
        }
    }
}

struct NoopLogService;
struct NoopJobService;
struct NoopSkillsService;
struct NoopMcpService;

impl LogService for NoopLogService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::disabled(ServiceName::Logs, "log source is not wired")
    }
}

impl JobService for NoopJobService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::disabled(ServiceName::Jobs, "job host is not wired")
    }

    fn list(&self, _params: JobListParams) -> Result<JobListResponse, AppServerError> {
        Err(AppServerError::service_unavailable(
            ErrorCode::CapabilityUnavailable,
            "jobs",
            "job host is not wired",
        ))
    }

    fn read(&self, _params: JobReadParams) -> Result<JobReadResponse, AppServerError> {
        Err(AppServerError::service_unavailable(
            ErrorCode::CapabilityUnavailable,
            "jobs",
            "job host is not wired",
        ))
    }
}

impl SkillsService for NoopSkillsService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::disabled(ServiceName::Skills, "skills registry is not wired")
    }

    fn list(&self, _params: SkillsListParams) -> Result<SkillsListResponse, AppServerError> {
        Err(AppServerError::service_unavailable(
            ErrorCode::CapabilityUnavailable,
            "skills",
            "skills registry is not wired",
        ))
    }

    fn write_config(
        &self,
        _params: SkillsConfigWriteParams,
    ) -> Result<SkillsConfigWriteResponse, AppServerError> {
        Err(AppServerError::service_unavailable(
            ErrorCode::CapabilityUnavailable,
            "skills",
            "skills registry is not wired",
        ))
    }
}

impl McpService for NoopMcpService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::disabled(ServiceName::Mcp, "MCP registry is not wired")
    }

    fn list_status(
        &self,
        _params: ListMcpServerStatusParams,
    ) -> Result<ListMcpServerStatusResponse, AppServerError> {
        Err(AppServerError::service_unavailable(
            ErrorCode::CapabilityUnavailable,
            "mcp",
            "MCP registry is not wired",
        ))
    }

    fn reload(&self) -> Result<ListMcpServerStatusResponse, AppServerError> {
        Err(AppServerError::service_unavailable(
            ErrorCode::CapabilityUnavailable,
            "mcp",
            "MCP registry is not wired",
        ))
    }

    fn call_tool(
        &self,
        _params: McpServerToolCallParams,
    ) -> Result<McpServerToolCallResponse, AppServerError> {
        Err(AppServerError::service_unavailable(
            ErrorCode::CapabilityUnavailable,
            "mcp",
            "MCP registry is not wired",
        ))
    }

    fn read_resource(
        &self,
        _params: McpResourceReadParams,
    ) -> Result<McpResourceReadResponse, AppServerError> {
        Err(AppServerError::service_unavailable(
            ErrorCode::CapabilityUnavailable,
            "mcp",
            "MCP registry is not wired",
        ))
    }

    fn oauth_login(
        &self,
        _params: McpServerOauthLoginParams,
    ) -> Result<McpServerOauthLoginResponse, AppServerError> {
        Err(AppServerError::service_unavailable(
            ErrorCode::CapabilityUnavailable,
            "mcp",
            "MCP registry is not wired",
        ))
    }
}
```

Add test fakes below the noop implementations, behind `#[cfg(test)]`:

```rust
#[cfg(test)]
#[derive(Clone)]
pub struct TestLogService {
    ready: bool,
}

#[cfg(test)]
impl TestLogService {
    pub fn ready() -> Self {
        Self { ready: true }
    }
}

#[cfg(test)]
impl LogService for TestLogService {
    fn health(&self) -> ServiceHealth {
        if self.ready {
            ServiceHealth::ready(ServiceName::Logs)
        } else {
            ServiceHealth::disabled(ServiceName::Logs, "test log service disabled")
        }
    }
}

#[cfg(test)]
#[derive(Clone)]
pub struct TestJobService {
    jobs: Vec<dasclaw_app_server_protocol::JobSnapshot>,
}

#[cfg(test)]
impl TestJobService {
    pub fn ready(jobs: Vec<dasclaw_app_server_protocol::JobSnapshot>) -> Self {
        Self { jobs }
    }
}

#[cfg(test)]
impl JobService for TestJobService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::ready(ServiceName::Jobs)
    }

    fn list(&self, _params: JobListParams) -> Result<JobListResponse, AppServerError> {
        Ok(JobListResponse {
            data: self.jobs.clone(),
            next_cursor: None,
        })
    }

    fn read(&self, params: JobReadParams) -> Result<JobReadResponse, AppServerError> {
        self.jobs
            .iter()
            .find(|job| job.job_id == params.job_id)
            .cloned()
            .map(|job| JobReadResponse { job })
            .ok_or_else(|| AppServerError::invalid_request("jobs", "job not found"))
    }
}
```

Add the full skills and MCP fakes below `TestJobService`:

```rust
#[cfg(test)]
#[derive(Clone)]
pub struct TestSkillsService {
    entries: Arc<std::sync::Mutex<Vec<dasclaw_app_server_protocol::SkillsListEntry>>>,
}

#[cfg(test)]
impl TestSkillsService {
    pub fn ready(entries: Vec<dasclaw_app_server_protocol::SkillsListEntry>) -> Self {
        Self {
            entries: Arc::new(std::sync::Mutex::new(entries)),
        }
    }
}

#[cfg(test)]
impl SkillsService for TestSkillsService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::ready(ServiceName::Skills)
    }

    fn list(&self, params: SkillsListParams) -> Result<SkillsListResponse, AppServerError> {
        let entries = self
            .entries
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        Ok(crate::skills_service::filter_entries_by_cwd(&entries, &params))
    }

    fn write_config(
        &self,
        params: SkillsConfigWriteParams,
    ) -> Result<SkillsConfigWriteResponse, AppServerError> {
        if params.path.is_none() && params.name.is_none() {
            return Err(AppServerError::invalid_request(
                "skills",
                "skills/config/write requires path or name",
            ));
        }
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        Ok(crate::skills_service::write_enabled_flag(&mut entries, &params))
    }
}

#[cfg(test)]
#[derive(Clone)]
pub struct TestMcpService {
    statuses: Vec<dasclaw_app_server_protocol::McpServerStatus>,
}

#[cfg(test)]
impl TestMcpService {
    pub fn ready(statuses: Vec<dasclaw_app_server_protocol::McpServerStatus>) -> Self {
        Self { statuses }
    }
}

#[cfg(test)]
impl McpService for TestMcpService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::ready(ServiceName::Mcp)
    }

    fn list_status(
        &self,
        _params: ListMcpServerStatusParams,
    ) -> Result<ListMcpServerStatusResponse, AppServerError> {
        Ok(ListMcpServerStatusResponse {
            data: self.statuses.clone(),
            next_cursor: None,
        })
    }

    fn reload(&self) -> Result<ListMcpServerStatusResponse, AppServerError> {
        Ok(ListMcpServerStatusResponse {
            data: self.statuses.clone(),
            next_cursor: None,
        })
    }

    fn call_tool(
        &self,
        params: McpServerToolCallParams,
    ) -> Result<McpServerToolCallResponse, AppServerError> {
        Ok(McpServerToolCallResponse {
            result: serde_json::json!({
                "server": params.server,
                "tool": params.tool,
                "arguments": crate::mcp_service::redact_tool_arguments(params.arguments),
            }),
        })
    }

    fn read_resource(
        &self,
        params: McpResourceReadParams,
    ) -> Result<McpResourceReadResponse, AppServerError> {
        if self.statuses.iter().any(|status| status.name == params.server) {
            Ok(McpResourceReadResponse { contents: vec![] })
        } else {
            Err(AppServerError::service_unavailable(
                ErrorCode::CapabilityUnavailable,
                "mcp",
                format!("unknown MCP server: {}", params.server),
            ))
        }
    }

    fn oauth_login(
        &self,
        params: McpServerOauthLoginParams,
    ) -> Result<McpServerOauthLoginResponse, AppServerError> {
        Ok(McpServerOauthLoginResponse {
            authorization_url: format!("https://auth.example.test/oauth?server={}", params.name),
        })
    }
}
```

- [x] **Step 4: Wire `AppServer` to P4 services**

In `crates/dasclaw_app_server/src/lib.rs`, add:

```rust
pub mod p4_services;
```

Add a field to `AppServer`:

```rust
p4_services: p4_services::P4Services,
```

Initialize it in `AppServer::new()`:

```rust
p4_services: p4_services::P4Services::default(),
```

Add a builder:

```rust
#[must_use]
pub fn with_p4_services(mut self, services: p4_services::P4Services) -> Self {
    self.p4_services = services;
    self.capabilities = self
        .capabilities
        .clone()
        .with_p4_services(self.p4_services.availability());
    self
}
```

Update `capabilities()` so readiness recalculates before returning:

```rust
#[must_use]
pub fn capabilities(&mut self) -> CapabilitiesListResponse {
    self.drain_runtime_turn_updates();
    self.capabilities = self
        .capabilities
        .clone()
        .with_p4_services(self.p4_services.availability());
    CapabilitiesListResponse {
        capabilities: self.capabilities.clone(),
        compatibility_profiles: compatibility_profiles(),
    }
}
```

Replace the four hard-coded disabled P4 services in `service_health()` with:

```rust
let mut services = vec![
    ServiceHealth::ready(ServiceName::Protocol),
    ServiceHealth::ready(ServiceName::Lifecycle),
    ServiceHealth::ready(ServiceName::Session),
    ServiceHealth::degraded(
        ServiceName::Runtime,
        "runtime bridge boundary is available; default host requires a runtime adapter",
    ),
    ServiceHealth::unavailable_fail_safe(
        ServiceName::DlpPolicy,
        "DLP/policy service is declared but not migrated in Phase 1",
    ),
    self.model_provider.health(),
    self.tools_health(),
    self.sandbox_health(),
];
services.extend(self.p4_services.health());
services
```

- [x] **Step 5: Run seam tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server p4_noop_services_remain_disabled_and_declared p4_ready_services_drive_capabilities_and_health_together
```

Expected: both tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/dasclaw_app_server/src/lib.rs crates/dasclaw_app_server/src/p4_services.rs
git commit -m "feat(app-server): add P4 service owner seams" -m "已检查 P4 service owner 是否已有，结论：app-server 只有 disabled health 与 declared capability，没有可注入 service owner seam。"
```

## Task 3: Route Logs Through `log/entry`

**Files:**
- Create: `crates/dasclaw_app_server/src/log_service.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/p4_services.rs`
- Modify: `crates/dasclaw_app_server/Cargo.toml`

- [x] **Step 1: Write failing log notification test**

Add this test in `crates/dasclaw_app_server/src/lib.rs`:

```rust
#[test]
fn p4_log_service_emits_log_entry_notification() {
    let log_service = log_service::AppServerLogService::new();
    let services = p4_services::P4Services::for_tests(
        log_service.clone(),
        p4_services::TestJobService::ready(vec![]),
        p4_services::TestSkillsService::ready(vec![]),
        p4_services::TestMcpService::ready(vec![]),
    );
    let mut server = AppServer::new().with_p4_services(services);

    log_service.emit_for_test(
        dasclaw_app_server_protocol::LogLevel::Warn,
        "app_server.tests",
        "test log message",
        serde_json::json!({"requestId": "req_1"}),
    );

    let notifications = server.drain_notifications();
    assert!(notifications.iter().any(|notification| {
        notification.method == dasclaw_app_server_protocol::event::LOG_ENTRY
            && notification.params["level"] == "warn"
            && notification.params["target"] == "app_server.tests"
            && notification.params["message"] == "test log message"
            && notification.params["fields"]["requestId"] == "req_1"
    }));
}
```

- [x] **Step 2: Run the failing test**

Run:

```bash
cargo nextest run -p dasclaw_app_server p4_log_service_emits_log_entry_notification
```

Expected: fail to compile because `log_service` module and `AppServerLogService` do not exist.

- [x] **Step 3: Add dependency and log service**

In `crates/dasclaw_app_server/Cargo.toml`, add:

```toml
dasclaw_observability = { path = "../dasclaw_observability" }
```

Create `crates/dasclaw_app_server/src/log_service.rs`:

```rust
use std::sync::{Arc, Mutex, mpsc};

use dasclaw_app_server_protocol::{LogEntryEvent, LogLevel, ServiceHealth, ServiceName};
use dasclaw_observability::{Observer, ObserverEvent, ObserverMetric};

#[derive(Clone)]
pub struct AppServerLogService {
    sender: mpsc::Sender<LogEntryEvent>,
    receiver: Arc<Mutex<mpsc::Receiver<LogEntryEvent>>>,
}

impl AppServerLogService {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender,
            receiver: Arc::new(Mutex::new(receiver)),
        }
    }

    pub fn drain(&self) -> Vec<LogEntryEvent> {
        let receiver = self.receiver.lock().unwrap_or_else(|poison| poison.into_inner());
        let mut entries = Vec::new();
        while let Ok(entry) = receiver.try_recv() {
            entries.push(entry);
        }
        entries
    }

    pub fn emit_for_test(
        &self,
        level: LogLevel,
        target: impl Into<String>,
        message: impl Into<String>,
        fields: serde_json::Value,
    ) {
        let fields = fields.as_object().cloned().unwrap_or_default();
        let _ = self.sender.send(LogEntryEvent {
            level,
            target: target.into(),
            message: message.into(),
            time: crate::unix_timestamp_string(),
            fields,
        });
    }
}

impl Default for AppServerLogService {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::p4_services::LogService for AppServerLogService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::ready(ServiceName::Logs)
    }
}

impl Observer for AppServerLogService {
    fn record_event(&self, event: &ObserverEvent) {
        let (level, target, message, fields) = match event {
            ObserverEvent::Error { component, message } => (
                LogLevel::Warn,
                "observability.error".to_string(),
                message.clone(),
                serde_json::json!({"component": component}),
            ),
            ObserverEvent::ToolCallStart { tool } => (
                LogLevel::Info,
                "observability.tool".to_string(),
                "tool call started".to_string(),
                serde_json::json!({"tool": tool}),
            ),
            ObserverEvent::ToolCallEnd {
                tool,
                duration,
                success,
            } => (
                LogLevel::Info,
                "observability.tool".to_string(),
                "tool call completed".to_string(),
                serde_json::json!({
                    "tool": tool,
                    "durationMs": duration.as_millis() as u64,
                    "success": success
                }),
            ),
            _ => (
                LogLevel::Debug,
                "observability.event".to_string(),
                format!("{event:?}"),
                serde_json::json!({}),
            ),
        };
        self.emit_for_test(level, target, message, fields);
    }

    fn record_metric(&self, metric: &ObserverMetric) {
        self.emit_for_test(
            LogLevel::Debug,
            "observability.metric",
            format!("{metric:?}"),
            serde_json::json!({}),
        );
    }

    fn name(&self) -> &str {
        "app_server"
    }
}
```

- [x] **Step 4: Drain log entries before notification drain**

In `crates/dasclaw_app_server/src/lib.rs`, add:

```rust
pub mod log_service;
```

Add helper method:

```rust
fn drain_p4_updates(&mut self) {
    if let Some(log_service) = self.p4_services.log_entry_source() {
        for entry in log_service.drain() {
            self.notifications.emit_log_entry(entry);
        }
    }
}
```

Add this method to `NotificationBus`:

```rust
pub fn emit_log_entry(&mut self, event: LogEntryEvent) {
    self.push(ServerNotification::log_entry(event));
}
```

Call `self.drain_p4_updates();` before every public drain:

```rust
pub fn drain_notifications_with_policy(&mut self) -> NotificationDrain {
    self.drain_p4_updates();
    self.notifications.drain_with_policy()
}
```

In `p4_services.rs`, expose the typed log service:

```rust
impl P4Services {
    pub fn log_entry_source(&self) -> Option<&crate::log_service::AppServerLogService> {
        self.logs
            .as_any()
            .downcast_ref::<crate::log_service::AppServerLogService>()
    }
}
```

To support this, extend `LogService`:

```rust
fn as_any(&self) -> &dyn std::any::Any;
```

Implement `as_any()` for all log service types:

```rust
fn as_any(&self) -> &dyn std::any::Any {
    self
}
```

- [x] **Step 5: Run log test and app-server check**

Run:

```bash
cargo nextest run -p dasclaw_app_server p4_log_service_emits_log_entry_notification
cargo check -p dasclaw_app_server --tests
```

Expected: test passes and `cargo check` reports 0 errors.

- [ ] **Step 6: Commit**

```bash
git add crates/dasclaw_app_server/Cargo.toml crates/dasclaw_app_server/src/lib.rs crates/dasclaw_app_server/src/log_service.rs crates/dasclaw_app_server/src/p4_services.rs
git commit -m "feat(app-server): route P4 logs to log entry notifications" -m "已检查 P4 logs owner 是否已有，结论：protocol 有 log/entry DTO，observability 有 Observer，但 app-server 没有 log source wiring。"
```

## Task 4: Add Native Jobs Host Routes

**Files:**
- Create: `crates/dasclaw_app_server/src/job_service.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/p4_services.rs`

- [x] **Step 1: Write failing jobs routing tests**

Add these tests:

```rust
#[test]
fn p4_jobs_list_routes_to_job_service() {
    let job = dasclaw_app_server_protocol::JobSnapshot {
        job_id: "job_1".to_string(),
        title: "Run audit".to_string(),
        description: "Audit repository".to_string(),
        state: dasclaw_app_server_protocol::JobSnapshotState::InProgress,
        created_at: "2026-06-19T00:00:00Z".to_string(),
        updated_at: None,
        thread_id: Some("thread_1".to_string()),
    };
    let services = p4_services::P4Services::for_tests(
        p4_services::TestLogService::ready(),
        p4_services::TestJobService::ready(vec![job]),
        p4_services::TestSkillsService::ready(vec![]),
        p4_services::TestMcpService::ready(vec![]),
    );
    let mut server = AppServer::new().with_p4_services(services);

    let response = server
        .handle_json_rpc(r#"{"jsonrpc":"2.0","id":"jobs","method":"jobs/list","params":{}}"#)
        .expect("jobs/list should produce response");
    let value: serde_json::Value = serde_json::from_str(&response).expect("response JSON");

    assert_eq!(value["result"]["data"][0]["jobId"], "job_1");
    assert_eq!(value["result"]["data"][0]["state"], "in_progress");
}

#[test]
fn p4_jobs_read_unknown_job_returns_structured_error() {
    let services = p4_services::P4Services::for_tests(
        p4_services::TestLogService::ready(),
        p4_services::TestJobService::ready(vec![]),
        p4_services::TestSkillsService::ready(vec![]),
        p4_services::TestMcpService::ready(vec![]),
    );
    let mut server = AppServer::new().with_p4_services(services);

    let response = server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":"jobs","method":"jobs/read","params":{"jobId":"missing"}}"#,
        )
        .expect("jobs/read should produce response");
    let value: serde_json::Value = serde_json::from_str(&response).expect("response JSON");

    assert!(value["error"]["message"].as_str().unwrap().contains("job not found"));
}
```

- [x] **Step 2: Run failing jobs tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server p4_jobs_list_routes_to_job_service p4_jobs_read_unknown_job_returns_structured_error
```

Expected: fail because router does not handle `jobs/list` or `jobs/read`.

- [x] **Step 3: Add jobs route handlers**

In `crates/dasclaw_app_server/src/lib.rs`, import P4 job types and add router arms:

```rust
method::JOBS_LIST => {
    route_with_optional_params(request.id, request.params, |params| self.jobs_list(params))
}
method::JOBS_READ => {
    route_with_params(request.id, request.params, |params| self.jobs_read(params))
}
```

Add handler methods:

```rust
fn jobs_list(
    &mut self,
    params: dasclaw_app_server_protocol::JobListParams,
) -> Result<dasclaw_app_server_protocol::JobListResponse, AppServerError> {
    self.p4_services.jobs.list(params)
}

fn jobs_read(
    &mut self,
    params: dasclaw_app_server_protocol::JobReadParams,
) -> Result<dasclaw_app_server_protocol::JobReadResponse, AppServerError> {
    self.p4_services.jobs.read(params)
}
```

Create `crates/dasclaw_app_server/src/job_service.rs` for the runtime-state mapping helper:

```rust
use dasclaw_app_server_protocol::JobSnapshotState;
use dasclaw_runtime::JobState;

pub fn map_job_state(state: JobState) -> JobSnapshotState {
    match state {
        JobState::Pending => JobSnapshotState::Pending,
        JobState::InProgress => JobSnapshotState::InProgress,
        JobState::Completed => JobSnapshotState::Completed,
        JobState::Submitted => JobSnapshotState::Submitted,
        JobState::Accepted => JobSnapshotState::Accepted,
        JobState::Failed => JobSnapshotState::Failed,
        JobState::Stuck => JobSnapshotState::Stuck,
        JobState::Cancelled => JobSnapshotState::Cancelled,
    }
}
```

- [x] **Step 4: Run jobs tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server p4_jobs_list_routes_to_job_service p4_jobs_read_unknown_job_returns_structured_error
cargo check -p dasclaw_app_server --tests
```

Expected: tests pass and `cargo check` reports 0 errors.

- [ ] **Step 5: Commit**

```bash
git add crates/dasclaw_app_server/src/lib.rs crates/dasclaw_app_server/src/job_service.rs crates/dasclaw_app_server/src/p4_services.rs
git commit -m "feat(app-server): add native P4 job host routes" -m "已检查 P4 jobs owner 是否已有，结论：runtime 有 JobState/JobContextCore 词汇，app-server 没有 job host/list/read 路由。"
```

## Task 5: Add Skills List And Config Routes

**Files:**
- Create: `crates/dasclaw_app_server/src/skills_service.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/p4_services.rs`

- [x] **Step 1: Write failing skills tests**

Add:

```rust
#[test]
fn p4_skills_list_routes_to_registry_and_preserves_codex_shape() {
    let entry = dasclaw_app_server_protocol::SkillsListEntry {
        cwd: "/repo".to_string(),
        skills: vec![dasclaw_app_server_protocol::SkillMetadata {
            name: "review".to_string(),
            description: "Review code".to_string(),
            short_description: Some("Review".to_string()),
            interface: None,
            dependencies: None,
            path: "/repo/.codex/skills/review/SKILL.json".to_string(),
            scope: dasclaw_app_server_protocol::SkillScope::Repo,
            enabled: true,
        }],
        errors: vec![],
    };
    let services = p4_services::P4Services::for_tests(
        p4_services::TestLogService::ready(),
        p4_services::TestJobService::ready(vec![]),
        p4_services::TestSkillsService::ready(vec![entry]),
        p4_services::TestMcpService::ready(vec![]),
    );
    let mut server = AppServer::new().with_p4_services(services);

    let response = server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":"skills","method":"skills/list","params":{"cwds":["/repo"],"forceReload":true}}"#,
        )
        .expect("skills/list should produce response");
    let value: serde_json::Value = serde_json::from_str(&response).expect("response JSON");

    assert_eq!(value["result"]["data"][0]["cwd"], "/repo");
    assert_eq!(value["result"]["data"][0]["skills"][0]["name"], "review");
    assert_eq!(value["result"]["data"][0]["skills"][0]["scope"], "repo");
}

#[test]
fn p4_skills_config_write_emits_skills_changed() {
    let entry = dasclaw_app_server_protocol::SkillsListEntry {
        cwd: "/repo".to_string(),
        skills: vec![],
        errors: vec![],
    };
    let services = p4_services::P4Services::for_tests(
        p4_services::TestLogService::ready(),
        p4_services::TestJobService::ready(vec![]),
        p4_services::TestSkillsService::ready(vec![entry]),
        p4_services::TestMcpService::ready(vec![]),
    );
    let mut server = AppServer::new().with_p4_services(services);

    let response = server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":"skills","method":"skills/config/write","params":{"name":"review","enabled":false}}"#,
        )
        .expect("skills/config/write should produce response");
    let value: serde_json::Value = serde_json::from_str(&response).expect("response JSON");
    assert_eq!(value["result"]["changed"], true);

    let notifications = server.drain_notifications();
    assert!(notifications.iter().any(|notification| {
        notification.method == dasclaw_app_server_protocol::event::SKILLS_CHANGED
    }));
}
```

- [x] **Step 2: Run failing skills tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server p4_skills_list_routes_to_registry_and_preserves_codex_shape p4_skills_config_write_emits_skills_changed
```

Expected: fail because skills routes and `emit_skills_changed` are missing.

- [x] **Step 3: Add skills route handlers**

Add router arms:

```rust
method::SKILLS_LIST => {
    route_with_optional_params(request.id, request.params, |params| self.skills_list(params))
}
method::SKILLS_CONFIG_WRITE => {
    route_with_params(request.id, request.params, |params| self.skills_config_write(params))
}
```

Add handler methods:

```rust
fn skills_list(
    &mut self,
    params: dasclaw_app_server_protocol::SkillsListParams,
) -> Result<dasclaw_app_server_protocol::SkillsListResponse, AppServerError> {
    self.p4_services.skills.list(params)
}

fn skills_config_write(
    &mut self,
    params: dasclaw_app_server_protocol::SkillsConfigWriteParams,
) -> Result<dasclaw_app_server_protocol::SkillsConfigWriteResponse, AppServerError> {
    let response = self.p4_services.skills.write_config(params)?;
    if response.changed {
        self.notifications
            .emit_skills_changed(dasclaw_app_server_protocol::SkillsChangedNotification {});
    }
    Ok(response)
}
```

Add notification bus method:

```rust
pub fn emit_skills_changed(&mut self, event: SkillsChangedNotification) {
    self.push(ServerNotification::skills_changed(event));
}
```

In `ServerNotification`, add constructor matching the existing pattern:

```rust
pub fn skills_changed(event: SkillsChangedNotification) -> Result<Self, serde_json::Error> {
    Self::new(event::SKILLS_CHANGED, event)
}
```

- [x] **Step 4: Create minimal skills service module**

Create `crates/dasclaw_app_server/src/skills_service.rs`:

```rust
use dasclaw_app_server_protocol::{
    SkillMetadata, SkillsConfigWriteParams, SkillsConfigWriteResponse, SkillsListEntry,
    SkillsListParams, SkillsListResponse,
};

pub fn filter_entries_by_cwd(
    entries: &[SkillsListEntry],
    params: &SkillsListParams,
) -> SkillsListResponse {
    if params.cwds.is_empty() {
        return SkillsListResponse {
            data: entries.to_vec(),
        };
    }

    let data = entries
        .iter()
        .filter(|entry| params.cwds.iter().any(|cwd| cwd == &entry.cwd))
        .cloned()
        .collect();
    SkillsListResponse { data }
}

pub fn write_enabled_flag(
    entries: &mut [SkillsListEntry],
    params: &SkillsConfigWriteParams,
) -> SkillsConfigWriteResponse {
    let mut changed = false;
    for skill in entries.iter_mut().flat_map(|entry| entry.skills.iter_mut()) {
        if matches_skill(skill, params) && skill.enabled != params.enabled {
            skill.enabled = params.enabled;
            changed = true;
        }
    }
    SkillsConfigWriteResponse { changed }
}

fn matches_skill(skill: &SkillMetadata, params: &SkillsConfigWriteParams) -> bool {
    if let Some(path) = &params.path {
        return &skill.path == path;
    }
    if let Some(name) = &params.name {
        return &skill.name == name;
    }
    false
}
```

Update `TestSkillsService` to store entries behind `Arc<Mutex<Vec<SkillsListEntry>>>` and call these helpers. If both `path` and `name` are missing, return:

```rust
Err(AppServerError::invalid_request(
    "skills",
    "skills/config/write requires path or name",
))
```

- [x] **Step 5: Run skills tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server p4_skills_list_routes_to_registry_and_preserves_codex_shape p4_skills_config_write_emits_skills_changed
cargo check -p dasclaw_app_server --tests
```

Expected: tests pass and `cargo check` reports 0 errors.

- [ ] **Step 6: Commit**

```bash
git add crates/dasclaw_app_server/src/lib.rs crates/dasclaw_app_server/src/p4_services.rs crates/dasclaw_app_server/src/skills_service.rs crates/dasclaw_app_server_protocol/src/lib.rs
git commit -m "feat(app-server): add P4 skills routes" -m "已检查 P4 skills owner 是否已有，结论：dasclaw_protocol 有 skill DTO，旧 desktop-client 有参考 registry，当前 app-server 没有 native skills service。"
```

## Task 6: Add MCP Status Reload And Tool Call Routes

**Files:**
- Create: `crates/dasclaw_app_server/src/mcp_service.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/p4_services.rs`
- Modify: `crates/dasclaw_app_server/Cargo.toml`

- [x] **Step 1: Write failing MCP route tests**

Add:

```rust
#[test]
fn p4_mcp_status_list_routes_to_service() {
    let status = dasclaw_app_server_protocol::McpServerStatus {
        name: "github".to_string(),
        tools: serde_json::json!({
            "list_issues": {
                "name": "list_issues",
                "description": "List issues",
                "inputSchema": {"type": "object"}
            }
        }),
        resources: vec![],
        resource_templates: vec![],
        auth_status: dasclaw_app_server_protocol::McpAuthStatus::OAuth,
    };
    let services = p4_services::P4Services::for_tests(
        p4_services::TestLogService::ready(),
        p4_services::TestJobService::ready(vec![]),
        p4_services::TestSkillsService::ready(vec![]),
        p4_services::TestMcpService::ready(vec![status]),
    );
    let mut server = AppServer::new().with_p4_services(services);

    let response = server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":"mcp","method":"mcpServerStatus/list","params":{"detail":"toolsAndAuthOnly"}}"#,
        )
        .expect("mcpServerStatus/list should produce response");
    let value: serde_json::Value = serde_json::from_str(&response).expect("response JSON");

    assert_eq!(value["result"]["data"][0]["name"], "github");
    assert_eq!(value["result"]["data"][0]["authStatus"], "oAuth");
    assert_eq!(value["result"]["data"][0]["tools"]["list_issues"]["name"], "list_issues");
}

#[test]
fn p4_mcp_tool_call_routes_to_service_without_token_leak() {
    let services = p4_services::P4Services::for_tests(
        p4_services::TestLogService::ready(),
        p4_services::TestJobService::ready(vec![]),
        p4_services::TestSkillsService::ready(vec![]),
        p4_services::TestMcpService::ready(vec![]),
    );
    let mut server = AppServer::new().with_p4_services(services);

    let response = server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":"mcp","method":"mcpServer/tool/call","params":{"threadId":"thread_1","server":"github","tool":"list_issues","arguments":{"repo":"x-claw","token":"secret-token"}}}"#,
        )
        .expect("mcpServer/tool/call should produce response");
    let value: serde_json::Value = serde_json::from_str(&response).expect("response JSON");

    assert_eq!(value["result"]["result"]["server"], "github");
    assert!(
        !response.contains("secret-token"),
        "MCP tool-call response must not echo raw secret arguments"
    );
}
```

- [x] **Step 2: Run failing MCP tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server p4_mcp_status_list_routes_to_service p4_mcp_tool_call_routes_to_service_without_token_leak
```

Expected: fail because MCP routes are missing.

- [x] **Step 3: Add MCP dependency and route handlers**

In `crates/dasclaw_app_server/Cargo.toml`, add:

```toml
dasclaw_mcp = { path = "../dasclaw_mcp" }
```

In `route_json_rpc()`, add:

```rust
method::MCP_SERVER_STATUS_LIST => {
    route_with_optional_params(request.id, request.params, |params| self.mcp_status_list(params))
}
method::CONFIG_MCP_SERVER_RELOAD => {
    route_with_no_params(request.id, request.params, || self.mcp_reload())
}
method::MCP_SERVER_TOOL_CALL => {
    route_with_params(request.id, request.params, |params| self.mcp_tool_call(params))
}
```

Add handlers:

```rust
fn mcp_status_list(
    &mut self,
    params: dasclaw_app_server_protocol::ListMcpServerStatusParams,
) -> Result<dasclaw_app_server_protocol::ListMcpServerStatusResponse, AppServerError> {
    self.p4_services.mcp.list_status(params)
}

fn mcp_reload(
    &mut self,
) -> Result<dasclaw_app_server_protocol::ListMcpServerStatusResponse, AppServerError> {
    let response = self.p4_services.mcp.reload()?;
    for status in &response.data {
        self.notifications
            .emit_mcp_startup_status_updated(dasclaw_app_server_protocol::McpServerStatusUpdatedNotification {
                name: status.name.clone(),
                status: "ready".to_string(),
                error: None,
            });
    }
    Ok(response)
}

fn mcp_tool_call(
    &mut self,
    params: dasclaw_app_server_protocol::McpServerToolCallParams,
) -> Result<dasclaw_app_server_protocol::McpServerToolCallResponse, AppServerError> {
    self.p4_services.mcp.call_tool(params)
}
```

Add notification bus method:

```rust
pub fn emit_mcp_startup_status_updated(&mut self, event: McpServerStatusUpdatedNotification) {
    self.push(ServerNotification::mcp_startup_status_updated(event));
}
```

- [x] **Step 4: Create MCP service helpers**

Create `crates/dasclaw_app_server/src/mcp_service.rs`:

```rust
use std::collections::BTreeMap;

use dasclaw_app_server_protocol::{
    McpAuthStatus, McpServerStatus,
};
use dasclaw_mcp::McpTool;

pub fn tools_to_status_value(tools: Vec<McpTool>) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for tool in tools {
        map.insert(
            tool.name.clone(),
            serde_json::json!({
                "name": tool.name,
                "description": tool.description,
                "inputSchema": tool.input_schema,
            }),
        );
    }
    serde_json::Value::Object(map)
}

pub fn build_status(
    name: impl Into<String>,
    tools: Vec<McpTool>,
    auth_status: McpAuthStatus,
) -> McpServerStatus {
    McpServerStatus {
        name: name.into(),
        tools: tools_to_status_value(tools),
        resources: vec![],
        resource_templates: vec![],
        auth_status,
    }
}

pub fn redact_tool_arguments(arguments: Option<serde_json::Value>) -> serde_json::Value {
    let Some(value) = arguments else {
        return serde_json::json!({});
    };
    match value {
        serde_json::Value::Object(map) => {
            let redacted = map
                .into_iter()
                .map(|(key, value)| {
                    let lower = key.to_ascii_lowercase();
                    if lower.contains("token")
                        || lower.contains("secret")
                        || lower.contains("password")
                        || lower.contains("key")
                    {
                        (key, serde_json::json!("[redacted]"))
                    } else {
                        (key, value)
                    }
                })
                .collect::<serde_json::Map<_, _>>();
            serde_json::Value::Object(redacted)
        }
        other => other,
    }
}
```

Use `redact_tool_arguments` in `TestMcpService::call_tool` so the test proves the app-server layer never echoes raw sensitive values.

- [x] **Step 5: Run MCP tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server p4_mcp_status_list_routes_to_service p4_mcp_tool_call_routes_to_service_without_token_leak
cargo check -p dasclaw_app_server --tests
```

Expected: tests pass and `cargo check` reports 0 errors.

- [ ] **Step 6: Commit**

```bash
git add crates/dasclaw_app_server/Cargo.toml crates/dasclaw_app_server/src/lib.rs crates/dasclaw_app_server/src/mcp_service.rs crates/dasclaw_app_server/src/p4_services.rs crates/dasclaw_app_server_protocol/src/lib.rs
git commit -m "feat(app-server): add P4 MCP status and tool routes" -m "已检查 P4 MCP owner 是否已有，结论：dasclaw_mcp 有 client/factory/executor 底座，app-server 没有 mcpServer/* route owner。"
```

## Task 7: Add MCP OAuth And Resource Read Fail-Safe Routes

**Files:**
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_app_server/src/mcp_service.rs`
- Modify: `crates/dasclaw_app_server/src/p4_services.rs`

- [x] **Step 1: Write failing OAuth/resource tests**

Add:

```rust
#[test]
fn p4_mcp_oauth_login_returns_authorization_url_and_completion_event() {
    let services = p4_services::P4Services::for_tests(
        p4_services::TestLogService::ready(),
        p4_services::TestJobService::ready(vec![]),
        p4_services::TestSkillsService::ready(vec![]),
        p4_services::TestMcpService::ready(vec![]),
    );
    let mut server = AppServer::new().with_p4_services(services);

    let response = server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":"mcp","method":"mcpServer/oauth/login","params":{"name":"github","scopes":["repo"],"timeoutSecs":30}}"#,
        )
        .expect("mcpServer/oauth/login should produce response");
    let value: serde_json::Value = serde_json::from_str(&response).expect("response JSON");

    assert_eq!(
        value["result"]["authorizationUrl"],
        "https://auth.example.test/oauth?server=github"
    );
    let notifications = server.drain_notifications();
    assert!(notifications.iter().any(|notification| {
        notification.method == dasclaw_app_server_protocol::event::MCP_SERVER_OAUTH_LOGIN_COMPLETED
            && notification.params["name"] == "github"
            && notification.params["success"] == true
    }));
}

#[test]
fn p4_mcp_resource_read_unknown_server_fails_safe() {
    let services = p4_services::P4Services::for_tests(
        p4_services::TestLogService::ready(),
        p4_services::TestJobService::ready(vec![]),
        p4_services::TestSkillsService::ready(vec![]),
        p4_services::TestMcpService::ready(vec![]),
    );
    let mut server = AppServer::new().with_p4_services(services);

    let response = server
        .handle_json_rpc(
            r#"{"jsonrpc":"2.0","id":"mcp","method":"mcpServer/resource/read","params":{"server":"missing","uri":"file:///secret"}}"#,
        )
        .expect("mcpServer/resource/read should produce response");
    let value: serde_json::Value = serde_json::from_str(&response).expect("response JSON");

    assert_eq!(value["error"]["data"]["code"], "CAPABILITY_UNAVAILABLE");
    assert!(!response.contains("secret-token"));
}
```

- [x] **Step 2: Run failing OAuth/resource tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server p4_mcp_oauth_login_returns_authorization_url_and_completion_event p4_mcp_resource_read_unknown_server_fails_safe
```

Expected: fail because OAuth/resource routes are not wired.

- [x] **Step 3: Add router arms and completion notification**

Add:

```rust
method::MCP_SERVER_OAUTH_LOGIN => {
    route_with_params(request.id, request.params, |params| self.mcp_oauth_login(params))
}
method::MCP_SERVER_RESOURCE_READ => {
    route_with_params(request.id, request.params, |params| self.mcp_resource_read(params))
}
```

Add handlers:

```rust
fn mcp_oauth_login(
    &mut self,
    params: dasclaw_app_server_protocol::McpServerOauthLoginParams,
) -> Result<dasclaw_app_server_protocol::McpServerOauthLoginResponse, AppServerError> {
    let name = params.name.clone();
    match self.p4_services.mcp.oauth_login(params) {
        Ok(response) => {
            self.notifications.emit_mcp_oauth_login_completed(
                dasclaw_app_server_protocol::McpServerOauthLoginCompletedNotification {
                    name,
                    success: true,
                    error: None,
                },
            );
            Ok(response)
        }
        Err(error) => {
            self.notifications.emit_mcp_oauth_login_completed(
                dasclaw_app_server_protocol::McpServerOauthLoginCompletedNotification {
                    name,
                    success: false,
                    error: Some(error.message.clone()),
                },
            );
            Err(error)
        }
    }
}

fn mcp_resource_read(
    &mut self,
    params: dasclaw_app_server_protocol::McpResourceReadParams,
) -> Result<dasclaw_app_server_protocol::McpResourceReadResponse, AppServerError> {
    self.p4_services.mcp.read_resource(params)
}
```

Add notification bus method and `ServerNotification` constructor:

```rust
pub fn emit_mcp_oauth_login_completed(
    &mut self,
    event: McpServerOauthLoginCompletedNotification,
) {
    self.push(ServerNotification::mcp_oauth_login_completed(event));
}
```

- [x] **Step 4: Run OAuth/resource tests**

Run:

```bash
cargo nextest run -p dasclaw_app_server p4_mcp_oauth_login_returns_authorization_url_and_completion_event p4_mcp_resource_read_unknown_server_fails_safe
cargo check -p dasclaw_app_server --tests
```

Expected: tests pass and `cargo check` reports 0 errors.

- [ ] **Step 5: Commit**

```bash
git add crates/dasclaw_app_server/src/lib.rs crates/dasclaw_app_server/src/mcp_service.rs crates/dasclaw_app_server/src/p4_services.rs crates/dasclaw_app_server_protocol/src/lib.rs
git commit -m "feat(app-server): add P4 MCP OAuth and resource routes" -m "已检查 P4 MCP OAuth/resource owner 是否已有，结论：dasclaw_mcp 有 OAuth/resource primitives，app-server 没有 fail-safe route owner。"
```

## Task 8: Final Verification And Docs Update

**Files:**
- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`

- [ ] **Step 1: Update P4 rows in gap matrix**

Change only the P4-relevant lines, and keep MCP split by real availability rather than treating every route as implemented:

```markdown
| P4 | MCP / skills / logs / jobs | `mcpServerStatus/list`、`config/mcpServer/reload`、MCP startup status event、`skills/list`、`skills/config/write`、`skills/changed`、`log/entry`、native `jobs/list` / `jobs/read` 已由 app-server service owner 接管；MCP `tool/call`、`resource/read`、`oauth/login` 仍 fail-safe routed 但不广告 implemented capability，完整 MCP session/OAuth/progress、elicitation、watcher、filesystem/command exec 仍归后续专项 | MCP registry、logs、jobs、skills 从 declared future 进入受测 app-server control-plane；未接线 MCP effectful paths 仍不冒充支持 |
```

If a slice is intentionally not merged in the current PR, leave that item unstruck and add an evidence note naming the exact route/test that remains absent.

Review follow-up:

- MCP implemented capability is method/event granular.
- The real registry service advertises only `mcpServerStatus/list`, `config/mcpServer/reload`, and `mcpServer/startupStatus/updated`.
- `mcpServer/tool/call`, `mcpServer/resource/read`, and `mcpServer/oauth/login` are still fail-safe/unavailable in the real service until MCP session/OAuth ownership exists.
- `item/mcpToolCall/progress` remains schema-only and is not advertised until the route has real turn/item context.

- [x] **Step 2: Run focused verification**

Run:

```bash
cargo nextest run -p dasclaw_app_server_protocol p4_
cargo nextest run -p dasclaw_app_server p4_
cargo check -p dasclaw_app_server_protocol --tests
cargo check -p dasclaw_app_server --tests
cargo fmt --all
python3.12 scripts/check_no_panics.py --base origin/xClaw
```

Expected:

- P4 protocol tests pass.
- P4 app-server tests pass.
- Both `cargo check` commands report 0 errors.
- `cargo fmt --all` exits 0.
- `check_no_panics.py` exits 0.

- [ ] **Step 3: Run quality skills**

Run the repo-required self-review steps before push:

```bash
cargo clippy --no-deps -p dasclaw_app_server_protocol --all-targets -- -D warnings
cargo clippy --no-deps -p dasclaw_app_server --all-targets -- -D warnings
```

Then run the required review skills for code changes:

- `code-quality-audit`
- `code-simplifier`
- `code-review-expert`

Run `adr-compliance-check` if the implementation touches `.github/workflows/code_style.yml`, `scripts/check_codex_*_drift.py`, starlark pins, `.ironclaw` literals, or `crates/dasclaw_*` verbatim-port purity constraints.

- [ ] **Step 4: Commit docs/verification**

```bash
git add docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
git commit -m "docs(app-server): mark verified P4 protocol progress" -m "已检查 P4 gap matrix 是否已有，结论：本文是现有全量对账表，本次只更新已由测试证明的 P4 行。"
```

## Self-Review

Spec coverage:

- MCP registry status/reload：Tasks 1, 2, 6。
- MCP OAuth/resource/tool-call fail-safe routes：Tasks 6, 7；real session/OAuth ownership remains follow-up and is not advertised as implemented capability.
- Skills registry：Tasks 1, 2, 5。
- Logs source：Tasks 1, 2, 3。
- Job host：Tasks 1, 2, 4。
- Honest capability/health boundary：Tasks 1, 2, 8。

Placeholder scan:

- Pass: no banned marker strings or vague implementation instructions remain in task steps.

Type consistency:

- `P4ServiceAvailability` is defined in Task 1 and consumed in Task 2; MCP availability is method/event granular.
- `SkillsListResponse`, `ListMcpServerStatusResponse`, `JobListResponse` are defined in Task 1 and used by app-server services in Tasks 2, 4, 5, 6, 7.
- `skills/config/write` emits `SkillsChangedNotification`; `mcpServer/oauth/login` emits `McpServerOauthLoginCompletedNotification`; both constructors are added before route tests expect them.

Execution notes:

- Use stacked commits in task order.
- Do not mark full MCP P4 complete in the gap matrix until the real service owns session/OAuth/resource/progress behavior and advertises those methods/events from capability tests.
- Keep commit message transparency line for every implementation commit because this work creates modules and makes protocol gap conclusions.
