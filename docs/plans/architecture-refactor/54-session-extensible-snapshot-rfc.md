# Doc 54 — 可扩展会话快照设计 RFC

> Status: **RFC（评审中）**
> Owner: dasclaw_session 设计组
> Tracks: [#957](https://github.com/Linnanli/xClaw/issues/957)
> Related: [ADR-157](adr-157-headless-agent-framework-capability-boundary.md) §2.3，[ADR-153](adr-153-runtime-cli-dependency-boundary.md) §4.4，[doc 53](53-headless-agent-capability-design.md) §4.3
> Will be promoted to: ADR-158（评审结论确定后）

## 0. 阅读路径

本 RFC 解 ADR-157 §2.3 列出的 **3 个迁移前置条件中的第 1 个**：
> `dasclaw_session` 能持久化桌面端的 `ThreadState` / `Tenant` / `Skills` schema

后续解：

- 前置 2：`dasclaw_runtime` 加可插拔的"3 阶段派发器"（另起 issue）
- 前置 3：审批完全统一到 `ApprovalInbox`（已部分由 B4 推进）

三者全部完成后，方可再次评估"桌面端 `ChatDelegate` 迁 `Agent` 门面"的可行性。

## 1. 背景与差距事实

### 1.1 框架现状

`dasclaw_session` 提供两个核心抽象：

**`SessionSnapshot`**（[crates/dasclaw_session/src/snapshot.rs](../../../crates/dasclaw_session/src/snapshot.rs#L117) L117）—— 9 个字段：

```rust
pub struct SessionSnapshot {
    pub session_id: String,
    pub version: u32,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub workspace_root: Option<PathBuf>,
    pub model: Option<String>,
    pub compaction: Option<SessionCompaction>,
    pub fork: Option<SessionFork>,
    pub prompt_history: Vec<SessionPromptEntry>,
    pub messages: Vec<ChatMessage>,
}
```

**`SessionStore`**（[crates/dasclaw_session/src/store.rs](../../../crates/dasclaw_session/src/store.rs#L28) L28）—— 4 个方法：

```rust
#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn save(&self, snapshot: &SessionSnapshot) -> Result<(), SessionError>;
    async fn load(&self, session_id: &str) -> Result<Option<SessionSnapshot>, SessionError>;
    async fn list(&self) -> Result<Vec<SessionMetadata>, SessionError>;
    async fn delete(&self, session_id: &str) -> Result<(), SessionError>;
}
```

实现：`InMemorySessionStore`、`JsonlSessionStore`（jsonl 落盘）。

### 1.2 桌面端实际维护的状态

| 信息 | 来源 | 框架能装 |
|---|---|---|
| 会话状态机 `ThreadState`（6 态：`Idle / Planning / Processing / AwaitingApproval / Interrupted / Completed`） | [desktop-client/ironclaw/src/agent/thread_ops.rs](../../../desktop-client/ironclaw/src/agent/thread_ops.rs#L215) L215+ | ❌ |
| 多租户 `Tenant`（每 tenant 一份 store / 一组凭证 / 一组配额） | `desktop-client/ironclaw/src/tenant.rs` | ❌ |
| LLM 调用账本（provider / model / input_tokens / output_tokens / cost / purpose / job_id / conversation_id） | [dispatcher.rs L553-567](../../../desktop-client/ironclaw/src/agent/dispatcher.rs#L553) | ❌ |
| 启用的技能 `active_skills` | [dispatcher.rs L361](../../../desktop-client/ironclaw/src/agent/dispatcher.rs#L361) | ❌ |
| 禁用的 MCP 扩展 `disabled_extensions` | [dispatcher.rs L362](../../../desktop-client/ironclaw/src/agent/dispatcher.rs#L362) | ❌ |
| 后台任务 `agent_jobs` 上下文 | JobDelegate / Scheduler | ❌ |
| 待审批工具调用快照（落 `AwaitingApproval`） | thread_ops `AwaitingApproval` 分支 | ❌ |

### 1.3 两条死路

**A. 把这 7 类字段直接加到 `SessionSnapshot` 上** → 违反 [ADR-153 §4.4](adr-153-runtime-cli-dependency-boundary.md) "headless CLI 不依赖 DB / HTTP"，且违反 [ADR-157 §2.1](adr-157-headless-agent-framework-capability-boundary.md) "窄门面"原则。第三方通过 `AgentBuilder::build()` 起 agent 时会被迫吃下租户、SQLite、agent_jobs 表。

**B. 桌面端在框架快照外维护侧表** → schema 拆两半，重启时核心 `messages` 与扩展 `ThreadState` **可能不同步**（核心 save 成功、扩展 save 失败 = 状态机回退、消息已前进），事务一致性丢失。比"不迁"更危险。

## 2. 设计目标与约束

### 2.1 目标

1. 框架接口能让桌面端**用同一份事务**持久化 7 类扩展信息
2. 第三方仅消费核心接口、**不被迫吃下桌面端 schema**
3. 扩展字段升级有清晰的版本管理与不兼容策略
4. 桌面端从"在 SQLite 自维护一整套"逐步降到"实现一个扩展 store trait"，最终解锁 ADR-157 §2.3 迁移评估

### 2.2 硬约束

| 约束 | 来源 |
|---|---|
| 第三方使用核心接口零依赖 DB / HTTP | ADR-153 §4.4 |
| `dasclaw_runtime` 公共面不膨胀 | ADR-157 §2.1 |
| 不破坏现有 `JsonlSessionStore` 落盘格式 | claw-code 兼容性（snapshot.rs 注释明示） |
| 不破坏现有 `Session::new` / `Session::run` 公共签名 | dasclaw_runtime 第三方稳定面 |

### 2.3 非目标（显式排除）

- ❌ 本 RFC 不启动 `ChatDelegate` → `Agent` 迁移（要等本 RFC + 前置 2 + 前置 3 全完成）
- ❌ 本 RFC 不改桌面端任何 SQLite schema
- ❌ 本 RFC 不把 LLM 账本 / agent_jobs 等桌面端业务表抽到框架公共面

## 3. 候选方案

四个候选方案，沿"灵活 ↔ 强类型"光谱排列。

### 3.1 方案 A — `extensions: serde_json::Value` 自定义槽

**思路**：在 `SessionSnapshot` 加一个透明 JSON 槽，框架不感知字段含义，业务方自由读写。

```rust
pub struct SessionSnapshot {
    // ...现有 9 个字段...
    #[serde(default, skip_serializing_if = "is_null_value")]
    pub extensions: serde_json::Value,
}
```

桌面端把 `ThreadState` / `Tenant` / `Skills` / 待审批工具调用等序列化进 `extensions`，框架 `save / load` 原子写整个 snapshot，事务一致性由"single key, single write"保证。

**优点**：
- 改动量极小（snapshot.rs + 一个版本号 bump）
- 完全兼容 jsonl 落盘格式（新字段默认空）
- 第三方零感知

**缺点**：
- 弱类型：扩展 schema 升级靠业务方自行管理 `extensions.version`，没有编译期保护
- `list()` 返回的 `SessionMetadata` 不含 `extensions`，桌面端要按 `ThreadState` 过滤会话（如"只看等审批的"）就必须 `load` 全量
- `extensions` 字段一旦广泛使用，反向把"它该装什么"沉淀成隐性契约，未来想升强类型反而更难

**适用阶段**：短期 unblock（≤ 3 个月）。

---

### 3.2 方案 B — `SessionStore` 拆"核心 + 扩展"两层

**思路**：保持 `SessionStore` 不变，新增一个独立的 `SessionExtensionStore` trait，由业务方实现。框架在 `Session` 上提供"原子写两层"的 API。

```rust
#[async_trait]
pub trait SessionExtensionStore: Send + Sync {
    type Extension: Serialize + DeserializeOwned + Send + Sync;

    async fn save_extension(
        &self,
        session_id: &str,
        extension: &Self::Extension,
    ) -> Result<(), SessionError>;

    async fn load_extension(
        &self,
        session_id: &str,
    ) -> Result<Option<Self::Extension>, SessionError>;
}

impl Session {
    pub async fn save_with_extension<E: SessionExtensionStore>(
        &self,
        core: &dyn SessionStore,
        ext: &E,
        extension_data: &E::Extension,
    ) -> Result<(), SessionError> {
        // 实现层负责事务边界（单 DB 走单事务；jsonl 走临时文件 + rename）
    }
}
```

**优点**：
- 类型安全：桌面端自己定义 `ExtensionData = DesktopSessionExtension { thread_state, tenant_id, active_skills, ... }`，编译期校验
- 第三方完全不接触扩展接口，核心 `SessionStore` 保持窄
- 升级有 trait 版本可循

**缺点**：
- 事务边界设计是难点：当核心 store 是 PostgreSQL、扩展 store 是 SQLite 时，"原子写两层"做不到（需要二阶段提交或 saga）。桌面端实际场景是"两者都在同一个 SQLite"，可走单事务；但要在 trait 文档里说清边界
- 接口面变大（4 个方法 → 4+2 个方法 + 一个新 trait + 一个新关联类型）
- `list()` 仍只返回核心 metadata，扩展过滤还得自己来

**适用阶段**：中长期正式接口（≥ 6 个月）。

---

### 3.3 方案 C — `SessionStore` 关联类型 + `type Extension = ()`

**思路**：把扩展类型升到 `SessionStore` 自身的关联类型上，默认 `()`。

```rust
#[async_trait]
pub trait SessionStore: Send + Sync {
    type Extension: Serialize + DeserializeOwned + Send + Sync + Default;

    async fn save(
        &self,
        snapshot: &SessionSnapshot,
        extension: &Self::Extension,
    ) -> Result<(), SessionError>;

    async fn load(
        &self,
        session_id: &str,
    ) -> Result<Option<(SessionSnapshot, Self::Extension)>, SessionError>;

    // ...
}

// 第三方零成本
impl SessionStore for InMemorySessionStore {
    type Extension = ();
    // ...
}

// 桌面端
impl SessionStore for DesktopSqliteStore {
    type Extension = DesktopSessionExtension;
    // ...
}
```

**优点**：
- 最强类型安全
- 真正"单事务"：核心 + 扩展通过同一个 `save` 调用落库
- `list` 也可以返回扩展元信息子集

**缺点**：
- **泛型在 `Agent` 公共 API 上传播**：`Session<S: SessionStore>` / `Agent::with_session_store::<S>(...)`。第三方要么写 `Session<InMemorySessionStore>`，要么用 `Session<dyn SessionStore<Extension = ()>>`，DX 立刻劣化
- 与现有 `Arc<dyn SessionStore>` 用法**不兼容**：trait 一旦有关联类型，就不能直接 `dyn`，必须 `dyn SessionStore<Extension = T>`，泛型仍要写出来
- 现有所有 `SessionStore` 用法需要修签名（破坏性升级）

**适用阶段**：仅在方案 B 被证伪后再考虑。

---

### 3.4 方案 D — Hybrid：A 先 unblock，B 收尾

**思路**：分两阶段实施。

**阶段 1（A）**：
- `SessionSnapshot.extensions: serde_json::Value` 落地
- 桌面端用它装 7 类信息
- 标注 `#[doc = "Unstable extension slot; subject to migration in #957 phase 2"]`
- 落地后立即开始阶段 2 的设计 PR

**阶段 2（B）**：
- 引入 `SessionExtensionStore` trait + 强类型扩展
- 提供从 `extensions: Value` → `Self::Extension` 的迁移路径
- 桌面端切到强类型；`extensions` 字段降级为"legacy fallback"
- 6~12 个月后移除 legacy fallback

**优点**：
- 解锁桌面端进展无需等设计周期
- 强类型有明确退场计划，不会被 weak slot 长期绑架

**缺点**：
- 两次破坏性变更（一次加 `extensions`、一次迁到 trait），桌面端要适配两轮
- 阶段 2 一旦拖延，A 就事实上长期化

**适用阶段**：项目当前进度（W3-A Phase 0 进行中、桌面端持续演进）的合理折中。

## 4. 推荐：方案 D（Hybrid）

理由按权重排：

1. **当前没有桌面端真在等迁移**：ADR-157 §2.3 明确"短期不迁"。所以阶段 1 选 A 不会被立刻"长期化"——阶段 2 的真实压力在 6+ 个月后才出现。
2. **方案 B 的事务边界设计需要时间**：跨 store 原子写要画清"同源 store 可单事务 / 异源 store 需 saga"的契约，仓促设计会埋坑。先用阶段 1 在生产环境验证"桌面端到底用 `extensions` 装哪些字段、多大、查询模式怎样"，再用真数据驱动阶段 2 的 trait 设计。
3. **方案 C 的泛型传播代价不可接受**：`Agent`/`Session` 公共 API 是第三方稳定面，加关联类型会破坏所有 `Arc<dyn SessionStore>` 现有用法。
4. **C 不解决 list 过滤问题**：即使强类型，桌面端"列出所有等审批的会话"仍要遍历，与 A/B 同等开销。

### 4.1 阶段 1 落地范围（次个 PR）

1. 在 `SessionSnapshot` 加 `extensions: serde_json::Value` 字段（带 `#[serde(default)]` 保旧 jsonl 兼容）
2. 加 `SessionSnapshot::extensions::<T: DeserializeOwned>() -> Option<T>` 与 `set_extensions<T: Serialize>(&mut self, value: &T)` 辅助方法
3. 文档明示"Unstable，#957 阶段 2 会替换"
4. 单元测试覆盖：默认空、写入读出对称、jsonl 落盘旧格式向后兼容
5. **不**改桌面端任何代码（桌面端切换由独立 PR 完成）

### 4.2 阶段 2 启动条件

- 桌面端有至少 1 个版本真用 `extensions` 装 ≥ 3 类信息（≥ 3 个月生产数据）
- 出现 ≥ 1 个具体的"weak slot 坑了我们"事件（schema 漂移 / 升级失败 / 多业务方冲突）
- 才正式提 `SessionExtensionStore` trait 设计 PR

## 5. 反对意见预案

| 反对 | 回应 |
|---|---|
| "方案 A 的 weak slot 永远不会被强类型替代" | DoD 明确"6~12 个月后移除 legacy fallback"，且阶段 2 启动条件硬性卡 ≥ 3 类信息使用 |
| "应该一步到位选 B" | 没有桌面端真在等迁，无紧迫性；先用阶段 1 收集真实需求 |
| "为什么不加在 `SessionMetadata` 上让 `list()` 也能查？" | metadata 是"轻量索引"语义，让它装业务字段会反向污染所有 store 实现的 list 路径 |
| "为什么不让桌面端继续维护侧表？" | §1.3 已给出反对理由：双 schema 重启不同步 |

## 6. 迁移路径与 PR 切片

| 顺序 | PR 边界 | 改动估算 | 依赖 |
|---|---|---|---|
| 1 | 本 RFC 合入（doc 54） | 文档 | 无 |
| 2 | RFC 评审通过后升 ADR-158 | 文档 | PR 1 |
| 3 | 阶段 1 实现：`SessionSnapshot.extensions` + 辅助方法 + 单元测试 | `crates/dasclaw_session/src/snapshot.rs` 约 80 行；2 个单元测试 | PR 2 |
| 4 | 桌面端 PoC：用 `extensions` 装 `ThreadState`（最小一类） | desktop-client 约 50 行 + 集成测试 | PR 3 |
| 5 | 桌面端逐步迁入 Tenant / Skills / agent_jobs | 多 PR | PR 4 |
| 6 | 阶段 2 启动：`SessionExtensionStore` trait 设计 PR | 文档 + trait 草案 | 阶段 1 ≥ 3 月生产 |

## 7. 验收（本 RFC 的 DoD）

- [x] 列清当前框架与桌面端的差距事实
- [x] 给出 ≥ 3 个候选方案，每个含代码草图、优缺点、适用阶段
- [x] 给出推荐方案与理由
- [x] 列清反对意见预案
- [x] 列清迁移 PR 切片
- [ ] **评审通过**（本 PR merge 即视为初评通过）
- [ ] **升 ADR-158**（独立 PR）

## 8. Sources read

- [docs/plans/architecture-refactor/adr-157-headless-agent-framework-capability-boundary.md](adr-157-headless-agent-framework-capability-boundary.md) §2.3 / §2.1
- [docs/plans/architecture-refactor/adr-153-runtime-cli-dependency-boundary.md](adr-153-runtime-cli-dependency-boundary.md) §4.4
- [docs/plans/architecture-refactor/53-headless-agent-capability-design.md](53-headless-agent-capability-design.md) §4.3
- [crates/dasclaw_session/src/snapshot.rs](../../../crates/dasclaw_session/src/snapshot.rs) L80-175
- [crates/dasclaw_session/src/store.rs](../../../crates/dasclaw_session/src/store.rs) L1-130
- [crates/dasclaw_session/src/lib.rs](../../../crates/dasclaw_session/src/lib.rs) L94-240
- [desktop-client/ironclaw/src/agent/dispatcher.rs](../../../desktop-client/ironclaw/src/agent/dispatcher.rs) L354-600
- [desktop-client/ironclaw/src/agent/thread_ops.rs](../../../desktop-client/ironclaw/src/agent/thread_ops.rs) L215-2460

## 9. Cross-cuts

无新增 `.ironclaw` / `IRONCLAW_BASE_DIR` 字面量。
