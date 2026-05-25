# ADR-153（草稿）：把 x-claw 演化成一个无头 agent 框架

- 状态：Accepted（2026-05-20，用户在 PR #675 反馈中拍板）
- 提案人：Coding Agent（基于 ADR-152 F2 系列交付的观察）
- 关联：ADR-152（agent and capability fusion）、ADR-129（verbatim port mandate）
- 时间：2026-05-18

---

## 1. 背景

ADR-152 把原本散落在 desktop-client/ironclaw 内部的能力（bash 校验、hooks、沙箱、网络代理、协议、LLM Provider 等）逐步抽出到 crates/dasclaw_* 下的独立 crate。截至本草稿落笔时，crates/ 下已有 37 个 dasclaw_* / ironclaw_* crate，desktop-client/ironclaw 自身还留着 377 个 .rs 文件、约 9.6MB 代码。

这就引出一个问题：**如果继续把 ironclaw 的能力都抽到 crates/，最终能不能形成一个「import 一个库 + 几行代码就能跑一个 agent」的无头框架？**

本 ADR 草稿尝试回答这个问题，并给出最小路径。

## 2. 现状盘点

把 desktop-client/ironclaw/src 当前内容按职责粗分为四类：

| 类别 | 目录 / 文件示例 | 是否可抽 | 备注 |
|------|------------------|-----------|------|
| 单一能力（已抽完或在抽） | tools/ 中的工具壳、sandbox、safety、secrets | 已部分迁移 | 工具壳保留是为了挂接 trait |
| 运行时编排 | agent/、orchestrator/、worker/、channels/ | 应抽 | 是「无头框架」的核心 |
| 桌面服务皮 | app.rs、service.rs、bootstrap.rs、cli/、setup/ | 不抽 | HTTP 服务、CLI、启动只属于桌面后端 |
| 多租户业务 | db/、migration/、tenant、pairing、history、webhooks、extensions、document_extraction、evaluation、estimation、skills、routines | 不抽 | 业务侧，与「跑一个 agent」无关 |

## 3. 目标形态

期望的最小可用形态：

```rust
use dasclaw_runtime::{Agent, AgentConfig};
use dasclaw_llm_provider::OpenAiCompatible;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let llm = OpenAiCompatible::from_env()?;
    let agent = Agent::builder()
        .llm(llm)
        .workspace(std::env::current_dir()?)
        .tools_default()      // 内置 shell / read / edit / grep
        .hooks_default()      // 内置 bash-validation / bash-permissions / safety
        .build()?;

    let result = agent.run("帮我把 README 里所有 TODO 列出来").await?;
    println!("{}", result.final_text);
    Ok(())
}
```

调用方拿到一个库就能跑，**完全不需要 HTTP 服务、不需要数据库、不需要租户**。

## 4. 路径：从现在到目标形态

按依赖顺序拆成 4 步：

### 4.1 步骤 1：完成 F3.1 ~ F3.6（已有 issue）
把 ironclaw 仍然内嵌的 LLM Provider / MCP / LSP / workspace_cap / observability / context manager 抽到 dasclaw_*。完成后 ironclaw 的「能力」基本上就只剩薄薄一层调度。

### 4.2 步骤 2：抽 dasclaw_runtime（新）
- 从 ironclaw 的 agent/ + orchestrator/ 抽出最小 agent loop（系统提示、tool 调度、token 预算、停止条件）
- 不带数据库、不带租户、不带 HTTP；只接受 LLM、tools、hooks、workspace 四个入参
- 提供 `Agent::builder()` 这一个外部 API

### 4.3 步骤 3：抽 dasclaw_session（可选）
- 把 ironclaw 的 channels/ + history/ 中**与桌面无关**的会话状态机抽出来（消息历史、断点续传、超时）
- 仍然不绑定 db；用 trait `SessionStore` 让调用方自己选 in-memory / sqlite / postgres

### 4.4 步骤 4 起：dasclaw_cli（新）

