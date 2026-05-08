# Agent Rules

## 语言规则

**所有回答必须用中文。**

## 任务启动 4 问（每次新任务必答）

开始任何代码 / 文档 / 架构任务之前，先在脑里或对话中**显式回答**这 4 个问题，按答案决定是否触发分析工具（详细规则见下方"分析工具使用规范"section）：

1. **是否新增模块 / crate / 文件？** → 是 → 必须先用 `semantic_search` 找等价实现
2. **结论里是否含"X 没有 Y / X 缺 Y / X 是独家"等否定语？** → 是 → 必须三层验证（`semantic_search` → `vscode_listCodeUsages` → `rg`）
3. **是否做跨项目对账（codex / claw-code / ironclaw / desktop-client）？** → 是 → 必须三层验证
4. **是否写架构对账类文档（如 ADR / 14-md / 对比矩阵）？** → 是 → 必须三层验证

4 问全答"否" → 跳过分析工具直接开发；任何一项答"是" → 必须先按"分析工具使用规范"section 走 Level 1 → 2 → 3。

**触发了三层验证的任务**，commit message 必须含一行 `已检查 X 是否已有，结论：…` 作为过程透明度记录。

**反模式**：跳过 4 问、靠"感觉"判断是否需要分析、字面量搜索代替语义搜索、否定性结论无双证据就落笔。

## 沟通规则

1. **不要假设用户清楚自己想要什么。** 当动机或目标不清晰时，停下来讨论，而不是猜测着往前冲。做错了再改的成本远高于多问一句。
2. **目标清晰但路径不是最短的，直接说并建议更好的办法。** 用户可能因为惯性选择了次优方案，AI 有责任指出更短的路径——但最终决定权在用户。

### 反馈原则

1. **开发完成后调用 MCP 工具**：
   - 每次代码开发完成后，必须调用 `mcp-feedback-enhanced` 工具。
   - 每次给开发者解释方案或者让开发选择方案，必须调用 `mcp-feedback-enhanced` 工具。
   - 工具调用后，Agent 必须等待用户反馈，收到明确指示后再继续执行后续任务。

### 会话轮换原则

**核心标准：不要用“1 个 PR”判断会话是否该结束；以“闭环 milestone”判断。**

#### 什么算一个闭环 milestone

满足以下 3 条中的至少 2 条，即视为完成一个阶段，可建议新开会话：
- 一个明确决策已定稿（如 ADR 从 draft 变 accepted，或关键方案被用户确认）
- 一段实现已落地并完成验证（build / test / lint / smoke 至少一类可执行验证通过）
- 产出已沉淀为可交接状态（PR、ADR、handoff、研究笔记、完成报告）

#### 何时必须提醒用户考虑新会话

- 同一会话内已经完成 1 个以上闭环 milestone，且准备进入下一个阶段
- 已完成 2 个以上非平凡 PR 或 stacked PR 切片，准备继续下一个子波次
- 已经写过 handoff、conversation summary，或已经明显依赖摘要态上下文推进工作
- 任务模式即将切换：例如从“实现/调试”切到“架构对账/研究/文档”，或反过来
- 出现 token budget、context compaction、summary 接管之类信号

#### 提醒动作（固定流程）

1. 先把当前状态写入 `/memories/session/<topic>-handoff.md`
2. 调用 `resolve_memory_file_uri` 拿到该文件的**完整 file:// URI**
3. 明确说明"当前已完成的闭环 milestone 是什么"
4. 给用户的下一会话开局 prompt 中**必须包含完整 URI**（不是 `/memories/session/...` 短路径），因为新会话的 agent 不一定能解析短路径

#### 注意

- **单个 PR 默认不是阶段边界**；PR 可能只是为了 reviewability 被拆小
- stacked PR 以“该组 PR 是否完成一个闭环 milestone”来判断，而不是以最前面那个 PR 是否已开来判断
- 如果只是一个很小的 typo / 文档修订 / lint 修复，不要机械地建议新会话
- 会话轮换属于**提醒机制，不是强制中断机制**：完成提醒动作后，若用户明确要求继续在当前会话推进，Agent 应继续执行任务。

### Skills 强制使用规范

每个开发回合都要主动核对是否触发 skill，不要靠"想起来"才用。以下四个 skill 是默认触发，**不是可选**：

| Skill | 触发时机 | 缺省后果 |
|-------|---------|---------|
| `code-quality-audit` | **写完一段非平凡实现后**（新函数/新模块/重构 > 50 LOC）、commit/push 前、用户说"审查/检查质量" | 会把补丁式代码、过长函数、unwrap/clone 滥用、重复造轮子的隐患合并进主线 |
| `code-simplifier` | **`code-quality-audit` 之后**，对刚改完的代码做收敛（消嵌套/去重复/改命名）。**verbatim port 场景必须跳过**（任何"简化"都违反 ADR-129 §1.3） | 留下啰嗦/低可读代码，后续重构成本飙升 |
| `adr-compliance-check` | **触及 `crates/dasclaw_*` / `.github/workflows/code_style.yml` / `scripts/check_codex_*_drift.py` / starlark pin / `.ironclaw` 字面量新增或豁免** 的 PR push 前 | 漏掉 verbatim 纯度违规、drift guard 接线缺失、CI roll-up `failure-check` stanza 缺失、PR 描述漏 ADR/Issue cite 等架构纪律问题 |
| `code-review-expert` | **PR 自审前**（push 之前）、PR 合并前、用户说"review/审查这次改动" | 漏掉 SOLID 违规、安全风险、依赖耦合等高阶问题 |

