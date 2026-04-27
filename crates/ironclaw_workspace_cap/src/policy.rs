//! Sandbox policy layer (W2.7).
//!
//! Ports codex's `SandboxPolicy` four-tier enum from
//! `codex-cli-main/codex-rs/protocol/src/protocol.rs` into a dependency-light
//! base crate. This module is **policy only** — it does not enforce anything
//! at runtime. Enforcement happens in:
//!
//! 1. The kernel-level [`crate::WorkspaceCapability`] (cap-std) — for the
//!    Rust process itself.
//! 2. `dasclaw_sandbox` (seccomp / Seatbelt / restricted token) — for child
//!    processes the agent spawns.
//!
//! W2.6 will wire `dasclaw_core` to consult this policy before every tool
//! invocation. W2 验收约束："WritableRoot 洞中洞契约测试 — sandbox 在
//! WorkspaceWrite 模式下写 `.git/hooks/*`、`.codex/*` 必须返回 SandboxError，
//! 且无任何 fallback。" 本模块负责 *决策* 那些路径不可写；强制是 sandbox 的
//! 职责。
//!
//! # 不在本模块范围（避免范围蔓延）
//!
//! - **TS / JsonSchema 导出**：留给 W2.6 的集成 crate（dasclaw_core 或 desktop-client
//!   facade），避免在底层 crate 引入 `ts-rs` / `schemars` 依赖。
//! - **git 指针文件解析**（worktree/submodule 的 `.git` 是文本指针）：codex
//!   有 `is_git_pointer_file` + `resolve_gitdir_from_file`，本切片仅保护
//!   `<root>/.git` 直接路径。worktree 场景在 W2.6 集成时按真实需求决定是否补。
//! - **运行时 cwd 自动注入**：调用方负责传入 `cwd` 到 [`SandboxPolicy::get_writable_roots_with_cwd`]，
//!   不在本模块抓取 `std::env::current_dir()`（避免被 race）。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 出站网络访问开关。
///
/// `ExternalSandbox` 用此枚举（独立类型让外部沙箱清楚指明意图），
/// `ReadOnly` / `WorkspaceWrite` 用 `bool` 与 codex 历史 wire format 对齐。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkAccess {
    /// 受限：拒绝出站网络。
    #[default]
    Restricted,
    /// 启用：允许出站网络。
    Enabled,
}

impl NetworkAccess {
    /// 出站网络是否启用。
    pub fn is_enabled(self) -> bool {
        matches!(self, NetworkAccess::Enabled)
    }
}

/// 沙箱策略四档（与 codex `SandboxPolicy` 序列化兼容）。
///
/// JSON wire format（serde tag = "type"，kebab-case）：
///
/// ```json
/// { "type": "danger-full-access" }
/// { "type": "read-only", "network_access": false }
/// { "type": "external-sandbox", "network_access": "restricted" }
/// { "type": "workspace-write", "writable_roots": ["/extra/path"], "network_access": false,
///   "exclude_tmpdir_env_var": false, "exclude_slash_tmp": false }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SandboxPolicy {
    /// **完全无限制**。仅供开发/调试或显式 trusted 场景使用。
    #[serde(rename = "danger-full-access")]
    DangerFullAccess,

    /// 只读：禁止任何文件写入；网络可选。
    #[serde(rename = "read-only")]
    ReadOnly {
        /// 是否允许出站网络。默认 `false`。
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        network_access: bool,
    },

    /// 进程已在外部沙箱（Docker、Firejail 等）内，宿主层信任外部隔离。
    #[serde(rename = "external-sandbox")]
    ExternalSandbox {
        /// 外部沙箱是否允许网络。
        #[serde(default)]
        network_access: NetworkAccess,
    },

    /// 工作区可写：默认 cwd + `/tmp`（Unix）+ `$TMPDIR` + 显式 `writable_roots`，
    /// 同时**洞中洞**保护 `.git/`、`.codex/`、`.agents/` 等敏感子路径。
    #[serde(rename = "workspace-write")]
    WorkspaceWrite {
        /// cwd 之外额外的可写根。
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        writable_roots: Vec<PathBuf>,

        /// 是否允许出站网络。默认 `false`。
        #[serde(default)]
        network_access: bool,

        /// `true` 时**不**把 `$TMPDIR` 加入默认可写根。
        #[serde(default)]
        exclude_tmpdir_env_var: bool,

        /// `true` 时**不**把 `/tmp`（Unix）加入默认可写根。
        #[serde(default)]
        exclude_slash_tmp: bool,
    },
}

