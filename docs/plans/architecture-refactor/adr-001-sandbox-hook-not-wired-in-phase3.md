# ADR-001：Phase 3 不在 agent 主进程接线进程内 sandbox hook

- 状态：**已采纳**（2026-04-23）
- 影响范围：`crates/x_claw_agent::SandboxExecutor` / `desktop-client/ironclaw/src/sandbox/agent_executor.rs` / Phase 3 Step E
- 相关代码：上游 `ironclaw-main/src/bridge/sandbox/`、`ironclaw-main/src/bin/sandbox_daemon.rs`、`ironclaw-main/crates/ironclaw_engine/`

## 背景

Phase 3 Step E 原计划是"把 `x_claw_agent::SandboxExecutor` hook 接到 ironclaw 的 `SandboxManager`"，并从 `desktop-client/ironclaw/src/sandbox/` 提一个 `crates/ironclaw_sandbox` 子 crate。

D-4 + G 完成后准备开工 E 接线，深入排查发现：

1. `SandboxManager`（本仓 `src/sandbox/manager.rs`）在 agent 主进程**从未被实例化**：`SandboxManager::new` / `SandboxManagerBuilder` grep 结果 = 0 处调用
2. `SandboxModeConfig::to_sandbox_config()`（配置 → sandbox 实例的唯一转换函数）grep 结果 = 0 处调用
3. `config.sandbox.enabled` 在 `main.rs` 的 6 处 gate 控制的全部是 `container_job_manager` / `prompt_queue` / `SandboxReaper` / `SandboxReadiness` 枚举，不控制 `SandboxManager`
4. `ShellTool::with_sandbox(...)` 唯一真实调用点在 `src/bin/sandbox_daemon.rs`（独立 binary，本仓**缺失**）

拉上游 `nearai/ironclaw` main 分支（HEAD `9dcd8969`，本地 `ironclaw-main/`）逐项比对，**上游完全一致**：
- 上游 main `SandboxManager::new` 生产调用 = 0 处
- 上游 main `to_sandbox_config` 调用者 = 0 处
- 上游 main 唯一 `ShellTool::with_sandbox` 在 `bin/sandbox_daemon.rs`

**结论**：不是我们改出的 bug，上游就是这个架构。`SandboxManager`（engine v1 代码）是历史残留。

## 上游真实沙箱架构

上游 ironclaw 的生产级沙箱走 **engine v2 + bridge + 进程外 daemon** 路径：

```
agent 主进程（宿主机）
    │
    │  LLM 发起工具调用
    ▼
ironclaw_engine::WorkspaceMounts  ← crates/ironclaw_engine
    │
    ▼
src/bridge/sandbox/intercept::maybe_intercept
    │
    │  SANDBOX_ENABLED=true ?
    ▼
ContainerizedMountFactory → ProjectSandboxManager（每项目一容器）
    │
    ▼
DockerTransport ← NDJSON over "docker exec -i"
    │
    ▼
容器内 sandbox_daemon binary（src/bin/sandbox_daemon.rs）
    │
    ▼
ShellTool / FileReadTool / ... （容器里执行）
```

关键设计特征：
- **进程外隔离**：真实沙箱执行在独立容器里的 `sandbox_daemon` 进程，不是在 agent 主进程里 hook 一个 executor
- **NDJSON wire protocol**：宿主 ↔ daemon 用 JSON-RPC 风格 NDJSON 通信（见 `bridge/sandbox/protocol.rs`）
- **每项目一容器**：`ProjectSandboxManager` 按 `project_id` 缓存 `DockerTransport`
- **按路径前缀拦截**：`/project/` 开头的工具调用走沙箱，其他走宿主机直执行
- **单一开关**：`SANDBOX_ENABLED` 环境变量（`engine_v2_sandbox_enabled()`）切换 `ContainerizedMountFactory` vs `FilesystemMountFactory`

本仓缺失情况（相对上游 main）：
- `crates/ironclaw_engine/` — ❌ 完全缺失
- `src/bridge/` 目录树 — ❌ 完全缺失（本仓无此目录）
- `src/bin/sandbox_daemon.rs` — ❌ 完全缺失
- 落后上游 387 commit（engine v2 + bridge + daemon 都在这 387 里）

