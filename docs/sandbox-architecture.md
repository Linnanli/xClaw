# IronClaw 沙箱架构技术选型文档

## 背景

IronClaw 桌面客户端支持 Skill（技能）和 Extension（插件）两种扩展机制。其中：

- **Skill**：SKILL.md 格式，提示词 + 可选的 `scripts/` 目录（bash/python 脚本）
- **Extension**：MCP Server 协议，HTTP 模式或 Stdio 模式（本地进程）

当 agent 执行 skill 脚本或激活 Stdio 模式插件时，需要沙箱隔离，防止：
1. 脚本读取用户敏感文件（API Key、私钥、企业数据）
2. 脚本访问非授权网络端点
3. 恶意脚本破坏宿主机环境

本文档梳理各沙箱方案的技术选型，供后续改造参考。

---

## 现状

ironclaw 目前有两套沙箱机制：

### 1. Docker 沙箱（服务端 Job）

位置：`ironclaw/src/sandbox/`

用于服务端的 Job 执行（`ContainerJobManager`），通过 `bollard` crate 操作 Docker API。

架构：
```
agent 发起 Job
  → ContainerJobManager 创建 Docker 容器
  → 容器内命令通过 HTTP 代理访问网络
  → 代理检查域名白名单 + 注入凭据
  → 容器销毁
```

默认网络白名单（`sandbox/config.rs`）：
- 包管理：crates.io、npmjs.org、pypi.org、proxy.golang.org
- 版本控制：github.com、api.github.com
- LLM API：api.openai.com、api.anthropic.com、api.near.ai

**问题**：Docker 需要用户安装，macOS/Windows 上需要 VM（Docker Desktop/Podman Desktop），不适合桌面客户端场景。

### 2. WASM 沙箱（内置工具）

位置：`ironclaw/src/tools/wasm/`

用于内置集成工具（Gmail、Slack、Google Slides），通过 Wasmtime 运行 `.wasm` 文件。

特点：
- 零依赖，内嵌在 ironclaw 里
- 网络访问通过 host function 白名单控制
- WASM 代码物理上无法读取宿主机凭据
- 启动延迟 < 10ms

**问题**：需要工具作者将代码编译为 WASM，对 Python/Node 脚本不友好。

---

## 各方案对比

### 方案一：Docker / Podman（容器）

**原理**：Linux namespace + cgroup 隔离，每个任务运行在独立容器中。

| 维度 | 评估 |
|------|------|
| 隔离级别 | 容器级（共享宿主机内核） |
| 启动延迟 | 1–3 秒（冷启动） |
| 运行时开销 | 5–15% |
| 桌面可用性 | ❌ macOS/Windows 需要 VM |
| 企业 IT 兼容性 | ⚠️ 经常被 IT 策略禁用 |
| 网络控制 | ✅ HTTP 代理白名单（ironclaw 已实现） |
| 凭据保护 | ✅ 代理边界注入，容器内不可见 |

**优势**：
- ironclaw 已有完整实现，包括网络代理和凭据注入
- 支持任意语言（Python/Node/Go 等）
- 镜像可预置依赖，环境一致性好

**劣势**：
- 桌面客户端场景不可靠（Docker 不一定可用）
- 冷启动慢，对话中频繁调用体验差
- 镜像管理复杂

**适用场景**：服务端 Job 执行（现有用途），不适合桌面客户端 skill 脚本。

**Podman 说明**：Podman 在 Linux 服务端有优势（无 daemon、rootless），但 macOS/Windows 上同样需要 VM，桌面场景问题相同。ironclaw 的 `connect_docker()` 已支持 Podman socket（`/var/run/docker.sock` 兼容路径），无需代码改动。

---

### 方案二：WASM（Wasmtime）

**原理**：WebAssembly 运行时，在用户态实现内存隔离和能力控制。

| 维度 | 评估 |
|------|------|
| 隔离级别 | 沙箱级（用户态隔离） |
| 启动延迟 | < 10ms |
| 运行时开销 | < 5% |
| 桌面可用性 | ✅ 零依赖，跨平台 |
| 企业 IT 兼容性 | ✅ 无需安装任何东西 |
| 网络控制 | ✅ host function 白名单 |
| 凭据保护 | ✅ 物理隔离，WASM 代码无法读取 |

