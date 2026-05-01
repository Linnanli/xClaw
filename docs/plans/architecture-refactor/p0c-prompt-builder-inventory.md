# P0-C — Prompt Builder Inventory

> **Status**: inventory only — closes #129. No decisions, no recommendations, no
> "should". All decisions deferred to the redline ADR follow-up #37-B.
>
> **Scope**: enumerate the prompt-assembly surface in `claw-code`,
> `desktop-client/ironclaw`, `ironclaw-main`, and `codex-cli-main` so the
> redline reviewer has a single self-contained reference. Every claim is
> backed by `path:line` evidence. Three-tier validation log in §10.
>
> **Mirrors**: `docs/plans/architecture-refactor/p0a-sandbox-activation-inventory.md`
> 12-section template (PR #134, Closes #126).

---

## 1. Source-of-truth file map

| Project | Builder / assembler | File | Lines |
|---|---|---|---|
| `claw-code` (upstream baseline) | `SystemPromptBuilder` + `load_system_prompt` | [claw-code/rust/crates/runtime/src/prompt.rs](claw-code/rust/crates/runtime/src/prompt.rs) | 1–920 |
| `claw-code` git context helper | `GitContext::detect` (referenced from `ProjectContext`) | [claw-code/rust/crates/runtime/src/git_context.rs](claw-code/rust/crates/runtime/src/git_context.rs) | (separate file) |
| `desktop-client/ironclaw` (fork) | `LayeredPromptBuilder` (top-level) | [desktop-client/ironclaw/src/llm/prompt/mod.rs](desktop-client/ironclaw/src/llm/prompt/mod.rs) | 1–320 |
| `desktop-client/ironclaw` (fork) | `StaticLayer::build` | [desktop-client/ironclaw/src/llm/prompt/static_layer.rs](desktop-client/ironclaw/src/llm/prompt/static_layer.rs) | 1–120+ |
| `desktop-client/ironclaw` (fork) | `dynamic_layer::build` | [desktop-client/ironclaw/src/llm/prompt/dynamic_layer.rs](desktop-client/ironclaw/src/llm/prompt/dynamic_layer.rs) | 1–200 |
| `desktop-client/ironclaw` caller | `Reasoning::build_system_prompt_with_tools` | [desktop-client/ironclaw/src/llm/reasoning.rs](desktop-client/ironclaw/src/llm/reasoning.rs#L807-L840) | 793–840 |
| `crates/x_claw_agent` (shared) | `pub const PROMPT_CACHE_BOUNDARY` (boundary literal contract) | [crates/x_claw_agent/src/prompt.rs](crates/x_claw_agent/src/prompt.rs#L47) | 47 |
| `codex-cli-main` (constants) | `pub const BASE_INSTRUCTIONS` (the `prompt.md` blob) | [codex-cli-main/codex-rs/models-manager/src/model_info.rs](codex-cli-main/codex-rs/models-manager/src/model_info.rs#L16) | 16 |
| `codex-cli-main` (assembler) | `Session::build_*_context` (no builder struct; inline composition) | [codex-cli-main/codex-rs/core/src/session/mod.rs](codex-cli-main/codex-rs/core/src/session/mod.rs#L2620-L2680) | 2620–2680 |
| `codex-cli-main` (instructions loader) | `AgentsMdManager::user_instructions` | [codex-cli-main/codex-rs/core/src/agents_md.rs](codex-cli-main/codex-rs/core/src/agents_md.rs#L78-L130) | 47–200 |
| `codex-cli-main` (rendering fragment) | `UserInstructions` (`ContextualUserFragment`) | [codex-cli-main/codex-rs/core/src/context/user_instructions.rs](codex-cli-main/codex-rs/core/src/context/user_instructions.rs#L4-L17) | 1–17 |
| `codex-cli-main` (rendering fragment) | `EnvironmentContext` | [codex-cli-main/codex-rs/core/src/context/environment_context.rs](codex-cli-main/codex-rs/core/src/context/environment_context.rs#L10-L17) | 1–90 |
| `ironclaw-main` (legacy server) | `Workspace::system_prompt()` (opaque user-stored string; no builder) | [ironclaw-main/src/agent/heartbeat.rs](ironclaw-main/src/agent/heartbeat.rs#L340) / [ironclaw-main/src/agent/routine_engine.rs](ironclaw-main/src/agent/routine_engine.rs#L1546) | 340 / 1546 |

---

## 2. Public API surface

### 2.1 `claw-code` `SystemPromptBuilder`

Source: [claw-code/rust/crates/runtime/src/prompt.rs#L94-L165](claw-code/rust/crates/runtime/src/prompt.rs#L94-L165).

Re-exported in [claw-code/rust/crates/runtime/src/lib.rs#L133-L134](claw-code/rust/crates/runtime/src/lib.rs#L133-L134).

| Item | Kind | Line | Notes |
|---|---|---|---|
| `pub const SYSTEM_PROMPT_DYNAMIC_BOUNDARY` | const `&str` | 39 | Literal `"__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__"` |
| `pub const FRONTIER_MODEL_NAME` | const `&str` | 41 | Literal `"Claude Opus 4.6"` |
| `MAX_INSTRUCTION_FILE_CHARS` | crate-private const | 42 | `4_000` |
| `MAX_TOTAL_INSTRUCTION_CHARS` | crate-private const | 43 | `12_000` |
| `pub enum PromptBuildError { Io(io::Error), Config(ConfigError) }` | error type | ~25–35 | |
| `pub struct ContextFile { path: PathBuf, content: String }` | data | 47 | |
| `pub struct ProjectContext { cwd, current_date, git_status, git_diff, git_context, instruction_files }` | data, all `pub`, `Default + Clone + PartialEq + Eq` | 54 | |
| `ProjectContext::discover(cwd, current_date) -> io::Result<Self>` | constructor | 65 | walks ancestor chain for instruction files; no git |
| `ProjectContext::discover_with_git(cwd, current_date) -> io::Result<Self>` | constructor | 81 | adds `git_status` + `git_diff` + `git_context` |
| `pub struct SystemPromptBuilder { … all fields private }` | builder, `Default + Clone + PartialEq + Eq` | 95 | |
| `SystemPromptBuilder::new()` | ctor | 107 | returns `Self::default()` |
| `with_output_style(name, prompt)` | builder | 110 | |
| `with_os(os_name, os_version)` | builder | 116 | |
| `with_project_context(ProjectContext)` | builder | 122 | |
| `with_runtime_config(RuntimeConfig)` | builder | 128 | |
| `append_section(section)` | builder | 134 | |
| `build(&self) -> Vec<String>` | output | 140 | |
| `render(&self) -> String` | output | 165 | `build().join("\n\n")` |
| `pub fn load_system_prompt(cwd, current_date, os_name, os_version) -> Result<Vec<String>, PromptBuildError>` | top-level entry | 432 | discovers ProjectContext + RuntimeConfig and runs builder |

### 2.2 `ironclaw` (desktop-client) `LayeredPromptBuilder`

Source: [desktop-client/ironclaw/src/llm/prompt/mod.rs](desktop-client/ironclaw/src/llm/prompt/mod.rs).

| Item | Kind | Line | Notes |
|---|---|---|---|
| `mod dynamic_layer` / `mod static_layer` | private modules | 14–15 | only re-exports below leak |
| `pub use dynamic_layer::DynamicLayerInput` | re-export | 17 | |
| `pub use static_layer::StaticLayer` | re-export | 18 | |
| `fn cache_boundary_section() -> String` | private | 30 | `"\n\n<!-- {PROMPT_CACHE_BOUNDARY} -->\n\n"` (HTML-comment wrapper) |
| `pub struct LayeredPrompt { pub text: String, pub static_changed: bool }` | output | 37 | |
| `pub struct LayeredPromptBuilder { … all fields private }` | builder | 51 | fields: `static_layer: Arc<String>`, `static_hash: u64`, `supports_cache_boundary: bool` |
| `LayeredPromptBuilder::new(tools: &[ToolDefinition], static_config: &StaticLayerConfig)` | ctor | 62 | builds static text once, computes hash, defaults `supports_cache_boundary=false` |
| `with_cache_boundary(enabled: bool) -> Self` | builder | 73 | |
| `refresh_static(&mut self, tools, config) -> bool` | mutator | 80 | rehashes; returns `true` iff static rebuilt |
| `build(&self, dynamic: &DynamicLayerInput) -> LayeredPrompt` | output | 95 | concat with marker iff `supports_cache_boundary`, else `\n\n` join |
| `static_hash(&self) -> u64` | accessor | 117 | |
| `pub struct StaticLayerConfig { identity: String, model_name: String, has_native_thinking: bool }` | data, `Hash + Clone + Debug` | 137 | |
| `impl Default for StaticLayerConfig` | trait | 147 | all-empty defaults |
| `pub struct DynamicLayerInput { 9 × Option<String> }` | data, `Default + Clone + Debug` | dynamic_layer.rs:8 | fields below in §3.2 |
| `pub struct StaticLayer` (zero-sized) | builder | static_layer.rs:11 | `pub fn build(tools, config) -> String` |

### 2.3 `codex` prompt-assembly surface

No public builder type. Composition lives inside `Session` and is driven by
several public/internal pieces:

| Item | Kind | File:Line |
|---|---|---|
| `pub const BASE_INSTRUCTIONS: &str = include_str!("../prompt.md")` | const | [codex-cli-main/codex-rs/models-manager/src/model_info.rs#L16](codex-cli-main/codex-rs/models-manager/src/model_info.rs#L16) |
| `pub struct AgentsMdManager<'a> { config: &'a Config }` | discovery | [codex-cli-main/codex-rs/core/src/agents_md.rs#L47](codex-cli-main/codex-rs/core/src/agents_md.rs#L47) |
| `AgentsMdManager::new(config)` | ctor | agents_md.rs:57 |
| `AgentsMdManager::user_instructions(env) -> Option<String>` | API | agents_md.rs:80 |
| `AgentsMdManager::user_instructions_with_fs(fs) -> Option<String>` | API | agents_md.rs:88 |
| `AgentsMdManager::instruction_sources(fs) -> Vec<AbsolutePathBuf>` | API | agents_md.rs:128 |
| `pub const DEFAULT_AGENTS_MD_FILENAME = "AGENTS.md"` | const | agents_md.rs:36 |
| `pub const LOCAL_AGENTS_MD_FILENAME = "AGENTS.override.md"` | const | agents_md.rs:38 |
| `pub(crate) struct UserInstructions { directory, text }` | rendering | [codex-cli-main/codex-rs/core/src/context/user_instructions.rs#L4](codex-cli-main/codex-rs/core/src/context/user_instructions.rs#L4) |
| `UserInstructions: ContextualUserFragment` (`START_MARKER = "# AGENTS.md instructions for "`, `END_MARKER = "</INSTRUCTIONS>"`) | trait | user_instructions.rs:9–17 |
| `pub(crate) struct EnvironmentContext { cwd, shell, current_date, timezone, network, subagents }` | rendering | [codex-cli-main/codex-rs/core/src/context/environment_context.rs#L10](codex-cli-main/codex-rs/core/src/context/environment_context.rs#L10) |
| inline composition (developer_sections / contextual_user_sections → `TurnContextItem`) | private | [codex-cli-main/codex-rs/core/src/session/mod.rs#L2620-L2680](codex-cli-main/codex-rs/core/src/session/mod.rs#L2620-L2680) |

### 2.4 `ironclaw-main` (legacy server)

No builder — `system_prompt` is an opaque user-managed string fetched from
workspace storage and concatenated as the first message:

- [ironclaw-main/src/agent/heartbeat.rs#L340-L355](ironclaw-main/src/agent/heartbeat.rs#L340-L355)
- [ironclaw-main/src/agent/routine_engine.rs#L1546-L1580](ironclaw-main/src/agent/routine_engine.rs#L1546-L1580)
- [ironclaw-main/src/channels/web/handlers/system_prompt.rs#L51-L65](ironclaw-main/src/channels/web/handlers/system_prompt.rs#L51-L65) (HTTP CRUD with `MAX_SYSTEM_PROMPT_SIZE = 64 * 1024`)

---

## 3. Section / field comparison

### 3.1 Section ordering

`claw-code` `SystemPromptBuilder::build()` ([prompt.rs#L140-L165](claw-code/rust/crates/runtime/src/prompt.rs#L140-L165)):

1. `get_simple_intro_section(has_output_style)`
2. (optional) `# Output Style: <name>\n<prompt>`
3. `get_simple_system_section()`
4. `get_simple_doing_tasks_section()`
5. `get_actions_section()`
6. `SYSTEM_PROMPT_DYNAMIC_BOUNDARY` — bare literal, no HTML wrapper
7. `# Environment context` (Model family / cwd / Date / Platform)
8. (optional) `# Project context` (rendered ProjectContext)
9. (optional) `# Claude instructions` (rendered instruction files)
10. (optional) runtime config (`# Runtime config`)
11. (any) `append_sections` appended in order

`ironclaw` `LayeredPromptBuilder::build()` ([prompt/mod.rs#L95-L116](desktop-client/ironclaw/src/llm/prompt/mod.rs#L95-L116)):

1. Static layer (fixed by `StaticLayer::build`):
   1. fixed identity line `"You are IronClaw Agent, a secure autonomous assistant."`
   2. `## Response Format` (or `## Response Format — CRITICAL` with `<think>/<final>` schema if `!has_native_thinking`)
   3. `## Guidelines` (incl. `<suggestions>` JSON tail directive)
   4. tool-guidance bullets (only when `tools.len() > 0`)
   5. `## Safety`
   6. `## Available Tools` (only when `tools.len() > 0`)
   7. `--- {identity}` (only when `config.identity` non-empty; `identity` is the workspace AGENTS.md / SOUL.md / IDENTITY.md blob from caller)
2. Boundary (only when `supports_cache_boundary == true`): `\n\n<!-- __SYSTEM_PROMPT_DYNAMIC_BOUNDARY__ -->\n\n` — HTML-comment wrapped
3. Dynamic layer (built by `dynamic_layer::build`, joined by `\n\n`, empty/empty-string fields skipped):
   1. `## Active Skills` (with non-override disclaimer)
   2. `## Channel`
   3. extensions guidance (caller-formatted, raw)
   4. `## Conversation Context`
   5. group guidance (raw)
   6. `## Runtime`
   7. `## Admin Policy` (with precedence note)
   8. `## Token Budget`
   9. `## Language`

When `supports_cache_boundary == false`, the join is `format!("{}\n\n{}", static, dynamic)` with no marker.

`codex` ordering ([session/mod.rs#L2620-L2680](codex-cli-main/codex-rs/core/src/session/mod.rs#L2620-L2680)):

1. `BASE_INSTRUCTIONS` (or `model.base_instructions`) is sent as `instructions` parameter on the API request, not concatenated into the user/developer message.
2. Developer message items are built from `developer_sections`, including:
   - `AvailablePluginsInstructions` (when plugins loaded)
   - commit-message trailer (when `Feature::CodexGitCommit` enabled)
   - guardian developer instructions (separate developer item when `separate_guardian_developer_message`)
3. Contextual user message items are built from `contextual_user_sections`:
   - `UserInstructions { directory, text }` (rendered with markers `# AGENTS.md instructions for <dir>` … `<INSTRUCTIONS>…</INSTRUCTIONS>`)
   - `EnvironmentContext` (cwd / shell / date / tz / network / subagents) when `config.include_environment_context`

### 3.2 Field-by-field comparison: contextual inputs

| Concept | claw-code (`ProjectContext`) | ironclaw static (`StaticLayerConfig`) | ironclaw dynamic (`DynamicLayerInput`) | codex |
|---|---|---|---|---|
| Working directory | `cwd: PathBuf` | — | — (caller embeds in `runtime_info`) | `EnvironmentContext.cwd: Option<PathBuf>` |
| Date | `current_date: String` | — | — (caller embeds) | `EnvironmentContext.current_date: Option<String>` |
| OS / platform | `os_name`/`os_version` on **builder**, not context | — | — (caller embeds) | `EnvironmentContext.shell: String` (no separate OS) |
| Timezone | not represented | — | — | `EnvironmentContext.timezone: Option<String>` |
| Network policy | not represented | — | — | `EnvironmentContext.network: Option<NetworkContext>` |
| Subagents context | not represented | — | — | `EnvironmentContext.subagents: Option<String>` |
| Git status snapshot | `git_status: Option<String>` | — | — | not in EnvironmentContext (separate fragment / not standard) |
| Git diff snapshot | `git_diff: Option<String>` | — | — | n/a |
| Git recent commits / branch | `git_context: Option<GitContext>` | — | — | n/a |
| Project doc / instruction files | `instruction_files: Vec<ContextFile>` (CLAUDE.md/CLAUDE.local.md/.claw/CLAUDE.md/.claw/instructions.md) | conflated into `identity: String` (caller passes pre-loaded blob) | n/a | discovered by `AgentsMdManager` from `AGENTS.md` walking from project root markers down |
| Output style | `output_style_name` + `output_style_prompt` (builder fields) | not represented | not represented | not represented |
| Identity / persona | not represented | `identity: String` | not represented | not represented (BASE_INSTRUCTIONS is fixed) |
| Model identity | implicit `FRONTIER_MODEL_NAME` constant | `model_name: String` | (also `runtime_info` may carry) | `model.base_instructions` swappable in `ModelsManager` |
| Native thinking model flag | not represented | `has_native_thinking: bool` (alters `## Response Format`) | n/a | not represented at this layer |
| Skills | not represented | not represented | `skill_context: Option<String>` (with non-override disclaimer) | not represented at this layer |
| Channel hint | not represented | not represented | `channel: Option<String>` | not represented |
| Extension guidance | not represented | not represented | `extensions_guidance: Option<String>` (caller pre-formatted) | not represented |
| Conversation context | not represented | not represented | `conversation_context: Option<String>` | not represented |
| Group chat guidance | not represented | not represented | `group_guidance: Option<String>` | not represented |
| Runtime metadata | partially via runtime_config (`with_runtime_config`) | not represented | `runtime_info: Option<String>` | not represented |
| Admin policy override | not represented | not represented | `admin_policy: Option<String>` (with precedence note) | not represented |
| Token budget | not represented | not represented | `token_budget: Option<String>` | not represented |
| Language preference | not represented | not represented | `language_preference: Option<String>` | not represented |
| Tool list (machine-readable) | injected via `with_runtime_config(RuntimeConfig)` if at all | tools passed to `LayeredPromptBuilder::new` and rendered as `## Available Tools` | n/a | `tools` are sent as a separate API field, not in prompt |
| Generic appender | `append_section(s)` on builder | none | none | `developer_sections: Vec<String>` filled inline |
| Plugin-injected sections | not represented | not represented | not represented | `AvailablePluginsInstructions::from_plugins(...)` ([session/mod.rs#L2625](codex-cli-main/codex-rs/core/src/session/mod.rs#L2625)) |

### 3.3 Boundary marker handling

| Property | claw-code | ironclaw | codex |
|---|---|---|---|
| Literal value | `__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__` | same (re-uses `x_claw_agent::PROMPT_CACHE_BOUNDARY` since W3-A P0-1) | n/a — codex relies on `instructions` API parameter being separate from messages, no in-band marker |
| Wrapper | bare token (its own section) | HTML comment `<!-- ... -->` | n/a |
| Always emitted? | always (unconditional `sections.push(SYSTEM_PROMPT_DYNAMIC_BOUNDARY.to_string())`) | only when `supports_cache_boundary == true` (set per-call from `model_name.contains("claude")` in `Reasoning::build_system_prompt_with_tools`) | n/a |

### 3.4 Instruction-file discovery

| Property | claw-code | codex |
|---|---|---|
| Filename(s) | `CLAUDE.md`, `CLAUDE.local.md`, `.claw/CLAUDE.md`, `.claw/instructions.md` ([prompt.rs:556-580](claw-code/rust/crates/runtime/src/prompt.rs#L556) test fixture) | `AGENTS.md` (default), `AGENTS.override.md` (local override) ([agents_md.rs:36-38](codex-cli-main/codex-rs/core/src/agents_md.rs#L36)) |
| Walk strategy | walk **ancestors** of cwd (root → cwd descent for ordering) | `project_root_markers` (default `.git`) determines the root; concatenate from project-root → cwd inclusive ([agents_md.rs:1-17](codex-cli-main/codex-rs/core/src/agents_md.rs#L1-L17)) |
| Per-file size cap | `MAX_INSTRUCTION_FILE_CHARS = 4_000` chars; truncated with `[truncated]` suffix | per-file cap derives from `project_doc_max_bytes` total budget; truncates and warns ([agents_md.rs#L160-L195](codex-cli-main/codex-rs/core/src/agents_md.rs#L160-L195)) |
| Total cap | `MAX_TOTAL_INSTRUCTION_CHARS = 12_000` chars | `config.project_doc_max_bytes` (configurable; `0` disables) |
| Dedup identical content | yes — see `dedupes_identical_instruction_content_across_scopes` test ([prompt.rs:594-610](claw-code/rust/crates/runtime/src/prompt.rs#L594)) | not represented (concatenation is path-ordered) |
| Hierarchical message | not represented | `HIERARCHICAL_AGENTS_MESSAGE` appended when `Feature::ChildAgentsMd` enabled ([agents_md.rs:115-119](codex-cli-main/codex-rs/core/src/agents_md.rs#L115)) |
| Global-home overlay | reads `RuntimeConfig` (e.g. `~/.claw/settings.json`) via `with_runtime_config` | `load_global_instructions(codex_dir)` reads `<codex_dir>/AGENTS.md` ([agents_md.rs:62-78](codex-cli-main/codex-rs/core/src/agents_md.rs#L62)) |

`ironclaw` does no instruction-file discovery in the prompt layer — the
`identity` string in `StaticLayerConfig` is pre-assembled by the caller (see
`Reasoning::build_system_prompt_with_tools` line [reasoning.rs:824](desktop-client/ironclaw/src/llm/reasoning.rs#L824) which passes `self.workspace_system_prompt.clone().unwrap_or_default()`).

`ironclaw-main` likewise has no discovery — `Workspace::system_prompt()`
returns a single user-stored string ([heartbeat.rs:340](ironclaw-main/src/agent/heartbeat.rs#L340)).

---

## 4. Behaviour comparison: missing inputs / errors / size budgets

| Scenario | claw-code | ironclaw | codex |
|---|---|---|---|
| Builder created without project context | `environment_section` falls back to `"unknown"` cwd / date and `"unknown unknown"` platform ([prompt.rs:166-191](claw-code/rust/crates/runtime/src/prompt.rs#L166)) | not applicable — context not part of builder | `EnvironmentContext` only added when `config.include_environment_context == true` ([session/mod.rs:2649](codex-cli-main/codex-rs/core/src/session/mod.rs#L2649)) |
| Builder created with no inputs at all | `SystemPromptBuilder::default().render()` returns the canned scaffolding + boundary + `unknown` env section | `LayeredPromptBuilder::new(&[], &StaticLayerConfig::default())` returns the canned IronClaw identity prompt + Response Format + Guidelines + Safety, no tools section, no identity tail; dynamic appended as empty string | `BASE_INSTRUCTIONS` always present; everything else conditional |
| Filesystem error during instruction-file discovery | bubbles as `PromptBuildError::Io` from `load_system_prompt` ([prompt.rs:432-450](claw-code/rust/crates/runtime/src/prompt.rs#L432)) | n/a (no discovery) | `read_agents_md` returns `Ok(None)` on `NotFound`, propagates other I/O ([agents_md.rs:144-198](codex-cli-main/codex-rs/core/src/agents_md.rs#L144)) |
| Config parse error | bubbles as `PromptBuildError::Config` | n/a | n/a |
| Empty/whitespace-only instruction file | dedupe trims and may collapse via `normalize_instruction_content`; `render_instruction_files` only emits scope/content blocks | n/a | files whose trimmed content is empty are skipped ([agents_md.rs:65-72](codex-cli-main/codex-rs/core/src/agents_md.rs#L65) and [agents_md.rs:194-200](codex-cli-main/codex-rs/core/src/agents_md.rs#L194)) |
| Empty `DynamicLayerInput` field (`Some("")`) | n/a | section omitted (`if !s.is_empty()` filter at every branch in [dynamic_layer.rs#L37-L99](desktop-client/ironclaw/src/llm/prompt/dynamic_layer.rs#L37-L99)) | n/a |
| Tool list empty | rendering of tools is owned by `RuntimeConfig`, not exercised here | `Available Tools` section + tool-guidance bullets both omitted ([static_layer.rs:69-72](desktop-client/ironclaw/src/llm/prompt/static_layer.rs#L69) and [static_layer.rs:88-91](desktop-client/ironclaw/src/llm/prompt/static_layer.rs#L88)) | tools live outside the prompt |
| Oversized instruction content | per-file truncated at 4 000 chars; total truncated at 12 000 chars; `[truncated]` suffix added | n/a (caller passes pre-formed string) | per-file truncated to remaining budget; warning logged; total budget = `project_doc_max_bytes` |
| HTTP-managed system_prompt size limit | n/a | n/a | n/a (codex has no HTTP system_prompt put route in this layer) |
| HTTP-managed system_prompt size limit (ironclaw-main) | — | `MAX_SYSTEM_PROMPT_SIZE = 64 * 1024` bytes (rejects beyond) ([handlers/system_prompt.rs:51-65](ironclaw-main/src/channels/web/handlers/system_prompt.rs#L51)) | n/a |
| Cache boundary on non-Anthropic model | always emitted regardless | suppressed (`is_anthropic = model_name.contains("claude")` then `with_cache_boundary(is_anthropic)` in [reasoning.rs:818-826](desktop-client/ironclaw/src/llm/reasoning.rs#L818)) | n/a |

---

## 5. Caller inventory

### 5.1 `claw-code` callers of `SystemPromptBuilder` / `load_system_prompt`

(via `grep_search "SystemPromptBuilder|load_system_prompt"` across `claw-code/**/*.rs`, full result in §10)

| Caller | File:Line | Use |
|---|---|---|
| CLI entry point | [claw-code/rust/crates/rusty-claude-cli/src/main.rs#L2124](claw-code/rust/crates/rusty-claude-cli/src/main.rs#L2124) | `load_system_prompt(cwd, date, env::consts::OS, "unknown")` |
| CLI elsewhere | [claw-code/rust/crates/rusty-claude-cli/src/main.rs#L6174](claw-code/rust/crates/rusty-claude-cli/src/main.rs#L6174) | `load_system_prompt(...)` |
| Tools crate | [claw-code/rust/crates/tools/src/lib.rs#L3621](claw-code/rust/crates/tools/src/lib.rs#L3621) | `load_system_prompt(...)` |
| Conversation module | [claw-code/rust/crates/runtime/src/conversation.rs#L925](claw-code/rust/crates/runtime/src/conversation.rs#L925) | `SystemPromptBuilder::new()...` |
| Self-tests | prompt.rs:711 / 807 / 847 | tests in same file |

### 5.2 `desktop-client/ironclaw` callers of `LayeredPromptBuilder`

| Caller | File:Line | Use |
|---|---|---|
| Reasoning loop | [desktop-client/ironclaw/src/llm/reasoning.rs#L807-L840](desktop-client/ironclaw/src/llm/reasoning.rs#L807-L840) | `Reasoning::build_system_prompt_with_tools` builds `StaticLayerConfig` from `model_name` / `has_native_thinking` / `workspace_system_prompt`, sets `with_cache_boundary(is_anthropic)`, fills `DynamicLayerInput` from runtime fields and emits `.text` |
| Mod tests | desktop-client/ironclaw/src/llm/prompt/mod.rs:185–256 | 8 tests (see §6) |
| Parity gate | [desktop-client/ironclaw/tests/parity_gate_p1.rs#L505](desktop-client/ironclaw/tests/parity_gate_p1.rs#L505) and [#L537](desktop-client/ironclaw/tests/parity_gate_p1.rs#L537) | parity assertions vs claw-code baseline |
| Layered boundary regression | [desktop-client/ironclaw/tests/prompt_layered_boundary_test.rs](desktop-client/ironclaw/tests/prompt_layered_boundary_test.rs#L11) | guards W3-A P0-1 invariant |

### 5.3 `codex` callers of the assembly path

| Caller | File:Line | Use |
|---|---|---|
| `Session::on_initialize` (or equivalent) | [codex-cli-main/codex-rs/core/src/session/mod.rs#L501-L502](codex-cli-main/codex-rs/core/src/session/mod.rs#L501-L502) | `AgentsMdManager::new(&config).user_instructions(environment.as_deref())` |
| Per-turn context build | [codex-cli-main/codex-rs/core/src/session/mod.rs#L2639-L2655](codex-cli-main/codex-rs/core/src/session/mod.rs#L2639-L2655) | renders `UserInstructions` and `EnvironmentContext` into items |
| `TurnContext` inheritance | [codex-cli-main/codex-rs/core/src/session/turn_context.rs#L72](codex-cli-main/codex-rs/core/src/session/turn_context.rs#L72) and [#L212](codex-cli-main/codex-rs/core/src/session/turn_context.rs#L212) and [#L482](codex-cli-main/codex-rs/core/src/session/turn_context.rs#L482) | propagates `user_instructions` across turns |
| `ModelInfo` defaults | [codex-cli-main/codex-rs/models-manager/src/model_info.rs#L55-L81](codex-cli-main/codex-rs/models-manager/src/model_info.rs#L55-L81) | `BASE_INSTRUCTIONS` fallback / per-model override |

### 5.4 `ironclaw-main` (legacy) callers of `Workspace::system_prompt()`

| Caller | File:Line |
|---|---|
| Heartbeat | [ironclaw-main/src/agent/heartbeat.rs#L340](ironclaw-main/src/agent/heartbeat.rs#L340) |
| Routine engine (`run_routine`) | [ironclaw-main/src/agent/routine_engine.rs#L1546](ironclaw-main/src/agent/routine_engine.rs#L1546) |
| Routine engine (passed to inner helper) | [ironclaw-main/src/agent/routine_engine.rs#L1568-L1578](ironclaw-main/src/agent/routine_engine.rs#L1568) |
| HTTP CRUD | [ironclaw-main/src/channels/web/handlers/system_prompt.rs#L51](ironclaw-main/src/channels/web/handlers/system_prompt.rs#L51) and [#L63](ironclaw-main/src/channels/web/handlers/system_prompt.rs#L63) |
| Debug feature | [ironclaw-main/src/channels/web/features/debug/mod.rs#L102-L114](ironclaw-main/src/channels/web/features/debug/mod.rs#L102) |

---

## 6. Test coverage matrix

### 6.1 `claw-code` — `prompt.rs#tests`

13 tests in [claw-code/rust/crates/runtime/src/prompt.rs#L519-L919](claw-code/rust/crates/runtime/src/prompt.rs#L519):

| # | Test name | Covers |
|---|---|---|
| 1 | `discovers_instruction_files_from_ancestor_chain` | walks ancestor chain, ordering root→leaf, includes `.claw/CLAUDE.md` and `.claw/instructions.md` |
| 2 | `dedupes_identical_instruction_content_across_scopes` | dedupe by normalized content |
| 3 | `truncates_large_instruction_content_for_rendering` | per-file 4 000-char truncation |
| 4 | `normalizes_and_collapses_blank_lines` | `normalize_instruction_content` + `collapse_blank_lines` |
| 5 | `displays_context_paths_compactly` | `display_context_path("/.../.claw/CLAUDE.md") == "CLAUDE.md"` |
| 6 | `discover_with_git_includes_status_snapshot` | `git status` capture |
| 7 | `discover_with_git_includes_recent_commits_and_renders_them` | `GitContext` recent-commits rendering |
| 8 | `discover_with_git_includes_diff_snapshot_for_tracked_changes` | `git diff` capture |
| 9 | `load_system_prompt_reads_claude_files_and_config` | end-to-end `load_system_prompt` |
| 10 | `renders_claude_code_style_sections_with_project_context` | full render contains `# System` / `# Project context` / `# Claude instructions` / `permissionMode` / `SYSTEM_PROMPT_DYNAMIC_BOUNDARY` |
| 11 | `truncates_instruction_content_to_budget` | `truncate_instruction_content` budget logic |
| 12 | `discovers_dot_claude_instructions_markdown` | `.claw/instructions.md` discovery + render |
| 13 | `renders_instruction_file_metadata` | scope / content rendering |

### 6.2 `ironclaw` — `prompt/mod.rs#tests`

8 tests in [desktop-client/ironclaw/src/llm/prompt/mod.rs#L181-L259](desktop-client/ironclaw/src/llm/prompt/mod.rs#L181):

| # | Test name | Covers |
|---|---|---|
| 1 | `test_static_hash_stable_for_same_inputs` | hash determinism |
| 2 | `test_static_hash_changes_on_tool_change` | hash sensitivity to tool list |
| 3 | `test_refresh_static_returns_false_when_unchanged` | mutator no-op semantics |
| 4 | `test_refresh_static_returns_true_when_tools_change` | mutator rebuild semantics |
| 5 | `test_build_includes_cache_boundary_when_enabled` | `with_cache_boundary(true)` injects `PROMPT_CACHE_BOUNDARY` |
| 6 | `test_build_omits_cache_boundary_when_disabled` | `with_cache_boundary(false)` suppresses marker |
| 7 | `test_build_contains_static_and_dynamic_content` | both layers present |
| 8 | (combined / partial) | covered above |

### 6.3 `ironclaw` — `prompt/dynamic_layer.rs#tests`

5 tests at [desktop-client/ironclaw/src/llm/prompt/dynamic_layer.rs#L107-L200](desktop-client/ironclaw/src/llm/prompt/dynamic_layer.rs#L107):

| # | Test name | Covers |
|---|---|---|
| 1 | `test_empty_input_produces_empty_string` | all-None input → `""` |
| 2 | `test_skill_context_with_disclaimer` | `## Active Skills` + non-override disclaimer |
| 3 | `test_admin_policy_precedence_note` | `## Admin Policy` + precedence note |
| 4 | `test_multiple_sections_joined` | section concat |
| 5 | `test_empty_strings_are_skipped` | `Some("")` is skipped |

### 6.4 `ironclaw` — `prompt/static_layer.rs#tests`

Tests file present at [desktop-client/ironclaw/src/llm/prompt/static_layer.rs#L116](desktop-client/ironclaw/src/llm/prompt/static_layer.rs#L116) (count not inventoried in this round; visible test names not enumerated here — declared as a known gap in §11 #6).

### 6.5 `ironclaw` — out-of-tree integration tests

| Test file | What it asserts |
|---|---|
| [desktop-client/ironclaw/tests/parity_gate_p1.rs#L505](desktop-client/ironclaw/tests/parity_gate_p1.rs#L505) | parity gate P1 — `LayeredPromptBuilder::new(&tools, &config)` output baseline |
| [desktop-client/ironclaw/tests/parity_gate_p1.rs#L537](desktop-client/ironclaw/tests/parity_gate_p1.rs#L537) | parity gate P1 — `with_cache_boundary(true)` behaviour |
| [desktop-client/ironclaw/tests/prompt_layered_boundary_test.rs#L11](desktop-client/ironclaw/tests/prompt_layered_boundary_test.rs#L11) | W3-A P0-1 boundary literal regression |
| [crates/x_claw_agent/src/prompt.rs#L52-L60](crates/x_claw_agent/src/prompt.rs#L52) | `boundary_literal_is_stable` — pins `PROMPT_CACHE_BOUNDARY` value |

### 6.6 `codex` — relevant tests

| Test file | What it asserts |
|---|---|
| [codex-cli-main/codex-rs/core/src/context/permissions_instructions_tests.rs#L7](codex-cli-main/codex-rs/core/src/context/permissions_instructions_tests.rs#L7) | renders sandbox-mode instruction text |
| [codex-cli-main/codex-rs/core/src/session/tests.rs (multiple)](codex-cli-main/codex-rs/core/src/session/tests.rs) | session-level fixtures pass `user_instructions: None` / `Some(...)` (see grep matches lines 1661, 2239, 2343, 2792, 3107, 3213, 3427, 4576) — exercise the propagation path but not the rendering markers |
| [codex-cli-main/codex-rs/codex-api/tests/models_integration.rs#L79](codex-cli-main/codex-rs/codex-api/tests/models_integration.rs#L79) | `BaseInstructions` round-trip |
| [codex-cli-main/codex-rs/core/src/session/rollout_reconstruction_tests.rs](codex-cli-main/codex-rs/core/src/session/rollout_reconstruction_tests.rs#L80) | rollout reconstruction with `user_instructions: None` |

---

## 7. Output diff (illustrative, derived from §3 mechanics)

Running the same minimal input — single-tool list, identity `"Be helpful."`,
no project context, no skills — through both builders produces the following
shape (verbatim text omitted; structure only, derived from the `build()` paths
in §3.1):

**`SystemPromptBuilder::new().with_os("linux", "6.8").render()`**

```
# … simple intro …
# … simple system …
# … doing-tasks …
# Executing actions with care
…
__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__
# Environment context
 - Model family: Claude Opus 4.6
 - Working directory: unknown
 - Date: unknown
 - Platform: linux 6.8
```

**`LayeredPromptBuilder::new(&[shell], &StaticLayerConfig{identity:"Be helpful.", model_name:"claude-…", has_native_thinking:false}).with_cache_boundary(true).build(&Default::default()).text`**

```
You are IronClaw Agent, a secure autonomous assistant.

## Response Format — CRITICAL
…
## Guidelines
…
- Call tools when they would help …
…
## Tool Call Style
…
## Safety
…

## Available Tools
…

---

Be helpful.

<!-- __SYSTEM_PROMPT_DYNAMIC_BOUNDARY__ -->

```

(Dynamic body is empty when all `DynamicLayerInput` fields are `None`.)

The two outputs are structurally and lexically disjoint outside the shared
boundary literal: claw-code emits markdown sections with `#`-level 1 headers
(`# Environment context`, `# Project context`, `# Claude instructions`),
while ironclaw uses `##`-level 2 headers (`## Available Tools`,
`## Active Skills`, `## Admin Policy`). The two builders also disagree on
whether any environment / cwd / date metadata is part of the static or
dynamic layer.

---

## 8. Cross-project comparison summary

| Dimension | claw-code | ironclaw (desktop-client) | codex | ironclaw-main |
|---|---|---|---|---|
| Single builder type | yes (`SystemPromptBuilder`) | yes (`LayeredPromptBuilder`) | no — composed inline in `Session` | no — no builder, opaque string from workspace |
| Prompt is a `Vec<String>` | yes (`build()` returns `Vec<String>`, `render()` joins) | no (single `String`) | n/a (multiple typed items) | n/a (string) |
| Static / dynamic layering | yes (single boundary marker) | yes (two-layer with hash cache) | yes — `BASE_INSTRUCTIONS` is API-level "instructions"; user/dev messages are per-turn | no |
| Boundary marker emitted inline | always | only on Anthropic models | n/a (uses API parameter separation) | n/a |
| Boundary literal | `__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__` | same (re-uses claw-code literal) | n/a | n/a |
| Project doc filename(s) | `CLAUDE.md` family | n/a (caller pre-loads identity) | `AGENTS.md` family | n/a |
| Project doc discovery in builder | yes | no | yes (`AgentsMdManager`) | no |
| Per-turn dynamic mutation | append_section + with_runtime_config | `refresh_static` + per-call `DynamicLayerInput` | per-turn `TurnContext.user_instructions` / `developer_instructions` | n/a |
| Static-prefix hash for cache | no (relies on boundary marker only) | yes (`u64` `static_hash` over tools + identity + model + thinking flag) | no | n/a |
| Native thinking branching | no | yes (`has_native_thinking` toggles `<think>/<final>` schema) | no | n/a |
| Output style support | yes (`with_output_style`) | no | no | n/a |
| Tool list rendered into prompt | via `RuntimeConfig` if at all | yes (`## Available Tools` in static) | no — separate API field | n/a |
| Non-override disclaimer for skills | n/a | yes (in `## Active Skills`) | n/a | n/a |
| Admin policy precedence note | n/a | yes (in `## Admin Policy`) | n/a | n/a |
| Total prompt-size cap | per-file 4 000 + total 12 000 chars (instructions only) | none enforced in builder | `project_doc_max_bytes` (configurable) | `64 * 1024` bytes (HTTP CRUD only) |
| Error type | `PromptBuildError { Io, Config }` | none — never fails | `io::Error` from `read_agents_md` | n/a |

---

## 9. Boundary marker handling (consolidated)

See §3.3 for the comparison table. Additionally:

- The literal is centralised at [crates/x_claw_agent/src/prompt.rs#L47](crates/x_claw_agent/src/prompt.rs#L47) (`pub const PROMPT_CACHE_BOUNDARY`).
- claw-code keeps its own duplicate constant `SYSTEM_PROMPT_DYNAMIC_BOUNDARY` at [prompt.rs#L39](claw-code/rust/crates/runtime/src/prompt.rs#L39); the literals are equal by inspection, but the source-of-truth for downstream consumers is `x_claw_agent::PROMPT_CACHE_BOUNDARY`.
- ironclaw imports the shared constant directly: `use x_claw_agent::PROMPT_CACHE_BOUNDARY;` ([prompt/mod.rs#L24](desktop-client/ironclaw/src/llm/prompt/mod.rs#L24)).
- A pinning test exists at [crates/x_claw_agent/src/prompt.rs#L52-L60](crates/x_claw_agent/src/prompt.rs#L52) (`boundary_literal_is_stable`).
- claw-code emits the marker as a bare token; ironclaw wraps it in an HTML comment (`<!-- … -->`) so it is inert to the model.
- ironclaw's wrapper is built by `cache_boundary_section()` ([prompt/mod.rs#L30](desktop-client/ironclaw/src/llm/prompt/mod.rs#L30)).

---

## 10. Methodology + three-tier validation log

### 10.1 Why three-tier was required

Issue #129 requires a comparison across `claw-code` / `ironclaw` /
`ironclaw-main` / `codex-cli-main`. AGENTS.md "任务启动 4 问" answers, with
quotes from the issue:

1. *"是否新增模块 / crate / 文件？"* — yes, this inventory document.
2. *"结论里是否含'X 没有 Y / X 缺 Y'等否定语？"* — yes (e.g.
   "ironclaw does no instruction-file discovery in the prompt layer";
   "ironclaw-main has no builder").
3. *"是否做跨项目对账？"* — yes, four projects.
4. *"是否写架构对账类文档？"* — yes.

All four → must do three-tier validation.

### 10.2 Tier-by-tier evidence

**Tier 1 — semantic_search (concept-level)**

| Concept | Tool / query | Result summary |
|---|---|---|
| Prompt-builder concept | `semantic_search "system prompt builder claw-code ironclaw codex"` (initial scoping) | located both builders + codex's inline composition |
| Cache-boundary concept | inspection of `crates/x_claw_agent/src/prompt.rs` doc-comment | confirmed shared constant + history (W3-A P0-1) |

**Tier 2 — `vscode_listCodeUsages`**

This tool is **not supported for Rust** in the current toolchain (mirrors the
limitation declared in `p0a-sandbox-activation-inventory.md` §10). Substituted
with a second `grep_search` of complementary patterns (see Tier 3) so the
"definition + callers" graph is still cross-validated through two distinct
queries.

**Tier 3 — `grep_search` / `rg` (literal-level)**

| Query (regex) | Scope | Hits / artefact |
|---|---|---|
| `SystemPromptBuilder\|load_system_prompt` | `claw-code/**/*.rs` | 18 matches → §5.1 caller table |
| `LayeredPromptBuilder\|llm::prompt::\|use.*prompt::` | `desktop-client/ironclaw/**/*.rs` | 20 matches → §5.2 caller table |
| `LayeredPromptBuilder` | `ironclaw-main/**/*.rs` (with `includeIgnoredFiles=true`) | 0 matches → ironclaw-main does **not** use `LayeredPromptBuilder` |
| `build_system_prompt\|prompt_builder\|system_prompt` | `ironclaw-main/**/*.rs` (with `includeIgnoredFiles=true`) | 20+ matches; all reference `Workspace::system_prompt()` opaque-string helper, none introduce a builder type → §5.4 + §1 |
| `^pub fn (build_system_prompt\|build_prompt\|system_prompt\|assemble_prompt\|prompt_for_compact)` | `codex-cli-main/codex-rs/**/*.rs` | 0 matches → codex has **no** named prompt-builder entry function |
| `user_instructions\|UserInstructions\|EnvironmentContext\|with_user_instructions\|build_full_user_instructions` | `codex-cli-main/codex-rs/**/*.rs` | 40+ matches → §2.3 surface table |
| `BASE_INSTRUCTIONS\|GPT_5_CODEX_INSTRUCTIONS` | `codex-cli-main/codex-rs/**/*.rs` | located `pub const BASE_INSTRUCTIONS` at `models-manager/src/model_info.rs:16` |
| `struct AgentsMdManager\|impl AgentsMdManager\|pub fn user_instructions` | `codex-cli-main/codex-rs/**/*.rs` | located `AgentsMdManager` at `core/src/agents_md.rs:47` |
| `struct UserInstructions\|struct EnvironmentContext\|fn render` | `codex-cli-main/codex-rs/core/src/context/**/*.rs` | located `UserInstructions` and `EnvironmentContext` rendering structs |

### 10.3 Tier limitations

- `vscode_listCodeUsages` does not support Rust → **declared above**, mitigated by a complementary second grep.
- `search_subagent` returned `"Response contained no choices"` once during exploration; fell back to `grep_search` directly. No claim in this document depends on the failed call.
- Tests in `desktop-client/ironclaw/src/llm/prompt/static_layer.rs` were not enumerated by name — see §11 #6.

---

## 11. Decision points (questions only, deferred to redline ADR #37-B)

This section is **questions only**, no options, no recommendations. All
decisions are reserved for the redline ADR follow-up #37-B.

1. Should there be a single unified prompt-builder type across `claw-code`,
   `ironclaw`, and `codex`, or should the three remain separate with only
   the boundary literal shared via `x_claw_agent`?
2. If a unified type exists, where should the project-doc loader live —
   inside the builder (claw-code style), outside as a discovery helper
   (codex `AgentsMdManager` style), or pre-loaded by the caller as an
   opaque string (ironclaw style)?
3. Which project-doc filename(s) should the unified path support — the
   `CLAUDE.md` family, the `AGENTS.md` family, both, or neither (caller
   passes pre-loaded blob)?
4. How should size budgets be expressed — character counts (claw-code
   4 000 / 12 000) or byte counts (codex `project_doc_max_bytes`)?
5. Should the boundary marker be always emitted (claw-code) or
   conditioned on the model family (ironclaw `is_anthropic`)?
6. Should the marker be a bare token (claw-code) or HTML-comment-wrapped
   (ironclaw)?
7. Should the static-layer prefix carry an in-process `u64` hash for cache
   invalidation (ironclaw `refresh_static`), or rely solely on byte-prefix
   stability for Anthropic's automatic caching?
8. Should the `<think>/<final>` schema branching by `has_native_thinking`
   be part of the builder contract or pushed back into model-specific
   adapters?
9. Should the `<suggestions>` JSON-tail directive (currently in
   ironclaw's `## Guidelines`) be part of the static layer, the dynamic
   layer, or out of scope for the unified builder?
10. Should the `## Active Skills` non-override disclaimer and the
    `## Admin Policy` precedence note be guaranteed by the unified builder
    or remain caller-provided strings?
11. How should error paths be expressed — bubbling `PromptBuildError`
    (claw-code), infallible builder + caller-side I/O (ironclaw),
    or `Result<Option<String>>` (codex `AgentsMdManager`)?
12. Should `ironclaw-main`'s opaque user-stored `system_prompt` be
    migrated onto the unified path, or left out of scope as a legacy
    server-side concern?
13. Should the four output styles (`SystemPromptBuilder.with_output_style`)
    survive in the unified builder, or be expressed through the
    identity / runtime-config channel?
14. Should section header levels be normalised across builders (claw-code
    uses `#`, ironclaw uses `##`)?
15. Should `EnvironmentContext` (codex) — with timezone, network policy,
    subagents — replace claw-code's `# Environment context` block, or
    coexist as a richer optional fragment?
16. Should the unified builder's `build()` return `Vec<String>` (claw-code),
    `String` (ironclaw), or a typed item list (codex `Vec<TurnContextItem>`)?

---

## 12. References

- Parent issue: [#37 redline ADR — prompt builder unification](https://github.com/Linnanli/xClaw/issues/37)
- Related: [#89 — codex / claw-code / ironclaw prompt parity gates](https://github.com/Linnanli/xClaw/issues/89) (per session-handoff index)
- Mirror inventory: `docs/plans/architecture-refactor/p0a-sandbox-activation-inventory.md` (Closes #126, PR #134)
- ADR-112 §5: `docs/plans/architecture-refactor/adr-112-compatibility-evaluation.md` — boundary-literal unification rationale (W3-A Phase 0 P0-1)
- AGENTS.md "任务启动 4 问" — §"任务启动 4 问"
- AGENTS.md "分析工具使用规范" — §"分析工具使用规范（Round 17 建立 / Round 18 实证）"
