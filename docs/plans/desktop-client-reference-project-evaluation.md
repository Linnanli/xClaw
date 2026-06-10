# desktop-client 替换候选评估：reference-projects 五库对比

日期：2026-06-05

补充：2026-06-08，加入 `reference-projects/1code` 与 `open-cowork` 的 app-server 接入适配评估。

## 结论先行

> 证据边界：本轮未拿到语义搜索 MCP / `vscode_listCodeUsages` 接口。以下排序是基于 `code-review-graph` 图谱/FTS、`rg`、README 与关键源码阅读得到的工程判断；涉及“未观察到某能力”的表述均按有限工具下的观察处理，不作为源码级缺失断言。

推荐不要直接把 `desktop-client` 整体替换成任一参考库。更稳的路线是：

1. 短期：保留现有 `desktop-client`，把参考库中成熟的 UI / 会话管理 / app-server 交互模式作为局部移植对象。
2. 中期：以 `crates/dasclaw_cli` / `dasclaw_runtime` 为核心，新增一个 thin daemon 或 app-server 兼容层，再让前端客户端连接这个协议层。
3. 长期：如果确实要换客户端壳，优先从 `CodexMonitor` 分叉改造；如果愿意接受 Electron/Node 技术栈，可把 `open-cowork` 作为消费级客户端 shell 的第二候选；如果要多 agent、多运行时、大量治理 UI，则参考 `desktop-cc-gui`，但不建议直接替换；`open-design` 不适合作为我们的主客户端替代品，只适合作为 daemon / adapter / plugin marketplace 的架构参考。

2026-06-08 补充判断：如果只在 `1code` 与 `open-cowork` 之间选，`1code` 的成品客户端能力更宽，尤其是 Codex / Claude 双运行时、Git worktree、内建终端、diff、远端会话浏览；但更适合接入我们的 `dasclaw app-server` 的仍是 `open-cowork`。原因是 `open-cowork` 的本地 MCP / Skills / permission / VM sandbox 已经以 Electron main + preload API 形式成体系存在，PoC 只需要把 `ClaudeAgentRunner` 替换为 `DasclawRuntimeClient`；`1code` 的远端 sandbox / background agent 明显依赖 21st.dev 后端，接入时要先拆掉云服务耦合和 Codex/Claude 专用 router。

候选排序：

| 排名 | 参考库 | 适合作为主客户端吗 | 核心判断 |
|---|---|---:|---|
| 1 | `reference-projects/CodexMonitor` | 中高 | Tauri + React + Codex app-server，体量最小，替换成本最低，适合改造成 `dasclaw_cli` 的 GUI 壳 |
| 2 | `reference-projects/open-cowork` | 中 | Electron + React + Claude/pi-coding-agent runner，MCP/Skills/权限/VM sandbox 很完整；若核心改接 `dasclaw app-server`，需要替换 agent runner，但壳层能力最贴近 PoC 计划 |
| 3 | `reference-projects/1code` | 中 | Electron + React + tRPC IPC，Codex/Claude/Git/worktree/terminal/远端会话能力更强；但云端 sandbox 和 background agents 依赖 21st.dev 后端，适合做产品能力参考，不如 `open-cowork` 适合作为 app-server PoC 基座 |
| 4 | `reference-projects/desktop-cc-gui` | 中 | 功能最完整，Codex/Claude/OpenCode 运行时抽象更强，但复杂度高，适合借鉴能力，不适合一口气替换 |
| 5 | `reference-projects/open-design` | 低 | Electron + Node daemon + 设计 artifact 平台，产品方向不同，适合作为 adapter / plugin / daemon 思路参考 |

## 方法与工具

任务启动 4 问结果：

| 问题 | 答案 | 处理 |
|---|---|---|
| 是否新增模块 / crate / 文件 | 是，新增本文档 | 先查是否已有同类替换评估 |
| 是否包含否定性结论 | 是 | 用图谱、源码精确搜索、关键文件阅读交叉验证 |
| 是否跨项目对账 | 是 | 对 `reference-projects/*`、`desktop-client`、`crates/dasclaw_cli` 做能力对账 |
| 是否架构对账文档 | 是 | 不只看 README，补充图谱与源码证据 |

工具情况：

| 工具 | 结果 | 本次用途 |
|---|---|---|
| `graphify` | 本轮未成功调用；`graphify` 不在 PATH，按 skill 安装 `graphifyy` 失败 | 未用于结论 |
| `code-review-graph` | 可用；四个参考库已有 `.code-review-graph/graph.db` | 用于规模、节点、边、概念命中、关键符号定位 |
| 语义搜索 MCP / `vscode_listCodeUsages` | 本轮工具系统没有暴露 | 不能声称已执行；以 code-review-graph FTS / 节点样本 + `rg` + 关键文件阅读补证 |
| `rg` | 可用 | 精确验证 app-server、MCP、terminal、sandbox、Tauri/Electron、dasclaw 等字面量 |

