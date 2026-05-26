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
  4. Dasclaw-specific local extensions wrapped in
     `// CODEX-DRIFT-IGNORE-START: <reason>` / `// CODEX-DRIFT-IGNORE-END`
     marker pairs. Each block must cite an issue or ADR in the START
     comment so PR review can audit it as an intentional local extension
     rather than upstream drift. Authorized by ADR-136 amendment 3 (#874).

This script normalizes the swaps + strips marker-wrapped local extensions
+ sorts consecutive `use` blocks before hashing, then compares each file
against its upstream peer. Any other diff is treated as drift and exits
non-zero so CI can block patch-style edits.

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

# ADR-136 Step C1.5 (Layers 1-4): parse_command + 14 leaves + config_types/openai_models cycle
# + Layer 3 hub (protocol, permissions, models, request_permissions, approvals,
# network_policy, items) + Layer 4 (error, error_tests).
for fname in [
    "account.rs",
    "agent_path.rs",
    "approvals.rs",
    "auth.rs",
    "config_types.rs",
    "dynamic_tools.rs",
    "error.rs",
    "error_tests.rs",
    "exec_output.rs",
    "exec_output_tests.rs",
    "items.rs",
    "mcp.rs",
    "memory_citation.rs",
    "message_history.rs",
    "models.rs",
    "network_policy.rs",
    "num_format.rs",
    "openai_models.rs",
    "parse_command.rs",
    "permissions.rs",
    "plan_tool.rs",
    "protocol.rs",
    "request_permissions.rs",
    "request_user_input.rs",
    "thread_id.rs",
    "tool_name.rs",
    "user_input.rs",
]:
    PAIRS.append((PROTO_LOCAL / fname, PROTO_UPSTREAM / fname))

# Asset files (non-Rust) vendored alongside the source — verbatim, no swaps.
ASSET_PAIRS: list[tuple[Path, Path]] = [
    (
        PROTO_LOCAL / "prompts" / "base_instructions" / "default.md",
        PROTO_UPSTREAM / "prompts" / "base_instructions" / "default.md",
    ),
]


SWAP_PATTERNS: list[tuple[re.Pattern[str], str]] = [
    # Reverse the use-path swap so we compare against upstream verbatim.
    (re.compile(r"\bdasclaw_absolute_path\b"), "codex_utils_absolute_path"),
    (re.compile(r"\bdasclaw_async_utils\b"), "codex_async_utils"),
    (re.compile(r"\bdasclaw_utils_image\b"), "codex_utils_image"),
    (re.compile(r"\bdasclaw_utils_string\b"), "codex_utils_string"),
    (re.compile(r"\bdasclaw_execpolicy\b"), "codex_execpolicy"),
    (re.compile(r"\bdasclaw_net_proxy\b"), "codex_network_proxy"),
    (re.compile(r"\bdasclaw_protocol\b"), "codex_protocol"),
]


_USE_RE = re.compile(r"^use\s+")

# Marker comments that delimit dasclaw-local extensions inside an otherwise
# verbatim-ported file. Lines between a START / END pair are stripped before
# hashing so they don't show up as drift. See module docstring + ADR-136
# amendment 3 (#874) for the authorization.
_DRIFT_IGNORE_START_RE = re.compile(r"^\s*//\s*CODEX-DRIFT-IGNORE-START\b")
_DRIFT_IGNORE_END_RE = re.compile(r"^\s*//\s*CODEX-DRIFT-IGNORE-END\b")


def _strip_drift_ignore_blocks(text: str, source_label: str) -> str:
    """Remove lines between paired CODEX-DRIFT-IGNORE markers.

    When a marker block is preceded by a blank line that exists solely to
    separate the local extension from the upstream code above, that blank
    line is also dropped so the surrounding whitespace matches upstream
    after the block is removed.

    Raises SystemExit if a START marker has no matching END (or vice
    versa) so misuse fails loudly rather than silently widening the
    exemption surface.
    """
    out: list[str] = []
    skip = False
    for lineno, line in enumerate(text.split("\n"), start=1):
        if not skip and _DRIFT_IGNORE_START_RE.match(line):
            # Drop the blank-line spacer that separated the marker block
            # from the preceding upstream code, if any.
            if out and out[-1] == "":
                out.pop()
            skip = True
            continue
        if skip and _DRIFT_IGNORE_END_RE.match(line):
            skip = False
            continue
        if not skip and _DRIFT_IGNORE_END_RE.match(line):
            raise SystemExit(
                f"ERROR: {source_label}:{lineno}: stray "
                f"// CODEX-DRIFT-IGNORE-END without matching START"
            )
        if not skip:
            out.append(line)
    if skip:
        raise SystemExit(
            f"ERROR: {source_label}: unterminated // CODEX-DRIFT-IGNORE-START "
            f"block; missing matching // CODEX-DRIFT-IGNORE-END"
        )
    return "\n".join(out)


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


def normalize(text: str, source_label: str) -> str:
    text = _strip_drift_ignore_blocks(text, source_label)
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
        local_norm = normalize(
            local.read_text(encoding="utf-8"),
            str(local.relative_to(REPO_ROOT)),
        )
        upstream_norm = normalize_upstream(upstream.read_text(encoding="utf-8"))
        if sha(local_norm) != sha(upstream_norm):
            drift.append(
                f"DRIFT: {local.relative_to(REPO_ROOT)} != "
                f"{upstream.relative_to(REPO_ROOT)} "
                f"(after use-path normalization + use-block sort)"
            )

    # Asset files are compared byte-for-byte (no normalization).
    for local, upstream in ASSET_PAIRS:
        if not local.exists():
            missing.append(f"local missing: {local.relative_to(REPO_ROOT)}")
            continue
        if not upstream.exists():
            missing.append(f"upstream missing: {upstream.relative_to(REPO_ROOT)}")
            continue
        if local.read_bytes() != upstream.read_bytes():
            drift.append(
                f"DRIFT: {local.relative_to(REPO_ROOT)} != "
                f"{upstream.relative_to(REPO_ROOT)} (asset byte-mismatch)"
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
    print(f"OK: {len(PAIRS)} files + {len(ASSET_PAIRS)} assets match upstream verbatim.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
