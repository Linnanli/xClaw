# F4.2 前置依赖评估：routines 物理搬迁可行性分析

**状态**：评估中，待人工决策
**关联 ADR**：[`adr-152-agent-and-capability-fusion.md`](adr-152-agent-and-capability-fusion.md) §3 F4.2 / §11.9
**关联红线**：[`adr-129`](adr-129-verbatim-migration-principle.md) §1.3 verbatim

## 1. 问题陈述

ADR-152 §3 F4.2 条款：

> **F4.2**：`routines` 实搬——填充 `dasclaw_routines`（当前为 W1 空壳，24 行 trait skeleton），把 desktop `routines/` 的 8.7K LoC verbatim 搬入。

执行前依赖图扫描发现：desktop `routines/` 7 个源文件、8693 LoC 共引用 **14 个仍在桌面端的本地模块**，物理搬到 `crates/dasclaw_routines` 后会形成 **底层 crate → 顶层 desktop** 反向依赖，与 §11.9 F4.0 修复同类问题再现。

## 2. 依赖矩阵

| desktop 模块 | routines 中引用数 | 当前位置 | 是否已 crate 化 |
| --- | --- | --- | --- |
| `crate::llm` | 20 | `desktop-client/ironclaw/src/llm/` | ✗ |
| `crate::tools` | 19 | `desktop-client/ironclaw/src/tools/` | ✗（F4.6+ 范围） |
| `crate::error` | 12 | `desktop-client/ironclaw/src/error.rs` | ✗ |
| `crate::channels` | 9 | `desktop-client/ironclaw/src/channels/` | ✗（F4.5 范围） |
| `crate::workspace` | 6 | `desktop-client/ironclaw/src/workspace/` | ✗ |
| `crate::tenant` | 5 | `desktop-client/ironclaw/src/tenant/` | ✗ |
| `crate::timezone` | 5 | `desktop-client/ironclaw/src/timezone.rs` | ✗ |
| `crate::agent` | 4 | `desktop-client/ironclaw/src/agent/` | ✗ |
| `crate::config` | 4 | `desktop-client/ironclaw/src/config/` | ✗ |
| `crate::safety` | 4 | `desktop-client/ironclaw/src/safety.rs`（薄壳→`dasclaw_safety`） | 部分（F4.0 已搬 crate，desktop 仍有薄壳） |
| `crate::worker` | 3 | `desktop-client/ironclaw/src/worker/` | ✗ |
| `crate::extensions` | 2 | `desktop-client/ironclaw/src/extensions/` | ✗ |
| `crate::util` | 2 | `desktop-client/ironclaw/src/util.rs` | ✗ |
| `crate::testing` | 1 | `desktop-client/ironclaw/src/testing.rs` | ✗ |

**合计**：96 处跨模块引用，14 个未下沉模块。

## 3. 反向依赖风险评估

把 routines 物理搬到 `crates/dasclaw_routines` 后，必须二选一：

- **方案 X（反向 path）**：`crates/dasclaw_routines/Cargo.toml` 加 `dasclaw = { path = "../../desktop-client/ironclaw" }` —— **违反 §11.9 原则**，与 F4.0 修复方向完全相反，CI 红线 `ADR-129 verbatim drift` 会触发。
- **方案 Y（trait 抽象 + 注入）**：对 14 个 desktop 模块每个抽 trait + 由 desktop 实现，routines 通过 trait 注入访问 —— **违反 §1.3 verbatim**，是逻辑改造而非物理搬迁。

两者均不可行 → ADR-152 §3 F4.2 在当前 F4 顺序下 **不能直接执行**。

## 4. 可行路径候选

### 候选 A：调整 F4 顺序

把 F4.2 拆为 F4.2a / F4.2b，前置 F4.3-F4.6 先行：

1. F4.3 `secrets` 桌面薄壳清理（独立，60 处 import）
2. F4.4 `orchestrator` 落点决策 + 搬迁
3. F4.5 `channels` 拆分（trait + relay + wasm + manager 进 crate）
4. F4.6+ `tools/builtin` 分批搬（21.5K LoC、28 工具）
5. 同时 `llm` / `agent::task` / `worker` / `extensions` / `error` / `workspace` / `tenant` / `config` / `util` / `timezone` 各自抽 crate（每个需单独 ADR 章节）
6. 全部完成后再做 F4.2 verbatim 搬迁

代价：F4.2 排到 F4 最后；新增 ~10 个子 ADR 章节。

### 候选 B：F4.2 改语义为「准备性 routines crate 接口锁定」

不动 desktop `routines/` 物理位置，仅在 `crates/dasclaw_routines` 内：

1. 把 W1 trait skeleton 扩成 routines 公共接口（基于 desktop 现有公共 API 反向抽取）
2. desktop `routines/` 改为 `impl dasclaw_routines::*` 风格的实现
3. 调用方（如 `channels`、`scheduler`）通过 `dasclaw_routines` trait 访问

代价：需写新 trait surface 设计（违反 verbatim）；ADR-152 §3 F4.2 条款必须修订。

### 候选 C：维持 ADR-152 §3 文本不动，把 F4.2 标为"暂缓至 F4.6 之后"

在 ADR-152 §11 加附录章节，正式声明 F4.2 的前置依赖未满足，推迟到所有依赖模块完成 crate 化之后。然后立刻推进 F4.3。

代价：ADR-152 修订；F4.2 不在本轮做。

## 5. 建议

**候选 C** 在三者中：

- 维持 ADR-129 §1.3 verbatim 红线
- 维持 §11.9 依赖方向原则
- 单一 ADR 修订（vs 候选 A 的 10+ 子 ADR）
- 路径与 F4.0 的"原"保留代码不动"条款作废"修订风格一致

建议立即推进 **F4.3 `secrets` 桌面薄壳清理**，该 PR 与 F4.2 无依赖关系，且已在 ADR-152 §3 明确范围（60 处 import 改写，与 F4.1 同质）。

## 6. 不在本评估范围

- 各 desktop 模块如何 crate 化（候选 A 路径）的具体设计
- routines trait surface 抽取（候选 B 路径）的 API 设计
- ADR-152 §11 附录修订文本（待决策后单独 PR）

## 7. Sources read

- [`adr-152-agent-and-capability-fusion.md`](adr-152-agent-and-capability-fusion.md) §3 / §11.9
- [`adr-129-verbatim-migration-principle.md`](adr-129-verbatim-migration-principle.md) §1.3
- desktop-client/ironclaw/src/routines/*.rs 8693 LoC 实际依赖扫描
