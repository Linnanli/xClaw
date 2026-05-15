# ADR-150 — Phase 3.1 Symlink 逃逸检测

- **状态**：Draft
- **作用域**：`crates/dasclaw_bash_validation/src/path_validation.rs`
- **依赖**：ADR-112、ADR-113、PR #567（3.1.A 落地 `validate_command_paths` 纯字符串边界判定）
- **触发上游**：`claude-code-main/src/utils/permissions/pathValidation.ts` +
  `claude-code-main/src/utils/fsOperations.ts`（`safeResolvePath` /
  `getPathsForPermissionCheck` / `resolveDeepestExistingAncestorSync`）
- **关联**：Phase 3.1 plan `docs/plans/bash-parity/phase-3.1-path-validation-deepening.md`
  §3 表格 / §5 S10 / §7「后续」「Phase 3.1.g」

---

## 1. 背景

PR #567 已落地 3.1.A：在 `crates/dasclaw_bash_validation/src/path_validation.rs`
中实现 `PATH_EXTRACTORS` + `validate_command_paths`，对 36 个路径敏感命令做
**纯字符串、零 IO** 的工作区边界判定（lexical `normalize_path` + 字符串前缀比较）。

Phase 3.1 plan §5 **S10 软链接逃逸** 被显式列为 3.1.A 范围外的「后续」，原因是：

- `claude-code-main/src/utils/permissions/filesystem.ts` 整文件 1777 LOC，
  含 SymlinkPolicy / git submodule / `getRealPath` 等大量 IO 逻辑；
- 3.1.A 必须保持「快速、零 syscall」以确保 BashTool 前置校验链每条命令成本可控；
- symlink 校验必然引入 IO + 平台差异，需要单独的威胁建模与平台 gating。

**当前残留风险**（已在 §1.1 实证）：在工作区内创建符号链接指向 `/etc`、
`~/.ssh`、`$HOME/.aws/credentials` 等敏感路径时，3.1.A 的字符串判定会通过
（解析后路径仍在 workspace 字符串前缀内），但实际文件操作会跨边界读写。

### 1.1 PoC（已能复现）

```bash
# workspace = /tmp/ws；ws/leak -> /etc
mkdir -p /tmp/ws && cd /tmp/ws && ln -s /etc leak
# 3.1.A validate_command_paths 判定：Passthrough（cat /tmp/ws/leak/passwd 在边界内）
# 实际：cat 跨边界读 /etc/passwd
```

`validate_command_paths` 应当对**任何一个**链路目标落在工作区外的请求返回
`Ask`/`Block`，与上游 `pathValidation.ts` 的 `getPathsForPermissionCheck`
逐跳收集语义保持一致。

---

## 2. 决策

引入 **Phase 3.1.g** 切片，在 `dasclaw_bash_validation::path_validation` 中
增加一个**可选、按需触发**的符号链接解析层，签名上保持 `validate_command_paths`
不变（输入仍是 `(PathCommand, args, cwd, workspace_dirs, home_dir)` —— 即 3.1.A
锁定的纯函数边界），通过一个**新结构体的可选字段**承接 IO 能力，且默认 Fail-Closed。

### 2.1 设计原则（按优先级）

1. **纯函数主入口不变** —— 3.1.A 已落地的 `validate_command_paths`
   签名保持只有 `&Path` 与字符串参数；不在主签名插入 `dyn FsOps` 之类的容器。
2. **IO 能力通过新构造函数承接** —— 新增 `validate_command_paths_with_fs(
   ..., fs: &dyn FsResolver)`，原 `validate_command_paths` 在内部委托给
   `validate_command_paths_with_fs(.., &NoopFsResolver)`。这避免补丁式
   `if symlink_enabled { ... } else { ... }` 分支，符合
   `code-quality.instructions.md` §3「禁标志位切换大块代码」。
3. **Fail-Closed 契约** —— `FsResolver` 任何方法返回 `Err` 或 None 时，对应
   target 一律降级为 `PathValidationOutcome::Ask`，与上游 `safeResolvePath`
   的「return original path on error」不同：上游靠 `isPathAllowed` 多层
   兜底，我们在第一层就必须 ask。
4. **平台 gating 在 trait 实现层** —— `FsResolver` 是平台中立的；
   `RealFsResolver` 内部按 `cfg(target_os = ...)` 分发，Windows 走 reparse-point
   API（不依赖 POSIX `lstat`），ADR-145 / ADR-146 已建立的 Windows 策略复用。
5. **零 panic** —— `RealFsResolver` 所有 `?` 转换错误，禁止 `unwrap`/`expect`
   （`scripts/check_no_panics.py` 兜底）。

