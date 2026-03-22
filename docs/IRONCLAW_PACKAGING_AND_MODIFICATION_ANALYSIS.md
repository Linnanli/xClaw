# IronClaw 打包方案与改造分析

> 生成日期：2026-03-21
> 核心问题：在"客户端本地 AI + 管理端集中管理"架构下，IronClaw 怎么打包？需要改造什么？改造大不大？

---

## 一、结论先行

| 问题 | 答案 |
|------|------|
| 怎么打包？ | Tauri Sidecar 打包完整 IronClaw 二进制（短期最优） |
| 需要改造 IronClaw 吗？ | 需要，但改动很小 |
| 改造大不大？ | **不大。核心改动约 200 行代码，3-5 天完成** |

---

## 二、打包方案

### 短期方案：Sidecar 打包完整 IronClaw（推荐）

```
desktop-client 安装包：
├── desktop-client 二进制        ~15MB
├── ironclaw sidecar 二进制      ~65MB (release + strip + LTO)
├── 前端资源                     ~5MB
└── 总计                         ~85MB
```

**为什么不裁剪？**

IronClaw 用 `--cli-only` 启动时，不需要的模块（Telegram、Slack、Docker 沙箱等）虽然编译进了二进制，但**不会被初始化、不会占运行时内存、不会监听端口**。从 `main.rs` 代码可以看到：

```rust
// --cli-only 模式下，这些通道全部跳过：
if !cli.cli_only {
    // Signal channel → 跳过
    // HTTP channel → 跳过
    // WASM channels → 跳过
}
// 只启动 Gateway channel（客户端需要的 Web API）
```

所以"多出来的代码"只是占磁盘空间（~30MB），不影响运行时性能。

### 中期方案：IronClaw 添加 Feature Flags

等产品稳定后，可以给 IronClaw 加 feature flags 编译精简版（减少到 ~40MB）。但这不是当前优先级。

---

## 三、IronClaw 需要的改造

### 改造清单总览

| # | 改造点 | 涉及文件 | 改动量 | 难度 | 说明 |
|---|--------|----------|--------|------|------|
| 1 | Sanitizer 支持动态规则加载 | `sanitizer.rs` | ~60 行 | ⭐⭐ | 新增 `with_additional_patterns()` 方法 |
| 2 | SafetyLayer 支持外部注入 | `lib.rs` | ~30 行 | ⭐ | 新增 Builder 模式构造器 |
| 3 | 审计事件钩子 (AuditSink) | 新文件 `audit.rs` + `lib.rs` | ~50 行 | ⭐⭐ | 新增 trait + 默认实现 |
| 4 | 配置支持管理端地址 | `config.rs` | ~20 行 | ⭐ | 新增 3 个配置字段 |
| 5 | Gateway API 策略注入端点 | `handlers/` | ~40 行 | ⭐⭐ | 可选，本地 API 接收策略更新 |
| **合计** | | | **~200 行** | **3-5 天** | |

---

## 四、改造点详细分析

### 改造点 1：Sanitizer 支持动态规则加载

#### 现状分析

`Sanitizer::new()` 在构造函数中硬编码了所有检测模式：

```rust
// ironclaw/crates/ironclaw_safety/src/sanitizer.rs
pub fn new() -> Self {
    let patterns = vec![
        // 18 个硬编码的 PatternInfo（AhoCorasick 匹配）
        PatternInfo { pattern: "ignore previous".to_string(), ... },
        PatternInfo { pattern: "ignore all previous".to_string(), ... },
        // ... 共 18 个
    ];
    
    // AhoCorasick 匹配器在构造时一次性构建，之后不可变
    let pattern_matcher = AhoCorasick::builder()
        .ascii_case_insensitive(true)
        .build(&pattern_strings)
        .expect("Failed to build pattern matcher");
    
    // 4 个硬编码的正则模式
    let regex_patterns = vec![
        RegexPattern { regex: Regex::new(r"base64...").unwrap(), ... },
        RegexPattern { regex: Regex::new(r"eval\s*\(").unwrap(), ... },
        RegexPattern { regex: Regex::new(r"exec\s*\(").unwrap(), ... },
        RegexPattern { regex: Regex::new(r"\x00").unwrap(), ... },
    ];
}
```

**关键问题**：AhoCorasick 匹配器构建后不可变，无法动态添加模式。