过程透明度记录：

`过程记录：本轮新增本文档；语义搜索 MCP / vscode_listCodeUsages 未暴露，不能声称完成仓库规约里的三层验证。已用 code-review-graph 图谱/FTS、精确 rg、README 与关键源码阅读交叉整理；本文所有“未观察到/不建议”均是有限工具下的评估措辞。`

2026-06-08 补充过程记录：

`过程记录：本轮更新本文档，比较 1code 与 open-cowork 谁更强、谁更适合接入 dasclaw app-server。semantic_search / vscode_listCodeUsages 工具仍未暴露；已用 code-review-graph CLI status、graph.db 节点/FTS 查询、rg 字面量搜索与关键源码阅读交叉验证。已检查 1code 是否比 open-cowork 更适合作为 app-server 接入基座，结论：1code 产品能力更宽，open-cowork 更适合作为 Electron app-server PoC 基座。`

## 图谱规模对比

`code-review-graph status --repo <path>` 显示四个参考库已有现成图谱：

| 项目 | 节点 | 边 | 文件 | 语言 | 更新时间 |
|---|---:|---:|---:|---|---|
| `CodexMonitor` | 5,021 | 60,806 | 664 | bash, javascript, rust, c, tsx, typescript | 2026-06-05 10:30 |
| `open-cowork` | 3,945 | 37,017 | 381 | python, javascript, powershell, bash, typescript, tsx | 2026-06-05 13:55 |
| `1code` | 3,227 | 24,794 | 511 | typescript, javascript, bash, tsx | 2026-06-08 17:16 |
| `desktop-cc-gui` | 21,725 | 290,024 | 2,014 | bash, javascript, python, typescript, rust, tsx | 2026-06-05 10:31 |
| `open-design` | 26,069 | 327,104 | 2,071 | bash, powershell, typescript, javascript, tsx, python | 2026-06-05 10:31 |

代码/文档形态：

| 项目 | Tauri 文件 | Rust 文件 | TS/TSX 文件 | package 数 | docs/markdown | 结构含义 |
|---|---:|---:|---:|---:|---:|---|
| `CodexMonitor` | 49 | 21 | 62 | 1 | 7 | 单应用 Tauri 客户端，Rust 后端较集中 |
| `open-cowork` | 0 | 0 | 143 | 2 | 13 | 单应用 Electron 客户端，Node 主进程内 agent runner / MCP / sandbox |
| `1code` | 0 | 0 | 主要为 TS/TSX | 1 | 多个 README / openspec / AGENTS 文档 | 单应用 Electron 客户端，tRPC IPC 聚合 Claude / Codex / Git / terminal / plugins / sandbox-import |
| `desktop-cc-gui` | 73 | 44 | 146 | 1 | 69 | 单应用 Tauri 客户端，但运行时/治理逻辑明显膨胀 |
| `open-design` | 0 | 0 | 111 | 22 | 623 | pnpm monorepo，Electron/Next/daemon/skills/插件平台 |
| 当前 `desktop-client` | 0 | 171 | 4 | 1 | 21 | Rust/Tauri 嵌入式 engine 为主，前端不是主要复杂度 |
| `crates/dasclaw_cli` | 0 | 57 | 0 | 0 | 1 | headless CLI / library proof point |

## 补充：1code vs open-cowork

### 一句话结论

`1code` 的成品能力更强，`open-cowork` 更适合接入我们的 `dasclaw app-server`。

更具体地说：如果目标是“抄一个现代 AI coding desktop 的成品体验”，`1code` 值得重点看；如果目标是“用 Electron 壳消费我们已经在推进的 Rust app-server / sidecar contract”，`open-cowork` 仍是更合适的 PoC 基座。

### code-review-graph 概念命中

以下为 `code-review-graph` 生成的 graph.db 节点/FTS 命中，配合 `rg` 与关键源码阅读使用：