### 2.2 接口草案

```rust
// crates/dasclaw_bash_validation/src/path_validation.rs

/// Trait describing the minimum IO surface the symlink-escape pass needs.
/// All methods MUST be infallible-or-None — propagating filesystem errors
/// upward is the caller's job. Returning None from any method causes the
/// outer validator to escalate the originating target to
/// `PathValidationOutcome::Ask` (Fail-Closed).
pub trait FsResolver {
    /// Equivalent of `lstat` — does NOT follow the final symlink.
    /// Returns `None` for ENOENT / EACCES / ELOOP / EIO / dangling, etc.
    fn lstat_kind(&self, p: &Path) -> Option<FsEntryKind>;

    /// Equivalent of `readlinkSync` — relative targets resolved against
    /// the symlink's *parent* (matches `path.resolve(dirname(p), target)`).
    fn readlink_absolute(&self, p: &Path) -> Option<PathBuf>;

    /// Equivalent of `realpathSync` — best-effort canonicalization of
    /// existing portion. Returns `None` if the deepest existing ancestor
    /// also fails (caller will Ask).
    fn realpath(&self, p: &Path) -> Option<PathBuf>;
}

pub enum FsEntryKind {
    File,
    Dir,
    Symlink,
    /// Char/block/FIFO/socket — treated like the upstream
    /// `safeResolvePath` early-return: do NOT realpath (avoids
    /// `realpathSync` hang on FIFO waiting for writer).
    Special,
}

/// Same as `validate_command_paths` but additionally walks the symlink
/// chain on each extracted target (upstream `getPathsForPermissionCheck`).
///
/// Any chain step that lands outside `workspace_dirs` flips the outcome
/// to `PathValidationOutcome::Ask` regardless of where the original
/// lexical path landed.
pub fn validate_command_paths_with_fs(
    cmd: PathCommand,
    args: &[String],
    cwd: &Path,
    workspace_dirs: &[PathBuf],
    home_dir: &Path,
    fs: &dyn FsResolver,
) -> PathValidationOutcome;
```

`validate_command_paths` 的当前实现等价于
`validate_command_paths_with_fs(.., &NoopFsResolver)`，其中
`NoopFsResolver` 所有方法返回 `None`（即「无 IO 能力」=「保留 3.1.A 行为」）。

### 2.3 链路收集算法（对照上游 §1）

直接镜像 `getPathsForPermissionCheck`（`fsOperations.ts:288-383`），逐跳
最多 `SYMLOOP_MAX = 40` 步，每步：

| 状态 | 上游行为 | 本 ADR 行为 |
|------|---------|------------|
| 不存在（包括 dangling） | 用 `resolveDeepestExistingAncestorSync` 把父链 realpath，rejoin 尾部 | 相同；`realpath` 返回 `None` → 整条 target Ask |
| Special（FIFO/Socket/Char/Block） | 跳出循环，不 realpath | 相同 |
| 非 symlink 普通文件/目录 | 跳出循环 | 相同 |
| symlink | `readlinkSync` 拿到 target，绝对化后追加到集合 | 相同；目标自身落 workspace 外即 Ask |
| 循环 | `visited` set + max-depth=40 | 相同；超限 Ask |

对于每个目标都用 `path_in_workspace`（3.1.A 已有，纯字符串比较）逐一判定，
**任何一跳**外逃 → 整个 `validate_command_paths_with_fs` 返回 Ask。

### 2.4 与 3.1.B / 3.1.C 的集成

- **3.1.B**（`validate_output_redirections` + 主入口 + AST argv 拆分）：
  其在 §3 表格中调用 `validate_command_paths`；直接替换为
  `validate_command_paths_with_fs`，传入 BashTool 持有的 `RealFsResolver`
  即可获得 symlink 防护。
- **3.1.C**（hook 接线 + 新 `DecisionReason`）：增设
  `DecisionReason::SymlinkEscape { original: PathBuf, resolved: PathBuf }`，
  审计日志契约固化「原始路径 + 解析后路径」**双字段**，
  排错路径与 PR #524 / #530 / #560 同结构。
- **Phase 3.1.g** 本身**不动 hook 接线**，落地后通过 feature flag 渐进开启
  （详见 §6 推广路径）。

---

## 3. 威胁模型

### 3.1 资产

- 工作区内文件（可读、可写）
- 工作区外受保护文件：`/etc/*`、`~/.ssh/*`、`~/.aws/credentials`、
  `~/.kube/config`、`/var/log/*`、Windows `%USERPROFILE%\.ssh\*`、
  `%APPDATA%\*` 等
