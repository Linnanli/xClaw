# IronClaw 嵌入式集成可行性分析报告

> 生成日期：2026-03-21
> 背景：评估将 IronClaw 作为核心库嵌入 Admin-Backend，替代当前 HTTP 代理方案的可行性和改造成本。

## 一、当前架构

```
Admin-Backend ──HTTP 代理──→ IronClaw Web Gateway (:38080)
                              │
                              ├── GET /api/skills
                              └── GET /api/extensions
```

Admin-Backend 通过 `reqwest` HTTP 客户端代理请求到 IronClaw Gateway，Gateway 不可用时回退到本地数据库。

## 二、Desktop-Client 的现状（重要发现）

Desktop-Client 在 `Cargo.toml` 中依赖了 IronClaw：

```toml
# desktop-client/Cargo.toml
ironclaw = { path = "../ironclaw", features = ["libsql"] }
```

**但实际上并没有在进程内运行 IronClaw 服务器**：
- `embedded_server.rs` 只是一个 HTTP 健康检查客户端，连接外部 IronClaw 实例（端口 38080）
- `main.rs` 注释明确写道："Desktop Client 使用外部 IronClaw 实例，不内嵌服务器"
- 如果 IronClaw 服务器未运行，Desktop Client 会打印启动指引并继续运行（聊天功能不可用）
- 只有测试代码（`tests/support/test_server.rs`）才在进程内创建 `GatewayState`

**这意味着**：Desktop-Client 虽然编译依赖了 ironclaw crate（主要用于类型共享和测试），但运行时仍然依赖外部 IronClaw 服务。这与 Admin-Backend 的 HTTP 代理模式本质相同。

**Desktop-Client 的 ironclaw 依赖带来的问题**：
- 编译时间长（需要编译 ironclaw 的 ~80+ 依赖）
- 二进制大小大（包含了 wasmtime、bollard 等不需要的代码）
- 实际只用到了类型定义和 `ironclaw_safety` crate

## 三、IronClaw 的依赖重量

### 核心依赖（~80+ crates）

| 类别 | 依赖 | 说明 |
|------|------|------|
| 异步运行时 | tokio, futures, tokio-stream | 必需 |
| HTTP | reqwest, axum, tower, hyper | 必需 |
| 序列化 | serde, serde_json, toml | 必需 |
| 数据库 | deadpool-postgres, tokio-postgres, libsql | 与 admin-backend 冲突 |
| CLI | clap, crossterm, rustyline, termimad | 不需要 |
| WASM 沙箱 | wasmtime, wasmtime-wasi, wasmparser | 重量级，不需要 |
| Docker | bollard | 不需要 |
| 加密 | aes-gcm, hkdf, hmac, sha2, blake3, rand | 部分需要 |
| LLM | rig-core | 不需要 |
| AWS | aws-config, aws-sdk-bedrockruntime | 不需要 |
| 文档提取 | pdf-extract, zip | 不需要 |
| 代理 | hyper, hyper-util, http-body-util | 不需要 |
| 平台 | security-framework (macOS), secret-service (Linux) | 不需要 |

### 编译影响估算

| 指标 | 当前 admin-backend | 嵌入 ironclaw 后 |
|------|-------------------|-----------------|
| 依赖数量 | ~30 crates | ~110+ crates |
| 编译时间（增量） | ~10-20s | ~60-120s |
| 编译时间（全量） | ~1-2min | ~5-10min |
| 二进制大小 | ~15-20MB | ~80-120MB |
| 内存占用 | ~50MB | ~200-500MB |

## 四、嵌入方案分析

### 方案 A：完整嵌入（像 Desktop-Client 一样）

```rust
// admin-backend/Cargo.toml
ironclaw = { path = "../ironclaw", features = ["libsql"] }
```

#### 需要做的事情

1. **初始化 IronClaw Config**
   - `Config::from_env()` 或 `Config::for_testing()`
   - 需要配置 LLM、数据库、安全等 ~20 个子配置

2. **创建 GatewayState**（~25 个字段）
   ```rust
   GatewayState {
       msg_tx, sse, workspace, session_manager,
       log_broadcaster, log_level_handle,
       extension_manager,  // ← 我们需要这个
       tool_registry,
       store,              // ← 需要 Arc<dyn Database>
       job_manager, prompt_queue, user_id,
       shutdown_tx, ws_tracker, llm_provider,
       skill_registry,     // ← 我们需要这个
       skill_catalog,      // ← 我们需要这个
       scheduler, chat_rate_limiter, oauth_rate_limiter,
       registry_entries, cost_guard, routine_engine,
       startup_time,
   }
   ```

3. **数据库层冲突**
   - Admin-Backend 使用 `deadpool-postgres` + `tokio-postgres`（直接连接）
   - IronClaw 使用 `Arc<dyn Database>` 抽象（支持 postgres 和 libsql）
   - 需要维护两套数据库连接池，或实现适配器

4. **直接调用内部 API**
   ```rust
   // 替代 HTTP 代理
   let skills = state.skill_registry.list_skills();
   let extensions = state.extension_manager.list_installed();
   ```

