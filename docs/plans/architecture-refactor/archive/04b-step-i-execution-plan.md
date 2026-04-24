# Step I 执行计划 — 删除 rig-core 与 rig_adapter.rs

> 归属：Phase 2（`04-phase2-claw-code-api.md`）最后一步
> 前置：Step A-H 已完成 + **生产稳定 ≥ 1 周**
> 预期改动量：~2300 行删除
> 预期耗时：1 天

---

## 1. 前置确认清单

动手前逐项打勾，任一项不满足就不要开始。

- [ ] Phase 2 合入 `x-claw-core` 分支时间 ≥ 7 天
- [ ] 这 7 天线上 / 本地灰度没有出现必须切回 `rig-llm` 的回归
- [ ] `cargo test --features claw-code-llm` 全绿（46 单测）
- [ ] `scripts/run-real-llm-tests.sh`（或本地 `cargo test --features claw-code-llm --test claw_code_real_llm_tests -- --ignored`）全绿（6 真实 LLM 测试）
- [ ] DLP / safety 回归测试全绿（5 项，Step H）
- [ ] `IRONCLAW_LLM_BACKEND=claw-code` 下 GUI 真实对话走过一次完整工具链路（目前已有 2-hop round-trip 证据）
- [ ] 本次 Step I 动手前在 `x-claw-core` 上新建一个 tag / checkpoint 分支，方便回滚
  - 建议：`git tag pre-step-i-$(date +%Y%m%d)` 并 push 到 `dasclaw`

---

## 2. 必须修改的文件清单（盘点 Source of Truth）

### 2.1 直接删除

| 路径 | 说明 |
|------|------|
| `desktop-client/ironclaw/src/llm/rig_adapter.rs` | 2026 行，核心删除对象 |
| `desktop-client/ironclaw/src/llm/rig_adapter_compat.rs` | 若存在（Step F 计划产物，当前实际已跳过，可能不存在） |
| `desktop-client/ironclaw/tests/` 下所有仅为 `rig_adapter` 存在的测试文件 | 见下文第 3 节搜索 |

### 2.2 需要编辑的文件（删除 `rig` / `rig_adapter` / `RigAdapter` / `ReasoningPatchClient` / `normalize_schema_strict` 引用）

根据 `grep` 结果，以下文件有确定引用，**执行前必须再 grep 一次确认**（可能有新增）：

- `desktop-client/ironclaw/Cargo.toml`
  - 删除：`rig-core = { version = "0.30", ... optional = true }` 这一行
  - 删除：`[features]` 里 `rig-llm = ["dep:rig-core"]`
  - `default = [...]` 从 `"rig-llm"` 拿掉
  - Phase 2 的注释块可以保留但改写为 `Phase 2 collapsed — claw-code-api only.`

- `desktop-client/ironclaw/src/llm/mod.rs`
  - 删除 `pub use rig_adapter::RigAdapter;`
  - 删除 `#[cfg(feature = "claw-code-llm")]` 三处 gate（因为 claw-code 变成唯一路径，可直接无条件编译）
  - 删除 `should_use_claw_code_backend()` 整个函数 + `IRONCLAW_LLM_BACKEND` env key
  - 删除 `create_registry_provider` 里的 `if should_use_claw_code_backend() { return claw_code_provider::... }` 分支，改为**无条件**走 claw-code
  - 删除 `ProviderProtocol::OpenAiCompletions|Anthropic|Ollama` 等匹配里所有调用 `create_*_from_registry` → `RigAdapter` 的路径（这些都由 `ClawCodeLlmProvider::from_registry_config` 接管）
  - `type PatchClient = rig_adapter::ReasoningPatchClient;` 这一行删除，相关 codex 路径改走 claw-code
  - mod.rs 里 `use rig::providers::openai | anthropic | ollama` 全部删除

- `desktop-client/ironclaw/src/llm/claw_code_provider.rs`
  - 移除 `#![cfg(feature = "claw-code-llm")]` 顶部 gate（变成默认模块）
  - 文件顶部注释 `替换 rig_adapter.rs（2026 行）` 改为 `取代原 rig_adapter.rs，唯一生产路径`
  - 所有 `fall back to --features rig-llm` / `build with --features rig-llm` 提示信息要删掉或替换为**直接报错**（该 provider 在 claw-code 里也不支持，就是不支持）
  - `GithubCopilot` 分支单独处理：**保留** `github_copilot::GithubCopilotProvider`（它原本就不走 rig），只要 `create_registry_provider` 保留那个 match 分支即可
  - 删除 3 处包含 `"rig-llm"` 的断言测试用例 / 把断言消息换掉

