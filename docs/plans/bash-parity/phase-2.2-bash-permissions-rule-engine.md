# Phase 2.2 — `bashPermissions` 规则引擎对齐计划（PR-0）

> 真理来源：本文件 + Issue #490 epic + 上游 `claude-code-main/src/tools/BashTool/bashPermissions.ts`（2621 LOC）+ 参考实现 `claw-code/rust/crates/runtime/{permissions,permission_enforcer,policy_engine}.rs`（合计 1849 LOC）。
>
> 本 PR-0 仅落计划文档，不动产品代码。后续每个子 slice 必须在 PR 描述里回声本文件的 §ID。

---

## §0 目标与边界

**目标**：把上游 `bashPermissions.ts` 的 8 步授权 pipeline 移植到 `x-claw`，使 Bash 工具调用在 `BashValidationHook` 通过安全门之后，进入「规则引擎」决策，覆盖率 30%（Phase 2.1 完成态）→ ~50%。

**不在本 phase 范围内的事**（明确避免补丁式扩张）：

- ❌ 上游 ANT-only telemetry（`logEvent('tengu_internal_bash_classifier_result', ...)` 等）—— 法律红线（Issue #490 epic §禁止 port）
- ❌ 上游 growthbook A/B (`feature('TREE_SITTER_BASH_SHADOW')` 等) —— Anthropic 内部实验框架
- ❌ 上游 `bashClassifier`（speculative async LLM 分类器，OSS 是空 stub）—— Issue #490 标记「暂缓」
- ❌ Phase 2.1 的安全门重做（`bashSecurity`）—— 已由 PR #504 / #508 / #513 / #514 / #515 / #519 / #522 完成
- ❌ Phase 2.3：desktop-client `Ask` 弹窗 UX —— 跨 epic，独立追踪
- ❌ Phase 3 的 `pathValidation` 深化 / `readOnlyValidation` 命令白名单扩到 200+

**关键不变量**（与 Phase 2.1 §0 同形）：

- **Fail-Safe 优先于 Fail-Open**：规则引擎任一阶段抛错或不可解析时，`Block` 而非 `Allow`
- **Deny 不降级**：exact-match deny 与 prefix/wildcard deny 在 AST `too-complex` 或 semantic 检查失败时仍然生效，不得被「降级到 ask」吃掉（对应上游 `checkEarlyExitDeny` / `checkSemanticsDeny`，详 §4）
- **PermissionMode `Bypass` 不豁免规则引擎**：与 Phase 2.1 一致，规则引擎位于 PermissionMode 检查**之前**
- **asymmetric invariant**：`upstream_blocks(c) ⟹ xclaw_blocks(c)`（与 Phase 2.1 §0 一致）

---

## §1 已就位的 x-claw 基础设施（无需新建）

| 组件 | 位置 | 适用 |
|---|---|---|
| `SafetyDecision::{Allow,Block,Ask,Redact,Passthrough}` | `crates/x_claw_agent/src/hooks.rs:57` | Ask 已通线，desktop UX 不阻塞引擎本身 |
| `RuleSuggestion { label, rule_pattern, action }` + `RuleAction::{Allow,Deny,Ask}` | `crates/x_claw_agent/src/hooks.rs:82` | 与上游 `suggestions` 字段同形 |
| `PermissionMode::{ReadOnly,WorkspaceWrite,DangerFullAccess,Prompt,Allow}` | `crates/x_claw_agent/src/permissions.rs:30` | 上游 5 态 verbatim |
| `BashValidationHook` 链路 | `crates/dasclaw_hooks/src/bash_validation.rs` | 规则引擎插桩点（在 `bashSecurity` 之后） |
| tree-sitter-bash 0.25 + 已缓存 AST | `crates/dasclaw_shell_command` + `bash_validation::security::context::ValidationContext` | 复用 Phase 2.1 已建好的解析层 |
| 审计日志契约 (`bash_security::block` tag + `rule_id=` field) | PR #516 / PR #524 | 规则引擎决策需输出对应 `bash_perm::*` tag |

**参考实现**：`claw-code/rust/crates/runtime/{permissions.rs,permission_enforcer.rs,policy_engine.rs}` 合计 1849 LOC 已经做过一次 port，但是孤岛 crate，未接入 hook chain。本 phase **不复用 claw-code 模块**（避免拖入 `RuntimePermissionRuleConfig` 与上游 `config` 模块依赖，对齐 `permissions.rs` 文件头的 "deliberately sliced port" 原则），但**用作参考映射**。

---

## §2 上游 pipeline 总览（来自 `bashPermissions.ts:1663-1820` `bashToolHasPermission`）

