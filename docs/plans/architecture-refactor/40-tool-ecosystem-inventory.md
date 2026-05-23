# 40 — 四方工具生态盘点（crates/ vs claude-code-main vs codex-cli-main vs ironclaw-main）

> **生成日期**: 2026-04-30
> **生成目的**: 为 F4.6 builtin tools 下沉决策提供数据底座；同时回答"哪些工具我们可以不要"
> **数据来源**: 四方 repo 直接 `ls` + 每个 dasclaw_* crate 的 `lib.rs` 头部注释（明文标注搬运来源）
> **方法论**: 严格按 AGENTS.md 三层验证规约（semantic_search 失败 → 回退 grep + ls，符合 code-review-graph-usage 坑位 4）

---

## 1. 总数对照

| 来源 | 工具组织方式 | 工具/文件数 | 备注 |
|---|---|---|---|
| **crates/** (当前 x-claw) | 45 个 dasclaw_* crate（含底座、工具、运行时） | 工具/能力相关 17 个 | 工具壳层未下沉 |
| **desktop-client/ironclaw**（当前活跃实现） | `src/tools/builtin/*.rs` 平铺 | 30 文件 + `git/` 9 + `lsp/` 10 | F4.6 真正要搬的源 |
| **ironclaw-main**（fork 上游） | `src/tools/builtin/*.rs` 平铺 | 27 文件 | desktop 的来源 |
| **codex-cli-main**（codex 路线） | 双层：`codex-rs/tools/` 24 spec + `codex-rs/core/src/tools/handlers/` 20 handler | 24 + 20 = 44 工件 | 双层架构 |
| **claude-code-main**（claude-code 参考） | `src/tools/{ToolName}/` TS 子目录 | **43 个工具** | TypeScript，非 Rust 移植目标 |

---

## 2. 当前 crates/ 的 17 个工具/能力 crate 来源（lib.rs 注释明文）

| crate | 行数 | 搬运来源（注释原文摘录） | 角色 |
|---|---|---|---|
| `dasclaw_apply_patch` | 1277 | "ported from codex-cli-main/codex-rs/apply-patch" | **底座**：patch 解析与应用 |
| `dasclaw_shell_command` | 3348 | "shared across Codex crates" | **底座**：shell 解析 + 安全 |
| `dasclaw_bash_permissions` | 多文件 | "port of upstream bashPermissions.ts (2621 LOC)"（**claude-code TS port**） | **底座**：bash 权限规则 |
| `dasclaw_bash_validation` | 多文件 | bash 校验子模块（codex/ironclaw 混源） | **底座**：bash 注入检测 |
| `dasclaw_execpolicy` | - | （drift guard `check_codex_execpolicy_drift.py`）→ codex | **底座**：Starlark 权限规则 |
| `dasclaw_net_proxy` | - | （drift guard）→ codex `network-proxy` | **底座**：HTTP 代理 + 域名 allowlist |
| `dasclaw_tool` | 977 | "Tool implementation-layer primitives shared across dasclaw hosts" | **框架**：Tool trait + 错误 + 参数 |
| `dasclaw_exec` | 687 | "W2.6 整合层" | **底座**：执行整合层 |
| `dasclaw_git_tools` | 24（空壳） | "fork private cargo, ~1,331 LOC"（**desktop 路线**） | **工具壳**：等 F4.6 填充 |
| `dasclaw_lsp` | 多文件 | "LSP integration: registry, JSON-RPC client" | **工具壳 + 底座** |
| `dasclaw_mcp` | 多文件 | "MCP building blocks" | **底座**：MCP transport |
| `dasclaw_wasm_tools` | 多文件 | "WASM sandbox primitives" | **底座**：WASM 工具沙箱 |
| `dasclaw_routines` | 1634 | "fork ironclaw 0.24 routines module promotion" | **能力域**：例程编排 |
| `dasclaw_channels` | - | "fork ironclaw 0.24 channels module promotion" | **能力域**：通道抽象 |
| `dasclaw_safety` | 大 | "Safety layer for prompt injection defense"（**ironclaw 路线**） | **能力域**：DLP/安全 |
| `dasclaw_core` | 大 | "agent runtime fork of claw-code" | **L3 Agent Runtime** |
| `dasclaw_runtime` | 大 | "Application-layer runtime types shared across dasclaw hosts" | **L3 运行时类型** |

**关键观察**：
- ✅ **底座层完整**：patch 解析、shell 解析、bash 权限/校验、execpolicy、net_proxy 已全部下沉
- ❌ **工具壳层缺失**：当前 28 个 builtin 工具壳（file.rs / shell.rs / glob_search.rs / grep_search.rs 等）一份都在 desktop-client/ironclaw/src/tools/builtin/，无 crates/ 副本
- ⚠️ **dasclaw_git_tools 是空壳**：lib.rs 仅 24 行注释，等 F4.6 把 desktop git/ 9 文件 1326 行搬下来

---

## 3. 四方工具明细对照表（按职能分组）

### 类型 I：四方都有 — **框架核心，绝对不能砍**

| 工具职能 | claude-code-main | codex-cli-main | ironclaw-main | desktop（当前） | 砍/留 |
|---|---|---|---|---|---|
| 文件读 | FileReadTool | `handlers/list_dir`（间接） | `file.rs` | `file.rs` | ✅ 留 |
| 文件写 | FileWriteTool | `tools/apply_patch_tool` | `file.rs` | `file.rs` | ✅ 留 |
| 文件编辑 | FileEditTool | `handlers/apply_patch` | `file.rs` | `code_edit.rs` | ✅ 留 |
| Shell 执行 | BashTool / PowerShellTool | `handlers/shell` + `unified_exec` | `shell.rs` | `shell.rs` | ✅ 留 |
| Glob 搜索 | GlobTool | （隐含于 list_dir） | `glob_tool.rs` | `glob_search.rs` | ✅ 留 |
| Grep 搜索 | GrepTool | `handlers/grep_files` | `grep_tool.rs` | `grep_search.rs` | ✅ 留 |
| WebFetch | WebFetchTool | （无） | `http.rs` | `http.rs` + `web_fetch.rs` | ✅ 留 |

**小计：7 类**，约 4000-5000 行代码，必须搬到 crates/。

### 类型 II：claude-code + ironclaw 有 — 取决于框架定位

| 工具职能 | claude-code | ironclaw-main | desktop（当前） | 砍/留 |
|---|---|---|---|---|
| 计划模式（plan） | EnterPlanModeTool / ExitPlanModeTool | `plan.rs` | `plan_mode.rs` | ✅ 留（ADR-153 agent 能力） |
| Memory 长期记忆 | （内置 Task 系统） | `memory.rs` | `memory.rs` | ✅ 留 |
| Tool 自省 | ToolSearchTool | `tool_info.rs` | `tool_info.rs` | ✅ 留 |
| Web 搜索 | WebSearchTool | （无单独） | `web_search.rs` | ⚠️ 可选（desktop 加的） |
| LSP | LSPTool | （无） | `lsp/` + `dasclaw_lsp` | ⚠️ 可选 |
| Skill | SkillTool | `skill_tools.rs` | `skill_tools.rs` | ❌ **砍**（desktop 专属） |
| 子 Agent | AgentTool / TaskCreateTool | （无单独） | `sub_agent.rs` | ⚠️ 可选 |
| JSON 操作 | （无） | `json.rs` | `json.rs` | ✅ 留 |
| 时间工具 | （无） | `time.rs` | `time.rs` | ✅ 留 |
| Path 工具 | （无） | `path_utils.rs` | `path_utils.rs` | ✅ 留 |
| Echo（调试） | （无） | `echo.rs` | `echo.rs` | ✅ 留 |
| HTML 转换 | （无） | `html_converter.rs` | `html_converter.rs` | ⚠️ 可选 |

**小计**：核心 6 类（plan/memory/tool_info/json/time/path）+ 可选 6 类。

### 类型 III：只有 ironclaw 路线有 — **desktop 部署专属，建议留 desktop 不下沉**

| 工具职能 | ironclaw-main | desktop（当前） | 行数 | 是否下沉 |
|---|---|---|---|---|
| Job 管理 | `job.rs` | `job.rs` | 2359 | ❌ **留 desktop**（绑 orchestrator，ADR-155 已决定 orchestrator 留 desktop） |
| 频道消息 | `message.rs` | `message.rs` | 1075 | ❌ **留 desktop**（绑 Tauri channels） |
| 重启 | `restart.rs` | `restart.rs` | 482 | ❌ **留 desktop**（Tauri 进程概念） |
| 扩展管理 | `extension_tools.rs` | `extension_tools.rs` | 828 | ❌ **留 desktop**（desktop 形态） |
| Secrets 检查工具 | `secrets_tools.rs` | `secrets_tools.rs` | 218 | ❌ **留 desktop**（底座 `dasclaw_runtime::secrets` 已搬） |
| Routines 工具壳 | `routine.rs` | `routine.rs` | 2639 | ⚠️ 看是否搬入 `dasclaw_routines` |
| 图像分析 | `image_analyze.rs` | `image_analyze.rs` | 257 | ❌ **留 desktop**（云 API） |
| 图像编辑 | `image_edit.rs` | `image_edit.rs` | 329 | ❌ **留 desktop**（云 API） |
| 图像生成 | `image_gen.rs` | `image_gen.rs` | 247 | ❌ **留 desktop**（云 API） |
| Session Fork | （无） | `session_fork.rs` | 173 | ⚠️ 可选 |
| File Guard | `file_edit_guard.rs` / `file_history.rs` | `file_guard.rs` | 184 | ✅ 留（搬随 file 工具） |
| System 信息 | `system.rs` | （已并入其他） | - | - |

**小计**：可砍/留 desktop 的有 8 类（job/message/restart/extension/secrets_tools/image_*3），共 ~6795 行。

### 类型 IV：claude-code 独有 — **当前不需要**

| 工具 | 说明 | 砍/留 |
|---|---|---|
| TodoWriteTool / TaskCreate/Update/Get/List/Stop/Output | claude-code 任务管理系统（与 ironclaw routines/job 不同形态） | ❌ 全砍 |
| EnterWorktreeTool / ExitWorktreeTool | git worktree 管理 | ❌ 砍 |
| TeamCreateTool / TeamDeleteTool | 团队管理 | ❌ 砍 |
| REPLTool | 交互式 REPL | ❌ 砍 |
| SleepTool | 等待 | ❌ 砍 |
| ScheduleCronTool | cron 定时 | ❌ 砍（routines 已覆盖） |
| RemoteTriggerTool | 远程触发 | ❌ 砍 |
| SyntheticOutputTool | 测试用 | ❌ 砍 |
| BriefTool | 简报 | ❌ 砍 |
| ConfigTool | 配置 | ❌ 砍 |
| NotebookEditTool | Jupyter notebook | ❌ 砍 |
| MCPTool / ListMcpResourcesTool / McpAuthTool / ReadMcpResourceTool | MCP 客户端工具（`dasclaw_mcp` 已覆盖底座） | ❌ 砍工具层 |
| AskUserQuestionTool / SendMessageTool | 用户交互（走 Tauri UI） | ❌ 砍 |

**小计**：14 类工具，**全砍**。

### 类型 V：codex 独有 — **不引入**

| 工具 | 说明 | 砍/留 |
|---|---|---|
| `view_image` / `image_detail` | 视觉模型（desktop image_* 已覆盖） | ❌ 砍 |
| `goal_tool` / `agent_jobs` / `multi_agents_v2` | codex 多 agent 协调（routines + sub_agent 替代） | ❌ 砍 |
| `code_mode` | codex 代码模式 | ❌ 砍 |
| `tool_search` / `tool_suggest` | 工具搜索/推荐（desktop `tool_info.rs` 覆盖） | ❌ 砍 |
| `request_user_input` / `request_permissions` | 用户交互（走 Tauri UI） | ❌ 砍 |
| `dynamic_tool` | 动态工具加载 | ⚠️ 选择性引入（未来需要时） |
| `mcp_resource_tool` | MCP 资源（dasclaw_mcp 覆盖底座） | ❌ 砍工具层 |

**小计**：8 类，**全砍**（dynamic_tool 待评估）。

---

## 4. F4.6 真正要搬的工具清单

按"无头 agent 框架最小必要集"汇总（类型 I + 类型 II 核心）：

| # | 工具文件 | desktop 当前位置 | 推荐目标 crate | 行数估算 |
|---|---|---|---|---|
| 1 | `file.rs` | builtin/ | `dasclaw_builtin_fs`（新） | 956 |
| 2 | `code_edit.rs` | builtin/ | `dasclaw_builtin_fs` | 438 |
| 3 | `file_guard.rs` | builtin/ | `dasclaw_builtin_fs` | 184 |
| 4 | `path_utils.rs` | builtin/ | `dasclaw_builtin_fs` | 737 |
| 5 | `glob_search.rs` | builtin/ | `dasclaw_builtin_search`（新或并入 fs） | 331 |
| 6 | `grep_search.rs` | builtin/ | `dasclaw_builtin_search` | 405 |
| 7 | `shell.rs` | builtin/ | **填充 `dasclaw_shell_command` 子模块** | 1666 |
| 8 | `json.rs` | builtin/ | `dasclaw_builtin_data`（新或合并） | 288 |
| 9 | `time.rs` | builtin/ | `dasclaw_builtin_data` | 594 |
| 10 | `memory.rs` | builtin/ | `dasclaw_builtin_agent`（新） | 974 |
| 11 | `plan_mode.rs` | builtin/ | `dasclaw_builtin_agent` | 276 |
| 12 | `tool_info.rs` | builtin/ | `dasclaw_builtin_agent` | 297 |
| 13 | `echo.rs` | builtin/ | `dasclaw_builtin_agent` | 48 |
| 14 | `git/` 9 文件 | builtin/git/ | **填充 `dasclaw_git_tools`**（已是空壳 24 行） | 1326 |
| 15 | `http.rs`（可选） | builtin/ | `dasclaw_builtin_net`（新） | 1513 |
| 16 | `web_fetch.rs`（可选） | builtin/ | `dasclaw_builtin_net` | 366 |
| 17 | `web_search.rs`（可选） | builtin/ | `dasclaw_builtin_net` | 552 |
| 18 | `html_converter.rs`（可选） | builtin/ | `dasclaw_builtin_net` | 128 |
| 19 | `sub_agent.rs`（可选） | builtin/ | `dasclaw_builtin_agent` | 400 |
| 20 | `session_fork.rs`（可选） | builtin/ | `dasclaw_builtin_agent` | 173 |
| 21 | `routine.rs`（可选） | builtin/ | **填充 `dasclaw_routines`** | 2639 |
| 22 | `lsp/` 10 文件（可选） | builtin/lsp/ | **填充 `dasclaw_lsp`** | ~1700 |

**汇总**：
- **核心必搬**：14 类（含 git/），约 **8520 行**
- **可选搬**：8 类，约 **7471 行**
- **总计 F4.6 工作量**：~16000 行（按"全部下沉"上限估算）

---

## 5. 留在 desktop-client 不下沉的清单

| # | 工具文件 | 行数 | 不下沉理由 |
|---|---|---|---|
| 1 | `job.rs` | 2359 | 绑 orchestrator，ADR-155 决定 orchestrator 留 desktop |
| 2 | `message.rs` | 1075 | 绑 Tauri channels |
| 3 | `restart.rs` | 482 | Tauri 进程概念 |
| 4 | `skill_tools.rs` | 1322 | desktop 专属 skill 系统 |
| 5 | `extension_tools.rs` | 828 | desktop 部署形态 |
| 6 | `secrets_tools.rs` | 218 | 底座已搬 `dasclaw_runtime::secrets`，工具壳是 desktop 设置 UI 用 |
| 7 | `image_analyze.rs` | 257 | 云 API 调用，desktop 场景 |
| 8 | `image_edit.rs` | 329 | 云 API 调用 |
| 9 | `image_gen.rs` | 247 | 云 API 调用 |

**小计**：9 类，**~7117 行留 desktop**。

---

## 6. 不引入的清单（节省工作量）

| 来源 | 不引入工具数 | 理由 |
|---|---|---|
| claude-code-main | 14 类（todo/team/worktree/repl/sleep/cron 等） | TS 实现 + 非框架核心 |
| codex-cli-main | 8 类（goal/multi_agents_v2/code_mode/tool_search 等） | ironclaw 路线已有等价 |
| ironclaw-main | 0 类（fork 上游 = 我们的当前实现） | 是我们的来源 |

**节省工作量**：避免移植约 **20+ 类工具**，节省 ~10000+ 行评估/移植成本。

---

## 7. 关键发现 & 决策待定

### 7.1 底座层 vs 工具壳层

- ✅ **底座完整**：apply_patch / shell_command / bash_perms / bash_valid / execpolicy / net_proxy / mcp / lsp 全部 7 类底座已在 crates/
- ❌ **工具壳缺失**：28 个 builtin 工具壳一份都在 desktop-client/，没有 crates/ 副本

### 7.2 三方 fork 路线对比

| 路线 | desktop 当前形态 |
|---|---|
| **ironclaw 平铺**（`builtin/*.rs`）| ✅ desktop 沿用 |
| **codex 双层**（spec + handlers 分离）| ❌ desktop 未采用 |
| **claude-code 子目录**（`{Tool}/`）| ❌ TypeScript 非移植目标 |

### 7.3 ADR-129 §1.3 verbatim port 红线在 F4.6 的歧义

- verbatim 要求："搬到 crates 时不能借机简化或重构"
- F4.6 实际要做："从单一 `src/tools/builtin/` 拆成多 crate（fs/shell/git/net/agent...）"
- **结构性重构 vs verbatim** 需要 ADR addendum 论证

### 7.4 待决策事项（移交给用户）

| # | 决策点 | 选项 |
|---|---|---|
| Q1 | 拆几个 crate？ | A) 单一 `dasclaw_builtin_tools` / B) 按职能 4-5 个（fs/shell/git/net/agent） / C) codex 双层（spec + handlers） |
| Q2 | 和已有底座如何并列？ | A) shell.rs 进 `dasclaw_shell_command` 子模块 / B) 新建 `dasclaw_builtin_shell` 并列 |
| Q3 | git/ 9 文件填 `dasclaw_git_tools` 空壳？ | 推荐 ✅ |
| Q4 | F4.6 vs F4.2.1+ 优先级？ | 待定 |
| Q5 | 类型 III 9 类全留 desktop？ | 推荐 ✅ |
| Q6 | 类型 IV / V 全不引入？ | 推荐 ✅ |

---

## 8. 数据采集方法（透明度）

```bash
# crates/ 来源
for crate in crates/dasclaw_*; do
  head -8 "$crate/src/lib.rs" | grep -E '^//'
done

# claude-code-main 工具数
ls claude-code-main/src/tools/ | wc -l  # → 43 子目录

# codex 双层
ls codex-cli-main/codex-rs/tools/src/*.rs | wc -l  # → 24 spec
ls codex-cli-main/codex-rs/core/src/tools/handlers/*.rs | wc -l  # → 20 handler

# ironclaw-main 工具
ls ironclaw-main/src/tools/builtin/*.rs | wc -l  # → 27

# desktop 当前
ls desktop-client/ironclaw/src/tools/builtin/*.rs | wc -l  # → 30 (+ git/9 + lsp/10)
```

三层验证补充：`mcp_code-review-g_cross_repo_search_tool` 因 embeddings=0 不可用，按 code-review-graph-usage 坑位 4 回退到 grep + ls，符合 AGENTS.md 三层规约。

---

## 9. 维护说明

- 本文档是 **F4.6 决策的事实底座**，每次新增/删除/改名工具时同步更新
- 如未来出现新的 fork 源，按"四方对照表"扩列
- F4.6 ADR 写完后，本文档对应小节加链接交叉引用