- `desktop-client/ironclaw/src/llm/openai_codex_provider.rs`
  - 第 499 行 `use crate::llm::rig_adapter::normalize_schema_strict;` → 把 `normalize_schema_strict` 搬到一个新模块（例如 `desktop-client/ironclaw/src/llm/schema_utils.rs`）保留，因为 **这不是 rig 专属工具**，其他路径还需要
  - **或**确认 claw-code-api 已有等价 schema 规范化，选择直接替换

- `desktop-client/ironclaw/src/llm/openai_streaming.rs`
  - 第 5 行注释 `Used by RigAdapter` → 改为 `Used by ClawCodeLlmProvider` 或直接删掉
  - 第 179 行 `super::rig_adapter::normalize_schema_strict(...)` → 引用新模块（同上）

- `desktop-client/ironclaw/src/llm/reasoning.rs`
  - 第 35 行文档注释 `from the 0 seed used in rig_adapter::normalized_tool_call_id` → 改为 `claw_code_provider::normalized_tool_call_id` 或直接用行为描述

- `desktop-client/ironclaw/src/llm/CLAUDE.md`
  - 全文替换所有 `RigAdapter` / `rig::providers` 提到的行为描述为 claw-code 对应实现（第 67, 158, 188, 227 行等）

- `desktop-client/ironclaw/FEATURE_PARITY.md`
  - 第 251, 569 行 Ollama 条目提到 `via rig::providers::ollama` → 改为 `via claw-code-api Ollama provider`

- `desktop-client/Cargo.toml`
  - 第 46, 94, 101 行注释提到 `--features claw-code-llm` + `IRONCLAW_LLM_BACKEND=claw-code` → 全部删掉或改为 "Phase 2 collapsed"
  - `[features]` 里 `claw-code-llm` 如果现在仍然是 opt-in，要确认是否改成默认或直接移除 feature 名（见下文第 4 节决策）

### 2.3 测试文件

- `desktop-client/ironclaw/tests/claw_code_real_llm_tests.rs`
  - 第 266 / 286 行的 `std::env::set_var("IRONCLAW_LLM_BACKEND", "claw-code")` 和 `remove_var` 全部删除（不再需要 env 开关即可走 claw-code）

- `desktop-client/ironclaw/src/llm/claw_code_provider.rs` 的 `env_routing` 单测族（1272-1335 行）
  - `IRONCLAW_LLM_BACKEND` 相关单测全部删除

- **全项目搜索删除残留**：执行前和执行后各跑一次
  ```bash
  # 在 x-claw 根目录
  grep -rn "rig_adapter\|rig::\|rig-core\|rig-llm\|IRONCLAW_LLM_BACKEND\|RigAdapter" \
      --include='*.rs' --include='*.toml' --include='*.md' .
  ```
  预期：执行后 0 结果（`target/` 和历史文档除外，历史文档是否保留留给归档决策）

---

## 3. 需要删除的测试目标（在 Cargo.toml 里）

无。当前 `[[test]]` 段没有任何 test 显式依赖 `rig-llm` feature（`claw_code_real_llm_tests` 依赖 `claw-code-llm`，而它要保留——但此时已默认开，可以把 `required-features` 去掉）。

---

## 4. 关键决策点（必须在动手前定）

### 4.1 `claw-code-llm` feature 保不保留？

**选项 A（推荐）**：保留 feature 名 `claw-code-llm` 作为 **no-op default feature**
- `default = ["postgres", "libsql", "html-to-markdown", "claw-code-llm"]`
- `claw-code-llm = ["dep:claw-code-api"]`
- 好处：下游依赖不需要同时改 `--features` 参数
- 坏处：多一个永远开着的 feature 名

**选项 B**：彻底移除 feature，把 `claw-code-api` 改为非 optional 依赖
- `claw-code-api = { path = "...", package = "api" }`（无 `optional = true`）
- `[features]` 里不再有 `claw-code-llm`
- 好处：彻底简化
- 坏处：`desktop-client/Cargo.toml`、CI 脚本、`scripts/run-real-llm-tests.sh` 所有传 `--features claw-code-llm` 的地方都要改

> **建议选 A**，延后选 B 到 Phase 3 或更晚。

### 4.2 `normalize_schema_strict` 去哪？

