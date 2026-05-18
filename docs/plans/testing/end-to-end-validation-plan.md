# 端到端验证计划：安全能力 + Agent 能力

> **目标**：在 ADR-148 EgressGate 全链路收口之后，验证主流程"用户消息 → agent 推理 → 工具调用 → 结果返回"端到端可用。
>
> **核心原则**：**先无头 API 测全部能力，再加客户端自动化兜底**。所有场景都要先有一份"不依赖 UI、不依赖人手"的版本能在 CI 里反复跑；UI 自动化只作为最后一层冒烟。
>
> **前置**：#73 / #84 / #92 / #483 全部 merge；`dasclaw_*` crate 骨架就位；`bash_validation` 已接入 `BeforeToolCall` hook；`x_claw_agent` 已改名为 `dasclaw_core`（#619）。
>
> **不在本计划范围**：多租户、Windows 真机沙箱（#481）、性能压测（#380），单独排。

---

## 一、两阶段总览

| 阶段 | 形式 | 是否需要人手 | 是否能进 CI | 何时做 |
|---|---|---|---|---|
| **阶段一：无头 API 端到端** | Rust 集成测试驱动整条 agent loop + 工具链 + hook + EgressGate | 否 | 是（默认每次 PR 跑） | **优先做完** |
| **阶段二：客户端自动化** | tauri-driver / playwright 驱动真实 desktop-client UI | 否（启动一次后自动跑） | 是（独立 job，可选） | 阶段一全绿之后 |

> 阶段一不通过，禁止进入阶段二。

---

## 二、阶段一：无头 API 端到端

### 2.1 测试框架与组织

- 位置：新建 `desktop-client/ironclaw/tests/e2e_headless/` 目录
- 入口：每个场景一个 Rust 集成测试文件，如 `s1_dlp_input.rs`、`a5_multi_entry_contract.rs`
- 共用 fixture：
  - **MockLLM**：可脚本化回应（"收到 user message X → 返回 tool_call Y"），覆盖各种推理分支
  - **InMemoryStorage**：会话状态走内存，不依赖 sqlite 文件
  - **TempWorkspace**：每个测试一个临时目录，结束自动清理
  - **HookSpy**：记录所有 hook 调用序列，断言"BeforeToolCall 必定先于 ToolExec"
  - **EgressSpy**：记录所有 EgressGate 触发点 + 输入输出 diff
- 运行：`cargo nextest run -p dasclaw -E 'test(e2e_headless::)'`

### 2.2 安全能力场景（无头版，5 个）

每个场景的"操作"全部用 Rust API 触发，不开 UI。

#### S1 — 入口 DLP 脱敏

- **驱动**：通过 `Session::submit_user_message()` 直接投入含敏感信息的字符串
- **断言**：
  - MockLLM 收到的 prompt 中字符串已被 DLP 替换（手机号 / 身份证 / 邮箱 / API key 形态）
  - DLP 命中事件被结构化记录（不依赖 grep 日志）
- **失败即 fail-safe**：DLP 模块异常时必须拒绝消息，禁止"漏过原文"

#### S2 — bash 工具危险命令拒绝

- **驱动**：MockLLM 直接产出 `tool_call: bash(rm -rf ~/.ssh/)`
- **断言**：
  - `dasclaw_hooks::bash_validation_hook` 返回 `Block`
  - 真实执行器**没有**被调用（用 ExecutorSpy 验证调用次数 = 0）
  - 错误信息回灌到下一轮 LLM 上下文，且不暴露内部路径
- **变体**：覆盖每条高危规则各跑一遍，构成 parity 矩阵

#### S3 — 沙箱越权拒绝

- **驱动**：MockLLM 产出 `tool_call: write_file(.git/hooks/pre-commit, ...)`
- **断言**：
  - sandbox 层（macOS Seatbelt / Linux landlock）返回 `SandboxError`
  - 即使 bash_validation 放行也必须被沙箱拦
- **CI 限制**：Linux runner 走 landlock；macOS runner 走 seatbelt；Windows 跳过（#481 单独覆盖）

#### S4 — 出口 EgressGate 脱敏

- **驱动**：MockLLM 产出 `tool_call: bash(env)`，ExecutorStub 返回含 `AWS_SECRET_ACCESS_KEY=AKIA...` 的字符串
- **断言**：
  - 工具结果进入 `Session::record_tool_result` 前已脱敏
  - `EgressKind::UserDisplay` 路径生效
  - 同步覆盖 `UserDisplay` / `LLMContext` / `AuditLog` 三种 EgressKind 各一遍

#### S5 — 工具三重可见性闸门

- **驱动**：构造 `ToolPolicy { bash: disabled }`，然后：
  1. 拿到 system prompt，断言不含 bash 描述
  2. 拿到 tool definitions 数组，断言不含 bash 条目
  3. 直接调 `Executor::execute(ToolCall { name: "bash", ... })`，断言返回 `ToolNotAvailable`
- **断言**：三处必须同步禁用，任一处遗漏即 fail

### 2.3 Agent 能力场景（无头版，5 个）

#### A1 — 基础对话与上下文保持

