# 41 — 当前现状 vs 31-target-architecture.md 漂移分析

> **生成日期**: 2026-04-30
> **对照文档**: [31-target-architecture.md](31-target-architecture.md) v2.4 (2026-04-26)
> **当前 base**: `origin/xClaw` @ commit `f0c1750c`（PR #751 ADR-155 已合并）
> **目的**: 识别蓝图与实际仓库的差距，判断 31 文档是否需要升级到 v2.5

---

## 1. TL;DR — 31 文档落后明显

**结论**: 31-target-architecture.md v2.4 写于 2026-04-26，距今已实际推进 4-5 天密集开发，**蓝图存在 3 类漂移**：

1. **蓝图遗漏 16 个 crate**：crates/ 实际有 45 个，蓝图只覆盖 26 个，漏了 16 个真实存在的 crate
2. **2 个蓝图 crate 未建**：`dasclaw_bridge_lite`、`dasclaw_common` 仍是 MISSING
3. **工具壳层下沉规划缺失**：蓝图 L4 提了"dasclaw_tools"概念但未细化，F4.6（builtin 工具壳下沉）未在 §4 crate 清单中显式建模

---

## 2. 蓝图 P0/P1/P2 落地情况（24 个 crate）

### 2.1 P0 必做（蓝图列 7 个）

| crate | 状态 | 备注 |
|---|---|---|
| `dasclaw_core` | ✅ 已建 | x_claw_agent 升级，agentic_loop 已 port |
| `dasclaw_bridge_lite` | ❌ **MISSING** | 蓝图核心 P0 缺口 |
| `dasclaw_sandbox` | ✅ 已建 | + `dasclaw_sandbox_linux` / `dasclaw_sandbox_windows` / `dasclaw_sandboxing` |
| `dasclaw_hooks` | ✅ 已建 | F3.x 完成 |
| `dasclaw_apply_patch` | ✅ 已建 | 1277 行（codex port） |
| `dasclaw_project_docs` | ✅ 已建 | |
| `dasclaw_pty` | ✅ 已建 | |

**P0 完成度**: 6/7 = **86%**（缺 `dasclaw_bridge_lite`）

### 2.2 P1 推荐（蓝图列 7 个）

| crate | 状态 | 备注 |
|---|---|---|
| `dasclaw_governance` | ✅ 已建 | |
| `dasclaw_mcp` | ✅ 已建 | |
| `dasclaw_execpolicy` | ✅ 已建 | |
| `dasclaw_bash_validation` | ✅ 已建 | |
| `dasclaw_lsp` | ✅ 已建 | 但 builtin/lsp/ 工具壳未下沉 |
| `dasclaw_git_tools` | ⚠️ 空壳 | lib.rs 仅 24 行，等 F4.6 |
| `dasclaw_routines` | ✅ 已建 | F4.2.0 PR #729 完成 1566 行 |

**P1 完成度**: 6.5/7 = **93%**（`dasclaw_git_tools` 是空壳）

### 2.3 P2 增强（蓝图列 5 个）

| crate | 状态 | 备注 |
|---|---|---|
| `dasclaw_features` | ✅ 已建 | |
| `dasclaw_observability` | ✅ 已建 | |
| `dasclaw_identity` | ✅ 已建 | |
| `dasclaw_crash` | ✅ 已建 | |
| `dasclaw_net_proxy` | ✅ 已建 | drift guard `check_codex_net_proxy_drift.py` 已挂 |

**P2 完成度**: 5/5 = **100%**

### 2.4 蓝图 §4.4 已存在保留（4 个）

| crate | 状态 |
|---|---|
| `ironclaw_auth` | ✅ 存在 |
| `dasclaw_workspace_cap` | ✅ 存在 |
| `dasclaw_safety`（升级自 `ironclaw_safety`） | ✅ 已升级 |
| `dasclaw_common`（升级自 `ironclaw_common`） | ❌ **MISSING** — 升级未完成 |

---

## 3. 蓝图遗漏的 16 个 crate（**重大落后**）

实际 crates/ 存在但 31 §4 完全未提及：

