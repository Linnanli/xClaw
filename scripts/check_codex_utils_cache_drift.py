#!/usr/bin/env python3.12
"""Drift guard: verify dasclaw_utils_cache/src remains byte-identical
to the upstream codex snapshot vendored under codex-cli-main/codex-rs/.

ADR-129 §1.3 + ADR-136 §3 amendment 2 PR-C1.prep-3 mandate verbatim port.
The only mechanical edits allowed are:

  1. `Cargo.toml` package name swap (codex-utils-cache -> dasclaw_utils_cache).

`src/lib.rs` has zero `codex_*` use-paths so no normalization is needed.
This script compares each Rust file against its upstream peer. Any diff
is treated as drift and exits non-zero so CI / pre-commit can block
patch-style edits.

Run: python3.12 scripts/check_codex_utils_cache_drift.py
"""

from __future__ import annotations

import hashlib
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

PAIRS: list[tuple[Path, Path]] = []

LOCAL = REPO_ROOT / "crates" / "dasclaw_utils_cache" / "src"
UPSTREAM = REPO_ROOT / "codex-cli-main" / "codex-rs" / "utils" / "cache" / "src"
for fname in ["lib.rs"]:
    PAIRS.append((LOCAL / fname, UPSTREAM / fname))


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
        local_text = local.read_text(encoding="utf-8")
        upstream_text = upstream.read_text(encoding="utf-8")
        if sha(local_text) != sha(upstream_text):
            drift.append(
                f"DRIFT: {local.relative_to(REPO_ROOT)} != "
                f"{upstream.relative_to(REPO_ROOT)}"
            )

    if missing:
        for line in missing:
            print(line, file=sys.stderr)
        return 2
    if drift:
        for line in drift:
            print(line, file=sys.stderr)
        print(
            "\nADR-129 §1.3 forbids hand-edits to ported files. "
            "If upstream changed, re-vendor and update the snapshot under "
            "codex-cli-main/codex-rs/utils/cache/.\n"
            "If a swap rule is missing, extend SWAP_PATTERNS in this script "
            "(no swaps currently needed for utils-cache).",
            file=sys.stderr,
        )
        return 1
    print(f"OK: {len(PAIRS)} file(s) match upstream verbatim.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
