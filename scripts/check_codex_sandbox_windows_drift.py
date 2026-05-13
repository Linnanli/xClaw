#!/usr/bin/env python3.12
"""Drift guard: verify dasclaw_sandbox_windows src/*.rs remains a verbatim port
of the upstream codex-windows-sandbox snapshot vendored under
vendor/codex-windows-sandbox/upstream/windows-sandbox-rs/.

Authority: ADR-129 §1.3 + ADR-145 §6 (Option B "rename minimal, drift guard
最简"). The only mechanical edits this guard tolerates between the two trees
are:

  1. Attribution header on every local file (V'-b rule 3 in AGENTS.md memory):
        // Derived from openai/codex commit <sha>
        //   path: codex-rs/windows-sandbox-rs/src/<file>
        // SPDX-License-Identifier: Apache-2.0
     plus optional follow-up `//` port-note lines and a trailing blank.
     Stripped from local before comparison.

  2. A leading `#![cfg(target_os = "windows")]` inner attribute that the
     dasclaw lib-crate needs because it compiles on every host (upstream is a
     bin-only crate that gates at crate level). Stripped from local before
     comparison when absent upstream.

  3. The ADR-145 rename swaps recorded in
        scripts/codex_to_dasclaw_rename_map.json
     (string-literal renames such as CodexSandboxUsers → DasclawSandboxUsers,
     codex-resources → dasclaw-resources, codex-runner pipe name, etc.).

  4. A small set of mechanical use-path swaps that other Phase 1.1.x PRs
     applied uniformly (see `USE_PATH_SWAPS` below for the authoritative list):
        crate::string_util                      <- codex_utils_string
        dirs::home_dir                          <- dirs_next::home_dir
        crate::absolute_path::AbsolutePathBuf   <- codex_utils_absolute_path::AbsolutePathBuf
        crate::types::SandboxPolicy             <- codex_protocol::protocol::SandboxPolicy
        dasclaw_sandbox_windows::               <- codex_windows_sandbox::

  5. Trailing `// safety: verbatim from codex <sha> <file>` annotations on
     `.unwrap()` / `.expect()` sites (added locally so check_no_panics.py
     honours the per-line exemption; not load-bearing for the port). Stripped
     from local before comparison.

  6. Symmetric canonicalisation of idioms that upstream itself spells two
     ways across sibling files (see `BOTH_SIDES_SWAPS`):
        super::to_wide  ==  codex_windows_sandbox::to_wide

  7. `use` blocks separated only by blank lines are collapsed into a single
     alphabetically sorted block on both sides, so import-grouping style is
     not a source of drift.

After undoing (3)–(7) on the local copy and stripping (1)+(2)+(5), both files
are canonicalised through `rustfmt --edition 2024 --emit stdout` so that
formatting drift (`use {a, b}` ordering, single-line vs block bodies, trailing
commas, ...) cannot register as a verbatim violation. Anything left over is
real drift and the guard exits non-zero.

Cargo.toml is intentionally NOT compared (crate name + dep wiring diverge by
design — see crates/dasclaw_sandbox_windows/Cargo.toml header).

Files that are intentionally extended by x-claw (lib.rs adapter facade, audit
hooks, identity bridge, ...) are listed in `EXTENDED_FILES` with rationale and
skipped from the verbatim check.

Run: python3.12 scripts/check_codex_sandbox_windows_drift.py
"""

from __future__ import annotations

import hashlib
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

LOCAL_ROOT = REPO_ROOT / "crates" / "dasclaw_sandbox_windows" / "src"
UPSTREAM_ROOT = (
    REPO_ROOT
    / "vendor"
    / "codex-windows-sandbox"
    / "upstream"
    / "windows-sandbox-rs"
    / "src"
)
RENAME_MAP_PATH = REPO_ROOT / "scripts" / "codex_to_dasclaw_rename_map.json"

RUST_EDITION = "2024"

