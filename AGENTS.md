# Agent Rules

## 语言规则 (Language Rules)

**所有回答必须用中文。** (All responses must be in Chinese.)

## Feature Parity Update Policy

- If you change implementation status for any feature tracked in `FEATURE_PARITY.md`, update that file in the same branch.
- Do not open a PR that changes feature behavior without checking `FEATURE_PARITY.md` for needed status updates (`❌`, `🚧`, `✅`, notes, and priorities).

## 客户端和后端功能复用规则

**核心原则**：Desktop Client 和 Admin Backend 新增功能时，必须优先复用主项目（`src/`）中已有的能力，避免重复实现。

### 架构关系

```
主项目 (src/)
├── 核心功能实现
├── CLI 命令 (src/cli/)
├── Web Gateway API (src/channels/web/handlers/)
└── 共享 Crates (crates/)

Desktop Client (desktop-client/)
├── Tauri 应用
├── 调用 Web Gateway API
└── 依赖共享 Crates

Admin Backend (admin-backend/)
├── 管理后台应用
├── 依赖主项目 crate
└── 依赖共享 Crates
```

### Desktop Client 功能实现策略

**强制要求**：Desktop Client 新增功能时，必须按以下优先级实现：

1. **优先级 1：复用 Web Gateway API**
   - 检查 `src/channels/web/handlers/` 是否已有对应的 API 端点
   - 如果存在，创建 Tauri 命令包装器调用 Web API
   - **禁止**重新实现已有的业务逻辑

2. **优先级 2：复用共享 Crate**
   - 检查 `crates/` 目录下是否有可复用的共享模块
   - 如果存在，直接依赖该 crate
   - 示例：`crates/ironclaw_auth/`, `crates/ironclaw_safety/`

3. **优先级 3：创建新的共享 Crate**
   - 如果功能在主项目中存在但未暴露为 API 或 crate
   - 考虑将其提取为共享 crate（参考"共享代码架构规则"）
   - 同时让主项目和 Desktop Client 都使用该 crate

4. **最后选择：独立实现**
   - 仅当功能是 Desktop Client 特有的（如本地主密码、离线模式）
   - 才允许独立实现

### Admin Backend 功能实现策略

**强制要求**：Admin Backend 新增功能时，必须按以下优先级实现：

1. **优先级 1：依赖主项目 Crate**
   - Admin Backend 通过 `ironclaw` crate 依赖主项目
   - 直接使用主项目中的核心功能模块
   - 示例：`use ironclaw::agent`, `use ironclaw::db`

2. **优先级 2：复用共享 Crate**
   - 使用 `crates/` 目录下的共享模块
   - 示例：`ironclaw_auth`, `ironclaw_safety`

3. **优先级 3：创建新的共享 Crate**
   - 如果功能需要在 Admin Backend 和其他组件间共享
   - 提取为独立的共享 crate

4. **最后选择：独立实现**
   - 仅当功能是 Admin Backend 特有的管理功能
   - 才允许独立实现

### 可复用的 Web Gateway API 端点

在实现 Desktop Client 功能前，必须检查以下 API 是否可用：

- **Memory APIs**: `memory_tree_handler`, `memory_list_handler`, `memory_read_handler`, `memory_write_handler`, `memory_search_handler`
- **Chat APIs**: `chat_send_handler`, `chat_history_handler`, `chat_threads_handler`, `chat_new_thread_handler`, `chat_events_handler`
- **Jobs APIs**: `jobs_list_handler`, `jobs_detail_handler`, `jobs_cancel_handler`, `jobs_restart_handler`, `jobs_events_handler`
- **Extensions APIs**: `extensions_list_handler`, `extensions_install_handler`, `extensions_uninstall_handler`
- **Skills APIs**: `skills_list_handler`, `skills_install_handler`, `skills_uninstall_handler`
- **Routines APIs**: `routines_list_handler`, `routines_create_handler`, `routines_delete_handler`, `routines_trigger_handler`
- **Logs APIs**: `logs_events_handler`, `logs_level_handler`
- **Approval APIs**: `approve_operation_handler`, `deny_operation_handler`

