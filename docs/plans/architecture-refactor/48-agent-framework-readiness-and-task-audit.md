# 48 — Agent 框架可用性与任务覆盖审查

> 日期：2026-04-30  
> 范围：吸收式重构、三库 tool / prompt 融合、GitHub Issues / Project 任务覆盖、红线验证完备性  
> 结论等级：**方向可行，当前未达到政企生产级框架可用；需先补齐安全强制链路与任务板缺口**

---

## 0. 结论

当前架构路线总体正确：以 desktop-client 的政企客户端能力为产品外壳，以 ironclaw 的工具、LLM、workspace、channel、job 能力为主体，以 codex 补 sandbox / apply-patch / execpolicy / project docs，以 claw-code 补 governance / bash validation / prompt / MCP，这个“吸收式重构”比直接升级某个 fork 更适合政企级客户端办公助手。

但当前实现还不能宣称最终 agent 框架已可交付。更准确的状态是：**核心能力已经局部落地，且红线意识开始进入 PR / Issue；但默认工具执行路径、prompt 合流、Project 任务覆盖、端到端验证仍未闭环**。

如果继续按现有方向做完所有必要任务，理论上可以产出符合目标架构的框架；但前提是补齐本文列出的 P0 / P1 缺口。若只完成当前 GitHub Project 里已有 W3 / W4 任务，不足以保证最终框架符合政企定位。

---

## 1. 判断标准

政企级客户端办公助手不是“能调用工具的 coding agent”，而是一个默认安全、可治理、可审计、可被管理员约束的本地助手框架。本文用以下标准判断：

1. 工具能力是否去重，并形成唯一注册、唯一策略、唯一审计路径。
2. Prompt 装配是否稳定、可解释、可测试，不因多库混合产生指令冲突。
3. 安全边界是否默认强制，而不是“支持但未接入”。
4. GitHub Issues / Project 是否能驱动从设计到验证的闭环。
5. 红线验证是否覆盖 DLP、sandbox、prompt、tool、audit、tenant isolation、CI gating。

---

## 2. 吸收式重构是否成立

### 2.1 成立的部分

四方能力矩阵已经把角色分清：desktop-client 保留 DLP、SafetyBridge、PolicySync、ManagedPolicy、DataReporter 等政企客户端资产；ironclaw 提供工具系统、LLM 编排、workspace、extensions、jobs；codex 提供 sandbox / apply-patch / execpolicy / project docs / app-server 可借鉴能力；claw-code 提供 governance 六件套、bash validation、MCP transport、prompt 工程。

这比“整体替换成 codex”或“整体升级 ironclaw-main”更稳，因为政企客户端的不可替代资产在 desktop-client，而不是上游 coding agent。

### 2.2 不成立或尚未证明的部分

当前文档和任务更像“迁移能力清单”，还不是完整的“融合后运行时 contract”。也就是说，很多 issue 写的是 port 某个模块，但没有全部写清：

- port 后哪个实现成为唯一真相源。
- 旧实现何时删除或降级为 wrapper。
- 同名工具、同类 prompt section、同类 hook / policy 在冲突时谁优先。
- 端到端路径是否证明工具调用、prompt 构造、安全审计真的穿过同一条链路。

建议后续每个吸收任务都补一个 `replacement contract` 字段：`replace / wrap / coexist / non-goal`。

---

## 3. Tool 功能重叠分析

### 3.1 当前主实现

当前 desktop-client/ironclaw 的工具入口已经收敛到 `ToolRegistry::bootstrap_tools()`，这是一条正确路线。它按 `BootstrapMode` 注册 builtin / dev tools，再按字段注册 memory、extension、skill、routine、image、vision、job、message 等能力。

关键证据：

- `bootstrap_tools()` 已在 [desktop-client/ironclaw/src/tools/registry.rs](../../../desktop-client/ironclaw/src/tools/registry.rs) 中实现。
- `init_tools()` 通过 `BootstrapContext { mode: Orchestrator { allow_local_tools } }` 调用它，见 [desktop-client/ironclaw/src/app.rs](../../../desktop-client/ironclaw/src/app.rs)。
- `PROTECTED_TOOL_NAMES` 已阻止动态工具 shadow `shell`、`apply_patch`、`memory_write` 等安全关键内置名。

