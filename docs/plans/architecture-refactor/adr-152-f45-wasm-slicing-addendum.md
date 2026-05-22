# ADR-152 F4.5 wasm 切片 —— 勘误与修订（Addendum）

> 配套文档：[`adr-152-f45-wasm-slicing.md`](adr-152-f45-wasm-slicing.md)
> 主 ADR：[`adr-152-agent-and-capability-fusion.md`](adr-152-agent-and-capability-fusion.md) §3 F4.5
> Verbatim 红线：[`adr-129-sandbox-windows-windows-crate-adoption.md`](adr-129-sandbox-windows-windows-crate-adoption.md) §1.3 / §1.3.1
> 复用红线：[`AGENTS.md`](../../../AGENTS.md) §6

## 1. 起因

前序 ADR `adr-152-f45-wasm-slicing.md`（PR #737 已合并）在 §3 提出了
**F4.5.2 四项前置抽取**：

| 子项 | 原 ADR 提案 |
| --- | --- |
| F4.5.2.a | 抽 `crate::safety::LeakDetector` → 新 crate `dasclaw_secret_scan` |
| F4.5.2.b | 抽 `crate::llm::recording::HttpInterceptor` → 新 crate `dasclaw_http_recording` |
| F4.5.2.c | 抽 `crate::cli::oauth_defaults` → 已有或新 crate |
| F4.5.2.d | 让 `crate::tools::tool` / `crate::tools::registry` 与 `dasclaw_tool` 对齐 |

在动手实现 F4.5.2.a 之前，按 [`AGENTS.md`](../../../AGENTS.md) §6 做三层
否定性结论验证（semantic_search → vscode_listCodeUsages → rg），**结论与
原 ADR 相反**：四项前置抽取**全部已经做完**。

继续按原 ADR 写代码会产生与 `dasclaw_safety` / `dasclaw_runtime` /
`dasclaw_llm_provider` / `dasclaw_tool` 重复的 crate，**直接踩 ADR-129
§1.3 verbatim 红线 + AGENTS §6 复用红线**。本 addendum 记录核查证据并
重新规划。

## 2. 核查证据

### 2.1 F4.5.2.a —— `safety::LeakDetector`