- `rig_adapter.rs` 里 line 186 是**项目原创的** OpenAI strict-mode schema 规范化工具，被 `openai_codex_provider.rs` 和 `openai_streaming.rs` 依赖
- 它与 rig 无强耦合，**不能跟着 rig_adapter.rs 一起删**
- 执行步骤：
  1. 新建 `desktop-client/ironclaw/src/llm/schema_utils.rs`（或合并到 `claw_code_provider.rs` 内 `pub(crate)` 模块）
  2. 把 `normalize_schema_strict` + 相关单测迁入
  3. 更新两处调用方 `use`
  4. 在删除 `rig_adapter.rs` 时确认该函数已在别处有副本

### 4.3 Codex ChatGPT `ReasoningPatchClient` 去向？

- `ReasoningPatchClient` 专为 rig 的 `http_client::HttpClientExt` 设计（line 1122）
- claw-code-api 原生支持 `reasoning_content`，**按计划 Codex ChatGPT 路径走 claw-code 之后不再需要这个 patch client**
- 执行步骤：
  1. 确认 `openai_codex_provider.rs` 已完全切到 claw-code-api（Step E 应该已完成，需核对）
  2. 未切的话，Step I 前把它切完；否则 Step I 放弃删除 `ReasoningPatchClient`
  3. 切完后 `ReasoningPatchClient` 跟 `rig_adapter.rs` 一起删

---

## 5. 执行顺序（严格按这个次序，避免破坏性中间态）

```
1. 创建 checkpoint: git tag pre-step-i-YYYYMMDD && git push dasclaw --tags
2. 新建分支: git checkout -b step-i-remove-rig
3. 搬迁 normalize_schema_strict → schema_utils.rs（或等价位置）
4. cargo build --features claw-code-llm -p ironclaw  ← 确认搬迁不破坏编译
5. 确认 openai_codex_provider 已不依赖 ReasoningPatchClient / rig
6. 编辑 Cargo.toml：
   - default 去掉 rig-llm
   - 删除 rig-llm feature
   - 删除 rig-core 依赖
7. 删除 src/llm/rig_adapter.rs
8. 编辑 src/llm/mod.rs：
   - 删 pub use rig_adapter::RigAdapter
   - 删 IRONCLAW_LLM_BACKEND 环境开关 + should_use_claw_code_backend
   - create_registry_provider 无条件走 claw-code
   - 删 use rig::providers::*
9. 编辑 claw_code_provider.rs：
   - 删掉所有 "fall back to rig-llm" 提示
   - 删 #[cfg(feature = "claw-code-llm")] gate（模块默认开启）
   - 删 IRONCLAW_LLM_BACKEND 相关单测
10. 编辑 tests/claw_code_real_llm_tests.rs：
   - 删 env set/remove
11. 清理文档：CLAUDE.md / FEATURE_PARITY.md / desktop-client/Cargo.toml 注释
12. cargo clean -p ironclaw
13. cargo build -p ironclaw                                 ← 期望 0 错 0 警
14. cargo build --all-features -p ironclaw                  ← 期望 0 错 0 警
15. cargo test -p ironclaw --lib                            ← 全绿
16. cargo test -p ironclaw --lib claw_code_provider         ← 46 测（注意：env_routing 子族删除后数字会下降）
17. scripts/run-real-llm-tests.sh                           ← 6 真实测试全绿
18. cd ../.. && cargo build -p desktop-client               ← 桌面端编译绿
19. cd desktop-client && cargo tauri dev → 手测一轮对话     ← GUI 冒烟
20. 最终全项目 grep 残留:
    grep -rn "rig_adapter\|rig::\|rig-core\|rig-llm\|IRONCLAW_LLM_BACKEND\|RigAdapter" \
        --include='*.rs' --include='*.toml' .
    期望 0 结果
21. git commit -m "chore: remove rig-core and rig_adapter (Phase 2 Step I)"
22. git push dasclaw step-i-remove-rig
23. 在 GitHub 发 PR 自审一遍
24. merge 回 x-claw-core
25. 更新父仓库 submodule 指针并 push
```

---

## 6. 验收标准（PR 合并门禁）

> 状态：✅ 已完成 / ⏳ 待跑（可选） / 详见 `04-phase2-claw-code-api.md` 末尾「待开发清单 T1–T5」

