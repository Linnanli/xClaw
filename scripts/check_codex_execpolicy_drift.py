#!/usr/bin/env python3.12
"""Drift guard: verify dasclaw_execpolicy + dasclaw_absolute_path src/tests
remain byte-identical to the upstream codex snapshot vendored under
codex-cli-main/codex-rs/.

ADR-129 §1.3 + ADR-132 mandate verbatim port. The only mechanical edits
allowed are:

  1. `Cargo.toml` package/bin name swap (codex-execpolicy → dasclaw_execpolicy,
     codex-utils-absolute-path → dasclaw_absolute_path).
  2. `use codex_utils_absolute_path::X` → `use dasclaw_absolute_path::X` in
     `crates/dasclaw_execpolicy/src/*.rs` + `tests/*.rs`.
  3. `use codex_execpolicy::X` → `use dasclaw_execpolicy::X` in the same files.

This script normalizes (2) + (3) before hashing, then compares each Rust file
against its upstream peer. Any other diff is treated as drift and exits
non-zero so CI / pre-commit can block patch-style edits.

Run: python3.12 scripts/check_codex_execpolicy_drift.py
"""

from __future__ import annotations

import hashlib
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

PAIRS: list[tuple[Path, Path]] = []

# ---- dasclaw_absolute_path ↔ codex utils/absolute-path ----------------------
ABS_LOCAL = REPO_ROOT / "crates" / "dasclaw_absolute_path" / "src"
ABS_UPSTREAM = (
    REPO_ROOT / "codex-cli-main" / "codex-rs" / "utils" / "absolute-path" / "src"
)
for fname in ["lib.rs", "absolutize.rs"]:
    PAIRS.append((ABS_LOCAL / fname, ABS_UPSTREAM / fname))

# ---- dasclaw_execpolicy ↔ codex execpolicy ---------------------------------
EXEC_LOCAL = REPO_ROOT / "crates" / "dasclaw_execpolicy"
EXEC_UPSTREAM = REPO_ROOT / "codex-cli-main" / "codex-rs" / "execpolicy"
for fname in [
    "src/amend.rs",
    "src/decision.rs",
    "src/error.rs",
    "src/executable_name.rs",
    "src/execpolicycheck.rs",
    "src/lib.rs",
    "src/main.rs",
    "src/parser.rs",
    "src/policy.rs",
    "src/rule.rs",
    "tests/basic.rs",
    "examples/example.codexpolicy",
]:
    PAIRS.append((EXEC_LOCAL / fname, EXEC_UPSTREAM / fname))


SWAP_PATTERNS: list[tuple[re.Pattern[str], str]] = [
    # Reverse the use-path swap so we compare against upstream verbatim.
    (re.compile(r"\bdasclaw_absolute_path\b"), "codex_utils_absolute_path"),
    (re.compile(r"\bdasclaw_execpolicy\b"), "codex_execpolicy"),
]


_USE_RE = re.compile(r"^use\s+")


def _sort_use_blocks(text: str) -> str:
    """Sort consecutive `use ...;` lines alphabetically.

    rustfmt normally orders `use` blocks alphabetically, but the rename from
    `codex_utils_absolute_path` → `dasclaw_absolute_path` shifts items into a
    new alphabetical position. To keep the verbatim guarantee semantically
    equivalent across that mechanical swap, we canonicalize use-block ordering
    on both sides before hashing.
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
            "\nADR-129 §1.3 + ADR-132 forbid hand-edits to ported files. "
            "If upstream needs to change, re-vendor codex-cli-main first.",
            file=sys.stderr,
        )
        return 1
    print(f"OK: {len(PAIRS)} files match upstream verbatim.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