#### 改造方案

```rust
// 新增方法：合并默认模式 + 管理端下发的额外模式
impl Sanitizer {
    /// 使用默认模式 + 额外模式创建 Sanitizer
    pub fn with_additional_patterns(
        extra_patterns: Vec<PatternInfo>,
        extra_regex: Vec<RegexPattern>,
    ) -> Self {
        let mut patterns = Self::default_patterns(); // 提取现有硬编码为方法
        patterns.extend(extra_patterns);
        
        let mut regex_patterns = Self::default_regex_patterns();
        regex_patterns.extend(extra_regex);
        
        let pattern_strings: Vec<&str> = patterns.iter()
            .map(|p| p.pattern.as_str()).collect();
        let pattern_matcher = AhoCorasick::builder()
            .ascii_case_insensitive(true)
            .build(&pattern_strings)
            .expect("Failed to build pattern matcher");
        
        Self { pattern_matcher, patterns, regex_patterns }
    }
    
    /// 完全重建 Sanitizer（用于热更新场景）
    pub fn rebuild_with_patterns(
        patterns: Vec<PatternInfo>,
        regex_patterns: Vec<RegexPattern>,
    ) -> Self {
        // ... 同上，但不合并默认模式
    }
}
```

**改动量**：~60 行（提取 `default_patterns()` 方法 + 新增 `with_additional_patterns()`）

#### 对比：LeakDetector 已有的扩展能力

LeakDetector 设计得更好，**已经支持动态扩展**：

```rust
// 已有的 API，无需改造！
impl LeakDetector {
    pub fn new() -> Self { Self::with_patterns(default_patterns()) }
    pub fn with_patterns(patterns: Vec<LeakPattern>) -> Self { ... }
    pub fn add_pattern(&mut self, pattern: LeakPattern) { ... }  // 16 个默认模式
}
```

LeakDetector 有 16 个默认泄露检测模式（OpenAI key、AWS key、GitHub token 等），但因为已有 `with_patterns()` 和 `add_pattern()`，**不需要任何改造**。

---

### 改造点 2：SafetyLayer 支持外部注入

#### 现状分析

```rust
// ironclaw/crates/ironclaw_safety/src/lib.rs
pub struct SafetyConfig {
    pub max_output_length: usize,
    pub injection_check_enabled: bool,
}

impl SafetyLayer {
    pub fn new(config: &SafetyConfig) -> Self {
        Self {
            sanitizer: Sanitizer::new(),        // 硬编码
            validator: Validator::new(),         // 硬编码
            policy: Policy::default(),           // 硬编码（7 条规则）
            leak_detector: LeakDetector::new(),  // 硬编码（16 个模式）
            config: config.clone(),
        }
    }
}
```

所有子组件都用默认构造器，没有注入点。

#### 改造方案

```rust
/// Builder 模式，允许客户端注入管理端下发的规则
pub struct SafetyLayerBuilder {
    config: SafetyConfig,
    extra_sanitizer_patterns: Vec<PatternInfo>,
    extra_sanitizer_regex: Vec<RegexPattern>,
    extra_policy_rules: Vec<PolicyRule>,
    extra_leak_patterns: Vec<LeakPattern>,
}

impl SafetyLayerBuilder {
    pub fn new(config: SafetyConfig) -> Self { ... }
    
    pub fn with_extra_sanitizer_patterns(mut self, p: Vec<PatternInfo>) -> Self {
        self.extra_sanitizer_patterns = p; self
    }
    pub fn with_extra_policy_rules(mut self, r: Vec<PolicyRule>) -> Self {
        self.extra_policy_rules = r; self
    }
    pub fn with_extra_leak_patterns(mut self, p: Vec<LeakPattern>) -> Self {
        self.extra_leak_patterns = p; self
    }
    
    pub fn build(self) -> SafetyLayer {
        let sanitizer = if self.extra_sanitizer_patterns.is_empty() 
            && self.extra_sanitizer_regex.is_empty() {
            Sanitizer::new()
        } else {
            Sanitizer::with_additional_patterns(
                self.extra_sanitizer_patterns,
                self.extra_sanitizer_regex,
            )
        };
        
        let mut policy = Policy::default();
        for rule in self.extra_policy_rules {
            policy.add_rule(rule);  // Policy 已有 add_rule()！
        }
        
        let mut leak_detector = LeakDetector::new();
        for pattern in self.extra_leak_patterns {
            leak_detector.add_pattern(pattern);  // LeakDetector 已有 add_pattern()！
        }
        
        SafetyLayer { sanitizer, validator: Validator::new(), policy, leak_detector, config: self.config }
    }
}
```

