# 沙箱启动指南（W3 档位 A）

> Issue: [#128](https://github.com/nallylin/x-claw/issues/128) ·
> ADR: [adr-121-p0a-sandbox-activation-decision.md](../../docs/plans/architecture-refactor/adr-121-p0a-sandbox-activation-decision.md)

本文档说明 dev-tool 启动路径上 `ShellTool` 的隔离机制：在何时被
`OsExecutor` 包裹、何时强制走 HTTPS allowlist 代理、以及如何通过环境
变量覆盖默认行为。

## 三种执行模式

`SANDBOX_EXECUTION_MODE` 控制 [`ExecutionMode`](../ironclaw/src/sandbox/config.rs)：

| 取值 | 别名 | 行为 |
| --- | --- | --- |
| `os_sandbox`（默认） | `os` / `bwrap` / `seatbelt` | OS 隔离 + allowlist 代理（W3 档位 A 主路径） |
| `direct` | `off` / `none` | 不沙箱化，仅供工程师本地调试 |
| `docker` | `container` | 交给历史 worker 流水线（本路径不接管） |

**ADR-121 D1=B 默认 fail-CLOSED**：未设环境变量时，`OsSandbox` 是默认
值。政企部署无需任何额外配置即可获得沙箱保护；要降级必须显式
`SANDBOX_EXECUTION_MODE=direct`，并接受 warn 级日志告警。

## 默认策略：WorkspaceWrite

ADR-121 D5=E：`SANDBOX_POLICY` 默认 `workspace_write`（不再是
`readonly`）。让"建项目、跑测试、生成文件"开箱即用，同时保留：

- 工作区外文件：拒绝
- 网络出站：必须经 allowlist 代理（`crates.io` / `registry.npmjs.org`
  / `github.com` / `pypi.org` / 用户配置 `extra_allowed_domains`）

需要更严：`SANDBOX_POLICY=readonly`。需要全开：见下节。

## FullAccess 双重 opt-in

`SANDBOX_POLICY=full_access` **必须**配合 `SANDBOX_ALLOW_FULL_ACCESS=true`，
否则 `app.rs::activate_sandbox` 会发 error 日志并自动降级到 WorkspaceWrite
（ADR-121 D6=A 简化版）。这避免单一环境变量误投生产即解除一切隔离。

## 环境变量速查

| 变量 | 默认 | 说明 |
| --- | --- | --- |
| `SANDBOX_ENABLED` | `true` | 总开关 |
| `SANDBOX_EXECUTION_MODE` | `os_sandbox` | 见上表 |
| `SANDBOX_POLICY` | `workspace_write` | `readonly` / `workspace_write` / `full_access` |
| `SANDBOX_ALLOW_FULL_ACCESS` | `false` | FullAccess 第二把钥匙 |
| `SANDBOX_TIMEOUT_SECS` | `120` | 单次命令超时 |
| `SANDBOX_EXTRA_DOMAINS` | _空_ | 逗号分隔追加 allowlist |
| `SANDBOX_PROXY_PORT` | _随机_ | allowlist HTTPS 代理监听端口 |

## 平台支持

- **Linux**：需 kernel 5.13+、安装 `bwrap`（`apt install bubblewrap`）。
  缺失时 `OsExecutor::new` 不会失败，但运行命令会返回友好错误（ADR-121
  D4a=D / D4b=A）。
- **macOS**：依赖 Codex `sandbox-pref::Auto` 回退到 `seatbelt`，best-effort。
- **Windows**：见 epic [#241](https://github.com/nallylin/x-claw/issues/241)
  fork `codex-windows-sandbox`，不在本 PR 范围。

## 启动流程

1. `app.rs::build_all` → `init_tools` → `activate_sandbox(ctx)`
2. 根据 `execution_mode` 分支：
   - `Direct`：warn 日志，`ctx.sandbox_executor = None`
   - `Docker`：debug 日志返回，由 worker 流水线接管
   - `OsSandbox`：构造 `OsExecutor` + 启 `start_network_proxy` +
     写 `ctx.proxy_env` + 写 `ctx.sandbox_policy`
3. `bootstrap_tools(ctx)` → `register_shell_tool(ctx)` 通过 builder
   fallback 把 executor / policy / proxy_env 注入 `ShellTool`

## 故障排查

| 现象 | 原因 | 处理 |
| --- | --- | --- |
| `Failed to start allowlist proxy` | `SANDBOX_PROXY_PORT` 占用 | 切端口 / kill 进程 |
| `OS sandbox activated without secrets store` | 未配 `secrets_store` | 配置 secrets backend；否则需 API key 的工具会失败 |
| 命令报 `bwrap: command not found` | Linux 未装 bubblewrap | `apt install bubblewrap` |
| `SANDBOX_POLICY=full_access requires …` | 未设第二把钥匙 | 加 `SANDBOX_ALLOW_FULL_ACCESS=true`，或别用 full_access |

## 不在本 PR

- W4 档位 C：`ShellTool` 直接调 `dasclaw_exec` 内部 API（见 ADR-121 §4）
- Windows 路径：见 #241
- 容器 worker 与 OsSandbox 的去重：见 ADR-113
