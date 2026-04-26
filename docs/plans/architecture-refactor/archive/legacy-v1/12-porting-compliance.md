# 12 — 移植合规 Checklist (codex Apache-2.0 / claw-code MIT)

> **目的**: 路线 B 需要从 codex (Apache-2.0) 和 claw-code (MIT) 移植代码到 `dasclaw_*` 与 `ironclaw_*` crate。
> 本文档定义**强制合规红线**和**风险最小化操作**, 所有移植 PR 必须按此 checklist 自检。

---

## 1. 上游许可证速查

| 上游项目 | 许可证 | 关键义务 | NOTICE 必需? |
|---|---|---|---|
| **codex** (`codex-cli-main/`) | Apache License 2.0 | §4 全部 4 条 | ✅ |
| **claw-code** (`claw-code/`) | MIT | 保留版权 + LICENSE | ⚠️ 推荐 |
| **ironclaw** (本仓 `desktop-client/ironclaw/`) | 自有 | 内部代码无对外义务 | N/A |

> **核实证据**: `codex-cli-main/LICENSE` (Apache 2.0 v2 全文); `claw-code/rust/Cargo.toml:8` (`license = "MIT"`)

---

## 2. Apache 2.0 强制红线 (codex 移植)

### 红线 1: 保留 `LICENSE` 文件
- 每个包含 codex 派生代码的 crate 根目录必须存在 `LICENSE-APACHE` (复制自 `codex-cli-main/LICENSE`)
- workspace 根目录 `LICENSE-APACHE` 也保留
- 不得修改 LICENSE 文本一个字

### 红线 2: 保留 / 创建 `NOTICE` 文件
- 包含 codex 派生代码的 crate 根目录创建 `NOTICE` 文件, 模板见 §6
- 必须列出: 上游项目名 / 版权人 / 许可证 / 引用的 commit hash
- workspace 根目录 `NOTICE` 汇总所有派生来源

### 红线 3: 修改文件头标注
- 每个移植 / 改写自 codex 的 `.rs` 文件**头部**必须加注释 (模板见 §7)
- 标注内容: 来源仓库 + commit + 修改日期 + 修改概述
- "改写"也算 (即使重命名变量 / 调整结构), 法律保险 + 审计便利

### 红线 4: 不使用 codex / OpenAI 商标
- crate 名禁用 `codex` / `openai` 前缀 → 我们用 `dasclaw_*` 完全避开 ✅
- 产品名、CLI 名、配置文件名、log target 都不得含 `codex` / `OpenAI`
- 文档可写 "基于 OpenAI codex 派生" (这是事实陈述, 不是商标使用) ✅

---

## 3. MIT 强制红线 (claw-code 移植)

### 红线 1: 保留版权声明
- 移植自 claw-code 的文件**头部**保留原 MIT 版权 + 许可声明 (如有)
- claw-code 当前文件头**无**显式 copyright (已核实), 在 NOTICE 集中声明即可

### 红线 2: 分发时附 LICENSE
- 包含 claw-code 派生代码的 crate 根附 `LICENSE-MIT` (复制 `claw-code/LICENSE`)
- workspace 根目录附 `LICENSE-MIT`

### 与 Apache 双许可的兼容性
- `dasclaw_*` crate 若同时含 codex (Apache-2.0) 和 claw-code (MIT) 派生代码 → 在 `Cargo.toml` 声明:
  ```toml
  license = "Apache-2.0 OR MIT"
  ```
- 这是 Rust 生态约定, 兼容两个上游

---

## 4. 触发条件: 何时必须合规