#### 优点
- 零网络延迟
- 不依赖 IronClaw Gateway 是否运行
- 可以访问所有内部 API

#### 缺点
- **编译时间增加 3-5 倍**
- **二进制大小增加 4-6 倍**
- **引入 ~80 个不需要的依赖**（wasmtime、bollard、clap、pdf-extract 等）
- **GatewayState 初始化复杂**（25 个字段，大部分不需要）
- **数据库层冲突**（两套连接池）
- **版本耦合**：IronClaw 更新可能破坏 admin-backend
- **IronClaw 没有 feature gate 来排除不需要的模块**

#### 改造工作量：🔴 大（2-3 周）

---

### 方案 B：选择性嵌入（提取共享 Crate）

将 IronClaw 中 Admin-Backend 需要的能力提取为独立的共享 crate。

```
crates/
├── ironclaw_auth/     # ✅ 已存在
├── ironclaw_safety/   # ✅ 已存在
├── ironclaw_skills/   # 🆕 提取 Skills 类型和解析器
└── ironclaw_extensions/ # 🆕 提取 Extensions 类型和注册表
```

#### 需要提取的内容

**ironclaw_skills crate**（从 `ironclaw/src/skills/` 提取）：
- `SkillManifest`、`SkillTrust`、`SkillSource`、`LoadedSkill` 类型
- `SkillRegistry`（技能注册表）
- `parser`（SKILL.md 解析器）
- 依赖：`serde`、`regex`、`serde_yml`、`sha2`

**ironclaw_extensions crate**（从 `ironclaw/src/extensions/` 提取）：
- `ExtensionKind`、`RegistryEntry`、`InstalledExtension` 类型
- `ExtensionRegistry`（内置注册表）
- 依赖：`serde`、`serde_json`

#### 优点
- 最小依赖引入
- 编译时间几乎不增加
- 类型安全（共享类型定义）
- 不需要运行 IronClaw Gateway

#### 缺点
- **需要 IronClaw 项目配合重构**（将模块提取为独立 crate）
- 提取后需要维护 crate 的 API 稳定性
- Skills 和 Extensions 的运行时状态（已安装列表、激活状态）仍然在 IronClaw 进程中

#### 改造工作量：🟡 中（1-2 周，需要修改 IronClaw 项目结构）

---

### 方案 C：优化 HTTP 代理（当前方案的改进）

保持 HTTP 代理架构，但优化实现。

```
Admin-Backend ──HTTP 代理──→ IronClaw Web Gateway
     │
     ├── 缓存层（减少请求频率）
     ├── 完整字段映射（不丢失信息）
     ├── 健康检查（快速检测 Gateway 状态）
     └── 删除冗余的本地 DB 表
```

#### 具体优化

1. **删除冗余表**
   - 删除 `skills` 表和 `plugins` 表
   - Gateway 不可用时返回空列表 + 状态提示

2. **完善字段映射**
   ```rust
   // 当前：丢失了 kind、authenticated、active、tools 等字段
   // 优化后：完整透传 IronClaw 的响应
   async fn get_plugins(State(state): State<AppState>) -> Result<Json<Value>> {
       let resp = state.http_client
           .get(format!("{}/api/extensions", state.gateway_url))
           .send().await?;
       // 直接透传，不做字段映射
       Ok(Json(resp.json().await?))
   }
   ```

3. **添加缓存层**
   ```rust
   // 缓存 skills 和 extensions 列表（TTL 30s）
   pub struct CachedGatewayProxy {
       http_client: reqwest::Client,
       gateway_url: String,
       cache: Arc<Mutex<HashMap<String, (Instant, Value)>>>,
       ttl: Duration,
   }
   ```

4. **健康检查**
   ```rust
   // 定期检查 Gateway 状态，前端展示连接状态
   .route("/api/gateway/health", get(check_gateway_health))
   ```

#### 优点
- **零改造成本**（在现有代码上优化）
- 编译时间不变
- 架构清晰（管理面 vs 执行面分离）
- IronClaw 更新不影响 admin-backend

#### 缺点
- 依赖 IronClaw Gateway 运行
- 网络延迟（本地回环，<1ms，可忽略）
- Gateway 不可用时功能降级

#### 改造工作量：🟢 小（2-3 天）

## 五、三种方案对比

| 维度 | 方案 A：完整嵌入 | 方案 B：选择性嵌入 | 方案 C：优化代理 |
|------|----------------|------------------|----------------|
| 改造工作量 | 🔴 2-3 周 | 🟡 1-2 周 | 🟢 2-3 天 |
| 编译时间影响 | 🔴 增加 3-5 倍 | 🟢 几乎不变 | 🟢 不变 |
| 二进制大小 | 🔴 增加 4-6 倍 | 🟢 增加 <5% | 🟢 不变 |
| 依赖复杂度 | 🔴 ~110+ crates | 🟢 增加 2-3 crates | 🟢 不变 |
| 网络依赖 | 🟢 无 | 🟡 部分（运行时状态） | 🟡 依赖 Gateway |
| 类型安全 | 🟢 完全 | 🟢 共享类型 | 🟡 JSON 映射 |
| 维护成本 | 🔴 高（版本耦合） | 🟡 中（crate 维护） | 🟢 低 |
| IronClaw 改动 | 🟢 无需改动 | 🔴 需要重构 | 🟢 无需改动 |
| 数据库冲突 | 🔴 两套连接池 | 🟢 无冲突 | 🟢 无冲突 |

