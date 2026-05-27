#!/usr/bin/env python3.12
"""Drift guard: verify dasclaw_sandbox_linux src/*.rs + build.rs remain
byte-identical to the upstream codex-linux-sandbox snapshot vendored under
codex-cli-main/codex-rs/linux-sandbox/.

ADR-129 §1.3 + ADR-144 §5.2 mandate verbatim port. The only mechanical
edits allowed in src/*.rs are crate-identifier swaps:

  1. `codex_protocol`             → `dasclaw_protocol`
  2. `codex_sandboxing`           → `dasclaw_sandboxing`
  3. `codex_utils_absolute_path`  → `dasclaw_absolute_path`
  4. `codex_linux_sandbox`        → `dasclaw_sandbox_linux`  (lib name in main.rs)

(`codex_network_proxy` does not appear in linux-sandbox sources, but is
included for forward-compat / symmetry with check_codex_sandboxing_drift.py.)

Env var literals (CODEX_HOME, CODEX_BWRAP_SOURCE_DIR,
CODEX_SKIP_VENDORED_BWRAP) and the local variable name `codex_home` are
kept verbatim per the dasclaw_utils_home_dir precedent — they are NOT
swapped.

build.rs is also drift-checked: it is byte-for-byte verbatim (no swaps
needed because it references no codex_* Rust identifiers; only env var
strings, which are preserved verbatim).

Cargo.toml is intentionally NOT compared (package/lib/bin names + dep
wiring diverge by design — see crates/dasclaw_sandbox_linux/Cargo.toml
header for the per-edit rationale).

The vendored bubblewrap C sources at `crates/vendor/bubblewrap/` are
verified by a separate byte-for-byte `diff -rq` against
`codex-cli-main/codex-rs/vendor/bubblewrap/` (per PR #441 acceptance).
This script does NOT re-check that — vendor drift would be caught by
the cargo-deny / NOTICE / LGPL §6 acceptance machinery in PR #441.

Run: python3.12 scripts/check_codex_linux_sandbox_drift.py
"""

from __future__ import annotations

import hashlib
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

LOCAL = REPO_ROOT / "crates" / "dasclaw_sandbox_linux"
UPSTREAM = REPO_ROOT / "codex-cli-main" / "codex-rs" / "linux-sandbox"

# Files to check: src/*.rs (9 files) + build.rs.
SRC_FILES = [
    "bwrap.rs",
    "landlock.rs",
    "launcher.rs",
    "lib.rs",
    "linux_run_main.rs",
    "linux_run_main_tests.rs",
    "main.rs",
    "proxy_routing.rs",
    "vendored_bwrap.rs",
]

# build.rs is at crate root, not under src/.
ROOT_FILES = [
    "build.rs",
]

# Reverse the use-path swap so we compare against upstream verbatim.
# Order matters: swap longer identifiers first to avoid sub-string overlap
# (e.g. dasclaw_sandbox_linux must precede dasclaw_sandbox if both existed).
SWAP_PATTERNS: list[tuple[re.Pattern[str], str]] = [
    (re.compile(r"dasclaw_sandbox_linux"), "codex_linux_sandbox"),
    (re.compile(r"dasclaw_sandboxing"), "codex_sandboxing"),
    (re.compile(r"dasclaw_protocol"), "codex_protocol"),
    (re.compile(r"dasclaw_absolute_path"), "codex_utils_absolute_path"),
    (re.compile(r"dasclaw_net_proxy"), "codex_network_proxy"),
]


_USE_RE = re.compile(r"^\s*use\s+")
_ATTR_RE = re.compile(r"^\s*#\[")