| 行为 | 是否触发义务 | 理由 |
|---|---|---|
| **复制 codex 源码任何片段** (函数 / 结构体 / 模块) | ✅ 触发 | Apache §1 "Derivative Work" |
| **改名 / 重命名 / 微调结构后使用** | ✅ 触发 | 仍是 Derivative Work |
| **完全重写, 仅参考思路 / 算法** | ⚠️ 法律不强制, **实务建议署名** | 避免后续争议, 成本极低 |
| 仅阅读上游代码学习概念 | ❌ 不触发 | 思想不受版权保护 |
| **桌面客户端二进制分发** (desktop-client) | ✅ 触发 §4 全部 | Apache 覆盖 Object form |
| 内部测试 / 私有部署 (零分发) | ❌ §4 不触发, NOTICE 仍建议 | 留底防未来分发 |
| SaaS / 云端不分发二进制 | ⚠️ Apache 不强制 (与 AGPL 不同), 仍建议 NOTICE | 业界惯例 + 声誉 |

> **x-claw 现状**: 桌面客户端通过 Tauri 分发 → **触发 Apache §4 全部 4 条**, 必须严格合规。

---

## 5. "借鉴修改" 灰色地带操作建议

法律上"借鉴思路 + 完全重写"不构成 Derivative Work, 但实务建议:

1. ✅ **commit message 注明**: `feat(kernel): port AgentRegistry from codex (inspired-by)`
2. ✅ **内部 `PORTING_LOG.md`** 记录: 来源 / 改动量 / 改写原因 (内部审计用, 不公开)
3. ❌ **禁止在公开 commit / issue 写**: "reverse-engineered" / "extracted from leaked code" — 留下"明知侵权"证据
4. ✅ **README 主动致谢**: "本项目 agent 内核基于 OpenAI codex (Apache-2.0) 派生" — 业界加分行为

---

## 6. NOTICE 文件模板

### Workspace 根 `NOTICE`

```text
x-claw
Copyright 2025 <Your Organization>

This product includes software developed by:

* OpenAI (https://github.com/openai/codex)
  Licensed under the Apache License, Version 2.0
  Portions of `dasclaw_agent_kernel`, `dasclaw_session`, `dasclaw_tasks`,
  `dasclaw_context_mgr`, `dasclaw_mailbox`, `dasclaw_agent_registry`
  are derived from openai/codex commit <COMMIT_HASH> (snapshot YYYY-MM-DD).

* claw-code (https://github.com/<owner>/claw-code)
  Licensed under the MIT License
  Portions of `dasclaw_apply_patch`, `dasclaw_branch_guard`,
  `dasclaw_rollout_trace`, `dasclaw_device_identity`
  are derived from claw-code commit <COMMIT_HASH> (snapshot YYYY-MM-DD).

* ironclaw (internal)
  Original work of <Your Organization>.

Full license texts available at:
  - LICENSE-APACHE
  - LICENSE-MIT
```

### Crate 级 `NOTICE` (示例: `dasclaw_agent_kernel/NOTICE`)

```text
dasclaw_agent_kernel
Copyright 2025 <Your Organization>

This crate contains code derived from:

OpenAI codex (https://github.com/openai/codex)
Copyright 2025 OpenAI
Licensed under the Apache License, Version 2.0

Derived files (see file headers for per-file modification notes):
  - src/agentic_loop.rs (from codex-rs/core/src/agent/agentic_loop.rs)
  - src/intent.rs       (from codex-rs/core/src/agent/intent.rs)
  - src/reasoning_ctx.rs (from codex-rs/core/src/agent/reasoning_ctx.rs)

See LICENSE-APACHE for full license text.
```

---

## 7. 移植文件头模板

### 完整移植 (90%+ 直接复制)

```rust
// SPDX-License-Identifier: Apache-2.0
//
// Derived from openai/codex commit <HASH>
//   path: codex-rs/core/src/agent/agentic_loop.rs
// Copyright 2025 OpenAI
//
// Modifications by x-claw 2025-MM-DD:
//   - Renamed `CodexAgent` → `DasclawAgentKernel`
//   - Removed Guardian-specific approval branches
//   - Added trait-based ToolRuntime injection point
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//   http://www.apache.org/licenses/LICENSE-2.0

use std::collections::HashMap;
// ... rest of file
```

### 部分移植 (<50% 来源, 大量原创)

