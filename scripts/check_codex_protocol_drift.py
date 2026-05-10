#!/usr/bin/env python3.12
"""Drift guard: verify dasclaw_protocol src remains byte-identical to the
upstream codex snapshot vendored under codex-cli-main/codex-rs/protocol/.

ADR-129 §1.3 + ADR-136 mandate verbatim file-level port. Currently only
`parse_command.rs` (31 LOC, ParsedCommand enum) is vendored — Step C1.2 ~
C1.5 will add `error.rs`, `config_types.rs`, `permissions.rs`, `models.rs`,
`protocol.rs`, `network_policy.rs` (~12,547 LOC, ~70% of codex-protocol).

The only mechanical edits allowed are:

  1. `Cargo.toml` package name swap (codex-protocol → dasclaw_protocol).
  2. `use codex_protocol::X` → `use dasclaw_protocol::X` for cross-file
     imports inside the slice.
  3. Other `use codex_utils_*::X` → `use dasclaw_*::X` swaps as upstream
     transitive deps get sliced (e.g. dasclaw_absolute_path).

This script normalizes the swaps + sorts consecutive `use` blocks before
hashing, then compares each file against its upstream peer. Any other diff
is treated as drift and exits non-zero so CI can block patch-style edits.

Run: python3.12 scripts/check_codex_protocol_drift.py
"""

from __future__ import annotations

import hashlib
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

PAIRS: list[tuple[Path, Path]] = []

# ---- dasclaw_protocol ↔ codex protocol file-level slice --------------------
PROTO_LOCAL = REPO_ROOT / "crates" / "dasclaw_protocol" / "src"
PROTO_UPSTREAM = REPO_ROOT / "codex-cli-main" / "codex-rs" / "protocol" / "src"

# ADR-136 Step C1.3 (Layers 1+2): parse_command + 14 leaves + config_types/openai_models cycle.
# Step C1.4 ~ C1.5 will append: protocol, permissions, models, approvals,
# network_policy, items, request_permissions (hub); error, error_tests.
for fname in [
    "account.rs",
    "agent_path.rs",
    "auth.rs",
    "config_types.rs",
    "dynamic_tools.rs",
    "exec_output.rs",
    "exec_output_tests.rs",
    "mcp.rs",
    "memory_citation.rs",
    "message_history.rs",
    "num_format.rs",
    "openai_models.rs",
    "parse_command.rs",
    "plan_tool.rs",
    "request_user_input.rs",
    "thread_id.rs",
    "tool_name.rs",
    "user_input.rs",
]:
    PAIRS.append((PROTO_LOCAL / fname, PROTO_UPSTREAM / fname))


SWAP_PATTERNS: list[tuple[re.Pattern[str], str]] = [
    # Reverse the use-path swap so we compare against upstream verbatim.
    (re.compile(r"\bdasclaw_absolute_path\b"), "codex_utils_absolute_path"),
    (re.compile(r"\bdasclaw_protocol\b"), "codex_protocol"),
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
            "\nADR-129 §1.3 + ADR-136 forbid hand-edits to ported files. "
            "If upstream needs to change, re-vendor codex-cli-main first.",
            file=sys.stderr,
        )
        return 1
    print(f"OK: {len(PAIRS)} files match upstream verbatim.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