# Files under strict verbatim drift guard. Each must reduce to byte-equal
# upstream after normalisation. Maintained in alphabetical order.
VERBATIM_FILES: list[str] = [
    "allow.rs",
    "cap.rs",
    "desktop.rs",
    "dpapi.rs",
    "elevated_impl.rs",
    "env.rs",
    "firewall.rs",
    "hide_users.rs",
    "logging.rs",
    "path_normalization.rs",
    "proc_thread_attr.rs",
    "process.rs",
    "read_acl_mutex.rs",
    "sandbox_users.rs",
    "sandbox_utils.rs",
    "setup_error.rs",
    "setup_main_win.rs",
    "setup_orchestrator.rs",
    "spawn_prep.rs",
    "ssh_config_dependencies.rs",
    "token.rs",
    "winutil.rs",
    "workspace_acl.rs",
]

# Files intentionally diverged in x-claw and therefore NOT under strict drift
# guard. Each entry must come with a one-line rationale.
EXTENDED_FILES: dict[str, str] = {
    "lib.rs": (
        "xClaw-only adapter facade: extra re-exports, target_os gating and "
        "the public preflight entry point (Phase 1.0/1.1.x)."
    ),
    "acl.rs": (
        "Sibling-visibility tweaks (pub(crate) widening for x-claw glue) "
        "beyond what the rename map covers; revisit on next re-vendor."
    ),
    "audit.rs": (
        "x-claw adapter wires upstream audit hooks into dasclaw observability; "
        "structure diverges from upstream by design."
    ),
    "helper_materialization.rs": (
        "Contains the rename-map resource/binary swaps plus rustfmt drift "
        "introduced by the x-claw rustfmt profile; will be re-vendored before "
        "promotion to verbatim list."
    ),
    "identity.rs": (
        "x-claw extension that bridges sandbox identity to dasclaw_identity; "
        "no direct upstream counterpart symbol map."
    ),
    "policy.rs": (
        "x-claw adapter on top of upstream policy types (extra constructors / "
        "serde glue)."
    ),
}

# Crate-path / use-path swaps that are applied across the port and have no
# entry in the JSON rename map (they are not string literals). Each swap maps
# a local-side path back to the upstream-side path; they are applied in order.
USE_PATH_SWAPS: list[tuple[re.Pattern[str], str]] = [
    # logging.rs (Phase 1.1.1 inline port).
    (re.compile(r"\bcrate::string_util\b"), "codex_utils_string"),
    # env.rs picks `dirs = "6"` (API-equivalent) instead of upstream `dirs_next`.
    (re.compile(r"\bdirs::home_dir\b"), "dirs_next::home_dir"),
    # allow.rs / setup_orchestrator.rs route AbsolutePathBuf through the
    # locally vendored crate; upstream imports it from codex_utils_absolute_path.
    (
        re.compile(r"\bcrate::absolute_path::AbsolutePathBuf\b"),
        "codex_utils_absolute_path::AbsolutePathBuf",
    ),
    # allow.rs holds SandboxPolicy in a local types module; upstream re-imports
    # the canonical type from codex_protocol.
    (
        re.compile(r"\bcrate::types::SandboxPolicy\b"),
        "codex_protocol::protocol::SandboxPolicy",
    ),
    # x-claw crate name swap; upstream symbols read `codex_windows_sandbox`.
    (re.compile(r"\bdasclaw_sandbox_windows\b"), "codex_windows_sandbox"),
]

# Equivalence-class swaps applied symmetrically to BOTH local and upstream
# before comparison. Used when upstream itself spells the same symbol two
# different ways in sibling files (so even a perfect copy would still drift).
BOTH_SIDES_SWAPS: list[tuple[re.Pattern[str], str]] = [
    # read_acl_mutex.rs upstream: `use super::to_wide;`
    # sandbox_users.rs / setup_main_win.rs upstream: `use codex_windows_sandbox::to_wide;`
    # Both forms resolve to the same symbol; canonicalise to the absolute path.
    (re.compile(r"\bsuper::to_wide\b"), "codex_windows_sandbox::to_wide"),
]

