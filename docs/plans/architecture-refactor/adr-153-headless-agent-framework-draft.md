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

### 4.4 步骤 4：dasclaw_cli（新）
- 一个最小 CLI 二进制，演示「无 db、无 HTTP」直接跑 agent
- 类似 claw-code 当前的体验，但全部来自 crates/

完成这 4 步以后，desktop-client/ironclaw 就是一个**纯粹的「桌面后端业务皮」**——HTTP + 数据库 + 租户 + Web 钩子，里面调 `dasclaw_runtime::Agent` 来真正跑 agent。

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