### 实现流程

#### Desktop Client 新功能实现流程

```
1. 需求分析
   ↓
2. 检查 src/channels/web/handlers/ 是否有对应 API
   ├─ 有 → 创建 Tauri 命令包装器 → 完成
   └─ 无 ↓
3. 检查 crates/ 是否有可复用模块
   ├─ 有 → 依赖该 crate → 完成
   └─ 无 ↓
4. 检查主项目 src/ 是否有相关功能
   ├─ 有 → 考虑提取为共享 crate 或添加 Web API
   └─ 无 → 评估是否为 Desktop Client 特有功能
       ├─ 是 → 独立实现
       └─ 否 → 在主项目中实现，然后复用
```

#### Admin Backend 新功能实现流程

```
1. 需求分析
   ↓
2. 检查主项目 src/ 是否有对应功能
   ├─ 有 → 通过 ironclaw crate 依赖 → 完成
   └─ 无 ↓
3. 检查 crates/ 是否有可复用模块
   ├─ 有 → 依赖该 crate → 完成
   └─ 无 ↓
4. 评估是否需要共享
   ├─ 需要 → 创建共享 crate
   └─ 不需要 → 独立实现
```

### 检查清单

#### Desktop Client 功能实现检查清单

- [ ] 已检查 `src/channels/web/handlers/` 是否有对应 API
- [ ] 已检查 `crates/` 是否有可复用模块
- [ ] 已检查主项目 `src/` 是否有相关功能
- [ ] 已评估是否为 Desktop Client 特有功能
- [ ] 如果复用 Web API，已创建 Tauri 命令包装器
- [ ] 已更新 `DESKTOP_CLIENT_FEATURE_CHECKLIST.md`
- [ ] 已添加相关测试
- [ ] 已验证功能正常工作

#### Admin Backend 功能实现检查清单

- [ ] 已检查主项目 `src/` 是否有对应功能
- [ ] 已检查 `crates/` 是否有可复用模块
- [ ] 已评估是否需要创建共享 crate
- [ ] 如果依赖主项目，已正确配置 `Cargo.toml`
- [ ] 已实现错误类型映射（如需要）
- [ ] 已添加相关测试
- [ ] 已验证功能正常工作

### 示例

#### 好的实践 ✅

**Desktop Client 复用 Web API**：
```rust
// desktop-client/src-tauri/src/commands/logs.rs
#[tauri::command]
pub async fn get_logs(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<LogEntry>, String> {
    // 调用 Web Gateway API
    let url = format!("{}/api/logs", state.gateway_url);
    let response = state.http_client
        .get(&url)
        .query(&[("limit", limit.unwrap_or(200))])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    response.json().await.map_err(|e| e.to_string())
}
```

**Admin Backend 复用主项目功能**：
```rust
// admin-backend/src/handlers.rs
use ironclaw::agent::Agent;
use ironclaw::db::Database;

pub async fn create_agent(db: &Database) -> Result<Agent, Error> {
    // 直接使用主项目的 Agent 和 Database
    Agent::new(db).await
}
```

**复用共享 Crate**：
```rust
// desktop-client/src-tauri/src/auth.rs
use ironclaw_auth::AuthManager;

pub fn verify_password(password: &str, hash: &str) -> Result<(), AuthError> {
    let auth = AuthManager::new("secret".to_string());
    auth.verify_password(password, hash)
}
```

#### 不好的实践 ❌

**重复实现已有功能**：
```rust
// ❌ 错误：重新实现密码哈希（ironclaw_auth 已有）
pub fn hash_password(password: &str) -> String {
    // 重复实现 Argon2 哈希...
}

// ❌ 错误：重新实现日志查询（Web API 已有）
pub async fn query_logs() -> Vec<LogEntry> {
    // 直接查询数据库，而不是调用 Web API...
}
```