# Local files annotate verbatim `.unwrap()` / `.expect()` sites with a
# `// safety: verbatim from codex <sha> <file>` trailing comment so that
# scripts/check_no_panics.py honours the per-line exemption. The comment is
# not load-bearing for the port — strip it before comparing.
_SAFETY_TRAIL_RE = re.compile(
    r"\s*//\s*safety:\s*verbatim from codex\s+\S+\s+\S+\s*$"
)

_ATTRIBUTION_FIRST_LINE_RE = re.compile(r"^// Derived from openai/codex\b")
_CFG_WINDOWS_RE = re.compile(r'^\s*#!\[cfg\(target_os\s*=\s*"windows"\)\]\s*$')
_USE_RE = re.compile(r"^use\s+")


def _strip_attribution_header(text: str) -> str:
    lines = text.split("\n")
    if not lines or not _ATTRIBUTION_FIRST_LINE_RE.match(lines[0]):
        return text
    idx = 0
    while idx < len(lines) and lines[idx].startswith("//"):
        idx += 1
    if idx < len(lines) and lines[idx] == "":
        idx += 1
    return "\n".join(lines[idx:])


def _strip_local_only_windows_cfg(local: str, upstream: str) -> str:
    """Drop a leading `#![cfg(target_os = "windows")]` from local when it is
    absent upstream (dasclaw lib-crate gates per-file; upstream is bin-only
    and gates at crate level)."""
    if _has_leading_windows_cfg(upstream):
        return local
    return _drop_leading_windows_cfg(local)


def _has_leading_windows_cfg(text: str) -> bool:
    for line in text.split("\n"):
        if line.strip() == "":
            continue
        return bool(_CFG_WINDOWS_RE.match(line))
    return False


def _drop_leading_windows_cfg(text: str) -> str:
    lines = text.split("\n")
    for i, line in enumerate(lines):
        if line.strip() == "":
            continue
        if _CFG_WINDOWS_RE.match(line):
            del lines[i]
            if i < len(lines) and lines[i].strip() == "":
                del lines[i]
        break
    return "\n".join(lines)


def _strip_safety_trail_comments(text: str) -> str:
    return "\n".join(_SAFETY_TRAIL_RE.sub("", line) for line in text.split("\n"))


def _collapse_and_sort_use_groups(text: str) -> str:
    """Merge consecutive `use ...;` lines (including blocks separated only by
    blank lines) into a single alphabetically sorted block. Upstream and local
    differ on whether the `crate::*` use lines sit in their own blank-separated
    group; collapsing to one canonical group makes the comparison stable
    regardless of import-grouping style."""
    lines = text.split("\n")
    out: list[str] = []
    i = 0
    while i < len(lines):
        if _USE_RE.match(lines[i]):
            block: list[str] = []
            while i < len(lines):
                if _USE_RE.match(lines[i]):
                    block.append(lines[i])
                    i += 1
                elif (
                    lines[i] == ""
                    and i + 1 < len(lines)
                    and _USE_RE.match(lines[i + 1])
                ):
                    i += 1
                else:
                    break
            out.extend(sorted(block))
        else:
            out.append(lines[i])
            i += 1
    return "\n".join(out)


def _build_per_file_literal_swaps() -> dict[str, list[tuple[str, str]]]:
    raw = json.loads(RENAME_MAP_PATH.read_text(encoding="utf-8"))
    swaps: dict[str, list[tuple[str, str]]] = {}
    for entry in raw["literal_replacements"]:
        fname = Path(entry["file"]).name
        swaps.setdefault(fname, []).append((entry["to"], entry["from"]))
    return swaps


def _apply_literal_swaps(text: str, pairs: list[tuple[str, str]]) -> str:
    for to_str, from_str in pairs:
        text = text.replace(to_str, from_str)
    return text