/// 一个可写根 + 它内部应保持只读的子路径列表（"洞中洞"）。
///
/// 例：cwd 是 `/repo` 时，`/repo/.git/hooks/pre-commit` 必须**不可写**，
/// 否则 agent 改 hook 后下次 commit 就能在用户机器上 RCE。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WritableRoot {
    /// 可写根的绝对路径。
    pub root: PathBuf,
    /// 在此 root 下应保持只读的子路径（绝对路径，构造时已确认是 `root` 的后代）。
    pub read_only_subpaths: Vec<PathBuf>,
}

impl WritableRoot {
    /// 判断 `path` 在本可写根策略下是否可写。
    ///
    /// 规则：
    /// 1. `path` 不是 `root` 的后代 → false
    /// 2. `path` 是任何 `read_only_subpaths[i]` 的后代（含等于） → false
    /// 3. 否则 → true
    pub fn is_path_writable(&self, path: &Path) -> bool {
        if !path.starts_with(&self.root) {
            return false;
        }
        for ro in &self.read_only_subpaths {
            if path.starts_with(ro) {
                return false;
            }
        }
        true
    }
}

impl SandboxPolicy {
    /// 默认只读策略：禁网络。
    pub fn new_read_only_policy() -> Self {
        SandboxPolicy::ReadOnly {
            network_access: false,
        }
    }

    /// 默认 workspace-write 策略：禁网络，自动 `/tmp` + `$TMPDIR`。
    pub fn new_workspace_write_policy() -> Self {
        SandboxPolicy::WorkspaceWrite {
            writable_roots: Vec::new(),
            network_access: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        }
    }

    /// 是否允许全盘**读**。当前所有四档都允许（保持 codex parity）— 收紧
    /// 读权限是上层 cap-std 层的职责。
    pub fn has_full_disk_read_access(&self) -> bool {
        true
    }

    /// 是否允许全盘**写**（不受 cwd 限制）。仅 `DangerFullAccess` 与
    /// `ExternalSandbox` 返回 true。`WorkspaceWrite` 受 cwd + writable_roots
    /// 限制，需用 [`Self::is_path_writable`] 决策具体路径。
    pub fn has_full_disk_write_access(&self) -> bool {
        matches!(
            self,
            SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
        )
    }

    /// 是否允许出站网络。
    pub fn has_full_network_access(&self) -> bool {
        match self {
            SandboxPolicy::DangerFullAccess => true,
            SandboxPolicy::ExternalSandbox { network_access } => network_access.is_enabled(),
            SandboxPolicy::ReadOnly { network_access } => *network_access,
            SandboxPolicy::WorkspaceWrite { network_access, .. } => *network_access,
        }
    }

    /// 计算**实际可写根列表**（含洞中洞），基于运行时 `cwd`。
    ///
    /// 仅 `WorkspaceWrite` 返回非空；其余三档返回空 `Vec`。空不代表"无写
    /// 权限"，调用方应配合 [`Self::has_full_disk_write_access`] 综合判断。
    pub fn get_writable_roots_with_cwd(&self, cwd: &Path) -> Vec<WritableRoot> {
        let SandboxPolicy::WorkspaceWrite {
            writable_roots,
            exclude_tmpdir_env_var,
            exclude_slash_tmp,
            network_access: _,
        } = self
        else {
            return Vec::new();
        };

        let mut roots: Vec<PathBuf> = writable_roots.to_vec();
        roots.push(cwd.to_path_buf());

        if cfg!(unix) && !exclude_slash_tmp {
            let slash_tmp = PathBuf::from("/tmp");
            if slash_tmp.is_dir() {
                roots.push(slash_tmp);
            }
        }

        if !exclude_tmpdir_env_var
            && let Some(tmpdir) = std::env::var_os("TMPDIR")
            && !tmpdir.is_empty()
        {
            let path = PathBuf::from(&tmpdir);
            if path.is_absolute() {
                roots.push(path);
            }
        }

        roots
            .into_iter()
            .map(|root| {
                let protect_missing_dot_codex = root == cwd;
                let read_only_subpaths =
                    default_read_only_subpaths_for_writable_root(&root, protect_missing_dot_codex);
                WritableRoot {
                    root,
                    read_only_subpaths,
                }
            })
            .collect()
    }

