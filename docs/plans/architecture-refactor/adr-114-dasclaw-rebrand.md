# ADR-114: `.ironclaw` → `.dasclaw` 命名空间渐进迁移

- **Status**: Accepted
- **Date**: 2026-04-29
- **Accepted-On**: 2026-05-05
- **Approver**: pending
- **Supersedes**: 无
- **Related**: ADR-106（`.codex` 双读兼容）、ADR-112 §1（产品命名空间收口）、AGENTS.md "禁止补丁式代码"红线

---

## 1. Context — 事实基线

`desktop-client/ironclaw/` 子项目（fork 自 ironclaw 上游）当前以 `.ironclaw` 为本地数据/配置/服务的命名空间根：

| 类别 | 位置 | 字面量 |
|---|---|---|
| **入口 API** | `bootstrap.rs::ironclaw_base_dir()` / env var `IRONCLAW_BASE_DIR` | `~/.ironclaw` 默认 |
| **运行数据** | `~/.ironclaw/{ironclaw.db, ironclaw.pid, gateway.log, .env}` | 用户已有数据 |
| **应用配置** | `~/.ironclaw/{settings.json, config.toml, mcp-servers.json, session.json}` | 用户已有配置 |
| **业务沙箱** | `workspace_dir.rs::projects_base()` = `~/.ironclaw/projects/{thread_id}/` | Docker bind mount 安全前缀 |
| **插件资产** | `~/.ironclaw/{tools/, channels/}/*.wasm` | 用户已安装产物 |
| **OS 服务** | launchd `com.ironclaw.daemon.plist` / systemd `ironclaw.service` | 用户已注册的系统服务 |
| **CLI 提示 / 文档** | `cli/{models,config,logs,tool,hooks,doctor}.rs` 及 `docs/*.md` 多处 | 用户可见路径文本 |
| **DB 迁移注释** | `migrations/V8__settings.sql` 等 | 注释字面量 |

**与 `.openclaw` 的边界**：`import/openclaw/mod.rs::detect()` 检测 `~/.openclaw`，是上游 OpenClaw 工具的独立命名空间，**不在本 ADR 范围**。

**与 ADR-106 的关系**：`dasclaw_project_docs::PriorityProjectDocLoader` 已实现 `~/.dasclaw/AGENTS.md > ~/.codex/AGENTS.md` 双读优先，user-global 层文档加载**不依赖**本 ADR。

### 1.1 直接 sed 的破坏性

| 风险 | 说明 |
|---|---|
| **数据孤立** | 旧用户 `~/.ironclaw/ironclaw.db`、secrets keychain 引用、settings.json 在直接改名后变成孤儿，重启后用户体验为"全新安装" |
| **公开 API 破坏** | `IRONCLAW_BASE_DIR` 是用户脚本 / CI / Docker compose 中的入口变量；直接删除等于无声 break |
| **OS 服务孤儿** | launchd plist / systemd unit 文件名一旦改，旧装的服务不会自停，造成新旧双开或端口冲突 |
| **半改半未改** | 多 PR 各自顺手改一处字面量，留下"路径已改名但默认目录未改 → 跑去找空目录"的中间态——这正是 AGENTS.md 红线"补丁式代码"的反例 |

---

## 2. Decision

### 2.1 目标终态

| 项 | 旧 | 新 |
|---|---|---|
| 默认数据根 | `~/.ironclaw` | `~/.dasclaw` |
| 主 env 变量 | `IRONCLAW_BASE_DIR` | `DASCLAW_BASE_DIR` |
| binary 名 | `ironclaw` | `dasclaw`（兼容 symlink 留 `ironclaw`） |
| launchd label | `com.ironclaw.daemon` | `com.dasclaw.daemon` |
| systemd unit | `ironclaw.service` | `dasclaw.service` |
| crate 内符号 | `ironclaw_base_dir()` 等 | `dasclaw_base_dir()` 等（保兼容 deprecated alias） |

### 2.2 规则二分（关键）

ADR 把全部修改点划分为**两类**，**严禁混做**：

#### 类 A：**可"顺手改"** — 各 W3+ PR 在自己影响半径内对齐

适用范围：
- A-1：**新增**的代码、文档、prompt 文案、测试
- A-2：本 PR 已经动到的现存文件中**纯展示文本**（用户可见 CLI 输出、docs/ markdown、源码注释中的路径示例）
- A-3：本 PR 新增引用 user-global 路径处——必须调用 §2.3 的 helper，不得硬编码 `.ironclaw` 字面量

