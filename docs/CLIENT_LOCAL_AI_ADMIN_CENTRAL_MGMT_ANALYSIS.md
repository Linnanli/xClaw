# 客户端本地 AI + 管理端集中管理 架构分析

> 生成日期：2026-03-21
> 核心思路：AI 能力（Agent + LLM + Tools）在客户端本地运行，管理端只做策略下发和数据收集
> 这是正确的企业桌面 AI 产品架构

---

## 一、架构全景

```
┌─────────────────────────────────────────────────────────┐
│                    Admin Backend                         │
│                   (集中管理控制台)                         │
│                                                         │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐   │
│  │ DLP 策略  │ │ 用户管理  │ │ 审计日志  │ │ 报表统计  │   │
│  │ 管理     │ │ RBAC     │ │ 查看     │ │ 分析     │   │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘   │
│                                                         │
│  ┌─────────────────────────────────────────────────┐    │
│  │              管理端数据库 (PostgreSQL)             │    │
│  │  - DLP 规则、字典、敏感操作规则                     │    │
│  │  - 用户/角色/权限                                 │    │
│  │  - 审计日志（从客户端上报）                         │    │
│  │  - 客户端注册信息和状态                            │    │
│  └─────────────────────────────────────────────────┘    │
│                                                         │
│  API: /api/policy/latest    ← 客户端拉取最新策略         │
│  API: /api/audit/report     ← 客户端上报审计事件         │
│  API: /api/clients/register ← 客户端注册/心跳            │
└──────────────────┬──────────────────────────────────────┘
                   │
          ┌────────┼────────┐        (HTTPS)
          │        │        │
          ▼        ▼        ▼
┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│ Client A     │ │ Client B     │ │ Client C     │
│              │ │              │ │              │
│ ┌──────────┐ │ │ ┌──────────┐ │ │ ┌──────────┐ │
│ │ IronClaw │ │ │ │ IronClaw │ │ │ │ IronClaw │ │
│ │ (本地AI) │ │ │ │ (本地AI) │ │ │ │ (本地AI) │ │
│ │ Agent    │ │ │ │ Agent    │ │ │ │ Agent    │ │
│ │ LLM      │ │ │ │ LLM      │ │ │ │ LLM      │ │
│ │ Tools    │ │ │ │ Tools    │ │ │ │ Tools    │ │
│ │ Safety   │ │ │ │ Safety   │ │ │ │ Safety   │ │
│ └──────────┘ │ │ └──────────┘ │ │ └──────────┘ │
│              │ │              │ │              │
│ ┌──────────┐ │ │ ┌──────────┐ │ │ ┌──────────┐ │
│ │ 策略引擎  │ │ │ │ 策略引擎  │ │ │ │ 策略引擎  │ │
│ │ (从管理端 │ │ │ │ (从管理端 │ │ │ │ (从管理端 │ │
│ │  同步策略) │ │ │ │  同步策略) │ │ │ │  同步策略) │ │
│ └──────────┘ │ │ └──────────┘ │ │ └──────────┘ │
│              │ │              │ │              │
│ ┌──────────┐ │ │ ┌──────────┐ │ │ ┌──────────┐ │
│ │ 审计上报  │ │ │ │ 审计上报  │ │ │ │ 审计上报  │ │
│ │ (异步上报 │ │ │ │ (异步上报 │ │ │ │ (异步上报 │ │
│ │  到管理端) │ │ │ │  到管理端) │ │ │ │  到管理端) │ │
│ └──────────┘ │ │ └──────────┘ │ │ └──────────┘ │
│              │ │              │ │              │
│ libSQL(本地) │ │ libSQL(本地) │ │ libSQL(本地) │
└──────────────┘ └──────────────┘ └──────────────┘
```

---

## 二、为什么这个架构是对的

### 对比三种架构

| 架构 | AI 运行位置 | 数据存储 | 管理能力 | 类比产品 |
|------|-----------|---------|---------|---------|
| ❌ 纯中心化 | 服务端 | 服务端 | ✅ 完整 | ChatGPT Web |
| ❌ 纯本地化 | 客户端 | 客户端 | ❌ 无 | Ollama |
| ✅ **本地 AI + 集中管理** | **客户端** | **客户端(AI) + 管理端(策略/审计)** | **✅ 完整** | **企业版 Copilot** |

### 为什么 AI 应该在客户端运行