这说明 tool 重叠问题已经开始被“单入口 + 受保护名称 + feature/domain 过滤”处理，不再是完全散落状态。

### 3.2 三库 tool 重叠矩阵

| 能力 | ironclaw / desktop 当前 | codex 来源 | claw-code 来源 | 建议归宿 |
|---|---|---|---|---|
| shell / bash | `ShellTool` + bash validator 集成 | exec-server / sandbox / portable-pty | bash_validation 六模块 | `ShellTool` 保留外壳；执行层走 `dasclaw_exec`；语义校验吸收 claw-code；pty 吸收 codex |
| read/write/list file | ironclaw builtin 已有 | codex handlers 有等价 | claw-code file_ops 有等价 | 保留 ironclaw 工具名与 UI 适配；补 codex sandbox 语义 |
| apply_patch | ironclaw 现有 `ApplyPatchTool`，但协议深度需确认 | codex lark 协议最强 | 无 | 以 `dasclaw_apply_patch` 为 parser 真相源，ironclaw tool 只做 wrapper |
| code_edit | ironclaw 已有 | codex apply-patch / edit flow | file_ops | 保留，但必须和 apply_patch 分清边界：小编辑 vs patch 协议 |
| grep/glob/search | ironclaw 已有 | codex 有等价 | 有基础能力 | 保留 ironclaw 工具名，避免重复注册 |
| web_fetch / web_search / http | ironclaw 已有 | codex web search / network policy | http client 简化 | 保留 ironclaw 工具；网络强制走 proxy / allowlist |
| MCP tools | ironclaw stdio / managed MCP | codex stdio/http 等 | claw-code 6 transport | 归入 `dasclaw_mcp`，不要三套 transport 并存 |
| job tools | ironclaw scheduler / container jobs | codex cloud tasks 不作为主线 | claw-code weak | 保留 ironclaw，Docker-less 客户端另做决策 |
| governance / green contract | 无完整 | guardian 部分互补 | claw-code 六件套最强 | 归入 `dasclaw_governance`，并接 hook / CI |
| plan / sub-agent / session fork | ironclaw 已有 | codex multi-agent v2 有补充 | claw-code 有 subagent/team cron | 先保留 ironclaw，后续按能力差距吸收 |

### 3.3 主要风险

#### R1. shell 安全能力“支持但未默认强制”

`register_dev_tools()` 当前注册的是 `ShellTool::new()`。`ShellTool` 的执行逻辑是：如果配置了 `sandbox`，走 `execute_sandboxed()`；否则走 `execute_direct()`。这意味着现状是“可以带 sandbox”，不是“企业模式默认带 sandbox”。

