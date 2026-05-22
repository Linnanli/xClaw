# ADR-152 F4.5 wasm 切片提案（F4.5.2 / F4.5.3）

- 状态：Proposed
- 决策范围：ADR-152 §3 F4.5 sub-波次拆分（F4.5.2、F4.5.3）
- 关联 ADR：[ADR-152 §3](adr-152-agent-and-capability-fusion.md)（F4.5 verbatim 清单）、
  [ADR-129 §1.3](adr-129-sandbox-windows-windows-crate-adoption.md)（verbatim port 红线）
- 已落地前置：
  - F4.5.1（PR #733，已合并）—— `channels` 的核心子集
    `trait + relay + manager` 抽到 `crates/dasclaw_channels`；
    transport（REPL/HTTP/Signal/Webhook/Web/Telegram）+ `channels/wasm` 留 desktop。
    F4.5 原清单中的 wasm 部分推迟到本 ADR 论证后再做。

## 1. 背景

ADR-152 §3 关于 F4.5 的原文（[adr-152-agent-and-capability-fusion.md:88](adr-152-agent-and-capability-fusion.md#L88)）：

> **F4.5**：`channels` 拆分——抽核心子集（trait + relay + wasm + manager）到新 crate，
> REPL/HTTP/Signal/Webhook/Web/Telegram 留 ironclaw。

F4.5.1 已经把 `trait + relay + manager` verbatim 搬到 `dasclaw_channels`，但
**wasm 这一块比原清单复杂得多**：把 `channels/wasm` 单独搬进 `dasclaw_channels`
会立即触发跨 crate 的破裂依赖，因为 `channels/wasm` 在 ironclaw 内部依赖 `tools/wasm`。

## 2. 当前 wasm 子系统的真实分层（已核查）

通过 `grep_search` 在 [desktop-client/ironclaw/src/channels/wasm/](../../../desktop-client/ironclaw/src/channels/wasm/) 与
[desktop-client/ironclaw/src/tools/wasm/](../../../desktop-client/ironclaw/src/tools/wasm/) 上做反向引用统计，
得到下述事实（统计时间：F4.5.1 合并后）：

### 2.1 `tools/wasm` 是 wasm 基础设施层

- 路径：[desktop-client/ironclaw/src/tools/wasm/](../../../desktop-client/ironclaw/src/tools/wasm/)
- 体量：13 个 `.rs` 文件，**10,520 行**
- 文件清单：
  `allowlist.rs / capabilities.rs / capabilities_schema.rs / credential_injector.rs /
  error.rs / host.rs / http_security.rs / limits.rs / loader.rs / mod.rs /
  rate_limiter.rs / runtime.rs / storage.rs / wrapper.rs`
- 暴露的类型/函数（被 `channels/wasm` 直接引用）：
  `FuelConfig`、`ResourceLimits`、`HostState`、`LogLevel`、`Capabilities`、
  `RateLimitConfig`、`CapabilitiesFile`、`RateLimitSchema`、`WorkspaceReader`、
  `WorkspaceCapability`、`WasmResourceLimiter`、`credential_injector::*`、
  `storage::{compute_binary_hash, verify_binary_integrity}`。
- 在 ironclaw 全树中代表"wasm 沙箱的 host 端实现"——
  把 wasmtime engine、capabilities schema、限流、凭证注入、二进制校验
  这些**所有 wasm 调用方都要复用**的能力收在一起。

### 2.2 `channels/wasm` 是 wasm 的 channel 形态扩展

- 路径：[desktop-client/ironclaw/src/channels/wasm/](../../../desktop-client/ironclaw/src/channels/wasm/)
- 体量：15 个 `.rs` 文件，**12,925 行**
- 文件清单：
  `bundled.rs / capabilities.rs / error.rs / host.rs / loader.rs / mod.rs /
  router.rs / runtime.rs / runtime_config_keys.rs / schema.rs / setup.rs /
  signature.rs / storage.rs / telegram_host_config.rs / wrapper.rs`
- 反向引用：在 channels/wasm 内部出现 **17 处** `use crate::tools::wasm::…`
  （loader/router/wrapper/host/runtime/capabilities/schema/storage 全部命中），
  channels/wasm 是 tools/wasm 的下游消费者，**没有反向依赖**（tools/wasm
  内部完全不 `use crate::channels::…`）。

### 2.3 结论：是分层，不是平行实现

之前对话中我曾粗略说过"两个并行的 wasm 实现"。这次 `grep_search` 三层核查
（含 `includeIgnoredFiles=true`）证实**不是**：

```
tools/wasm   ← wasm host 基础设施（runtime/capabilities/限流/凭证/校验）
   ▲
   │ use crate::tools::wasm::*    (17 处)
   │
channels/wasm ← 在基础设施上叠的 channel 形态（telegram/bundled/router/setup …）
```

因此**任何"抽个新的 dasclaw_wasm_sandbox 基础 crate"**之类的提案
都属于发明新结构，违反 ADR-129 §1.3 verbatim port 红线，**正式作废**。

## 3. 推迟 wasm 切片的真正原因

直接把 `channels/wasm` 搬进 `dasclaw_channels` 在 F4.5.1 阶段做不了，因为：

1. `dasclaw_channels` 不依赖 `tools/wasm`；如果硬把 channels/wasm 搬过去，
   那 17 处 `use crate::tools::wasm::*` 会全部断链，等同于一次 17 处的非 verbatim
   改写。
2. `tools/wasm` 本身依赖几个 desktop-only 模块：

   | 反向引用 | 模块路径 |
   | --- | --- |
   | `crate::tools::registry::{ToolRegistry, WasmRegistrationError, WasmToolRegistration}` | `tools/registry`（desktop） |
   | `crate::llm::recording::{HttpExchangeRequest, HttpExchangeResponse, HttpInterceptor}` | `llm/recording`（desktop） |
   | `crate::safety::LeakDetector` | `safety`（desktop） |
   | `crate::tools::tool::{Tool, ToolError, ToolOutput}` | `tools/tool`（已部分外抽，需对齐） |
   | `crate::cli::oauth_defaults` | `cli`（desktop） |

   即"把 `tools/wasm` 升级成 crate"在做之前要先把上面这 5 个 desktop 依赖整理掉
   （extract 或 verbatim 搬入新 crate），否则就是补丁式硬塞。

3. 桌面端 wasm 调用方除了 channels/wasm 之外还有 `tools/builtin` 中通过
   `ToolRegistry::register_wasm_from_storage` 注册的第三方/LLM 生成工具——
   这条链路也要在搬迁中一起对齐，不能只看 channels 一侧。

## 4. 决策

把 ADR-152 §3 F4.5 余下的 wasm 部分拆成 **F4.5.2（前置整理）**、
**F4.5.3（搬 tools/wasm）**、**F4.5.4（搬 channels/wasm）** 三段，每段独立 PR、
独立可 revert，全部按 ADR-129 §1.3 verbatim。

### 4.1 F4.5.2 — 前置整理（multiple sub-PR）

把 `tools/wasm` 当前依赖的 5 个 desktop 模块按 verbatim port 规则一个一个抽
（或对齐 trait），完成后 `tools/wasm` 的所有外部 import 都应在 `crate::*` 之外。
**每个 sub-PR 独立交付、独立测试、独立 ADR drift 守卫**。

| sub-PR | 内容 | 体量预估 | 依赖 |
| --- | --- | --- | --- |
| F4.5.2.a | 抽 `crate::safety::LeakDetector` → 新 crate `dasclaw_secret_scan`（暂名）。`LeakDetector` 单一关注点（密钥泄露扫描），下游被 tools/wasm + llm/recording 共用。 | < 1k LoC，纯 verbatim mv | 无 |
| F4.5.2.b | 抽 `crate::llm::recording::HttpInterceptor`（含 `HttpExchangeRequest/Response`）→ 新 crate `dasclaw_http_recording`（暂名）。给 tools/wasm 与 llm 共用。 | 中等，依赖 4.5.2.a 完成（recording 内部用 LeakDetector）| F4.5.2.a |
| F4.5.2.c | 抽 `crate::cli::oauth_defaults` → 落到既有 `dasclaw_identity` 或新建小 crate（待该 sub-PR 论证）。 | < 500 LoC | 无 |
| F4.5.2.d | 把 `crate::tools::tool::{Tool, ToolError, ToolOutput}` 与 `crate::tools::registry::*` 与既有 `dasclaw_tool` 对齐，并梳理 `ToolRegistry` 的循环依赖（如真存在）。 | 中等，可能涉及二次 verbatim 校验 | F4.5.2.a-c |

> **注**：F4.5.2.a/b/c/d 任意 sub-PR 在自家实际打开 grep 后发现"已经在
> `dasclaw_*` 里有等价实现"，必须三层验证（semantic_search → vscode_listCodeUsages
> → rg）后采用"重用而非新建"路径，不再开新 crate（AGENTS.md §6 复用优先于新建）。

### 4.2 F4.5.3 — `tools/wasm` → `dasclaw_wasm_tools`

前置：F4.5.2.a–d 全部 merge。

- `git mv desktop-client/ironclaw/src/tools/wasm crates/dasclaw_wasm_tools/src/...`
- `crates/dasclaw_wasm_tools/Cargo.toml` 用 F4.2.0（routines）/F4.5.1（channels）已经验证过的模板：`version = "0.0.0-w1"`、`publish = false`、`edition = "2024"`。
- 桌面端 `src/tools/wasm.rs` 改写为
  `pub use dasclaw_wasm_tools::*;` 一行薄壳（同 F4.5.1 留 channels 薄壳的做法），
  让 channels/wasm 与 tools/builtin 内现存所有 `use crate::tools::wasm::*` 调用点**零改动**。
- 验证：crate 级 `cargo check -p dasclaw_wasm_tools --tests` +
  `cargo nextest run -p dasclaw_wasm_tools` + crate 级 clippy `-D warnings` +
  桌面级 `cargo check -p dasclaw` 全绿。

### 4.3 F4.5.4 — `channels/wasm` → 进 `dasclaw_channels`

前置：F4.5.3 merge。

- `git mv desktop-client/ironclaw/src/channels/wasm crates/dasclaw_channels/src/wasm/...`
- 在 `dasclaw_channels` 的 `Cargo.toml` 增 `dasclaw_wasm_tools = { path = "...", version = "0.0.0-w1" }`。
- 内部 `use crate::tools::wasm::*` 改写为 `use dasclaw_wasm_tools::*;`
  ——这 17 处属于 ADR-129 §1.3.1 表中 **"use 路径"行**的可 fork 化范围
  （跨 crate 自指改写，不可改导入项数量与名称），不构成 verbatim 违规。
- `desktop-client/ironclaw/src/channels/mod.rs` 中 `pub mod wasm;` 改成
  `pub use dasclaw_channels::wasm;`（与 F4.5.1 留下的薄壳模式一致）。
- 评估 `setup.rs / router.rs / telegram_host_config.rs` 这几个文件
  里**与 desktop UI/设置向导耦合的部分**是否要留在桌面端。
  若耦合明显，按 ADR-129 §1.3 拆出最小 verbatim 单元留 desktop，channels/wasm
  里仅保留 channel 抽象本身。该评估在 F4.5.4 PR body 中给出，**不在本 ADR 预设**。

## 5. 非目标（What's NOT in this ADR）

- **不**升级 wasmtime / wasmtime-wasi 版本。当前 `RUSTSEC-2026-0149` 等
  Wasmtime cluster advisory 由 `deny.toml [advisories].ignore` 维护
  （含 F4.5.1 时新增的 RUSTSEC-2026-0149，由 PR #735 落地），
  待 F4.5.3 完成、`tools/wasm` 真正成 crate 后再做 wasmtime bump。
- **不**重写任何 wasm 业务逻辑（host.rs 的 wasmtime 调用、capabilities schema、
  限流、credential_injector、storage 校验都按 verbatim port 红线照搬）。
- **不**新增任何抽象层（不引入"wasm 基础 crate"、不引入新 trait、
  不合并/拆分 ironclaw-main 上游已有的 src 文件）。
- **不**改变 builtin tools（21.5 K LoC、28 工具）的搬迁顺序。
  那些是**纯 native Rust**（`tokio::process::Command` / `reqwest::Client` / `std::fs`），
  与 wasm 沙箱无关，按 ADR-152 §3 F4.6 单独走。

## 6. 风险与回滚

| 风险 | 处理 |
| --- | --- |
| F4.5.2.a–d 任一前置抽取在跨平台/admin-backend/scanner 里命中未知反向依赖 | 该 sub-PR 单独 revert 即可，不影响 F4.5.3/4.5.4。每个 sub-PR 必须含自身的 ADR drift 守卫与回滚步骤段。 |
| `tools/wasm` 中 `wasmtime` 全家桶的 feature 列表与 desktop 现有 lockfile 不一致 | 新 crate 的 Cargo.toml 中 wasmtime 依赖项 1:1 复刻 desktop 当前的 dependency 行；不能"顺手"开关 features。 |
| 17 处 `use crate::tools::wasm::*` 在 F4.5.4 改成 `use dasclaw_wasm_tools::*` 时遗漏一两处 | 走 `cargo check -p dasclaw_channels` + `cargo check -p dasclaw` 双门，加 `python3.12 scripts/check_no_panics.py --base origin/xClaw`。 |
| ironclaw-main 上游有新的 wasm patch 没 cherry-pick 进来 | 每个 sub-PR 开始前先 `cargo nextest run -p dasclaw_wasm_tools`（搬完之后）对照上游测试预期；不在本 ADR 解决，归属上游 sync 流程。 |

## 7. 引用

- 主 ADR：[adr-152-agent-and-capability-fusion.md](adr-152-agent-and-capability-fusion.md) §3 F4.5（搬迁清单原文）
- Verbatim 红线：[adr-129-sandbox-windows-windows-crate-adoption.md](adr-129-sandbox-windows-windows-crate-adoption.md) §1.3 与 §1.3.1（fork 化边界表）
- 工程规约：[AGENTS.md](../../../AGENTS.md) §6"复用优先于新建"、Skills 强制使用规范
- F4.5.1 落地参考：PR #733（已合并）— `crates/dasclaw_channels` 的 lib.rs、`channel.rs/manager.rs/relay/*.rs` 与桌面 `channels/mod.rs` 薄壳
- 已落地相关基础修复：PR #735（`deny.toml` 追加 RUSTSEC-2026-0149）