| 概念 | `1code` | `open-cowork` | 判断 |
|---|---:|---:|---|
| `trpc` | 168 | 0 | `1code` 的本地 IPC 类型化更成熟，适合参考 Electron main / renderer API 组织 |
| `codex` | 107 | 0 | `1code` 明确支持 Codex runtime / ACP provider；`open-cowork` 当前主 runner 不在 Codex |
| `claude` | 109 | 481 | `open-cowork` 核心强耦合 Claude/pi-coding-agent；`1code` 是 Claude + Codex 双路径 |
| `mcp` | 78 | 292 | `open-cowork` MCP 管理更集中，server lifecycle / transport / tool discovery 更完整 |
| `skill` | 19 | 409 | `open-cowork` skills/plugin runtime 更重，适合 dasclaw GUI PoC 的壳层插件参考 |
| `plugin` | 19 | 137 | 同上，`open-cowork` 插件生态面更明显 |
| `sandbox` | 7 | 366 | `open-cowork` 有本地 WSL/Lima sandbox；`1code` 主要是远端 sandbox import / preview |
| `permission` | 0 | 53 | `open-cowork` 已有 permission request / response / timeout deny 流程 |
| `terminal` | 174 | 2 | `1code` 内建终端能力明显更强 |
| `git` | 222 | 4 | `1code` Git / changes / PR / worktree 能力明显更强 |
| `worktree` | 67 | 0 | `1code` 每会话 worktree 隔离更成熟 |
| `remote` | 40 | 285 | 两者都涉及 remote；`1code` 更多是 21st.dev 远端会话/云端 sandbox，`open-cowork` 更多是远程控制和 sandbox/agent 运行环境语义 |

### 1code 的强项

`1code` 是一个更完整的成品 AI coding client：

| 能力 | 证据 | 对我们的价值 |
|---|---|---|
| Electron + tRPC IPC 架构 | `src/main/windows/main.ts` 用 `createIPCHandler` 挂 `createAppRouter`；`src/main/lib/trpc/routers/index.ts` 聚合 projects/chats/claude/codex/terminal/files/skills/plugins/changes | 可参考它的 Electron typed IPC 和 router 分层 |
| Codex / Claude 双运行时 | `src/main/lib/trpc/routers/codex.ts` 有 ACP provider、active stream、MCP snapshot、`streamText` subscription；`claude-code.ts` 管 Anthropic OAuth / integration | 产品能力宽，适合作 Codex/Claude 体验参考 |
| Git / worktree / PR / diff | DB schema 中 chat 有 `worktreePath`、`branch`、`baseBranch`、`prUrl`、`prNumber`；图谱 `git=222`、`worktree=67` | 可参考会话隔离、changes UI、PR 工作流 |
| 内建终端 | `src/main/lib/terminal/manager.ts` 管 PTY session、resize、fallback shell、port manager；图谱 `terminal=174` | 可参考 terminal panel / port preview 体验 |
| 本地持久化 | `src/main/lib/db/index.ts` 使用 `better-sqlite3` + Drizzle migrations；schema 覆盖 projects/chats/sub_chats/accounts | 可参考 Electron 本地状态模型 |

### 1code 的 app-server 接入风险

| 风险 | 证据 | 影响 |
|---|---|---|
| 远端 sandbox / background agent 依赖 21st.dev | `remote-trpc.ts` 类型引用 `web/server/api/root`，用 `signedFetch` 调 `https://21st.dev`；`claude-code.ts` 注释写 server creates sandbox，并调 `/api/auth/claude-code/start` | 如果接 dasclaw app-server，需要先拆远端 backend 耦合，不能把云端 sandbox 当作本地可复用实现 |
| 本地 chat runner 是 Codex/Claude 专用 router | `codex.chat` subscription 内部直接构造 ACP provider、MCP snapshot、AI SDK `streamText` | 要接 app-server 需要新增 `dasclawRouter` 或替换 chat path，迁移面大于“换 runner” |
| sandbox 不是本地执行隔离 | 图谱 sandbox 节点集中在 `sandbox-import`、remote API diff/file、preview URL | 对我们 DLP / policy / enterprise sandbox 的映射帮助有限 |

### open-cowork 的 app-server 适配优势

`open-cowork` 更贴近现有 PoC 计划里的“Electron shell + Rust app-server / sidecar”：

| 能力 | 证据 | 对接 dasclaw app-server 的意义 |
|---|---|---|
| 单一 runner 接缝 | `SessionManager` 明确 `Delegates AI execution to ClaudeAgentRunner`，并通过 `AgentRunner` interface 暴露 `run/cancel` | 可以把 `ClaudeAgentRunner` 替换成 `DasclawRuntimeClient`，renderer/session/store 不必一开始全量重写 |
| 本地 MCP 管理 | `MCPManager` 管 stdio / SSE / Streamable HTTP、server lifecycle、tool/resource/prompt discovery | 可作为 dasclaw MCP 设置页和工具发现 UI 的直接参考 |
| Skills / plugin runtime | `SessionManager` 注入 `PluginRuntimeService` 和 `AgentRuntimeExtensionManager`；图谱 `skill=409`、`plugin=137` | 更贴合 PoC 计划中“JS 生态用于壳层，Rust 核心保留”的路线 |
| Permission bridge | `requestPermission` 60 秒超时默认 `deny`，向 renderer 发 `permission.request` | 可映射到 app-server 的 approval request / response contract |
| 本地 VM sandbox | preload 暴露 `sandbox.getStatus/checkWSL/checkLima/startLimaInstance`；Lima agent 有 path containment / validateCommand / executeCommand | 可借鉴为 dasclaw app-server 的可选 sandbox backend UI，而不是依赖第三方云 |