    /// **最高级决策 API**：给定 `path` 与 `cwd`，返回是否允许写入。
    ///
    /// 此方法是 sandbox enforcer / dasclaw_core hook 的主要入口。决策树：
    ///
    /// 1. `DangerFullAccess` / `ExternalSandbox` → 任何 `path` 都允许
    /// 2. `ReadOnly` → 全部拒绝
    /// 3. `WorkspaceWrite` → 综合所有可写根：
    ///    - **一票否决**：任何 root 的 `read_only_subpaths` 命中 `path`，立即拒绝
    ///      （防止 macOS `cwd` 嵌套在 `$TMPDIR` 下时，TMPDIR root 不感知
    ///      cwd 内 `.git` 而错误放行）
    ///    - 否则要求 `path` 至少落在一个 root 内
    pub fn is_path_writable(&self, path: &Path, cwd: &Path) -> bool {
        if self.has_full_disk_write_access() {
            return true;
        }
        if matches!(self, SandboxPolicy::ReadOnly { .. }) {
            return false;
        }

        let roots = self.get_writable_roots_with_cwd(cwd);

        // 一票否决：任何 root 把 path 列为只读子路径，全局拒绝。
        for wr in &roots {
            for ro in &wr.read_only_subpaths {
                if path.starts_with(ro) {
                    return false;
                }
            }
        }

        // 必须落在至少一个 root 内才允许写。
        roots.iter().any(|wr| path.starts_with(&wr.root))
    }
}