**优势**：
- ironclaw 已有完整实现（Gmail/Slack 工具）
- 零依赖，内嵌运行时
- 启动极快，适合对话中频繁调用
- 安全性高：无文件系统访问、网络必须经过 host function

**劣势**：
- 需要工具作者将代码编译为 WASM（Python/Node 脚本不能直接用）
- WASM 生态相对 Docker 小，部分库不支持
- 调试困难

**适用场景**：内置集成工具（Gmail/Slack 等），需要高安全性和零依赖的场景。

**WASM 编译工具链**：
- Rust → `cargo build --target wasm32-wasip2`
- Python → `py2wasm`（Wasmer 出品）或 `componentize-py`
- Node.js → `jco` (JavaScript Component Model)
- Go → `tinygo`

---

### 方案三：OS 原生沙箱

不需要容器或 VM，直接使用操作系统内核提供的进程隔离原语。

#### 3a. macOS — Seatbelt（sandbox-exec）

**原理**：Apple 的 `sandbox-exec` 工具，通过策略文件限制进程的文件系统访问和 syscall。

| 维度 | 评估 |
|------|------|
| 隔离级别 | syscall 级 |
| 启动延迟 | ~1ms |
| 运行时开销 | 1–5% |
| 依赖 | macOS 内置，零安装 |
| 网络控制 | ⚠️ 需要配合 pf 防火墙规则 |

**已知漏洞**：shell built-in 命令（`cd`、`export`、`source`）不走 `execve`，Seatbelt 拦截不到，导致 CVE-2026-22708（Cursor 沙箱绕过）。

**加固方案**：
1. 禁止 `sh`/`bash`/`zsh` 作为入口，只允许直接执行二进制
2. 结合命令白名单，只允许 skill `requires.bins` 声明的二进制
3. 参考 Cursor 开源的策略模板

**Rust 接入**：直接通过 `std::process::Command` 调用 `sandbox-exec -f profile.sb`，无需额外 crate。

#### 3b. Linux — Landlock + seccomp

**原理**：
- **Landlock**（Linux 5.13+）：进程调用 `landlock_restrict_self()` 限制自身文件系统访问，基于 eBPF 思路，VFS 层检查
- **seccomp-BPF**：过滤危险 syscall（`execve` 白名单、`ptrace` 禁止等）

| 维度 | 评估 |
|------|------|
| 隔离级别 | 文件系统 + syscall 双层 |
| 启动延迟 | < 1ms |
| 运行时开销 | < 2% |
| 依赖 | Linux 内核 5.13+，零安装 |
| 网络控制 | ⚠️ 需要配合 iptables/nftables |

**优势**：两层叠加，Landlock 防文件越权，seccomp 防危险 syscall，互补。

**Rust crate**：
- `landlock`（官方绑定，Red Hat 维护）：https://crates.io/crates/landlock
- `seccompiler`（Firecracker 团队出品）：https://crates.io/crates/seccompiler

#### 3c. Windows — Job Object + Windows Filtering Platform

**原理**：
- **Job Object**：Windows 内核对象，限制进程组的资源使用和权限
- **Windows Filtering Platform (WFP)**：内核级网络过滤，Chrome/Edge 用于网络沙箱

| 维度 | 评估 |
|------|------|
| 隔离级别 | 进程权限 + 网络过滤 |
| 启动延迟 | < 1ms |
| 运行时开销 | 几乎零 |
| 依赖 | Windows 内置，零安装 |
| 网络控制 | ✅ WFP 可精确控制出站连接 |

**Rust crate**：`windows`（微软官方）：https://crates.io/crates/windows

---

### 方案四：microVM（Firecracker）

**原理**：AWS 开源的轻量虚拟机，基于 KVM，每个任务独立内核，VM 级隔离。