**执行顺序（默认管线）**：
1. 实现完成 → `cargo check -p <touched-crate> --tests` 0 错 0 警（完整 `cargo build` 由 CI 兑现）
2. `code-quality-audit` 自查（必须）
3. `code-simplifier` 收敛（必须；verbatim port 跳过）
4. `cargo fmt` + `python3.12 scripts/check_no_panics.py`
5. `adr-compliance-check` 自查（触及 ADR 红线场景必须）
6. commit 前 `code-review-expert` 自审（必须）
7. push + 开 PR

**不需要执行 skill 的场景**：
- 单字符/单行 typo 修复
- 仅文档/注释修改
- 仅 fmt/lint 自动修复
- 用户明确说"跳过 review，先跑起来"

**遗忘检测**：每次准备 push 前，先问自己"刚才有没有跑过 code-quality-audit、adr-compliance-check（适用时）和 code-review-expert？"，如果没有，回到对应步骤补做。

### GitHub PR 工作流

#### 默认原则

- **先完成局部闭环，再开 PR**：实现、验证、必要的自审都完成后再提交，不要把“未验证草稿”直接推成 PR
- **PR 要小而完整**：优先拆成可 review 的小 PR，但每个 PR 都必须有明确边界、验证结果和 merge 顺序
- **能 stacked 就 stacked，不要把无关改动塞进同一个 PR**

#### 标准流程

1. 完成当前切片实现
2. 运行最小充分验证：先窄测试，再必要的 build / workspace build
3. 按 Skills 管线完成 `code-quality-audit` → `code-simplifier`（verbatim port 跳过）→ `adr-compliance-check`（触及 ADR 红线时）→ `code-review-expert`（适用时）
4. commit，commit message 说明当前切片的真实边界
5. push 分支
6. 开 PR，并在 PR 描述中写清：
    - 这个 PR 做了什么
    - 明确没做什么
    - 验证命令和结果
    - 风险 / 后续步骤
7. 如果是 stacked PR，必须额外写清：
    - base 分支不是 `xClaw` 而是上一个 feature branch
    - merge 顺序
    - “请先 merge #X，再看本 PR”
8. 开完 PR 后，用 `gh pr checks` 或等价方式至少看一轮状态，并把结果同步给用户
9. 若当前闭环 milestone 已完成，按“会话轮换原则”判断是否该建议新会话

#### PR 描述最低要求

- 背景 / 目标
- 改动范围
- 非目标（What’s NOT in this PR）
- 验证- **Cross-cuts**：声明本 PR 与 ADR-114（`.ironclaw` → `.dasclaw` 命名迁移）的关系。三选一：`类A`（无新增 `.ironclaw` / `IRONCLAW_BASE_DIR` 字面量）/ `类B issue#XXX`（集中工程，需带 `adr-114-class-b` label 豁免 grep guard）/ `不涉及`- 后续 PR / 下一步

#### 禁止事项

- ❌ 还没跑基本验证就开 PR
- ❌ stacked PR 不写 base / merge 顺序
- ❌ PR 标题和 commit / 实际改动边界不一致
- ❌ 一个 PR 混入多个互不相干的主题
- ❌ 明明已经完成闭环 milestone，却继续在同一会话无限追加新阶段

#### PR 加速策略（手段 1/2/3）

**前提**：sequential merge 红线不变，并行只发生在"准备 + CI"阶段，不在"merge"阶段。任何手段都不得改变"一次只 merge 一个 PR 到 base"的事实。

- **手段 1（流水线重叠 / pipeline overlap）**：PR-N 等 CI 期间，本地切下一分支预先准备 PR-(N+1)
  - 触发条件：单线垂直推进、PR 之间文件强依赖、必须串行 merge
  - 操作顺序：commit PR-N → push → 切 PR-(N+1) 分支 → 本地完整跑流水线 → 等 PR-N CI 绿 → merge → `git checkout base && git pull --ff-only` → `git checkout PR-(N+1) && git rebase origin/<base>` → push → 开 PR
  - **教训**：开始 PR-(N+1) 本地改之前必须先 commit PR-N 全部待提交内容，否则切回 base 合 merge 时会被未提交改动拦下
  - 实测节省：~5 min / PR（W4 ADR-121 1.1.4c→1.1.4d 验证）

- **手段 2（多 PR 并行 in-flight）**：文件零重叠的批次同时开 PR，CI 并行
  - 触发条件：`cat-scan` 验证 ≥2 个候选文件**完全互不重叠**且互不依赖未移植符号
  - 必须项：每个 PR 描述显式声明"与 PR #X 无文件重叠"
  - merge 顺序：先到先 merge，后者必须 `git rebase origin/<base>` 后再 merge；不允许把并行 PR 互相设为对方分支的 base（那是 stacked，不是并行）
  - 适用：W4 sandbox 端口、独立叶子模块、互不依赖的文档/脚本
  - 理论上限：~40% 提速（受 GitHub Actions 并发额度与 reviewer 容量制约）

- **手段 3（git worktree 隔离）**：长流水线时避免本地 `git checkout` 切分支重编
  - 触发条件：单分支 cargo target 已占大量磁盘、切分支会触发 ≥3 min 增量重编
  - 操作：`git worktree add ../x-claw-B feature/B`，每个 worktree 独立 `target/`
  - 配合 sccache 使用，防止跨 worktree 重复编译

**红线**：
- ❌ 并行 PR 之间存在文件重叠 / 同一函数修改 → 必然 rebase 冲突，禁止
- ❌ 跳过 cat-scan 直接并发开 PR
- ❌ 把并行 PR 的 base 互相设为对方分支（那是 stacked，不是并行；stacked 必须显式声明且单线推进）
- ❌ 用并行 PR 绕过单 PR 的"小而完整"原则——并行的每个 PR 仍须各自满足 PR 描述最低要求

### ADR-114 红线（`.ironclaw` → `.dasclaw` 命名空间迁移）

**真理来源**：[`docs/plans/architecture-refactor/adr-114-dasclaw-rebrand.md`](docs/plans/architecture-refactor/adr-114-dasclaw-rebrand.md)