1. **隐私**：用户的对话、代码、文件不离开本机
2. **延迟**：本地推理（如果用本地模型）零网络延迟
3. **离线**：断网也能用（如果用本地模型）
4. **成本**：不需要企业部署 GPU 服务器
5. **弹性**：每个客户端独立运行，一个挂了不影响其他人

### 为什么管理应该集中

1. **策略统一**：所有客户端执行相同的 DLP 规则
2. **审计合规**：所有操作记录集中存储，方便审计
3. **用户管理**：统一的 RBAC 和权限控制
4. **可见性**：管理员能看到所有客户端的状态和行为

---

## 三、数据流设计

### 3.1 策略下发（管理端 → 客户端）

```
管理员在 Admin Backend 配置 DLP 规则
  ↓
存入管理端数据库
  ↓
客户端定时拉取（或 WebSocket 推送）
  ↓
客户端本地缓存策略
  ↓
IronClaw Safety 层使用这些策略过滤内容
```

#### 策略同步 API

```
GET /api/policy/latest?client_id={id}&last_sync={timestamp}

Response:
{
  "version": 42,
  "updated_at": "2026-03-21T10:00:00Z",
  "dlp_rules": [
    {
      "id": "uuid",
      "name": "身份证号脱敏",
      "pattern": "\\d{17}[\\dXx]",
      "replacement": "***",
      "severity": "high",
      "category": "pii",
      "enabled": true
    }
  ],
  "sensitive_operations": [
    {
      "id": "uuid",
      "name": "删除文件",
      "operation_type": "file_delete",
      "requires_approval": true,
      "risk_level": "high"
    }
  ],
  "system_config": {
    "dlp_enabled": true,
    "dlp_fail_open": false,
    "audit_enabled": true
  }
}
```

#### 客户端策略引擎

```rust
/// 客户端策略引擎 - 从管理端同步策略并应用到本地 IronClaw
pub struct PolicyEngine {
    /// 管理端 API 地址
    admin_url: String,
    /// 客户端 ID
    client_id: String,
    /// 认证 Token
    auth_token: String,
    /// 本地缓存的策略版本
    cached_version: u64,
    /// 本地缓存的 DLP 规则
    cached_rules: Vec<DlpRule>,
    /// 同步间隔
    sync_interval: Duration,
}

impl PolicyEngine {
    /// 定时从管理端拉取最新策略
    pub async fn sync_loop(&mut self) {
        loop {
            match self.fetch_latest_policy().await {
                Ok(policy) => {
                    if policy.version > self.cached_version {
                        self.apply_policy(&policy);
                        self.cached_version = policy.version;
                        tracing::info!(
                            "策略已更新: v{} → v{}",
                            self.cached_version, policy.version
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!("策略同步失败: {}，使用本地缓存", e);
                    // 故障安全：同步失败时继续使用本地缓存的策略
                }
            }
            tokio::time::sleep(self.sync_interval).await;
        }
    }

    /// 将管理端的 DLP 规则转换为 IronClaw Safety 可用的格式
    fn apply_policy(&mut self, policy: &PolicyResponse) {
        self.cached_rules = policy.dlp_rules.clone();
        // 更新本地 IronClaw 的 Safety 配置
        // 这里需要 IronClaw 支持动态更新安全规则
    }
}
```

### 3.2 审计上报（客户端 → 管理端）

```
用户在客户端进行操作（对话、使用工具等）
  ↓
IronClaw 本地记录操作日志
  ↓
审计上报服务异步批量上报到管理端
  ↓
管理端存入审计日志数据库
  ↓
管理员在 Admin Backend 查看审计日志
```

#### 审计上报 API

```
POST /api/audit/report

Body:
{
  "client_id": "uuid",
  "events": [
    {
      "event_id": "uuid",
      "timestamp": "2026-03-21T10:05:00Z",
      "user_id": "user@company.com",
      "action": "chat_message",
      "details": {
        "thread_id": "uuid",
        "message_length": 256,
        "dlp_triggered": true,
        "dlp_rule_matched": "身份证号脱敏",
        "was_blocked": false
      }
    },
    {
      "event_id": "uuid",
      "timestamp": "2026-03-21T10:06:00Z",
      "user_id": "user@company.com",
      "action": "tool_execution",
      "details": {
        "tool_name": "file_read",
        "file_path": "/src/main.rs",
        "sensitive_op": false
      }
    }
  ]
}
```