```rust
// SPDX-License-Identifier: Apache-2.0
//
// Portions derived from openai/codex commit <HASH>
//   path: codex-rs/core/src/session/handlers.rs (function `handle_user_input` only)
// Copyright 2025 OpenAI, Apache-2.0
//
// Original code Copyright 2025 <Your Organization>
//
// Modifications and additions by x-claw 2025-MM-DD:
//   - Replaced session storage layer with SessionStore trait
//   - Added skill injection hook
//   - Removed CLI-specific handlers

use std::sync::Arc;
// ... rest of file
```

### 仅借鉴思路 (零代码复制)

```rust
// SPDX-License-Identifier: Apache-2.0
//
// Conceptually inspired by openai/codex `agent/registry.rs` design,
// but independently implemented.
// No code copied from upstream.

use std::collections::HashMap;
// ... rest of file
```

---

## 8. 字符串常量改写 Checklist (防指纹)

> **风险**: 上游的 prompt / error message / log 字符串原封不动嵌入二进制 → `strings` 命令扫描即可识别派生关系。

### 必须改写 (Source of binary fingerprints)

- [ ] System prompts (移植自 codex 的所有 system prompt 文本)
- [ ] User-facing error messages (英文 / 中文)
- [ ] Log target 字符串 (`tracing::info!(target = "codex...")` → `target = "dasclaw..."`)
- [ ] CLI help text / man page
- [ ] HTTP user-agent (避免 `User-Agent: codex/x.y`)
- [ ] 工具描述 markdown (tool description JSON 中的 description 字段)
- [ ] 配置文件 schema 字段名 (避免一字不差)

### 可保留 (公共领域 / 行业惯例)

- ✅ 错误码常量 (HTTP 4xx/5xx, OS errno)
- ✅ 标准协议字段 (OpenAI API schema, MCP protocol)
- ✅ 数学 / 算法常量

---

## 9. Cargo.toml 许可声明

### 仅含 codex 派生代码

```toml
[package]
name = "dasclaw_agent_kernel"
version = "0.1.0"
license = "Apache-2.0"
authors = ["x-claw <opensource@xclaw.example>"]
```

### 含 codex + claw-code 派生代码

```toml
license = "Apache-2.0 OR MIT"
```

### 仅含 claw-code 派生代码

```toml
license = "MIT"
```

### 仅含 ironclaw 原创 (无派生)

```toml
license = "<your private license>"  # 或商用许可
```

---

## 10. 公开物料致谢模板

### README.md 顶部

```markdown
# x-claw

Enterprise AI desktop assistant.

## Acknowledgements

x-claw is built upon several open source projects:

- [OpenAI codex](https://github.com/openai/codex) (Apache-2.0) — provides
  the foundation of our agent kernel (`dasclaw_agent_kernel`,
  `dasclaw_session`, `dasclaw_tasks`, `dasclaw_context_mgr`).
- [claw-code](https://github.com/<owner>/claw-code) (MIT) — provides the
  patch / governance utilities (`dasclaw_apply_patch`, `dasclaw_branch_guard`).
- ironclaw (internal) — orchestration layer, channels, skills, routines,
  and enterprise integrations.

See [NOTICE](./NOTICE) and [LICENSE-APACHE](./LICENSE-APACHE) /
[LICENSE-MIT](./LICENSE-MIT) for full attribution.
```

### About 页 / 关于界面 (Tauri UI)

桌面客户端必须可访问的"开源致谢"界面, 列出:
- 派生上游 + 许可证
- 完整 LICENSE 全文 (可链接)
- NOTICE 全文

---

## 11. PR 提交前 Checklist (强制)

每个移植 / 改写自 codex 或 claw-code 的 PR, 提交前必须勾选:

### Apache 2.0 (codex 派生)
- [ ] 文件头加 §7 模板注释 (含 SPDX / 来源 commit / 修改日期 / 修改概述)
- [ ] crate `Cargo.toml` 含 `license = "Apache-2.0"` 或 `"Apache-2.0 OR MIT"`
- [ ] crate 根目录有 `LICENSE-APACHE`
- [ ] crate 根目录 `NOTICE` 包含本派生条目
- [ ] workspace 根 `NOTICE` 汇总更新
- [ ] crate 名不含 `codex` / `openai`
- [ ] log target / user-agent 不含 `codex` / `openai`
- [ ] 移植的 prompt / error 字符串已改写 (§8)