- 实际定义位置：[`crates/dasclaw_safety/src/leak_detector.rs`](../../../crates/dasclaw_safety/src/leak_detector.rs#L132)
  （`pub struct LeakDetector`）
- desktop 端 [`desktop-client/ironclaw/src/safety/mod.rs`](../../../desktop-client/ironclaw/src/safety/mod.rs#L13)
  仅 13 行，核心是 `pub use dasclaw_safety::*;`，就是一个再导出薄壳。
- 结论：**没有"抽 LeakDetector 到新 crate"这件事可做** —— 已经在
  `dasclaw_safety` 里。

### 2.2 F4.5.2.b —— `HttpInterceptor` / `HttpExchange*`

- 实际定义位置：[`crates/dasclaw_runtime/src/recording.rs`](../../../crates/dasclaw_runtime/src/recording.rs#L47)
  （`pub trait HttpInterceptor`）+
  [`crates/dasclaw_runtime/src/lib.rs`](../../../crates/dasclaw_runtime/src/lib.rs#L50)
  （`pub use recording::{HttpExchange, HttpExchangeRequest, HttpExchangeResponse, HttpInterceptor};`）
- `recording.rs` 文件首部注释明确写："These types and the
  `HttpInterceptor` trait were verbatim-ported from … only the trait
  stays here so other crates can pass `Option<Arc<dyn HttpInterceptor>>`
  without dragging ironclaw down with them."
- 结论：**已经做完**，连搬迁理由都写在文件头了。

### 2.3 F4.5.2.c —— `cli::oauth_defaults`

- 实际定义位置：[`crates/dasclaw_llm_provider/src/provider/oauth_helpers.rs`](../../../crates/dasclaw_llm_provider/src/provider/oauth_helpers.rs#L4)
  文件首注释："The constants and helper functions here were originally
  in `cli/oauth_defaults.rs` and are moved here so the `llm` module is
  self-contained. `cli/oauth_defaults` re-exports everything for
  backward compatibility."
- 结论：**已经做完**，desktop 端 `cli/oauth_defaults` 现在是再导出薄壳。

### 2.4 F4.5.2.d —— `tools::tool` / `tools::registry` 与 `dasclaw_tool` 对齐

- [`desktop-client/ironclaw/src/tools/tool.rs`](../../../desktop-client/ironclaw/src/tools/tool.rs#L17-L23)：
  ```rust
  pub use dasclaw_runtime::Tool;
  pub use dasclaw_tool::{
      ApprovalContext, ApprovalRequirement, RiskLevel, ToolDiscoverySummary, ToolDomain,
      ToolError, ToolOutput, ToolRateLimitConfig, ToolSchema, ...
  };
  ```
- 文件头注释引用了 ADR-154 §3.1 修订：`Tool` trait 物理位置在
  `dasclaw_runtime`，纯数据类型在 `dasclaw_tool`。
- 结论：**已经做完**。`ToolRegistry` 留在 desktop 是有意为之（它本身耦合
  Database / ExtensionManager / orchestrator / skills / builtins，属于
  desktop-only 整合层）。

## 3. 重新评估 `tools/wasm` 的真实外部依赖

把前序 ADR §3 的 5 条外部依赖逐条对照"已落地的 dasclaw_* crate"：

| `tools/wasm` 内的 `use crate::...` | 真实可达替代 | 状态 |
| --- | --- | --- |
| `crate::llm::recording::{HttpExchangeRequest, HttpExchangeResponse, HttpInterceptor}` | `dasclaw_runtime::recording::*` | ✅ 直接可换 |
| `crate::safety::LeakDetector` | `dasclaw_safety::LeakDetector` | ✅ 直接可换 |
| `crate::tools::tool::{Tool, ToolError, ToolOutput}` | `dasclaw_runtime::Tool` + `dasclaw_tool::{ToolError, ToolOutput}` | ✅ 直接可换 |
| `crate::cli::oauth_defaults` | `dasclaw_llm_provider::provider::oauth_helpers` | ✅ 直接可换 |
| `crate::tools::registry::{ToolRegistry, WasmRegistrationError, WasmToolRegistration}` | **无等价 crate**；`ToolRegistry` 重度耦合 desktop | ❌ 仅 `loader.rs` 使用 |

唯一真实阻塞 = `tools/wasm/loader.rs` 通过 `ToolRegistry` 把 wasm 工具
注册到 desktop 的工具表里。这件事**不能也不该**搬到独立 crate，因为
`ToolRegistry` 自己依赖 `db / extensions / orchestrator / skills /
builtins`（[`desktop-client/ironclaw/src/tools/registry.rs`](../../../desktop-client/ironclaw/src/tools/registry.rs#L12-L34)）。

## 4. 修订后的方案

### 4.1 F4.5.2 取消

四个子项**全部标记为 N/A（已落地）**，不会有 sub-PR。

### 4.2 F4.5.3（修订）—— `tools/wasm` 切片，`loader.rs` 留 desktop

| 旧提案 | 修订提案 |
| --- | --- |
| `git mv tools/wasm → crates/dasclaw_wasm_tools`，desktop 留 `pub use` 薄壳 | `git mv tools/wasm 除 loader.rs 外的 12 个文件 → crates/dasclaw_wasm_tools`，`loader.rs` 留在 desktop 并把 `use crate::tools::wasm::*` 改成 `use dasclaw_wasm_tools::*`；desktop 仍提供 `pub use dasclaw_wasm_tools::*;` 薄壳 |

`tools/wasm` 现在 13 个文件，把以下 12 个搬到新 crate（verbatim，每个
文件内部的 `use crate::tools::wasm::xxx` → `use crate::xxx`、
`use crate::safety::LeakDetector` → `use dasclaw_safety::LeakDetector`
等改写归 ADR-129 §1.3.1"use 路径"可 fork 行）：

- `allowlist.rs`
- `capabilities.rs`
- `capabilities_schema.rs`
- `credential_injector.rs`
- `error.rs`
- `host.rs`
- `limits.rs`
- `runtime.rs`
- `storage.rs`
- `wrapper.rs`
- `mod.rs`（按需收缩）
- 任何当前未列举的子文件（实施时 `ls` 确认）

留在 desktop 的：
- `loader.rs`（保持 `ToolRegistry` 注册路径，依赖 `dasclaw_wasm_tools::*`）

### 4.3 F4.5.4（修订）—— `channels/wasm` 切片，`setup.rs` + `loader.rs` 留 desktop

[`desktop-client/ironclaw/src/channels/wasm/setup.rs`](../../../desktop-client/ironclaw/src/channels/wasm/setup.rs#L14-L17)
依赖 `Config / Database / ExtensionManager / PairingStore`，
[`desktop-client/ironclaw/src/channels/wasm/loader.rs`](../../../desktop-client/ironclaw/src/channels/wasm/loader.rs#L14-L21)
依赖 `bootstrap::dasclaw_base_dir / db::SettingsStore / pairing::PairingStore`。
这些是 desktop 整合层，不搬。

搬到 `crates/dasclaw_channels/src/wasm/` 的（15 文件中的 13 个）：
- `capabilities.rs`、`schema.rs`、`error.rs`、`runtime.rs`、`wrapper.rs`、
  `host.rs`、`router.rs`、`storage.rs`、`mod.rs`、以及其他纯 wasm 运行时
  / 协议适配文件（实施时 `ls` 确认）

需要改写的 `use` 路径（属 ADR-129 §1.3.1 可 fork 行）：
- `use crate::tools::wasm::*` → `use dasclaw_wasm_tools::*`（17 处）
- desktop-only 的 `use crate::config / db / extensions / pairing / bootstrap`
  这些**仅出现在 setup.rs + loader.rs 里**，本来就不搬。

留在 desktop 的：
- `setup.rs`
- `loader.rs`

desktop 仍以 `pub use dasclaw_channels::wasm::*;` 暴露符号。

## 5. 实施顺序

| Phase | 描述 | 依赖 |
| --- | --- | --- |
| **F4.5.3** | 新建 crate `dasclaw_wasm_tools`，搬 12 个 `tools/wasm` 文件；desktop 留 `loader.rs` + 再导出薄壳 | 无（所有 dasclaw_* 依赖都已就位） |
| **F4.5.4** | 把 `channels/wasm` 的 13 个文件搬到 `dasclaw_channels::wasm`；改写 17 处 `use`；desktop 留 `setup.rs` + `loader.rs` + 再导出薄壳 | F4.5.3 |

每段单独 PR、单独可 revert。

## 6. 非目标（保持不变）

- 不升 `wasmtime` / `wasmtime-wasi` 版本（RUSTSEC 集群按既定策略 ignore）
- 不改 wasm 业务逻辑
- 不新建额外抽象 crate（特别是不新建 `dasclaw_secret_scan` /
  `dasclaw_http_recording` —— 已被前述证据证伪）
- 不重写 `ToolRegistry`

## 7. 风险与回滚

| 风险 | 概率 | 应对 |
| --- | --- | --- |
| `dasclaw_wasm_tools` 与 `dasclaw_channels::wasm` 出现循环依赖 | 低 | 单向：channels::wasm → wasm_tools，反向 0 处（已 grep 验证） |
| `loader.rs` 留 desktop 导致编译错误 | 低 | 改写 5 行 `use`，CI 兜底 |
| `wrapper.rs` 内对 `cli::oauth_defaults` 的引用 | 低 | 改成 `use dasclaw_llm_provider::provider::oauth_helpers as oauth_defaults;`（属可 fork 行） |
| 17 处反向引用漏改 | 低 | 一次性 `sed` 改写 + `cargo check -p ironclaw` 兜底 |

每个 PR 单独 revert 即可；F4.5.4 revert 不影响 F4.5.3。

## 8. 与前序 ADR 的关系

本 addendum **不删改**前序 ADR 文本，只在其结论之上做修订：
- 前序 §3 F4.5.2 列表 → **本 addendum §4.1 取消**
- 前序 §3 F4.5.3 → **本 addendum §4.2 修订**（`loader.rs` 留 desktop）
- 前序 §3 F4.5.4 → **本 addendum §4.3 修订**（`setup.rs` + `loader.rs` 留 desktop）
- 前序 §4 五条 desktop-only 反向依赖表 → 真实结论是其中 4 条已不存在；
  唯一剩下的是 `ToolRegistry`，且按 §4.2 留 desktop 即可

阅读顺序：先看前序 ADR §1–§2 了解背景与分层证据，再直接跳到本
addendum §4 看真实方案。

## 9. 引用文件清单（实际打开过的）

- [`AGENTS.md`](../../../AGENTS.md) §6
- [`adr-152-f45-wasm-slicing.md`](adr-152-f45-wasm-slicing.md) 全文
- [`adr-152-agent-and-capability-fusion.md`](adr-152-agent-and-capability-fusion.md) §3 F4.5
- [`adr-129-sandbox-windows-windows-crate-adoption.md`](adr-129-sandbox-windows-windows-crate-adoption.md) §1.3 / §1.3.1
- [`crates/dasclaw_safety/src/leak_detector.rs`](../../../crates/dasclaw_safety/src/leak_detector.rs)
- [`crates/dasclaw_runtime/src/recording.rs`](../../../crates/dasclaw_runtime/src/recording.rs)
- [`crates/dasclaw_runtime/src/lib.rs`](../../../crates/dasclaw_runtime/src/lib.rs)
- [`crates/dasclaw_llm_provider/src/provider/oauth_helpers.rs`](../../../crates/dasclaw_llm_provider/src/provider/oauth_helpers.rs)
- [`desktop-client/ironclaw/src/safety/mod.rs`](../../../desktop-client/ironclaw/src/safety/mod.rs)
- [`desktop-client/ironclaw/src/tools/tool.rs`](../../../desktop-client/ironclaw/src/tools/tool.rs)
- [`desktop-client/ironclaw/src/tools/registry.rs`](../../../desktop-client/ironclaw/src/tools/registry.rs)
- [`desktop-client/ironclaw/src/tools/wasm/loader.rs`](../../../desktop-client/ironclaw/src/tools/wasm/loader.rs)
- [`desktop-client/ironclaw/src/tools/wasm/wrapper.rs`](../../../desktop-client/ironclaw/src/tools/wasm/wrapper.rs)
- [`desktop-client/ironclaw/src/channels/wasm/setup.rs`](../../../desktop-client/ironclaw/src/channels/wasm/setup.rs)
- [`desktop-client/ironclaw/src/channels/wasm/loader.rs`](../../../desktop-client/ironclaw/src/channels/wasm/loader.rs)