`crates/dasclaw_cli/` 是一个最小 CLI 二进制，证明「无 db、无 HTTP、无 Tauri」就能直接跑 agent。它本身**不是**桌面后端的替代品；它是验证 dasclaw_* 这一套 crate 真的能在第三方进程里独立运行的「最小宿主」。CLI 自己只做三件事：拿到一个 LLM provider、构造 agent loop、把工具集喂给 agent。

为了避免一次性塞太多东西进一个 PR，CLI 按子步骤递进交付，每一子步对应一个独立 PR、每一步都保留前面所有子步的行为不退化：

#### 4.4.1 子步骤 4：CLI 骨架（已落地，PR #800）

- 提供 `dasclaw_cli::run(responder, system, user) -> String` 这一**唯一**库入口，不做任何 I/O。
- 提供内置 `EchoResponder`：返回 `echo: <last user content>`，仅供 smoke test，不打网络。
- 二进制 `dasclaw-cli` 默认子命令 `echo`：从 stdin 或 `--prompt` 读用户输入，把回复打到 stdout。
- 范围之外：不接真实 provider、没有工具调度、没有 streaming。

#### 4.4.2 子步骤 5：CLI 接真实 LLM provider（已落地，PR #802）

- 新增 `dasclaw_cli::provider::ProviderArgs` 把 provider 类型（anthropic / openai-compat / ollama）+ base-url + model + 环境变量名打成一个 clap flatten。
- 新增 `dasclaw_cli::provider::build_responder(&ProviderArgs)`，把上面的 args 转成 `dasclaw_runtime::LlmProviderResponder`。
- 二进制增加 `run` 子命令；`echo` 不变。
- API key 走环境变量：`ANTHROPIC_API_KEY` / `OPENAI_API_KEY` / `DASCLAW_API_KEY`；缺 key 时给出明确的环境变量提示。
- 测试用 wiremock 跑一次完整 round-trip，不连真实 provider。

#### 4.4.3 子步骤 6：CLI 接本地工具（已落地，PR #804）

- 新增 `dasclaw_cli::tools` 模块：
  - `ToolHandler = Arc<dyn Fn(&Value) -> Result<String, String> + Send + Sync>`
  - `CliTool { definition, handler }`
  - `StaticToolExecutor`：name → handler 字典，实现 `async dasclaw_runtime::ToolExecutor`。
  - 内置两个演示工具：`echo`（返回入参 `message`）、`now`（返回 epoch 秒）。
- 新增 `dasclaw_cli::run_with_tools(...)`：第二个入口，和 `run` 并存；不破坏 `run` 的零工具调用路径。
- `run` 子命令增加 `--enable-tools` flag（默认关），开启后把 `default_builtins()` 喂给 agent。
- 范围之外：不接 MCP；不桥接 `dasclaw_fs_tools` / `dasclaw_shell_command`（这俩的 `Tool` trait 还在 ironclaw 里耦合着 `JobContext`，另开 issue 解耦）。

#### 4.4.4 子步骤 7：dasclaw_mcp::McpToolExecutor（已落地，PR #806）

这一步**不在** `dasclaw_cli` 里落，是在 `dasclaw_mcp` 里加一层：

- 新增 `dasclaw_mcp::McpToolExecutor`，把一个或多个 `Arc<McpClient>` 包成 `dasclaw_runtime::ToolExecutor`。
- 工具名按 `<server>_<tool>` 限定，避免多 server 命名冲突（与 ironclaw `create_tools_for` 同步）。
- 未知工具 / 传输错误一律返回 `is_error=true` 的 `ToolResult`，喂回 LLM 而不是把异常抛进 agent loop。
- 提供 `from_clients(...)`（生产用，自动调 `list_tools()`）与 `from_routes(...)`（测试用，跳过 transport）两种构造入口。
- 与 #804 文件零重叠，并行交付。

为什么这一步独立成 PR 而不是塞进 CLI：`McpToolExecutor` 是一个**通用**桥接，无头 agent 框架的任何宿主（CLI、ironclaw 桌面后端、第三方进程）都会用到它，让它住在 `dasclaw_mcp` 里、不依赖任何 CLI 概念，是正确的分层。