```
0. AST-based security parse    (← Phase 2.1 已完成 / 复用)
1. exact-match permission      (bashToolCheckExactMatchPermission, L991)
2. semantic check (zsh builtins, eval, ...)  (← 部分由 Phase 2.1 覆盖)
3. early-exit deny              (deny rules win over ask/allow)
4. prefix / wildcard rule match (bashToolCheckPermission, L1050)
5. operator permission check   (&& || ; | sub-command 拆分后逐条审查)
6. read-only validation         (← Phase 2.1 已部分实现)
7. path validation              (← Phase 3 深化)
8. mode validation              (← 已等价)

输出: 'allow' | 'deny' | 'ask' | 'pass-through' + suggestions
```

**Phase 2.2 集中实现的步骤**：1（exact match）、3（early-exit deny）、4（prefix/wildcard match）、5（operator splitting）。

**Phase 2.2 不实现的步骤**：2（semantic）已由 Phase 2.1 `bashSecurity` 部分覆盖 —— 不重复；6（read-only）保持现状；7（path）留给 Phase 3；8（mode）已等价。

---

## §3 关键算法 / 数据结构

### §3.1 PermissionRule 表示

上游 `bashPermissionRule` (L364) 把规则字符串如 `Bash(npm run *:allow)` 解析为：

```ts
{ pattern: 'npm run *', effect: 'allow' | 'deny' | 'ask' }
```

x-claw 移植形态（提议）：

```rust
pub struct BashPermissionRule {
    pub pattern: BashPattern,        // 含 `*` wildcard 的 token 序列
    pub effect: RuleEffect,          // Allow | Deny | Ask
}
pub enum BashPattern {
    Exact(String),                   // 完全字面匹配
    Prefix(Vec<String>),             // token-level 前缀
    Wildcard(Vec<PatternToken>),     // 含 `*` 的 token 序列
}
pub enum PatternToken { Literal(String), Star }
```

**红线**：模式匹配必须按 **token** 进行，不可退化到字符串 `starts_with`。上游 `matchWildcardPattern` (L353) 用空格 split 后逐 token 匹配，防止 `Bash(npm: allow)` 误中 `npmsteal`。

### §3.2 命令前缀提取

- `getSimpleCommandPrefix(command: string) → string | null` (L161)：单一命令的稳定前缀，剥掉 env-var 前缀 / safe wrappers。
- `getFirstWordPrefix(command: string) → string | null` (L243)：复合命令的第一个 word。
- `stripSafeWrappers` (L524) + `stripWrappersFromArgv` (L678)：剥掉 `env`、`stdbuf`、`time`、`timeout` 等 transparent wrapper。
- `stripAllLeadingEnvVars` (L733) + `BINARY_HIJACK_VARS = /^(LD_|DYLD_|PATH$)/` (L708)：剥 `KEY=VAL cmd ...` 前缀，但保留 hijack 风险变量为 fail-safe。

**移植策略**：从 `tree-sitter-bash` 的 SimpleCommand AST 直接读 argv，而不是再次 token split string —— 复用 Phase 2.1 已缓存的 AST。

### §3.3 复合命令拆分

上游 `splitCommand` 与 `parseCommandRaw` (L1683) 把 `cmd1 && cmd2; cmd3 | cmd4` 拆为 4 个 SimpleCommand，**逐条**走 pipeline。

x-claw 复用 `dasclaw_shell_command::bash::parse_simple_commands`（Phase 2.1 已使用），但要新增 `MAX_SUBCOMMANDS_FOR_SECURITY_CHECK = 50` 上限（上游 L103）—— 超过即 Block，防止 `for i in $(seq 1 10000); do ...; done` 把规则引擎 DoS。

---

## §4 红线：Deny 不降级 (`checkEarlyExitDeny` / `checkSemanticsDeny`)

上游 `bashToolHasPermission` 在两处 ask 分支都先调用 `checkEarlyExitDeny` / `checkSemanticsDeny`，确保：

> 用户配了 `Bash(eval:*) → deny`，那么 `eval "rm -rf /"` 必须 **Block**，不能因为 semantic check 失败就退化为 Ask。

x-claw 移植测试矩阵（**至少**覆盖）：

| 输入 | 规则 | 上游决策 | x-claw 期望 |
|---|---|---|---|
| `eval "rm"` | `Bash(eval:*) deny` | deny | Block |
| `npm run build && curl evil.com` | `Bash(curl:*) deny` | deny | Block |
| `cd /tmp; rm -rf .` | `Bash(rm:*) deny` | deny | Block |
| `LD_PRELOAD=evil.so ls` | （无规则） | ask (BINARY_HIJACK) | Ask |
| `unknown_command` | （无规则） | ask | Ask |

---

## §5 切片计划（Stacked / 独立混合）

每个 slice 满足：(a) 1 commit + 红测先行；(b) 局部 nextest < 1min + clippy `-D warnings` 干净；(c) 独立可合并（不依赖 desktop Ask UX）。