| crate | 推断角色 | 来源 |
|---|---|---|
| `dasclaw_absolute_path` | 路径工具 | 底座 |
| `dasclaw_bash_permissions` | bash 权限规则 | **port from claude-code TS bashPermissions.ts** |
| `dasclaw_cert_trust` | 证书信任 | 底座 |
| `dasclaw_channels` | 通道抽象 | fork ironclaw 0.24 channels 提升 |
| `dasclaw_exec` | 执行整合 | W2.6 整合层 |
| `dasclaw_llm_provider` | LLM provider 抽象 | 新建 |
| `dasclaw_process_hardening` | 进程加固 | 底座 |
| `dasclaw_runtime` | 应用层运行时类型 | **核心** — 蓝图未列是重大遗漏 |
| `dasclaw_sandbox_linux` | Linux landlock | 蓝图说"dasclaw_sandbox"，实际拆了三平台 crate |
| `dasclaw_sandbox_windows` | Windows JobObject | 同上 |
| `dasclaw_sandboxing` | 沙箱抽象层 | 同上 |
| `dasclaw_shell_command` | shell 解析 + 安全 | **底座** — 蓝图说"L4 TOOLS"但 crate 边界未画 |
| `dasclaw_tool` | Tool trait 框架 | 框架层 |
| `dasclaw_utils_home_dir` | home_dir 工具 | utils |
| `dasclaw_utils_rustls_provider` | rustls 适配 | utils |
| `dasclaw_wasm_tools` | WASM 沙箱 | F4.5 部分完成 |

**判断**: 这 16 个 crate **不是"违规建出来的"**，而是蓝图写得过于抽象（只画了 L4 概念图，没把每个能力 crate 列详细）。实际工程上要把"能力域"拆成可独立测试编译的 crate，自然就比蓝图细化得多。

---

## 4. 蓝图 ADR-101 与 F4 实际推进的关系

### 蓝图描述（§3 ADR-101）

> "以 `crates/x_claw_agent`（已完成 LoopDelegate Route B）为起点升级为 `dasclaw_core`；上游 ironclaw-main 0.26 的关键模块（gate/ bridge/ auth/ ownership/）逐项移植到 dasclaw_* 子 crate"
>
> 调整后工量：**4-6 周**

### 实际推进（截至今日）

- ✅ ADR-152 §3 把吸收式重构拆成 F1-F6 六个子波次
- ✅ F1-F3 已完成（命名翻转 / 类型迁移 / 工具子集）
- ⚠️ F4 进行中：F4.0/F4.1/F4.2/F4.3/F4.4/F4.5 部分完成，**F4.6 未启动**
- ❌ **F4.6 builtin 工具壳下沉**在蓝图中完全没单独建模（蓝图只在 L4 画了"dasclaw_tools"概念框）

**判断**: 蓝图 §3 ADR-101 的"4-6 周"估算可能略低估，因为 F4.6 单独的工作量（~16000 行工具壳）蓝图没单独算账。

---

## 5. ADR-155（最新）vs 蓝图 §10 ADR 清单

蓝图 §3 ADR 清单截止 ADR-110，**ADR-111 到 ADR-155 共 45 个新 ADR** 蓝图完全没纳入。

最近 5 个关键 ADR 对蓝图的影响：

| ADR | 决策 | 是否影响蓝图 |
|---|---|---|
| ADR-152 | F4 子波次拆分（F4.0-F4.6） | ✅ 蓝图 ADR-101 落地路径细化 |
| ADR-153 | headless agent 框架强制（dasclaw_runtime 不能有 HTTP/DB/tenant） | ✅ **重大**：蓝图 L3 边界更严格 |
| ADR-114 | `.ironclaw` → `.dasclaw` 命名迁移 | ✅ 蓝图未提 |
| ADR-129 §1.3 | verbatim port 红线 | ✅ 蓝图未提 |
| ADR-155 | orchestrator 留 desktop（不迁 crates/） | ✅ **重大**：蓝图 L2-L3 边界明确化 |

---

## 6. 蓝图明确落后的 4 处具体内容

### 6.1 L4 能力域 `dasclaw_tools` 概念过时

蓝图 §1 图把所有 builtin 工具都画在 `dasclaw_tools` 一个 crate 里，但实际工程要做的是：

- ✅ 底座（`dasclaw_apply_patch` / `dasclaw_shell_command` / `dasclaw_bash_*` / `dasclaw_execpolicy`）已下沉
- ❌ 工具壳层（28 个 builtin *.rs）尚未下沉，F4.6 待启动
- ❓ 工具壳是否拆成多个 `dasclaw_builtin_*` crate 还是塞进单一 crate 未定（见 40 文档 §7.4 Q1）

### 6.2 §4 crate 清单需要从 24 扩到 ~45

蓝图 §4.1/4.2/4.3 表格需要重写，加入：

- 9 个新 crate（runtime / tool / exec / shell_command / bash_permissions / channels / wasm_tools 等）
- 三平台 sandbox 拆分细节（sandbox_linux / sandbox_windows / sandboxing）
- F4.6 工具壳相关新 crate（待 F4.6 ADR 决策后填）

### 6.3 §5 desktop-client 客户端外壳清单需要更新