def _rustfmt(text: str) -> str:
    """Run rustfmt on the given text; return the canonical form. If rustfmt
    rejects the input (which can happen on a header-stripped fragment that has
    an unclosed module attribute, etc.) we fall back to the original text and
    rely on the surrounding swap normalisation."""
    proc = subprocess.run(
        ["rustfmt", "--edition", RUST_EDITION, "--emit", "stdout"],
        input=text,
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        return text
    return proc.stdout


def _normalise_local(
    fname: str,
    local_text: str,
    upstream_text: str,
    per_file_swaps: dict[str, list[tuple[str, str]]],
) -> str:
    text = _strip_attribution_header(local_text)
    text = _strip_local_only_windows_cfg(text, upstream_text)
    text = _strip_safety_trail_comments(text)
    text = _apply_literal_swaps(text, per_file_swaps.get(fname, []))
    for pat, repl in USE_PATH_SWAPS:
        text = pat.sub(repl, text)
    for pat, repl in BOTH_SIDES_SWAPS:
        text = pat.sub(repl, text)
    text = _collapse_and_sort_use_groups(text)
    return _rustfmt(text)


def _normalise_upstream(upstream_text: str) -> str:
    text = upstream_text
    for pat, repl in BOTH_SIDES_SWAPS:
        text = pat.sub(repl, text)
    text = _collapse_and_sort_use_groups(text)
    return _rustfmt(text)


def _sha(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def main() -> int:
    if shutil.which("rustfmt") is None:
        print(
            "ERROR: rustfmt not found on PATH; this drift guard relies on it "
            "to canonicalise formatting before hashing.",
            file=sys.stderr,
        )
        return 2

    overlap = set(VERBATIM_FILES) & set(EXTENDED_FILES)
    if overlap:
        for name in sorted(overlap):
            print(
                f"config error: {name} appears in both VERBATIM_FILES and "
                "EXTENDED_FILES",
                file=sys.stderr,
            )
        return 2

    per_file_swaps = _build_per_file_literal_swaps()

    drift: list[str] = []
    missing: list[str] = []

    for fname in VERBATIM_FILES:
        local = LOCAL_ROOT / fname
        upstream = UPSTREAM_ROOT / fname
        if not local.exists():
            missing.append(f"local missing: {local.relative_to(REPO_ROOT)}")
            continue
        if not upstream.exists():
            missing.append(f"upstream missing: {upstream.relative_to(REPO_ROOT)}")
            continue
        local_norm = _normalise_local(
            fname,
            local.read_text(encoding="utf-8"),
            upstream.read_text(encoding="utf-8"),
            per_file_swaps,
        )
        upstream_norm = _normalise_upstream(upstream.read_text(encoding="utf-8"))
        if _sha(local_norm) != _sha(upstream_norm):
            drift.append(
                f"DRIFT: {local.relative_to(REPO_ROOT)} != "
                f"{upstream.relative_to(REPO_ROOT)} "
                "(after attribution-strip + cfg-strip + rename-map swap + "
                "use-path swap + rustfmt)"
            )

    classified = set(VERBATIM_FILES) | set(EXTENDED_FILES)
    upstream_extras = sorted(
        p.name for p in UPSTREAM_ROOT.glob("*.rs") if p.name not in classified
    )
    for name in upstream_extras:
        missing.append(
            f"upstream has unclassified file: {name} "
            "(add to VERBATIM_FILES after porting, or to EXTENDED_FILES with "
            "rationale)"
        )

    if missing:
        for line in missing:
            print(line, file=sys.stderr)
        return 2
    if drift:
        for line in drift:
            print(line, file=sys.stderr)
        print(
            "\nADR-129 §1.3 + ADR-145 §6 forbid hand-edits to verbatim ports. "
            "If upstream changed, re-vendor codex-windows-sandbox first; if "
            "the divergence is intentional x-claw glue, move the file from "
            "VERBATIM_FILES to EXTENDED_FILES with a rationale, or extend the "
            "rename map / USE_PATH_SWAPS list with a documented entry.",
            file=sys.stderr,
        )
        return 1
    print(
        f"OK: {len(VERBATIM_FILES)} files verbatim-match upstream "
        f"(plus {len(EXTENDED_FILES)} extended/glue files skipped)."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
