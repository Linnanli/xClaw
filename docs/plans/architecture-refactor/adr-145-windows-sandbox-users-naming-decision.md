# ADR-145：Windows OS sandbox 用户/组/资源命名决策（Codex* vs Dasclaw* / Xclaw*）

- **Status**：Accepted（**选项 B — 全 rename 为 `Dasclaw*`**）
- **Date**：2025-11-15
- **Decider**：nally（产品 + 工程） — 签字于 epic #380 Batch 4 B4-0 mcp-feedback 回执
- **Related**：
  - [#241](https://github.com/Linnanli/xClaw/issues/241) Phase 4 — Fork codex-windows-sandbox epic（blocked）
  - [#380](https://github.com/Linnanli/xClaw/issues/380) Batch 4 B4-0 — 本 ADR 即 B4-0 交付物
  - [ADR-114](adr-114-dasclaw-rebrand.md) — dasclaw rebrand 规范（类 A / 类 B 分类）
  - [ADR-129](adr-129-sandbox-windows-windows-crate-adoption.md) — `dasclaw_sandbox_windows` crate verbatim 移植决策
  - [ADR-130](adr-130-sandbox-windows-lib-bin-split.md) — lib/bin 拆分（`sandbox_users` 留 bin-only）
  - [ADR-141](adr-141-windows-enterprise-sandbox-support.md) — Windows enterprise gate（已签）
  - [49-handoff](49-sandbox-windows-phase-1.1.4i-handoff.md) — Phase 1.1.4i 移交清单
  - [50-completion-and-roadmap](50-sandbox-windows-phase-1.2-completion-and-roadmap.md) — Phase 1.2 完成 + 后续路线

---

## 1. 背景

`crates/dasclaw_sandbox_windows/` 是从 `openai/codex` commit `6e838a19fa` **verbatim 移植**的 Windows OS sandbox 实现。verbatim 移植意味着源代码里仍保留 **30+ 处 `Codex*` 字面量**，这些字面量直接对应 **主机 OS 命名空间资源**——一旦 setup.exe 跑过，这些名字会落到客户机器的注册表 / Windows Local Users / Local Groups / Windows Firewall / Kernel mutex / 安装目录里。

### 1.1 受影响的命名空间（grep 实测）

| 类别 | 字面量 | 位置 | OS 落点 |
|---|---|---|---|
| Local Group | `CodexSandboxUsers` | `sandbox_users.rs:50` | `net localgroup` |
| Local Group desc | `Codex sandbox internal group (managed)` | `sandbox_users.rs:51` | 同上 |
| Local User | `CodexSandboxOffline` | `setup_orchestrator.rs:43` | `net user` |
| Local User | `CodexSandboxOnline` | `setup_orchestrator.rs:44` | 同上 |
| Setup binary | `codex-windows-sandbox-setup.exe` | `setup_orchestrator.rs:564,573,579` | Program Files 安装名 |
| Helper binary | `codex-command-runner.exe` | `helper_materialization.rs:32`, `elevated/runner_client.rs:96` | 同上 |
| Helper binary | `codex.exe` | `helper_materialization.rs:459` | 同上 |
| 资源目录 | `codex-resources` | `helper_materialization.rs:22`, `setup_orchestrator.rs:573` | 安装目录子文件夹 |
| 用户主目录 | `codex-home` / `CodexHome` | `setup_orchestrator.rs:1249-1445` | `%USERPROFILE%` 子目录 |
| Kernel mutex | `Local\CodexSandboxReadAcl` | `read_acl_mutex.rs:21` | Windows Object Manager namespace |
| Firewall rule (内部名 × 4) | `codex_sandbox_offline_block_outbound` 等 | `firewall.rs:36-45` | `netsh advfirewall` |
| Firewall rule (友好名 × 4) | `Codex Sandbox Offline - Block Non-Loopback Outbound` 等 | `firewall.rs:41-44` | 同上 |

### 1.2 已发生的 ADR-114 类 A/B 分类约束

ADR-114 §2 把 rebrand 工作分为：
- **类 A**（默认禁止新增）：纯字面量替换（dot-prefix 旧目录名 / 旧环境变量等，详见 ADR-114 §4.2）——通过 `scripts/check_no_new_ironclaw_literal.py` 在 CI 强制
- **类 B**（独立集中工程 PR）：crate rename、API rename、协议字段 rename——必须独立 ADR + 集中 PR + 升级文档

**Windows sandbox 用户/组/资源命名 = 典型类 B**：跨 OS 命名空间 + 跨版本升级需要迁移脚本 + 影响 IT 部署。

---

## 2. 问题

**B4-0 的核心问题**：Phase 4 把 setup 流程暴露给政企 IT 部署之前，必须决定 **3 个命名问题**：

1. **Q1 — Local Group / Local User 名**：`CodexSandboxUsers` / `CodexSandboxOffline` / `CodexSandboxOnline` 是否保留？
2. **Q2 — 二进制 / 资源目录名**：`codex-windows-sandbox-setup.exe` / `codex-command-runner.exe` / `codex-resources` / `codex-home` 是否保留？
3. **Q3 — Kernel mutex / Firewall rule 名**：`Local\CodexSandboxReadAcl` + 4 条 `codex_sandbox_offline_*` 防火墙规则是否保留？

**为什么必须人签**：
- 这些名字一旦落地客户机器 = **正式的产品对外 API**（IT 写 GPO 脚本会硬编码）
- 升级路径（A → B 或 B → A）需要写 PowerShell 迁移工具
- 影响商务定位（"x-claw 产品里冒出 Codex 字样" vs "drift guard 简洁")

---

## 3. 选项

### 选项 A — 全保留 Codex* 命名（verbatim 严格不动）

**做法**：
- `SANDBOX_USERS_GROUP = "CodexSandboxUsers"` 不变
- 所有 30+ 处 `Codex*` 字面量原样保留
- ADR-114 类 B 工作清单**不收录** Windows sandbox 命名（标"verbatim 例外"）
- setup.exe 安装到 `Program Files\Codex Sandbox\`
- desktop-client 文档解释："底层 sandbox 由 codex 上游开源实现 verbatim 移植，OS 命名空间资源保留上游名称以便后续 cherry-pick 升级"

**优点**：
1. ✅ **verbatim drift guard 最简**：B4-1 只需 `diff -r vendor/codex-windows-sandbox/ crates/dasclaw_sandbox_windows/` 字符级比对（除 `dasclaw_sandbox_windows::` crate path 替换外零 diff）
2. ✅ **upstream 安全更新 cherry-pick 零冲突**：codex 上游修 sandbox 0-day，我们 `git cherry-pick` 即可，不必处理 rename 冲突
3. ✅ **工程量 0**：无新 PR，无迁移脚本，无版本兼容矩阵
4. ✅ **与 ADR-129/130 一致**：那两个 ADR 已明确"verbatim 优先"

**缺点**：
1. ❌ **产品对外露 Codex 字眼**：政企 IT 在自己 Windows 机器上看到 `CodexSandboxUsers` 组、`Codex Sandbox Offline - Block Non-Loopback Outbound` 防火墙规则，**会困惑/抵触**（"我装的是 x-claw，不是 codex"）
2. ❌ **与 ADR-114 表面冲突**：ADR-114 已把旧字面量严格清理（详见其 §4.2），但 OS 资源命名空间反而留对手品牌名，**对外解释成本高**
3. ❌ **法律/品牌风险（待法务）**：在自家产品安装包写入第三方品牌名可能涉及商标使用边界（codex 是 Apache-2.0 OK，但 "Codex" 商标是否需要 attribution？需 ADR-141 §7 二级核查）

---

### 选项 B — 全 rename 为 `DasclawSandbox*`（ADR-114 类 B 集中 PR）

**做法**：
- `SANDBOX_USERS_GROUP = "DasclawSandboxUsers"`
- 所有 30+ 处 `Codex*` 字面量 rename：
  - User/Group：`Codex*` → `Dasclaw*`
  - Binary：`codex-windows-sandbox-setup.exe` → `dasclaw-windows-sandbox-setup.exe` 等
  - 目录：`codex-resources` → `dasclaw-resources`、`codex-home` → `dasclaw-home`
  - Mutex：`Local\CodexSandboxReadAcl` → `Local\DasclawSandboxReadAcl`
  - Firewall：`codex_sandbox_*` → `dasclaw_sandbox_*` + friendly 名同步
- 独立 PR + ADR-114 类 B 集中工程模式
- 写 PowerShell 迁移脚本 `migrate-codex-to-dasclaw.ps1`（如有早期客户已装 A 版）

**优点**：
1. ✅ **品牌完整**：客户机器上只见 `DasclawSandbox*`，与 desktop-client / ADR-114 完全一致
2. ✅ **与 ADR-114 哲学统一**：清掉所有非品牌字面量，单一规范
3. ✅ **政企部署文档简洁**：B4-3 不再需要"为什么有 Codex 字眼"的脚注章节
4. ✅ **绕开商标边界**：自家品牌，无需 attribution

**缺点**：
1. ❌ **verbatim drift guard 复杂**：B4-1 必须维护一份"允许的 rename 映射表"（30+ 条），每次 codex 上游变 → 我们 rename 后再 diff
2. ❌ **upstream cherry-pick 成本上升**：codex 修 sandbox 0-day，我们要应用 patch 后再做 30+ 处 sed 替换，**响应延迟从分钟级到小时级**
3. ❌ **集中 PR 工程量大**：30+ 字面量 + 测试 + 文档 + 迁移脚本 ≈ 200-300 LOC，独立 1-2 周工作量
4. ❌ **跨版本升级要写迁移工具**：如果先发了 A 版给少量客户，后转 B 版，需 setup.exe 在卸载阶段识别旧组名 `CodexSandboxUsers` 并清理

---

### 选项 C — 混合分层（推荐方向，待你判断）

**做法**：按"对外可见性"分层：

| 层 | 字面量类别 | 决策 | 理由 |
|---|---|---|---|
| L1 客户可见 | Local Group / User / 安装路径 / Firewall friendly 名 | **rename 为 Dasclaw*** | IT / 管理员肉眼看到的 |
| L2 半透明 | Binary 名 (`*.exe`) / 资源目录 (`codex-resources`) | **rename** | 装在 Program Files 下可见 |
| L3 完全内部 | Kernel mutex 名 (`Local\CodexSandboxReadAcl`) / Firewall rule **内部** ID (`codex_sandbox_*`) | **保留 Codex*** | 客户不会主动用 PowerShell 列 kernel 对象；保留有助于 verbatim drift guard |

**优点**：
1. ✅ **客户看到的全是 Dasclaw***（品牌干净）
2. ✅ **kernel 层与 codex 上游字面量同源**（drift guard 简化 70%）
3. ✅ **cherry-pick 安全补丁**：L3 层 0 改动，L1/L2 改动多在 const 顶部，patch 冲突可控
4. ✅ **工程量中等**：~12 处 L1/L2 字面量 rename，drift guard 维护 12 条映射表

**缺点**：
1. 🟡 **mental model 复杂**：开发者要记"哪层 rename / 哪层不动"
2. 🟡 **drift guard 仍非零成本**：但 12 条映射 vs 30+ 条，可承受
3. 🟡 **文档要解释分层**：B4-3 需要 1 段说明（IT 不需关心，开发者要懂）

---

## 4. 比较矩阵

| 维度 | A (全保留) | B (全 rename) | C (分层) |
|---|---|---|---|
| 客户可见品牌一致性 | ❌ 见 Codex | ✅ 纯 Dasclaw | ✅ 纯 Dasclaw |
| verbatim drift guard 复杂度 | 🟢 最低 (B4-1 ~50 LOC) | 🔴 最高 (~200 LOC + 30 条映射) | 🟡 中等 (~80 LOC + 12 条映射) |
| codex 上游 cherry-pick 成本 | 🟢 零冲突 | 🔴 每次 30+ sed | 🟡 ~12 sed |
| 政企部署文档清晰度 | 🔴 需大段解释 | 🟢 简洁 | 🟢 简洁（开发者文档加分层说明） |
| 商标/法务风险 | 🟡 需法务复核 Codex 商标使用边界 | 🟢 无 | 🟡 内部层仍有，但客户不可见 |
| ADR-114 哲学一致 | ❌ 表面冲突（需 ADR-114 §3 加例外条款） | ✅ 完美一致 | 🟡 部分一致（L3 例外） |
| 集中工程 PR 工程量 | 🟢 无 | 🔴 200-300 LOC + 迁移脚本 | 🟡 80-120 LOC，无迁移（首发即 C） |
| 升级风险（A→B 路径） | 🟢 无 | 🔴 需 PowerShell 迁移工具 | 🟢 无（首发即 C，不存在迁移） |
| B4-5 e2e 测试影响 | 🟢 无 | 🟡 测试要更新组名 | 🟡 测试要更新 L1/L2 名 |

---

## 5. 推荐

**Agent 推荐**：**选项 C（分层）**

**主要理由**：
1. **客户看到的 100% Dasclaw 品牌** — 这是政企部署 IT 体验的核心
2. **kernel/firewall 内部 ID 与 codex 上游同名** — 保住 verbatim drift guard 70% 简洁度
3. **首发即 C，无迁移成本** — 当前 #241 仍 `blocked`，没有客户部署过 A 版
4. **工程量可控** — ADR-114 类 B 集中 PR 约 80-120 LOC，独立 1 个 PR 完成
5. **与 ADR-114 哲学方向一致** — 客户可见层做品牌清理，内部 verbatim 层保留 attribution

**前置条件**：
- ✅ #241 仍 `blocked`，无 A 版生产部署 → 不需写迁移工具
- ✅ ADR-130 已明确 `sandbox_users` 是 bin-only → rename 影响面被 lib/bin 边界局限

**如果你倾向 A 或 B**：
- 选 **A** 的场景：极度看重 cherry-pick codex 安全补丁的响应速度（如 sandbox 高频 0-day），愿意接受客户问"为什么有 Codex 字眼"的解释成本
- 选 **B** 的场景：商务/法务明确要求"产品里禁出现 Codex 任何字面量"，可接受 200+ LOC 集中 PR + 后续 cherry-pick 成本上升

---

## 6. 决策（已签）

- [ ] 选项 A — 全保留 Codex*
- [x] **选项 B — 全 rename Dasclaw***
- [ ] 选项 C — 分层（agent 推荐方案，未采纳）
- [ ] 其他

**签字栏**：
- **nally**：✅ 2025-11-15（mcp-feedback 回执原文：`"B, 禁止补丁式代码, 记得做code review"`）
- **决策理由**（基于回执 + ADR §3-§5 分析）：
  - 客户机器 OS 命名空间 100% 自家品牌一致性优先
  - 与 ADR-114 §2 类 B 规范哲学完全一致，避免后续"为什么有 Codex 字眼"的解释成本
  - #241 仍 `blocked`、无 A 版生产部署 → 首发即 B，无需写 A→B 迁移工具
  - 接受 cherry-pick codex 安全补丁的 30+ sed 成本（可脚本化，由 drift guard 映射表驱动）

---

## 7. 决策后行动项（B 路径，ADR-114 类 B 集中工程 PR）

本 ADR 签字（status: Accepted）后立即执行：

1. **PR-ADR-145**（本 ADR 自身）：状态翻 Accepted + 签字 → 独立 PR 合入 `xClaw`
2. **PR-rename-W-B**：ADR-114 类 B 集中工程 PR — **一次性** rename 全 30+ `Codex*` 字面量为 `Dasclaw*`
   - 改动范围：`crates/dasclaw_sandbox_windows/src/{sandbox_users.rs, setup_orchestrator.rs, firewall.rs, read_acl_mutex.rs, helper_materialization.rs, elevated/runner_client.rs, cap.rs, lib.rs}` + 相关测试 + 文档
   - 同时产出 `scripts/codex_to_dasclaw_rename_map.json`（30+ 条映射表，drift guard 复用）
   - **禁止补丁式**：一次完成，不分批，不留半成品（nally 明确指令 + ADR §3 选项 B 描述）
3. **PR-drift-guard**（B4-1 实施）：CI job 加载映射表，比对 `vendor/codex-windows-sandbox/` 与 `crates/dasclaw_sandbox_windows/` 除映射外字符级一致
4. **PR-migrate-script**（OQ-145-3）：可选 — `setup.exe` 卸载流程同时清理 `CodexSandboxUsers` + `DasclawSandboxUsers` 双名（弱防御，10 LOC）
5. **B4-3 文档**：`desktop-client/docs/windows-sandbox-setup-guide.md` 只见 `Dasclaw*`，无 Codex 脚注

---

## 8. Open Questions

- **OQ-145-1** — Resolved：选 B 后 OS 命名空间不含 Codex 字面量，法务复核不再前置需要（`NOTICE` 文件仍保留 Apache-2.0 attribution）
- **OQ-145-2** — Resolved：选 B 全 rename，`codex-resources` → `dasclaw-resources`（保持品牌一致；中性方案 `resources` 留作 future ADR，本轮不变）
- **OQ-145-3** — 留 PR-migrate-script 实施：**默认实现双名清理**（10 LOC 弱防御），具体由 PR-migrate-script 落地

---

**Sources read**（agent 起草时实际查阅）：
- `crates/dasclaw_sandbox_windows/src/lib.rs` L1-30, L116-130, L188-310（verbatim 注释 + 模块边界）
- `crates/dasclaw_sandbox_windows/src/sandbox_users.rs` L50-77（`SANDBOX_USERS_GROUP` 定义）
- `crates/dasclaw_sandbox_windows/src/setup_orchestrator.rs` L43-44, L564-579, L1249-1445（用户名 + 安装路径 + 测试常量）
- `crates/dasclaw_sandbox_windows/src/firewall.rs` L36-46（防火墙规则名）
- `crates/dasclaw_sandbox_windows/src/read_acl_mutex.rs` L21（kernel mutex）
- `crates/dasclaw_sandbox_windows/src/helper_materialization.rs` L22-32, L432-459（资源目录 + helper binary）
- [ADR-114 §2-§3](adr-114-dasclaw-rebrand.md)（类 A / 类 B 边界）
- [ADR-129](adr-129-sandbox-windows-windows-crate-adoption.md) + [ADR-130](adr-130-sandbox-windows-lib-bin-split.md)（verbatim 移植决策 + lib/bin 拆分）
- [#241](https://github.com/Linnanli/xClaw/issues/241) blocked label 现状（无 A 版生产部署）
- [#380](https://github.com/Linnanli/xClaw/issues/380) Batch 4 B4-0~B4-6（本 ADR 是 B4-0 交付物）
