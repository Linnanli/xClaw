#!/usr/bin/env python3.12
"""Drift guard: verify dasclaw_sandboxing src/*.rs + *.sbpl remain
byte-identical to the upstream codex-sandboxing snapshot vendored under
codex-cli-main/codex-rs/sandboxing/.

ADR-129 §1.3 + ADR-135 §3.1 mandate verbatim port. The only mechanical
edits allowed in src/*.rs are:

  1. `use codex_protocol::X`            → `use dasclaw_protocol::X`
  2. `use codex_network_proxy::X`       → `use dasclaw_net_proxy::X`
  3. `use codex_utils_absolute_path::X` → `use dasclaw_absolute_path::X`

(The same crate names also appear inside type paths and macro args; the
swap is purely textual, scoped to the three known crate identifiers.)

This script normalizes (1)-(3) before hashing, then compares each Rust
file against its upstream peer. Any other diff is treated as drift and
exits non-zero so CI / pre-commit can block patch-style edits.

.sbpl asset files reference no codex-* symbols and are compared
byte-for-byte without normalization.

Cargo.toml is intentionally NOT compared (package/lib name + dep wiring
diverge by design — see crates/dasclaw_sandboxing/Cargo.toml header).

Run: python3.12 scripts/check_codex_sandboxing_drift.py
"""

from __future__ import annotations

import hashlib
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

LOCAL = REPO_ROOT / "crates" / "dasclaw_sandboxing" / "src"
UPSTREAM = REPO_ROOT / "codex-cli-main" / "codex-rs" / "sandboxing" / "src"

FILES = [
    "bwrap.rs",
    "bwrap_tests.rs",
    "landlock.rs",
    "landlock_tests.rs",
    "lib.rs",
    "manager.rs",
    "manager_tests.rs",
    "policy_transforms.rs",
    "policy_transforms_tests.rs",
    "seatbelt.rs",
    "seatbelt_tests.rs",
]

# .sbpl files contain no codex-* identifiers — compared byte-for-byte.
ASSETS = [
    "restricted_read_only_platform_defaults.sbpl",
    "seatbelt_base_policy.sbpl",
    "seatbelt_network_policy.sbpl",
]

# Reverse the use-path swap so we compare against upstream verbatim.
SWAP_PATTERNS: list[tuple[re.Pattern[str], str]] = [
    (re.compile(r"\bdasclaw_protocol\b"), "codex_protocol"),
    (re.compile(r"\bdasclaw_net_proxy\b"), "codex_network_proxy"),
    (re.compile(r"\bdasclaw_absolute_path\b"), "codex_utils_absolute_path"),
]


_USE_RE = re.compile(r"^use\s+")


def _sort_use_blocks(text: str) -> str:
    """Sort consecutive `use ...;` lines alphabetically.

    rustfmt orders `use` blocks alphabetically, but the rename shifts items
    into a new alphabetical position. To keep the verbatim guarantee
    semantically equivalent across that mechanical swap, we canonicalize
    use-block ordering on both sides before hashing.
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


def normalize_local(text: str) -> str:
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
    for fname in FILES:
        local = LOCAL / fname
        upstream = UPSTREAM / fname
        if not local.exists():
            missing.append(f"local missing: {local.relative_to(REPO_ROOT)}")
            continue
        if not upstream.exists():
            missing.append(f"upstream missing: {upstream.relative_to(REPO_ROOT)}")
            continue
        local_norm = normalize_local(local.read_text(encoding="utf-8"))
        upstream_norm = normalize_upstream(upstream.read_text(encoding="utf-8"))
        if sha(local_norm) != sha(upstream_norm):
            drift.append(
                f"DRIFT: {local.relative_to(REPO_ROOT)} != "
                f"{upstream.relative_to(REPO_ROOT)} "
                f"(after use-path normalization + use-block sort)"
            )

    for fname in ASSETS:
        local = LOCAL / fname
        upstream = UPSTREAM / fname
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

    # Guard against new upstream files we didn't pick up.
    known = set(FILES) | set(ASSETS)
    upstream_extras = sorted(
        p.name for p in UPSTREAM.iterdir()
        if p.is_file() and p.name not in known
    )
    if upstream_extras:
        for name in upstream_extras:
            missing.append(
                f"upstream has extra file not in drift list: {name} "
                "(re-vendor required: add to FILES/ASSETS + copy to "
                "crates/dasclaw_sandboxing/src/)"
            )

    if missing:
        for line in missing:
            print(line, file=sys.stderr)
        return 2
    if drift:
        for line in drift:
            print(line, file=sys.stderr)
        print(
            "\nADR-129 §1.3 + ADR-135 §3.1 forbid hand-edits to ported files. "
            "If upstream needs to change, re-vendor codex-cli-main first.",
            file=sys.stderr,
        )
        return 1
    print(f"OK: {len(FILES)} files + {len(ASSETS)} assets match upstream verbatim.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