#### 客户端审计上报服务

```rust
/// 审计事件上报服务 - 异步批量上报到管理端
pub struct AuditReporter {
    admin_url: String,
    client_id: String,
    auth_token: String,
    /// 本地事件队列（断网时缓存）
    event_queue: Vec<AuditEvent>,
    /// 批量上报阈值
    batch_size: usize,
    /// 上报间隔
    report_interval: Duration,
}

impl AuditReporter {
    /// 记录一个审计事件（本地）
    pub fn record(&mut self, event: AuditEvent) {
        self.event_queue.push(event);

        // 达到批量阈值时立即上报
        if self.event_queue.len() >= self.batch_size {
            self.flush();
        }
    }

    /// 定时上报循环
    pub async fn report_loop(&mut self) {
        loop {
            if !self.event_queue.is_empty() {
                match self.send_batch().await {
                    Ok(()) => {
                        tracing::debug!(
                            "审计事件上报成功: {} 条",
                            self.event_queue.len()
                        );
                        self.event_queue.clear();
                    }
                    Err(e) => {
                        tracing::warn!(
                            "审计上报失败: {}，保留 {} 条事件在本地队列",
                            e, self.event_queue.len()
                        );
                        // 故障安全：上报失败时保留在本地队列
                        // 下次循环重试
                    }
                }
            }
            tokio::time::sleep(self.report_interval).await;
        }
    }
}
```

### 3.3 客户端注册和心跳

```
客户端首次启动
  ↓
向管理端注册（POST /api/clients/register）
  ↓
获取 client_id 和 auth_token
  ↓
定时发送心跳（POST /api/clients/heartbeat）
  ↓
管理端更新客户端在线状态
```

---

## 四、关键问题：IronClaw 需要改什么？

### 问题 1：IronClaw Safety 规则是硬编码的

当前 IronClaw 的安全规则：

```rust
// ironclaw_safety/src/lib.rs
impl SafetyLayer {
    pub fn new(config: &SafetyConfig) -> Self {
        Self {
            sanitizer: Sanitizer::new(),      // 硬编码的注入检测规则
            validator: Validator::new(),       // 硬编码的验证规则
            policy: Policy::default(),        // 硬编码的 7 条策略规则
            leak_detector: LeakDetector::new(), // 硬编码的泄露检测规则
            config: config.clone(),
        }
    }
}
```

**需要改造**：让 SafetyLayer 支持动态加载规则

```rust
// 改造后
impl SafetyLayer {
    /// 从管理端策略创建（客户端使用）
    pub fn with_dynamic_rules(
        config: &SafetyConfig,
        dlp_rules: Vec<DlpRule>,        // 从管理端同步的 DLP 规则
    ) -> Self {
        let mut sanitizer = Sanitizer::new();
        // 将管理端的 DLP 规则注入到 Sanitizer
        for rule in &dlp_rules {
            sanitizer.add_pattern(&rule.pattern, &rule.replacement);
        }
        Self {
            sanitizer,
            validator: Validator::new(),
            policy: Policy::default(),
            leak_detector: LeakDetector::new(),
            config: config.clone(),
        }
    }

    /// 热更新规则（策略同步时调用）
    pub fn update_rules(&mut self, dlp_rules: Vec<DlpRule>) {
        let mut sanitizer = Sanitizer::new();
        for rule in &dlp_rules {
            sanitizer.add_pattern(&rule.pattern, &rule.replacement);
        }
        self.sanitizer = sanitizer;
    }
}
```

### 问题 2：IronClaw 没有审计事件钩子

当前 IronClaw 有 `HookRegistry`（生命周期钩子），但没有专门的审计事件输出。

**需要添加**：审计事件发射器

```rust
// 在 IronClaw 的 Agent 或 SafetyLayer 中添加
pub trait AuditSink: Send + Sync {
    fn emit(&self, event: AuditEvent);
}

// 客户端实现这个 trait，把事件发送到 AuditReporter
pub struct RemoteAuditSink {
    reporter: Arc<Mutex<AuditReporter>>,
}

impl AuditSink for RemoteAuditSink {
    fn emit(&self, event: AuditEvent) {
        self.reporter.lock().unwrap().record(event);
    }
}
```

### 问题 3：IronClaw 打包膨胀

