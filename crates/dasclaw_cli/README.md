# `dasclaw_cli`

Headless agent CLI binary (`dasclaw-cli`). The proof-point that
[`dasclaw_runtime::Agent`](../dasclaw_runtime/README.md) runs without any
desktop dependency: no Tauri, no database, no channels, no HTTP server.

ADR reference: [ADR-153 §4.4](../../docs/plans/architecture-refactor/adr-153-headless-agent-framework.md)
(CLI proof-point — slices 4 through 8).

```text
$ dasclaw-cli --prompt "Hi"
echo: Hi
```

## Install / run

```bash
cargo build -p dasclaw_cli --release
./target/release/dasclaw-cli --help
```

Or use it as a library from another crate — `lib.rs` exposes `run`,
`run_with_tools`, `EchoResponder`, plus the `provider`, `tools`, `mcp` modules.

## Subcommands

### `echo` (default)

Deterministic built-in responder; replies `echo: <last user content>`. **No
network, no LLM key.** Used as the default smoke driver and exercised in
[`tests/end_to_end.rs`](tests/end_to_end.rs).

```bash
dasclaw-cli echo --prompt "Hi"          # → echo: Hi
dasclaw-cli echo                        # reads prompt from stdin
echo "Hi" | dasclaw-cli echo
```

### `run` — real LLM-backed agent

Wires a `dasclaw_llm_provider`-backed responder. Honours the standard provider
env vars (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `DASCLAW_API_KEY`, …).

```bash
dasclaw-cli run --provider anthropic --model claude-sonnet-4 \
  --prompt "Summarise this README."
```

#### Tool dispatch (`run` only)

Two opt-in flags, freely combinable:

| Flag | Effect |
|---|---|
| `--enable-tools` | Advertise the builtin demo tools `echo` and `now` and dispatch them locally. |
| `--mcp-config <path>` | Spin up one or more MCP servers from a JSON file; their tools are advertised under `<server_name>_<tool>`. |

When both flags are set, the two executors are merged through
[`CompositeToolExecutor`](../dasclaw_runtime/src/composite_executor.rs). Duplicate
tool names across the two sources fail at startup, not at dispatch — matching
the safety stance of the rest of the runtime.

#### Builtin tools

| Tool | Description |
|---|---|
| `echo` | Returns the `message` argument verbatim. |
| `now`  | Returns the current epoch seconds. |

Useful for smoke-testing the agent ↔ tool round-trip without any external
service. Implementation in [`src/tools.rs`](src/tools.rs).

## Common flags

| Flag | Default | Meaning |
|---|---|---|
| `--prompt <text>` | — | User prompt. When omitted, the prompt is read from stdin. |
| `--system <text>` | `"You are dasclaw, a headless agent."` | System prompt prepended to the conversation. |

## MCP config schema (`--mcp-config`)

Top-level shape, loaded by [`mcp::load_executor`](src/mcp.rs):

```json
{
  "servers": [
    {
      "name": "fs",
      "command": "npx",
      "args": ["@modelcontextprotocol/server-filesystem", "/tmp"],
      "env": { "EXTRA": "value" }
    },
    {
      "name": "remote",
      "url": "https://mcp.example.com/v1",
      "headers": { "Authorization": "Bearer XYZ" }
    }
  ]
}
```

Transport is selected by which fields the entry carries:

- **stdio** — entry has `command` (and optional `args`, `env`); launched as a
  subprocess via `StdioMcpTransport`. The parent's environment is inherited;
  `env` is layered on top.
- **HTTP** — entry has `url` (and optional `headers`); spoken to via
  `HttpMcpTransport`.

Mixing both `command` and `url` in one entry, or omitting both, is a parse-time
error (`McpConfigError`).

### Ordering and name collisions

`servers` is ordered. Each advertised tool ends up qualified as
`<server_name>_<tool>` in the LLM-visible list. If two servers ever advertise
the **same qualified name**, the **last** one wins, matching the contract of
`McpToolExecutor::from_clients`.