操作规则：
- 每个 PR body 在"改动范围"中显式标注 `Cross-cuts: ADR-114 类 A 顺手改`
- 不得在类 A 名义下改动 §2.3 列出的"集中工程"任何一项
- 现存代码中的字面量**未触及不必动**——避免 PR scope 蔓延

#### 类 B：**集中工程** — 各自独立 issue + 独立 PR

| B-Ⅰ | bootstrap dual-read | `bootstrap.rs::compute_ironclaw_base_dir` 改为 `compute_base_dir`：先读 `DASCLAW_BASE_DIR`，回退 `IRONCLAW_BASE_DIR`（带 `tracing::warn` 提示 deprecated）；默认目录改成 `~/.dasclaw`，但若 `~/.ironclaw` 存在且 `~/.dasclaw` 不存在则启动时 emit migration notice |
| B-Ⅱ | 数据迁移器 | 新增 `bootstrap::migrate_ironclaw_to_dasclaw`：首启动检测 `~/.ironclaw/ironclaw.db` 等 → symlink 或 move 到 `~/.dasclaw/`；写迁移标记 `~/.dasclaw/.migrated_from_ironclaw`；可重入；失败 fail-safe（保留旧目录 + 提示用户手动） |
| B-Ⅲ | 内部 API rename | `ironclaw_base_dir()` → `dasclaw_base_dir()`，旧名保留为 `#[deprecated]` reexport 一个发布周期 |
| B-Ⅳ | OS 服务改名 | `service install` 子命令同时支持旧名 uninstall + 新名 install；提供 `dasclaw service migrate` 子命令 |
| B-Ⅴ | binary / crate 改名 | Cargo workspace 改 `ironclaw` package 名 → `dasclaw`，加 `[[bin]]` alias 保留 `ironclaw` 启动入口指向同一 main |

每项 B 单独 issue，引用本 ADR 编号；不得在 W3 ProjectDocs / W4 hook 收口等业务 PR 中"顺手"做。

### 2.3 强制 helper（类 A 的护栏）

新增 `desktop-client/ironclaw/src/bootstrap.rs::base_dir()` 作为类 A 唯一允许的入口：
- 在 B-Ⅰ 落地前：内部仍调用 `ironclaw_base_dir()`（行为兼容）
- 在 B-Ⅰ 落地后：内部走 dual-read 逻辑

类 A 中**新增**任何路径引用必须调用 `base_dir()`，**禁止**：
- 硬编码 `~/.ironclaw` 或 `.ironclaw` 字符串字面量于新代码
- 直接调 `dirs::home_dir().join(".ironclaw")`（grep guard 见 §4.2）

### 2.4 文档与 prompt 文案

类 A 范围内，**新增**文档/prompt **必须**用 `~/.dasclaw/`；现有文档不强制改写（避免 doc churn），但当 PR 因其他原因已动到该 doc 文件时**鼓励**顺手改。

### 2.5 与 ADR-106 的耦合

`dasclaw_project_docs` user-global 层（`~/.dasclaw/AGENTS.md > ~/.codex/AGENTS.md`）**已经**符合本 ADR 终态，无需改动。涉及该 crate 的后续 PR（含 #59）按类 A 规则处理新增引用即可。

---

## 3. Consequences

### 3.1 正面

- 用户数据零丢失（B-Ⅱ 迁移器保证）
- env API 一个发布周期内向后兼容（B-Ⅰ dual-read）
- 业务 issue 不被 rebrand 阻塞（类 A 顺手改，类 B 并行推进）
- 防止"补丁式代码"——任何半改半未改都会撞 §4.2 grep guard

### 3.2 负面 / 成本

- 文档 churn：发布周期内一段时间出现新旧路径并存，需要 README 顶部加 deprecation banner
- B-Ⅱ 迁移器复杂度：要处理 keychain 引用、绝对路径写入 settings.json 中的旧目录字符串等边界
- CI 矩阵需新增 `DASCLAW_BASE_DIR` 用例

### 3.3 风险

- 类 A / 类 B 边界判定主观争议 → 由 §4 检查清单 + grep guard 强制执行
- B-Ⅴ crate rename 触发整个 workspace `Cargo.lock` churn，需独立时间窗