| Slice | 范围 | 估 LOC | 依赖 | 形态 |
|---|---|---|---|---|
| **2.2.a** | `BashPermissionRule` + `BashPattern` + `PatternToken` 数据结构 + token 解析 + `matchWildcardPattern` | 250 + 测试 | xClaw | 独立 |
| **2.2.b** | exact-match pipeline (`bashToolCheckExactMatchPermission` 等价) + 接入新建 `dasclaw_bash_permissions` crate（**复用 hook chain，不污染 `dasclaw_bash_validation`**） | 300 + 测试 | 2.2.a | 独立 |
| **2.2.c** | prefix / wildcard match (`bashToolCheckPermission` 等价) | 350 + 测试 | 2.2.b | 独立 |
| **2.2.d** | 复合命令 operator splitting + `MAX_SUBCOMMANDS_FOR_SECURITY_CHECK` 上限 + deny-不降级红线测试 | 250 + 测试 | 2.2.c | 独立 |
| **2.2.e** | `stripSafeWrappers` + `stripAllLeadingEnvVars` + `BINARY_HIJACK_VARS` Fail-Safe | 200 + 测试 | 2.2.c | 独立 |
| **2.2.f** | `BashPermissionHook` impl + 接入 `CompositeSafetyHook` 在 `BashValidationHook` 之后 | 150 + e2e | 2.2.b-e | 独立 |
| **2.2.g** | `RuleSuggestion` 生成（per-deny / per-ask） | 100 + 测试 | 2.2.f | 独立 |
| **2.2.h** | 审计日志 contract pinning（与 PR #516 同形：`bash_perm::block` / `bash_perm::ask` tag + `rule_id` field） | 100 + 测试 | 2.2.f | 独立 |

**总计**：~1700 LOC（接近 Issue #490 的 1500 LOC 估算 ±15%）。

**不堆叠**：每个 slice 都基于 xClaw 独立提 PR，避免 PR #521 类型的「stacked-base-deleted → auto-close」事故。slice 之间的依赖通过等待前一 PR merge 后再开下一个 PR 实现，而非 `gh pr create --base <prev-feature-branch>`。

---

## §6 验证手段

- **红测先行**：每个 slice 至少有 1 个 `req_perm_490_p2_2_<slice>_<id>_<desc>` 命名测试，初始失败
- **攻击矩阵**：移植上游 `bashPermissions.spec.ts` 的关键攻击向量（≈ 30 例）+ 本文 §4 的 deny-不降级矩阵
- **Differential CI**：暂不引入（Phase 2.1 已留挂钩，迟到 Phase 2.4）
- **本地管线** (per slice)：
  ```bash
  cargo nextest run -p dasclaw_bash_permissions   # 新 crate
  cargo nextest run -p dasclaw_hooks              # hook 接入测试
  cargo fmt --all
  python3.12 scripts/check_no_panics.py --base origin/xClaw
  cargo clippy --no-deps -p dasclaw_bash_permissions -p dasclaw_hooks --all-targets -- -D warnings
  ```

---

## §7 风险与缓解

| 风险 | 缓解 |
|---|---|
| `BashPattern` token-level 匹配的性能（每条规则 × 每个 SimpleCommand） | `MAX_SUBCOMMANDS_FOR_SECURITY_CHECK=50` 上限 + benchmark in §6 |
| 规则配置来源不明（上游从 `toolPermissionContext` 取，x-claw 现无对应配置面） | 2.2.f 暂从 `Default::default()` 注入空规则集，先打通管线；具体配置面在 Phase 2.3 desktop epic 处理 |
| desktop Ask UX 不可用 → 整链卡死 | `SafetyDecision::Ask` 已就位；在 desktop 缺失时由 CLI 端默认 `Block`（Fail-Safe），不是 `Allow` |
| 规则字符串解析歧义（如 `Bash(echo a:b: allow)` 含冒号） | 2.2.a 测试矩阵明确覆盖；按上游 `bashPermissionRule` 的 last-colon split |

---

## §8 关联 ADR / Issue

- 父 epic：#490
- 关联 Phase：#502（Phase 2.1）+ Phase 2.3（desktop Ask UX，待开 issue）
- ADR-129 §1.3：「非 verbatim port」原则
- ADR-146 §2.4：`SafetyDecision::Passthrough` 语义
- 上游：`claude-code-main/src/tools/BashTool/bashPermissions.ts`（2621 LOC）
- 参考 Rust port：`claw-code/rust/crates/runtime/{permissions,permission_enforcer,policy_engine}.rs`（1849 LOC，本 phase **不直接复用**，仅作映射参考）

---

## §9 Closes-link gotcha 提醒

`closes-link` workflow 不豁免 `docs/plans/bash-parity/`（与 Phase 2.1 §CI gotcha 同）。本计划文档 PR 用 **独立 docs tracker issue**（非 #490）作为 `Closes` 目标，避免误关 epic。