## 六、推荐方案

### 推荐：方案 C（优化 HTTP 代理）+ 方案 B 的类型共享

**理由**：

1. **Admin-Backend 只需要 Skills 和 Extensions 的"列表展示"功能**
   - 不需要安装/卸载/激活等写操作
   - 不需要运行 AI 代理循环
   - HTTP 代理完全满足需求

2. **完整嵌入的代价远大于收益**
   - 引入 ~80 个不需要的依赖（wasmtime、bollard、clap 等）
   - 编译时间和二进制大小大幅增加
   - GatewayState 初始化复杂度高
   - 数据库层冲突需要额外适配

3. **架构分离是正确的设计**
   - Admin-Backend = 管理面（配置、审计、RBAC）
   - IronClaw = 执行面（AI 代理、工具、扩展）
   - 两者通过 API 通信是微服务最佳实践

4. **渐进式改进路径**
   - 短期：方案 C，优化代理（2-3 天）
   - 中期：提取 `ironclaw_skills` 和 `ironclaw_extensions` 类型 crate（方案 B 的轻量版）
   - 长期：如果需要更深度集成，再考虑完整嵌入

### 推荐的实施步骤

#### 第一阶段（短期，2-3 天）

1. 删除冗余的 `skills` 和 `plugins` 数据库表
2. 优化 `/api/skills` 和 `/api/plugins` 端点，完整透传 IronClaw 响应
3. 添加 Gateway 健康检查端点
4. 前端适配 IronClaw 的完整字段

#### 第二阶段（中期，可选）

5. 提取 `ironclaw_skills` 类型 crate（仅类型定义 + 解析器，不含运行时）
6. 提取 `ironclaw_extensions` 类型 crate（仅类型定义 + 注册表）
7. Admin-Backend 使用共享类型进行反序列化，替代 `serde_json::Value`

#### 第三阶段（长期，按需）

8. 如果需要离线管理（Gateway 不可用时仍能管理 Skills/Extensions）
9. 考虑嵌入 SkillRegistry 和 ExtensionRegistry 的只读副本
10. 通过共享 crate 实现，不需要完整嵌入 IronClaw

## 七、关键结论

| 问题 | 回答 |
|------|------|
| 嵌入改造大吗？ | **大**。完整嵌入需要 2-3 周，引入 ~80 个不需要的依赖 |
| 需要修改 IronClaw 代码吗？ | **方案 A 不需要，方案 B 需要**（见下方详细分析） |
| 值得嵌入吗？ | **不值得**。Admin-Backend 只需要列表展示，HTTP 代理足够 |
| Desktop-Client 嵌入了吗？ | **没有真正嵌入**。虽然编译依赖了 ironclaw crate，但运行时连接外部服务 |
| 客户端启动 IronClaw 太重了吗？ | **是的，太重了**（见下方详细分析） |
| 最佳方案是什么？ | 优化 HTTP 代理 + 渐进式类型共享 |

### 问题 1：是否需要修改 IronClaw 代码？

| 方案 | 是否需要修改 IronClaw | 说明 |
|------|---------------------|------|
| 方案 A：完整嵌入 | ❌ 不需要 | 直接依赖 ironclaw crate，但会引入所有不需要的依赖 |
| 方案 B：选择性嵌入 | ✅ 需要 | 需要将 `skills/` 和 `extensions/` 模块提取为独立 crate |
| 方案 C：优化代理 | ❌ 不需要 | 只修改 admin-backend 代码 |

**方案 B 需要的 IronClaw 改动**：
- 将 `ironclaw/src/skills/` 中的类型定义提取到 `ironclaw/crates/ironclaw_skills/`
- 将 `ironclaw/src/extensions/` 中的类型定义提取到 `ironclaw/crates/ironclaw_extensions/`
- 修改 `ironclaw/Cargo.toml` 添加 workspace 成员
- 修改 `ironclaw/src/skills/mod.rs` 改为 `pub use ironclaw_skills::*`
- 这是一个**非破坏性重构**，IronClaw 的外部 API 不变

**如果不想修改 IronClaw**，方案 C 是最佳选择。

### 问题 2：客户端启动 IronClaw 服务是否太重？

**答案：是的，太重了。**

IronClaw 作为完整服务启动时需要：

| 资源 | 消耗 |
|------|------|
| 内存 | ~200-500MB（WASM 运行时、LLM 上下文、数据库连接池） |
| CPU | 启动时高（编译 WASM、初始化扩展），运行时中等 |
| 磁盘 | ~100MB+（数据库、WASM 模块、技能文件） |
| 启动时间 | ~5-15 秒（数据库迁移、扩展加载、WASM 编译） |
| 端口 | 38080（Web Gateway） |
| 进程 | 独立进程，需要单独管理生命周期 |

