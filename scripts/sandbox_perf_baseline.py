#!/usr/bin/env python3.12
"""B6-2 sandbox startup latency baseline.

跨平台测进程沙箱"启动到 noop 退出"的端到端 wall-clock 延迟。
W2 验收阈值：p50 < 50 ms。

支持后端：
- macOS  → /usr/bin/sandbox-exec  + 最小 (allow default) profile + /usr/bin/true
- Linux  → bwrap --dev-bind / / --proc /proc /bin/true
- 也可通过 --baseline-only 跳过沙箱、只测裸 spawn /usr/bin/true 作对照

输出：
- stdout: JSON 一行（host 元数据 + p50/p95/p99/min/max/n + samples_ms）
- 同时把同一份 JSON 追加到 docs/plans/architecture-refactor/sandbox-perf-baseline.md
  下方 "## Runs" 段（如果 --append-doc 启用）。

非目标：
- 不对 codex 自身跑分；codex baseline 数据按 W2 决定（人工跑或后续 CI 任务）
- 不测 dasclaw_exec → dasclaw_sandboxing 完整 pipeline；W2.1 的 backend 还是 stub
- 不做并发吞吐基准；本脚本只测启动延迟
"""

from __future__ import annotations

import argparse
import json
import platform
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DOC_PATH = (
    REPO_ROOT
    / "docs"
    / "plans"
    / "architecture-refactor"
    / "sandbox-perf-baseline.md"
)

MACOS_PROFILE = "(version 1)(allow default)"


def build_command(backend: str) -> list[str]:
    if backend == "baseline":
        return ["/usr/bin/true"] if Path("/usr/bin/true").exists() else ["true"]
    if backend == "sandbox-exec":
        return ["/usr/bin/sandbox-exec", "-p", MACOS_PROFILE, "/usr/bin/true"]
    if backend == "bwrap":
        return [
            "bwrap",
            "--dev-bind",
            "/",
            "/",
            "--proc",
            "/proc",
            "/bin/true",
        ]
    raise ValueError(f"unknown backend: {backend}")


def detect_default_backend() -> str:
    system = platform.system()
    if system == "Darwin" and shutil.which("sandbox-exec"):
        return "sandbox-exec"
    if system == "Linux" and shutil.which("bwrap"):
        return "bwrap"
    return "baseline"


def run_one(cmd: list[str]) -> float:
    start = time.perf_counter()
    proc = subprocess.run(cmd, capture_output=True)
    elapsed_ms = (time.perf_counter() - start) * 1000.0
    if proc.returncode != 0:
        raise RuntimeError(
            f"{cmd[0]} returned {proc.returncode}: stderr={proc.stderr!r}"
        )
    return elapsed_ms


def percentile(samples: list[float], p: float) -> float:
    """linear-interpolation percentile（statistics.quantiles 在 n<2 时会抛）。"""
    if not samples:
        raise ValueError("empty samples")
    if len(samples) == 1:
        return samples[0]
    ordered = sorted(samples)
    rank = p / 100.0 * (len(ordered) - 1)
    low = int(rank)
    high = min(low + 1, len(ordered) - 1)
    weight = rank - low
    return ordered[low] * (1 - weight) + ordered[high] * weight


def gather(backend: str, runs: int, warmup: int) -> dict:
    cmd = build_command(backend)
    # warmup（吃 fork/exec/page cache 冷启动），不计入统计
    for _ in range(warmup):
        run_one(cmd)
    samples = [run_one(cmd) for _ in range(runs)]
    return {
        "backend": backend,
        "command": cmd,
        "runs": runs,
        "warmup": warmup,
        "host": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
        },
        "stats_ms": {
            "min": round(min(samples), 3),
            "p50": round(percentile(samples, 50), 3),
            "p95": round(percentile(samples, 95), 3),
            "p99": round(percentile(samples, 99), 3),
            "max": round(max(samples), 3),
            "mean": round(statistics.fmean(samples), 3),
        },
        "samples_ms": [round(s, 3) for s in samples],
        "iso_ts": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }


def append_to_doc(payload: dict) -> None:
    if not DOC_PATH.exists():
        raise FileNotFoundError(
            f"{DOC_PATH} 不存在；先创建文档骨架再用 --append-doc。"
        )
    text = DOC_PATH.read_text(encoding="utf-8")
    marker = "## Runs"
    if marker not in text:
        raise RuntimeError(
            f"{DOC_PATH} 没找到 `## Runs` 段；检查文档结构。"
        )
    snippet = (
        f"\n### {payload['iso_ts']} · {payload['host']['system']} "
        f"{payload['host']['release']} · backend={payload['backend']}\n\n"
        "```json\n"
        f"{json.dumps({k: v for k, v in payload.items() if k != 'samples_ms'}, indent=2)}\n"
        "```\n"
    )
    DOC_PATH.write_text(text.rstrip() + "\n" + snippet, encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--backend",
        choices=["auto", "sandbox-exec", "bwrap", "baseline"],
        default="auto",
        help="auto = host 自动选；baseline = 不带沙箱裸 spawn 作对照",
    )
    parser.add_argument("--runs", type=int, default=100)
    parser.add_argument("--warmup", type=int, default=10)
    parser.add_argument(
        "--append-doc",
        action="store_true",
        help="追加结果到 docs/plans/architecture-refactor/sandbox-perf-baseline.md",
    )
    parser.add_argument(
        "--threshold-ms",
        type=float,
        default=50.0,
        help="p50 阈值；超过非 0 退出（用于 CI gate；默认 50ms）",
    )
    args = parser.parse_args()

    backend = detect_default_backend() if args.backend == "auto" else args.backend
    payload = gather(backend, args.runs, args.warmup)
    print(json.dumps(payload, indent=2))

    if args.append_doc:
        append_to_doc(payload)

    p50 = payload["stats_ms"]["p50"]
    if p50 > args.threshold_ms:
        print(
            f"\n[FAIL] p50={p50} ms 超过阈值 {args.threshold_ms} ms",
            file=sys.stderr,
        )
        return 2
    print(
        f"\n[OK] p50={p50} ms <= 阈值 {args.threshold_ms} ms",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
