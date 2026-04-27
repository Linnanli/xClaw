# 42 · 客户端 Job 运行时路线方案记录

**状态**：📋 决策搁置（Decision deferred） · 仅记录候选方案，不立即执行
**日期**：2026-04-08（W3.1c 合并后）
**前置文档**：
- [41-docker-vs-os-sandbox-capability-comparison.md](41-docker-vs-os-sandbox-capability-comparison.md)（codex 进程沙箱 vs Docker 容器能力对比）
- [orchestrator/mod.rs:108](../../../desktop-client/ironclaw/src/orchestrator/mod.rs#L108)（当前 Job 路径硬依赖 Docker 的代码证据）

## 0 · 背景

W3.1c 已删除 Docker 工具执行沙箱层（合并于 PR #15），命令路径改用 [OsExecutor](../../../desktop-client/ironclaw/src/sandbox/os_executor.rs)（codex `dasclaw_exec` 进程级沙箱）。

但 **后台 Job 容器层完整保留**（[orchestrator/{job_manager,reaper}.rs](../../../desktop-client/ironclaw/src/orchestrator/job_manager.rs)），因为：

- ✅ 进程沙箱**不能**替代 Job（缺资源限额、跨重启不可见、无镜像隔离 — 详见 41 文档）
- ⚠️ Job 路径依然硬依赖 Docker / OrbStack / Colima / Rancher 任一 socket
- ❓ Docker-less 客户端 → Job 功能直接禁用（[orchestrator/mod.rs:108](../../../desktop-client/ironclaw/src/orchestrator/mod.rs#L108) `if docker_status.is_ok() { ... } else { (None, None) }`）

本文记录已讨论过的所有候选路线，**避免决策时重复脑暴**。

---

## 1 · 行业现状速查

| 产品 | 桌面端 Job 实现 | 隔离方案 |
|---|---|---|
| Cursor Background Agent | 远端云 VM | 云端 microVM (Firecracker) |
| Devin / Sweep / GitHub Copilot Coding Agent | 云端 | 云端 microVM |
| Replit Agent / Bolt.new / v0 | 云端 / WebContainer | Firecracker / WASM |
| Claude Code CLI | 本地子进程 | 无强隔离 |
| Aider / Cline / Continue / Cody | 本地子进程 | 无强隔离 |
| Codex CLI | 本地子进程 + 单次 sandbox | Seatbelt / Landlock / bubblewrap |
| **ironclaw（当前）** | **本地 Docker 容器** | **bollard + memory/CPU limits** |

**观察**：ironclaw 走"本地容器化 Job"是少数派路线。主流要么云端要么放弃强隔离。

---

## 2 · 候选路线 A：本地容器运行时矩阵

### 2.1 完整矩阵

| 方案 | 平台 | 启动延迟 | 资源限额 | Daemonless | 集成成本 | bollard 兼容 |
|---|---|---|---|---|---|---|
| Docker Desktop | mac+linux+win | 2–10s | ✅ | ❌ | 已就绪 | ✅ |
| OrbStack | macOS | 1–3s | ✅ | 半 | **已自动探测** | ✅ |
| Colima | macOS | 5s+ | ✅ | 半 | **已自动探测** | ✅ |
| Rancher Desktop | mac+linux+win | 5s+ | ✅ | 半 | **已自动探测** | ✅ |
| Podman rootless | Linux 强 / mac 中 | 2–5s | ✅ | ✅ | 加 1 行 socket 路径 | ✅ |
| Apple `container` | macOS 26+ Apple Silicon | 1–3s | ✅ | ✅ | 中（CLI 或 Swift FFI） | ❌ |
| Lima | macOS | 首启 30s+ | ✅ | ✅ | 中（CLI） | 间接 |
| systemd-nspawn + systemd-run | Linux | <1s | ✅ cgroups v2 | ✅ | 中 | ❌ |
| youki / crun | Linux | <1s | ✅ | ✅ | 高（自管 cgroups+namespaces） | ❌ |
| bubblewrap (`bwrap`) | Linux | <0.5s | ❌ 仅 namespaces | ✅ | 低（exec） | ❌ |
| sandbox-exec | macOS | <0.1s | ❌ 无 mem/cpu | ✅ | codex 已用 | ❌ |
| Apple Virtualization.framework | macOS 11+ | 3–10s | ✅ | ✅ | 高（Swift） | ❌ |
| WSL2 + Docker Engine | Windows | 3–10s | ✅ | ✅（Linux 内 daemon） | 中 | ✅ |
| Windows Job Object | Windows | <0.1s | ✅ mem/cpu/io | ✅ | codex 已用 | ❌ |

### 2.2 跨平台真相

**macOS**：XNU 内核没 namespaces，**任何"容器"都必须 VM**。差别只是 VM 多轻。OrbStack 是当前最优；Apple `container` 是中期方向但要 macOS 26+M 芯片。

**Windows**：所有 Linux 容器**必经 WSL2**。原生 Windows 容器只能跑 Windows 镜像（与 ironclaw 无关）。

**Linux**：唯一真正"轻量化收益"的平台。Podman rootless 干掉 daemon，bubblewrap/youki/crun 提供更细粒度选择。

### 2.3 「客户端自带轻量 Docker」可行性

**直接结论：物理上不可行**。`dockerd` 依赖 Linux 内核 namespaces/cgroups/overlayfs/iptables，这些在 Windows/macOS 内核完全不存在。任何 Win/Mac 上的"Docker"都必须有 Linux VM。

**真实可行的"客户端自带容器运行时"方案**：

| 方案 | 平台 | 安装包增量 | 用户体验 |
|---|---|---|---|
| A. 内嵌静态 Podman+crun | Linux | ~50 MB | 开箱即用，rootless |
| B. 内嵌 Firecracker/krun + 极简 rootfs | Linux+macOS | ~150 MB | 真正"自带容器" |
| C. 内嵌 WSL2 自定义 distro（OrbStack 路线） | Windows | ~80 MB | 装 ironclaw 等于装好 dockerd，**离线** |
| D. 客户端命令本地 + Job 上云 | 跨平台 | 0（甚至变小） | Cursor 路线 |

详见 §3 三个切分粒度。

---

## 3 · 候选路线 B：Job 上云（三种切分粒度）

```
┌──────────────────────────────────────────────────┐
│ A. UI / Tauri 前端          ← 永远本地           │
│ B. 文件系统访问（工作目录）  ← 永远本地           │
│ C. LLM API 调用             ← 已经"云"           │
│ D. Agent loop（决策、调度） ← 可本地可云端        │
│ E. 命令工具（短任务）        ← 推荐本地（codex）  │
│ F. Job 容器（长任务）        ← 上云的核心目标     │
│ G. Memory / 历史 / 配置     ← 可本地可云端        │
└──────────────────────────────────────────────────┘
```

### 3.1 方案 1：最小上云 — 只搬 F（容器执行层）⭐⭐⭐⭐⭐

```
本地: A/B/D/E/G  ←→  云端: F
```

- ✅ 客户端摆脱 Docker 依赖
- ✅ Agent 决策延迟低（不走网络）
- ✅ 离线时命令仍能用，Job 不可用
- ✅ 代码不出域（仅临时文件 sync 到云容器）
- ⚠️ 需做文件 sync 协议（类 mutagen / syncthing）
- ⚠️ 网络抖动时 Job 失败重试逻辑

### 3.2 方案 2：中等上云 — 搬 D + F（Agent + 容器）⭐⭐⭐

```
本地: A/B/E/G  ←→  云端: C/D/F
```

- ✅ 客户端真正变薄（< 50 MB）
- ✅ 多设备同步天然
- ⚠️ 每 tool 调用 +100ms RPC 延迟
- ⚠️ 客户端必须实现 "Reverse Tool Server"
- ⚠️ 离线完全不可用

### 3.3 方案 3：完全上云 — 搬 B/D/F（含工作目录）⭐

```
本地: A/E/G  ←→  云端: B'/C/D/F
```

Cursor / Devin / Replit 路线。

- ✅ 客户端极薄
- ✅ 性能最好
- ⚠️ **代码出域**（B 端拒绝率高）
- ⚠️ 离线完全不可用
- ⚠️ 产品定位变成"在线 IDE"

### 3.4 决策矩阵

| 维度 | 方案 1（最小） | 方案 2（中等） | 方案 3（完全） |
|---|---|---|---|
| 客户端组件 | 几乎全保留 | UI + 命令 + 文件代理 | UI + 文件代理 |
| 决策延迟 | 不变 | +100ms/tool | +100ms/tool |
| 离线能力 | 命令可用 | 完全不可用 | 完全不可用 |
| 代码安全 | 仅临时上传 | 代码本地 | **代码上云** |
| 客户端工作量 | 加 RPC + sync | 大改 | 几乎重写 |
| 服务端工作量 | 容器编排 + 存储 | + Agent runtime | + IDE 后端 |
| 商业模式 | 卖 license + 私有部署 | SaaS + 私有部署 | SaaS |

---

## 4 · 决策前置问题

只有先回答这些产品问题才能选路线：

1. **代码是否可以离开用户机器？**
   - 不能 → 只能方案 1（临时上传也是临时的）
   - 可以 → 方案 2/3 都开放

2. **客户是消费者还是 B 端企业？**
   - B 端（要私有部署、合规） → 方案 1 友好
   - 消费者 SaaS → 方案 2/3 ROI 高

3. **要离线能用吗？**
   - 要 → 方案 1
   - 不要 → 方案 2/3 都行

4. **客户端要做多薄？**
   - 不在意（< 200 MB） → 方案 1
   - 要极薄（< 30 MB） → 方案 2/3

---

## 5 · 演进路径建议（与决策无关、可立即并行的工作）

无论最终选哪条路，以下工作都是无悔投资：

### 5.1 立即可做（小工作量）

- ✅ **Linux Podman socket 探测**（[docker_conn.rs](../../../desktop-client/ironclaw/src/sandbox/docker_conn.rs) 加 1 行 + 1 测试）：让 Linux 用户摆脱 Docker Desktop 商用授权门槛 — **零成本最高 ROI**

### 5.2 中期可做（中等工作量）

- ⚠️ **`JobRuntime` trait 抽象**：把 [ContainerJobManager](../../../desktop-client/ironclaw/src/orchestrator/job_manager.rs) 的 Docker 细节与 JobConfig/JobMode/JobOutput 解耦，方便后续加 RemoteJobRuntime / LocalProcessJobRuntime

- ⚠️ **Windows WSL distro 探测增强**：当前 [docker_conn.rs:31](../../../desktop-client/ironclaw/src/sandbox/docker_conn.rs#L31) 是 `#[cfg(unix)]`，Windows 仅靠 bollard 默认 named pipe 探测，缺 WSL2 socket（`\\wsl$\Ubuntu\var\run\docker.sock`、`tcp://localhost:2375`）路径

### 5.3 长期决策依赖项

- ❓ **方案 1 落地**：`RemoteJobRuntime` impl + RPC 协议（gRPC 或 HTTP+SSE）+ 文件 sync
- ❓ **方案 2/3 落地**：Agent loop 上云、Reverse Tool Server、IDE 后端

---

## 6 · 推荐技术蓝图（如选方案 1）

```rust
trait JobRuntime: Send + Sync {
    async fn start(&self, spec: JobSpec) -> Result<JobHandle>;
    async fn wait(&self, h: &JobHandle) -> Result<JobResult>;
    async fn cancel(&self, h: &JobHandle) -> Result<()>;
    fn watch_events(&self, h: &JobHandle) -> EventStream;
}

struct DockerJobRuntime { docker: Docker, ... }       // 现有 ContainerJobManager 包一层
struct RemoteJobRuntime { client: GrpcClient, ... }   // 远程代理
struct LocalProcessJobRuntime { ... }                 // 兜底（Docker-less 客户端，弱隔离）
```

**演进顺序（如确认走方案 1）**：

1. **W4.1** 抽 `JobRuntime` trait（不破坏现有 Docker 路径，所有现有调用透明迁移）
2. **W4.2** 加 `LocalProcessJobRuntime` 兜底（Docker-less 客户端可用，弱隔离，文档说明限制）
3. **W4.3** 加 `RemoteJobRuntime`（云端方案落地，需配套服务端基建）

---

## 7 · 与现有架构的兼容性

| 当前组件 | 改动需求 | 兼容性影响 |
|---|---|---|
| [docker_conn.rs](../../../desktop-client/ironclaw/src/sandbox/docker_conn.rs) | 加 Podman socket 探测 | 向下兼容（既有 socket 仍探测） |
| [job_manager.rs](../../../desktop-client/ironclaw/src/orchestrator/job_manager.rs) | 包装为 `DockerJobRuntime` | 向下兼容（trait 默认实现转发） |
| [orchestrator/mod.rs](../../../desktop-client/ironclaw/src/orchestrator/mod.rs) | 按 config 选 runtime | 配置加新字段，旧配置仍走 Docker |
| [sandbox/proxy/](../../../desktop-client/ironclaw/src/sandbox/proxy/) | 与本决策无关 | 独立工程，参见 W3.2 |

---

## 8 · 关于「客户端自带 Docker」的最终澄清

**不存在"裁剪过的轻量 Docker"在 Windows/macOS 进程内直接跑**，因为：

```
dockerd
  ↓ 调用
Linux namespaces (CLONE_NEWNS / CLONE_NEWPID / CLONE_NEWNET / ...)
  ↓ 提供方
Linux kernel
  ↓ 不存在于
Windows / macOS
```

任何 Win/Mac 上的"轻量容器方案"本质上都是 **「轻量 Linux VM + VM 内的 dockerd」**：
- Docker Desktop = 自带的 WSL2 / HyperKit Linux VM + dockerd
- OrbStack = 自带的 Apple Virtualization.framework Linux VM + dockerd
- Apple `container` = 自带的 Apple Virtualization.framework microVM + 自家 runtime

差别只是「VM 多轻」「文件共享多优化」「网络多便利」。

**真正的轻量化只在 Linux**（Podman rootless 直接调内核 namespaces，不需要 daemon、不需要 VM）。

---

## 9 · 待办（决策时再回填）

- [ ] 产品决策：方案 1 / 2 / 3（待回答 §4 四个问题）
- [ ] 是否做内嵌运行时（Win WSL distro / Linux 静态 Podman）— 取决于产品对客户端"开箱即用"程度的诉求
- [ ] 是否做 Apple `container` 集成 — 取决于 macOS 26 + Apple Silicon 用户占比是否到达投入门槛

---

## 参考

- [41-docker-vs-os-sandbox-capability-comparison.md](41-docker-vs-os-sandbox-capability-comparison.md)
- [orchestrator/mod.rs](../../../desktop-client/ironclaw/src/orchestrator/mod.rs)
- [orchestrator/job_manager.rs](../../../desktop-client/ironclaw/src/orchestrator/job_manager.rs)
- [docker_conn.rs](../../../desktop-client/ironclaw/src/sandbox/docker_conn.rs)
- Cursor Background Agent docs（云端 Job 行业参考）
- OrbStack architecture（macOS 轻量 VM 行业最佳实践）
- Apple `container` 26.1k stars (https://github.com/apple/container)
- youki CNCF sandbox project (https://github.com/youki-dev/youki)
- bubblewrap user namespaces sandbox (https://github.com/containers/bubblewrap)