**当前 Desktop-Client 的做法**：
- 要求用户手动启动 IronClaw 服务（`cargo run -- run --no-onboard`）
- 如果服务未运行，聊天功能不可用，但其他功能正常
- 这说明即使 Desktop-Client 团队也认为嵌入运行太重，选择了外部服务模式

**更轻量的替代方案**：

1. **按需启动**：Desktop-Client 在需要时自动启动 IronClaw 子进程
   ```rust
   // 在后台启动 IronClaw 服务
   let child = Command::new("ironclaw")
       .args(["run", "--no-onboard", "--cli-only"])
       .spawn()?;
   ```

2. **懒加载**：只在用户首次使用聊天功能时启动

3. **精简模式**：IronClaw 添加 `--minimal` 模式，只启动必要的组件
   - 禁用 WASM 沙箱
   - 禁用 Docker 沙箱
   - 禁用不需要的通道
   - 减少内存占用到 ~50-100MB

4. **微服务拆分**（长期）：将 IronClaw 拆分为核心服务 + 可选插件
   - 核心：聊天、内存、设置（轻量）
   - 插件：WASM、Docker、扩展（按需加载）

## 八、最终推荐：方案 B（选择性嵌入）是架构最优解

> 抛开开发时间，从架构合理性和长期维护角度分析。

### 为什么方案 B 最好？

#### 1. 类型安全 > JSON 映射

方案 C（HTTP 代理）的核心问题是**类型不安全**：

```rust
// 方案 C：JSON 映射，编译器无法帮你检查
let resp: serde_json::Value = client.get("/api/extensions").send().await?.json().await?;
let name = resp["name"].as_str().unwrap_or(""); // 运行时才发现字段名错了

// 方案 B：共享类型，编译期检查
use ironclaw_extensions::InstalledExtension;
let ext: InstalledExtension = client.get("/api/extensions").send().await?.json().await?;
let name = &ext.name; // 编译期保证字段存在
```

IronClaw 的 API 响应结构会随版本演进。方案 C 下，IronClaw 改了字段名，admin-backend 编译照过，运行时才崩。方案 B 下，共享类型变了，编译直接报错。

#### 2. 单一事实来源（Single Source of Truth）

方案 B 让类型定义只存在一个地方：

```
ironclaw/crates/ironclaw_skills/src/lib.rs    ← 唯一定义
  ├── ironclaw 主项目 use
  ├── admin-backend use
  └── desktop-client use
```

方案 C 下，admin-backend 需要自己维护一套类型定义（或用 `serde_json::Value`），与 IronClaw 的类型定义是**两份独立的代码**，容易不同步。

#### 3. 依赖最小化

方案 B 提取的 crate 非常轻量：

```toml
# ironclaw_skills 的依赖（估算）
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_yml = "0.0.12"
regex = "1"
sha2 = "0.10"
# 总共 ~4 个依赖，编译时间 <5s
```

对比方案 A 的 ~80+ 依赖，方案 B 只引入真正需要的东西。

#### 4. 解耦但不割裂

方案 C 的问题是**过度解耦**：admin-backend 和 IronClaw 之间只有 HTTP 接口，没有任何编译期约束。接口变了，只能靠人工检查或集成测试发现。

方案 B 通过共享 crate 建立了**恰到好处的耦合**：
- 类型定义共享 → 编译期保证一致性
- 运行时逻辑独立 → 不引入不需要的依赖
- API 可以继续用 HTTP → 架构不变

#### 5. 为未来的"策略下发"铺路

中期规划中，admin-backend 需要将 DLP 规则推送到 IronClaw 客户端。如果有共享的类型 crate：

```rust
// 共享类型：admin-backend 和 desktop-client 都用同一个 DlpRule 结构
use ironclaw_safety::PolicyRule;

// admin-backend 序列化
let rules: Vec<PolicyRule> = db.get_dlp_rules().await?;
let json = serde_json::to_string(&rules)?;

// desktop-client 反序列化
let rules: Vec<PolicyRule> = serde_json::from_str(&json)?;
safety_layer.load_custom_rules(rules);
```

没有共享类型，策略下发就需要手动维护两端的数据结构一致性。

### 方案 B 的具体实施路径

```
阶段 1：提取类型 crate（不改变运行时架构）
├── 创建 ironclaw/crates/ironclaw_skills/（类型 + 解析器）
├── 创建 ironclaw/crates/ironclaw_extensions/（类型 + 注册表）
├── IronClaw 主项目改为 use 这些 crate
└── 验证 IronClaw 编译通过，行为不变

阶段 2：Admin-Backend 使用共享类型
├── admin-backend 依赖 ironclaw_skills 和 ironclaw_extensions
├── 替换 serde_json::Value 为强类型
├── 保持 HTTP 代理架构不变
└── 删除冗余的本地 DB 表

阶段 3：Desktop-Client 优化（可选）
├── desktop-client 改为依赖共享 crate 而非完整 ironclaw
├── 去掉对 ironclaw 主 crate 的依赖
└── 编译时间和二进制大小大幅减少
```

### 需要修改 IronClaw 的范围