#### 4.4.5 子步骤 8：CLI 接 MCP 服务器（已落地，PR #812 + #814 + #817）

把 #806 的 `McpToolExecutor` 真正接进 `dasclaw-cli`，让用户能在不写代码的情况下把任意 MCP 服务器挂上 agent。设计要点：

- **配置面**：新增 `--mcp-config <path>` 指向一个 JSON 文件，结构复用 `dasclaw_mcp::config::McpServerConfig`：

  ```jsonc
  {
    "servers": [
      { "name": "fs", "command": "npx", "args": ["@modelcontextprotocol/server-filesystem", "/tmp"] },
      { "name": "http-demo", "url": "https://mcp.example.com/v1", "headers": { "Authorization": "Bearer …" } }
    ]
  }
  ```

  选 JSON config 而不是逐个 CLI flag 的原因：MCP server 的参数空间（command/args/env/url/headers/auth）太大，flag 化会让 `run` 子命令的入口爆炸；JSON config 跟 claw-code / ironclaw 现有的 MCP 配置思路一致，未来加 OAuth / 多 server 不需要再动 CLI。

- **运行面**：CLI 启动时按 config 顺序起每个 server：
  - 对 stdio 类型：用 `dasclaw_mcp::StdioMcpTransport::spawn` 起子进程。
  - 对 HTTP 类型：构造 `HttpMcpTransport`（不带 session-manager / secrets，CLI 暂不接 OAuth）。
  - 把每个 transport 包成 `McpClient::new_with_transport(name, transport, None, None, "cli", Some(config))`，再喂给 `McpToolExecutor::from_clients(...)`。

- **合并面**：`--enable-tools` 与 `--mcp-config` 可同时启用。届时需要一个 `CompositeToolExecutor`（在 `dasclaw_runtime` 或 `dasclaw_cli` 里）把 `StaticToolExecutor` 和 `McpToolExecutor` 的工具集求并、按 name 分发。**注意**：合并器是个新的小公共组件，会按 3 层代码审查先确认 crates/ 没有等价物再决定落在哪。

- **范围之外**：
  - 不接 OAuth（hosted MCP 服务器需要 `SessionManager + SecretsStore`，CLI 暂时没有持久化存储，另开 issue）。
  - 不做工具白名单 / 审批（CLI 默认信任 config 里列出的 server；审批走 agent 层的 hook 系统，不在 CLI 入口里硬编码）。
  - 不做 `--mcp-stdio name=cmd` 这种 shell 友好的 inline flag；先用 JSON config 收口，inline flag 等用户真有需求再加。

#### 4.4.6 必要性分析（为什么无头 agent 框架真的需要这些能力）

| 能力 | 是否必要 | 理由 |
|------|----------|------|
| 真实 LLM provider 接入（4.4.2） | **必要** | 无头框架的存在意义就是「一个进程跑 agent」，没有 provider 就只剩 echo |
| 本地工具调度（4.4.3） | **必要** | agent loop 的 tool dispatch trait 不接 executor 就触发 `AgentError::ToolsNotSupported`，模型一旦返回 `tool_call` 就崩；至少要有一个 executor 兜底 |
| MCP 桥接（4.4.4） | **必要** | MCP 已是模型生态的事实标准工具协议，连 fs/github/postgres 等服务的标准路径；不接 MCP 等于把无头框架封在「只能用内置工具」的玩具状态 |
| CLI 接 MCP（4.4.5） | **必要但可后置** | 库层（#806）做完后，任何宿主都能在代码里接 MCP；CLI 接 MCP 的价值是「不写代码就能用」，是降低无头框架对用户的门槛，但不是无头框架本身的依赖 |
| OAuth-backed MCP / 审批 / streaming / 工具白名单 | **暂不必要** | 这些都属于「桌面后端 / 企业部署」层的关注点，无头 CLI 的最小目标是验证 dasclaw_* 能独立工作，不应一次性把全部生态搬过来 |