- [x] `desktop-client/ironclaw/src/llm/rig_adapter.rs` 不存在
- [x] `Cargo.toml` 不含 `rig-core` 依赖
- [x] `Cargo.toml` 不含 `rig-llm` feature
- [x] `cargo build -p ironclaw` 0 错 0 警（2025 最新一次）
- [ ] `cargo build --all-features -p ironclaw` 0 错 0 警 ← **可选，在下一次 feature 改动时顺带跑**
- [x] `cargo build -p desktop-client` 0 错 0 警（2025 最新一次）
- [x] `cargo test -p ironclaw --lib` 全绿（claw_code_provider 子族通过）
- [x] `scripts/run-real-llm-tests.sh` 中的 DashScope 分支已通过 `claw_code_real_llm_tests`（6/6）证明可用；其余 provider 非本次验收阻塞项
- [x] DashScope 下 `reasoning_content` 自然工作（无需手工 patch）← claw-code-api `openai_compat` hook 接管，`test_qwen_full_tool_round_trip_with_final_answer` 验证
- [x] 全项目 grep `rig_adapter|rig::|rig-core|rig-llm|IRONCLAW_LLM_BACKEND|RigAdapter` 结果为 0 逻辑命中（仅 4 条历史标注保留）
- [x] GUI 手测：最少一次带工具调用的真实对话走通（`test_qwen_tool_call_round_trip` + `full_tool_round_trip_with_final_answer` 已在测试层覆盖 2-hop 回路）

---

## 7. 回滚策略

Step I 是破坏性改动，一旦合并**无法通过 feature flag 切回**。回滚路径：

1. `git revert <step-i-merge-commit>`
2. 或从 `pre-step-i-YYYYMMDD` tag 新建热修复分支
3. Phase 2 的架构目标不依赖 LLM 实现层，前端 / DLP / dispatcher 等都不受影响

---

## 8. 可能踩到的坑

基于 Phase 2 已经暴露过的问题推测：

| 坑 | 预防 |
|-----|------|
| `normalize_schema_strict` 被删了才发现 Codex / 其他调用方还在用 | 第 5 节第 3 步**先搬迁再删源**，顺序硬性要求 |
| `github_copilot::GithubCopilotProvider` 误被当作 rig 路径删 | 它不走 rig，保留独立分支；代码审查时重点盯 |
| `openai_codex_session.rs` 里可能有隐式 rig 引用 | 执行第 15 步 `cargo build` 会揪出来；不过要注意 `cfg(feature = "...")` 下的代码不会被默认 build 检查 → 必须跑 `--all-features` |
| DashScope `object` / `finish_reason` 等字段在 claw-code-api 层已经处理 — 但如果某个老 trace fixture 假设 `ReasoningPatchClient` 的行为，可能失败 | 跑一遍 `cargo test -p ironclaw` 全量，不要只跑 `claw_code_provider` 子集 |
| Test traces (`tests/fixtures/llm_traces/`) 中若 record 了 rig 特有的请求/响应格式 | `TestRig` 是上层 harness，不直接依赖 rig；但 trace JSON 的 schema 需要与 claw-code-api 返回保持兼容 — 执行第 5 步 `cargo test` 会立刻暴露 |
| 桌面端 `desktop-client/Cargo.toml` 里 feature passthrough 没同步改 | 第 18 步构建桌面端；改动 `claw-code-llm` feature 就必须同步那一行 |
| GitHub CI / 生产构建脚本 (`scripts/*.sh`) 里还传 `--features rig-llm` | grep 扫 `scripts/` 并同步改 |

---

## 9. 不在 Step I 范围内

以下**不要**在 Step I 里顺手做，留给 Phase 3 / 后续：

- 拆分 `agent/` 为独立 crate（Phase 3 的事）
- `sandbox/` / `secrets/` 抽 trait（Phase 3）
- 升级 `claw-code-api` 上游版本（会引入额外 PR 风险，单独做）
- 优化 `claw-code-api` 覆盖的模型范围（Phase 2 验收时能跑的就保留，发现 gap 提 issue）
- 重写 `FEATURE_PARITY.md`（只更新 rig 相关条目，结构性重写另开 task）

---

## 10. 产出物

- `desktop-client/ironclaw/src/llm/rig_adapter.rs` 删除（~2026 行 -）
- `rig-core` crate 依赖删除（编译产物体积预期下降）
- 2 个 feature flag 减为 0（`rig-llm` + 可选的 `claw-code-llm`）
- 1 个环境开关 `IRONCLAW_LLM_BACKEND` 删除
- Phase 2 彻底收尾，Phase 3 可正式开工