**硬性规则**（业务代码零容忍）：

1. **禁止在 PR diff 中新增 `.ironclaw` 或 `IRONCLAW_BASE_DIR` 字面量**
   - CI 通过 [`scripts/check_no_new_ironclaw_literal.py`](scripts/check_no_new_ironclaw_literal.py) 自动拦截（workflow `code_style.yml` 的 `no-new-ironclaw-literal` job）
   - 删除 `.ironclaw` 行不算违规；只算**新增**行
   - 已存在的 `.ironclaw` 字面量（迁移路径 / 历史注释 / fixture）不会触发
2. **白名单文件**（meta 自指，可含字面量）：
   - `docs/plans/architecture-refactor/adr-114-dasclaw-rebrand.md`
   - `scripts/check_no_new_ironclaw_literal.py`
   - `.github/workflows/code_style.yml`
   - `.github/pull_request_template.md`
3. **类 B 集中工程豁免**：
   - 必须带 `adr-114-class-b` label
   - 必须有专属 issue（如 ADR-114 §6 OQ-3 → issue #218）
   - PR 标题 + body 显式声明 `类B issue#XXX`
   - grep guard job 在该 label 下整体跳过；不允许零碎打补丁式豁免
4. **PR 描述必须含 `Cross-cuts:` 三元声明**（见上节 PR 描述最低要求）

**自查命令**（push 前）：

```bash
# 1. 字面量自查（不需 base 比较）
grep -rn '\.ironclaw\b\|IRONCLAW_BASE_DIR' --include='*.rs' --include='*.toml' --include='*.sql' --include='*.md' \
  desktop-client/ admin-backend/ crates/ scripts/ docs/ 2>&1 | grep -v 'adr-114\|ironclaw-main/\|claw-code/\|codex-cli-main/'

# 2. 用 grep guard 自身做 PR diff 扫描（推荐）
python3 scripts/check_no_new_ironclaw_literal.py --base origin/xClaw
```

**违规处理**：CI 红 → 要么改写为 `.dasclaw` / `DASCLAW_BASE_DIR`，要么补 `adr-114-class-b` label + 专属 issue。**禁止以注释或 `#[allow(...)]` 方式绕过 grep guard**。

### Base 分支既有 broken 测试处置规范（2026-04-30 PR #79/#81 实证）

**触发场景**：CI 改造（如完整 build 下沉 PR CI）后，第一波 PR 突然在与本身改动无关的测试上 fail。

**根因模式**：
- 改 CI 之前 Tests job 是 `if: false`，PR 阶段不跑测试
- 多个 broken 测试在历史上合进了 base（xClaw），从未被 PR CI 拦截
- CI 改造后第一次让 broken 浮现，每个新 PR 都会"莫名其妙"红
- 多个 broken 之间还会**互锁**：PR-A 修测试-X 但被测试-Y broken 挡住，PR-B 修测试-Y 但被测试-X broken 挡住，单独发都过不了

**处置流程（按优先级）**：

1. **先确认是不是 base 既有问题**：在 `origin/<base>` HEAD 上跑同一测试。如果同样 fail → 是历史欠债，不是本 PR 引入。
2. **本地全套验证不能只跑 touched crate**：base broken 的测试可能在 untouched crate 里。`cargo check -p <crate>` 不会发现，必须跑相关测试 binary：`cargo nextest run -p ironclaw --test <broken_test>`。
3. **互锁的 broken 优先合并到单 PR**，不要 stacked：
   - stacked PR 解决不了 base broken 测试问题
   - rebase + 合并 commit 到一个 PR，PR 描述里写清 `Closes #X + Closes #Y`
   - 关闭被合并那个 PR 时留 comment 说明合并去向
4. **修复方式按 broken 类型选**：
   - **测试断言固化错误设计**（如 hard-coded expected list 没跟实现同步）→ 更新断言到当前真实状态
   - **代码真有 bug**（如 plan_mode `parameters: {type:object}` 缺 `properties`）→ 修代码
   - **测试与 fork 策略冲突**（如 telegram_auth_integration `CI=true` 触发 panic 但 xClaw 不激活该 channel）→ 改 gate 条件，fork 默认跳过 + 上游 opt-in
   - **跨模块互锁**：必须一次性修齐
5. **Regression test enforcement workflow 注意点**：
   - 该 workflow 只监听 `pull_request` 默认 events，**不监听 `labeled`**，加 label 不会自动 rerun
   - 临时绕过：推空 commit 带 `[skip-regression-check]` marker 触发 rerun（commit 消息扫描，比 label 鲁棒）
   - 根因修复方式：让脚本的 `^tests/` 正则识别嵌套 crate 路径（如 `desktop-client/ironclaw/tests/...`）
6. **本地分层策略边界**（AGENTS.md 本地节奏）：
   - "本地不要跑 workspace test" 仍然有效，但**前提是 CI 必须在 PR 阶段跑全集**
   - 一旦 CI 改造让 PR 第一次跑全集，**24h 内修齐 base broken**，不要让它过夜污染所有 PR
   - 改 CI 的 PR 自身要预跑 workspace test 一遍，提前发现 broken

**反模式（禁止）**：
- ❌ 看到无关测试 fail 就直接 `--no-verify` 推或者关 PR 重开
- ❌ 把 base broken 当成本 PR 的责任去研究
- ❌ 把互锁的两个 broken 修拆成两个 stacked PR（解不了，base 仍 broken）
- ❌ 修测试断言时不验证"是断言写错了还是代码真坏了"——盲目改断言会固化 bug
- ❌ Tests CI 红就建议合并（违反 AGENTS.md "PR 验证未通过不开 PR"）

