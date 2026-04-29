//! Bash command semantic validator — Layer 2 security analysis.
//!
//! Supplements the existing ShellTool Layer 1 checks (blocked commands,
//! dangerous patterns, injection detection) with a 5-stage semantic
//! validation pipeline:
//!
//! 1. **Read-only detection** — identifies purely read-only commands for risk downgrade
//! 2. **Destructive command warning** — flags rm -rf, dd, mkfs, etc.
//! 3. **Path validation** — detects traversal and out-of-workspace references
//! 4. **Command intent classification** — categorizes into 8 semantic intents
//! 5. **sed validation** — special handling for sed -i in-place editing

use std::path::Path;

use crate::tools::tool::RiskLevel;

// ─── Types ──────────────────────────────────────────────────────────────

/// Semantic classification of a bash command's intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandIntent {
    /// Read-only operations: ls, cat, grep, find, etc.
    ReadOnly,
    /// File system writes: cp, mv, mkdir, touch, etc.
    Write,
    /// Destructive operations: rm -rf, shred, etc.
    Destructive,
    /// Network operations: curl, wget, ssh, etc.
    Network,
    /// Process management: kill, pkill, etc.
    ProcessManagement,
    /// Package management: apt, brew, pip, npm, etc.
    PackageManagement,
    /// System administration: sudo, chmod, mount, etc.
    SystemAdmin,
    /// Unknown or unclassifiable command.
    Unknown,
}

impl std::fmt::Display for CommandIntent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadOnly => write!(f, "read-only"),
            Self::Write => write!(f, "write"),
            Self::Destructive => write!(f, "destructive"),
            Self::Network => write!(f, "network"),
            Self::ProcessManagement => write!(f, "process-management"),
            Self::PackageManagement => write!(f, "package-management"),
            Self::SystemAdmin => write!(f, "system-admin"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// A warning produced during validation.
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub stage: &'static str,
    pub message: String,
}

/// Result of the 5-stage bash validation pipeline.
#[derive(Debug)]
pub struct BashValidationResult {
    pub intent: CommandIntent,
    pub risk_level: RiskLevel,
    pub warnings: Vec<ValidationWarning>,
}

// ─── Command Classification Constants ───────────────────────────────────

/// Commands that are strictly read-only (no filesystem or state modification).
const READ_ONLY_COMMANDS: &[&str] = &[
    "ls",
    "ll",
    "la",
    "dir",
    "cat",
    "head",
    "tail",
    "less",
    "more",
    "wc",
    "sort",
    "uniq",
    "grep",
    "egrep",
    "fgrep",
    "rg",
    "ag",
    "fd",
    "find",
    "which",
    "whereis",
    "whatis",
    "man",
    "info",
    "file",
    "stat",
    "du",
    "df",
    "free",
    "uptime",
    "uname",
    "hostname",
    "whoami",
    "id",
    "groups",
    "env",
    "printenv",
    "echo",
    "printf",
    "date",
    "cal",
    "bc",
    "expr",
    "test",
    "true",
    "false",
    "pwd",
    "tree",
    "diff",
    "cmp",
    "md5sum",
    "sha256sum",
    "sha1sum",
    "xxd",
    "od",
    "hexdump",
    "strings",
    "readlink",
    "realpath",
    "basename",
    "dirname",
    "seq",
    "tput",
    "column",
    "jq",
    "yq",
    "xargs",
    "tr",
    "cut",
    "paste",
    "awk",
    "sed",
    "locate",
    "type",
    // Process viewers (read-only, not process-modifying)
    "ps",
    "top",
    "htop",
    "jobs",
];

/// Commands that perform filesystem writes.
const WRITE_COMMANDS: &[&str] = &[
    "cp", "mv", "rm", "mkdir", "rmdir", "touch", "chmod", "chown", "chgrp", "ln", "install", "tee",
    "truncate", "mkfifo", "mknod", "dd",
];

/// Commands that are always destructive regardless of arguments.
const DESTRUCTIVE_COMMANDS: &[&str] = &["shred", "wipefs"];

