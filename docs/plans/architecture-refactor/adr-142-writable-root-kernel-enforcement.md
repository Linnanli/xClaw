# ADR-142：WritableRoot 洞中洞内核层强制（三平台 Spike 结论）

- 状态：Accepted (Phase 0 Spike — research-only)
- 关联 issue：[#370](https://github.com/Linnanli/xClaw/issues/370)（拆自 epic [#324](https://github.com/Linnanli/xClaw/issues/324) sub-task 2）
- 关联文档：
  - [`docs/plans/architecture-refactor/31-target-architecture.md`](31-target-architecture.md) §3 sandbox
  - [`docs/plans/architecture-refactor/32-execution-plan.md`](32-execution-plan.md) §W2 残留
  - [`crates/dasclaw_exec/src/lib.rs`](../../../crates/dasclaw_exec/src/lib.rs) L11-15 known-limitation 注释
  - [`codex-cli-main/codex-rs/protocol/src/protocol.rs`](../../../codex-cli-main/codex-rs/protocol/src/protocol.rs) L1033 `WritableRoot` 上游 schema
- 不替代：ADR-129（dasclaw_sandbox_windows verbatim 红线）
- 阶段：Phase 0（可行性结论 + 实施大纲）；Phase 1（三平台实现）由后续 PR 拆分推进

---

## 1. 背景

`crates/dasclaw_exec/src/lib.rs:11-15` 自陈：

> 内核层暂不强制（known limitation），用户态决策。

含义：`WorkspaceWrite` 模式下"洞中洞"语义（允许 `.git/` 写、拒 `.git/hooks/` `.git/config` 写）目前仅在 Rust 用户态由 `is_path_writable` 函数核验。一旦 agent 通过子进程绕过 Rust 调用栈（例如 `bash -c "echo hook > .git/hooks/pre-commit"`），用户态守门即被绕过。

epic #324 sub-task 2 要求把这层守门下沉到内核层，使任何子进程的 `write(2)` 系统调用都被 OS 直接拒绝，与 codex 上游对齐。

本 ADR 的 Phase 0 目标：**仅**回答"三平台是否真的可表达洞中洞"，不出代码 PR。

## 2. 决策

**结论：三平台均可表达洞中洞，进入 Phase 1 实施。**

| 平台 | 实施载体 | 上游证据 | 风险 |
|---|---|---|---|
| Linux | bubblewrap `--bind` / `--ro-bind` 嵌套 (主) ＋ landlock V5 `path_beneath_rules` (次) | codex `linux-sandbox/src/bwrap.rs` + `tests/suite/landlock.rs::sandbox_reenables_writable_subpaths_under_unreadable_parents` | 低：codex 已生产实证 |
| macOS | seatbelt `(deny file-write* (subpath …))` 嵌套规则 | codex `sandboxing/src/seatbelt_tests.rs::create_seatbelt_args_with_read_only_git_and_codex_subpaths`、`explicit_unreadable_paths_are_excluded_from_full_disk_read_and_write_access` | 低：codex 已生产实证 |
| Windows | DACL DENY entries via `dasclaw_sandbox_windows::acl::SetEntriesInAclW(DENY_ACCESS=3)` | `crates/dasclaw_sandbox_windows/src/acl.rs`（codex `windows-sandbox-rs/src/acl.rs` verbatim port） | 中：与 ADR-129 verbatim 红线交互（见 §6） |

## 3. 三层验证证据

### 3.1 Level 1（语义层 / `semantic_search`）

查询：`landlock ruleset read-only subpath nested path enforcement writable root sandbox`

命中关键节点（截选）：

| 文件 | 行 | 证据 |
|---|---|---|
| `codex-cli-main/codex-rs/protocol/src/protocol.rs` | 1033 | `WritableRoot { root, read_only_subpaths, protected_metadata_names }` 协议层定义 |
| `codex-cli-main/codex-rs/linux-sandbox/src/landlock.rs` | 158 | `install_filesystem_landlock_rules_on_current_thread` 用 V5 ABI + `path_beneath_rules` |
| `codex-cli-main/codex-rs/linux-sandbox/src/bwrap.rs` | 2430 | `split_policy_reenables_nested_writable_roots_after_unreadable_parent` 单元测试 |
| `codex-cli-main/codex-rs/linux-sandbox/tests/suite/landlock.rs` | 857-925 | `sandbox_reenables_writable_subpaths_under_unreadable_parents` 集成测试 |
| `codex-cli-main/codex-rs/linux-sandbox/src/linux_run_main_tests.rs` | 238 | 测试名直陈 `root_write_read_only_carveout_requires_direct_runtime_enforcement` |
| `codex-cli-main/codex-rs/sandboxing/src/seatbelt_tests.rs` | 1048+ | `create_seatbelt_args_with_read_only_git_and_codex_subpaths` 实测 sbpl 嵌套生效 |
| `codex-cli-main/codex-rs/sandboxing/src/seatbelt_tests.rs` | 188+ | `explicit_unreadable_paths_are_excluded_from_full_disk_read_and_write_access` |
| `crates/dasclaw_sandbox_windows/src/acl.rs` | 58 | `const DENY_ACCESS: i32 = 3` + `SetEntriesInAclW` 直绑 Win32 API |

### 3.2 Level 3（字面量层 / `grep_search`）

查询：`crates/dasclaw_sandbox_windows/src/**` matches `read_only_subpath|is_path_writable|deny|DENY|ACL|JobObject` → 20+ 命中确认 Windows ACL DENY 实现已在 verbatim port 范围内（`acl.rs:1-2` 注明 `path: codex-rs/windows-sandbox-rs/src/acl.rs`）。

### 3.3 否定性反证

`grep_search codex-cli-main/codex-rs/**/*.rs` 查询 `DENY|ACL|deny_entry|file_deny|read_only_subpath.*windows` → 0 命中。

**含义**：codex 上游主仓 (`codex-rs/`) 不直接持有 Windows ACL 实现；Windows 路径**仅**在 `windows-sandbox-rs` 子 crate 中提供（已被 dasclaw verbatim port 至 `crates/dasclaw_sandbox_windows`）。这与 ADR-129 锁定的 verbatim 范围一致。

## 4. 实施大纲（Phase 1，本 ADR 不出代码）

### 4.1 Linux PR（PR-A）

- 路径：`crates/dasclaw_exec/src/sandbox/linux.rs`（或扩展 `dasclaw_sandbox` 现有 Linux 后端）
- 实现：构造 `bwrap` 命令行时，将 `read_only_subpaths` 转 `--ro-bind` 覆盖 `--bind` 之上（codex `bwrap.rs::create_filesystem_args` 算法）
- Fallback：纯 landlock V5 + `path_beneath_rules`（不绑定 bubblewrap 时）
- 失败路径测试：在 `WorkspaceWrite` 下 `bash -c "echo > .git/hooks/x"` 必须返回 non-zero exit + EACCES，不能返回成功
- 红线：禁止用户态预检替代内核检查；fail-safe = 失败拒绝写

### 4.2 macOS PR（PR-B）

- 路径：`crates/dasclaw_exec/src/sandbox/macos.rs`（或新建 `seatbelt.rs`）
- 实现：生成 sbpl 时按 codex `seatbelt.rs` 规则集合：先 `(allow file-write* (subpath "/repo"))`，再 `(deny file-write* (subpath "/repo/.git/hooks"))` 嵌套
- 失败路径测试：与 Linux 对称，验 `sandbox-exec` 拒绝写 `.git/hooks/`

### 4.3 Windows PR（PR-C）

- 路径：复用 `crates/dasclaw_sandbox_windows::acl`（**禁止**修改 acl.rs 本体，受 ADR-129 verbatim 锁定）
- 实现：在 `dasclaw_exec` Windows 后端调用 `dasclaw_sandbox_windows` 的公共 API（如 `apply_writable_root_with_carveouts`）；若该 API 不存在则在 `dasclaw_exec` 侧组合 `acl.rs` 已暴露的低层函数
- 失败路径测试：验 DACL DENY entry 生效（写 `.git\hooks\x` 返回 `ERROR_ACCESS_DENIED`）

### 4.4 共通验收

每个平台 PR 必须满足：

- [ ] 用户态 `is_path_writable` **保留作为预检**，但不再作为唯一守门——失败路径测试必须证明子进程 syscall 也被拒绝
- [ ] PICT 测试矩阵覆盖 (writable_root × read_only_subpath × access_mode × sandbox_kind) 组合
- [ ] `cargo nextest run -p dasclaw_exec --test e2e_writable_root_kernel` 全过
- [ ] 失败路径覆盖率 100%（AGENTS.md 安全模块要求）

## 5. 替代方案与拒绝理由

### 5.1 维持用户态决策（ADR-only 文档化已知限制）

- 拒绝理由：epic #324 明确要求"按 codex 上游最佳方案实现，不接受当前的折中"。三平台均有上游可用证据，无降级理由。

### 5.2 仅 Linux 实现，macOS/Windows 标 known-limitation

- 拒绝理由：codex 上游三平台均已实证可表达；标 known-limitation 等于自我降级。

## 6. 风险与红线

| 风险 | 缓解 |
|---|---|
| Linux landlock V5 在内核 < 5.13 上 abi mismatch | `set_compatibility(BestEffort)` + 启动 abi 探测；不可用时 fallback bubblewrap-only |
| macOS seatbelt deprecated 警告 | codex 已接受此 deprecation 风险；本 ADR 与上游对齐 |
| Windows ACL 与 ADR-129 verbatim 红线冲突 | **禁止**修改 `crates/dasclaw_sandbox_windows/src/acl.rs` 本体；只在消费侧组合调用 |
| 失败路径回退到允许写（fail-open） | 测试覆盖率门禁：失败路径 100%；AGENTS.md 安全模块要求 |

## 7. 后续工作

- [ ] 开 #371（Linux PR-A）、#372（macOS PR-B）、#373（Windows PR-C）三个 sub-issue
- [ ] PR-A/B/C 并行开发（文件零重叠 — A 在 Linux 后端、B 在 macOS 后端、C 在 Windows 后端）
- [ ] Phase 1 完成后回写 #324 epic body 关闭 sub-task 2
- [ ] 更新 `crates/dasclaw_exec/src/lib.rs:11-15` 的 known-limitation 注释为"已通过 ADR-142 三平台内核强制覆盖"