**实证案例**（PR #79 + #81 + 后续修补）：
- xClaw 同时 broken 三处：`shell_risk_regression`（4 处分级漂移）/`telegram_auth_integration`（CI gate 错位）/`tool_schema_validation`（hard-coded list 过期 + plan_mode schema bug）
- 三者互锁，单独 PR 全过不了 Tests
- 处置：rebase 合并到 #79 单 PR，一次修齐 4 类问题，`Closes #76 + Closes #80`
- 教训：CI 改造 PR (#50) 自身没跑 workspace test，broken 集中在合并后第一波 feature PR 暴露

### 分析工具使用规范（Round 17 建立 / Round 18 实证）

**重大架构决策/能力盘点/跨项目对比前，必须按三级顺序使用工具**：

| 级别 | 工具 | 使用场景 |
|------|------|---------|
| Level 1 语义层 | `semantic_search` | 概念搜索（跨命名等价实现），**必须先用** |
| Level 2 符号层 | `vscode_listCodeUsages` | LSP 引用/定义/实现图，核验判断 |
| Level 3 字面量层 | `rg` / `grep` | 已知确切词后再用 |

**反模式（禁止）**：
- ❌ 跳过 Level 1，直接 `rg <英文词>` 找概念 — 漏掉异名等价实现
- ❌ 判定"独家/缺失"时只看单一 repo — 必须三方交叉验证
- ❌ 未读解构文档（如 `decode-claude-code-main/`）就给架构结论
- ❌ 写架构对账文档时没有先按本规范核验能力表每一格 — 会把主观猜测当结论

**执行流程（最低标准）**：
1. 先用 `semantic_search` 搜概念（≥ 2 种语义表达）
2. 若有符号级疑问，用 `vscode_listCodeUsages` 打引用图
3. 最后才用 `rg` 定位确切位置
4. 对于"X 没有 Y"这类否定性结论，**必须明确给出 Level 1 + Level 3 双证据**才能落笔

**触发判断表**（什么场景必须做三层验证）：

| 场景 | 是否必须做三层验证 |
|------|---------------------|
| 否定性结论："X 没有 Y" / "X 是独家的" / "缺失 Y" | ✅ 必须 |
| 新增模块 / 新建 crate / 新建文件 | ✅ 必须（先 semantic_search 是否已有等价） |
| 跨项目对账（codex / claw-code / ironclaw 能力对比） | ✅ 必须 |
| 架构对账文档（如 14-md） | ✅ 必须（Round 17/18 实证错误率 4/19） |
| 已知模块内 bug 修复 | ❌ 不需要 |
| 仅文档 / 格式化 / lint 修改 | ❌ 不需要 |
| 已确定模块内 feature 扩展（不涉及"是否复用"） | ⚠️ 可选 |
| commit message 写"已检查 X 是否已有，结论：…" | ✅ 推荐（过程透明度） |

**教训来源**：
- Round 1-15 多次误判（"codex 没 forkSubagent"、"ironclaw Prompt Cache 独家"）的根因均为字面量搜索陷阱。
- **Round 18 实证**：14 文档 Round 17 版本列出的 4 项 P0/P1 "缺口"（`<system-reminder>` 标签 / CYBER_RISK_INSTRUCTION 文本 / 多层 CLAUDE.md 加载 / 压缩阈值），经 `semantic_search` 验证全部是**伪缺口** —— claw-code `runtime/src/prompt.rs:480` 与 `prompt.rs:197`、codex `openai_models.rs:306` 早已实现。4/19 的文档错误率直接证明：**不做 semantic_search 就动笔写对账文档是不合格的**。

---


## 代码图谱与打包工具（2026-04-29 启用）

### Repomix 核心模块打包

预先生成的 LLM 友好打包（位于 `docs/repomix-out/`，已 gitignore）：

| 文件 | 模块 | 文件数 | tokens |
|---|---|---|---|
| `codex-rs-core.md` | codex-cli-main 内核 (core/tools/protocol/mcp-server/exec/sandboxing/...) | 602 | 947K |
| `claw-code-core.md` | claw-code (rust/+src/) | 70 | 274K |
| `ironclaw-main-core.md` | ironclaw-main (src/+crates/) | 522 | 1.78M |
| `desktop-ironclaw-core.md` | desktop-client/ironclaw (src/+crates/) | 385 | 1.11M |

重新生成命令模板（注意根 `.gitignore` 含 `/ironclaw-main/`，需 `--no-gitignore`）：
```bash
repomix --compress --style markdown --no-gitignore \
  --include "src/**/*.rs,crates/**/*.rs" --ignore "**/target/**" \
  -o /Users/nallylin/Documents/code/x-claw/docs/repomix-out/<name>.md
```

MCP 接入：`.vscode/mcp.json` 已添加 `repomix` server (`npx -y repomix --mcp`)。

**何时使用 Repomix**（适合「全景概览 / 一次性灌入 LLM」）：
- 接手陌生模块前先读一遍打包文件，比逐个 `read_file` 快
- 跨 crate 重构前先核对模块边界与公共 API
- 让外部 LLM（无 IDE 工具链）也能理解代码：把 `.md` 直接拷给它
- 写 ADR / 设计文档前盘点既有实现
- ⚠️ **不适合**：单个符号查询、增量改动追踪、行级精确定位 → 改用 `grep_search` 或 code-review-graph

### code-review-graph 增量图谱

CLI: `/Users/nallylin/.local/bin/code-review-graph`（多 repo registry 已注册 4 个）：

| alias | path | nodes | edges | files |
|---|---|---|---|---|
| `ironclaw` | `desktop-client/ironclaw` | 20519 | 196690 | 1116 |
| `codex-cli` | `codex-cli-main` | 34382 | 336693 | 2273 |
| `claw-code` | `claw-code` | 4522 | 37559 | 156 |
| `ironclaw-main` | `ironclaw-main` | 23674 | 220750 | 846 |

常用命令：
- `code-review-graph repos` — 列出已注册 repo
- `code-review-graph build --repo <path>` — 全量重建
- `code-review-graph update --repo <path>` — 增量更新
- `code-review-graph watch --repo <path>` — 自动增量（**仅 desktop-client/ironclaw 启用**）
- `code-review-graph detect-changes --base HEAD~N [--brief]` — impact radius 报告
- `code-review-graph status` — 图谱统计

MCP 接入：`.vscode/mcp.json` 中 `code-review-graph` server 已指向 `desktop-client/ironclaw`，提供 `mcp_code-review-g_*` 工具集（impact_radius / affected_flows / review_context / semantic_search_nodes / community / minimal_context 等）。

注意事项：
- `register` 要求路径下有 `.git` 或 `.code-review-graph`；首次对子目录用 `build` 自动创建后再 register
- `watch` 是后台守护进程，改文件即触发增量；不要并发对同一 repo 跑 build/update（共享 SQLite）
- `detect-changes` 风险评分: ≥0.7 高风险，需重点 review；untested 列表标记缺测试

**何时使用 code-review-graph**（适合「精确定位 / 影响面分析」）：
- PR review 前评估改动影响半径：`detect-changes --base origin/xClaw` → 看 impact_radius 与 untested
- 重构前查调用方/被调用方：`mcp_code-review-g_impact_radius` / `affected_flows`
- 找语义相近的实现避免重复造轮子：`mcp_code-review-g_semantic_search_nodes`
- 给 LLM 提供「最小可读上下文」而非整文件：`mcp_code-review-g_minimal_context`
- 探索代码社群结构 / 模块耦合：`mcp_code-review-g_community`
- ⚠️ **不适合**：纯文档变更（图谱不索引）、模块全貌讲解 → 改用 Repomix 或 `read_file`

### 工具选型速查

| 任务 | 首选工具 |
|---|---|
| 「这个 crate 大概做什么 / 给我一份概览」 | Repomix `*-core.md` |
| 「改了 X，会波及哪些测试 / 调用方」 | code-review-graph `detect-changes` / `impact_radius` |
| 「找一个名字叫 XXX 的函数」 | `grep_search` / `file_search` |
| 「找一个『大概是这意思』的实现」 | code-review-graph `semantic_search_nodes` |
| 「贴给外部 LLM 让它出方案」 | Repomix 打包 |
| 「PR 评审清单 / 风险打分」 | code-review-graph `detect-changes --brief` |

## Rust 开发规范

### 本地快速开发节奏（完整 build 下沉 CI）

**原则**：本地只跑必要的几项以保障实现质量；实测耗时 10–16 分钟的 `cargo build -p ironclaw --tests` / `cargo check --workspace --all-targets` 不再是本地强制门。全量编译 + clippy + cargo-deny + 全类 workspace test 由 CI 兜底（见 `.github/workflows/test.yml` `Code Style` `Clippy (default)` `Tests` 等 jobs）。

**本地必做（按顺序）**：

```bash
# 1. 只 check 本轮改动的 crate 及下游直接依赖者（1–3 分钟）
cargo check -p <touched-crate> --tests
cargo check -p <one-direct-downstream> --tests   # 可选：如增删了 pub API

# 2. 跑本轮改动范围内的 nextest（1 分钟以内）
cargo nextest run -p <touched-crate>

# 3. fmt + check_no_panics
cargo fmt --all
python3.12 scripts/check_no_panics.py --base origin/<base-branch>

# 4. crate 级 clippy（不跑 workspace clippy，CI 兜底）
cargo clippy --no-deps -p <touched-crate> --all-targets -- -D warnings
```

**本地不要跑**：

- `cargo build` / `cargo check --workspace --all-targets`（超过 10 分钟，CI 会跑）
- `cargo clippy --workspace`（依赖 ironclaw 等大 crate 的预存 lint，本地跑也是为别人跑）
- `cargo nextest run --workspace`（包含 heavy integration，CI 事后补跑）

**冒烟 build（可选）**：开发者可随时在本地手动跑 `cargo build -p ironclaw --tests`，但不作为 PR 提交门。

**如果本地 cargo check 过了但 CI 红** → 大概率是 link-time / proc-macro / cfg-flag combos / desktop-client 集成问题，补跑 `cargo build -p <被 CI 报错的 crate>` 复现。

#### 时间优化最佳实践（实战总结）

**针对单轮多文件改动，按"build 一次/检查多次"原则压缩耗时**：

1. **测试合并跑，复用一次编译**：把多 crate 测试用 `-p A -p B` + `-E '组合 expr'` 合并到单次 `cargo nextest run` 调用：

   ```bash
   # 不要这样（重复 build 两次，10+ 分钟）：
   cargo nextest run -p ironclaw_safety
   cargo nextest run -p ironclaw -E 'test(req_p0g_w6)'

   # 这样跑（一次 build 复用，3-5 分钟）：
   cargo nextest run -p ironclaw_safety -p ironclaw \
     -E 'test(req_p0g_w6) | test(process_tool_result)'
   ```

2. **fmt + no_panics 用 `&&` 串**：两者都是秒级，串起来一次拿结果：

   ```bash
   cargo fmt --all && python3.12 scripts/check_no_panics.py --base origin/xClaw
   ```

3. **本地 clippy 只跑改动 crate，不要 `--workspace`**：rust toolchain 漂移会触发 untouched crate 的预存 lint 假阳（如 `collapsible-if` / `is_multiple_of`）。**真理来源是 CI clippy job**，本地只确认自己改的文件零警告即可：

   ```bash
   cargo clippy --no-deps -p <touched-crate> --all-targets -- -D warnings
   # 如果报错只在 untouched 文件 → 忽略，CI 决定
   ```

4. **e2e / heavy integration 优先 CI 跑**：本地只在与本轮改动直接相关时跑（如 P0-G/W6 改了 `tool_output_stash` 就要本地跑 `recorded_baseball_stats`）。其它 e2e 让 CI Tests job 兜底。

5. **PR 等 CI 期间利用空档**：CI 跑 Tests/Clippy 加起来 10-25 分钟，这段时间可以并行：开下一个 issue 工作分支、写 ADR、跑 skills 管线（`code-quality-audit` 等）、读相关历史 PR。**禁止 sleep 等 CI**。

6. **regression-test-check 失败 ≠ 真没测试**：CI 的 `git diff '*.rs'` glob 偶尔匹配不到 added `#[test]`。本地 `git diff origin/xClaw...HEAD -U0 -- '*.rs' | grep -E '^\+.*#\[test\]'` 能复现 MATCH 时，加空 commit `[skip-regression-check]` marker 跳过即可。

### 开发流程（TDD）

新增 Rust 函数/模块/结构体/API 端点时（不含最小 bug 修复）：
1. 编写测试用例（Red）
2. 实现功能代码（Green）
3. 重构优化（Refactor）
4. `cargo check -p <touched-crate> --tests` 本地验证（0 错 0 警）
5. 完整 `cargo build` / workspace clippy 由 CI 兑现，不是本地强制项

### 测试运行器：默认使用 nextest

**项目默认用 `cargo nextest run` 替代 `cargo test`**，进程级并行更快，无 fixture 串扰风险。

```bash
# 推荐
cargo nextest run -p ironclaw --lib sandbox::os_executor
cargo nextest run -p desktop-client --lib engine_startup_tests

# 仅当 nextest 未安装时回退
cargo test -p ironclaw
```

未装请：`cargo install cargo-nextest --locked`。CI 使用何种命令以仓库 workflow 为准（不强制改 CI）。

### 构建缓存清理：默认使用 cargo sweep --time 3

**磁盘满时优先用 `cargo sweep --time 3`，禁止用 `cargo clean` 或 `cargo sweep --time 0` 一次清干净**。一次性清掉全部产物会导致下次 build 需要从零重编全部依赖（10+ 分钟），sccache 也帮不上忙（增量缓存丢了）。

```bash
# 推荐：清掉 3 天没访问过的产物，保留近期增量缓存
cargo sweep --time 3

# 自动定时清理（cron）
0 3 * * * cd ~/Documents/code/x-claw && cargo sweep --time 3

# 禁止：一次清干净
# cargo clean              ← 不要用
# cargo sweep --time 0     ← 不要用
```

未装请：`cargo install cargo-sweep --locked`。

详细工作流（含 sccache、`cargo build -p desktop-client --lib` 增量编译）见 [desktop-client/README.md](desktop-client/README.md)。

### 外部库使用

- **先搜索项目中该库的现有用法**（`rg "libsql::" src/`），参考项目代码而非外部文档
- **优先使用成熟社区库**，避免重复造轮子（如 `config-rs`、`reqwest`、`serde`）
- 检查 `Cargo.toml` 中的依赖版本，注意 breaking changes
- 分阶段实现，先验证核心功能编译通过

### 编码标准参考

详细的 Rust 编码规范、错误处理模式、测试代码示例和质量门禁脚本用法见 `.kiro/steering/rust-coding-standards.md`（手动引用）。

### 数据库访问层迁移策略（Admin Backend）

Admin Backend 正在从 `tokio-postgres + deadpool-postgres` 逐步迁移到 **SQLx**。

**规则：**
- **新模块**必须使用 `sqlx`，通过 `state.sqlx_pool` 访问数据库
- **存量模块**维持 `tokio-postgres`，按需迁移，不强制一次性重写
- 两套连接池在 `AppState` 中并存：`db_pool`（legacy）和 `sqlx_pool`（新）

**SQLx 使用模式：**
```rust
// 1. 定义强类型结果结构体
#[derive(sqlx::FromRow, serde::Serialize)]
struct MyRow { field: String, count: i64 }

// 2. 用 query_as 绑定参数（$1 占位符）
let rows = sqlx::query_as::<_, MyRow>("SELECT field, COUNT(*) AS count FROM t WHERE id = $1")
    .bind(id)
    .fetch_all(&state.sqlx_pool)
    .await
    .map_err(|e| Error::Database(e.to_string()))?;
```

**DATE_TRUNC / INTERVAL 的处理：**
这两类 SQL 片段无法通过 `$1` 参数化（pg 协议限制）。必须用枚举白名单替代字符串，
参考 `admin-backend/src/handlers/reports.rs` 中的 `Period` 枚举模式。

---

## 测试规则

### 核心原则

> **测试的目标不是追求高覆盖率，而是确保系统在所有情况下都是安全的。**

1. **测试失败路径和成功路径同等重要** — 每个功能都要有失败路径测试
2. **安全功能必须"故障安全"（Fail-Safe）** — 失败时拒绝操作，而非允许操作（Fail-Open）
3. **真实环境测试不可或缺** — 模拟测试只能验证逻辑，无法验证集成
4. **契约测试验证业务目标可达成** — 不只是"能解析"，而是"够用"
5. **安全审计测试验证无泄露** — 日志、错误信息、网络响应中不得包含原始敏感数据
6. **`cargo test` 通过 ≠ `cargo build` 通过** — 测试可能绕过有问题的代码
7. **测试不应固化错误设计** — 测试断言必须从业务目标出发，而非从实现假设出发

### PICT 测试设计（新增）

当一个功能包含**多参数、多角色、多状态、多环境或多失败模式组合**时，补测试应优先使用 `pict-test-designer` 设计测试矩阵，而不是只凭直觉补 happy path。

规则：
- **先用 `pict-test-designer`，再决定是否需要真实 PICT 生成** — `pict-test-designer` 负责参数拆解、取值分组、约束整理、预期结果设计；真实 PICT 负责机械生成可复现的 pairwise 组合。
- **仅用 `pict-test-designer` 即可的场景** — 参数规模小到中等、组合仍可人工审查、当前目标是补测试设计或生成测试代码，而不是输出可审计的组合覆盖证明。
- **必须再跑真实 PICT 的场景** — 参数较多且组合明显爆炸、需要证明 2-way 覆盖完整、需要把组合结果沉淀为 CI/评审可复现资产、或属于审批/权限/配额/策略/安全等高风险组合逻辑。
- **agent 内的“设计 -> 真实生成 -> 映射 -> 落代码” workflow 统一使用 `pairwise-test-generator` skill** — 不要把这套多步流程直接展开复制到 `AGENTS.md`。
- **真实 PICT 生成统一走 `scripts/pict_generate.py`** — 不要在 agent workflow 里手动切换 `pict`、`pypict`、Docker。默认先走本机 `pict`；若本机不可用且允许容器，再用 `python3 scripts/pict_generate.py <model-file> --backend docker --docker-build-if-missing --format json`。
- **不要假设存在可直接拉取的公开 `pict` 镜像** — 当前 Docker 路径是通过上游 `microsoft/pict` 仓库里的 `Containerfile` 在本地构建镜像，不是依赖 Docker Hub 现成官方镜像。
- **PICT 只用于测试设计，不替代现有测试分层** — 生成出的组合必须映射到单元、失败路径、契约、安全审计、集成、E2E 测试中。
- **不要新建一套平行的“大而全测试”** — 优先扩充现有测试文件和测试族。
- **小而直接的校验函数不必强行 PICT 化** — 简单校验继续用聚焦型单元测试。
- **涉及真实边界的问题不得因使用 PICT 而降级** — SSE 事件格式、Tauri 状态注入、迁移完整性、策略验签等仍必须保留真实环境或跨层验证。
- **安全相关场景必须显式覆盖失败路径组合** — 尤其是 DLP、审批、权限、策略同步和敏感信息处理。

### PICT 模型文件放置

- **与单个方案/重构/测试设计强绑定的 `.pict` 模型** 放在对应的 `docs/plans/` 下，与同名矩阵或方案文档相邻，便于一起评审和迭代。
- **可复用、长期维护、跨模块共享的 `.pict` 模型** 不应长期堆在 `docs/plans/`；当模型从“某次方案资产”演变为“项目公共测试资产”时，应迁移到单独的共享目录（如后续新增 `docs/pict-models/` 或测试资产目录）。
- **当前 `owner-id-model1.pict` / `owner-id-model2.pict` 放在 `docs/plans/` 是合适的**，因为它们直接服务于 `owner-id-pict-test-matrix.md` 这份具体方案，不是通用公共模型。

推荐流程：
1. 用 `pict-test-designer` 输出参数、取值、约束、预期结果
2. 判断是否满足“必须再跑真实 PICT”的条件
3. 若需要，使用真实 PICT 生成 pairwise 组合结果
4. 将组合结果映射到现有测试族，补 Rust/前端测试代码
5. 仍需完成失败路径、契约、真实环境和安全审计验证，不能只停在组合测试

### 测试维度矩阵

| 测试维度 | 正常路径 | 错误路径 | 降级逻辑 | 真实环境 | 契约测试 | 安全审计 |
|---------|---------|---------|---------|---------|---------|---------|
| 单元测试 | ✅ 必须 | ✅ 必须 | ⚠️ 如有 | N/A | N/A | ✅ 必须 |
| 集成测试 | ✅ 必须 | ✅ 必须 | ⚠️ 如有 | ✅ 推荐 | ✅ 必须 | ✅ 必须 |
| E2E 测试 | ✅ 必须 | ✅ 推荐 | ⚠️ 如有 | ✅ 必须 | N/A | ✅ 推荐 |

### 覆盖率目标

| 维度 | 功能模块 | 安全模块 |
|------|---------|---------|
| 单元测试 | >90% | 100% |
| 集成测试 | >80% | 100% |
| 失败路径 | >80% | 100% |
| 契约测试 | >90% | 100% |
| 安全审计 | — | 100% |

### 覆盖率验证

```bash
# 全量覆盖率报告（需要 cargo-llvm-cov）
./scripts/coverage.sh

# 按模块过滤
./scripts/coverage.sh safety

# 输出 lcov 格式（CI 集成用）
COV_FORMAT=lcov ./scripts/coverage.sh

# 包含集成测试
COV_ALL_TARGETS=1 ./scripts/coverage.sh
```

安装：`cargo install cargo-llvm-cov`

### 测试文件组织

```
tests/
├── {module}_unit_tests.rs           # 单元测试
├── {module}_failure_tests.rs        # 失败路径测试
├── {module}_integration_tests.rs    # 集成测试
├── {module}_contract_tests.rs       # 契约测试
├── {module}_security_audit_tests.rs # 安全审计测试
├── {module}_reliability_tests.rs    # 可靠性测试
├── {module}_regression_tests.rs     # 回归测试
└── integration_smoke_tests.rs       # 编译+迁移+路由冒烟（全局唯一）
```

### 测试命名规范

- 需求测试: `req_{module}_{id}_{description}`
- 安全测试: `test_security_{attack_type}`
- 失败路径: `test_failure_{scenario}`
- 契约测试: `test_contract_{interface}_{case}`
- 审计测试: `test_audit_{security_concern}`
- 可靠性: `test_{failure_scenario}_recovery`
- 回归测试: `test_{feature}_backward_compatibility`

### 历史教训摘要

以下是项目中真实发生过的测试盲区，新功能开发时必须警惕：

| 盲区 | 教训 | 防护措施 |
|------|------|---------|
| DLP 失败路径缺失 | 测试只覆盖成功路径，降级逻辑允许原始消息发送 | 每个功能必须有失败路径测试 |
| model-configs 404 | `cargo test` 通过但 `cargo build` 失败，迁移文件存在但未执行 | `integration_smoke_tests.rs` 三层防护 |
| client-models 缺字段 | 测试主动断言了错误行为，把错误设计固化为"规范" | 契约测试从业务目标出发 |
| SSE 事件名不匹配 | 测试用模拟数据，未验证真实后端事件格式 | 真实环境集成测试 + 参考已有实现 |
| Tauri state not managed | 单元测试绕过 Tauri 状态注入，运行时 panic | 启动时序测试 + 冒烟测试 |
| tokio-postgres JSONB 类型 | `row.get::<_, String>()` 对 JSONB 列运行时 panic，编译期不报错；`tokio::time::interval` 首次 tick 立即触发，启动即崩溃 | 写 `row.get` 前核对迁移文件中列的实际类型；启动时立即执行的后台任务必须有冒烟测试覆盖 |

详细案例分析、代码示例和 E2E 测试模式见 `docs/testing-guide.md`。

---

## Admin Backend 冒烟测试（integration_smoke_tests.rs）

**强制要求**：每次新增迁移文件时，必须同步在 `integration_smoke_tests.rs` 的 `required` 列表中追加对应条目。

三层防护：
1. **编译检查** — 调用 `create_router()` 强制编译所有 handler
2. **迁移完整性** — 查询 `information_schema` 验证每个迁移对应的表存在
3. **HTTP 路由冒烟** — 用 `tower::ServiceExt::oneshot` 验证路由不返回 404

---

## Tauri 命令注册契约测试

所有 Tauri 命令通过 `all_tauri_commands!()` 宏集中管理（`desktop-client/src/lib.rs`），`main.rs` 和测试共用同一份注册表。

新增命令流程：
1. 在 `src/ipc.rs` 或 `src/commands.rs` 实现命令
2. 在 `all_tauri_commands!()` 宏中添加
3. 在 `tests/tauri_command_contract_tests.rs` 的 `FRONTEND_INVOKED_COMMANDS` 中添加
4. 运行 `cargo test --test tauri_command_contract_tests`
5. 前端添加对应 `invoke()` 调用

> 普通单元测试直接调用 Rust 函数，绕过 IPC 路由，无法捕获 "Command not found" 错误。契约测试是唯一能在 CI 阶段验证 IPC 路由完整性的方法。

---

## 启动时序测试（Desktop Client）

涉及 Tauri 状态管理的改动，必须运行：
```bash
cargo test -p desktop-client --lib engine_startup_tests
```

覆盖维度：启动时序（状态转换）、失败路径（未就绪时友好错误）、并发安全（多线程初始化）、冒烟测试（模拟真实启动流程）。

> 单元测试通过 ≠ 运行时安全。涉及异步状态注入的场景，必须用时序测试和冒烟测试覆盖。

参考：`desktop-client/src/engine_startup_tests.rs`

---

## 统一检查清单

### 功能开发

- [ ] 已检查主项目/共享 crate 是否有可复用能力
- [ ] 已遵循 TDD 流程（测试优先）
- [ ] 单元测试覆盖正常路径 + 错误路径
- [ ] 已编写集成测试和契约测试
- [ ] `cargo check -p <touched-crate> --tests` 本地编译通过（0 错 0 警）；完整 `cargo build` / workspace clippy 由 CI 兑现
- [ ] 所有测试通过
- [ ] **W3-A Phase 0 进行中**：若本 PR 属于 P0-1 ~ P0-10 任务族，必须按 [ADR-112 §5.4 评估嵌入节奏](docs/plans/architecture-refactor/adr-112-compatibility-evaluation.md#54-评估嵌入节奏b1--b2--b3) 自带 H1 unit + H2 integration 测试（W3-A 收尾后此项删除）

### 安全功能（额外）

- [ ] 失败路径测试覆盖率 100%
- [ ] 安全审计测试：敏感信息不泄露（日志、错误、网络）
- [ ] 降级逻辑采用 Fail-Safe 设计
- [ ] 契约测试验证业务目标可达成


### GitHub PR

- [ ] 当前切片已完成最小充分验证后再开 PR
- [ ] 已按需执行 `code-quality-audit`、`code-simplifier`、`adr-compliance-check`（触及 ADR 红线时）、`code-review-expert`
- [ ] PR 描述已写清背景、范围、非目标、验证、后续步骤
- [ ] 如果是 stacked PR，已写清 base、merge 顺序、先看哪个 PR
- [ ] 已至少运行一轮 `gh pr checks` 或等价检查并同步结果
- [ ] 如果当前已完成闭环 milestone，已按会话轮换原则写 handoff 并提醒用户考虑新会话

---

## 参考文档索引

- `docs/testing-guide.md` — 测试代码示例、历史教训详细分析、E2E 测试模式
- `docs/architecture-guide.md` — 架构详细说明、复用流程图、代码示例
- `desktop-client/DLP_LESSONS_SUMMARY.md` — DLP 测试盲区教训
- `desktop-client/DLP_TESTING_LESSONS_LEARNED.md` — 详细经验教训
- `desktop-client/DLP_SUPPLEMENTARY_TEST_PLAN.md` — 补充测试计划
- `desktop-client/src/engine_startup_tests.rs` — 启动时序测试
- `admin-backend/tests/integration_smoke_tests.rs` — 冒烟测试
- `desktop-client/tests/tauri_command_contract_tests.rs` — Tauri 命令契约测试
- `admin-backend/ui/cypress/` — Cypress E2E 测试
- `.kiro/steering/rust-coding-standards.md` — Rust 编码标准、错误处理模式、测试代码示例、质量门禁脚本