/// 计算指定可写根下应保持只读的子路径（"洞中洞"）。
///
/// 默认保护：
/// - `<root>/.git`：防 agent 改 hook 后 commit 时 RCE
/// - `<root>/.agents`：codex 的 agent 描述文件目录
/// - `<root>/.codex`：codex 项目元数据；`protect_missing_dot_codex=true` 时
///   即使目录不存在也保护（防首次创建绕过审批流）
///
/// 不像 codex 那样解析 `.git` 文本指针文件（worktree/submodule 场景）—
/// 当前切片仅保护直接路径。worktree 增强留给 W2.6 集成 dasclaw_core 时按
/// 真实场景需求增量补。
pub fn default_read_only_subpaths_for_writable_root(
    writable_root: &Path,
    protect_missing_dot_codex: bool,
) -> Vec<PathBuf> {
    let mut subpaths: Vec<PathBuf> = Vec::new();

    let dot_git = writable_root.join(".git");
    if dot_git.is_dir() || dot_git.is_file() {
        subpaths.push(dot_git);
    }

    let dot_agents = writable_root.join(".agents");
    if dot_agents.is_dir() {
        subpaths.push(dot_agents);
    }

    let dot_codex = writable_root.join(".codex");
    if protect_missing_dot_codex || dot_codex.is_dir() {
        subpaths.push(dot_codex);
    }

    // dedup（保持顺序）。codex 同等逻辑。
    let mut seen = std::collections::HashSet::new();
    subpaths.retain(|p| seen.insert(p.clone()));
    subpaths
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // ---------- enum helpers ----------

    #[test]
    fn network_access_default_is_restricted() {
        assert_eq!(NetworkAccess::default(), NetworkAccess::Restricted);
        assert!(!NetworkAccess::default().is_enabled());
        assert!(NetworkAccess::Enabled.is_enabled());
    }

    #[test]
    fn read_only_policy_constructor_disables_network() {
        let p = SandboxPolicy::new_read_only_policy();
        assert!(matches!(
            p,
            SandboxPolicy::ReadOnly {
                network_access: false
            }
        ));
        assert!(!p.has_full_network_access());
    }

    #[test]
    fn workspace_write_policy_constructor_defaults() {
        let p = SandboxPolicy::new_workspace_write_policy();
        match p {
            SandboxPolicy::WorkspaceWrite {
                writable_roots,
                network_access,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
            } => {
                assert!(writable_roots.is_empty());
                assert!(!network_access);
                assert!(!exclude_tmpdir_env_var);
                assert!(!exclude_slash_tmp);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn full_disk_write_access_only_for_danger_or_external() {
        assert!(SandboxPolicy::DangerFullAccess.has_full_disk_write_access());
        assert!(
            SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Restricted
            }
            .has_full_disk_write_access()
        );
        assert!(!SandboxPolicy::new_read_only_policy().has_full_disk_write_access());
        assert!(!SandboxPolicy::new_workspace_write_policy().has_full_disk_write_access());
    }

    #[test]
    fn full_network_access_per_variant() {
        assert!(SandboxPolicy::DangerFullAccess.has_full_network_access());
        assert!(
            SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Enabled
            }
            .has_full_network_access()
        );
        assert!(
            !SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Restricted
            }
            .has_full_network_access()
        );
        assert!(
            SandboxPolicy::ReadOnly {
                network_access: true
            }
            .has_full_network_access()
        );
        assert!(
            !SandboxPolicy::ReadOnly {
                network_access: false
            }
            .has_full_network_access()
        );
    }

    // ---------- serde wire format ----------

    #[test]
    fn serde_round_trip_all_variants() {
        let cases = [
            SandboxPolicy::DangerFullAccess,
            SandboxPolicy::ReadOnly {
                network_access: false,
            },
            SandboxPolicy::ReadOnly {
                network_access: true,
            },
            SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Restricted,
            },
            SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Enabled,
            },
            SandboxPolicy::new_workspace_write_policy(),
            SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![PathBuf::from("/extra")],
                network_access: true,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            },
        ];
        for p in cases {
            let j = serde_json::to_string(&p).expect("serialize");
            let back: SandboxPolicy = serde_json::from_str(&j).expect("deserialize");
            assert_eq!(p, back, "round trip for {j}");
        }
    }

    #[test]
    fn serde_uses_kebab_case_type_tag() {
        let j = serde_json::to_string(&SandboxPolicy::DangerFullAccess).unwrap();
        assert!(j.contains("\"danger-full-access\""), "got {j}");

        let j = serde_json::to_string(&SandboxPolicy::ExternalSandbox {
            network_access: NetworkAccess::Restricted,
        })
        .unwrap();
        assert!(j.contains("\"external-sandbox\""), "got {j}");
        assert!(j.contains("\"restricted\""), "got {j}");

        let j = serde_json::to_string(&SandboxPolicy::new_workspace_write_policy()).unwrap();
        assert!(j.contains("\"workspace-write\""), "got {j}");
    }

    // ---------- WritableRoot core logic ----------

    #[test]
    fn writable_root_rejects_path_outside_root() {
        let wr = WritableRoot {
            root: PathBuf::from("/work"),
            read_only_subpaths: vec![],
        };
        assert!(!wr.is_path_writable(Path::new("/etc/passwd")));
        assert!(!wr.is_path_writable(Path::new("/work2/file"))); // sibling
    }

    #[test]
    fn writable_root_accepts_path_inside_root() {
        let wr = WritableRoot {
            root: PathBuf::from("/work"),
            read_only_subpaths: vec![],
        };
        assert!(wr.is_path_writable(Path::new("/work/src/main.rs")));
        assert!(wr.is_path_writable(Path::new("/work"))); // root itself
    }

    #[test]
    fn writable_root_blocks_read_only_subpath() {
        let wr = WritableRoot {
            root: PathBuf::from("/work"),
            read_only_subpaths: vec![PathBuf::from("/work/.git"), PathBuf::from("/work/.codex")],
        };
        // 洞中洞
        assert!(!wr.is_path_writable(Path::new("/work/.git/hooks/pre-commit")));
        assert!(!wr.is_path_writable(Path::new("/work/.git/config")));
        assert!(!wr.is_path_writable(Path::new("/work/.codex/instructions.md")));
        // 同名前缀但不是子路径不应误判 — `.gitignore` 不是 `.git/` 后代
        assert!(wr.is_path_writable(Path::new("/work/.gitignore")));
        // 旁边的目录可写
        assert!(wr.is_path_writable(Path::new("/work/src/main.rs")));
    }

    // ---------- 洞中洞默认计算 ----------

    #[test]
    fn default_read_only_subpaths_protects_existing_dot_dirs() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(".git")).unwrap();
        std::fs::create_dir(tmp.path().join(".agents")).unwrap();
        std::fs::create_dir(tmp.path().join(".codex")).unwrap();

        let subs = default_read_only_subpaths_for_writable_root(tmp.path(), false);
        assert!(subs.contains(&tmp.path().join(".git")));
        assert!(subs.contains(&tmp.path().join(".agents")));
        assert!(subs.contains(&tmp.path().join(".codex")));
    }

    #[test]
    fn default_read_only_subpaths_skips_missing_dirs_unless_dot_codex_protected() {
        let tmp = TempDir::new().unwrap();
        // 啥都没创建。
        let subs_unprotected =
            default_read_only_subpaths_for_writable_root(tmp.path(), /* dot_codex */ false);
        assert!(subs_unprotected.is_empty(), "got {subs_unprotected:?}");

        // protect_missing_dot_codex=true 即使 .codex 不存在也加入保护清单
        // （首次创建走审批流）。
        let subs_protected = default_read_only_subpaths_for_writable_root(tmp.path(), true);
        assert_eq!(subs_protected, vec![tmp.path().join(".codex")]);
    }

    #[test]
    fn default_read_only_subpaths_dedup() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(".codex")).unwrap();
        // 调两次相同路径不应产生重复。
        let mut a = default_read_only_subpaths_for_writable_root(tmp.path(), true);
        let b = default_read_only_subpaths_for_writable_root(tmp.path(), true);
        assert_eq!(a, b);
        a.extend(b);
        // dedup 内置在函数里，组合的 a 自带已含 .codex 一次（因为函数自身去重，
        // extend 后会变两份 — 这里只是验证函数本身返回去重后的结果）。
        assert_eq!(
            default_read_only_subpaths_for_writable_root(tmp.path(), true).len(),
            1
        );
    }

    // ---------- 整合契约：is_path_writable end-to-end ----------

    #[test]
    fn danger_full_access_writes_anywhere() {
        let p = SandboxPolicy::DangerFullAccess;
        let cwd = Path::new("/repo");
        assert!(p.is_path_writable(Path::new("/repo/main.rs"), cwd));
        assert!(p.is_path_writable(Path::new("/etc/passwd"), cwd));
    }

    #[test]
    fn read_only_blocks_all_writes() {
        let p = SandboxPolicy::new_read_only_policy();
        let cwd = Path::new("/repo");
        assert!(!p.is_path_writable(Path::new("/repo/main.rs"), cwd));
        assert!(!p.is_path_writable(Path::new("/tmp/work"), cwd));
    }

    #[test]
    fn external_sandbox_writes_anywhere() {
        let p = SandboxPolicy::ExternalSandbox {
            network_access: NetworkAccess::Restricted,
        };
        let cwd = Path::new("/repo");
        assert!(p.is_path_writable(Path::new("/etc/hosts"), cwd));
    }

    /// **W2 验收契约**："在 WorkspaceWrite 模式下写 `.git/hooks/*`、
    /// `.codex/*` 必须返回 SandboxError，且无任何 fallback。"
    /// 本测试验证策略层正确地决策了"不可写"。
    #[test]
    fn workspace_write_blocks_dot_git_hooks_under_cwd() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path();
        std::fs::create_dir_all(cwd.join(".git/hooks")).unwrap();

        let p = SandboxPolicy::new_workspace_write_policy();

        // 验收契约：cwd 内文件可写
        assert!(p.is_path_writable(&cwd.join("src/main.rs"), cwd));

        // 验收契约：洞中洞拒绝
        assert!(!p.is_path_writable(&cwd.join(".git/hooks/pre-commit"), cwd));
        assert!(!p.is_path_writable(&cwd.join(".git/config"), cwd));
        assert!(!p.is_path_writable(&cwd.join(".git"), cwd));

        // .codex 即使不存在也保护（protect_missing_dot_codex=true 因 root==cwd）
        assert!(!p.is_path_writable(&cwd.join(".codex/secrets.toml"), cwd));
    }

    /// **跨 root 一票否决回归测试**。在 macOS 上 `tempfile` 创建的 cwd
    /// 通常落在 `$TMPDIR`（`/var/folders/...`）之下；如果 `is_path_writable`
    /// 用 `.any()` 在每个 root 视角独立决策，TMPDIR root 看不到 cwd 内的
    /// `.git`，会把 `.git/hooks/*` 错误放行。本测试锁死正确语义：
    /// **任何 root 列出的洞中洞，全局都不可写**。
    #[test]
    fn workspace_write_dot_git_blocked_even_when_cwd_under_tmpdir() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path();
        std::fs::create_dir_all(cwd.join(".git/hooks")).unwrap();

        // 显式制造 "cwd 在 TMPDIR 下" 的局面：把 TMPDIR 指向 tempdir 父目录。
        // 我们用 unsafe { set_var } 因为本测试是 single-threaded，
        // 且函数文档允许在测试初始化阶段调用。
        let original_tmpdir = std::env::var_os("TMPDIR");
        let parent = cwd.parent().expect("tempdir has parent");
        // SAFETY: tests run in a single thread by default for this module
        // (cargo test 用同一进程多线程，但 std::env 在测试间同步可能 race；
        // 这里仅写一次再读一次，最坏情况是其他线程的 SandboxPolicy 短暂
        // 看到此 TMPDIR — 不影响其他测试的断言因为它们用独立 cwd)。
        unsafe {
            std::env::set_var("TMPDIR", parent);
        }

        let p = SandboxPolicy::new_workspace_write_policy();
        let writable = p.is_path_writable(&cwd.join(".git/hooks/pre-commit"), cwd);

        // 还原 TMPDIR 避免污染其他测试。
        unsafe {
            match original_tmpdir {
                Some(v) => std::env::set_var("TMPDIR", v),
                None => std::env::remove_var("TMPDIR"),
            }
        }

        assert!(
            !writable,
            ".git/hooks/pre-commit must be blocked even when cwd ⊂ TMPDIR"
        );
    }

    #[test]
    fn workspace_write_allows_explicit_extra_root_but_blocks_outside() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path().join("project");
        let extra = tmp.path().join("artifacts");
        std::fs::create_dir(&cwd).unwrap();
        std::fs::create_dir(&extra).unwrap();

        let p = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![extra.clone()],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        assert!(p.is_path_writable(&cwd.join("a.rs"), &cwd));
        assert!(p.is_path_writable(&extra.join("out.bin"), &cwd));

        let outside = tmp.path().join("other");
        std::fs::create_dir(&outside).unwrap();
        assert!(!p.is_path_writable(&outside.join("evil"), &cwd));
    }

    #[test]
    fn workspace_write_excludes_slash_tmp_when_requested() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path();

        let slash_tmp = Path::new("/tmp");
        // exclude=false（默认）→ 包含 /tmp（仅 Unix）
        let p_default = SandboxPolicy::new_workspace_write_policy();
        let roots = p_default.get_writable_roots_with_cwd(cwd);
        let has_tmp_default = roots.iter().any(|r| r.root == slash_tmp);

        // exclude=true → 不包含 /tmp
        let p_excluded = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };
        let roots_excluded = p_excluded.get_writable_roots_with_cwd(cwd);
        let has_tmp_excluded = roots_excluded.iter().any(|r| r.root == slash_tmp);

        if cfg!(unix) {
            // /tmp 通常存在；exclude=false 应包含，exclude=true 应不包含
            if Path::new("/tmp").is_dir() {
                assert!(
                    has_tmp_default,
                    "default policy should include /tmp on unix"
                );
            }
            assert!(!has_tmp_excluded, "exclude_slash_tmp must drop /tmp");
        } else {
            // 非 Unix 平台两个策略都不该有 /tmp
            assert!(!has_tmp_default);
            assert!(!has_tmp_excluded);
        }
    }
}