/// Patterns indicating destructive operations (substring match).
const DESTRUCTIVE_PATTERNS: &[(&str, &str)] = &[
    ("rm -rf /", "Recursive forced deletion at root"),
    ("rm -rf ~", "Recursive forced deletion of home directory"),
    ("rm -rf *", "Recursive forced deletion in current directory"),
    ("rm -rf .", "Recursive forced deletion of current directory"),
    ("mkfs", "Filesystem creation destroys existing data"),
    ("dd if=", "Direct disk write — can overwrite partitions"),
    ("> /dev/sd", "Writing to raw disk device"),
    (
        "chmod -R 777",
        "Recursively setting world-writable permissions",
    ),
    ("chmod -R 000", "Recursively removing all permissions"),
    (":(){ :|:& };:", "Fork bomb"),
];

/// Commands that perform network operations.
const NETWORK_COMMANDS: &[&str] = &[
    "curl",
    "wget",
    "ssh",
    "scp",
    "rsync",
    "ftp",
    "sftp",
    "nc",
    "ncat",
    "telnet",
    "ping",
    "traceroute",
    "dig",
    "nslookup",
    "host",
    "whois",
    "ifconfig",
    "ip",
    "netstat",
    "ss",
    "nmap",
];

/// Commands that manage processes (modifying, not viewing).
const PROCESS_COMMANDS: &[&str] = &[
    "kill", "pkill", "killall", "bg", "fg", "nohup", "disown", "wait", "nice", "renice",
];

/// Commands that manage packages.
const PACKAGE_COMMANDS: &[&str] = &[
    "apt", "apt-get", "yum", "dnf", "pacman", "brew", "pip", "pip3", "npm", "yarn", "pnpm", "bun",
    "cargo", "gem", "go", "rustup", "snap", "flatpak",
];

/// Commands that require system administrator privileges.
const SYSTEM_ADMIN_COMMANDS: &[&str] = &[
    "sudo",
    "su",
    "chroot",
    "mount",
    "umount",
    "fdisk",
    "parted",
    // NOTE: `lsblk` and `blkid` are read-only block-device inspectors and are
    // intentionally NOT in this list. Treating them as SystemAdmin produced High
    // risk classifications for harmless inspection commands and conflicted with
    // ShellTool's pattern table (see shell_risk_regression::word_boundary_no_false_positives).
    "blkid",
    "systemctl",
    "service",
    "journalctl",
    "dmesg",
    "modprobe",
    "insmod",
    "rmmod",
    "iptables",
    "ufw",
    "firewall-cmd",
    "sysctl",
    "crontab",
    "at",
    "useradd",
    "userdel",
    "usermod",
    "groupadd",
    "groupdel",
    "passwd",
    "visudo",
];

/// Git subcommands that are read-only safe.
const GIT_READ_ONLY_SUBS: &[&str] = &[
    "status",
    "log",
    "diff",
    "show",
    "branch",
    "tag",
    "stash",
    "remote",
    "fetch",
    "ls-files",
    "ls-tree",
    "cat-file",
    "rev-parse",
    "describe",
    "shortlog",
    "blame",
    "bisect",
    "reflog",
];

/// System paths that workspace-scoped write commands should not target.
const SYSTEM_PATHS: &[&str] = &[
    "/etc/", "/usr/", "/var/", "/boot/", "/sys/", "/proc/", "/dev/", "/sbin/", "/lib/", "/opt/",
];

// ─── Stage 1: Read-only detection ───────────────────────────────────────
// Read-only detection is integrated into Stage 4 (classify_by_token +
// classify_read_only_variant). Commands matching READ_ONLY_COMMANDS get
// CommandIntent::ReadOnly → RiskLevel::Low automatically.

fn is_git_read_only(cmd: &str) -> bool {
    let sub = cmd.split_whitespace().skip(1).find(|p| !p.starts_with('-'));
    matches!(sub, Some(s) if GIT_READ_ONLY_SUBS.contains(&s))
}

// ─── Stage 2: Destructive command check ─────────────────────────────────

fn check_destructive(cmd: &str, first: &str) -> Option<ValidationWarning> {
    for &(pattern, desc) in DESTRUCTIVE_PATTERNS {
        if cmd.contains(pattern) {
            return Some(ValidationWarning {
                stage: "destructive",
                message: desc.to_string(),
            });
        }
    }
    if DESTRUCTIVE_COMMANDS.contains(&first) {
        return Some(ValidationWarning {
            stage: "destructive",
            message: format!("'{first}' is inherently destructive"),
        });
    }
    // General rm -rf detection beyond specific patterns above
    if first == "rm" && has_force_recursive(cmd) {
        return Some(ValidationWarning {
            stage: "destructive",
            message: "Recursive forced deletion — verify target path".to_string(),
        });
    }
    None
}