**改动量**：~30 行（Builder struct + build 方法）

**关键发现**：`Policy` 已有 `add_rule()` 方法，`LeakDetector` 已有 `add_pattern()` 方法，只有 `Sanitizer` 需要新增扩展方法。

---

### 改造点 3：审计事件钩子 (AuditSink)

#### 现状分析

IronClaw 有 `HookRegistry` 用于生命周期钩子，有 `LogBroadcaster` 用于日志广播，但**没有专门的审计事件输出通道**。客户端需要将以下事件上报给管理端：

- DLP 拦截/脱敏事件
- 安全策略违规事件
- 用户操作审计（发送消息、使用工具等）

#### 改造方案

```rust
// 新文件：ironclaw/crates/ironclaw_safety/src/audit.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 审计事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEvent {
    DlpMatch { rule_id: String, action: String, content_hash: String },
    PolicyViolation { rule_id: String, severity: String },
    ToolExecution { tool_name: String, success: bool },
    UserAction { action: String, details: String },
}

/// 审计事件包装
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub timestamp: DateTime<Utc>,
    pub client_id: String,
    pub event: AuditEvent,
}

/// 审计输出 trait — 客户端实现此 trait 将事件转发给管理端
#[async_trait::async_trait]
pub trait AuditSink: Send + Sync {
    async fn emit(&self, record: AuditRecord);
}

/// 默认实现：仅写日志（IronClaw 独立运行时使用）
pub struct LogAuditSink;

#[async_trait::async_trait]
impl AuditSink for LogAuditSink {
    async fn emit(&self, record: AuditRecord) {
        tracing::info!(audit = ?record, "audit event");
    }
}
```

**改动量**：~50 行（新文件 `audit.rs` + `lib.rs` 中注册到 SafetyLayer）

**客户端侧实现**：Desktop-Client 实现一个 `HttpAuditSink`，将事件 POST 到管理端 API。

---

### 改造点 4：配置支持管理端地址

#### 改造方案

在 IronClaw 配置中新增管理端相关字段：

```toml
# .env 或 config.toml
ADMIN_URL=https://admin.example.com
CLIENT_ID=client-001
CLIENT_AUTH_TOKEN=xxx
```

```rust
// 在 Config 结构体中新增
pub struct AdminConfig {
    pub admin_url: Option<String>,      // 管理端地址
    pub client_id: Option<String>,      // 客户端标识
    pub auth_token: Option<String>,     // 认证 token
    pub sync_interval_secs: u64,        // 策略同步间隔（默认 300s）
}
```

**改动量**：~20 行

**说明**：这些配置由 Desktop-Client 在启动 Sidecar 时通过环境变量注入，IronClaw 本身只需要读取即可。

---

### 改造点 5：Gateway API 策略注入端点（可选）

#### 场景

Desktop-Client 从管理端拉取到新策略后，需要通知正在运行的 IronClaw Sidecar 更新规则。有两种方式：

| 方式 | 说明 | 优缺点 |
|------|------|--------|
| A. 重启 Sidecar | 用新配置重启 IronClaw | 简单但有中断 |
| B. 本地 API 热更新 | 调用 Gateway API 注入新规则 | 无中断，需要新端点 |

#### 改造方案（方式 B）

```rust
// 新增 Gateway API 端点
// POST /api/admin/policy/update
async fn update_policy(
    State(state): State<Arc<GatewayState>>,
    Json(payload): Json<PolicyUpdatePayload>,
) -> impl IntoResponse {
    // 验证请求来自本地 Desktop-Client
    // 更新 SafetyLayer 中的规则
    // 返回更新结果
}

#[derive(Deserialize)]
struct PolicyUpdatePayload {
    sanitizer_patterns: Option<Vec<PatternInfo>>,
    policy_rules: Option<Vec<PolicyRuleDto>>,
    leak_patterns: Option<Vec<LeakPatternDto>>,
}
```

**改动量**：~40 行

**建议**：短期用方式 A（重启），中期实现方式 B。

---

## 五、改造难度评估

### 各组件现有扩展能力对比