### 注意事项

1. **避免重复实现**
   - 重复实现会导致维护成本增加
   - 可能引入不一致的行为
   - 浪费开发时间

2. **保持架构清晰**
   - Desktop Client → Web API → 主项目核心功能
   - Admin Backend → 主项目 Crate → 核心功能
   - 共享逻辑 → 共享 Crate

3. **版本同步**
   - 主项目更新时，Desktop Client 和 Admin Backend 自动受益
   - 共享 Crate 更新时，需要同步更新所有使用方

4. **测试覆盖**
   - 主项目的测试覆盖核心功能
   - Desktop Client 和 Admin Backend 测试集成和 UI 层

## Skills System

本项目在 `.trae/skills/` 目录下定义了一系列可复用的技能模块，用于指导 AI Agent 执行特定类型的任务。每个技能都包含详细的指导原则、工具支持和最佳实践。

### 可用技能

当前可用的技能包括：

1. **Code Quality Gate** (`code-quality-gate`) - 全面的代码质量门禁检查
2. **Engineer Mindset Coding** (`engineer-mindset-coding`) - 符合工程最佳实践的生产级代码
3. **TDD Practitioner** (`tdd-practitioner`) - 测试驱动开发方法论

### 使用方式

- **查看技能详情**：每个技能在 `.trae/skills/<skill-name>/SKILL.md` 文件中都有完整的文档
- **调用技能**：在与 AI Agent 交互时，可以明确引用技能名称（如 "使用 TDD Practitioner 技能实现此功能"）
- **技能协作**：这些技能可以组合使用，例如：
  - `engineer-mindset-coding` 定义代码结构和质量标准
  - `tdd-practitioner` 通过测试驱动实现这些标准
  - `code-quality-gate` 验证最终代码符合所有质量要求

### 扩展技能

如需添加新技能，在 `.trae/skills/` 下创建新目录，包含 `SKILL.md` 文件，遵循现有技能的 YAML front-matter 格式。

## MCP Interactive Feedback 规则

**强制要求**：每次AI Agent执行完成前必须调用MCP Interactive Feedback工具，确保与用户保持交互，及时获取反馈，避免遗漏关键步骤，提高任务执行质量。

### 执行规范

1. **调用时机**：
   - 在任务开始执行时，首先调用MCP Interactive Feedback工具
   - 在任务执行过程中，根据用户反馈调整后再次调用
   - 在任务最终完成前，必须调用MCP Interactive Feedback工具进行确认

2. **工具调用**：
   - 使用 `mcp_mcp_feedback_enhanced_interactive_feedback` 工具（已验证可用）
   - 必需参数：`summary`（任务执行摘要）、`project_directory`（项目目录）
   - 可选参数：`timeout`（超时时间，默认600秒）

3. **反馈处理**：
   - 如果用户反馈包含具体指令，必须按照指令调整执行
   - 如果用户反馈为"好的执行"或类似确认，可以继续执行
   - 如果用户反馈指出问题，必须修正问题后重新调用

4. **验证机制**：
   - 每次Agent执行必须包含至少一次MCP Interactive Feedback调用
   - 未调用MCP Interactive Feedback的Agent执行视为不完整
   - 用户可以通过检查执行日志验证是否遵循此规则

## 共享代码架构规则

**原则**：当 client（desktop-client）和 backend（admin-backend）应用层有相同的逻辑时，应该在 `crates/` 目录下创建共享 crate。

### 识别共享逻辑的场景

1. **认证和授权**
   - 密码哈希验证（Argon2）
   - JWT token 生成和验证
   - 示例：`crates/ironclaw_auth/`

2. **数据验证和清理**
   - 输入验证规则
   - 数据清理和转换
   - 示例：`crates/ironclaw_safety/`

