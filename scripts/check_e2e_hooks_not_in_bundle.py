#!/usr/bin/env python3.12
"""
E2E hook bundle guard: ensure tauri_plugin_webdriver (and future E2E-only
plugins) is only referenced behind the established debug-only feature gate
``cfg(all(debug_assertions, feature = "webdriver"))``.

Why this guard exists
---------------------
PR #1037 introduced a WebDriver automation plugin for E2E testing. The plugin
must NEVER ship in production releases. We rely on three layers of defense:

  1. Cargo.toml: ``tauri-plugin-webdriver`` is ``optional = true`` and only
     pulled in when ``webdriver`` feature is enabled.
  2. main.rs: the plugin is registered inside
     ``#[cfg(all(debug_assertions, feature = "webdriver"))]``.
  3. **This grep guard** (CI): rejects any source file under
     ``desktop-client/src/`` that mentions ``tauri_plugin_webdriver`` without
     the cfg gate immediately preceding the line.

Scope
-----
Scans ``desktop-client/src/**/*.rs`` only (production crate sources). Test
files under ``desktop-client/tests/`` and dev-only modules are out of scope —
they never ship in the bundle.

Rule
----
For every line in scope containing the literal ``tauri_plugin_webdriver``,
one of the **preceding 5 lines** must contain ``debug_assertions`` AND
``feature = "webdriver"``. Otherwise it's a violation.

Local usage
-----------
::

    python3.12 scripts/check_e2e_hooks_not_in_bundle.py

CI usage
--------
::

    python3 scripts/check_e2e_hooks_not_in_bundle.py

Exit codes: 0 = clean; 1 = at least one violation.
"""

from __future__ import annotations

import argparse
import re
import sys
import unittest
from dataclasses import dataclass
from pathlib import Path


# Hooks that must never ship in production bundle without proper cfg gate.
# Keep this list narrow — only true E2E-only crates belong here.
HOOK_PATTERNS: tuple[str, ...] = ("tauri_plugin_webdriver",)

# Required cfg gate signals (BOTH must appear in the same window above the
# hook reference).
REQUIRED_GATE_SIGNALS: tuple[str, ...] = ("debug_assertions", 'feature = "webdriver"')

# Look back this many lines for the cfg gate. Accounts for the gate attribute
# itself plus a few lines of doc comments or formatting between gate and hook
# use. See desktop-client/src/main.rs:60-63 for the canonical pattern.
GATE_WINDOW = 5

# Files scanned (relative to repo root). Glob patterns.
SCAN_GLOBS: tuple[str, ...] = ("desktop-client/src/**/*.rs",)

# Whitelist: this script itself + workflow yml that mentions the hook name.
PATH_WHITELIST: tuple[str, ...] = (
    "scripts/check_e2e_hooks_not_in_bundle.py",
    ".github/workflows/code_style.yml",
)


@dataclass(frozen=True)
class Violation:
    path: str
    line_no: int
    hook: str
    snippet: str


def is_whitelisted(path: str) -> bool:
    return path in PATH_WHITELIST


def find_violations_in_text(path: str, text: str) -> list[Violation]:
    lines = text.splitlines()
    violations: list[Violation] = []
    for idx, line in enumerate(lines):
        for hook in HOOK_PATTERNS:
            if hook not in line:
                continue
            # Look back GATE_WINDOW lines for the required cfg gate.
            window_start = max(0, idx - GATE_WINDOW)
            window = "\n".join(lines[window_start:idx])
            if all(sig in window for sig in REQUIRED_GATE_SIGNALS):
                continue
            # Allow the same line to also contain the gate (single-line cfg).
            if all(sig in line for sig in REQUIRED_GATE_SIGNALS):
                continue
            violations.append(
                Violation(
                    path=path,
                    line_no=idx + 1,
                    hook=hook,
                    snippet=line.rstrip(),
                )
            )
    return violations


