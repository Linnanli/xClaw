#!/usr/bin/env python3.12
"""Drift guard: verify dasclaw_utils_rustls_provider/src remains byte-identical
to the upstream codex snapshot vendored under codex-cli-main/codex-rs/.

ADR-129 §1.3 + #324 sub-task 3 (ADR-137) mandate verbatim port. The only
mechanical edit is the package name swap in Cargo.toml — source has zero
codex_* use-paths so no body swaps are required.

Run: python3.12 scripts/check_codex_utils_rustls_provider_drift.py
"""

from __future__ import annotations

import hashlib
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

PAIRS: list[tuple[Path, Path]] = []

LOCAL = REPO_ROOT / "crates" / "dasclaw_utils_rustls_provider" / "src"
UPSTREAM = (
    REPO_ROOT / "codex-cli-main" / "codex-rs" / "utils" / "rustls-provider" / "src"
)
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
            "If upstream needs to change, re-vendor codex-cli-main first.",
            file=sys.stderr,
        )
        return 1
    print(f"OK: {len(PAIRS)} files match upstream verbatim.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