### 二选一建议

| 目标 | 推荐 |
|---|---|
| 做 Electron + dasclaw app-server 最小 PoC | `open-cowork` |
| 借鉴成熟产品体验、Codex/Claude 双 runtime、Git/worktree/terminal | `1code` |
| 做云端 background agent | 不能直接从 `1code` 复用，需另找 21st.dev 后端实现或自建 app-server/cloud runner |
| 保留 Rust safety / DLP / approval / app-server contract | 两者都要重接，但 `open-cowork` 的 permission/sandbox 壳更接近我们要的 contract |

结论：`1code` 更像“完整 AI coding client 产品参考”，`open-cowork` 更像“可替换 runner 的 Electron shell PoC 基座”。因此，当前 `open-cowork based dasclaw GUI PoC` 计划不需要因为 `1code` 加入而改主线；建议把 `1code` 降级为产品体验参考库，重点吸收它的 tRPC IPC、Codex/Claude 切换、Git/worktree、terminal 和远端会话浏览设计。

## 候选一：CodexMonitor

### 适配度

`CodexMonitor` 是三个候选里最像“可以改造成我们客户端”的项目。它的 README 定位就是 Tauri app，用于跨本地 workspace 编排多个 Codex agents；Rust 侧通过 `codex app-server` 维护 workspace session，前端通过 Tauri command / event 接收 app-server 事件。

关键证据：

| 能力 | 证据 |
|---|---|
| Tauri 2 + React 19 + Vite | `reference-projects/CodexMonitor/package.json`、`src-tauri/Cargo.toml` |
| Codex app-server 启动 | `reference-projects/CodexMonitor/src-tauri/src/backend/app_server.rs::spawn_workspace_session` 构造 `codex app-server` |
| workspace / thread / live subscribe | `reference-projects/CodexMonitor/src-tauri/src/codex/mod.rs` 包含 `start_thread`、`resume_thread`、`read_thread`、`thread_live_subscribe`、`fork_thread`、`list_threads` |
| terminal / PTY | `reference-projects/CodexMonitor/src-tauri/src/terminal.rs` 使用 `portable_pty` |
| 远程 daemon 模式 | README 和 `src-tauri/src/codex/mod.rs` 多处 `remote_backend::call_remote` |

### 优点

| 维度 | 判断 |
|---|---|
| 替换成本 | 三者最低；Rust 后端体量约 21 个 `.rs` 文件，app-server 边界集中 |
| 技术栈 | 与 Tauri 客户端方向一致；无需引入 Electron |
| 产品形态 | workspace/thread/terminal/git/GitHub/agent controls 与客户端需求接近 |
| 可改造性 | 可以把 `codex app-server` 启动点替换为 `dasclaw-cli daemon/app-server` 或直接链接 `dasclaw_runtime` |

### 风险

| 风险 | 说明 |
|---|---|
| 协议绑定 Codex app-server | 当前事件、request/response、thread live 都围绕 Codex app-server JSON-RPC；`dasclaw_cli` 目前不是 app-server |
| 安全能力需重新接入 | 本轮图谱与精确搜索未观察到 `dasclaw_safety` / enterprise sandbox 等本仓强约束；如果采用它，仍需重新接回企业安全线 |
| 数据模型需重接 | workspace/thread 与本仓 owner_id、policy、conversation_tracker、admin sync、routine/job 概念不一致 |

### 可行性

可行，但不是“替换即可”。需要先补一个协议适配层：

1. 将 `dasclaw_cli` 扩展为可长驻的 `dasclaw app-server` / daemon，输出稳定 JSON-RPC 或 SSE 事件。
2. 或者在 Tauri Rust 后端直接调用 `dasclaw_runtime::Agent` / `dasclaw_cli` library entry，而不是 spawn CLI。
3. 保留 CodexMonitor 的 UI shell、workspace/thread/terminal 模型，逐步替换后端 session 类型。

推荐用途：`首选分叉基底`，适合做 thin client PoC。

## 候选二：open-cowork

### 适配度

按本轮入口观察，`open-cowork` 是 Electron + React + Node 主进程的一体化 AI agent 桌面客户端；当前可见主线更接近主进程内 runner，而不是 `app-server` 型架构。它的核心是 `SessionManager` 在 Electron main process 内创建 `ClaudeAgentRunner`，再通过 `@mariozechner/pi-coding-agent` / `pi-ai` 跑 agent loop，并把 MCP、Skills、权限、sandbox、远程控制、GUI automation 串进这个 loop。

`code-review-graph` 概念命中显示：