---

## 4. Enforcement

### 4.1 PR 模板补充

每个 W3+ PR body 必须包含：
```
Cross-cuts: ADR-114 [类A 无新增 .ironclaw 字面量 | 类B issue#XXX | 不涉及]
```

缺该字段由 board sync 脚本拒绝。

### 4.2 Grep guard（CI）

新增 `scripts/check_no_new_ironclaw_literal.py`：
- 入参：`--base origin/xClaw`
- 行为：对 PR diff 中**新增行**做 grep `\.ironclaw\b|IRONCLAW_BASE_DIR`
- 命中 → 失败，提示参考本 ADR
- 例外白名单：`docs/plans/architecture-refactor/adr-114-*.md` 自身、B-Ⅰ/B-Ⅱ/B-Ⅲ/B-Ⅳ/B-Ⅴ 标记的 issue PR（通过 PR label `adr-114-class-b` 豁免）

### 4.3 类 B issue 链

落地本 ADR 后立即开 5 个 issue：
- `chore(bootstrap): adr-114-Ⅰ — DASCLAW_BASE_DIR dual-read`
- `chore(bootstrap): adr-114-Ⅱ — ~/.ironclaw → ~/.dasclaw migration helper`
- `chore(bootstrap): adr-114-Ⅲ — rename internal base_dir API`
- `chore(service): adr-114-Ⅳ — launchd/systemd unit rename`
- `chore(workspace): adr-114-Ⅴ — Cargo package + binary rename`

5 个 issue 全部完成后，本 ADR 升 `Status: Accepted` + 删除 §4.2 白名单（grep guard 转为强制）。

---

## 5. 决策范围外

- `~/.openclaw` 导入器（独立上游命名空间）
- `ironclaw-main/` 旧仓库代码（参考材料，本 ADR 仅约束 `desktop-client/ironclaw/` 子项目和 `crates/` 下新代码）
- `decode-claude-code-main/` / `claw-decode-main/` / `codex-cli-main/` 等参考库（只读快照）

---

## 5.A Accepted Addendum (2026-05-05)

5 个类 B 子 issue（#106 #107 #108 #109 #110）全部 closed，grep guard
`scripts/check_no_new_ironclaw_literal.py` + CI job `no-new-ironclaw-literal`
（`.github/workflows/code_style.yml`）+ PR 模板 Cross-cuts 字段已落地，本 ADR
升级为 **Accepted**。

post-Accepted 状态下 §4.2 白名单的实际形态：

- **文件级白名单**保留：仅 ADR-114 自身 markdown、grep guard 脚本本身、
  `.github/pull_request_template.md`、`.github/workflows/code_style.yml`。
  这 4 个文件存在意义就是描述这两个字面量（守卫规则、CI 接入、PR 模板说明），
  纳入业务 grep 没有意义。新增任何业务代码 / 子 crate 进白名单一律拒绝。
- **Label 豁免** `adr-114-class-b` 保留：用于 OQ-1 / OQ-2 / OQ-3 等仍开放的
  收尾子 issue，以及 `bootstrap.rs` 的 dual-read 维护型 PR。任何使用该 label
  的 PR 必须在描述里说明为什么属于类 B（CI 不强制，由 reviewer 拦截滥用）。
- **业务代码不进入任何白名单**：所有 `desktop-client/`、`crates/`、
  `admin-backend/` 下的 PR diff 必须 0 新增字面量；如撞守卫，要么改用
  `dasclaw_*` 等价物，要么打 class-B label 并在 PR 描述说明依据。

「白名单转为强制」在原文中的精神含义已完成：grep guard 默认对所有非 class-B PR
强制，不再有任意业务代码 PR 享有沉默豁免。

---

## 6. Open Questions

- **OQ-1**：B-Ⅴ crate rename 时，`desktop-client/ironclaw/` 目录路径是否同步改为 `desktop-client/dasclaw/`？建议**是**，但需独立子 issue（git history 影响）
- **OQ-2**：keychain entry name `com.ironclaw.*` 是否纳入 B-Ⅱ 迁移器？建议**是**（不迁则用户所有 secrets 失联）
- **OQ-3**：`migrations/V8__settings.sql` 中的注释字面量是否走类 A？建议**是**（注释是纯文本展示）