3. **通用业务逻辑**
   - 跨应用的领域模型
   - 共享的计算逻辑
   - 通用的错误处理

4. **协议和格式**
   - 序列化/反序列化逻辑
   - 通信协议实现
   - 数据格式转换

### 创建共享 Crate 的流程

```
1. 识别重复代码
   ↓
2. 评估是否适合共享（逻辑相同且稳定）
   ↓
3. 在 crates/ 下创建新 crate
   ↓
4. 参考现有 crate 架构（如 ironclaw_safety）
   ↓
5. 实现共享逻辑（遵循 TDD）
   ↓
6. 更新 workspace Cargo.toml
   ↓
7. 迁移应用层代码使用共享 crate
   ↓
8. 实现错误类型映射（如需要）
   ↓
9. 运行完整测试验证
   ↓
10. 更新文档和检查清单
```

### 共享 Crate 架构规范

参考 `crates/ironclaw_safety/` 和 `crates/ironclaw_auth/` 的模式：

```
crates/your_crate/
├── Cargo.toml           # 依赖配置
├── src/
│   ├── lib.rs          # 公共 API 和统一管理器
│   ├── error.rs        # 错误类型定义
│   ├── module1.rs      # 功能模块1
│   ├── module2.rs      # 功能模块2
│   └── ...
└── tests/              # 集成测试（可选）
```

### 应用层集成规范

1. **依赖声明**
   ```toml
   [dependencies]
   your_crate = { path = "../../crates/your_crate" }
   ```

2. **错误类型映射**
   ```rust
   impl From<your_crate::Error> for AppError {
       fn from(err: your_crate::Error) -> Self {
           // 映射到应用层错误类型
       }
   }
   ```

3. **API 重导出**（可选）
   ```rust
   pub use your_crate::{Manager, Config};
   ```

### 检查清单

- [ ] 已识别重复的逻辑代码
- [ ] 已评估逻辑是否适合共享
- [ ] 已参考现有共享 crate 的架构
- [ ] 已在 crates/ 下创建新 crate
- [ ] 已实现完整的单元测试
- [ ] 已更新 workspace Cargo.toml
- [ ] 已迁移所有应用层代码
- [ ] 已实现错误类型映射
- [ ] 所有测试通过
- [ ] 编译无错误和警告

### 注意事项

- **不要过度抽象**：只有当逻辑真正相同且稳定时才共享
- **保持独立性**：共享 crate 不应依赖应用层代码
- **版本管理**：共享 crate 的 API 变更需要同步更新所有使用方
- **文档完善**：共享 crate 需要有清晰的文档和使用示例

## Rust 代码开发规则

**强制要求**：每次为 desktop-client 或其他 Rust 项目添加新的功能或逻辑代码时，必须遵循以下技能和流程。

### 适用范围

- 新增 Rust 函数、方法、模块或结构体
- 修改现有逻辑的重大重构
- 添加新的 API 端点或命令处理器
- 不适用于：仅修复 bug 的最小改动、注释更新、格式调整

### 必须遵循的技能

1. **TDD Practitioner** (`tdd-practitioner`)
   - 先编写测试用例（Red 阶段）
   - 再实现功能代码（Green 阶段）
   - 最后重构优化（Refactor 阶段）
   - 所有单元测试必须通过

2. **Engineer Mindset Coding** (`engineer-mindset-coding`)
   - 代码结构清晰，易于维护
   - 完整的错误处理和边界情况处理
   - 充分的代码注释和文档
   - 遵循 Rust 最佳实践和项目约定

3. **Code Quality Gate** (`code-quality-gate`)
   - 代码审查检查清单
   - 性能和安全性评估
   - 编译警告检查（应为 0 个警告）
   - 测试覆盖率检查

### 执行流程