def collect_violations(repo_root: Path) -> list[Violation]:
    violations: list[Violation] = []
    for glob in SCAN_GLOBS:
        for file_path in sorted(repo_root.glob(glob)):
            rel = file_path.relative_to(repo_root).as_posix()
            if is_whitelisted(rel):
                continue
            try:
                text = file_path.read_text(encoding="utf-8")
            except (OSError, UnicodeDecodeError):
                continue
            violations.extend(find_violations_in_text(rel, text))
    return violations


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo-root",
        default=".",
        help="repository root (default: cwd)",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="run inline unittests instead of scanning the tree",
    )
    args = parser.parse_args(argv)

    if args.self_test:
        return _run_self_test()

    violations = collect_violations(Path(args.repo_root).resolve())
    if not violations:
        print("OK: No ungated E2E hook references found in production bundle sources.")
        return 0

    print("::error::E2E hook bundle guard hit: references to E2E-only plugins")
    print("must be gated by cfg(all(debug_assertions, feature = \"webdriver\")).")
    print("")
    for v in violations[:20]:
        print(f"{v.path}:{v.line_no}: [{v.hook}] {v.snippet}")
    print("")
    print(f"Total: {len(violations)} violation(s)")
    return 1


# ---------------------------------------------------------------------------
# Inline tests: python3.12 scripts/check_e2e_hooks_not_in_bundle.py --self-test
# ---------------------------------------------------------------------------


def _run_self_test() -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(_GuardTests)
    runner = unittest.TextTestRunner(verbosity=2)
    return 0 if runner.run(suite).wasSuccessful() else 1


class _GuardTests(unittest.TestCase):
    def test_properly_gated_passes(self) -> None:
        src = (
            "    #[cfg(all(debug_assertions, feature = \"webdriver\"))]\n"
            "    let builder = builder.plugin(tauri_plugin_webdriver::init());\n"
        )
        self.assertEqual(find_violations_in_text("main.rs", src), [])

    def test_ungated_use_fails(self) -> None:
        src = "let b = b.plugin(tauri_plugin_webdriver::init());\n"
        v = find_violations_in_text("main.rs", src)
        self.assertEqual(len(v), 1)
        self.assertEqual(v[0].hook, "tauri_plugin_webdriver")

    def test_gate_missing_debug_assertions_fails(self) -> None:
        src = (
            "#[cfg(feature = \"webdriver\")]\n"
            "let b = b.plugin(tauri_plugin_webdriver::init());\n"
        )
        v = find_violations_in_text("main.rs", src)
        self.assertEqual(len(v), 1)

    def test_gate_missing_feature_fails(self) -> None:
        src = (
            "#[cfg(debug_assertions)]\n"
            "let b = b.plugin(tauri_plugin_webdriver::init());\n"
        )
        v = find_violations_in_text("main.rs", src)
        self.assertEqual(len(v), 1)

    def test_gate_far_above_fails(self) -> None:
        # Cfg gate 10 lines above (outside GATE_WINDOW=5) doesn't count.
        prefix = "#[cfg(all(debug_assertions, feature = \"webdriver\"))]\n"
        body = "\n".join(["// blank"] * 10) + "\n"
        src = prefix + body + "let b = b.plugin(tauri_plugin_webdriver::init());\n"
        v = find_violations_in_text("main.rs", src)
        self.assertEqual(len(v), 1)

    def test_gate_within_window_passes(self) -> None:
        # 4-line gap is within GATE_WINDOW=5.
        prefix = "#[cfg(all(debug_assertions, feature = \"webdriver\"))]\n"
        body = "\n".join(["// blank"] * 3) + "\n"
        src = prefix + body + "let b = b.plugin(tauri_plugin_webdriver::init());\n"
        self.assertEqual(find_violations_in_text("main.rs", src), [])

    def test_single_line_gate_passes(self) -> None:
        # Defensive: same-line gate also accepted (rare style).
        src = "#[cfg(all(debug_assertions, feature = \"webdriver\"))] tauri_plugin_webdriver::init();\n"
        self.assertEqual(find_violations_in_text("main.rs", src), [])

    def test_whitelist(self) -> None:
        self.assertTrue(is_whitelisted("scripts/check_e2e_hooks_not_in_bundle.py"))
        self.assertTrue(is_whitelisted(".github/workflows/code_style.yml"))
        self.assertFalse(is_whitelisted("desktop-client/src/main.rs"))


if __name__ == "__main__":
    sys.exit(main())