| 概念 | 命中数 | 解释 |
|---|---:|---|
| `claude` | 481 | 主运行时围绕 Claude / Anthropic / pi-coding-agent |
| `mcp` | 292 | MCP 是一等能力，有 manager、bundled MCP、GUI operate server |
| `sandbox` | 366 | WSL/Lima/PathResolver 是核心安全能力 |
| `agent` | 260 | 主进程内 agent runner / extension manager |
| `session` | 277 | SQLite 持久化 session + prompt queue + permissions |
| `codex` | 0 | 本轮未观察到 Codex app-server / Codex CLI 绑定 |
| `dasclaw` | 0 | 本轮未观察到 dasclaw 绑定 |
| `tauri` | 0 | 本轮观察为 Electron/Node 技术栈 |
| `daemon` | 0 | 本轮未观察到独立 daemon 架构 |
| `pty` | 0 | 本轮未观察到 xterm/PTY 型终端能力 |

关键证据：

| 能力 | 证据 |
|---|---|
| Electron + React + Vite | `reference-projects/open-cowork/package.json`、`vite.config.ts`、`electron-builder.yml` |
| 主进程会话管理 | `src/main/session/session-manager.ts` 注释说明 Session CRUD、SQLite、workspace-scoped sessions、sandbox integration、delegates to `ClaudeAgentRunner` |
| Agent runner | `src/main/claude/agent-runner.ts` 使用 `@mariozechner/pi-coding-agent` 的 `createAgentSession`、`createCodingTools`，负责 provider routing、MCP tools bridge、stream events、skills injection、permission handling |
| MCP 管理 | `src/main/mcp/mcp-manager.ts` 管 stdio / SSE / Streamable HTTP、server lifecycle、OAuth、tool/resource/prompt discovery |
| GUI automation MCP | `src/main/mcp/gui-operate-server.ts` 提供 click/type/scroll/screenshot/display tools |
| VM sandbox | `src/main/sandbox/lima-agent/index.ts`、`lima-bridge.ts`、WSL/Lima adapter；README 写 Windows WSL2、macOS Lima |
| 权限弹窗 | `SessionManager.requestPermission` / `requestSudoPassword`，60 秒超时默认 deny |
| 安全 IPC | `src/preload/index.ts` 通过 `contextBridge` 暴露 allowlist 的 `ClientEvent`，不暴露完整 `ipcRenderer` |

### 优点

| 维度 | 判断 |
|---|---|
| 产品完整度 | 比 `CodexMonitor` 更像可直接面向普通用户的 AI desktop app：配置、MCP、Skills、权限、远程控制、GUI automation、sandbox setup 都齐 |
| 体量 | 图谱 3,945 节点 / 37,017 边，甚至小于 `CodexMonitor`，远低于 `desktop-cc-gui` / `open-design` |
| MCP / Skills | MCP manager 和 plugin/skills runtime 边界清晰，可借鉴度高 |
| 权限体验 | 已有 tool permission 和 sudo password dialog 流程，适合作为 UI/UX 参考 |
| Sandbox | WSL2 / Lima 的 VM 级隔离比纯路径守卫更接近“普通用户可理解”的安全故事 |
| GUI automation | 自带 GUI operate MCP server，适合借鉴到未来 dasclaw tool/plugin 生态 |

### 风险

| 风险 | 说明 |
|---|---|
| 技术栈偏离 | 它是 Electron/Node，不是 Tauri/Rust；直接采用会绕开本仓现有 Rust 安全与 runtime 投资 |
| 核心运行时不匹配 | 当前核心是 `ClaudeAgentRunner` + `pi-coding-agent`，不是 `dasclaw_cli` / `dasclaw_runtime` / app-server |
| Codex/dasclaw app-server 模型需验证/补接 | 本轮图谱和 rg 未观察到 Codex app-server / dasclaw 绑定；要接 dasclaw 需替换 agent runner 或新增 dasclaw runner |
| Sandbox 语义不同 | WSL/Lima 是 VM/路径守卫/命令模式，和本仓 `dasclaw_sandbox` 的 fail-closed enterprise sandbox 不是同一套安全契约 |
| Linux 不是主平台 | README 主打 Windows/macOS；electron-builder 有 Linux target，但产品说明和 sandbox 重点不在 Linux |
| 终端能力需专题验证 | 本轮图谱 `pty=0`，不像 `CodexMonitor` 有明确的 `portable_pty` 终端 dock 证据 |

### 可行性

如果目标是“做一个全新的 Electron 版 dasclaw 消费级客户端”，`open-cowork` 有中等可行性；如果目标是“低风险替换当前 Tauri/Rust `desktop-client`”，可行性偏低。

可行路线有两条：