## 决策

**Phase 3 不在 agent 主进程接线进程内 sandbox hook。** 具体：

1. `x_claw_agent::SandboxExecutor` trait **保留不删**，作为可选契约
   - 允许未来 wasm 沙箱 / 其他进程内 runtime 按该 trait 接线
   - `NoopSandboxExecutor` 继续作为 `HookBundle::noop()` 默认值
   - Phase 3 的 `hook_bundle_with_safety()` helper 里 `sandbox` 槽永远是 `Noop`

2. `desktop-client/ironclaw/src/sandbox/agent_executor.rs::SandboxAgentExecutor` **保留但加注释**
   - 代码编译通过、有单测，保留不会增加维护负担
   - 文件头注释更新：标明"Phase 3 未接线原因 + Phase 4 将被 engine v2 替代"
   - 作为未来若需要把 engine v1 `SandboxManager` 挂进进程内 hook 的参考实现

3. 真实沙箱能力对齐推到 **Phase 4 Step E-2 ~ E-7**（见 05b 章节"E 章节重新定义"）：
   - E-2 引入 `ironclaw_engine` crate
   - E-3 搬运 `src/bridge/sandbox/`（2624 行）
   - E-4 新增 `[[bin]] sandbox_daemon` target
   - E-5 Dockerfile 打包 sandbox_daemon
   - E-6 端到端集成测试（Docker round-trip）
   - E-7 清理 engine v1 残留

## 理由

**为何不在 Phase 3 合并 E-2~E-7**

- **范围边界**：Phase 3 的目标是"抽出 `x_claw_agent` crate"，不是"对齐上游 engine v2"。合并会让 Phase 3 边界模糊
- **工作量**：上游 387 commit 差距跨 agent loop / workspace / channels 整条链，不是"搬 2624 行"那么简单
- **测试依赖**：engine v2 sandbox 需要真实 Docker 环境 + 容器生命周期 + NDJSON 协议调试，打进 Phase 3 会阻塞整体进度
- **架构目标一致性**："最干净的架构、保留 ironclaw 的安全能力"的前提是先抽出 `x_claw_agent` 稳定，再对齐上游新能力。顺序错了会反复返工

**为何不干脆删 `SandboxAgentExecutor` adapter**

- 代码本身无害（编译通过、有单测），删了节省 ~80 行但增加未来复用成本
- 留作"错误假设的历史记录"，ADR 链接回这个文件，后人能快速理解为什么不用它
- 如果 Phase 4 开始前发现上游 engine v2 路径不适合我们的产品形态（desktop client 不一定按 project_id 分容器），这个 adapter 可能作为 fallback 路径复用

## 对齐 `AGENTS.md` 原则

- ✅ **"复用优先"**：Phase 4 直接 port 上游 engine v2，不自创
- ✅ **"最干净的架构"**：Phase 3 不在边界上埋"看起来在做 sandbox 但实际没在做"的假象
- ✅ **"保留 ironclaw 的安全能力"**：ironclaw 的沙箱能力本来就在上游 main 里，Phase 4 对齐后自然获得
- ✅ **"不要补丁式代码"**：不为了"完成 E" 而硬接一个无消费者的 hook

## 后续动作（本 ADR 采纳后立即执行）

1. 更新 `05b-phase3-execution-plan.md` E 章节（已完成）
2. 更新 `SandboxAgentExecutor` 文件头注释（同 commit）
3. 更新 `x_claw_agent::hooks::SandboxExecutor` trait doc（同 commit）
4. 提交 + 推送子模块 + 父仓

## 参考

- 上游源码：`ironclaw-main/src/bridge/sandbox/`、`ironclaw-main/src/bin/sandbox_daemon.rs`、`ironclaw-main/crates/ironclaw_engine/`
- 上游相关 PR 提示：`crates/ironclaw_engine` 的 `docs/plans/2026-04-10-engine-v2-sandbox.md`（在上游仓库里，本仓未 port）
- 本仓现状 commit：`b70f553e`（submodule x-claw-core）
- 架构总览：`docs/plans/architecture-refactor/02-target-architecture.md`