// ─── Stage 3: Path validation ───────────────────────────────────────────

fn check_paths(cmd: &str, first: &str, workspace: &Path) -> Vec<ValidationWarning> {
    let mut warnings = Vec::new();

    if cmd.contains("../") {
        let ws = workspace.to_string_lossy();
        if !cmd.contains(&*ws) {
            warnings.push(ValidationWarning {
                stage: "path",
                message: "Contains '../' traversal — verify path resolves within workspace"
                    .to_string(),
            });
        }
    }

    if cmd.contains("~/") || cmd.contains("$HOME") {
        warnings.push(ValidationWarning {
            stage: "path",
            message: "References home directory — verify it stays within workspace".to_string(),
        });
    }

    if WRITE_COMMANDS.contains(&first) {
        for sys_path in SYSTEM_PATHS {
            if cmd.contains(sys_path) {
                warnings.push(ValidationWarning {
                    stage: "path",
                    message: format!("Write command targets system path '{sys_path}'"),
                });
                break;
            }
        }
    }

    warnings
}

// ─── Stage 4: Intent classification ─────────────────────────────────────

/// Classify the semantic intent of a bash command (pipeline-aware).
///
/// For pipelines (`cmd1 | cmd2 && cmd3`), returns the highest-risk intent
/// across all segments.
pub fn classify_intent(cmd: &str) -> CommandIntent {
    cmd.split(['|', '&', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|seg| {
            let first = extract_first_command(seg);
            classify_by_token(&first, seg)
        })
        .max_by_key(|intent| intent_to_risk_level(*intent))
        .unwrap_or(CommandIntent::Unknown)
}

fn classify_by_token(first: &str, cmd: &str) -> CommandIntent {
    if READ_ONLY_COMMANDS.contains(&first) {
        return classify_read_only_variant(first, cmd);
    }
    if DESTRUCTIVE_COMMANDS.contains(&first) {
        return CommandIntent::Destructive;
    }
    if WRITE_COMMANDS.contains(&first) {
        return classify_write_variant(first, cmd);
    }
    if NETWORK_COMMANDS.contains(&first) {
        return CommandIntent::Network;
    }
    if PROCESS_COMMANDS.contains(&first) {
        return CommandIntent::ProcessManagement;
    }
    // Build tools with package management capabilities: sub-classify by subcommand
    if matches!(
        first,
        "cargo" | "npm" | "yarn" | "pnpm" | "bun" | "pip" | "pip3" | "go"
    ) {
        return classify_build_tool_intent(first, cmd);
    }
    if PACKAGE_COMMANDS.contains(&first) {
        return CommandIntent::PackageManagement;
    }
    if SYSTEM_ADMIN_COMMANDS.contains(&first) {
        return CommandIntent::SystemAdmin;
    }
    if first == "git" {
        return if is_git_read_only(cmd) {
            CommandIntent::ReadOnly
        } else {
            CommandIntent::Write
        };
    }
    CommandIntent::Unknown
}

/// Distinguish read-only commands that have write modes (sed -i, awk -i, find -delete).
fn classify_read_only_variant(first: &str, cmd: &str) -> CommandIntent {
    if (first == "sed" || first == "awk") && cmd.contains(" -i") {
        return CommandIntent::Write;
    }
    // `find -delete` removes matched files. Treated as Write (reversible scope, bounded
    // by the find expression) rather than Destructive, to align with ShellTool's pattern
    // table (`find` is in MEDIUM_RISK_PATTERNS). Truly destructive variants (e.g. paired
    // with `rm -rf` via -exec) are caught below.
    if first == "find" && cmd.contains("-delete") {
        return CommandIntent::Write;
    }
    if first == "find" && cmd.contains("-exec") {
        return CommandIntent::Write;
    }
    CommandIntent::ReadOnly
}

/// Distinguish write commands: rm with -rf is Destructive, others are Write.
fn classify_write_variant(first: &str, cmd: &str) -> CommandIntent {
    if first == "rm" && has_force_recursive(cmd) {
        return CommandIntent::Destructive;
    }
    CommandIntent::Write
}

/// Sub-classify build tools that also do package management (cargo, npm, go, etc.).
///
/// Development subcommands (build, test, run) → Write (Medium).
/// Package operations (install, add, uninstall) → PackageManagement (High).
fn classify_build_tool_intent(first: &str, cmd: &str) -> CommandIntent {
    let sub = cmd.split_whitespace().skip(1).find(|p| !p.starts_with('-'));
    let sub = sub.unwrap_or("");

    match first {
        "cargo" => match sub {
            // Read-only analysis subcommands — no filesystem mutation.
            "check" | "clippy" => CommandIntent::ReadOnly,
            "build" | "test" | "run" | "bench" | "doc" | "fmt" => CommandIntent::Write,
            _ => CommandIntent::PackageManagement,
        },
        "npm" | "yarn" | "pnpm" | "bun" => match sub {
            "test" | "run" | "start" | "dev" | "build" | "lint" | "exec" => CommandIntent::Write,
            // Reversible package operations: install/uninstall/add/remove/update/ci.
            // These mutate node_modules / lockfile but are recoverable, so they map to
            // Write (Medium) — matching ShellTool's MEDIUM_RISK_PATTERNS contract
            // (see shell_risk_regression::medium_risk_commands).
            "install" | "uninstall" | "add" | "remove" | "update" | "ci" => {
                CommandIntent::Write
            }
            _ => CommandIntent::PackageManagement,
        },
        "pip" | "pip3" => match sub {
            "list" | "show" | "freeze" | "check" => CommandIntent::ReadOnly,
            _ => CommandIntent::PackageManagement,
        },
        "go" => match sub {
            "build" | "test" | "run" | "vet" | "fmt" | "generate" => CommandIntent::Write,
            "list" | "doc" | "env" | "version" => CommandIntent::ReadOnly,
            _ => CommandIntent::PackageManagement,
        },
        _ => CommandIntent::PackageManagement,
    }
}

/// Check if a command has both -r and -f flags (combined or separate).
fn has_force_recursive(cmd: &str) -> bool {
    // Combined flags: -rf, -fr (also catches -rfi, -fri, etc.)
    if cmd.contains("-rf") || cmd.contains("-fr") {
        return true;
    }
    // Separate flags: -r -f
    let args: Vec<&str> = cmd.split_whitespace().collect();
    args.iter().any(|&a| a == "-r") && args.iter().any(|&a| a == "-f")
}

// ─── Stage 5: sed validation ────────────────────────────────────────────

fn check_sed(cmd: &str, first: &str) -> Option<ValidationWarning> {
    if first != "sed" {
        return None;
    }
    if cmd.contains(" -i") {
        return Some(ValidationWarning {
            stage: "sed",
            message: "sed -i performs in-place file modification".to_string(),
        });
    }
    None
}

// ─── Risk Mapping ───────────────────────────────────────────────────────

/// Map a [`CommandIntent`] to an ironclaw [`RiskLevel`].
pub fn intent_to_risk_level(intent: CommandIntent) -> RiskLevel {
    match intent {
        CommandIntent::ReadOnly => RiskLevel::Low,
        CommandIntent::Write | CommandIntent::Unknown | CommandIntent::Network => RiskLevel::Medium,
        CommandIntent::Destructive
        | CommandIntent::ProcessManagement
        | CommandIntent::PackageManagement
        | CommandIntent::SystemAdmin => RiskLevel::High,
    }
}

// ─── Pipeline ───────────────────────────────────────────────────────────

/// Run the 5-stage semantic validation pipeline on a bash command.
///
/// This is Layer 2 validation, meant to run **after** ShellTool's Layer 1
/// (blocked commands, dangerous patterns, injection detection).
///
/// For pipelines (`cmd1 | cmd2`), each segment is validated independently
/// and the overall result takes the maximum risk across all segments.
pub fn validate(cmd: &str, workspace: &Path) -> BashValidationResult {
    let segments: Vec<&str> = cmd
        .split(['|', '&', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    if segments.is_empty() {
        return BashValidationResult {
            intent: CommandIntent::Unknown,
            risk_level: RiskLevel::Medium,
            warnings: Vec::new(),
        };
    }

    let mut max_risk = RiskLevel::Low;
    let mut max_intent = CommandIntent::ReadOnly;
    let mut all_warnings = Vec::new();

    for seg in &segments {
        let result = validate_segment(seg, workspace);
        if result.risk_level > max_risk {
            max_risk = result.risk_level;
            max_intent = result.intent;
        }
        all_warnings.extend(result.warnings);
    }

    BashValidationResult {
        intent: max_intent,
        risk_level: max_risk,
        warnings: all_warnings,
    }
}

fn validate_segment(seg: &str, workspace: &Path) -> BashValidationResult {
    let first = extract_first_command(seg);
    let mut warnings = Vec::new();

    // Stage 4: Intent classification (includes Stage 1: read-only detection)
    let intent = classify_by_token(&first, seg);
    let mut risk = intent_to_risk_level(intent);

    // Stage 2: Destructive check
    if let Some(w) = check_destructive(seg, &first) {
        warnings.push(w);
        if risk < RiskLevel::High {
            risk = RiskLevel::High;
        }
    }

    // Stage 3: Path validation
    warnings.extend(check_paths(seg, &first, workspace));

    // Stage 5: sed validation
    if let Some(w) = check_sed(seg, &first) {
        warnings.push(w);
    }

    BashValidationResult {
        intent,
        risk_level: risk,
        warnings,
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────

/// Extract the first command token, skipping env var assignments (KEY=val).
fn extract_first_command(cmd: &str) -> String {
    let mut remaining = cmd.trim();

    // Skip leading environment variable assignments.
    loop {
        let next = remaining.trim_start();
        if let Some(eq_pos) = next.find('=') {
            let before_eq = &next[..eq_pos];
            let is_env_var = !before_eq.is_empty()
                && before_eq
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_');
            if is_env_var {
                match next[eq_pos + 1..].find(' ') {
                    Some(space) => {
                        remaining = &next[eq_pos + 1 + space..];
                        continue;
                    }
                    None => return String::new(),
                }
            }
        }
        break;
    }

    remaining
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string()
}

// ─── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ws() -> PathBuf {
        PathBuf::from("/workspace/project")
    }

    #[test]
    fn read_only_ls() {
        let r = validate("ls -la", &ws());
        assert_eq!(r.intent, CommandIntent::ReadOnly);
        assert_eq!(r.risk_level, RiskLevel::Low);
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn write_cp() {
        let r = validate("cp file.txt dest/", &ws());
        assert_eq!(r.intent, CommandIntent::Write);
        assert_eq!(r.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn rm_without_force_is_write() {
        let r = validate("rm file.txt", &ws());
        // rm without -rf → Write (Medium), not Destructive (High)
        assert_eq!(r.intent, CommandIntent::Write);
        assert_eq!(r.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn rm_force_recursive_is_destructive() {
        let r = validate("rm -rf /tmp/dir", &ws());
        assert_eq!(r.intent, CommandIntent::Destructive);
        assert_eq!(r.risk_level, RiskLevel::High);
        assert!(r.warnings.iter().any(|w| w.stage == "destructive"));
    }

    #[test]
    fn shred_is_destructive() {
        let r = validate("shred secret.txt", &ws());
        assert_eq!(r.intent, CommandIntent::Destructive);
        assert_eq!(r.risk_level, RiskLevel::High);
    }

    #[test]
    fn network_curl() {
        let r = validate("curl https://example.com", &ws());
        assert_eq!(r.intent, CommandIntent::Network);
        assert_eq!(r.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn sysadmin_sudo() {
        let r = validate("sudo apt update", &ws());
        assert_eq!(r.intent, CommandIntent::SystemAdmin);
        assert_eq!(r.risk_level, RiskLevel::High);
    }

    #[test]
    fn sed_inplace_warning() {
        let r = validate("sed -i 's/old/new/' file.txt", &ws());
        assert_eq!(r.intent, CommandIntent::Write);
        assert_eq!(r.risk_level, RiskLevel::Medium);
        assert!(r.warnings.iter().any(|w| w.stage == "sed"));
    }

    #[test]
    fn find_delete_is_write() {
        // `find -delete` is bounded by the find expression \u2014 maps to Write (Medium),
        // matching ShellTool's MEDIUM_RISK_PATTERNS contract.
        let r = validate("find . -name '*.tmp' -delete", &ws());
        assert_eq!(r.intent, CommandIntent::Write);
        assert_eq!(r.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn path_traversal_warning() {
        let r = validate("cat ../../etc/passwd", &ws());
        assert!(r.warnings.iter().any(|w| w.stage == "path"));
    }

    #[test]
    fn home_dir_warning() {
        let r = validate("cat ~/secrets.txt", &ws());
        assert!(
            r.warnings
                .iter()
                .any(|w| w.stage == "path" && w.message.contains("home directory"))
        );
    }

    #[test]
    fn git_read_only() {
        let r = validate("git log --oneline", &ws());
        assert_eq!(r.intent, CommandIntent::ReadOnly);
        assert_eq!(r.risk_level, RiskLevel::Low);
    }

    #[test]
    fn git_push_is_write() {
        let r = validate("git push origin main", &ws());
        assert_eq!(r.intent, CommandIntent::Write);
        assert_eq!(r.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn pipeline_takes_max_risk() {
        // ls is Low, rm -rf is High → overall should be High
        let r = validate("ls | rm -rf /tmp/x", &ws());
        assert_eq!(r.risk_level, RiskLevel::High);
    }

    #[test]
    fn env_var_prefix_skipped() {
        let r = validate("FOO=bar ls", &ws());
        assert_eq!(r.intent, CommandIntent::ReadOnly);
        assert_eq!(r.risk_level, RiskLevel::Low);
    }

    #[test]
    fn unknown_command_is_medium() {
        let r = validate("someunknowncmd --flag", &ws());
        assert_eq!(r.intent, CommandIntent::Unknown);
        assert_eq!(r.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn empty_command() {
        let r = validate("", &ws());
        assert_eq!(r.intent, CommandIntent::Unknown);
        assert_eq!(r.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn system_path_write_warning() {
        let r = validate("cp config.yaml /etc/app/", &ws());
        assert!(
            r.warnings
                .iter()
                .any(|w| w.stage == "path" && w.message.contains("/etc/"))
        );
    }

    #[test]
    fn npm_install_is_write() {
        // Reversible package operation \u2014 maps to Write (Medium), aligned with
        // ShellTool's MEDIUM_RISK_PATTERNS (see shell_risk_regression contract).
        let r = validate("npm install express", &ws());
        assert_eq!(r.intent, CommandIntent::Write);
        assert_eq!(r.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn cargo_build_is_write() {
        let r = validate("cargo build --release", &ws());
        assert_eq!(r.intent, CommandIntent::Write);
        assert_eq!(r.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn cargo_check_is_read_only() {
        // Pure analysis, no filesystem mutation \u2014 maps to ReadOnly (Low).
        let r = validate("cargo check", &ws());
        assert_eq!(r.intent, CommandIntent::ReadOnly);
        assert_eq!(r.risk_level, RiskLevel::Low);
    }

    #[test]
    fn cargo_clippy_is_read_only() {
        let r = validate("cargo clippy --all-targets", &ws());
        assert_eq!(r.intent, CommandIntent::ReadOnly);
        assert_eq!(r.risk_level, RiskLevel::Low);
    }

    #[test]
    fn cargo_install_is_package_management() {
        let r = validate("cargo install ripgrep", &ws());
        assert_eq!(r.intent, CommandIntent::PackageManagement);
        assert_eq!(r.risk_level, RiskLevel::High);
    }

    #[test]
    fn lsblk_is_not_sysadmin() {
        // Block-device inspector \u2014 read-only, must not be classified as SystemAdmin/High.
        let r = validate("lsblk", &ws());
        assert_ne!(r.intent, CommandIntent::SystemAdmin);
        assert_ne!(r.risk_level, RiskLevel::High);
    }

    #[test]
    fn process_kill_is_high() {
        let r = validate("kill -9 1234", &ws());
        assert_eq!(r.intent, CommandIntent::ProcessManagement);
        assert_eq!(r.risk_level, RiskLevel::High);
    }

    #[test]
    fn awk_inplace_is_write() {
        let r = validate("awk -i inplace '{print}' file.txt", &ws());
        assert_eq!(r.intent, CommandIntent::Write);
        assert_eq!(r.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn classify_intent_pipeline_aware() {
        let intent = classify_intent("grep foo | kill -9 1234");
        assert_eq!(intent, CommandIntent::ProcessManagement);
    }

    #[test]
    fn intent_display() {
        assert_eq!(CommandIntent::ReadOnly.to_string(), "read-only");
        assert_eq!(CommandIntent::Destructive.to_string(), "destructive");
        assert_eq!(CommandIntent::Unknown.to_string(), "unknown");
    }
}