| 改动 | 影响 | 风险 |
|------|------|------|
| 创建 `crates/ironclaw_skills/` | 新增文件 | 无 |
| 创建 `crates/ironclaw_extensions/` | 新增文件 | 无 |
| 修改 `ironclaw/Cargo.toml` workspace | 添加成员 | 低 |
| 修改 `ironclaw/src/skills/mod.rs` | `pub use ironclaw_skills::*` | 低（非破坏性） |
| 修改 `ironclaw/src/extensions/mod.rs` | `pub use ironclaw_extensions::*` | 低（非破坏性） |

**这是一个纯粹的重构**，不改变任何外部行为。IronClaw 的所有 API、测试、功能都不受影响。

### 三种方案的长期维护成本对比

| 维度 | 方案 A | 方案 B | 方案 C |
|------|--------|--------|--------|
| 类型安全 | ✅ 编译期 | ✅ 编译期 | ❌ 运行时 |
| IronClaw 升级影响 | 🔴 高（全量依赖） | 🟢 低（仅类型 crate） | 🟡 中（JSON 可能变） |
| 接口不一致风险 | 🟢 无 | 🟢 无 | 🔴 高 |
| 编译时间 | 🔴 长 | 🟢 短 | 🟢 短 |
| 新功能集成成本 | 🟢 低（直接调用） | 🟡 中（可能需要扩展 crate） | 🟡 中（需要映射新字段） |
| 团队协作成本 | 🔴 高（深度耦合） | 🟢 低（接口清晰） | 🟡 中（隐式契约） |

### 结论

**方案 B 是"正确的难"**：前期需要一次性的 IronClaw 重构投入，但换来的是长期的类型安全、维护简单、架构清晰。

**方案 C 是"简单的错"**：短期最快，但随着功能增加，JSON 映射的维护成本会持续累积，接口不一致的 bug 会越来越多。

**方案 A 是"过度的对"**：理论上最完整，但实际引入了大量不需要的复杂度。

## 九、IronClaw 能力分类：客户端 vs 管理端适用性分析

### 完整能力清单

#### 🖥️ 纯客户端能力（只在 Desktop-Client 使用）

这些能力是 AI 代理的核心运行时功能，管理端不需要。

| 能力模块 | API | 说明 | 管理端需要？ |
|---------|-----|------|------------|
| **聊天** | `/api/chat/send`, `/api/chat/events`, `/api/chat/ws` | AI 对话、SSE 事件流、WebSocket | ❌ |
| **聊天历史** | `/api/chat/history`, `/api/chat/threads` | 对话历史、线程管理 | ❌ |
| **任务执行** | `/api/jobs/*` | 任务列表、详情、取消、重启、文件 | ❌ |
| **内存/工作区** | `/api/memory/*` | 文档树、读写、搜索 | ❌ |
| **审批** | `/api/chat/approval` | 工具调用审批 | ❌ |
| **配对** | `/api/pairing/*` | 通道配对（Telegram 等） | ❌ |
| **LLM 推理** | `agent`, `llm`, `context` 模块 | AI 推理、上下文管理 | ❌ |
| **工具执行** | `tools`, `sandbox` 模块 | MCP/WASM 工具执行、Docker 沙箱 | ❌ |
| **自修复** | `worker`, `orchestrator` 模块 | 卡住任务检测、工具修复 | ❌ |
| **文档提取** | `document_extraction` 模块 | PDF/ZIP 文本提取 | ❌ |
| **转录** | `transcription` 模块 | 语音转文字 | ❌ |
| **隧道** | `tunnel` 模块 | 网络隧道 | ❌ |

#### 🔧 纯管理端能力（只在 Admin-Backend 使用）

这些能力是 Admin-Backend 独有的，IronClaw 没有。

| 能力模块 | 说明 |
|---------|------|
| **用户管理 + RBAC** | 多用户、角色、权限 |
| **DLP 规则 CRUD** | 动态安全规则管理 |
| **字典管理** | 关键词词库 CRUD |
| **敏感操作管理** | 操作审批规则 |
| **客户端管理** | 客户端注册、心跳、在线状态 |
| **策略版本控制** | 策略变更记录、版本号 |
| **审计日志系统** | 操作级审计（比 IronClaw 的对话级更细） |

#### 🔄 两端都需要的能力（共享）

| 能力模块 | IronClaw API | 客户端用途 | 管理端用途 | 共享方式 |
|---------|-------------|-----------|-----------|---------|
| **技能管理** | `/api/skills` | 安装/卸载/使用技能 | 查看已安装技能、审计 | 代理 or 共享 crate |
| **扩展管理** | `/api/extensions` | 安装/激活/使用扩展 | 查看已安装扩展、审计 | 代理 or 共享 crate |
| **例程管理** | `/api/routines` | 创建/触发/管理例程 | 查看例程列表、审计 | 代理 or 共享 crate |
| **设置管理** | `/api/settings` | AI 运行时配置 | 查看/修改 AI 配置 | 代理 or 共享 crate |
| **日志** | `/api/logs/events` | 查看运行日志 | 集中日志监控 | 代理 |
| **安全层** | `ironclaw_safety` crate | 运行时输入/输出检查 | DLP 规则的执行引擎 | ✅ 已有共享 crate |
| **认证** | `ironclaw_auth` crate | 密码哈希、JWT | 密码哈希、JWT | ✅ 已有共享 crate |

