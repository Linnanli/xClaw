# 46 — macOS Process Memory Limit: Alternatives Research

> **Status**: research note — supplements [ADR-45 §macOS](45-resource-limits-adr.md).
> **Date**: Round 21
> **Trigger**: W3.3-2 实测发现 `setrlimit(RLIMIT_AS / RLIMIT_DATA)` 在 Darwin 上返回 EINVAL，需要给 macOS 找替代方案。

## TL;DR

macOS 进程级内存限制的可用方案，按落地难度排序：

| 方案 | 可行性 | 强制力 | 工程成本 |
|------|--------|--------|---------|
| **`memorystatus_control` SPI** | ✅ 公开 SDK 头 | ✅ kernel 强制 kill | **低** — 单 syscall |
| Mach `task_set_phys_footprint_limit` | ⚠️ 私有 SPI | ✅ kernel 强制 | 中 — 需链接私有框架 |
| Virtualization.framework / Apple Containerization | ✅ 公开 | ✅ VM 隔离 | **极高** — 整个 Linux VM |
| Lima / OrbStack / Colima | ✅ 第三方 | ✅ VM 隔离 | 极高 — 用户需安装 |
| 协作式：libdispatch memory pressure | ✅ 公开 | ❌ 仅观测 | 低但**不可强制** |
| `posix_spawn` 属性 | ❌ 无内存限制属性 | — | — |
| `setrlimit(RLIMIT_AS/RLIMIT_DATA)` | ❌ EINVAL | — | — |
| `setrlimit(RLIMIT_RSS)` | ❌ Darwin 不实现 | — | — |
| App Sandbox SBPL `(allow ...)` | ❌ 无内存指令 | — | — |
| `nice` / `setpriority` | ❌ CPU 调度，非内存 | — | — |
| launchd `LimitToHardware` | ❌ 仅 launchd 启动的 daemon | — | — |

**推荐**：W3.3-3 在 Linux 走 cgroup v2，macOS 走 `memorystatus_control`。两者都是 kernel 强制、生产级方案，与代码生成型 agent 沙箱目标匹配。

## 1. `memorystatus_control` — 推荐方案

### 来源与公开性

定义在 `<sys/kern_memorystatus.h>`，随 Xcode Command Line Tools 安装的 macOS SDK 一同发布：

```
/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/include/sys/kern_memorystatus.h
```

