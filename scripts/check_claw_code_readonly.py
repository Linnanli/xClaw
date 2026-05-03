#!/usr/bin/env python3.12
"""
ADR-118 红线守卫：claw-code/ 子仓只读化检查。

W6-C PR-A.4 完成后（#158 merged），主仓代码不再依赖 claw-code/ 路径下
的任何 crate（path-dep 已替换为 `crates/dasclaw_llm_provider/`）。
本守卫从父仓角度禁止 PR 修改 `claw-code` submodule pointer——任何
能力端口都应通过 `dasclaw_*` 自实现路径完成，不应再 bump 子仓 commit。

使用方法（本地）：
    python3.12 scripts/check_claw_code_readonly.py --base origin/xClaw

使用方法（CI）：
    python3 scripts/check_claw_code_readonly.py --base "$BASE" --head HEAD

例外：
    无白名单。如确需破例（例：补一个安全 critical 的 fix 到子仓），
    应先在主仓提交一个修订 ADR-118 的 PR 显式开闸，再 bump pointer。
"""
from __future__ import annotations

import argparse
import subprocess
import sys

GUARDED_SUBMODULE: str = "claw-code"


def changed_paths(base: str, head: str) -> list[str]:
    """返回 base..head 之间发生变更的路径（含 submodule 路径条目）。"""
    out = subprocess.run(
        ["git", "diff", "--name-only", f"{base}..{head}"],
        check=True,
        capture_output=True,
        text=True,
    )
    return [line for line in out.stdout.splitlines() if line]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True, help="base ref (e.g. origin/xClaw)")
    parser.add_argument("--head", default="HEAD", help="head ref (default: HEAD)")
    args = parser.parse_args()

    paths = changed_paths(args.base, args.head)
    if GUARDED_SUBMODULE in paths:
        sys.stderr.write(
            "ERROR: claw-code/ submodule pointer 已被改动；ADR-118 已将该子仓\n"
            "标记为只读，禁止 bump pointer。\n\n"
            "如需端口能力到主仓，请走 dasclaw_* crate 自实现路径，参考：\n"
            "  docs/plans/architecture-refactor/adr-118-claw-code-readonly-and-self-impl.md\n\n"
            "如确需破例修订 ADR-118 重新开闸，请先提交独立 PR 修改 ADR，\n"
            "再 bump submodule pointer。\n"
        )
        return 1

    print("OK: claw-code submodule pointer unchanged.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