### 管理端需要的 IronClaw 能力详细分析

| 能力 | 管理端需要的操作 | 需要的深度 | 推荐方式 |
|------|----------------|-----------|---------|
| **Skills 列表** | 只读（列表、详情） | 浅（类型 + API） | HTTP 代理 + 共享类型 |
| **Skills 安装/卸载** | 写操作 | 深（文件系统操作） | HTTP 代理（让 IronClaw 执行） |
| **Extensions 列表** | 只读（列表、状态） | 浅（类型 + API） | HTTP 代理 + 共享类型 |
| **Extensions 安装/激活** | 写操作 | 深（WASM/OAuth） | HTTP 代理（让 IronClaw 执行） |
| **Routines 列表** | 只读 + 触发 | 中 | HTTP 代理 |
| **Settings 读写** | 读写 | 浅（key-value） | HTTP 代理 or 直接 DB |
| **Logs 监控** | 只读（SSE 流） | 浅 | HTTP 代理（SSE 转发） |
| **安全规则执行** | 规则下发 | 中 | 共享 crate + API |

### 关键洞察

1. **管理端对 IronClaw 的需求主要是"只读"**
   - 列表展示：Skills、Extensions、Routines、Settings
   - 日志监控：Logs SSE
   - 少量写操作：Settings 修改、Skills 安装（可通过 API 代理）

2. **管理端不需要 IronClaw 的"重量级"能力**
   - 不需要 LLM 推理（agent、llm 模块）
   - 不需要工具执行（tools、sandbox 模块）
   - 不需要 WASM 运行时（wasmtime）
   - 不需要 Docker 沙箱（bollard）
   - 不需要文档提取（pdf-extract）

3. **管理端的独有能力 IronClaw 没有**
   - 多用户 RBAC
   - DLP 规则动态管理
   - 客户端管理
   - 策略版本控制
   - 操作级审计

### 结论

**Admin-Backend 对 IronClaw 的依赖是"薄层"的**：主要是类型定义和 API 代理，不需要嵌入 IronClaw 的核心运行时。

**推荐架构**：

```
┌─────────────────────────────────────────────────────┐
│              Admin-Backend（管理面）                   │
│                                                     │
│  独有能力：RBAC、DLP、审计、客户端管理、策略版本        │
│                                                     │
│  共享 Crate：                                        │
│  ├── ironclaw_auth（密码、JWT）     ← 已有           │
│  ├── ironclaw_safety（安全规则）    ← 已有           │
│  ├── ironclaw_skills（类型定义）    ← 建议提取        │
│  └── ironclaw_extensions（类型定义）← 建议提取        │
│                                                     │
│  HTTP 代理（IronClaw 运行时状态）：                    │
│  ├── GET /api/skills              ← 只读列表         │
│  ├── GET /api/extensions          ← 只读列表         │
│  ├── GET /api/routines            ← 只读列表         │
│  ├── GET/PUT /api/settings        ← 读写配置         │
│  └── GET /api/logs/events         ← 日志监控         │
└──────────────────────┬──────────────────────────────┘
                       │ HTTP（只读为主）
                       ▼
┌─────────────────────────────────────────────────────┐
│         IronClaw Web Gateway（执行面）                │
│                                                     │
│  重量级能力（管理端不需要）：                           │
│  ├── LLM 推理（agent、llm）                          │
│  ├── 工具执行（tools、sandbox）                       │
│  ├── WASM 运行时（wasmtime）                         │
│  ├── Docker 沙箱（bollard）                          │
│  ├── 文档提取（pdf-extract）                         │
│  └── 聊天/任务/内存（chat、jobs、memory）             │
└─────────────────────────────────────────────────────┘
```

## 十、共享能力深度分析：自己实现的难度和架构合理性

### 1. Skills（技能管理）

**IronClaw 实现了什么**：
- `SkillRegistry`：扫描文件系统（`~/.ironclaw/skills/` 和 `workspace/skills/`），加载 SKILL.md 文件
- `parser`：解析 YAML 前置元数据 + Markdown 提示词
- 安装/卸载：写入/删除文件系统中的 SKILL.md 文件
- 信任模型：Installed（只读工具）vs Trusted（完全工具访问）
- 激活选择器：根据用户消息关键词匹配激活哪些技能

**管理端需要什么**：
- 列出所有已安装技能（名称、版本、描述、信任级别、来源）
- 可能需要：远程安装/卸载技能（通过 API 代理）

**自己实现的难度**：🟢 低
- 核心就是文件系统扫描 + YAML 解析，~200 行代码
- 但需要知道 IronClaw 的 skills 目录在哪里
- 信任模型和激活选择器管理端不需要

**是否值得自己实现**：❌ 不值得
- 管理端不运行在 IronClaw 的机器上，无法直接扫描文件系统
- 必须通过 API 获取，HTTP 代理是唯一合理方式