Combining `--enable-tools` with `--mcp-config` is stricter: the merge through
`CompositeToolExecutor` errors out on duplicates — caught at startup.

### What is **not** wired (deliberate)

- **Hosted / OAuth-backed servers.** The CLI has no persistent secrets store,
  so the HTTP transport sends only the static `headers` map and does not wire
  `Mcp-Session-Id` or `SecretsStore`. Hosted servers belong in ironclaw.
- **Approval prompts.** Headless mode surfaces `AgentError::ApprovalRequested`
  instead of stalling on a TTY.
- **Streaming REPL.** This slice is request-response only.

## Library entry points (`lib.rs`)

| Symbol | Purpose |
|---|---|
| `run(responder, system, user) -> Result<String, CliError>` | Build a no-tools agent and run one prompt. |
| `run_with_tools(responder, executor, defs, system, user) -> Result<String, CliError>` | Same, with a `ToolExecutor` and advertised tool defs. |
| `EchoResponder` | Deterministic `AgentResponder` for tests / smoke. |
| `CliError` | `Agent(AgentError)` or `Build(String)`. |
| Module [`provider`](src/provider.rs) | `ProviderArgs` clap group + `build_responder` factory. |
| Module [`tools`](src/tools.rs) | `StaticToolExecutor` + `default_builtins`. |
| Module [`mcp`](src/mcp.rs) | `load_executor` + `McpConfig` schema types + `McpConfigError`. |

Keeping I/O (argv / stdin / stdout) out of these functions is what lets the
integration tests assert behaviour without spawning the binary.

## Tests

End-to-end coverage under [`tests/`](tests/):

| File | What it locks in |
|---|---|
| [`end_to_end.rs`](tests/end_to_end.rs) | `EchoResponder` round-trip through `run`. |
| [`tool_e2e.rs`](tests/tool_e2e.rs) | Mock LLM → builtin `echo` tool → final text (full agent loop with tools). |
| [`mcp_e2e.rs`](tests/mcp_e2e.rs) | Single-server `--mcp-config` drives stdio fixture through `initialize` / `tools/list` / `tools/call`. |
| [`mcp_multi_e2e.rs`](tests/mcp_multi_e2e.rs) | Two stdio MCP servers in one config; `<name>_<tool>` namespacing keeps them disjoint. |
| [`live_provider.rs`](tests/live_provider.rs) | Optional live-LLM smoke (skipped without API key). |
| [`fixtures/stdio_echo_mcp.rs`](tests/fixtures/stdio_echo_mcp.rs) | Minimal stdio MCP server used by the MCP tests. |

Run them all with `cargo nextest run -p dasclaw_cli`.

## Errors

| Error | Meaning |
|---|---|
| `CliError::Build(msg)` | `AgentBuilder` rejected the configuration. |
| `CliError::Agent(AgentError)` | Loop failure — see `AgentError` variants in `dasclaw_runtime`. |
| `McpConfigError::Read / Parse` | Config file missing or invalid JSON. |
| `McpConfigError::Spawn { server, … }` | Could not start a stdio subprocess. |
| `McpConfigError::ListTools` | `tools/list` round-trip failed for at least one server. |

## Stability

The library surface (`run`, `run_with_tools`, `EchoResponder`, `provider`,
`tools`, `mcp`) is shared by ironclaw and admin-backend wiring slices. Pin to
the exact patch version while ADR-153 is open; the CLI flag shape will stay
stable across the §4.4 slices.

## See also

- [`dasclaw_runtime`](../dasclaw_runtime/README.md) — the agent + traits this CLI is built on.
- [`dasclaw_llm_provider`](../dasclaw_llm_provider/) — concrete LLM backends behind `--provider`.
- [`dasclaw_mcp`](../dasclaw_mcp/) — stdio / HTTP MCP transports loaded by `--mcp-config`.
- ADR-153 §4.4 — the slice-by-slice plan and milestones.