| 维度 | 评估 |
|------|------|
| 隔离级别 | VM 级（独立内核） |
| 启动延迟 | ~125ms |
| 运行时开销 | < 5% |
| 依赖 | Linux + KVM，服务端专用 |
| 桌面可用性 | ❌ 需要 KVM，不适合桌面 |

**适用场景**：服务端 Job 执行的升级方案，比 Docker 安全性高，启动比 Docker 快 10 倍。可作为 ironclaw 服务端沙箱的长期演进方向。

---

## 业界开源方案

### 跨平台

| 项目 | 语言 | 说明 |
|------|------|------|
| `sandbox-runtime` | Rust | OS 级沙箱封装，支持文件系统和网络限制，自动选择平台原语 |
| `wasmtime` | Rust | WASM 运行时，ironclaw 已使用 |
| `bubblewrap` | C | GNOME 项目，Linux 容器沙箱，Flatpak 底层 |

### Linux 专用

| 项目 | 语言 | 说明 |
|------|------|------|
| `syd` | Rust | 用户态应用内核，组合 Landlock + Namespaces + Seccomp-BPF，无需 root |
| `firejail` | C | 成熟的 Linux 沙箱工具，但需要 SETUID |
| `minijail` | C | Google 出品，Chrome OS 使用，seccomp + namespace |
| `gVisor` | Go | Google 出品，用户态 Linux 内核，比容器更安全 |

### macOS 专用

| 项目 | 说明 |
|------|------|
| Cursor 沙箱策略模板 | Cursor 开源了 Seatbelt 策略文件，可直接参考 |

### 服务端 / 高安全

| 项目 | 语言 | 说明 |
|------|------|------|
| `firecracker` | Rust | AWS 开源 microVM，Lambda 底层 |
| `kata-containers` | Go/Rust | 容器 + VM 结合，OCI 兼容 |

---

## 推荐技术选型

### 桌面客户端（Desktop Client）

**Skill 脚本执行**：

```
macOS  → Seatbelt (sandbox-exec) + 命令白名单
Linux  → Landlock (landlock crate) + seccomp (seccompiler crate)
Windows → Job Object (windows crate) + WFP 网络过滤
```

或者评估 `sandbox-runtime` crate 的跨平台抽象是否满足需求，可省去分平台实现。

**降级策略**：
- 沙箱不可用时（极少情况），拒绝执行脚本，提示用户
- 不允许静默降级为无沙箱执行

**WASM 工具**：继续使用现有 Wasmtime 方案，适合内置集成工具。

### 服务端（Admin Backend / ironclaw 服务端）

**短期**：继续使用现有 Docker 沙箱（`ContainerJobManager`），已有完整的网络代理和凭据注入。

**长期**：评估迁移到 Firecracker microVM，提升安全性（VM 级隔离）和启动速度（125ms vs 1-3s）。

---

## 加固方案

### 防 shell built-in 绕过（Cursor CVE-2026-22708 教训）

**问题**：`sh -c "command"` 执行时，shell built-in（`cd`、`export`、`source`、`eval` 等）不走 `execve`，OS 原生沙箱拦截不到。

**加固措施**：
1. **禁止 shell 解释器作为入口**：不允许 `sh`/`bash`/`zsh`/`python`/`node` 直接执行任意字符串，只允许执行预定义的二进制文件
2. **命令白名单**：skill 的 `requires.bins` 声明哪些二进制可以运行，沙箱只允许这些二进制的 `execve`
3. **seccomp execve 白名单**（Linux）：通过 seccomp-BPF 限制 `execve` 只能调用白名单内的路径

### 网络访问控制

**问题**：OS 原生沙箱本身不控制网络，需要额外机制。

**方案**：
- **Linux**：iptables/nftables 规则，或 seccomp 过滤 `connect` syscall
- **macOS**：pf 防火墙规则，或 Network Extension（需要签名）
- **通用**：在 skill 的 `metadata.openclaw` 中新增 `network_allowlist` 字段，Admin 审核时验证，运行时通过代理强制执行

### 凭据保护

**问题**：skill 脚本可能通过环境变量读取 API Key。