如前面分析的，完整 IronClaw 二进制包含很多客户端不需要的模块。

**短期方案**：接受全量打包，用 `--cli-only` 禁用不需要的通道
**中期方案**：添加 feature flags 编译精简版

---

## 五、客户端需要新增的模块

```
desktop-client/
├── src/
│   ├── main.rs                    # Tauri 入口
│   ├── sidecar.rs                 # IronClaw Sidecar 管理
│   ├── policy_engine.rs           # ✨ 策略引擎（从管理端同步 DLP 规则）
│   ├── audit_reporter.rs          # ✨ 审计上报（异步批量上报事件）
│   ├── client_registration.rs     # ✨ 客户端注册和心跳
│   └── commands/
│       ├── chat.rs                # 对话（转发到本地 IronClaw）
│       ├── policy.rs              # ✨ 策略相关命令
│       └── ...
```

### 模块职责

| 模块 | 职责 | 通信方向 |
|------|------|---------|
| `sidecar.rs` | 管理本地 IronClaw 进程 | Client → 本地 IronClaw |
| `policy_engine.rs` | 定时拉取管理端策略，更新本地 Safety 规则 | Client ← Admin Backend |
| `audit_reporter.rs` | 收集本地审计事件，批量上报管理端 | Client → Admin Backend |
| `client_registration.rs` | 客户端注册、心跳、状态上报 | Client ↔ Admin Backend |

---

## 六、管理端需要新增的 API

```
Admin Backend 新增 API：

策略下发：
  GET  /api/policy/latest          # 客户端拉取最新策略
  GET  /api/policy/version         # 检查策略版本号

审计收集：
  POST /api/audit/report           # 客户端批量上报审计事件
  GET  /api/audit/events           # 管理员查看审计日志（已有）

客户端管理：
  POST /api/clients/register       # 客户端注册
  POST /api/clients/heartbeat      # 客户端心跳
  GET  /api/clients                # 管理员查看客户端列表（已有）
  PUT  /api/clients/:id/disable    # 禁用某个客户端
```

---

## 七、数据存储分工

```
管理端数据库（PostgreSQL/SQLite）：
  ├── DLP 规则和字典              ← 管理员配置
  ├── 敏感操作规则                ← 管理员配置
  ├── 用户/角色/权限              ← 管理员配置
  ├── 策略变更记录                ← 管理员操作产生
  ├── 客户端注册信息              ← 客户端注册时写入
  ├── 审计日志                   ← 客户端上报
  └── 系统设置                   ← 管理员配置

客户端本地数据库（libSQL）：
  ├── 对话历史                   ← IronClaw 本地存储
  ├── 工作空间/记忆               ← IronClaw 本地存储
  ├── 技能和扩展配置              ← IronClaw 本地存储
  ├── 本地设置                   ← IronClaw 本地存储
  ├── 策略缓存                   ← 从管理端同步
  └── 待上报审计事件队列           ← 等待上报到管理端
```

### 数据归属原则

| 数据类型 | 归属 | 原因 |
|---------|------|------|
| 对话内容 | 客户端本地 | 隐私保护，不上传对话原文 |
| DLP 规则 | 管理端 | 统一管理，下发到客户端 |
| 审计事件 | 管理端（客户端上报） | 合规需要，集中审计 |
| 技能/扩展 | 客户端本地 | 本地运行，不需要集中管理 |
| 用户权限 | 管理端 | 统一 RBAC |
| 系统设置 | 管理端 | 统一配置 |

---

## 八、关键设计决策

### 决策 1：审计上报的粒度

```
选项 A：上报完整对话内容
  ✅ 管理员可以审查所有对话
  ❌ 隐私风险大
  ❌ 数据量大

选项 B：只上报元数据（推荐）
  ✅ 保护用户隐私
  ✅ 数据量小
  ✅ 满足合规需求
  ❌ 管理员看不到对话原文

选项 C：上报脱敏后的摘要
  ✅ 平衡隐私和可见性
  🟡 实现复杂度中等

推荐：选项 B（元数据）+ 可选的选项 C（脱敏摘要）
```

上报的元数据示例：
```json
{
  "action": "chat_message",
  "timestamp": "2026-03-21T10:05:00Z",
  "user_id": "user@company.com",
  "thread_id": "uuid",
  "message_length": 256,
  "dlp_triggered": true,
  "dlp_rules_matched": ["身份证号脱敏", "手机号脱敏"],
  "was_blocked": false,
  "tools_used": ["file_read", "web_search"],
  "model_used": "gpt-4",
  "token_count": 1500
}
```

