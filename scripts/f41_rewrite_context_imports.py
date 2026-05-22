#!/usr/bin/env python3
"""F4.1 import rewriter — replaces `use {crate,ironclaw}::context::*` shim
imports with direct references to dasclaw_core / dasclaw_runtime.

Usage:
    python3 scripts/f41_rewrite_context_imports.py <file>...

Each input file is rewritten in place. Lines that don't match the
`use <root>::context::{...};` or `use <root>::context::X;` form are left
untouched. `<root>` is `crate` (for desktop-client/ironclaw/src and
desktop-client/ironclaw/tests when in same crate) or `ironclaw` (for
desktop-client/src and integration tests in desktop-client/ironclaw/tests).

For each captured symbol we map to a target crate based on §11.8.9.3 of
ADR-152:

    Memory / ActionRecord / ConversationMemory       -> dasclaw_core::context::memory
    ContextManager / ContextSummary
        / FallbackDeliverable / JobContext            -> dasclaw_runtime::context
    JobState / StateTransition / TokenBudgetExceeded -> dasclaw_runtime
"""

import re
import sys
from pathlib import Path

MEMORY = {"ActionRecord", "ConversationMemory", "Memory"}
RUNTIME_CTX = {"ContextManager", "ContextSummary", "FallbackDeliverable", "JobContext"}
RUNTIME_ROOT = {"JobState", "StateTransition", "TokenBudgetExceeded"}
ALL_SYMS = MEMORY | RUNTIME_CTX | RUNTIME_ROOT

USE_RE = re.compile(
    r"^(?P<indent>\s*)(?P<vis>pub(?:\s*\([^)]*\))?\s+)?use\s+(?P<root>crate|ironclaw)::context::(?P<rest>[^;]+);\s*$"
)


def render(group: set[str], target: str) -> str:
    items = sorted(group)
    if len(items) == 1:
        return f"use {target}::{items[0]};"
    return "use " + target + "::{" + ", ".join(items) + "};"


def rewrite_line(line: str) -> list[str] | None:
    m = USE_RE.match(line)
    if not m:
        return None
    rest = m.group("rest").strip()
    if rest.startswith("{") and rest.endswith("}"):
        syms_text = rest[1:-1]
    else:
        syms_text = rest
    symbols = [s.strip() for s in syms_text.split(",") if s.strip()]
    if not symbols:
        return None
    if not set(symbols).issubset(ALL_SYMS):
        return None  # unknown symbol — bail out to keep things safe

    indent = m.group("indent") or ""
    vis = m.group("vis") or ""
    prefix = f"{indent}{vis}".replace("use", "").rstrip()
    if prefix:
        prefix += " "
    # Actually rebuild line-by-line with indent + vis preserved
    mem = {s for s in symbols if s in MEMORY}
    ctx = {s for s in symbols if s in RUNTIME_CTX}
    rt = {s for s in symbols if s in RUNTIME_ROOT}

    out: list[str] = []
    if mem:
        out.append(f"{indent}{vis}{render(mem, 'dasclaw_core::context::memory')}")
    if ctx:
        out.append(f"{indent}{vis}{render(ctx, 'dasclaw_runtime::context')}")
    if rt:
        out.append(f"{indent}{vis}{render(rt, 'dasclaw_runtime')}")
    return out


def process_file(path: Path) -> int:
    text = path.read_text(encoding="utf-8")
    lines = text.splitlines(keepends=False)
    out: list[str] = []
    changed = 0
    for line in lines:
        rewritten = rewrite_line(line)
        if rewritten is None:
            out.append(line)
        else:
            out.extend(rewritten)
            changed += 1
    if changed:
        new_text = "\n".join(out)
        if text.endswith("\n"):
            new_text += "\n"
        path.write_text(new_text, encoding="utf-8")
    return changed


def main() -> None:
    total = 0
    for arg in sys.argv[1:]:
        n = process_file(Path(arg))
        if n:
            print(f"{arg}: {n}")
        total += n
    print(f"total: {total}")


if __name__ == "__main__":
    main()