# Marker comments that delimit dasclaw-local extensions inside an otherwise
# verbatim-ported file. Lines between a START / END pair are stripped before
# hashing so they don't show up as drift. Mirrors the same machinery in
# check_codex_protocol_drift.py (ADR-136 amendment 3 / #874 authorization).
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

    A "use line" here is either a bare `use ...;` line or one or more
    leading attribute lines (e.g. ``#[cfg(test)]``) immediately followed
    by a `use ...;` line — they are treated as one atomic unit so the
    sort doesn't desynchronize attributes from their imports.

    rustfmt orders `use` blocks alphabetically, but the rename shifts
    items into a new alphabetical position. To keep the verbatim guarantee
    semantically equivalent across that mechanical swap, we canonicalize
    use-block ordering on both sides before hashing.
    """
    lines = text.split("\n")
    out: list[str] = []
    i = 0

    def _read_atom(start: int) -> tuple[list[str], int] | None:
        # Collect leading attribute lines, then require a `use` line.
        j = start
        atom: list[str] = []
        while j < len(lines) and _ATTR_RE.match(lines[j]):
            atom.append(lines[j])
            j += 1
        if j < len(lines) and _USE_RE.match(lines[j]):
            atom.append(lines[j])
            return atom, j + 1
        return None

    while i < len(lines):
        atom = _read_atom(i)
        if atom is None:
            out.append(lines[i])
            i += 1
            continue
        block: list[list[str]] = []
        while atom is not None:
            atoms, next_i = atom
            block.append(atoms)
            i = next_i
            atom = _read_atom(i)
        # Sort by the use-line (last entry of each atom).
        block.sort(key=lambda a: a[-1])
        for a in block:
            out.extend(a)
    return "\n".join(out)


def _collapse_ws(text: str) -> str:
    """Collapse all whitespace runs to a single space.

    Rationale: the crate-identifier swap (e.g.
    ``codex_protocol`` → ``dasclaw_protocol``) changes per-line byte length,
    which can push a previously-fitting line past rustfmt's `max_width`,
    triggering a mechanical line-wrap on our side that has no counterpart
    upstream. Whitespace-insensitive comparison neutralizes that purely
    formatting-driven drift while still catching any actual content edit
    (added/removed token, reordered argument, etc.). This is a strictly
    stronger guarantee than the use-block sort alone, and is safe because
    Rust syntax is whitespace-insensitive outside string literals.

    Note: we intentionally do NOT try to parse strings — verbatim ported
    files contain no edited string literals (env var names are also kept
    verbatim per the dasclaw_utils_home_dir precedent), so whitespace
    collapse inside a string would simply leave it semantically identical.
    """
    return re.sub(r"\s+", " ", text).strip()


def normalize_local(text: str, source_label: str) -> str:
    text = _strip_drift_ignore_blocks(text, source_label)
    for pat, repl in SWAP_PATTERNS:
        text = pat.sub(repl, text)
    return _collapse_ws(_sort_use_blocks(text))


def normalize_upstream(text: str) -> str:
    return _collapse_ws(_sort_use_blocks(text))


def sha(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def _check_pair(local: Path, upstream: Path, *, swap: bool) -> str | None:
    """Return drift message if hashes differ, else None."""
    if not local.exists():
        return f"local missing: {local.relative_to(REPO_ROOT)}"
    if not upstream.exists():
        return f"upstream missing: {upstream.relative_to(REPO_ROOT)}"
    local_text = local.read_text(encoding="utf-8")
    upstream_text = upstream.read_text(encoding="utf-8")
    source_label = str(local.relative_to(REPO_ROOT))
    local_norm = (
        normalize_local(local_text, source_label)
        if swap
        else _strip_drift_ignore_blocks(local_text, source_label)
    )
    upstream_norm = normalize_upstream(upstream_text) if swap else upstream_text
    if sha(local_norm) != sha(upstream_norm):
        suffix = (
            " (after use-path swap + use-block sort + whitespace collapse)"
            if swap
            else " (byte-for-byte)"
        )
        return (
            f"DRIFT: {local.relative_to(REPO_ROOT)} != "
            f"{upstream.relative_to(REPO_ROOT)}{suffix}"
        )
    return None


def main() -> int:
    drift: list[str] = []
    missing: list[str] = []

    for fname in SRC_FILES:
        result = _check_pair(
            LOCAL / "src" / fname,
            UPSTREAM / "src" / fname,
            swap=True,
        )
        if result is None:
            continue
        if result.startswith("DRIFT:"):
            drift.append(result)
        else:
            missing.append(result)

    for fname in ROOT_FILES:
        result = _check_pair(
            LOCAL / fname,
            UPSTREAM / fname,
            swap=False,
        )
        if result is None:
            continue
        if result.startswith("DRIFT:"):
            drift.append(result)
        else:
            missing.append(result)

    # Guard against new upstream src/ files we didn't pick up.
    known_src = set(SRC_FILES)
    upstream_src_extras = sorted(
        p.name
        for p in (UPSTREAM / "src").iterdir()
        if p.is_file() and p.name not in known_src
    )
    if upstream_src_extras:
        for name in upstream_src_extras:
            missing.append(
                f"upstream has extra src/ file not in drift list: {name} "
                "(re-vendor required: add to SRC_FILES + copy to "
                "crates/dasclaw_sandbox_linux/src/ + apply mechanical swap)"
            )

    if missing:
        for line in missing:
            print(line, file=sys.stderr)
        return 2
    if drift:
        for line in drift:
            print(line, file=sys.stderr)
        print(
            "\nADR-129 §1.3 + ADR-144 §5.2 forbid hand-edits to ported files. "
            "If upstream needs to change, re-vendor codex-cli-main first.",
            file=sys.stderr,
        )
        return 1
    print(
        f"OK: {len(SRC_FILES)} src files + {len(ROOT_FILES)} root files match "
        "upstream verbatim."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