```
1. 理解需求
   ↓
2. 查看技能文档（.trae/skills/）
   ↓
3. 编写测试用例（TDD - Red）
   ↓
4. 实现功能代码（TDD - Green）
   ↓
5. 代码重构优化（TDD - Refactor）
   ↓
6. 运行 cargo build 完整编译验证
   ↓
7. 运行单元测试验证
   ↓
8. 执行代码质量检查
   ↓
9. 更新相关文档和检查清单
```

### 检查清单

- [ ] 已查看相关技能文档
- [ ] 已编写单元测试（测试优先）
- [ ] 已实现功能代码
- [ ] 已进行代码重构优化
- [ ] 已运行 cargo build 验证编译
- [ ] 编译通过且无错误（警告应为 0）
- [ ] 所有单元测试通过
- [ ] 代码符合项目约定和最佳实践
- [ ] 已更新 DESKTOP_CLIENT_FEATURE_CHECKLIST.md
- [ ] 已更新相关代码注释和文档

## 外部库API使用验证策略

**目的**：防止因使用错误的API版本或模式而导致大量编译错误。

### 问题案例

在desktop-client开发中，曾因使用错误的libSQL 0.6 API导致59个编译错误：
- 错误使用：`Connection::open()` 直接打开连接
- 正确使用：`libsql::Builder::new_local(path).build().await` 然后 `db.connect()`

### 预防策略

1. **查找现有用法**
   - 在实现新功能前，先搜索项目中该库的现有使用模式
   - 使用 `grep` 或 `rg` 搜索关键API调用
   - 示例：`rg "libsql::" src/` 查找libSQL的使用方式

2. **参考项目代码**
   - 优先参考项目内已有的实现，而不是外部文档
   - 项目代码反映了实际使用的版本和配置
   - 示例：参考 `src/db/libsql/mod.rs` 了解正确的libSQL使用模式

3. **完整编译验证**
   - 使用 `getDiagnostics` 进行快速检查
   - **必须**使用 `cargo build` 进行完整编译验证
   - `getDiagnostics` 可能遗漏跨模块的类型错误

4. **分阶段实现**
   - 先实现核心功能并验证编译
   - 再逐步添加辅助功能
   - 避免一次性添加大量未验证的代码

5. **依赖版本检查**
   - 检查 `Cargo.toml` 中的依赖版本
   - 注意主版本号变化可能带来的breaking changes
   - 查看依赖的CHANGELOG了解API变更

### 工作流程

```
1. 需要使用外部库API
   ↓
2. 搜索项目中该库的现有用法
   ↓
3. 参考现有代码实现新功能
   ↓
4. 使用 getDiagnostics 快速检查
   ↓
5. 使用 cargo build 完整验证
   ↓
6. 如有错误，对比现有用法找出差异
   ↓
7. 修正后重新验证
```

### 检查清单

- [ ] 已搜索项目中该库的现有使用模式
- [ ] 已参考项目内类似功能的实现
- [ ] 已检查 Cargo.toml 中的依赖版本
- [ ] 已使用 cargo build 完整编译验证
- [ ] 编译通过且无错误（警告可接受）
- [ ] 对于新增功能，已按照"Rust 代码开发规则"执行 TDD 和单元测试


## 浏览器端到端（E2E）测试规则

**强制要求**：前端代码编写完成后，必须按照本规则编写 E2E 测试，确保浏览器环境中的功能正常运行。

### 适用范围

- SSE 连接和消息接收
- UI 交互和状态管理
- 本地存储和 Cookie 管理
- 网络请求和错误处理
- 跨浏览器兼容性

### 推荐的测试框架

#### 1. Cypress（推荐用于 UI 测试）

**优点**：
- 最简单的 API
- 优秀的调试工具
- 自动等待
- 实时重新加载

**使用场景**：
- UI 交互测试
- 表单验证
- 页面导航
- 状态管理