这正好对应 issue [#28](https://github.com/Linnanli/xClaw/issues/28)：OS sandbox + network proxy 已 port，但 app boot 还没有构造 `OsExecutor::new()` 或 `start_network_proxy()`，也没有把 proxy env 注入默认 `ShellTool`。

对政企场景，这是 P0 缺口。shell 是最高风险工具，必须从“可选增强”升级成 enterprise mode 的默认强制链路。

#### R2. ApplyPatchTool 归属还需最终明确

Project 里已有 W3 issue：[#52](https://github.com/Linnanli/xClaw/issues/52) port codex lark parser，[#53](https://github.com/Linnanli/xClaw/issues/53) 注册 ApplyPatchTool。这个拆分合理，但还缺一个明确 contract：

- `dasclaw_apply_patch` 是唯一 parser。
- ironclaw builtin `ApplyPatchTool` 只做 schema / approval / UI / audit wrapper。
- `code_edit` 与 `apply_patch` 的适用边界要写入 prompt 和 tool metadata。

否则最后容易出现两个编辑工具都能改文件，但失败语义、diff preview、审批语义不一致。

#### R3. MCP / WASM / extension tool 命名冲突需要 registry 层策略

`PROTECTED_TOOL_NAMES` 能阻止动态工具覆盖内置名，这是好事。但 MCP / WASM / extension 自带工具可能仍与彼此重名。当前保护重点是 built-in shadowing，不等于完整 namespace 设计。

建议后续给动态工具加 namespace 规则：

- builtin：裸名，如 `shell` / `read_file`。
- MCP：`mcp.<server>.<tool>`。
- WASM：`wasm.<package>.<tool>`。
- Extension：`ext.<slug>.<tool>`。

同时 `tool_info` 应能显示 canonical name、display name、source、approval mode、sandbox mode。

---

## 4. Prompt 工程合流可用性分析

### 4.1 当前 prompt 能不能用

可以用，但还不是最终形态。

当前 ironclaw 已经有 `LayeredPromptBuilder`，把 prompt 分为 static / dynamic 两层，并通过 `x_claw_agent::PROMPT_CACHE_BOUNDARY` 统一为 claw-code 的 `__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__`。`Reasoning::build_system_prompt_with_tools()` 已经把工具定义、workspace identity、skill context、channel、extension guidance、conversation context、runtime 等拼进 prompt。

这说明 prompt 不是拼不起来，也不是完全混乱。它现在具备：

- 静态 / 动态分层。
- Claude 模型才注入 cache boundary。
- 非 Claude 模型不注入 boundary。
- skill context 有“不得覆盖 core instructions / safety / tool approval”的免责声明。
- admin policy 有组织策略优先级声明。

### 4.2 仍未完成的合流点

当前有两套同概念 builder：

- ironclaw：`LayeredPromptBuilder`，强在缓存、hash、模块化、cache boundary 开关。
- claw-code：`SystemPromptBuilder`，强在 ProjectContext、git status / diff、instruction files 截断、output style、Claude 官方 section 结构。

Issue [#37](https://github.com/Linnanli/xClaw/issues/37) 和 [#38](https://github.com/Linnanli/xClaw/issues/38) 已经完整记录这个问题，但这两个 issue 内容几乎重复，需要合并关闭一个。当前 `crates/x_claw_agent::prompt` 也明确写了：目前只统一 boundary 常量，builder 实现合并由 #38 追踪。

### 4.3 Prompt 混搭后的主要风险

#### P1. Project docs 尚未进入最终 prompt 链路

W3 Project 里已有 #54-#59：ProjectDocLoader、优先级、三层合并、向上递归、max_bytes、OnSessionStart 注入 ProjectDocs。这个任务划分是对的。

但在这些完成前，prompt 仍缺少 codex / claw-code 的多层项目指令能力。政企办公场景常依赖组织规范、项目规范、当前目录规范，缺这个会降低可控性。

#### P2. output style 和客户端 UI 约束可能冲突

ironclaw static prompt 当前要求非原生 thinking 模型输出 `<think>` / `<final>`，还要求每次响应带 `<suggestions>`。这可能和桌面端 assistant-ui 的渲染、政企办公助手的简洁回复、以及未来 Claude output style 冲突。

如果吸收 claw-code 的 `with_output_style()`，必须明确优先级：

1. system / safety / admin policy。
2. client output contract。
3. output style。
4. skill / extension guidance。
5. user preference。

否则“output style 让模型用某格式回复”和“客户端要求 `<final>` / `<suggestions>`”会互相踩。

#### P3. Tool 描述进入 static layer，动态开关进入 tool_definitions_filtered，需验证一致

`StaticLayer` 把工具列表放进 prompt；agent loop 同时会给 LLM function-calling tool definitions。后续如果按 managed policy 动态禁用某些工具，必须保证：

- prompt 里的工具列表不包含被禁用工具。
- function-calling definitions 也不包含被禁用工具。
- tool executor 再做一次强校验，不能只靠 prompt 隐藏。

这一点在历史经验里已经出现过：只过滤 initial tool defs 不够，每轮刷新和 preflight 前也要过滤。

### 4.4 Prompt 合流建议

不要直接把 `SystemPromptBuilder` 和 `LayeredPromptBuilder` 粗暴合并。建议新增 `dasclaw_prompt` 或扩展 `x_claw_agent::prompt` 为三层 contract：

1. `PromptStaticInput`：identity、core rules、tool specs、safety contract、client output contract。
2. `PromptDynamicInput`：ProjectDocs、skills、extensions、conversation、runtime、admin policy、token budget、language。
3. `PromptRenderPolicy`：model family、cache boundary、output style、client renderer capabilities。

验收不应只看字符串 contains，而要有 snapshot / invariant tests：

- boundary 只出现一次。
- ProjectDocs 在 boundary 后。
- safety / admin policy 不可被 skill / output style 覆盖。
- disabled tool 不出现在 prompt 和 function definitions。
- 中文用户场景下不被英文默认 prompt 拉回英文。

---

## 5. GitHub Issues / Project 任务划分审查

### 5.1 当前 Project 状态

GitHub Project：`Architecture Refactor (W1-W9)`，当前项目板显示 31 个条目。

结构优点：

- 已按 `wave:W3` / `wave:W4`、`area:*`、`effort:*`、`risk:*` 打标签。
- W3 / W4 大部分 issue 有目标、验收、Depends on、Estimate。
- 有 `adr-redline` 标签，说明红线任务没有完全混在普通任务里。
- P0-3 PR #51 已把 ADR-113 §2.2 / §2.3 做成编译期、启动期、测试期三层红线。

明显问题：

- Project 名称是 W1-W9，但当前板上主要是 W3 / W4，W5-W9 没有完整 project items。
- PR #51 / #78 / #79 已 merged，但 Project 状态仍显示 In Review，项目板状态滞后。
- PR #81 已 closed，但 Project 状态为空，容易误导后续排期。
- Issue [#37](https://github.com/Linnanli/xClaw/issues/37) 与 [#38](https://github.com/Linnanli/xClaw/issues/38) 内容重复。
- Issue [#27](https://github.com/Linnanli/xClaw/issues/27) 与 [#28](https://github.com/Linnanli/xClaw/issues/28) 标题重复，其中 #28 内容更完整。
- #27 / #28 / #37 / #38 / #34 等关键风险 issue 不在当前 Project 板上，导致“项目板完成”不等于“架构风险完成”。

### 5.2 当前任务是否合理

W3 / W4 的拆法整体合理：

- W3 ProjectDocs 按 trait、优先级、三层合并、递归查找、截断、OnSessionStart 注入、parity test 拆分，粒度清楚。
- W3 apply-patch 按 parser、tool 注册、fixture port 拆分，依赖关系合理。
- W3 hooks 有 lifecycle trace、SafetyBridge 通过 hooks 接入、P0-3 红线 PR，方向正确。
- W4 governance 按 scaffolding、policy_engine、recovery_recipes、trust_resolver、branch_lock、stale_base、green_contract、lane_events、bash_validation、features flag、测试 port 拆分，基本可执行。

不合理处主要是“任务可以做，但完成定义还不够框架级”：

- 多数 issue 验收是模块级单测，缺少“最终唯一入口 / 替换旧路径 / 删除旧能力”的验收。
- 红线任务只覆盖 hooks / SafetyBridge / bash_validation 部分，未覆盖 sandbox / prompt / dynamic tools / audit。
- Project 缺 dependency / blocking 字段，Depends on 写在 body 里，机器不可读。
- 缺“enterprise mode 默认配置”的任务，容易把能力做成 opt-in。

### 5.3 红线验证覆盖情况

| 红线 | 当前覆盖 | 判断 |
|---|---|---|
| Hook 只能一个编排入口 | PR #51 已做编译期 + 启动期 + 测试期 | 覆盖较好 |
| Safety 不得变成声明式 event-hook 规则 | PR #51 已做启动期 panic | 覆盖较好 |
| SafetyBridge 统一通过 hook seam | #61 已列为 `adr-redline` | 有任务，未完成 |
| bash_validation 接入 BeforeToolCall | #73 已列为 `adr-redline` | 有任务，未完成 |
| shell 默认 sandbox / proxy / resource limits | #28 有任务，但无 `adr-redline` / `phase:P0` | 覆盖不足 |
| Prompt builder 合流不破坏 boundary / policy 优先级 | #37/#38 有 P2 任务 | 覆盖不足，优先级偏低 |
| ProjectDocs 注入位置与截断 | #54-#59 / #63 有任务 | 覆盖中等 |
| disabled tool 不进入 prompt / tool defs / executor | 未见明确 issue | 缺漏 |
| dynamic tool namespace 与 shadowing | 未见明确 issue | 缺漏 |
| audit log 对 tool / prompt / policy decision 100% 覆盖 | 未见明确 issue | 缺漏 |
| tenant / backend user / scope isolation 回归 | 未见架构板 issue | 缺漏 |
| CI 全绿门禁 | #80 仍 open，项目状态有 stale PR | 覆盖不足 |

### 5.4 能否做完现有 Project 就产出目标框架

不能。做完当前 Project 板上的 31 个条目，最多能把 W3 / W4 的 Hooks、ApplyPatch、ProjectDocs、Governance、BashValidation 推进一大步，但仍缺：

- W3.2b sandbox / network proxy app boot 激活。
- Prompt builder 最终合流与 output contract 验证。
- W5 MCP / execpolicy / feature flags / observability 等任务板。
- enterprise mode 默认安全配置。
- full-path E2E：用户消息 → DLP → prompt → tool defs → tool call → sandbox/proxy → audit → UI。
- Project 状态自动同步与 CI gating 清理。

因此，“当前 Project 完成”不能等同于“符合目标架构的框架完成”。需要把关键 open issue 和缺失 issue 纳入 Project，并新增 red-line milestone。

---

## 6. 建议新增 / 合并的 GitHub 任务

> 标签：`audit:first-review`  
> 说明：本节对应第一轮架构可用性与任务覆盖审查产出的新增 / 升级任务。当前 [#28](https://github.com/Linnanli/xClaw/issues/28)、[#37](https://github.com/Linnanli/xClaw/issues/37)、[#84](https://github.com/Linnanli/xClaw/issues/84)-[#91](https://github.com/Linnanli/xClaw/issues/91) 已统一补充 `audit:first-review` 标签，便于和 §9 的 `audit:second-review` 区分。

### 6.1 需要合并或关闭重复项

1. 合并 #37 / #38：保留标题更完整的 #37，关闭 #38 或反过来，保留一个 canonical issue。
2. 合并 #27 / #28：保留 #28，因为它有完整背景、grep 证据、方案和验收。
3. 清理 Project 状态：#51 / #78 / #79 已 merged，应移到 Done；#81 closed 应移出或标记 Superseded。

### 6.2 建议新增 P0 issue

#### P0-A: enterprise mode shell sandbox/proxy fail-closed

目标：在 enterprise mode 下，`ShellTool` 必须由 app boot 注入 `OsExecutor`、sandbox policy、proxy env、resource limits；sandbox 不可用时 shell 不注册或 fail closed。

验收：

- `ToolRegistry` enterprise bootstrap 下 `shell.sandboxed == true`。
- `start_network_proxy()` 在 app boot 非测试路径被调用并持有 handle。
- 非 allowlisted host 请求失败或被代理拒绝。
- proxy credential injection 有审计记录。
- macOS / Linux 有实测；Windows 明确 skip / unsupported 并禁用 shell。

#### P0-B: tool visibility triple gate

目标：被 policy 禁用的工具不能出现在 prompt、function definitions、executor preflight。

验收：

- 单测：禁用 `shell` 后 prompt 不含 shell。
- 单测：tool definitions 不含 shell。
- 单测：即使模型伪造 shell tool call，executor 拒绝。
- 集成测试：managed policy 改变后下一轮 LLM call 生效。

#### P0-C: prompt invariant and snapshot harness

目标：建立 prompt 合流的不可变契约。

验收：

- boundary 只出现一次。
- ProjectDocs 在 boundary 后。
- admin policy / safety / tool approval 优先于 skill / output style。
- output style 不破坏客户端渲染 contract。
- 中英文交互各有 snapshot。

#### P0-D: end-to-end enterprise tool audit

目标：证明真实工具调用全链路可审计。

验收：

- 用户消息触发 shell / read_file / apply_patch 至少三类工具。
- 每次工具调用有 decision、sandbox mode、policy mode、risk level、duration、exit code、redaction 状态。
- DataReporter 或等价审计 sink 收到结构化事件。
- 日志 / audit 不泄露原始 secret。

#### P0-E: project board hygiene automation

目标：Project 状态与 PR / Issue 状态一致。

验收：

- merged PR 自动 Done。
- closed unmerged PR 自动 Closed / Superseded。
- `adr-redline` issue 必须有验收测试字段。
- open P0 / adr-redline 没有 Project item 时 CI 或脚本报错。

### 6.3 建议新增 P1 issue

- Dynamic tool namespace：MCP / WASM / extension 工具统一命名和展示。
- Tool collision report：启动时输出工具来源、shadowing、禁用原因。
- Prompt builder ADR：决定放在 `x_claw_agent::prompt` 还是新 `dasclaw_prompt`。
- Job runtime decision：Docker-less 客户端是否禁用 Job，还是转云端 Job。
- Windows sandbox/resource limit explicit non-goal or implementation plan。

---

## 7. 最终框架可用性判断

### 7.1 当前可用范围

当前框架适合作为：

- 内部架构重构主线。
- 开发态 agent runtime。
- 关闭高风险工具后的政企客户端办公助手 MVP 底座。
- DLP / policy / audit / prompt / tool / hook 能力的集成试验平台。

### 7.2 当前不应宣称

当前不应宣称为：

- 已完成生产级 enterprise agent framework。
- 三平台 sandbox 等价。
- 所有工具调用都不可绕过治理。
- prompt 合流已完成。
- GitHub Project 做完即可完成 W1-W9 架构重构。

### 7.3 做完必要任务后的可达状态

如果补齐本文 P0 / P1 任务，并把 W5-W9 纳入 Project，最终可以产出符合目标架构的框架。可达状态应是：

1. `bootstrap_tools()` 是工具注册唯一入口。
2. tool visibility、tool execution、tool audit 三层一致。
3. shell / network / filesystem / resource limits 在 enterprise mode 默认 fail closed。
4. prompt builder 单一 contract，ProjectDocs、skills、admin policy、output style 有明确优先级。
5. hooks 只做生命周期编排，SafetyHook / ApprovalGate / SandboxExecutor / SecretProvider 是安全能力 seam。
6. governance 六件套接入 hook / CI / lane completion，而不是孤立 crate。
7. Project board 覆盖 W3-W9，且红线任务可机器检查。
8. 至少一条真实 E2E 证明：用户输入到工具执行到审计上报全链路可用。

---

## 8. 下一步建议

优先级最高的不是继续铺新 crate，而是做一个“P0 红线补齐 Sprint”：

1. 先合并/清理重复 issue，修正 Project stale 状态。
2. 把 #28 加入 Project，提升为 `phase:P0` + `adr-redline`。
3. 新增 P0-A 到 P0-E 五个 issue。
4. 把 #37/#38 合并为一个 canonical prompt issue，并提升 prompt invariant harness 为 P0。
5. W3 / W4 继续推进，但每个 port issue 都补 `replacement contract`。
6. 完成后跑 ADR-112 的最小 contract 子集，确认红线真的从文档进入 CI。

一句话总结：**当前架构方向值得继续，但任务板还不足以保证最后产出政企级框架。把 sandbox/proxy、prompt invariant、tool triple gate、audit E2E、Project hygiene 这五组红线补进来后，路线才真正闭环。**

---

## 9. 二次审查新增风险面

> 标签：`audit:second-review`  
> 说明：本节不是第一轮 tool 重叠 / prompt 合流审查的原始内容，而是 2026-04-30 第二次审查时新增。新增 GitHub issues 均已打 `audit:second-review` 标签，便于后续筛选。

### 9.1 使用的验证路径

本轮按 AGENTS.md 的三层验证要求补做了额外风险面审查：

1. 语义层：使用 `semantic_search` 查询 SafetyBridge、DLP、tool audit、tenant identity、MCP / WASM / extension governance、approval gate 等概念。
2. 图谱层：使用 code-review-graph `minimal_context` / `semantic_search_nodes` 检查 ironclaw 子图上下文；audit/policy/tenant 组合在函数节点级没有形成清晰集中入口。
3. 符号层：尝试 `vscode_listCodeUsages` 查询 Rust 符号 `scan_user_input`，当前 VS Code usages 工具对 Rust 无 reference provider，因此退回字面层。
4. 字面层：由于环境无 `rg`，改用 `grep_search` 检索 `SafetyBridge`、`DataReporter`、`TenantCtx`、`PolicySync`、`process_tool_result`、`ToolResult`、`McpToolWrapper`、`Capabilities`、`ApprovalGate` 等关键字。

### 9.2 新增风险矩阵

| 编号 | 二次审查新增风险 | 证据 | 任务 |
|---|---|---|---|
| S2-1 | 附件 / extracted text 未证明进入同一 DLP gate | [desktop-client/src/ipc/chat.rs](../../../desktop-client/src/ipc/chat.rs) 对 `content` 调 `scan_user_input`，但随后把 `IncomingAttachment { extracted_text, data, ... }` 注入 agent message；未见附件级 fail-safe DLP gate | [#92](https://github.com/Linnanli/xClaw/issues/92) |
| S2-2 | 工具原始输出在 sanitize 前进入 UI/channel preview 与 stash | [desktop-client/ironclaw/src/agent/dispatcher.rs](../../../desktop-client/ironclaw/src/agent/dispatcher.rs) 与 `thread_ops.rs` 在 `process_tool_result` 前发送 `StatusUpdate::ToolResult { preview: output.clone() }`，并写入 `tool_output_stash` | [#93](https://github.com/Linnanli/xClaw/issues/93) |
| S2-3 | 多个 tool execution surface 没有统一 parity contract | [crates/x_claw_agent/src/hooks.rs](../../../crates/x_claw_agent/src/hooks.rs) 明确 tool-level hooks / ApprovalGate 由各 `LoopDelegate` 负责；chat path 有 preflight，但 worker/job/routine/container 等路径需要矩阵证明 | [#94](https://github.com/Linnanli/xClaw/issues/94) |
| S2-4 | enterprise tenant / backend_user / managed policy fail-closed 边界未成任务 | [desktop-client/src/ipc/chat.rs](../../../desktop-client/src/ipc/chat.rs) 的 quota 在无 `ADMIN_API_URL` 时跳过；[desktop-client/src/engine.rs](../../../desktop-client/src/engine.rs) 同时维护 `scope_id` / `backend_user_id` 且有 fallback 路径；需定义 enterprise mode 与 dev mode 的硬边界 | [#95](https://github.com/Linnanli/xClaw/issues/95) |
| S2-5 | MCP / WASM / extension 的问题不止命名冲突，还需要动态能力治理 | [desktop-client/ironclaw/src/app.rs](../../../desktop-client/ironclaw/src/app.rs) 会加载 MCP/WASM/extension 并注册工具；WASM capabilities 是 opt-in，但 enterprise source allowlist、per-tool allow/deny、approval mode、capability audit 还没有红线任务 | [#96](https://github.com/Linnanli/xClaw/issues/96) |

### 9.3 新增任务说明

#### #92 — P0-F / Attachment DLP gate before agent injection

来源：二次审查新增 `S2-1`。附件是政企办公助手的高频入口，文档、图片 OCR、音频转写都可能含密钥、PII 或内部资料。如果只扫描 chat text，不扫描 `extracted_text` / attachment data，DLP 边界会被绕过。

依赖：无硬依赖；阻塞 [#85](https://github.com/Linnanli/xClaw/issues/85)，因为 E2E audit 不能在附件绕过 DLP 时宣称全链路安全。

验收重点：secret in attachment fail closed；PII in attachment format-preserving redaction；conversation report / logs / model context 不含原始敏感附件文本。

#### #93 — P0-G / Sanitize tool output before previews and stash

来源：二次审查新增 `S2-2`。当前 LLM tool result 会经 `process_tool_result()` sanitize，但 UI/channel preview 和 `tool_output_stash` 有机会先拿到 raw output。这是安全路径分叉，不是单纯 audit 增强。

依赖：无硬依赖；阻塞 [#85](https://github.com/Linnanli/xClaw/issues/85)；关联 [#84](https://github.com/Linnanli/xClaw/issues/84)。

验收重点：raw secret 不得出现在 `ToolResult.preview`、`tool_output_stash`、Tauri events、web/SSE events、channel relay events。

#### #94 — P0-H / Tool execution surface parity

来源：二次审查新增 `S2-3`。第一轮已经发现 tool triple gate；第二轮进一步确认还有“执行面数量多”的问题。chat、pending approval、deferred tools、worker jobs、containers、routines、scheduler、builder helpers 都可能执行工具，必须有统一矩阵证明没有绕过。

依赖：关联 [#84](https://github.com/Linnanli/xClaw/issues/84)、[#61](https://github.com/Linnanli/xClaw/issues/61)、[#73](https://github.com/Linnanli/xClaw/issues/73)；阻塞 [#85](https://github.com/Linnanli/xClaw/issues/85)。

验收重点：每个生产 tool execution surface 都列出并测试 policy visibility、hook preflight、approval、rate limit、sandbox/proxy、output sanitization、audit/status event。

#### #95 — P0-I / Enterprise tenant/policy identity fail-closed contract

来源：二次审查新增 `S2-4`。当前存在合理的本地开发 fallback，但政企生产模式必须明确何时 fail closed。尤其是 `backend_user_id` 缺失、signed policy 缺失/过期、quota 服务缺失、scope fallback 等场景。

依赖：关联 [#86](https://github.com/Linnanli/xClaw/issues/86) 和 [#85](https://github.com/Linnanli/xClaw/issues/85)。

验收重点：enterprise mode 下缺 backend identity / signed policy / required admin service 时必须 fail closed 或进入明确受限模式，并产生审计事件；tenant/scope isolation 有回归测试。

#### #96 — P0-J / Dynamic capability governance for MCP/WASM/extensions

来源：二次审查新增 `S2-5`。第一轮 [#87](https://github.com/Linnanli/xClaw/issues/87) 解决 namespace；第二轮确认 namespace 不足以覆盖治理问题。MCP/WASM/extension 是动态能力来源，需要 source allowlist、capability audit、per-tool allow/deny、approval mode 和 managed policy 统一生效。

依赖：依赖或协同 [#84](https://github.com/Linnanli/xClaw/issues/84)、[#87](https://github.com/Linnanli/xClaw/issues/87)、[#88](https://github.com/Linnanli/xClaw/issues/88)；动态工具纳入 E2E 时阻塞 [#85](https://github.com/Linnanli/xClaw/issues/85)。

验收重点：enterprise mode 下未批准 MCP server / WASM package / extension source 不得进入 prompt、tool definitions、UI catalog 或 executor；capability grant 与 approval mode 必须可审计。

### 9.4 当前判断更新

第一轮结论仍成立，但“红线补齐 Sprint”的范围应扩大：除了 sandbox/proxy、prompt invariant、tool triple gate、audit E2E、Project hygiene，还必须纳入 **附件 DLP、工具输出预览清洗、执行面 parity、tenant/policy fail-closed、动态能力治理**。

因此当前最小闭环顺序建议调整为：

1. 先做 [#28](https://github.com/Linnanli/xClaw/issues/28)、[#84](https://github.com/Linnanli/xClaw/issues/84)、[#92](https://github.com/Linnanli/xClaw/issues/92)、[#93](https://github.com/Linnanli/xClaw/issues/93)。
2. 再做 [#94](https://github.com/Linnanli/xClaw/issues/94) 把所有 execution surface 拉平。
3. 并行推进 [#37](https://github.com/Linnanli/xClaw/issues/37) 与 [#95](https://github.com/Linnanli/xClaw/issues/95)。
4. 最后用 [#85](https://github.com/Linnanli/xClaw/issues/85) 做真正的企业全链路 E2E audit。