- **驱动**：MockLLM 脚本化"第一轮记住 X，第二轮被问 X 时输出 X"
- **断言**：第二轮 prompt 含第一轮的 user/assistant 消息序列，且顺序正确

#### A2 — 工具调用：文件读写闭环

- **驱动**：MockLLM 产出 `write_file → read_file → final_text` 三步
- **断言**：
  - 临时目录里 hello.txt 内容正确
  - 每步 tool_result 都经过 EgressGate
  - 最终 assistant 文本含读出的内容

#### A3 — 多轮工具调用 + Compaction

- **驱动**：MockLLM 连续要求读 10 个文件后 token 数撞 compaction 阈值
- **断言**：
  - compaction 后 `tool_use` 与 `tool_result` 配对完整（不会孤儿 tool_use）
  - 后续追问仍能命中早期内容（验证 compaction 不丢关键信息）

#### A4 — Routines 触发链

- **驱动**：用 `RoutineEngine::trigger_now()` API 直接触发 routine（不走 cron 等待）
- **断言**：
  - routine 产生的 tool_call 也经过 `BeforeToolCall` hook
  - 输出经过 EgressGate
  - 与 chat 入口的 hook 调用序列**逐项一致**（用 HookSpy 做 diff）

#### A5 — 多入口契约对齐（#94 核心）

- **驱动**：构造同一个 `ToolCall { name: "bash", args: "rm -rf /tmp/xclaw-test" }`，从三处分别投入：
  1. `ChatEntry::dispatch()`
  2. `JobEntry::dispatch()`
  3. `RoutineEntry::dispatch()`
- **断言**：三处的 HookSpy 记录、EgressSpy 记录、AuditLog 记录**完全相同**（用 `assert_eq` 对结构体比较，允许时间戳归一化）
- **意义**：这是 #94 的契约测试；当前不通过即 #94 仍未完成

### 2.4 阶段一验收基线

| 类别 | 通过率门槛 |
|---|---|
| 安全 S1–S5（无头） | **5/5 全过**，任一失败视为安全闭环未达标 |
| Agent A1–A4（无头） | **4/4 全过** |
| Agent A5（无头，#94 契约） | 允许暂未通过，是 #94 的实施驱动 |
| 三个 crate 现有套件 | 100% 通过 |

### 2.5 CI 接入

- 新增 GitHub Actions job：`E2E Headless`
- 触发条件：所有 PR + push to xClaw
- 命令：
  ```
  cargo nextest run -p dasclaw_hooks -p dasclaw_sandbox -p dasclaw_bash_validation -p dasclaw_core -p dasclaw -E 'test(e2e_headless::)'
  ```
- 失败即 block merge

---

## 三、阶段二：客户端自动化（阶段一全绿后才启动）

### 3.1 框架选型

- **首选**：`tauri-driver` + WebDriver（Tauri 官方推荐）
- **备选**：`playwright` + Tauri webview（生态成熟，但需要桥接）
- 选型在阶段一完成后单独开一个 issue 决定

### 3.2 范围

- 把阶段一的 10 个场景**重写**为 UI 驱动版本：
  - 真实点击"新建会话"、键入消息、按发送
  - 验证 UI 上看到的脱敏 / 错误提示 / 工具结果展示
  - 验证 IPC 边界（前端 ↔ 后端 Tauri command）的序列化没问题
- 新增 UI 专属场景（无头测不了的）：
  - U1：设置面板禁用工具后，UI 立即反映（按钮置灰 / 提示已禁用）
  - U2：长会话滚动 + 虚拟化列表表现
  - U3：错误态显示（沙箱拒绝时 UI 文案 + 图标）

### 3.3 运行

- 独立 GitHub Actions job：`E2E UI`，默认手动触发 + 每晚 cron
- 不 block PR merge（避免 UI 偶发抖动卡 CI）
- 失败结果上传截图 + tauri 日志

---

## 四、与未完成 issue 的关系

| 测试场景 | 关联 issue | 关系 |
|---|---|---|
| S2 | #73 #490 | bash_validation 接入与 parity 增强 |
| S3 | #481 | Windows 真机沙箱（macOS / Linux 优先） |
| S4 | #483 | EgressGate 已 merge，是这条测试的根基 |
| S5 | #84 #485 | 三重可见性闸门 |
| A4 | #94 | Routines 入口契约 |
| A5 | #94 #485 | 多入口契约 — 是 #94 的核心驱动 |
| U1–U3 | 待开 issue | 阶段二启动时一起建 |

---

## 五、实施步骤

1. **本计划合入 xClaw 主线**（PR #618）。
2. 按"阶段一 2.1 框架"建 `e2e_headless/` 目录骨架 + MockLLM / Spy 工具。开 issue 跟踪。
3. 先实施 S1 + A1 两个最小闭环跑通框架。
4. 补齐 S2–S5 + A2–A4。
5. A5 与 #94 同步推进。
6. 阶段一全绿后启动阶段二选型 + 实施。

---

## 六、记录与反馈

- 测试中发现 bug → 直接开 issue，标 `area:safety` 或 `area:agent`
- 验收通过 → 在 #492 评论 close 对应缺口条目
- 阶段一覆盖率与缺口跟踪在 `docs/plans/testing/runlog-<date>.md`