1. `替换 runner 路线`：保留 Electron shell / SessionManager / MCP UI / Skills UI / permission UI，把 `ClaudeAgentRunner` 替换为 `DasclawAgentRunner`，后者 spawn 或链接 `dasclaw_cli` / `dasclaw app-server`。
2. `借鉴模块路线`：不采用整库，只迁移 MCP manager UX、permission dialog、sandbox setup、GUI operate MCP server、skills/plugin runtime 设计。

推荐用途：`Electron 消费级客户端参考`。如果我们决定新客户端不再坚持 Tauri/Rust shell，它可以作为 PoC 基底；否则更适合作为 UI/UX 与 MCP/sandbox/permission 参考库。

## 候选三：desktop-cc-gui

### 适配度

`desktop-cc-gui` 是功能最接近“成熟多 agent 客户端”的参考库。它比 `CodexMonitor` 多了统一 engine abstraction、Codex adapter、Claude/OpenCode/Gemini 等运行时概念、plan enforcement、runtime lifecycle、capability matrix、多种 runtime contracts 和性能/质量脚本。

关键证据：

| 能力 | 证据 |
|---|---|
| Tauri 2 + React 19 + Vite | `reference-projects/desktop-cc-gui/package.json`、`src-tauri/Cargo.toml` |
| Codex app-server 封装增强 | `src-tauri/src/backend/app_server.rs` 中有 `RuntimeShutdownSource`、turn timeout、plan state、runtime manager |
| 统一 engine adapter | `src-tauri/src/engine/codex_adapter.rs` 将 Codex events 转成统一 `EngineEvent` |
| Claude runtime | `src-tauri/src/engine/claude.rs` 管理 Claude stream-json、process lifecycle、interrupt/error |
| MCP config | `src-tauri/src/codex/mcp_config.rs` 解析 `.claude.json` / app config 里的 MCP servers |
| 质量门禁 | `package.json` 中大量 `check:*`、`perf:*`、runtime contract 脚本 |

### 优点

| 维度 | 判断 |
|---|---|
| 功能完整度 | 三者最高，已具备多 runtime、多 session、usage、terminal、Git、MCP、policy router 等概念 |
| 架构抽象 | `EngineEvent` / runtime adapter 模式值得借鉴，可作为 `dasclaw` UI 事件层设计参考 |
| 产品成熟度 | README 覆盖 terminal、git panel、session activity、AI runtime safety、engine capability matrix |

### 风险

| 风险 | 说明 |
|---|---|
| 复杂度高 | 图谱 21,725 节点 / 290,024 边，远高于 `CodexMonitor`；直接替换会把大量外部工程复杂度带入主线 |
| 运行时假设多 | 代码中存在 Codex/Claude/OpenCode 等多引擎路径；如果核心只用 `dasclaw_cli`，多数抽象会变成迁移负担 |
| 与本仓安全模型需重新映射 | 本轮搜索命中 `sandbox` 主要是 Codex access mode 转换，`safety` 命中较少；采用时仍需接回本仓 `dasclaw_safety` / enterprise sandbox 体系 |
| 命名/品牌/治理耦合 | package 脚本与 env 中有 `MOSSX_*`、ccgui branding、runtime evidence gates，需要大量清理 |

### 可行性

技术上可行，但工程上不建议做“全量替换”。更适合拆成可借鉴模块：

1. 借鉴 `EngineEvent` / `CodexSessionAdapter`，为 `dasclaw_runtime` 定义 UI event contract。
2. 借鉴 runtime lifecycle / timeout / startup buffering，增强我们的客户端启动与错误体验。
3. 借鉴 MCP config UI 和 capability matrix，但底层仍接入本仓 `dasclaw_mcp` / `dasclaw_cli`。

推荐用途：`能力参考库`，不是首选替换基底。

## 候选四：open-design

### 适配度

`open-design` 是另一类系统：Next.js web app + Node daemon + Electron desktop shell + skills/design systems/plugins/artifacts。它的核心不是通用 agent chat 客户端，而是本地优先设计 artifact 生成平台。

关键证据：

| 能力 | 证据 |
|---|---|
| 三种部署拓扑 | `reference-projects/open-design/docs/architecture.md` 描述 local web + daemon、Vercel + daemon、direct API |
| daemon 负责 agent adapter pool | `docs/architecture.md` §3.2/§3.3 |
| 多 agent adapter contract | `docs/agent-adapters.md` 定义 `AgentAdapter.detect/capabilities/run/cancel/resume` |
| Codex 只是 adapter 之一 | `apps/daemon/src/agents.ts` re-export runtimes registry；README 支持 21 个 CLI |
| Electron desktop shell | `apps/desktop/src/main/runtime.ts` 使用 Electron `BrowserWindow`、IPC、webview allowlist、安全路径校验 |
| 插件/技能/设计资产生态 | README 中 100+ skills、150 design systems、261 plugins；目录也显示 `skills/`、`plugins/`、`design-systems/`、`design-templates/` 是主体 |