**加固措施**：
1. 执行 skill 脚本前，清空所有敏感环境变量（参考 ironclaw 现有的 `SAFE_ENV_VARS` 白名单机制）
2. 需要凭据的操作通过 WASM host function 或代理边界注入，不暴露给脚本进程

### 审核流程（Admin 端）

沙箱是最后一道防线，审核是第一道防线：

1. Admin 上传 skill 包时，扫描 `scripts/` 目录中的脚本内容
2. 检测危险模式：`curl | bash`、`wget -O- | sh`、base64 解码执行等
3. 验证 `requires.bins` 声明的二进制是否在已知安全列表内
4. 标注"需要网络访问"的 skill，要求管理员明确审批网络白名单

---

## 实施路线图

### Phase 1（当前）
- 服务端：Docker 沙箱（已实现）
- 桌面客户端：WASM 工具（已实现）
- Admin：skill 上传格式校验 + 安全扫描（待实现，见需求 14）

### Phase 2（桌面客户端沙箱）
- 实现跨平台 OS 原生沙箱：Seatbelt / Landlock+seccomp / Job Object
- 评估 `sandbox-runtime` crate 的可用性
- 实现命令白名单和环境变量清理

### Phase 3（服务端升级）
- 评估 Firecracker microVM 替换 Docker
- 实现 skill 级别的网络白名单声明和运行时强制

---

## 业界案例深度分析

### Codex CLI（OpenAI）

**开源地址**：https://github.com/openai/codex

**沙箱架构**：内核层强制隔离（Kernel-level enforcement）

| 平台 | 技术 |
|------|------|
| macOS | Seatbelt（sandbox-exec） |
| Linux | Landlock + seccomp |
| Windows | 暂不支持原生沙箱（仅 WSL2） |

**核心设计理念**：安全边界在 OS 内核层强制执行，agent 无法绕过，因为 OS 在 syscall 层面直接拒绝。

**配置方式**：TOML 配置文件，通过 `--profile` 切换预设：
```toml
[sandbox]
network_access = false          # 完全禁止网络
filesystem_write = "workspace"  # 只允许写当前工作目录
```

**网络控制**：
- 默认禁止所有网络访问
- `network_access = true` 开启后仍可配置域名白名单
- 已知问题：DNS/SSH socket 也被阻断，导致 `git push` 失败（GitHub Issue #12867）

**优势**：
- 内核层强制，agent 物理上无法绕过
- 适合审查不可信代码的场景（恶意代码无法逃逸）
- 配置简单，profile 切换明确可审计

**劣势**：
- 粒度粗，无法做细粒度的业务规则校验
- Windows 支持弱（依赖 WSL2）
- 网络控制过于激进，容易误伤正常操作（git push 等）

---

### Claude Code（Anthropic）

**开源沙箱代码**：Anthropic 已开源沙箱实现

**沙箱架构**：双层隔离 = OS 原生沙箱（内核层）+ 网络代理（应用层）

| 平台 | 文件系统隔离 | 网络隔离 |
|------|------------|---------|
| macOS | Seatbelt | Unix domain socket → 代理 |
| Linux | **bubblewrap** | Unix domain socket → 代理 |
| WSL2 | bubblewrap | Unix domain socket → 代理 |
| WSL1 | ❌ 不支持 | — |

**关键设计**：网络隔离不是直接用 OS 防火墙，而是通过 **Unix domain socket 代理**：

```
沙箱内进程
  → 所有网络请求必须经过 Unix socket
  → 代理运行在沙箱外部
  → 代理检查域名白名单
  → 新域名触发用户确认弹窗
  → 可配置 allowManagedDomainsOnly 完全禁止未知域名
```

这个设计和 ironclaw 现有的 Docker 网络代理思路完全一致，只是把 Docker 换成了 OS 原生沙箱。

**文件系统隔离默认行为**：
- 写权限：仅限当前工作目录及子目录
- 读权限：整个文件系统（除特定黑名单目录）
- 可通过 `sandbox.filesystem.allowWrite` 扩展写权限路径