- 上游 `dangerousPatterns.ts` 黑名单（已被 3.1.A `is_dangerous_removal_path`
  覆盖一部分）

### 3.2 攻击者能力

| 能力 | 备注 |
|------|------|
| 在 workspace 内创建任意符号链接 | bash 工具默认允许在 cwd 内 `ln -s` |
| 在 workspace 内的子目录创建符号链接 | 同上；嵌套深度不限 |
| 在 dangling 状态预埋链接 | 例如 `ln -s /etc/cron.d/x evil`，之后请求 `echo "*/1 * * * * ..." > evil` |
| 利用 macOS `/tmp -> /private/tmp` 等系统级 symlink | workspace 本身可能就在 `/private/var/folders/...` 但 cwd 输入 `/tmp/...` |
| 借助 `..` + symlink 拼接 | 例如 `link/../../etc` |

### 3.3 已识别攻击向量（覆盖必须 100%）

| ID | 向量 | 当前 3.1.A 行为 | 本 ADR 目标行为 |
|----|------|----------------|----------------|
| T-SYM-1 | workspace 内 symlink → `/etc` | Passthrough（误放） | Ask（双字段日志） |
| T-SYM-2 | dangling symlink + 写入（`echo > evil`） | Passthrough | Ask（resolveDeepestExistingAncestor 命中父链外逃） |
| T-SYM-3 | 链式 symlink（A → B → /etc/shadow） | Passthrough | Ask（中间跳 B 也要 in-workspace） |
| T-SYM-4 | 循环 symlink（A → B → A） | Passthrough | Ask（max-depth 40 触发 → None → Ask） |
| T-SYM-5 | 工作区目录本身是 symlink（macOS `/tmp` → `/private/tmp`） | 视前缀拼配而定，可能误拒 | 对 `workspace_dirs` 启动时 realpath 一次缓存（见 §6 推广路径 Step 1） |
| T-SYM-6 | UNC / `\\?\` 路径 | 已在 3.1.A `expand_tilde_and_home` 后未单独拦 | Windows 实现里 pre-syscall 拒（参照上游 `safeResolvePath:139-144`） |
| T-SYM-7 | FIFO / socket 作为目标 | Passthrough | `FsEntryKind::Special` → 跳出循环，原 lexical 路径仍走 3.1.A 边界判定 |
| T-SYM-8 | `~` / `~user` 后接 symlink | 3.1.A 已对 `~user/~+/~-` 拒，但 `~/.cache -> /etc` 仍漏 | 在 home 展开后照常进入符号链路解析 |

### 3.4 非目标（What's NOT in scope）

- ❌ TOCTOU 完整解决方案（exec 时的二次解析）—— 与上游一致，仅做**校验时刻**的尽力解析；完整 TOCTOU 防护需 OS 沙箱层兜底（参见 ADR-002 / ADR-141 / ADR-144）。
- ❌ 跨 mount / bind-mount 边界识别 —— 不做。bind-mount 表现为 symlink 不可见的 inode 重复，本 ADR 不解决。
- ❌ Git submodule / sparse-checkout / worktree 的边界识别 —— 上游
  `filesystem.ts` 的 SymlinkPolicy 也未承担，留给业务层（Phase 3.x 之外）。
- ❌ 修改 3.1.A 已落地的 `PATH_EXTRACTORS` / `is_dangerous_removal_path` /
  `expand_tilde_and_home` 任何字符串契约。

---

## 4. 跨平台差异

### 4.1 macOS

- `/tmp` → `/private/tmp`、`/var` → `/private/var`：常见系统级 symlink，工作区
  实际存放在 `/private/var/folders/...` 但用户输入 `/tmp/...` 是常态。
- 应对：启动时对 `workspace_dirs` 做一次 `realpath`（缓存），后续 in-workspace
  比较时**同时**用原始字符串与 realpath 双值做前缀匹配。
- `realpath(2)` 对不存在路径返回 `ENOENT`，与上游一致。
- 不存在 NTFS reparse-point 概念，纯 POSIX `lstat` + `readlink`。

### 4.2 Linux

- 行为与上游 Node `lstatSync`/`realpathSync` 语义一致。
- `/proc/<pid>/root` 等伪文件系统：lstat 看到的是 symlink，需用统一 max-depth + None-on-error 兜底。
- container 内 `/dev/null` 等 special device 走 `FsEntryKind::Special` 分支，
  跳过 realpath（避免 ELOOP/EACCES 跨 namespace）。

### 4.3 Windows

- 无符号链接（无特权用户场景）；但有 **junctions** 与 **reparse points**
  与 **mountpoints**。
- 实现路径：
  1. 用 `std::fs::symlink_metadata` 拿到 `file_attributes`；
  2. `attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0` → 当作 symlink；
  3. 调 `std::fs::read_link`（Rust std 已封装 `DeviceIoControl + FSCTL_GET_REPARSE_POINT`）。
- UNC 路径 (`\\server\share\...`、`\\?\C:\...`、`\\.\PIPE\...`)：
  **pre-syscall 拒绝**，直接返回 `None`，触发 Ask。理由与上游 `safeResolvePath:139-144` 一致：防止 DNS/SMB 查询。
- ADR-145 已对 Windows sandbox user 命名做过决策；本 ADR 与之正交，
  不复用其用户上下文。

### 4.4 共性约束

- **`SYMLOOP_MAX = 40`**（与上游 `fsOperations.ts:298` 同）—— Linux POSIX 默认
  也是 40，macOS 是 32，Windows 没有定义 → 取 40 兼容上游审计。
- **无 follow-final-link 行为差异容忍**：始终从 `lstat` 出发，永不
  `stat`（避免隐式跨边界）。

---

## 5. Fail-Closed 契约

| 触发条件 | 当前 3.1.A 行为 | 本 ADR 行为 |
|---------|----------------|------------|
| `FsResolver.lstat_kind` 返回 None（任何 IO 错误） | N/A | **Ask**（DecisionReason::SymlinkResolveFailed） |
| `FsResolver.realpath` 返回 None | N/A | **Ask** |
| `FsResolver.readlink_absolute` 返回 None | N/A | **Ask** |
| max-depth 超限（40） | N/A | **Ask**（DecisionReason::SymlinkLoop） |
| 链路任一节点 in-workspace 判定为 false | N/A | **Ask**（DecisionReason::SymlinkEscape，含原始 + resolved 双字段） |
| Special file（FIFO/Socket/Char/Block） | Passthrough | 跳出链路收集，原 lexical 路径走 3.1.A 边界判定（Passthrough 或 Ask 按原有逻辑） |
| 工作区目录本身需要 realpath（macOS `/tmp`） | 可能误拒 | 启动时 realpath 缓存，双值匹配（不影响 outcome 类型，只防误拒） |

**Fail-Safe vs Fail-Open**：所有 IO 错误一律 Ask，**不允许** Allow/Passthrough。
与上游 `safeResolvePath` 的差异：上游 return original path → 让 `isPathAllowed`
多层规则兜底；我们在第一层直接 Ask（更保守），符合 AGENTS.md「安全功能必须 Fail-Safe」。

---

## 6. 推广路径（Phase 3.1.g 落地步骤）

| Step | 内容 | 估算 | 验证 |
|------|------|------|------|
| 6.1 | 引入 `FsResolver` trait + `NoopFsResolver` + `RealFsResolver`（POSIX 子模块），完整单元测试覆盖 T-SYM-1~T-SYM-4、T-SYM-7、T-SYM-8 | ~250 prod + ~300 test | 单元测试 + nextest |
| 6.2 | Windows 子模块：reparse-point + UNC pre-syscall 拒，补 T-SYM-6 测试（`cfg(target_os = "windows")`） | ~120 prod + ~80 test | CI 跑 windows job |
| 6.3 | workspace `realpath` 缓存（启动时一次），覆盖 T-SYM-5 macOS 误拒 | ~60 prod + ~40 test | nextest（mock FsResolver） |
| 6.4 | 3.1.B 切回写：`check_path_constraints` 主入口调用 `validate_command_paths_with_fs(.., &resolver)` | ~10 LOC diff | 现有 3.1.B 测试不应回归 |
| 6.5 | 3.1.C 切回写：新增 `DecisionReason::SymlinkEscape` + 审计日志契约 pin，对照 PR #560 同结构 | ~80 prod + ~150 test | snapshot 测试（insta） |
| 6.6 | E2E：BashTool 端到端 fixture（在临时 workspace 创建 PoC symlink + 实际跑 hook） | ~6 fixture | E2E job |

**Feature flag**：6.1~6.3 期间挂 `dasclaw_bash_validation/symlink-defense`
cargo feature，**默认开**（与 3.1.A 后默认行为不向后兼容声明在 §7）。

---

## 7. 兼容性 & 向后兼容声明

- **行为变更**：启用 `symlink-defense` 后，部分原本 Passthrough 的命令会
  变成 Ask（用户提示）。这是**安全提升**，但属于行为变更。
- **影响面**：仅影响 BashTool 路径敏感命令的**用户体验**（更多 prompt），
  不影响 API / 配置 schema。
- **回滚策略**：cargo feature 默认开，但用户可显式关闭（仅紧急回滚用途；
  CI 在 6.1 收尾前要求该 feature ON）。
- **drift guard**：`code-review-graph` 应当能捕获
  `validate_command_paths` 与 `validate_command_paths_with_fs` 调用方
  不一致的情况；6.4 切回写后 PR CI 跑一次 `detect-changes` 兜底。

---

## 8. 测试策略

| 层级 | 范围 | 工具 |
|------|------|------|
| 单元测试 | `FsResolver` mock，覆盖 §3.3 全部 T-SYM-* + Fail-Closed 五条路径 + max-depth | `nextest`，禁 `unwrap` |
| 集成测试 | `RealFsResolver` 真实临时目录 PoC（`tempfile`），macOS / Linux 双跑 | `nextest --features symlink-defense` |
| 平台测试 | Windows reparse-point + UNC，独立 CI job | CI Windows runner |
| 契约测试 | `DecisionReason::SymlinkEscape` 序列化形状，snapshot 固化 | `insta` |
| 安全审计 | 日志中不得包含 `/etc/passwd` 等敏感内容原文（只记 path + decision，不读 file 内容） | `tests/bash_validation_security_audit_tests.rs` |

**覆盖率目标**：
- 单元测试 失败路径 100%
- 平台分支 100%（POSIX + Windows 各自走完）
- `RealFsResolver::realpath` / `readlink_absolute` / `lstat_kind` 任一返回 None 的路径均必须有专用断言

---

## 9. 与上游差异声明

| 项 | 上游（pathValidation.ts / fsOperations.ts） | 本 ADR | 原因 |
|----|--------|--------|------|
| 错误时返回原 path | 是 | 否，返回 None → Ask | Fail-Safe（AGENTS.md 强制） |
| 用 `realpathSync` 也接受 ENOENT | 是 | 否，None → 走 `resolveDeepestExistingAncestor` 一次后仍 None 即 Ask | 同上 |
| FIFO/Socket/Char/Block 处理 | 早返回，不 realpath | 相同 | 避免 hang |
| max-depth | 40 | 40 | 对齐 |
| UNC | 早返回 | 早返回 + Windows 子模块 enforced | Windows 平台一致 |
| memoize | `lodash-es/memoize` | workspace `realpath` 用 `OnceLock`；逐路径解析**不缓存** | 路径数 ≪ 链长，缓存收益不显著且增加状态 |

---

## 10. 风险与缓解

| 风险 | 缓解 |
|------|------|
| `realpath` 在大型 workspace（深层 git）变慢 | 链长 ≤ 40 + 仅按需调用（命中 symlink 才走）；workspace 启动时单次缓存 |
| Windows reparse-point 行为差异（FILESYSTEM_FLAGS 等） | 单独 CI job + `RealFsResolver::lstat_kind` 平台子模块全 mock 覆盖 |
| Fail-Closed 引起过多用户 prompt | 配 feature flag；后续可演进出 SymlinkAllowlist（非本 ADR 范围） |
| 与 ADR-148 egress-gate / ADR-147 composite-safety-hook 顺序冲突 | 3.1.C 接线时显式声明 `BashPermissionHook` 内部顺序：拉 symlink 解析在 redirect 校验之前；写入 ADR-150 §6.5 |
| `code-review-graph` 未识别新 trait 引入 | 6.1 PR 必须本地跑 `detect-changes --base origin/xClaw` 并贴报告 |

---

## 11. 决策记录

- 2026-05-15 起草（本 ADR）；状态 Draft。
- 落地拆分见 §6；6.1 单独 sub-issue，**不与 3.1.B / 3.1.C 并发**（依赖
  3.1.A 已 merge）。
- 接受标准：上述全部 §8 测试通过 + ADR 状态变 Accepted + Phase 3.1 plan
  §7 后续表格回写 §6 行号。

---

## 12. Sources read

- `claude-code-main/src/utils/permissions/pathValidation.ts` §`safeResolvePath` 调用点、§`isPathAllowed` §3.7
- `claude-code-main/src/utils/fsOperations.ts:120-260` `safeResolvePath` / `resolveDeepestExistingAncestorSync`
- `claude-code-main/src/utils/fsOperations.ts:288-383` `getPathsForPermissionCheck`
- `crates/dasclaw_bash_validation/src/path_validation.rs:1-80, 665-760` 现 3.1.A 实现
- `docs/plans/bash-parity/phase-3.1-path-validation-deepening.md` §3 §5 §7
- ADR-112 §5、ADR-113 §1、ADR-145 §3、ADR-147、ADR-148
