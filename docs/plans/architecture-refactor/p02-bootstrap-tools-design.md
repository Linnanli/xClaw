# P0-2 Tool System bootstrap_tools() — 范围更正与设计

> **状态**：📝 Draft — 待 review
> **日期**：2026-04-29
> **作者**：xClaw agent
> **关联**：[ADR-112 §5](./adr-112-compatibility-evaluation.md#5-phase-0-路线图w3-a-收尾10-行红线) P0-2 / [adr-112-input-checklist.md](./adr-112-input-checklist.md) §2.5.4
> **依赖**：[PR #39](https://github.com/Linnanli/xClaw/pull/39)（P0-1）合并后实施
> **跟踪 issue**：TBD

---

## 1. 背景：文档与实际不符（重要更正）

按 P0-1 教训（[AGENTS.md Round 17/18 三层验证规范](../../../AGENTS.md#分析工具使用规范)），对 P0-2 范围做实际代码扫描。

### 1.1 文档描述（不准确）

| 来源 | 描述 |
|---|---|
| `adr-112-input-checklist.md` L484 | "5 个 register_\* 方法" |
| `adr-112-input-checklist.md` L542 | "5 文件 / 6 register_\* 匹配" |
| `adr-112-compatibility-evaluation.md` §8 行 #12 | "ironclaw 5 register_\*" |

### 1.2 实际三层验证

`grep -nE 'fn register_' desktop-client/ironclaw/src/tools/registry.rs`：**19 个 register_\* 方法**。

`grep` 调用方（外部，不含 #[cfg(test)]）：**12 个调用点跨 4 个文件**。

```
调用方分布
├─ app.rs          (8): builtin / secrets / memory×2 / image / vision / extension / dev / skill
├─ main.rs         (2): job / message
├─ agent_loop.rs   (1): routine
└─ worker/container.rs (1): container
```

### 1.3 19 个 register_* 完整分类

| # | 方法 | 行号 | async | 调用方 | 类别 |
|---|---|---|---|---|---|
| 1 | `register_sync` | 179 | — | dispatcher.rs / 测试 | **dispatch helper** |
| 2 | `register_builtin_tools` | 280 | — | app.rs:318 / testing/mod.rs / 多测试 | **bootstrap** |
| 3 | `register_tool_info` | 298 | — | （未发现外部 caller，疑似死代码或 future） | **discovery** |
| 4 | `register_orchestrator_tools` | 310 | — | （= register_builtin_tools 别名） | **alias** |
| 5 | `register_container_tools` | 319 | — | worker/container.rs:95 | **bootstrap** |
| 6 | `register_dev_tools` | 382 | — | app.rs:759 / register_builder_tool 内部 | **bootstrap** |
| 7 | `register_memory_tools_with_resolver` | 419 | — | app.rs:371 | **bootstrap** |
| 8 | `register_memory_tools` | 435 | — | app.rs:376 | **bootstrap** |
| 9 | `register_job_tools` | 457 | — | main.rs:574 | **bootstrap** |
| 10 | `register_secrets_tools` | 523 | — | app.rs:322 | **bootstrap** |
| 11 | `register_extension_tools` | 536 | — | app.rs:738 | **bootstrap** |
| 12 | `register_skill_tools` | 551 | — | app.rs:889 | **bootstrap** |
| 13 | `register_routine_tools` | 573 | — | agent_loop.rs:802 | **bootstrap** |
| 14 | `register_message_tools` | 605 | ✅ | main.rs:821 | **bootstrap** |
| 15 | `register_image_tools` | 640 | — | app.rs:417 | **bootstrap** |
| 16 | `register_vision_tools` | 665 | — | app.rs:423 | **bootstrap** |
| 17 | `register_builder_tool` | 688 | ✅ | （动态创建 + 返回 Arc<dyn SoftwareBuilder>） | **dynamic factory** |
| 18 | `register_wasm` | 730 | ✅ | （runtime 插件加载） | **dynamic plugin** |
| 19 | `register_wasm_from_storage` | 799 | ✅ | （runtime 插件加载） | **dynamic plugin** |

### 1.4 ADR-112 修订建议

§8 行 #12 描述 "ironclaw 5 register_*" 需更正为 "**ironclaw 19 register_\* 方法（其中 12 个是跨 4 文件调用的 bootstrap 类，3 个动态注册类需保留独立 API）**"。

---

## 2. 设计目标

### 2.1 必达目标（DoD）

1. 把 **12 个 bootstrap 类** register_* 调用合并为单一入口 `bootstrap_tools(ctx: &BootstrapContext)`
2. 4 个调用文件（app.rs / main.rs / agent_loop.rs / worker/container.rs）简化为 1 个 bootstrap 调用点
3. 红测断言 `count_register_methods() == 1`（只剩 `bootstrap_tools`，加上 dynamic 类作为白名单豁免）

### 2.2 非目标

- ❌ 不收口 **3 个 dynamic 类**：
  - `register_wasm` / `register_wasm_from_storage` — runtime 插件 API，调用方在用户代码（含 SDK 文档示例）
  - `register_builder_tool` — 动态创建并返回 `Arc<dyn SoftwareBuilder>`（**返回值有意义**，与 bootstrap 无副作用模式不兼容）
- ❌ 不动 `register_sync` — 内部 dispatch helper，dispatcher.rs / registry 测试在用
- ❌ 不动 `register_tool_info` / `register_orchestrator_tools` 别名 — 后续清理（疑似死代码）

### 2.3 兼容性约束

- 测试代码 `testing/mod.rs:522` 用 `register_builtin_tools()` 简单初始化 — 必须保留 test convenience 入口
- worker/container.rs 路径需在 BootstrapContext 中加 mode 字段区分 orchestrator vs container

---

## 3. 设计方案

### 3.1 BootstrapContext 数据结构

```rust
/// 工具引导上下文 — bootstrap_tools() 的唯一入参。
///
/// 每个字段是 Option，bootstrap_tools 内部按字段是否 Some 决定调用哪些子 register_*。
/// 这样 app.rs / main.rs / worker/container.rs 都可以用同一个入口，差异由 ctx 表达。
pub struct BootstrapContext {
    /// 部署模式：决定 register_dev_tools 和 register_container_tools 走哪条路径。
    pub mode: BootstrapMode,

    // ─── 主流程依赖 ──────────────────────────
    /// Workspace 实例（memory tools 必需）。
    pub workspace: Option<Arc<Workspace>>,
    /// libsql 连接池（memory_tools_with_resolver 必需，覆盖 workspace 路径）。
    pub db_pool: Option<Arc<LibsqlPool>>,
    /// SecretsStore（secrets / http 鉴权工具必需）。
    pub secrets_store: Option<Arc<SecretsStore>>,
    /// ExtensionManager（extension / message tools 必需）。
    pub extension_manager: Option<Arc<ExtensionManager>>,
    /// 频道注册表（message tools 必需）。
    pub channels: Option<Arc<ChannelRegistry>>,
    /// 技能注册表 + 目录（skill tools 必需）。
    pub skill_registry: Option<Arc<SkillRegistry>>,
    pub skill_catalog: Option<Arc<SkillCatalog>>,
    /// 例程存储 + 引擎（routine tools 必需）。
    pub routine_store: Option<Arc<RoutineStore>>,
    pub routine_engine: Option<Arc<RoutineEngine>>,
    /// 任务参数（job tools 必需）。
    pub job_config: Option<JobToolsConfig>,
    /// 图像/视觉 API（image / vision tools 必需）。
    pub image_api: Option<ImageApiConfig>,
    pub vision_api: Option<VisionApiConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapMode {
    /// 主进程 / orchestrator：不注册 dev/container 工具（除非 allow_local_tools）。
    Orchestrator { allow_local_tools: bool },
    /// 沙箱容器内：注册 dev tools（filesystem, shell, code）。
    Container,
    /// 测试：仅注册 builtin tools（兼容 testing/mod.rs）。
    Test,
}

impl BootstrapContext {
    /// 测试便捷构造器（替代 register_builtin_tools()）。
    pub fn for_test() -> Self {
        Self {
            mode: BootstrapMode::Test,
            ..Default::default()
        }
    }
}
```

### 3.2 bootstrap_tools 实现骨架

```rust
impl ToolRegistry {
    /// 单一引导入口 — 替代 12 个分散的 register_* 调用。
    ///
    /// 调用顺序由依赖关系决定：
    /// 1. builtin（无依赖）
    /// 2. secrets（依赖 secrets_store）
    /// 3. dev / container（依赖 mode）
    /// 4. memory / job / extension / skill / routine / image / vision（独立）
    /// 5. message（依赖 extension_manager + channels，async，最后）
    pub async fn bootstrap_tools(
        self: &Arc<Self>,
        ctx: &BootstrapContext,
    ) -> Result<(), BootstrapError> {
        match ctx.mode {
            BootstrapMode::Test => {
                self.register_builtin_tools_internal();
                return Ok(());
            }
            BootstrapMode::Orchestrator { allow_local_tools: false } => {
                self.register_builtin_tools_internal();
            }
            BootstrapMode::Orchestrator { allow_local_tools: true } | BootstrapMode::Container => {
                self.register_builtin_tools_internal();
                self.register_dev_tools_internal();
            }
        }

        if let Some(ss) = &ctx.secrets_store {
            self.register_secrets_tools_internal(Arc::clone(ss));
        }
        if let Some(pool) = &ctx.db_pool {
            self.register_memory_tools_with_resolver_internal(Arc::clone(pool));
        } else if let Some(ws) = &ctx.workspace {
            self.register_memory_tools_internal(Arc::clone(ws));
        }
        if let Some(cfg) = &ctx.job_config {
            self.register_job_tools_internal(cfg);
        }
        if let Some(em) = &ctx.extension_manager {
            self.register_extension_tools_internal(Arc::clone(em));
        }
        if let (Some(reg), Some(cat)) = (&ctx.skill_registry, &ctx.skill_catalog) {
            self.register_skill_tools_internal(Arc::clone(reg), Arc::clone(cat));
        }
        if let (Some(store), Some(eng)) = (&ctx.routine_store, &ctx.routine_engine) {
            self.register_routine_tools_internal(Arc::clone(store), Arc::clone(eng));
        }
        if let Some(api) = &ctx.image_api {
            self.register_image_tools_internal(api);
        }
        if let Some(api) = &ctx.vision_api {
            self.register_vision_tools_internal(api);
        }
        // async 依赖最后
        if let (Some(ch), Some(em)) = (&ctx.channels, &ctx.extension_manager) {
            self.register_message_tools_internal(Arc::clone(ch), Arc::clone(em)).await;
        }

        Ok(())
    }
}
```

### 3.3 12 个原 pub register_* 处理

- 改为 `_internal` 私有方法（保留实现，重命名）
- 公共 API 仅暴露 `bootstrap_tools` 一个入口
- 例外保留 pub：`register_wasm` / `register_wasm_from_storage` / `register_builder_tool` / `register_sync` / `register_tool_info`（5 个，非 bootstrap 类）

### 3.4 调用方改造

| 文件 | 旧 | 新 |
|---|---|---|
| `app.rs` | 8 处 `tools.register_*` | 1 处 `tools.bootstrap_tools(&ctx).await?` |
| `main.rs` | 2 处 `register_job/message_tools` | 通过 ctx 字段（job_config / channels）传入，仍是 1 处 bootstrap |
| `agent_loop.rs` | 1 处 `register_routine_tools` | 通过 ctx 字段（routine_store / routine_engine）传入 |
| `worker/container.rs` | 1 处 `register_container_tools` | `bootstrap_tools(&ctx { mode: Container, ..})` |
| `testing/mod.rs` | `register_builtin_tools` | `bootstrap_tools(&BootstrapContext::for_test()).await?` |

但 `app.rs` / `main.rs` / `agent_loop.rs` 是同一个进程的不同初始化阶段（按 channel/skill/routine 等组件就绪节奏注册），**无法在单点合并**，因此这三处会保留为 3 个 bootstrap_tools 调用，每次传不同 ctx 字段。

修订后的 DoD：**4 文件 12 调用点 → 4 文件 4 调用点（每个文件 1 处）**。

---

## 4. 红测设计

按 [AGENTS.md TDD 流程](../../../AGENTS.md#开发流程tdd)：

```rust
// desktop-client/ironclaw/tests/bootstrap_tools_unification_test.rs

#[test]
fn req_p02_a_only_bootstrap_tools_is_pub_for_initialization() {
    // 元测试：扫描 registry.rs 的 pub fn register_*
    // 白名单：register_wasm / _from_storage / _builder_tool / _sync / _tool_info
    // 应当不再有其他 pub register_*_tools 方法
}

#[tokio::test]
async fn req_p02_b_bootstrap_tools_test_mode_registers_builtins() {
    let registry = Arc::new(ToolRegistry::new(...));
    registry.bootstrap_tools(&BootstrapContext::for_test()).await.unwrap();
    assert!(registry.count() >= 4); // echo/time/json/http
}

#[tokio::test]
async fn req_p02_c_bootstrap_tools_orchestrator_mode_skips_dev_by_default() {
    let registry = Arc::new(ToolRegistry::new(...));
    let ctx = BootstrapContext { mode: BootstrapMode::Orchestrator { allow_local_tools: false }, ..Default::default() };
    registry.bootstrap_tools(&ctx).await.unwrap();
    // 检查没有注册 file_read / shell 等 dev tools
}

#[tokio::test]
async fn req_p02_c_bootstrap_tools_container_mode_includes_dev() {
    // 同上但 mode=Container，应当包含 dev tools
}

#[tokio::test]
async fn req_p02_d_caller_count_equals_one_per_file() {
    // 元测试：扫描 app.rs / main.rs / agent_loop.rs / worker/container.rs
    // 每个文件应当只有 1 个 bootstrap_tools 调用
}

#[test]
fn req_p02_meta_legacy_register_methods_are_private() {
    // 扫描 registry.rs 源码：register_builtin_tools / register_dev_tools 等 12 个
    // 应当不带 `pub` 前缀
}
```

---

## 5. PR 拆分建议

按 [AGENTS.md GitHub PR 工作流](../../../AGENTS.md#github-pr-工作流) 默认原则（小而完整，能 stacked 就 stacked）：

| PR | 范围 | base | LOC 估算 |
|---|---|---|---|
| **PR #1（本 PR）** | 设计文档 + ADR-112 §8 修订 | `adr-112-w3a-compatibility-evaluation` | +200 / -3 |
| **PR #2** | 红测 + BootstrapContext 数据结构（无实现） | PR #1 | +120 / 0 |
| **PR #3** | bootstrap_tools 实现 + 12 个 _internal 重命名 | PR #2 | +150 / -50 |
| **PR #4** | 调用方改造（app/main/agent_loop/worker/testing） | PR #3 | +80 / -150 |

净累积 diff：约 **+550 / -200**（实质重构减少 ~150 行重复代码 + 新增 BootstrapContext 抽象）。

---

## 6. 风险与缓解

| 风险 | 评估 | 缓解 |
|---|---|---|
| BootstrapContext 字段过多（15+ Option） | 🟡 中 | 用 `..Default::default()` 简化构造；按 mode 把不相关字段标 `#[doc(hidden)]` |
| async / sync 混合（message_tools async） | 🟢 低 | bootstrap_tools 整体 async；sync 子方法直接调用 |
| 测试代码 `register_builtin_tools` 改造工作量 | 🟡 中 | 保留私有 `register_builtin_tools_internal`；测试改 `BootstrapContext::for_test()` |
| worker/container.rs 路径独立但仍走 bootstrap_tools | 🟢 低 | mode=Container 显式区分 |
| ADR-112 §8 错误描述被引用 | 🟢 低 | 本 PR 同步修订 |

---

## 7. 决策点（待 review）

请用户确认以下设计决策：

1. **D1 - BootstrapContext vs Builder pattern**：选 struct + `..Default::default()`（简单），不用 builder pattern（避免 13 个 with_xxx 方法）。✅ 推荐
2. **D2 - dynamic API（wasm/builder）保留 pub**：不强行收口到 bootstrap，因为它们语义不同（动态、有返回值、运行时插件）。✅ 推荐
3. **D3 - 测试 convenience**：`BootstrapContext::for_test()` 静态方法 vs 让测试代码也用 full struct。✅ 推荐 for_test
4. **D4 - 调用点目标**：4 文件每个 1 个 bootstrap_tools 调用（合计 4 个），非"所有文件 1 个"（不现实，因为 app.rs / main.rs / agent_loop.rs 是不同初始化阶段）
5. **D5 - PR 拆分**：4 个 stacked PR vs 2 个（设计 + impl 合一）。✅ 推荐 4 个

如有不同意见请在本 PR review 中提出。

---

## 8. Refs

- ADR-112 §5 P0-2
- AGENTS.md 三层验证规范 / TDD 流程 / GitHub PR 工作流
- PR #36 / #39（P0-1）
