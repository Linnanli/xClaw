# Sandbox 进程启动延迟基线（W2 验收 / B6-2）

> Issue: [#380](https://github.com/Linnanli/xClaw/issues/380) Batch 6 B6-2  
> Plan reference: [`32-execution-plan.md`](32-execution-plan.md) · "性能基准：进程沙箱启动 < 50ms (对比 codex baseline)"

## 目的

固化"进程沙箱启动到一个 noop 子进程退出"的端到端 wall-clock 延迟基线，
为后续 W2.2/W2.3/W2.4 的真实 backend（macOS Seatbelt / Linux landlock+bubblewrap /
Windows RestrictedToken）提供可比较的参照数据，并给 CI 一条可执行的 gate（默认 p50 ≤ 50 ms）。

## 方法论

直接调用 host 提供的进程沙箱 CLI 包装一个最便宜的 noop（`/usr/bin/true`），
**测的是"沙箱前置成本 + fork/exec + ld.so + true 退出"端到端 wall-clock**，不是
沙箱内部子模块（policy parsing、profile compile）单独的耗时。

| 后端 | 命令 | 备注 |
|------|------|------|
| `sandbox-exec` (macOS) | `/usr/bin/sandbox-exec -p '(version 1)(allow default)' /usr/bin/true` | `(allow default)` 是最小 profile，不阻断任何系统调用，测的是 `sandbox-exec` 本身的 fork+ profile install 成本 |
| `bwrap` (Linux) | `bwrap --dev-bind / / --proc /proc /bin/true` | 以最少 mount 的容器执行 noop |
| `baseline` (对照) | `/usr/bin/true` | 不带任何沙箱，作 fork+exec 基线，用于隔离纯 OS 进程创建成本 |

执行样本：默认 100 次（`--runs 100`），10 次 warmup（`--warmup 10`）丢弃。
统计：`min / mean / p50 / p95 / p99 / max`，p50 作为阈值判定指标。

> **不对 codex 自身跑分**：codex 上游的 sandbox 调用最终也是壳到 `sandbox-exec` /
> `bwrap`，host 同后端的延迟即是 codex baseline；如未来需要 codex 端到端对比，
> 单独发起 follow-up，本基线提供横切对照所需的 host backend 数字。

## 用法

```bash
# auto-detect backend（macOS→sandbox-exec, Linux→bwrap, 其他→baseline）
python3.12 scripts/sandbox_perf_baseline.py --append-doc

# 显式 backend 与样本数
python3.12 scripts/sandbox_perf_baseline.py --backend sandbox-exec --runs 200 --warmup 20

# 仅作对照（裸 spawn）
python3.12 scripts/sandbox_perf_baseline.py --backend baseline --runs 100

# CI gate（p50 > 50 ms 时返回 exit code 2）
python3.12 scripts/sandbox_perf_baseline.py --backend auto --threshold-ms 50
```

脚本：[`scripts/sandbox_perf_baseline.py`](../../../scripts/sandbox_perf_baseline.py)

## 阈值

- **W2 验收红线**：p50 ≤ **50 ms**（来源：`32-execution-plan.md`）
- 超阈值：脚本以 exit code 2 退出，CI 应判失败
- 同 host 同后端的两轮跑分波动若 > 30 % p50，建议用 `--runs 300` 重测以收敛

## Runs

> 每次跑分追加在下方。脚本 `--append-doc` 自动维护，不要手改格式。

### 2026-05-13T05:22:01Z · Darwin 23.6.0 · backend=baseline

```json
{
  "backend": "baseline",
  "command": [
    "/usr/bin/true"
  ],
  "runs": 100,
  "warmup": 10,
  "host": {
    "system": "Darwin",
    "release": "23.6.0",
    "machine": "x86_64"
  },
  "stats_ms": {
    "min": 3.217,
    "p50": 4.078,
    "p95": 6.138,
    "p99": 6.883,
    "max": 6.991,
    "mean": 4.354
  },
  "iso_ts": "2026-05-13T05:22:01Z"
}
```

### 2026-05-13T05:23:09Z · Darwin 23.6.0 · backend=sandbox-exec

```json
{
  "backend": "sandbox-exec",
  "command": [
    "/usr/bin/sandbox-exec",
    "-p",
    "(version 1)(allow default)",
    "/usr/bin/true"
  ],
  "runs": 100,
  "warmup": 10,
  "host": {
    "system": "Darwin",
    "release": "23.6.0",
    "machine": "x86_64"
  },
  "stats_ms": {
    "min": 15.446,
    "p50": 16.598,
    "p95": 18.474,
    "p99": 21.446,
    "max": 158.14,
    "mean": 18.179
  },
  "iso_ts": "2026-05-13T05:23:09Z"
}
```