完成这 4 步以后，desktop-client/ironclaw 就是一个**纯粹的「桌面后端业务皮」**——HTTP + 数据库 + 租户 + Web 钩子，里面调 `dasclaw_runtime::Agent` 来真正跑 agent。

#### 4.4.7 主流程完成里程碑（2026-05-25）

§4.4 规划的 5 个 CLI 子步骤（step 4 ~ 8）全部落地：

| 子步 | PR | 内容 |
|------|------|------|
| 4 | #800 | CLI 骨架 + `EchoResponder` + `echo` 子命令 |
| 5 | #802 | `provider` 模块 + `run --provider {anthropic\|openai\|openai_compat\|ollama}` + wiremock e2e |
| 6 | #804 | `tools` 模块 + `StaticToolExecutor` + `--enable-tools` + `run_with_tools` |
| 7 | #806 | `dasclaw_mcp::McpToolExecutor`（住在 `dasclaw_mcp`，不在 CLI） |
| 8 | #812 + #814 + #817 | `mcp` 模块（stdio + HTTP transport）+ `--mcp-config` + `dasclaw_runtime::CompositeToolExecutor` 合并器 |

集成测试就位（[crates/dasclaw_cli/tests/](../../../crates/dasclaw_cli/tests/)）：

- `end_to_end.rs` — echo / provider 基线
- `live_provider.rs` — wiremock LLM round-trip
- `tool_e2e.rs` — `--enable-tools` 路径
- `mcp_e2e.rs` — stdio MCP server 子进程端到端（成功 + 未知工具错误注入）

PR #817 CI 实测 Tests (default) 3m26s 通过、Clippy (default) 11m49s 通过。

**仍待事项（均按 ADR 原文规划留给后续 slice，非本路径阻塞）**：

- step 3 `dasclaw_session`：ADR §4.3 标「可选，等到有第二个调用方时再抽」，当前仍只有 ironclaw 桌面后端一个调用方，暂搁。
- §4.4.6 列「暂不必要」的 5 项：OAuth-backed MCP / 审批 / streaming / 工具白名单 / inline `--mcp-stdio` flag — 全部属于桌面后端 / 企业部署关注点，按规划另开 ADR。

**过程教训沉淀**：本批次 step 8 的 PR #816 因合并 stacked 下层 PR #814 时 base 分支被同时删除，触发 GitHub 自动关闭且无法 reopen 的失败模式（GraphQL 报 `base ref deleted`），被迫开 #817 替代、原 PR 编号永久失效。AGENTS.md "标准流程" §7 已新增「路径 A / 路径 B」硬规则与对应禁止事项条款。

## 5. 不属于本 ADR 范围

- 不重写 LLM 协议、不改 tool 调用约定（保持 ADR-152 既定形态）
- 不动 sandbox 模型（受 ADR-151 约束）
- 不引入新 async runtime（继续用 tokio）

## 6. 风险与折衷

| 风险 | 缓解 |
|------|-------|
| 抽 agent loop 时 ironclaw 的桌面后端会一直依赖一个还在变的 crate | 抽完一次就锁定 `Agent` 公共 API，后续只加不改 |
| 「无头框架」与 desktop-client 的桌面后端长期分叉 | crates/dasclaw_runtime 只暴露能力，编排策略仍由桌面后端做，避免重复 |
| F3.x 全部完成前，dasclaw_runtime 拿不到所需依赖 | dasclaw_runtime 计划放在步骤 2，确实必须等 F3 系列收敛 |

## 7. 决策请求

希望对以下几点做决定：

1. 是否同意把「无头 agent 框架」作为 ADR-152 之后的延伸路线？
2. 抽 dasclaw_runtime 这一步是否作为 ADR-152 的 F5？还是开一个新的顶层 ADR（例如 ADR-154）？
3. dasclaw_session 是否一起抽，还是等到有第二个调用方时再抽？

定稿后会把本草稿编号转正、补充任务拆解和验收清单。