**示例**：
```javascript
describe('Chat UI Tests', () => {
  beforeEach(() => {
    cy.visit('http://localhost:5173');
  });

  it('should display chat interface', () => {
    cy.get('.chat-container').should('be.visible');
    cy.get('.message-input').should('be.visible');
  });

  it('should send message', () => {
    cy.get('.message-input').type('Hello');
    cy.get('.send-button').click();
    cy.get('.message-item').should('contain', 'Hello');
  });
});
```

#### 2. Playwright（推荐用于 SSE 测试）

**优点**：
- 完整的浏览器自动化
- 支持多个浏览器
- 可以测试 SSE 连接
- 支持截图和录制

**使用场景**：
- SSE 连接测试
- 网络中断模拟
- 跨浏览器测试
- 性能测试

**示例**：
```rust
#[tokio::test]
async fn test_sse_connection() {
    let playwright = Playwright::new();
    let browser = playwright.chromium().launch().await.unwrap();
    let page = browser.new_page().await.unwrap();
    
    page.goto("http://localhost:5173").await.unwrap();
    
    // 等待 SSE 连接
    page.wait_for_selector(".sse-connected").await.unwrap();
    
    // 验证连接状态
    let status = page.text_content(".sse-status").await.unwrap();
    assert_eq!(status, Some("Connected".to_string()));
}
```

### 测试编写流程

#### 第一步：准备测试环境

```bash
# 1. 启动后端服务
cargo run -- run --cli-only --no-onboard &

# 2. 启动前端开发服务器
cd desktop-client/src-ui
npm run dev &

# 3. 运行测试
npm run test:e2e
```

#### 第二步：编写测试用例

```javascript
// cypress/e2e/feature.cy.js

describe('Feature Tests', () => {
  beforeEach(() => {
    cy.visit('http://localhost:5173');
    cy.get('.app-container').should('be.visible');
  });

  it('should work correctly', () => {
    // 测试逻辑
  });
});
```

#### 第三步：验证测试通过

```bash
# 运行所有 E2E 测试
npm run test:e2e

# 运行特定测试
npm run test:e2e -- --spec "cypress/e2e/feature.cy.js"

# 调试模式
npm run test:e2e -- --headed
```

### 测试检查清单

#### 功能测试

- [ ] 页面加载正确
- [ ] UI 元素显示正确
- [ ] 用户交互有效
- [ ] 数据显示正确
- [ ] 错误处理正确

#### SSE 测试

- [ ] SSE 连接建立
- [ ] 消息接收正确
- [ ] 消息显示正确
- [ ] 重连机制工作
- [ ] 错误处理正确

#### 性能测试

- [ ] 页面加载时间 < 3s
- [ ] 消息显示延迟 < 100ms
- [ ] 内存使用稳定
- [ ] CPU 使用合理

#### 兼容性测试

- [ ] Chrome 浏览器
- [ ] Firefox 浏览器
- [ ] Safari 浏览器
- [ ] Edge 浏览器

### 常见测试模式

#### 1. 等待元素出现

```javascript
// 等待元素出现（最多 5 秒）
cy.get('.message-item', { timeout: 5000 }).should('be.visible');

// 等待元素包含特定文本
cy.get('.status').should('contain', 'Connected');
```

#### 2. 模拟用户交互

```javascript
// 输入文本
cy.get('.input').type('Hello World');

// 点击按钮
cy.get('.button').click();

// 选择下拉菜单
cy.get('.select').select('Option 1');
```

#### 3. 验证网络请求

```javascript
// 拦截 API 请求
cy.intercept('GET', '**/api/messages', {
  statusCode: 200,
  body: [{ id: 1, content: 'Test' }]
});

// 验证请求被发送
cy.intercept('POST', '**/api/messages').as('sendMessage');
cy.get('.send-button').click();
cy.wait('@sendMessage');
```

#### 4. 模拟网络错误

```javascript
// 模拟网络中断
cy.intercept('GET', '**/api/events', { forceNetworkError: true });

// 模拟服务器错误
cy.intercept('GET', '**/api/events', {
  statusCode: 500,
  body: 'Internal Server Error'
});
```