| 组件 | 现有扩展 API | 需要新增 | 改造难度 |
|------|-------------|---------|---------|
| **Policy** | ✅ `add_rule()` 已有 | 无 | ⭐ 零改造 |
| **LeakDetector** | ✅ `with_patterns()` + `add_pattern()` 已有 | 无 | ⭐ 零改造 |
| **Sanitizer** | ❌ 无扩展 API | `with_additional_patterns()` | ⭐⭐ 中等 |
| **SafetyLayer** | ❌ 只有 `new()` | Builder 模式 | ⭐ 简单 |
| **Validator** | N/A | 无需改造 | ⭐ 无 |

**关键发现**：IronClaw 的 `Policy` 和 `LeakDetector` 已经设计了扩展接口，只有 `Sanitizer` 需要真正的改造。这说明 IronClaw 的架构本身就考虑了可扩展性，改造成本比预期更低。

### 改造不涉及的部分

以下部分**完全不需要改动**：

- Agent 核心循环（`agent_loop.rs`）— `AgentDeps.safety` 是 `Arc<SafetyLayer>`，替换构造方式即可
- LLM 调用链 — 不受影响
- 工具系统 — 不受影响
- 数据库层 — 不受影响
- 所有 Channel 实现 — 不受影响
- `--cli-only` 启动逻辑 — 不受影响

---

## 六、Desktop-Client 侧的工作（不改 IronClaw）

以下工作在 Desktop-Client 中完成，**不需要修改 IronClaw 源码**：

| 工作项 | 说明 | 改动量 |
|--------|------|--------|
| 策略同步服务 | 定时从管理端拉取最新 DLP 规则 | ~150 行 |
| 审计上报服务 | 实现 `HttpAuditSink`，POST 事件到管理端 | ~100 行 |
| Sidecar 管理器 | 启动/停止/重启 IronClaw 进程 | ~200 行 |
| 配置注入 | 将管理端地址、规则文件路径注入 Sidecar 环境变量 | ~50 行 |
| 客户端注册 | 启动时向管理端注册，定时心跳 | ~100 行 |

---

## 七、实施路线图

### 第一阶段（1-2 天）：最小可用

```
1. Sanitizer 新增 with_additional_patterns()     [~60 行]
2. SafetyLayer 新增 Builder 模式                  [~30 行]
3. Desktop-Client Sidecar 管理器                  [~200 行]
```

**效果**：客户端可以 Sidecar 启动 IronClaw，使用默认安全规则。

### 第二阶段（2-3 天）：策略同步

```
4. 配置支持管理端地址                              [~20 行]
5. Desktop-Client 策略同步服务                     [~150 行]
6. Desktop-Client 配置注入                         [~50 行]
```

**效果**：管理端可以下发 DLP 规则到客户端。

### 第三阶段（2-3 天）：审计上报

```
7. AuditSink trait + 默认实现                      [~50 行]
8. Desktop-Client HttpAuditSink                    [~100 行]
9. Desktop-Client 客户端注册 + 心跳                [~100 行]
```

**效果**：客户端安全事件可以上报到管理端。

### 第四阶段（可选）：热更新

```
10. Gateway API 策略注入端点                       [~40 行]
```

**效果**：策略更新无需重启 Sidecar。

---

## 八、总结

| 维度 | 结论 |
|------|------|
| **打包方式** | Sidecar 完整 IronClaw 二进制，~85MB，可接受 |
| **IronClaw 改造量** | ~200 行代码，集中在 `ironclaw_safety` crate |
| **改造难度** | 低。Policy 和 LeakDetector 已有扩展 API，只需改 Sanitizer + SafetyLayer |
| **改造风险** | 极低。不涉及核心 Agent 循环、LLM 调用链、数据库层 |
| **Desktop-Client 工作量** | ~600 行（策略同步 + 审计上报 + Sidecar 管理） |
| **总工期** | IronClaw 改造 3-5 天 + Desktop-Client 集成 5-7 天 |
| **是否值得** | 值得。改造量小，收益大（支持企业级策略管理和审计） |

### 核心结论

> IronClaw 的架构设计已经为扩展预留了空间（Policy.add_rule、LeakDetector.with_patterns），真正需要改造的只有 Sanitizer 的模式加载方式和 SafetyLayer 的构造方式。这是一个**低风险、低成本、高收益**的改造。