---

### 2. Extensions（扩展管理）

**IronClaw 实现了什么**：
- `ExtensionManager`：~7000 行代码，极其复杂
  - MCP Server 管理（OAuth 2.1、DCR、HTTP 传输）
  - WASM Tool 管理（下载、编译、沙箱执行、能力认证）
  - WASM Channel 管理（热激活、运行时状态）
  - Channel Relay 管理（Slack/Telegram 通过外部服务）
  - Telegram 所有者绑定（验证码流程）
  - OAuth 流程管理（授权 URL、回调、token 刷新）
  - 扩展升级（版本比较、重新安装）
- `ExtensionRegistry`：内置注册表（已知扩展列表）
- `OnlineDiscovery`：在线发现新扩展

**管理端需要什么**：
- 列出所有已安装扩展（名称、类型、状态、工具列表）
- 可能需要：远程安装/卸载扩展

**自己实现的难度**：🔴 极高
- ExtensionManager 有 7000+ 行代码，涉及 OAuth、WASM、Docker、文件系统
- 管理端不需要这些运行时能力，只需要列表展示

**是否值得自己实现**：❌ 绝对不值得
- 管理端只需要"只读列表"，不需要安装/激活/认证
- HTTP 代理是唯一合理方式

---

### 3. Routines（例程管理）

**IronClaw 实现了什么**：
- `RoutineEngine`：定时任务引擎
  - Cron 调度（基于 `cron` crate）
  - 事件驱动触发（文件变更、消息匹配）
  - 任务执行（调用 LLM + 工具）
  - 运行历史记录
- 依赖：`Database`、`LlmProvider`、`Workspace`、`ToolRegistry`、`SafetyLayer`

**管理端需要什么**：
- 列出所有例程（名称、调度规则、状态）
- 查看运行历史
- 可能需要：手动触发、启停

**自己实现的难度**：🔴 极高
- RoutineEngine 依赖 LLM、工具注册表、工作区等核心组件
- 管理端不需要执行例程，只需要查看和控制

**是否值得自己实现**：❌ 不值得
- HTTP 代理是唯一合理方式

---

### 4. Settings（设置管理）

**IronClaw 实现了什么**：
- `Settings` 结构体：~100 个配置项，涵盖 LLM、通道、安全、WASM、沙箱等
- `SettingsStore` trait：数据库 key-value 存储
- 配置优先级：环境变量 > TOML 文件 > 数据库 > 默认值
- 导入/导出功能

**管理端需要什么**：
- 查看/修改 IronClaw 的运行时配置
- 例如：切换 LLM 模型、调整安全参数、配置通道

**自己实现的难度**：🟡 中等
- 如果只是 key-value CRUD，很简单
- 但需要理解每个配置项的含义和验证规则
- Settings 结构体有 ~100 个字段，验证逻辑复杂

**是否值得自己实现**：❌ 不值得
- 管理端修改的是 IronClaw 的配置，必须写入 IronClaw 的数据库
- HTTP 代理是唯一合理方式（除非共享数据库）

---

### 5. Logs（日志）

**IronClaw 实现了什么**：
- `LogBroadcaster`：SSE 实时日志流
- `LogLevelHandle`：运行时日志级别调整
- 基于 `tracing` 的结构化日志

**管理端需要什么**：
- 实时查看 IronClaw 的运行日志
- 调整日志级别

**自己实现的难度**：🟢 低（但没意义）
- 日志是 IronClaw 进程产生的，管理端无法自己生成
- 必须从 IronClaw 获取

**是否值得自己实现**：❌ 不可能自己实现
- 日志在 IronClaw 进程中，只能通过 API 获取
- HTTP 代理（SSE 转发）是唯一方式

---

### 6. Safety（安全层）

**IronClaw 实现了什么**：
- `ironclaw_safety` crate（已独立）：
  - 提示注入检测和清理（Sanitizer）
  - 凭证泄露检测（credential_detect、leak_detector）
  - 策略规则检查（7 条硬编码规则）
  - 输出长度限制
  - XML 属性转义

**管理端需要什么**：
- 当前：不直接使用（DLP 规则管理是 admin-backend 独有的）
- 未来：策略下发时，需要将 admin-backend 的 DLP 规则转换为 ironclaw_safety 的格式

**自己实现的难度**：N/A（已有共享 crate）

**是否值得自己实现**：✅ 已有共享 crate `ironclaw_safety`
- Desktop-Client 已经在用
- 未来策略下发时，admin-backend 也可以依赖这个 crate

---

### 7. Auth（认证）

**IronClaw 实现了什么**：
- `ironclaw_auth` crate（已独立）：
  - Argon2 密码哈希
  - JWT 生成和验证

**管理端需要什么**：
- 用户登录认证
- JWT token 管理

**自己实现的难度**：N/A（已有共享 crate）

**是否值得自己实现**：✅ 已有共享 crate `ironclaw_auth`
- Admin-Backend 已经在用

---

### 总结表