### 决策 2：策略同步方式

```
选项 A：客户端定时拉取（Pull）
  ✅ 实现简单
  ✅ 客户端控制节奏
  ❌ 有延迟（取决于拉取间隔）

选项 B：管理端主动推送（Push via WebSocket）
  ✅ 实时生效
  ❌ 需要维护长连接
  ❌ 客户端离线时推送丢失

选项 C：Pull + Push 混合（推荐）
  ✅ 正常情况下 Pull（每 5 分钟）
  ✅ 紧急策略变更时 Push（WebSocket 通知）
  ✅ 客户端上线时立即 Pull 最新版本

推荐：选项 C（混合模式）
```

### 决策 3：离线时的行为

```
客户端无法连接管理端时：
  ├── 策略：使用本地缓存的最新策略（故障安全）
  ├── 审计：事件存入本地队列，恢复连接后补报
  ├── AI 功能：正常使用（不依赖管理端）
  └── 心跳：管理端标记客户端为"离线"
```

---

## 九、实施路线图

```
第 1 阶段：基础通信（2 周）
  ├── 管理端：策略下发 API（GET /api/policy/latest）
  ├── 管理端：审计收集 API（POST /api/audit/report）
  ├── 管理端：客户端注册 API（POST /api/clients/register）
  ├── 客户端：PolicyEngine（策略拉取 + 本地缓存）
  ├── 客户端：AuditReporter（事件收集 + 批量上报）
  └── 客户端：ClientRegistration（注册 + 心跳）

第 2 阶段：策略执行（2 周）
  ├── 改造 ironclaw_safety：支持动态加载 DLP 规则
  ├── 客户端：将管理端 DLP 规则注入 IronClaw Safety
  ├── 客户端：DLP 触发时生成审计事件
  └── 测试：策略下发 → 客户端执行 → 审计上报 全链路

第 3 阶段：Sidecar 集成（1 周）
  ├── 配置 Tauri Sidecar（打包 IronClaw 二进制）
  ├── 客户端启动时：启动 Sidecar → 同步策略 → 就绪
  ├── 客户端运行中：定时同步策略 + 上报审计
  └── 客户端退出时：停止 Sidecar

第 4 阶段：管理端增强（1 周）
  ├── 管理端：客户端状态仪表盘
  ├── 管理端：审计日志查看和搜索
  ├── 管理端：策略推送通知（WebSocket）
  └── 管理端：客户端禁用/启用

总计：约 6 周
```

---

## 十、与之前方案的对比

| 维度 | 纯 Sidecar（之前） | 本地 AI + 集中管理（现在） |
|------|-------------------|------------------------|
| AI 运行位置 | 客户端本地 | 客户端本地 |
| 数据管理 | ❌ 数据孤岛 | ✅ 审计集中 + 策略统一 |
| DLP 策略 | ❌ 各自独立 | ✅ 管理端统一下发 |
| 审计合规 | ❌ 无法集中审计 | ✅ 客户端上报到管理端 |
| 用户隐私 | ✅ 数据在本地 | ✅ 对话在本地，只上报元数据 |
| 离线使用 | ✅ 完全离线 | ✅ 离线可用（用缓存策略） |
| 管理端价值 | ❌ 形同虚设 | ✅ 真正的集中管理 |
| 实现复杂度 | 🟢 低 | 🟡 中（需要同步机制） |

---

## 十一、总结

```
正确的架构：

  客户端 = IronClaw（本地 AI）+ 策略引擎 + 审计上报
  管理端 = 策略管理 + 审计查看 + 用户管理 + 客户端管理

  数据流：
  管理端 ──策略下发──→ 客户端（DLP 规则、安全策略）
  客户端 ──审计上报──→ 管理端（操作元数据、DLP 触发记录）
  客户端 ──心跳────→ 管理端（在线状态、版本信息）

  AI 对话数据留在客户端本地，不上传
  管理端只收集审计元数据和策略执行结果
```

> **核心结论**：AI 能力在客户端本地运行是对的。关键是要建立客户端和管理端之间的"策略下发 + 审计上报"通道，让管理端能够统一管理策略并收集审计数据，而不是让每个客户端成为数据孤岛。