#### 5. 测试 SSE 连接

```javascript
// 等待 SSE 连接建立
cy.get('.sse-status').should('contain', 'Connected');

// 等待消息接收
cy.get('.message-item').should('have.length.greaterThan', 0);

// 模拟网络中断
cy.intercept('GET', '**/api/events', { forceNetworkError: true });

// 等待重连
cy.get('.sse-status').should('contain', 'Reconnecting');
```

### 最佳实践

#### 1. 使用 Page Objects 模式

```javascript
// cypress/support/pages/ChatPage.js
export class ChatPage {
  visit() {
    cy.visit('http://localhost:5173');
  }

  sendMessage(text) {
    cy.get('.message-input').type(text);
    cy.get('.send-button').click();
  }

  getMessages() {
    return cy.get('.message-item');
  }
}

// cypress/e2e/chat.cy.js
import { ChatPage } from '../support/pages/ChatPage';

describe('Chat Tests', () => {
  const page = new ChatPage();

  it('should send message', () => {
    page.visit();
    page.sendMessage('Hello');
    page.getMessages().should('contain', 'Hello');
  });
});
```

#### 2. 使用测试数据工厂

```javascript
// cypress/support/factories.js
export function createMessage(overrides = {}) {
  return {
    id: Math.random(),
    content: 'Test message',
    timestamp: new Date(),
    ...overrides
  };
}

// cypress/e2e/messages.cy.js
it('should display messages', () => {
  const messages = [
    createMessage({ content: 'First' }),
    createMessage({ content: 'Second' })
  ];
  
  cy.intercept('GET', '**/api/messages', messages);
  cy.visit('http://localhost:5173');
  cy.get('.message-item').should('have.length', 2);
});
```

#### 3. 使用自定义命令

```javascript
// cypress/support/commands.js
Cypress.Commands.add('login', (username, password) => {
  cy.get('.username-input').type(username);
  cy.get('.password-input').type(password);
  cy.get('.login-button').click();
  cy.get('.app-container').should('be.visible');
});

// cypress/e2e/auth.cy.js
it('should login successfully', () => {
  cy.login('user@example.com', 'password');
  cy.get('.user-menu').should('be.visible');
});
```

### 文件结构

```
desktop-client/
├── cypress/
│   ├── e2e/
│   │   ├── chat.cy.js           # 聊天功能测试
│   │   ├── sse_connection.cy.js # SSE 连接测试
│   │   ├── auth.cy.js           # 认证测试
│   │   └── performance.cy.js    # 性能测试
│   ├── support/
│   │   ├── commands.js          # 自定义命令
│   │   ├── pages/               # Page Objects
│   │   └── factories.js         # 测试数据工厂
│   └── cypress.config.js        # Cypress 配置
├── tests/
│   └── e2e_sse_browser_tests.rs # Rust E2E 测试
└── scripts/
    └── run-e2e-tests.sh         # E2E 测试启动脚本
```

### 参考资源

- `E2E_TESTING_BROWSER_ENVIRONMENT.md` - 详细的 E2E 测试方案
- `desktop-client/cypress/e2e/sse_connection.cy.js` - Cypress 测试示例
- `desktop-client/tests/e2e_sse_browser_tests.rs` - Rust E2E 测试示例
- `scripts/run-e2e-tests.sh` - E2E 测试启动脚本

### 执行流程

```
前端功能开发完成
  ↓
编写 E2E 测试用例
  ↓
启动测试环境
  ↓
运行 E2E 测试
  ↓
验证测试通过
  ↓
提交代码
```

### 检查清单

- [ ] 已编写 E2E 测试用例
- [ ] 已启动测试环境（后端 + 前端）
- [ ] 已运行 E2E 测试
- [ ] 所有测试通过
- [ ] 已验证跨浏览器兼容性
- [ ] 已检查性能指标
- [ ] 已更新测试文档