### 优点

| 维度 | 判断 |
|---|---|
| daemon / adapter 思路 | 很适合借鉴：本地 daemon 保持权限与秘密，web/desktop 只是 UI |
| plugin / skill marketplace | 对未来 dasclaw skill/plugin 分发有参考价值 |
| 安全边界意识 | Electron 主进程中有路径校验、webview allowlist、HMAC token 等安全设计 |

### 风险

| 风险 | 说明 |
|---|---|
| 技术栈不匹配 | 从 Tauri/Rust 迁到 Electron/Node，会偏离本仓 Rust 安全与运行时复用路线 |
| 产品方向不匹配 | open-design 的主域是 design artifact / preview / export，而不是 dasclaw agent runtime 客户端 |
| 与 `dasclaw_cli` 集成会绕远 | 它把 agent loop 委托给外部 CLI adapter，而我们已经有 `dasclaw_runtime` / `dasclaw_cli` 可作为内核 |
| 资产体量大 | 623 个 markdown、22 个 package、设计系统/模板/插件占主体；引入会带来大量非客户端资产 |

### 可行性

不建议作为 `desktop-client` 替换基底。可借鉴：

1. daemon / web / desktop 三拓扑分离。
2. `AgentAdapter` capability negotiation。
3. plugin / skill manifest 与 marketplace。
4. Electron 安全边界设计中的路径 allowlist、trusted picker token、webview sandbox 思路。

推荐用途：`架构灵感库`，不是客户端替代库。

## 与当前 `desktop-client` / `dasclaw_cli` 的关键差异

当前 `desktop-client` 的核心不是“一个壳 spawn 一个 CLI”，而是在 Tauri 进程内初始化 IronClaw / dasclaw 组件：

| 当前能力 | 证据 | 替换影响 |
|---|---|---|
| 内嵌 engine 初始化 | `desktop-client/src/engine.rs::start_ironclaw_engine` 调用 `Config::from_env`、`AppBuilder::build_all`、`Agent::new`、`agent.run` | 替换客户端要重建 engine lifecycle |
| 安全 / egress gate | `desktop-client/src/engine.rs` 创建 `SafetyBridge` 和 `IronclawEgressGate` | 参考库不能直接替代安全层 |
| 数据上报 / admin sync | `desktop-client/src/engine.rs` 管理 `AdminConfigSync`、`ConversationTracker`、`DataReporter` | 新客户端要明确保留或砍掉 |
| job/routine/tools | `register_message_tools`、`register_job_tools`、`RoutineEngine`、scheduler slot | thin CLI shell 不天然具备 |
| Tauri channel | `TauriChannel` + `ChannelManager` | 参考库 app-server events 需要映射到现有 UI stream |

`crates/dasclaw_cli` 当前定位更轻：

| 能力 | 当前状态 | 对客户端替换的含义 |
|---|---|---|
| Headless agent proof point | README 明确 no Tauri / no database / no channels / no HTTP server | 不能直接当完整 desktop backend |
| `run` / `run_with_tools` / `run_with_tools_and_safety_sanitizer` | library entry 已有 | 可作为 daemon/app-server 内核 |
| MCP config | `--mcp-config` 支持 stdio / HTTP | 可接 GUI MCP 配置，但 hosted/OAuth/secrets store 未做 |
| Shell tool sandbox | `--enable-shell-tool` 走 default-deny sandbox | 很适合保留，但需要 UI approval/error 表达 |
| 不支持 streaming REPL | README 明确 streaming / interactive REPL 不在当前 slice | GUI 需要补事件流，否则体验会退化 |
| approval prompts | README 写 headless mode surfaces `AgentError::ApprovalRequested` | GUI 要补 approval bridge，而不是让 CLI 卡住 |

因此，`dasclaw_cli` 适合作为新客户端的核心，但前提是新增一层长期运行协议：

```text
GUI shell
  -> Tauri command / HTTP / WebSocket / JSON-RPC
  -> dasclaw daemon / app-server compatibility layer
  -> dasclaw_runtime::Agent + dasclaw_cli library pieces
  -> dasclaw_* safety / mcp / tools / sandbox / provider crates
```

## 替换可行性分级

