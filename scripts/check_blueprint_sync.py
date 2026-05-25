#!/usr/bin/env python3.12
"""Blueprint ↔ crates/ sync guard.

Verifies that the set of `dasclaw_*` crate directories under `crates/` is
in sync with the names mentioned in the target architecture blueprint at
`docs/plans/architecture-refactor/31-target-architecture.md`.

Two failure modes:

  1. **Crate exists but blueprint never mentions it.** The blueprint
     drifted behind reality (the W3-W6 anti-pattern this guard is built to
     prevent). Either add the crate to §4 or document why it is intentional.

  2. **Blueprint names a crate that does not exist under crates/.** Either
     the crate was renamed without updating the blueprint, or it is a
     planned-but-not-yet-built entry that must be listed in `PLANNED` below.

Run: python3.12 scripts/check_blueprint_sync.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
CRATES_DIR = REPO_ROOT / "crates"
BLUEPRINT = (
    REPO_ROOT
    / "docs"
    / "plans"
    / "architecture-refactor"
    / "31-target-architecture.md"
)

# Names that appear in the blueprint but are NOT yet (or never will be)
# real crate directories. Each entry MUST have a reason and a tracking
# ADR / issue / milestone so future readers can prune as work lands.
PLANNED: dict[str, str] = {
    # P0 gap — bridge layer between dasclaw_core and host implementations
    # (31 §4.1 P0 list, ADR-101 long-term, not yet built).
    "dasclaw_bridge_lite": "ADR-101 §P0 long-term gap; not yet built",
    # F4.6 工具壳分层 (ADR-156). Eight new crates land in waves F4.6.1 ~
    # F4.6.8; remove an entry from PLANNED the moment its crate directory
    # is created.
    # F4.6.1 (`dasclaw_misc_tools`) landed in PR #761 — entry removed.
    "dasclaw_memory_tools": "ADR-156 §6.3.1 — 取消下沉 (desktop-only, runtime workspace dep)",
    # F4.6.2 (`dasclaw_image_tools`) landed — entry removed.
    # F4.6.4 (`dasclaw_sub_agent_tools`) landed — entry removed.
    # F4.6.6-a (`dasclaw_fs_tools`) landed — entry removed.
    # F4.6.7 (`dasclaw_shell_tools`) phase 1 landed — entry removed.
    # F4.6.8 (`dasclaw_net_tools`) phase 1 (web_fetch) landed — entry removed.
}

# Symbols that look like crate names but are not. Examples: conceptual
# umbrella terms used in prose (`dasclaw_tools`, `dasclaw_common`),
# placeholders for symbol families (`dasclaw_parsed_command` refers to
# the type inside dasclaw_protocol, not a crate), or regex-edge artifacts
# (`dasclaw_sandbox_` from trailing-underscore matches).
KNOWN_NON_CRATES: set[str] = {
    "dasclaw_common",  # umbrella term in prose, not a crate
    "dasclaw_parsed_command",  # symbol inside dasclaw_protocol
    "dasclaw_sandbox_",  # regex artefact: matches before _linux/_windows boundary
    "dasclaw_tool",  # crate dasclaw_tool DOES exist; singular vs plural is fine
    "dasclaw_tools",  # umbrella term for the F4.6 tool family
    "dasclaw_utils_template",  # template-only reference in §1.3, not a crate
}

CRATE_NAME_RE = re.compile(r"dasclaw_[a-z0-9_]+")


def scan_blueprint(text: str) -> set[str]:
    """Extract every `dasclaw_*` token mentioned in the blueprint."""
    return set(CRATE_NAME_RE.findall(text))


def scan_crates_dir(crates_dir: Path) -> set[str]:
    """List actual `dasclaw_*` subdirectories under crates/."""
    return {
        entry.name
        for entry in crates_dir.iterdir()
        if entry.is_dir() and entry.name.startswith("dasclaw_")
    }


def main() -> int:
    if not BLUEPRINT.exists():
        print(f"blueprint missing: {BLUEPRINT.relative_to(REPO_ROOT)}", file=sys.stderr)
        return 2
    if not CRATES_DIR.exists():
        print(f"crates/ missing: {CRATES_DIR.relative_to(REPO_ROOT)}", file=sys.stderr)
        return 2

    blueprint_mentions = scan_blueprint(BLUEPRINT.read_text(encoding="utf-8"))
    real_crates = scan_crates_dir(CRATES_DIR)
    planned = set(PLANNED.keys())
    known_non_crates = KNOWN_NON_CRATES

    # Failure 1: crate on disk that the blueprint never mentions.
    missing_from_blueprint = sorted(real_crates - blueprint_mentions)

    # Failure 2: blueprint name that has no crate on disk and is not
    # explicitly planned or marked as a non-crate symbol.
    ghosts_in_blueprint = sorted(
        blueprint_mentions - real_crates - planned - known_non_crates
    )

    errors_found = bool(missing_from_blueprint) or bool(ghosts_in_blueprint)

    if missing_from_blueprint:
        print(
            "ERROR: crates exist under crates/ but the blueprint never mentions them.\n"
            "       Either add them to docs/plans/architecture-refactor/31-target-architecture.md §4,\n"
            "       or rename / remove the crate. Drift list:",
            file=sys.stderr,
        )
        for name in missing_from_blueprint:
            print(f"  - {name}", file=sys.stderr)
        print("", file=sys.stderr)

    if ghosts_in_blueprint:
        print(
            "ERROR: blueprint references crates that do not exist under crates/.\n"
            "       Either create the crate, fix the name in the blueprint, or add\n"
            "       it to PLANNED in scripts/check_blueprint_sync.py with a reason.\n"
            "       Ghost list:",
            file=sys.stderr,
        )
        for name in ghosts_in_blueprint:
            print(f"  - {name}", file=sys.stderr)
        print("", file=sys.stderr)

    if errors_found:
        return 1

    planned_realized = sorted(planned & real_crates)
    if planned_realized:
        print(
            "WARN: PLANNED entries now have real crates; remove them from PLANNED:",
            file=sys.stderr,
        )
        for name in planned_realized:
            print(f"  - {name} ({PLANNED[name]})", file=sys.stderr)
        return 1

    print(
        f"OK: {len(real_crates)} crates on disk, "
        f"{len(planned)} planned (ADR-156 / ADR-101), "
        f"blueprint mentions consistent."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
