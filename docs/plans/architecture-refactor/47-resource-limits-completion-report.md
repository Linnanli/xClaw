# 47 — Resource Limits (W3.3) Completion Report

> **Status**: completed in W3.3-3 (PR stack #30 / #32 / this PR).
> **Date**: 2026-04-28
> **Refs**: [ADR-45](./45-resource-limits-adr.md), [46 macOS memory research](./46-macos-memory-limit-research.md)

## 概要

W3.3 把 codex 已有但 dasclaw 缺失的"进程级资源限额"补齐。最终落地：

| 限额维度 | Linux 后端 | macOS 后端 | 默认值 |
|---------|-----------|------------|--------|
| 内存（virtual address space） | `RLIMIT_AS`（pre_exec） | — | 4 GiB |
| 内存（RSS, 真实物理） | **cgroup v2 `memory.max`**（spawn 后） | **`memorystatus_control` SPI**（spawn 后） | 4 GiB |
| CPU 时间 | `RLIMIT_CPU` | `RLIMIT_CPU` | 600 s |
| 打开文件数 | `RLIMIT_NOFILE` | `RLIMIT_NOFILE` | 1024 |
| 子进程数 | `RLIMIT_NPROC` | `RLIMIT_NPROC` | 1024 |

两层互补：
- **rlimit 层**（pre_exec hook）始终生效，覆盖 CPU/FD/NPROC，**Linux 上**也兜底覆盖 memory。
- **kernel-native 层**（cgroup v2 / memorystatus_control）按平台启用，做 RSS-based 内存限制——RLIMIT_AS 在 macOS 直接 EINVAL，在 Linux 也容易误伤 `cargo build` 之类内存稀疏使用的工具。

## PR 切片

| PR | 范围 | 状态 |
|----|------|------|
| [#30](https://github.com/Linnanli/xClaw/pull/30) W3.3-1 | `ResourceLimits` struct + `apply_in_pre_exec` Unix helper | merged |
| [#32](https://github.com/Linnanli/xClaw/pull/32) W3.3-2 | Seatbelt + seccomp 后端注入 pre_exec rlimit；`set_one` 改为 "shrink soft only"；ADR-45 macOS 调研补 ADR-46 | open（review 中） |
| 本 PR W3.3-3 | Linux cgroup v2 模块 + macOS memorystatus_control SPI + execute spawn/wait 重构 + 完工报告 | open（本 PR） |

## 关键技术决策

### 1. cgroup v2 自动检测，不强求

`OsExecutor` 启动时探测 `/sys/fs/cgroup/cgroup.controllers`，若 memory 控制器存在且 `/sys/fs/cgroup/dasclaw/` 可创建/已存在，则启用 cgroup v2；否则降级为 rlimit-only。失败原因写入 `DetectionResult::Unavailable(reason)` 用于诊断。

不写 `cpu.max`：`max_cpu_secs` 是 RLIMIT_CPU 的"总 CPU 时间"语义，与 cgroup `cpu.max` 的"每周期配额"语义不等价；CPU 限额由 rlimit 层独占，cgroup 只补 RLIMIT_AS 的 RSS-based 短板。

### 2. macOS 走 `memorystatus_control` SPI

详细方案对比见 [46-macos-memory-limit-research.md](./46-macos-memory-limit-research.md)：`setrlimit(RLIMIT_AS/RLIMIT_DATA)` 在 Darwin 返回 EINVAL，私有 Mach SPI `task_set_phys_footprint_limit` 走同条 ledger 路径无功能优势，VM 方案（Virtualization.framework / Lima / OrbStack）成本不匹配。最终选 `memorystatus_control(MEMORYSTATUS_CMD_SET_MEMLIMIT_PROPERTIES, ...)`：公开 SDK 头、无 entitlement、kernel 强制 SIGKILL。

### 3. execute 路径从 `output()` 改为 `spawn → 加固 → wait_with_output`

cgroup v2 必须知道 child pid 才能写 `cgroup.procs`；macOS memorystatus_control 必须知道 child pid 才能限制目标进程。两者都要求父进程在 spawn 之后、子进程开始消耗资源之前注入加固。`Command::output()` 把 spawn + wait 合并，无法插入这一步——所以两个后端的 execute 都改为：

```rust
cmd.stdout(piped()).stderr(piped());
let child = cmd.spawn()?;
// 此时 child.id() 可用：
//   Linux: cgroup.assign_pid(child.id())
//   macOS: memorystatus::set_memory_limit(child.id() as i32, bytes)
let output = child.wait_with_output()?;
```

### 4. 加固层失败不中止 execute

cgroup v2 写 `cgroup.procs` 失败、或 `memorystatus_control` 返回错误，**不**让整个 execute 失败：rlimit 层（CPU/FD/NPROC，Linux 上还含 RLIMIT_AS）在 pre_exec 已经生效，是兜底；kernel-native 层是增强项，失败时降级到"双层只剩一层"，调用方仍然得到一个被限制的子进程。这是 ADR-45 "fail-safe but not fail-closed" 的细化。

后续接入 `tracing` 后会在加固失败路径加 `warn!` 日志。

## 文件改动统计

| 文件 | 变更 | LOC |
|------|------|-----|
| `crates/dasclaw_sandbox/src/lib.rs` | W3.3-1：`ResourceLimits` struct + `with_resource_limits` builder | +75 |
| `crates/dasclaw_sandbox/src/rlimit.rs` | W3.3-1：`apply_in_pre_exec` Unix helper；W3.3-2：`set_one` 改为 shrink-soft-only | +130 |
| `crates/dasclaw_sandbox/src/macos/mod.rs` | W3.3-2：pre_exec 注入；W3.3-3b：spawn + memorystatus_control | +30 |
| `crates/dasclaw_sandbox/src/macos/memorystatus.rs` | W3.3-3b：`memorystatus_control` FFI + `set_memory_limit` | +130（新文件） |
| `crates/dasclaw_sandbox/src/linux/mod.rs` | W3.3-2：pre_exec 注入；W3.3-3a：spawn + cgroup assign_pid | +35 |
| `crates/dasclaw_sandbox/src/linux/cgroup_v2.rs` | W3.3-3a：`detect()` + `CgroupGuard` RAII | +200（新文件） |
| `docs/plans/architecture-refactor/45-resource-limits-adr.md` | ADR Round 20 ACCEPTED | +280（新文件） |
| `docs/plans/architecture-refactor/46-macos-memory-limit-research.md` | macOS 内存方案对比（Round 21） | +100（新文件） |
| 本完工报告 | | +70（新文件） |
| **合计** | | **~1050 LOC** |

ADR-45 §LOC 估算 950 LOC，实际 ~1050 LOC，偏差 +10%；偏差来自 macOS 调研路径意外发现的 setrlimit EINVAL 触发 ADR-46 调研笔记 + memorystatus FFI 模块。

## 测试

| 维度 | 计数 | 备注 |
|------|------|------|
| 单元测试（rlimit） | 8 | 含 unlimited、default、clamp、higher-than-current 等失败/边界 |
| 单元测试（cgroup_v2） | 4 | unique_suffix、OnceLock 一致性；真实 mkdir/write 路径需 root，由集成场景验证 |
| 单元测试（memorystatus） | 3 | 自身设大限额、非法 pid、小字节 clamp；避免对其他进程干扰 |
| Seatbelt 集成测试 | 3 | `seatbelt_propagates_resource_limits_to_child`、`seatbelt_scrubs_parent_env`、`seatbelt_runs_echo_under_real_sandbox`，全部走真实 `/usr/bin/sandbox-exec` |
| 全 crate | **34 / 34 通过**，0 编译警告 | macOS 本地 |

未在自动化测试覆盖的真实路径：
- Linux cgroup v2 真实 `memory.max` 触发 SIGKILL 场景——需要在带 cgroup v2 写权限的 Linux CI runner 上跑，工程上是 W4 集成测试基础设施的事。
- macOS `memorystatus_control` 真实 OOM kill——同上，需要专门的 OOM 测试 fixture。

## 后续 follow-up

| 项 | 描述 | 拟落地 Wave |
|---|------|-----------|
| `tracing` 接入 | 加固层失败、cgroup 探测结果都改成 `warn!` / `info!` 而非默认无日志 | W4 observability |
| `disable_cgroup_v2` 配置开关 | ADR-45 提到的 escape hatch，现在没接入 config；用户可通过 `ResourceLimits::unlimited()` 间接绕过 | W4 config 整合时一起做 |
| Windows Job Objects | `JOB_OBJECT_LIMIT_PROCESS_MEMORY` + `JOB_OBJECT_LIMIT_ACTIVE_PROCESS` | W5 Windows backend |
| cgroup v2 真实 OOM 集成测试 | 在 docker / podman 内启 cgroup namespace 跑限额触发 | W4 |
| macOS memorystatus 真实 OOM 集成测试 | spawn 一个故意 alloc 超限的子进程，断言 SIGKILL + reason=7 | W4 |

## 参考

- [ADR-45 — Resource Limits ADR](./45-resource-limits-adr.md)
- [46 — macOS memory limit research](./46-macos-memory-limit-research.md)
- [PR #30](https://github.com/Linnanli/xClaw/pull/30) / [PR #32](https://github.com/Linnanli/xClaw/pull/32)
- Linux: [cgroup v2 controllers](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html)
- Darwin: [`bsd/sys/kern_memorystatus.h`](https://github.com/apple/darwin-xnu/blob/main/bsd/sys/kern_memorystatus.h)
