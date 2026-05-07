# ADR-129: sandbox-windows `windows` crate 适配策略

- **Status**: � **Accepted**（2026-05-07 nally 在 mcp-feedback-enhanced 签字）
- **Date**: 2026-05-07
- **Approver**: nally
- **Authors**: GitHub Copilot agent (sandbox-windows port worker)
- **Tracker**: [#263](https://github.com/Linnanli/xClaw/issues/263) — Phase 1.1.4 parent
- **Epic**: [#241](https://github.com/Linnanli/xClaw/issues/241) — fork codex-windows-sandbox
- **Hand-off**: [`49-sandbox-windows-phase-1.1.4i-handoff.md`](49-sandbox-windows-phase-1.1.4i-handoff.md) §2.1 / §3.2
- **Related**:
  - [ADR-121](adr-121-p0a-sandbox-activation-decision.md) — P0-A 沙箱默认激活与 Windows 隔离方案
  - ADR-130（同批起草）— sandbox-windows lib/bin split

## 1. Context

### 1.1 触发条件

Phase 1.1.4i 已完成 30 个 verbatim 文件端口（Wave i-1 ~ i-4）。剩余 3 个文件中：

- **`firewall.rs`（512 LOC）** 直接使用 `windows = 0.58` 重型 COM crate，调用 `INetFwPolicy2 / INetFwRule3 / NetFwRule / CoCreateInstance / CoInitializeEx` 等 Win32 防火墙 API。
- **`elevated_impl.rs`（306 LOC）** 与上游 lib.rs inline `windows_impl { ... }` 子模块也使用 `windows` crate 中部分 COM API（暂未逐项核实，但与 firewall 落在同一依赖体系下）。

而我方 `crates/dasclaw_sandbox_windows/Cargo.toml` 当前**只有** `windows-sys = 0.52`（轻量 sys-style binding），未引入 `windows = 0.58` 重型 COM crate。

### 1.2 上游事实

`codex-cli-main/codex-rs/windows-sandbox-rs/Cargo.toml` 显示上游**同时**依赖：

```toml
# 顶层 [dependencies]（所有平台）
windows = { version = "0.58", features = [
    "Win32_Foundation",
    "Win32_NetworkManagement_WindowsFirewall",
    "Win32_System_Com",
    "Win32_System_Variant",
] }

# [target.'cfg(windows)'.dependencies.windows-sys]
windows-sys = { version = "0.52", features = [ ...28 个 features... ] }
```

两个 crate 在同一二进制内**共存**，分工：

- `windows-sys`：低层 sys binding，零开销 FFI，覆盖 90% 调用（Token / ACL / Job Object / Console / Registry / 文件系统）
- `windows`：高级 COM/RAII binding，仅用于 Firewall（INetFw* COM 接口）+ Variant 等需要类型安全 COM 包装的场景

### 1.3 Verbatim port 红线

`AGENTS.md` 与 ADR-121 明确：sandbox-windows 端口必须 **verbatim**，禁止补丁式改写。任何"改写为 windows-sys 等价实现避免引入 windows crate"的方案都属于补丁式重写，直接违反端口红线。

### 1.4 候选方案

| 选项 | 描述 | 评估 |
|---|---|---|
| **A**（推荐）| 对齐上游：在 `[target.'cfg(windows)'.dependencies]` 下新增 `windows = "0.58"`，feature 列表与上游 1:1，与现有 `windows-sys` 共存 | verbatim 端口零适配代价；编译时间增加约 30–60 s（windows crate 大但 codegen 已优化）；Cargo registry 直接拉取，无 vendoring 阻塞 |
| **B** | 把 `windows` crate 调用改写为 `windows-sys` + 手写 COM vtable | 严重违反 verbatim 红线；维护成本极高（COM RAII 全靠 windows crate）；**否决** |
| **C** | 把 firewall/elevated_impl 缓后到独立可选 feature `firewall-windows-coms` | 制造双轨编译矩阵；CI 需多跑一组；与 ADR-121 "默认激活" 方向不一致 |

## 2. Decision

**采用方案 A：对齐上游，引入 `windows = 0.58` 与 `windows-sys = 0.52` 共存。**

具体做法：

1. 在 `crates/dasclaw_sandbox_windows/Cargo.toml` 的 `[target.'cfg(windows)'.dependencies]` block 下新增：

   ```toml
   windows = { version = "0.58", features = [
     "Win32_Foundation",
     "Win32_NetworkManagement_WindowsFirewall",
     "Win32_System_Com",
     "Win32_System_Variant",
   ] }
   ```

   注意必须 gate 在 `cfg(windows)`，与 `windows-sys` 一致 —— 上游放在顶层是因为有 cross-target build 需要 stub，我方 lib 端不需要。

2. **不**新增 feature flag。`windows` 与 `windows-sys` 都是 always-on（在 windows target 上）。

3. firewall.rs 端口（Wave i-5）：与上游字节级一致，仅替换 crate root（`codex_windows_sandbox::SetupErrorCode` → `dasclaw_sandbox_windows::SetupErrorCode`）。该文件**不**注册到 lib.rs（保持 "bin-only" 定位，由 setup_main_win bin 在 Wave i-7 引入）。

4. elevated_impl.rs / inline `windows_impl` 端口（Wave i-6）：依赖图详见 ADR-130 §2.3 与 hand-off §2.2，本 ADR 仅授权其使用 `windows` crate API。

## 3. Consequences

### 3.1 正向

- 解锁 Wave i-5 / i-6 启动条件
- 与上游 1:1 对齐，未来 rebase 上游修复零摩擦
- 编译期类型安全（COM Interface trait）

### 3.2 负向

- crate 编译耗时 +30~60 s（windows crate 体积约 2 GB 元数据，但 codegen 增量极小）
- Linux/macOS 开发机 cargo metadata 仍会下载 `windows` crate（registry 不区分 target，但 build 不会编译 Win32 .lib）

### 3.3 缓解

- `cargo sweep --time 3` 已是 AGENTS.md 强制项，避免 target/ 膨胀失控
- CI Windows runner 已配 sccache，二次构建命中缓存

## 4. Validation

引入后必跑：

```bash
# 1. crate 单点 check（应保持秒级）
cargo check -p dasclaw_sandbox_windows --tests

# 2. 跨平台冒烟：Linux/macOS 上 windows crate 不应触发 Win32 编译
cargo check -p dasclaw_sandbox_windows --target x86_64-unknown-linux-gnu  # CI only

# 3. clippy 红线
cargo clippy --no-deps -p dasclaw_sandbox_windows --all-targets -- -D warnings
```

CI（GitHub Actions Windows runner）在 Wave i-5 PR 中需绿。

## 5. Non-goals

- 不评估"是否应整库迁移到 `windows = 0.58` 替代 `windows-sys = 0.52`"。本 ADR 只解决"firewall/elevated 必须用 `windows` crate"的局部约束。
- 不涉及 lib/bin split（→ ADR-130）。
- 不涉及 `windows-rs` 与 `winapi = 0.3` 的关系（`winapi` 已在 dasclaw_sandbox_windows 用于 PTY 路径，与本 ADR 正交）。

## 6. References

- 上游 [`codex-cli-main/codex-rs/windows-sandbox-rs/Cargo.toml`](../../../codex-cli-main/codex-rs/windows-sandbox-rs/Cargo.toml)
- 上游 [`firewall.rs`](../../../codex-cli-main/codex-rs/windows-sandbox-rs/src/firewall.rs) `#![cfg(target_os = "windows")]`
- [Hand-off §2.1 Wave i-5 风险点](49-sandbox-windows-phase-1.1.4i-handoff.md#21-wave-i-5-firewallrs-512-loc--重型-windows-crate-适配)