[Apple darwin-xnu source](https://github.com/apple/darwin-xnu/blob/main/bsd/sys/kern_memorystatus.h) 确认它在 `#ifndef KERNEL` 下导出给用户态：

```c
int memorystatus_control(uint32_t command, int32_t pid, uint32_t flags,
                         void *buffer, size_t buffersize);
```

虽然 Apple 文档称之为 "private SPI"，但：

- 头文件公开，无 entitlement 即可调用（自身或自己 spawn 的子进程）
- WebKit / Chromium / Safari 渲染进程长期使用
- App Store apps 也使用 `memlimit_active` / `memlimit_inactive`（通过 framework 间接调用）

### API 用法

```c
typedef struct memorystatus_memlimit_properties {
    int32_t  memlimit_active;          // MB, when process is foreground
    uint32_t memlimit_active_attr;     // bit 0 = MEMORYSTATUS_MEMLIMIT_ATTR_FATAL
    int32_t  memlimit_inactive;        // MB, when process is background
    uint32_t memlimit_inactive_attr;
} memorystatus_memlimit_properties_t;

memorystatus_memlimit_properties_t mlp = {
    .memlimit_active   = 4096,                                      // 4 GiB
    .memlimit_active_attr = MEMORYSTATUS_MEMLIMIT_ATTR_FATAL,       // 超限即 kill
    .memlimit_inactive = 4096,
    .memlimit_inactive_attr = MEMORYSTATUS_MEMLIMIT_ATTR_FATAL,
};
memorystatus_control(MEMORYSTATUS_CMD_SET_MEMLIMIT_PROPERTIES,
                     child_pid, 0, &mlp, sizeof(mlp));
```

超限后 `JETSAM_REASON_MEMORY_PERPROCESSLIMIT`（reason=7） SIGKILL 子进程。

### 与 W3.3 集成路径

1. 父进程在 `Command::output()` 执行 `posix_spawn` 之后、子进程开始消耗内存之前调用 `memorystatus_control`。
2. **关键**：必须在父进程调，**不在 pre_exec 调**——理由是：
   - `memorystatus_control` 不在 POSIX async-signal-safe 列表
   - 调用时机要在子进程**已经存在**之后（fork 完成后）
3. Rust 用 `std::process::Command::spawn() → child.id() → memorystatus_control(child_id)` 时序。
4. 不能用 `output()` 的便利封装，需要切到手动 `spawn()` + `wait_with_output()`。

### 限制

- **仅自己 spawn 的子进程或自身**——非 root 不能给任意 pid 设限
- 设置必须在子进程开始执行**之前**完成（competition window）；实战会有微小窗口，子进程极快分配大内存可能逃逸
- macOS 14+ 强化了 entitlement 检查，但对自有子进程仍然放行

## 2. `task_set_phys_footprint_limit` — 备选方案

私有 Mach SPI，签名：

```c
kern_return_t task_set_phys_footprint_limit(
    task_t task, int new_limit_mb, int *old_limit_mb);
```

Chromium 在 macOS 上历史使用过此接口设 renderer 进程内存上限。优点：直接绑定到 Mach task port，不走 jetsam priority-list 协议；缺点：

- 真正的 private SPI（不在公开 SDK 头），需手动声明 extern 链接
- App Store 审核可能拒
- 与 `memorystatus_control` 在底层走同一条 ledger 路径，没有功能优势

**结论**：除非 `memorystatus_control` 不可用，否则不需要这个备选。

## 3. Virtualization.framework / Apple Containerization

macOS 15+ 提供原生 Linux 容器（Apple Containerization 项目）。优点：在 Linux VM 内 cgroup v2 完整可用；缺点：

- 需要 macOS 15.x，老版本回退困难
- 启动一个 VM 仅为限内存——成本严重失衡
- 与 dasclaw_sandbox 当前 Seatbelt 路径（spawn 进程 + sbpl）架构不匹配

**结论**：不在 W3.3 范围内；如果未来要做"完全跨平台容器化"再考虑。

## 4. Lima / OrbStack / Colima

第三方 Linux VM 工具。优点：成熟，容器化能力齐全；缺点：用户需要单独安装。**不在 dasclaw 实现职责内**——可由用户在 config 层选择 "external sandbox" 时通过这些工具落地。

## 5. libdispatch memory pressure（仅观测）

`DISPATCH_SOURCE_TYPE_MEMORYPRESSURE` 让进程**接收**系统内存压力事件（normal/warn/critical），用于自愿降级。**不能强制**。

```c
dispatch_source_t src = dispatch_source_create(
    DISPATCH_SOURCE_TYPE_MEMORYPRESSURE, 0,
    DISPATCH_MEMORYPRESSURE_WARN | DISPATCH_MEMORYPRESSURE_CRITICAL,
    queue);
```

可用于**辅助路径**：父进程订阅压力事件，主动 kill 子进程。但有竞态，不适合作为唯一防线。

## 6. setrlimit 路径（已实测失败）

W3.3-2 实测：

| 资源 | macOS setrlimit 结果 |
|------|---------------------|
| `RLIMIT_CPU` | ✅ 成功，子进程 ulimit -t 生效 |
| `RLIMIT_NOFILE` | ✅ 成功 |
| `RLIMIT_NPROC` | ✅ 成功 |
| `RLIMIT_AS` | ❌ EINVAL |
| `RLIMIT_DATA` | ❌ EINVAL |
| `RLIMIT_RSS` | ❌ 历史 BSD 名义存在，Darwin 不实现 |

实测代码见 `/tmp/rlprobe`（已删除），耗时 ~30 秒。

## 7. 不可行方案归档

- **App Sandbox SBPL**：`sandbox-exec` 的 `.sb` 文件支持 `(allow file-* ...)` `(allow network-* ...)`，**没有 `(memory ...)` 操作符**。
- **`posix_spawn` 属性**：`POSIX_SPAWN_*` 标志覆盖文件描述符、信号、组 ID，没有内存。
- **`launchctl limit memorylimit`**：仅作用于 launchd 启动的 daemon plist，agent 进程内 spawn 的子进程不受影响。
- **`nice` / `setpriority`**：调 CPU 调度优先级，与内存无关。
- **`SIGXCPU`-style soft kill**：仅 CPU 时间，无 memory 对应。

## 决议（建议加入 ADR-45）

1. **W3.3-3 macOS 内存限制走 `memorystatus_control`**——与 Linux cgroup v2 同 Wave 落地。  
2. 实现位置：`crates/dasclaw_sandbox/src/macos/memorystatus.rs`（新文件）。
3. 调用时机：父进程，在 `Command::spawn()` 之后、`child.wait()` 之前。重构 `SeatbeltSandbox::execute` 从 `output()` 切换到 `spawn() + memorystatus_control(child.id()) + wait_with_output()`。
4. 失败模式：如果 `memorystatus_control` 返回非零（旧 macOS、entitlement 缺失），**不阻断执行**——记 warn log，向用户暴露 "memory enforcement unavailable on this platform" 状态字段。这与 cgroup v2 在缺失情况下的降级（escape hatch `disable_cgroup_v2`）对称。
5. 单元测试可在 macOS runner 上跑：spawn 一个 `python3 -c 'b = bytearray(2*1024*1024*1024)'`，断言子进程被 SIGKILL（exit signal == 9）且 `JETSAM_REASON_MEMORY_PERPROCESSLIMIT` 出现在 `/var/log/system.log`（可选）。

## 参考

- [Apple darwin-xnu kern_memorystatus.h](https://github.com/apple/darwin-xnu/blob/main/bsd/sys/kern_memorystatus.h) — SPI 定义来源
- [WebKit Source/WTF/wtf/cocoa/MemoryFootprintCocoa.cpp](https://github.com/WebKit/WebKit) — `memorystatus_control` 真实使用样例
- [Chromium memory_purger_mac.cc](https://source.chromium.org/chromium/chromium/src) — `task_set_phys_footprint_limit` 历史使用
- [Asahi Lina's macOS memory limit notes](https://asahilinux.org/) — 社区分析
- ADR-45 Round 20 — 三项决策原文