### MIT (claw-code 派生)
- [ ] 文件头加 §7 模板注释
- [ ] crate `Cargo.toml` 含 `license = "MIT"` 或 `"Apache-2.0 OR MIT"`
- [ ] crate 根目录有 `LICENSE-MIT`
- [ ] crate 根目录 / workspace `NOTICE` 包含本条目

### 借鉴 (无代码复制)
- [ ] 文件头加 §7 第 3 种模板 ("Conceptually inspired by ... No code copied")
- [ ] commit message 标注 `(inspired-by codex/claw-code)`
- [ ] `PORTING_LOG.md` 记录借鉴源 + 改写原因

---

## 12. 风险矩阵

| 风险 | 概率 | 后果 | 缓解 |
|---|---|---|---|
| 二进制字符串指纹被逆向 | 高 | 中 (社区曝光) | §8 字符串改写 |
| 法律诉讼 (Apache §4 违约) | 低 | 高 (停产 + 赔偿) | §11 PR checklist 严格执行 |
| 商标侵权 (使用 OpenAI/codex 名) | 低 | 中 (律师函) | crate 名 / 产品名避开 |
| 媒体 / X 平台扒皮曝光 | 中 | 高 (品牌损伤) | 主动 README 致谢 + 完整 NOTICE |
| 员工跳槽泄露派生事实 | 中 | 低 (本来就该公开) | 内部 onboarding 明确合规义务 |
| 未来项目开源时合规反查 | 高 (一定发生) | 低 (现在就守规则就没事) | 从第一天起按 checklist 执行 |

---

## 13. 内部 `PORTING_LOG.md` 格式 (不公开)

```markdown
# x-claw Porting Log (Internal)

## 2025-01-15 dasclaw_agent_kernel
- Source: openai/codex@d3b04493 codex-rs/core/src/agent/
- Files ported: agentic_loop.rs, intent.rs, reasoning_ctx.rs
- Lines copied: ~3200 (60% of original)
- Lines rewritten: ~1100 (rebind to traits)
- Reviewer: <name>
- Compliance: NOTICE updated, file headers updated, prompt strings rewritten

## 2025-01-18 dasclaw_apply_patch
- Source: claw-code@610b3470 rust/crates/tools/apply_patch
- Files ported: apply_patch.rs, three_way_merge.rs
- ...
```

---

## 14. 许可证全文存放位置

```
x-claw/
├── LICENSE-APACHE          # codex Apache 2.0 全文
├── LICENSE-MIT             # claw-code MIT 全文
├── NOTICE                  # workspace 汇总署名
├── PORTING_LOG.md          # 内部不公开
├── crates/
│   └── dasclaw_agent_kernel/
│       ├── LICENSE-APACHE  # 与根目录相同 (Apache 要求每个 distribution unit 都附)
│       ├── NOTICE          # crate 级派生条目
│       └── src/
└── desktop-client/
    └── ...
```

---

## 15. 最低执行标准 (TL;DR)

如果只能做 5 件事, 必须做:

1. **每个 dasclaw_\* crate 根附 `LICENSE-APACHE` + `NOTICE`**
2. **每个移植文件头加 §7 模板注释**
3. **crate 名不含 codex / openai**
4. **改写 prompt / error / log target 字符串**
5. **README 主动致谢上游**

做完这 5 件 = 法律层面零风险, 声誉层面加分。

---

## 附: 参考资料

- [Apache 2.0 全文](https://www.apache.org/licenses/LICENSE-2.0)
- [Apache 2.0 FAQ](https://www.apache.org/foundation/license-faq.html)
- [SPDX License Identifiers](https://spdx.org/licenses/)
- [Linux Foundation: How to comply with Apache 2.0](https://www.linuxfoundation.org/blog/blog/how-to-comply-with-the-apache-2-0-license)