| 方案 | 可行性 | 预计收益 | 主要成本 | 建议 |
|---|---:|---|---|---|
| A. 直接用 `CodexMonitor` 替换 `desktop-client` | 中 | 快速得到现代 Tauri UI、workspace/thread/terminal | 协议从 Codex app-server 改 dasclaw；安全/admin/job 要补 | 可做 PoC，不直接主线替换 |
| B. 直接用 `open-cowork` 替换 `desktop-client` | 中低 | Electron 消费级 shell、MCP/Skills/权限/VM sandbox 成熟 | 要重写 runner 接 dasclaw，且放弃 Tauri/Rust shell | 可做 Electron PoC，不建议直接主线替换 |
| C. 直接用 `desktop-cc-gui` 替换 | 中低 | 功能完整，多 runtime 抽象现成 | 复杂度和外部假设过高，迁移风险大 | 不建议 |
| D. 直接用 `open-design` 替换 | 低 | daemon/plugin/adapter 生态强 | 技术栈和产品域偏离 | 不建议 |
| E. 保留 `desktop-client`，移植参考库 UI/adapter 片段 | 高 | 风险低，保留现有安全/engine | 视觉与交互需要逐步重构 | 推荐短期路线 |
| F. 新建 thin client，`CodexMonitor` 作基底，`dasclaw_cli` 作 daemon 内核 | 中高 | 客户端壳清爽，核心复用 dasclaw crates | 需要先定义 app-server/daemon 协议 | 推荐中期 PoC |
| G. 新建 Electron client，`open-cowork` 作基底，`dasclaw_cli` 作 runner/app-server 内核 | 中 | 消费级安装、MCP、Skills、权限体验更快成型 | Rust 安全栈要跨进程接回，runner 替换面大 | 备选 PoC |

## 推荐路线

### Phase 0：定义不可丢能力清单

在动替换前，先把当前 `desktop-client` 的不可丢能力列成 contract：

| 能力 | 是否必须保留 |
|---|---|
| `dasclaw_safety` / egress gate / tool-output sanitizer | 必须 |
| enterprise sandbox fail-closed | 必须 |
| MCP stdio/HTTP tool loading | 必须 |
| approval bridge | 必须 |
| streaming turn events | 必须 |
| conversation tracking / admin sync | 取决于产品路线 |
| routine/job scheduler | 取决于产品路线 |
| terminal PTY | 建议保留 |
| Git/GitHub panel | 可分阶段 |

### Phase 1：做 `dasclaw app-server` PoC

不要先换 UI。先给 `dasclaw_cli` / `dasclaw_runtime` 补一个最小 daemon：

1. `session/start`
2. `turn/send`
3. `turn/cancel`
4. `thread/read`
5. `tools/list`
6. `mcp/status`
7. event stream: `turn/started`、`text/delta`、`tool/start`、`tool/result`、`approval/requested`、`turn/completed`、`turn/error`

这一步完成后，`CodexMonitor` / `desktop-cc-gui` 的 UI 才有稳定目标可接。

### Phase 2：用 CodexMonitor 做 thin client PoC

把 CodexMonitor 的启动点从 `codex app-server` 换成 `dasclaw app-server`：

| CodexMonitor 现有点 | dasclaw 替换点 |
|---|---|
| `build_codex_command_with_bin(..., ["app-server"])` | `build_dasclaw_command(..., ["app-server"])` |
| `WorkspaceSession` JSON-RPC pending map | 保留，改 method/event schema |
| `codex_core::*_core` | 换成 `dasclaw_core_client::*` |
| `thread_live_subscribe` | 对应 dasclaw event subscription |
| `terminal.rs` | 可先原样保留 |

### Phase 3：回迁到当前 `desktop-client` 或替换壳二选一

PoC 跑通后再决策：

| 结果 | 动作 |
|---|---|
| CodexMonitor 壳明显更好，安全/admin/job 可补齐 | 开 stacked PR 逐步迁移 |
| 当前 `desktop-client` 的 Rust 内嵌 engine 优势更大 | 只回迁 UI/adapter/terminal 改进 |
| `desktop-cc-gui` 的 runtime abstraction 更合适 | 局部移植 `EngineEvent` / adapter 层，不迁整库 |

## 最终建议

如果目标是“尽快有一个可以围绕 `dasclaw_cli` 运转的现代客户端”，选 `CodexMonitor` 做 PoC。

如果目标是“做一个面向普通用户的一键安装 Electron 客户端，并快速拥有 MCP / Skills / 权限弹窗 / VM sandbox / GUI automation”，`open-cowork` 值得做备选 PoC，但要接受重写 agent runner 接入 `dasclaw_cli` 的成本。

如果目标是“未来支持多 agent、多 CLI、多运行时能力矩阵”，参考 `desktop-cc-gui` 的 adapter/event/lifecycle 设计，但不要整库替换。

如果目标是“构建插件、skills、artifact、设计系统生态”，参考 `open-design`，但它不应该成为 `desktop-client` 的替代基底。

一句话判断：`CodexMonitor` 适合做 Tauri thin shell，`open-cowork` 适合做 Electron consumer shell，`desktop-cc-gui` 适合抄多 runtime 架构，`open-design` 适合抄生态；真正的核心仍应是我们的 `dasclaw_runtime` / `dasclaw_cli` / `dasclaw_*` crates。