蓝图 §5.1 "零改动保留" 列了 9 项，但 ADR-155 又新增了：

- **orchestrator 5 个文件**（agent_orchestrator.rs / orchestration_executor.rs / orchestrator_state.rs / orchestrator_commands.rs / orchestrator_runtime.rs）明确留 desktop
- **builtin 工具中类型 III 9 项**（job/message/restart/extension/secrets_tools/image_*3 + skill_tools）建议留 desktop（见 40 文档 §5）

### 6.4 §8 待用户决策点 D1-D10 进展未更新

蓝图列了 10 个决策点 D1-D10，但实际推进中：

- D1（dasclaw_core 合并）→ ✅ 已落地（agentic_loop port）
- D2（fork 处理）→ ✅ ADR-105 v2.3 已定（保留 fork 至 W6+）
- D5（命名前缀）→ ✅ 已定 dasclaw_*
- D7（AGENTS.md 优先级）→ ✅ ADR-106 已落地
- D8（Approval 推送）→ ❓ 未推进
- D9（crash 后端）→ ❓ 未推进
- D10（bash_validation 独立）→ ✅ 已落地

蓝图 §8 应该把已落地的标记为 done，未推进的转给后续 ADR。

---

## 7. 重大落后判定

| 维度 | 落后程度 | 影响 |
|---|---|---|
| crate 数量遗漏 | 🔴 严重（16/45 = 35% 漏列） | 后人按蓝图找 crate 会找不到 |
| ADR 清单时效性 | 🔴 严重（缺 45 个新 ADR） | 蓝图 §3 已不能反映完整决策史 |
| 工具壳下沉规划 | 🟡 中等（L4 概念过时） | F4.6 启动前必须补 |
| desktop 留守清单 | 🟡 中等（缺 orchestrator + 类型 III 9 项） | 但 ADR-155 已独立记录 |
| L3/L4 边界定义 | 🟡 中等（ADR-153 比蓝图更严） | ADR-153 是 source of truth |
| 整体架构图 | 🟢 基本可用 | mermaid 图大方向仍正确 |

---

## 8. 建议处理方式

### 选项 A：发布 31 v2.5（推荐）

- 把蓝图从 v2.4 升级到 v2.5
- §4 crate 清单从 24 扩到 ~45（按 P0/P1/P2 + utils + builtin 五档分类）
- §3 ADR 清单加 ADR-111~155 索引（不复制全文，只列标题+链接）
- §5 desktop 保留清单加 orchestrator + 类型 III 9 项
- §8 决策点 D1-D10 标记进展（done/pending）
- 新增 §11 F4.6 builtin 工具壳下沉规划（待 F4.6 ADR 落地后回填）

**工作量**: 1 个 PR，~300 行文档改动，半天完成

### 选项 B：写 31 增量补丁 ADR

- 不动 31 v2.4 主体
- 新建 ADR-156（如）"31 蓝图增量更新：crate 清单 + F4.6 规划"
- 把蓝图缺口写进增量 ADR

**工作量**: 同 A，但分割文档维护成本更高

### 选项 C：先做 F4.6 决策 ADR，再统一升级 31 v2.5

- 先开 F4.6 落点决策 ADR（类似 ADR-155 形态）
- F4.6 决策定下来后，把"工具壳下沉规划"和"crate 数量补全"一起写进 31 v2.5

**工作量**: 同 A，但顺序更稳

---

## 9. 推荐顺序

1. **先做选项 C 第一步**：开 F4.6 落点决策 ADR（解决 Q1-Q3 见 40 文档 §7.4）
2. **F4.6 ADR 合并后**：升级 31 v2.5（按选项 A）
3. **后续**：F4.6 实施开始（按新 ADR 拆切片 PR）

---

## 10. 三层验证证据

- ✅ `ls -d crates/dasclaw_*` 取 45 个 crate 真实清单
- ✅ 蓝图 §4 列表用 grep 提取并比对
- ✅ `head -8 crates/*/src/lib.rs` 验每个 crate 来源注释
- ✅ ADR-155 PR #751 commit `f0c1750c` 已合并验证
- ✅ AGENTS.md 三层规约：semantic_search → vscode_listCodeUsages → grep，本次第一层因 embeddings=0 失败已回退 grep
- ✅ ADR-114 类 A 守卫：本文档不含 `.ironclaw` / `IRONCLAW_BASE_DIR` 新增字面量

---

## 11. 执行进度回顾（2026-05-25 更新）

> 本节是对 §1~§10 历史快照（2026-04-30）的增量状态覆盖。原文按时间封存，不重写。
> 当前 base：`origin/xClaw` @ PR #792（ADR-156 §6.4 mod.rs 薄壳化）合并后。

