#!/usr/bin/env python3.12
"""Drift guard: verify dasclaw_shell_command + dasclaw_parsed_command src
remain byte-identical to the upstream codex snapshot vendored under
codex-cli-main/codex-rs/.

ADR-129 §1.3 + ADR-133 mandate verbatim port. The only mechanical edits
allowed are:

  1. `Cargo.toml` package name swap (codex-shell-command → dasclaw_shell_command,
     codex-protocol parse_command slice → dasclaw_parsed_command).
  2. `use codex_utils_absolute_path::X` → `use dasclaw_absolute_path::X`.
  3. `use codex_protocol::parse_command::X` → `use dasclaw_parsed_command::parse_command::X`.
  4. `use codex_shell_command::X` → `use dasclaw_shell_command::X`.

Slicing note (ADR-133 §amend): only `protocol/src/parse_command.rs` (31 LOC)
is vendored from codex-protocol — the other ~16k LOC + 30+ transitive deps
are intentionally NOT pulled in. The drift guard therefore pairs only that
one file, not the whole protocol crate.

This script normalizes the swaps + sorts consecutive `use` blocks before
hashing, then compares each file against its upstream peer. Any other diff
is treated as drift and exits non-zero so CI can block patch-style edits.

Run: python3.12 scripts/check_codex_shell_command_drift.py
"""

from __future__ import annotations

import hashlib
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

PAIRS: list[tuple[Path, Path]] = []

# ---- dasclaw_parsed_command ↔ codex protocol parse_command slice -----------
PC_LOCAL = REPO_ROOT / "crates" / "dasclaw_parsed_command" / "src"
PC_UPSTREAM = REPO_ROOT / "codex-cli-main" / "codex-rs" / "protocol" / "src"
PAIRS.append((PC_LOCAL / "parse_command.rs", PC_UPSTREAM / "parse_command.rs"))

# ---- dasclaw_shell_command ↔ codex shell-command ---------------------------
SC_LOCAL = REPO_ROOT / "crates" / "dasclaw_shell_command" / "src"
SC_UPSTREAM = REPO_ROOT / "codex-cli-main" / "codex-rs" / "shell-command" / "src"
for fname in [
    "bash.rs",
    "lib.rs",
    "parse_command.rs",
    "powershell.rs",
    "shell_detect.rs",
    "command_safety/is_dangerous_command.rs",
    "command_safety/is_safe_command.rs",
    "command_safety/mod.rs",
    "command_safety/powershell_parser.rs",
    "command_safety/powershell_parser.ps1",
    "command_safety/windows_dangerous_commands.rs",
    "command_safety/windows_safe_commands.rs",
]:
    PAIRS.append((SC_LOCAL / fname, SC_UPSTREAM / fname))


SWAP_PATTERNS: list[tuple[re.Pattern[str], str]] = [
    # Reverse the use-path swap so we compare against upstream verbatim.
    (re.compile(r"\bdasclaw_absolute_path\b"), "codex_utils_absolute_path"),
    (re.compile(r"\bdasclaw_parsed_command\b"), "codex_protocol"),
    (re.compile(r"\bdasclaw_shell_command\b"), "codex_shell_command"),
]


_USE_RE = re.compile(r"^use\s+")


def _sort_use_blocks(text: str) -> str:
    """Sort consecutive `use ...;` lines alphabetically.

    rustfmt orders `use` blocks alphabetically; renames may shift items into
    a new position. We canonicalize use-block ordering on both sides before
    hashing so the verbatim guarantee survives the mechanical swap.
    """
    lines = text.split("\n")
    out: list[str] = []
    i = 0
    while i < len(lines):
        if _USE_RE.match(lines[i]):
            j = i
            block: list[str] = []
            while j < len(lines) and _USE_RE.match(lines[j]):
                block.append(lines[j])
                j += 1
            out.extend(sorted(block))
            i = j
        else:
            out.append(lines[i])
            i += 1
    return "\n".join(out)


def normalize(text: str) -> str:
    for pat, repl in SWAP_PATTERNS:
        text = pat.sub(repl, text)
    return _sort_use_blocks(text)


def normalize_upstream(text: str) -> str:
    return _sort_use_blocks(text)


def sha(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def main() -> int:
    drift: list[str] = []
    missing: list[str] = []
    for local, upstream in PAIRS:
        if not local.exists():
            missing.append(f"local missing: {local.relative_to(REPO_ROOT)}")
            continue
        if not upstream.exists():
            missing.append(f"upstream missing: {upstream.relative_to(REPO_ROOT)}")
            continue
        local_norm = normalize(local.read_text(encoding="utf-8"))
        upstream_norm = normalize_upstream(upstream.read_text(encoding="utf-8"))
        if sha(local_norm) != sha(upstream_norm):
            drift.append(
                f"DRIFT: {local.relative_to(REPO_ROOT)} != "
                f"{upstream.relative_to(REPO_ROOT)} "
                f"(after use-path normalization + use-block sort)"
            )

    if missing:
        for line in missing:
            print(line, file=sys.stderr)
        return 2
    if drift:
        for line in drift:
            print(line, file=sys.stderr)
        print(
            "\nADR-129 §1.3 + ADR-133 forbid hand-edits to ported files. "
            "If upstream needs to change, re-vendor codex-cli-main first.",
            file=sys.stderr,
        )
        return 1
    print(f"OK: {len(PAIRS)} files match upstream verbatim.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
