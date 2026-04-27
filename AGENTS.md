# Agent Rules

## 语言规则

**所有回答必须用中文。**

## 沟通规则

1. **不要假设用户清楚自己想要什么。** 当动机或目标不清晰时，停下来讨论，而不是猜测着往前冲。做错了再改的成本远高于多问一句。
2. **目标清晰但路径不是最短的，直接说并建议更好的办法。** 用户可能因为惯性选择了次优方案，AI 有责任指出更短的路径——但最终决定权在用户。

### 反馈原则

1. **开发完成后调用 MCP 工具**：
   - 每次代码开发完成后，必须调用 `mcp-feedback-enhanced` 工具。
   - 每次给开发者解释方案或者让开发选择方案，必须调用 `mcp-feedback-enhanced` 工具。
   - 工具调用后，Agent 必须等待用户反馈，收到明确指示后再继续执行后续任务。

### Skills 强制使用规范

每个开发回合都要主动核对是否触发 skill，不要靠"想起来"才用。以下三个 skill 是默认触发，**不是可选**：

| Skill | 触发时机 | 缺省后果 |
|-------|---------|---------|
| `code-quality-audit` | **写完一段非平凡实现后**（新函数/新模块/重构 > 50 LOC）、commit/push 前、用户说"审查/检查质量" | 会把补丁式代码、过长函数、unwrap/clone 滥用、重复造轮子的隐患合并进主线 |
| `code-simplifier` | **`code-quality-audit` 之后**，对刚改完的代码做收敛（消嵌套/去重复/改命名） | 留下啰嗦/低可读代码，后续重构成本飙升 |
| `code-review-expert` | **PR 自审前**（push 之前）、PR 合并前、用户说"review/审查这次改动" | 漏掉 SOLID 违规、安全风险、依赖耦合等高阶问题 |

**执行顺序（默认管线）**：
1. 实现完成 → `cargo build` 0 错误 0 警告
2. `code-quality-audit` 自查（必须）
3. `code-simplifier` 收敛（必须）
4. `cargo fmt` + `python3.12 scripts/check_no_panics.py`
5. commit 前 `code-review-expert` 自审（必须）
6. push + 开 PR

**不需要执行 skill 的场景**：
- 单字符/单行 typo 修复
- 仅文档/注释修改
- 仅 fmt/lint 自动修复
- 用户明确说"跳过 review，先跑起来"

**遗忘检测**：每次准备 push 前，先问自己"刚才有没有跑过 code-quality-audit 和 code-review-expert？"，如果没有，回到对应步骤补做。

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

**教训来源**：
- Round 1-15 多次误判（"codex 没 forkSubagent"、"ironclaw Prompt Cache 独家"）的根因均为字面量搜索陷阱。
- **Round 18 实证**：14 文档 Round 17 版本列出的 4 项 P0/P1 "缺口"（`<system-reminder>` 标签 / CYBER_RISK_INSTRUCTION 文本 / 多层 CLAUDE.md 加载 / 压缩阈值），经 `semantic_search` 验证全部是**伪缺口** —— claw-code `runtime/src/prompt.rs:480` 与 `prompt.rs:197`、codex `openai_models.rs:306` 早已实现。4/19 的文档错误率直接证明：**不做 semantic_search 就动笔写对账文档是不合格的**。

详见 [14-claude-code-capability-parity.md §0 §4.1 §6](docs/plans/architecture-refactor/14-claude-code-capability-parity.md)。

---

## 架构规则：代码复用优先

### 复用优先级

**Admin Backend**：
1. 通过 `ironclaw` crate 依赖主项目核心功能
2. 复用共享 Crate
3. 创建新的共享 Crate
4. 仅管理后台特有功能才独立实现

### 共享 Crate 规范

参考 `crates/ironclaw_auth/` 的模式。共享 crate 不应依赖应用层代码，API 变更需同步更新所有使用方。

### 可复用的 Web Gateway API

实现 Desktop Client 功能前必须检查：Memory、Chat、Jobs、Extensions、Skills、Routines、Logs、Approval 相关 handler 是否已存在于 `src/channels/web/handlers/`。

详细的架构说明、流程图和代码示例见 `docs/architecture-guide.md`。

---

## Rust 开发规范

### 开发流程（TDD）

新增 Rust 函数/模块/结构体/API 端点时（不含最小 bug 修复）：
1. 编写测试用例（Red）
2. 实现功能代码（Green）
3. 重构优化（Refactor）
4. `cargo build` 完整编译验证（不只是 `cargo test`）
5. 0 编译错误，0 编译警告

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

## E2E 测试

前端功能完成后必须编写 E2E 测试。推荐 Cypress（UI 测试）和 Playwright（SSE/跨浏览器测试）。

关键要求：
- 同时覆盖模拟环境和真实环境
- 新功能先检查主项目是否有相同实现（如 `src/channels/web/static/app.js`）
- 验证前后端实际通信格式，不只依赖 mock

参考：`admin-backend/ui/cypress/`、`docs/testing-guide.md`

---

## 统一检查清单

### 功能开发

- [ ] 已检查主项目/共享 crate 是否有可复用能力
- [ ] 已遵循 TDD 流程（测试优先）
- [ ] 单元测试覆盖正常路径 + 错误路径
- [ ] 已编写集成测试和契约测试
- [ ] `cargo build` 编译通过，0 错误 0 警告
- [ ] 所有测试通过

### 安全功能（额外）

- [ ] 失败路径测试覆盖率 100%
- [ ] 安全审计测试：敏感信息不泄露（日志、错误、网络）
- [ ] 降级逻辑采用 Fail-Safe 设计
- [ ] 契约测试验证业务目标可达成

### 新增迁移

- [ ] 已在 `integration_smoke_tests.rs` 的 `required` 列表追加表名验证

### 新增 Tauri 命令

- [ ] 已添加到 `all_tauri_commands!()` 宏
- [ ] 已添加到 `FRONTEND_INVOKED_COMMANDS`
- [ ] `cargo test --test tauri_command_contract_tests` 通过

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