### 11.1 §9 推荐顺序的执行情况

| 步骤 | 状态 | 落地证据 |
|---|---|---|
| 1. F4.6 落点决策 ADR | ✅ 完成 | ADR-156（`adr-156-f46-builtin-tools-landing-decision.md` 已合入主线） |
| 2. F4.6 实施 | ✅ phases 4.6.1 ~ 4.6.8 + §6.4 全部完成 | PR #784 / #786 / #788 / #790 / #792 已合并 |
| 3. 31 v2.5 升级 | ⏳ 待启动 | 计划下一个 PR 处理（本文件状态同步是其前置） |

### 11.2 crate 清单状态变化（41 § 2~3 表格的更新）

- **总数**：从 §1 写作时的 45 → 当前 47（新增 `dasclaw_fs_tools`、`dasclaw_misc_tools`、`dasclaw_image_tools`、`dasclaw_net_tools`、`dasclaw_shell_tools`、`dasclaw_sub_agent_tools` 等 F4.6 系列下沉 crate，部分名字源自 §3 当时未列项）。
- **§2.2 P1**：`dasclaw_git_tools` 当时标记"⚠️ 空壳 24 行"，现已落地为完整 git 工具 crate（F4.6 系列 PR）。
- **§2.4 升级未完成项**：`dasclaw_common` 仍为 MISSING（ADR-156 未涵盖，留 ADR-101 后续）；`dasclaw_bridge_lite` 同样仍为 MISSING（仍属 ADR-156 §6 PLANNED 清单的合法待办）。
- **`check_blueprint_sync.py`** 当前自检：`47 crates on disk, 2 planned`（`dasclaw_bridge_lite` + `dasclaw_memory_tools`，后者按 ADR-156 §6.3.1 取消下沉，留 PLANNED 项见 ADR-101 后续清理）。

### 11.3 §4 / §6.1 工具壳层下沉的最终落点

ADR-156 落地后，原 28 个 builtin `*.rs` 的处置：

| 类型 | 处置 | 涉及 |
|---|---|---|
| 下沉到独立 crate | ✅ 完成 | `fs_tools` / `git_tools` / `image_tools` / `misc_tools` / `net_tools` / `shell_tools` / `sub_agent_tools` / `lsp`（共 8 个）|
| 留守 desktop（ADR-156 §6.3.1 / §8 + ADR-155） | ✅ 维持 | `memory` / `tool_info` / `extension_tools` / `skill_tools` / `job` / `routine` / `message`（7 个） |
| 入口聚合层 | ✅ 已薄壳化（§6.4） | `desktop-client/ironclaw/src/tools/builtin/mod.rs` 改为 wildcard re-export + 留守 `pub mod`，shim `shell.rs` / `lsp/mod.rs` 已删 |

§ 1 的"3 类漂移"中，"工具壳层下沉规划缺失"项已被 ADR-156 全面覆盖，不再属于漂移。

### 11.4 §5 ADR 清单进展

§5 表格列出 5 条最近 ADR，本次新增重要 ADR：

| ADR | 状态 | 内容 |
|---|---|---|
| ADR-156 | ✅ 已落地（F4.6 全部 phase + §6.4 收尾合并） | F4.6 builtin 工具壳下沉决策 |

### 11.5 §6.4 决策点 D1-D10 增量

| 决策点 | §6.4 状态 | 本次更新 |
|---|---|---|
| D8（Approval 推送）| 未推进 | 仍未推进 |
| D9（crash 后端）| 未推进 | 仍未推进 |

（其余决策点维持 §6.4 标注。）

### 11.6 §7 落后矩阵的状态更新

| 维度 | §7 标注 | 当前状态 |
|---|---|---|
| 工具壳下沉规划 | 🟡 中等 | 🟢 已闭环（ADR-156 落地 + 实施完毕） |
| crate 数量遗漏 | 🔴 严重 | 🟡 部分闭环（数量已稳定 47，蓝图 §4 仍未升级 — 等 31 v2.5） |
| ADR 清单时效性 | 🔴 严重 | 🟡 维持（待 31 v2.5 整合 ADR-156） |
| desktop 留守清单 | 🟡 中等 | 🟢 已收敛（ADR-156 §6.3.1 + §8 已明确） |

### 11.7 下一步

按 §9 推荐顺序，下一步是 **31 v2.5 升级**（选项 A）：把 §4 crate 清单从 24 扩到 47、§3 ADR 清单补齐 ADR-111~156、新增 F4.6 落地总结小节。该任务作为本 PR 的后续 PR 独立提交。

