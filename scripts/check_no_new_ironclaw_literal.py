#!/usr/bin/env python3.12
"""
ADR-114 grep guard：禁止 PR diff 中引入新的 ``.ironclaw`` / ``IRONCLAW_BASE_DIR`` 字面量。

ADR-114 把 ``.ironclaw`` → ``.dasclaw`` 命名空间迁移划成「类 A 顺手改」+「类 B
集中工程」。所有「类 A」PR 都必须**不**新增 ``.ironclaw`` 字面量；只有显式标记为
``adr-114-class-b`` 的 PR 才允许操作这些字面量（CI 通过 label 决定是否跳过本 job）。

行为：
  对 ``base..head`` 的 unified=0 diff 遍历每一条**新增**行（以 ``+`` 开头但不是
  ``+++`` 的行），用如下正则做命中匹配::

      \\.ironclaw\\b
      \\bIRONCLAW_BASE_DIR\\b

  任意命中即失败，并打印文件路径 + 新增行号 + 命中片段。

文件级白名单（始终豁免）：
  - ``docs/plans/architecture-refactor/adr-114-*.md``    — ADR 自身要展示字面量
  - ``scripts/check_no_new_ironclaw_literal.py``         — 本脚本含正则字面量

CI label 豁免：在 ``.github/workflows/code_style.yml`` 的对应 job 上用
``if: !contains(...labels..., 'adr-114-class-b')`` 整体跳过本检查（不在脚本侧
判断 label，避免脚本依赖 GH API）。

本地用法::

    python3.12 scripts/check_no_new_ironclaw_literal.py --base origin/xClaw

CI 用法::

    python3 scripts/check_no_new_ironclaw_literal.py --base "$BASE" --head HEAD

退出码：0 = 干净；1 = 命中至少一条新字面量。
"""

from __future__ import annotations

import argparse
import fnmatch
import re
import subprocess
import sys
import unittest
from dataclasses import dataclass


LITERAL_PATTERN = re.compile(r"\.ironclaw\b|\bIRONCLAW_BASE_DIR\b")

# 文件级白名单（glob，相对仓库根）。仅用于「文件本身的存在意义就是描述这两个
# 字面量」的少数 meta 文件——业务代码绝不允许加进来。
FILE_WHITELIST: tuple[str, ...] = (
    "docs/plans/architecture-refactor/adr-114-*.md",
    "scripts/check_no_new_ironclaw_literal.py",
    ".github/workflows/code_style.yml",
    ".github/pull_request_template.md",
    "AGENTS.md",
)


@dataclass(frozen=True)
class Violation:
    path: str
    line_no: int
    snippet: str


def is_whitelisted(path: str) -> bool:
    return any(fnmatch.fnmatch(path, pat) for pat in FILE_WHITELIST)


def parse_added_lines(diff_text: str) -> list[tuple[str, int, str]]:
    """从 ``git diff -U0`` 输出中解析 (path, new_line_no, line) 三元组。

    只收集**新增**行（以 ``+`` 开头但不是 ``+++`` 文件头）。
    """
    results: list[tuple[str, int, str]] = []
    current_path: str | None = None
    new_line_no = 0

    hunk_re = re.compile(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@")

    for raw in diff_text.splitlines():
        if raw.startswith("+++ "):
            # 形如 "+++ b/path/to/file" 或 "+++ /dev/null"
            target = raw[4:].strip()
            if target == "/dev/null":
                current_path = None
            elif target.startswith("b/"):
                current_path = target[2:]
            else:
                current_path = target
            continue
        if raw.startswith("--- "):
            continue
        if raw.startswith("@@"):
            m = hunk_re.match(raw)
            if m:
                new_line_no = int(m.group(1))
            continue
        if raw.startswith("+") and not raw.startswith("+++"):
            if current_path is not None:
                results.append((current_path, new_line_no, raw[1:]))
            new_line_no += 1
            continue
        if raw.startswith("-") and not raw.startswith("---"):
            # 删除行不增加 new 端行号
            continue
        # 上下文行（-U0 下基本不出现，但保险起见）
        new_line_no += 1

    return results


def collect_violations(base: str, head: str) -> list[Violation]:
    out = subprocess.run(
        ["git", "diff", "-U0", f"{base}..{head}"],
        check=True,
        capture_output=True,
        text=True,
    )
    violations: list[Violation] = []
    for path, line_no, line in parse_added_lines(out.stdout):
        if is_whitelisted(path):
            continue
        m = LITERAL_PATTERN.search(line)
        if m:
            violations.append(Violation(path=path, line_no=line_no, snippet=line.rstrip()))
    return violations


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", help="base ref (e.g. origin/xClaw)")
    parser.add_argument("--head", default="HEAD", help="head ref (default: HEAD)")
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="run inline unittests instead of scanning git diff",
    )
    args = parser.parse_args(argv)

    if args.self_test:
        return _run_self_test()

    if not args.base:
        parser.error("--base is required unless --self-test is passed")

    violations = collect_violations(args.base, args.head)
    if not violations:
        print("OK: No new .ironclaw / IRONCLAW_BASE_DIR literals introduced.")
        return 0

    print("::error::ADR-114 grep guard 命中：禁止新增 .ironclaw / IRONCLAW_BASE_DIR 字面量")
    print("参考: docs/plans/architecture-refactor/adr-114-dasclaw-rebrand.md §4.2")
    print("如确属类 B 集中工程，给 PR 打 'adr-114-class-b' label（CI 会自动豁免）。")
    print("")
    for v in violations[:20]:
        print(f"{v.path}:{v.line_no}: {v.snippet}")
    print("")
    print(f"Total: {len(violations)} violation(s)")
    return 1