| 能力 | 复杂度 | 自己实现难度 | 推荐方式 | 理由 |
|------|--------|------------|---------|------|
| Skills | 中 | 🟢 低 | HTTP 代理 | 管理端无法访问 IronClaw 文件系统 |
| Extensions | 极高 | 🔴 极高 | HTTP 代理 | 7000+ 行代码，涉及 OAuth/WASM/Docker |
| Routines | 高 | 🔴 极高 | HTTP 代理 | 依赖 LLM、工具注册表等核心组件 |
| Settings | 中 | 🟡 中 | HTTP 代理 | 需要写入 IronClaw 的数据库 |
| Logs | 低 | ❌ 不可能 | HTTP 代理（SSE） | 日志在 IronClaw 进程中 |
| Safety | 中 | N/A | ✅ 已有共享 crate | `ironclaw_safety` |
| Auth | 低 | N/A | ✅ 已有共享 crate | `ironclaw_auth` |

### 架构结论

**这 7 个共享能力中**：
- 2 个已有共享 crate（Safety、Auth）→ 直接用
- 5 个必须通过 HTTP 代理（Skills、Extensions、Routines、Settings、Logs）→ 因为它们的运行时状态在 IronClaw 进程中

**自己在管理端实现这些能力不合理**，原因：
1. Skills/Extensions/Routines 的状态在 IronClaw 的文件系统和内存中，管理端无法直接访问
2. Settings 需要写入 IronClaw 的数据库，管理端有自己的数据库
3. Logs 是 IronClaw 进程产生的，管理端无法自己生成
4. 即使自己实现了，也会和 IronClaw 的状态不同步

**不需要改造 IronClaw**：
- IronClaw 已经提供了完整的 HTTP API
- Admin-Backend 通过 HTTP 代理这些 API 是正确的架构
- 唯一可以优化的是共享类型定义（方案 B），但这是锦上添花，不是必须的

---

### 最终架构推荐

```
┌─────────────────────────────────────────────────────────────┐
│                    Admin-Backend（统一管理控制台）              │
│                                                             │
│  独有能力（自己的数据库）：                                     │
│  ├── 用户管理 + RBAC                                         │
│  ├── DLP 规则 CRUD + 字典管理                                 │
│  ├── 敏感操作管理                                             │
│  ├── 客户端管理                                               │
│  ├── 策略版本控制                                             │
│  └── 审计日志系统                                             │
│                                                             │
│  共享 Crate（编译期依赖）：                                     │
│  ├── ironclaw_auth（密码、JWT）                               │
│  └── ironclaw_safety（安全规则，未来策略下发用）                 │
│                                                             │
│  HTTP 代理（运行时依赖 IronClaw Gateway）：                    │
│  ├── Skills 列表/安装/卸载                                    │
│  ├── Extensions 列表/安装/激活                                │
│  ├── Routines 列表/触发/启停                                  │
│  ├── Settings 读写                                           │
│  └── Logs 实时监控                                           │
└──────────────────────┬──────────────────────────────────────┘
                       │ HTTP API
                       ▼
┌─────────────────────────────────────────────────────────────┐
│              IronClaw Web Gateway (:38080)                    │
│              （不需要改造，已有完整 API）                        │
└─────────────────────────────────────────────────────────────┘
```

**这个架构不需要改造 IronClaw**，也不需要把 IronClaw 当作 Rust 库嵌入。Admin-Backend 通过 HTTP 代理获取 IronClaw 的运行时状态，通过共享 crate 获取编译期类型安全。

## 十一、参考文件

| 文件 | 说明 |
|------|------|
| `ironclaw/Cargo.toml` | IronClaw 依赖列表（~80+ crates） |
| `ironclaw/src/lib.rs` | IronClaw 公共 API（~40 个模块） |
| `desktop-client/Cargo.toml` | Desktop-Client 嵌入 IronClaw 的方式 |
| `desktop-client/tests/support/test_server.rs` | GatewayState 初始化示例（~25 个字段） |
| `desktop-client/src/embedded_server.rs` | Desktop-Client 连接外部 IronClaw（非嵌入） |
| `admin-backend/Cargo.toml` | Admin-Backend 当前依赖（~30 crates） |
| `admin-backend/src/routes.rs` | 当前 HTTP 代理实现 |
| `docs/IRONCLAW_CAPABILITY_COMPARISON.md` | 功能对比分析 |
| `ironclaw/src/skills/mod.rs` | Skills 系统结构 |
| `ironclaw/src/skills/registry.rs` | SkillRegistry 实现（~1000 行） |
| `ironclaw/src/extensions/mod.rs` | Extensions 类型定义 |
| `ironclaw/src/extensions/manager.rs` | ExtensionManager 实现（~7000 行） |
| `ironclaw/src/agent/routine_engine.rs` | RoutineEngine 实现 |
| `ironclaw/src/settings.rs` | Settings 结构体（~100 个配置项） |
| `ironclaw/src/config/mod.rs` | Config 系统（~20 个子配置） |
| `ironclaw/src/db/mod.rs` | Database trait（7 个子 trait） |
| `ironclaw/src/channels/web/server.rs` | IronClaw Web Gateway 路由定义 |