**bubblewrap 说明**：
- Linux 上 Claude Code 用的是 `bubblewrap`（不是 Landlock）
- bubblewrap 是 GNOME/Flatpak 的沙箱工具，基于 Linux namespace，不需要 root
- 比 Landlock 更成熟，但需要用户安装（`apt install bubblewrap socat`）
- 如果 bubblewrap 不可用，默认降级为无沙箱运行（可通过 `sandbox.failIfUnavailable = true` 改为硬失败）

**应用层 Hook 系统**（与沙箱互补）：
Claude Code 还有 17 个生命周期 Hook 点（PreToolUse、PostToolUse 等），可以在应用层做任意业务规则校验。这和 IronClaw 的 Kiro Hook 系统非常相似。

**优势**：
- 双层防御：OS 沙箱 + 网络代理，互补
- 网络代理设计优雅，可做细粒度域名控制
- 沙箱失败时有明确的降级策略
- 已开源，可直接参考实现

**劣势**：
- Linux 上依赖 bubblewrap 安装（非零依赖）
- WSL1 不支持
- 应用层 Hook 和 agent 共享进程边界，理论上可被绕过

---

### 两者对比总结

| 维度 | Codex CLI | Claude Code |
|------|-----------|-------------|
| 安全边界位置 | 内核层（syscall） | 内核层 + 应用层双重 |
| macOS 技术 | Seatbelt | Seatbelt |
| Linux 技术 | Landlock + seccomp | bubblewrap |
| 网络控制 | 直接禁止/允许 | Unix socket 代理（更灵活） |
| 配置粒度 | 粗粒度（profile） | 细粒度（路径/域名级别） |
| 降级策略 | 无明确说明 | 可配置硬失败或软降级 |
| 开源 | ✅ | ✅（沙箱部分） |
| Windows | WSL2 only | WSL2 only |
| 适合场景 | 不可信代码审查 | 日常开发 + 企业治理 |

---

### 对 IronClaw 的启示

**Claude Code 的网络代理模式值得直接借鉴**：

ironclaw 现有的 Docker 沙箱已经实现了网络代理（`sandbox/proxy/`），思路和 Claude Code 完全一致。迁移到 OS 原生沙箱时，只需要把 Docker 容器替换为 Seatbelt/bubblewrap，网络代理层可以直接复用。

**bubblewrap vs Landlock 的选择**：

Claude Code 在 Linux 上选择了 bubblewrap 而不是 Landlock，原因可能是：
- bubblewrap 更成熟，社区更大
- bubblewrap 基于 namespace，隔离更完整（进程、挂载点、网络命名空间）
- Landlock 只做文件系统访问控制，需要配合 seccomp 才能覆盖网络

对于 IronClaw，建议 Linux 上也优先考虑 bubblewrap，而不是自己组合 Landlock + seccomp。

**`sandbox.failIfUnavailable` 模式**：

Claude Code 的这个设计对政企场景很重要——管理员可以强制要求沙箱可用，不允许降级。IronClaw 的 Admin 配置下发中可以加入类似的 `require_sandbox: true` 字段。

---

## 参考资料

- [Cursor 沙箱实现博客](https://cursor.com/en-US/blog/agent-sandboxing)
- [CVE-2026-22708：Cursor 沙箱绕过分析](https://danusminimus.github.io/posts/The-Agent-Security-Paradox-When-Trusted-Commands-In-Cursor-Become-Attack-Vectors/)
- [Claude Code 沙箱技术博客](https://www.anthropic.com/engineering/claude-code-sandboxing)
- [Claude Code 沙箱文档](https://code.claude.com/docs/en/sandboxing)
- [Codex CLI GitHub](https://github.com/openai/codex)
- [Codex vs Claude Code 架构对比](https://blakecrosley.com/blog/codex-vs-claude-code-2026)
- [Landlock 官方文档](https://landlock.io/)
- [bubblewrap GitHub](https://github.com/containers/bubblewrap)
- [Firecracker 设计文档](https://github.com/firecracker-microvm/firecracker/blob/main/docs/design.md)
- [ironclaw sandbox 实现](ironclaw/src/sandbox/)
- [ironclaw WASM tool 实现](ironclaw/src/tools/wasm/)