# ---------------------------------------------------------------------------
# 内联测试：python3.12 scripts/check_no_new_ironclaw_literal.py --self-test
# ---------------------------------------------------------------------------


def _run_self_test() -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(_GuardTests)
    runner = unittest.TextTestRunner(verbosity=2)
    return 0 if runner.run(suite).wasSuccessful() else 1


class _GuardTests(unittest.TestCase):
    def test_pattern_matches_dot_ironclaw(self) -> None:
        self.assertIsNotNone(LITERAL_PATTERN.search('let p = "~/.ironclaw/db";'))
        self.assertIsNotNone(LITERAL_PATTERN.search("dirs::home_dir().join(\".ironclaw\")"))

    def test_pattern_matches_env_var(self) -> None:
        self.assertIsNotNone(LITERAL_PATTERN.search('env::var("IRONCLAW_BASE_DIR")'))

    def test_pattern_skips_plain_word(self) -> None:
        # 没有前导点，不算命名空间字面量
        self.assertIsNone(LITERAL_PATTERN.search("desktop-client/ironclaw/src/main.rs"))
        self.assertIsNone(LITERAL_PATTERN.search('package = "ironclaw"'))

    def test_pattern_skips_extended_word(self) -> None:
        # \b 边界确保 IRONCLAW_BASE_DIR_LEGACY 不被作为本守卫的关注点
        # （扩展名称如有歧义应在 ADR 单独处理）
        self.assertIsNone(LITERAL_PATTERN.search("IRONCLAW_BASE_DIR_LEGACY"))
        # .ironclawish 不是 .ironclaw 命名空间
        self.assertIsNone(LITERAL_PATTERN.search("foo.ironclawish"))

    def test_whitelist_adr_doc(self) -> None:
        self.assertTrue(is_whitelisted("docs/plans/architecture-refactor/adr-114-dasclaw-rebrand.md"))
        self.assertTrue(is_whitelisted("scripts/check_no_new_ironclaw_literal.py"))
        self.assertTrue(is_whitelisted(".github/workflows/code_style.yml"))
        self.assertTrue(is_whitelisted(".github/pull_request_template.md"))
        self.assertFalse(is_whitelisted("desktop-client/ironclaw/src/bootstrap.rs"))
        self.assertFalse(is_whitelisted("docs/plans/architecture-refactor/adr-115-foo.md"))
        self.assertFalse(is_whitelisted(".github/workflows/test.yml"))

    def test_parse_added_lines_basic(self) -> None:
        diff = (
            "diff --git a/foo.rs b/foo.rs\n"
            "--- a/foo.rs\n"
            "+++ b/foo.rs\n"
            "@@ -10,0 +11,2 @@\n"
            "+let p = \"~/.ironclaw/db\";\n"
            "+let q = 42;\n"
        )
        added = parse_added_lines(diff)
        self.assertEqual(
            added,
            [
                ("foo.rs", 11, 'let p = "~/.ironclaw/db";'),
                ("foo.rs", 12, "let q = 42;"),
            ],
        )

    def test_parse_added_lines_skips_deletions_and_dev_null(self) -> None:
        diff = (
            "diff --git a/old.rs b/old.rs\n"
            "--- a/old.rs\n"
            "+++ /dev/null\n"
            "@@ -1,1 +0,0 @@\n"
            "-let stale = \"~/.ironclaw/old\";\n"
        )
        # 整文件删除不应当作新增 → 无违规
        added = parse_added_lines(diff)
        self.assertEqual(added, [])


if __name__ == "__main__":
    raise SystemExit(main())
