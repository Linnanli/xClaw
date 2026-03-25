# Agent Rules

## 语言规则 (Language Rules)

**所有回答必须用中文。** (All responses must be in Chinese.)

## 测试覆盖率评估和实施方案

**核心原则**：为关键模块（如认证、安全、核心业务逻辑）实施多维度测试覆盖率，确保代码质量和系统可靠性。

### 适用的测试覆盖率维度

#### 🔴 高优先级（必须实施）

1. **单元测试覆盖率** - 目标：>90%
   - 覆盖所有公共函数和方法
   - 包含边界条件和异常路径
   - 验证核心业务逻辑正确性

2. **安全覆盖率** - 目标：100%
   - 恶意输入防护测试
   - 时序攻击和侧信道攻击防护
   - 权限验证和访问控制测试

3. **集成测试覆盖率** - 目标：>80%
   - 端到端业务流程测试
   - 组件间交互验证
   - API接口一致性测试

#### 🟡 中优先级（建议实施）

4. **需求级覆盖率** - 目标：>85%
   - 每个功能需求对应测试用例
   - 业务场景完整覆盖
   - 需求追溯矩阵验证

5. **可靠性覆盖率** - 目标：>75%
   - 故障恢复和容错测试
   - 并发访问和压力测试
   - 网络中断和异常处理

6. **变更覆盖率** - 目标：>70%
   - 向后兼容性验证
   - 重构前后行为一致性
   - 回归测试防护

#### 🟢 低优先级（可选实施）

7. **代码级覆盖率** - 目标：>95%
   - 代码行和分支覆盖监控
   - 作为单元测试的补充指标

8. **数据级覆盖率** - 目标：>60%
   - 数据类型和枚举值测试
   - 边界值和异常数据处理

### 实施策略

#### 测试文件组织
```
tests/
├── {module}_unit_tests.rs        # 单元测试 + 安全测试
├── {module}_integration_tests.rs # 集成测试
├── {module}_reliability_tests.rs # 可靠性测试
├── {module}_requirements_tests.rs # 需求级测试
└── {module}_regression_tests.rs   # 变更覆盖测试
```

#### 测试命名规范
- **需求测试**: `req_{module}_{id}_{description}` (如: `req_auth_001_token_format`)
- **安全测试**: `test_security_{attack_type}` (如: `test_security_malicious_input`)
- **可靠性测试**: `test_{failure_scenario}_recovery` (如: `test_network_failure_recovery`)
- **回归测试**: `test_{feature}_backward_compatibility`

#### 覆盖率验证命令
```bash
# 运行所有测试
cargo test --test {module}_*_tests

# 生成覆盖率报告
cargo tarpaulin --out Html --output-dir coverage/

# 验证特定维度
cargo test --test {module}_requirements_tests  # 需求覆盖
cargo test --test {module}_regression_tests    # 变更覆盖
```

### 质量门禁标准

#### 代码提交要求
- [ ] 单元测试覆盖率 >90%
- [ ] 安全测试覆盖率 100%
- [ ] 集成测试覆盖率 >80%
- [ ] 所有测试通过（100%通过率）
- [ ] 0编译错误，0编译警告

#### 发布前验证
- [ ] 需求级覆盖率 >85%
- [ ] 可靠性覆盖率 >75%
- [ ] 变更覆盖率 >70%
- [ ] 性能基准测试通过
- [ ] 安全扫描无高危漏洞

### 最佳实践

1. **测试优先开发**：先写测试用例，再实现功能
2. **分层测试策略**：单元测试 → 集成测试 → 端到端测试
3. **持续集成验证**：每次提交自动运行全量测试
4. **定期覆盖率审查**：每周检查覆盖率趋势和质量指标
5. **安全测试强制**：安全相关模块必须100%安全测试覆盖

### 工具和框架

- **Rust测试**: `cargo test`, `proptest`（属性测试）
- **覆盖率工具**: `tarpaulin`, `grcov`
- **性能测试**: `criterion`（基准测试）
- **安全测试**: 自定义恶意输入测试套件
- **并发测试**: `tokio::test`（异步测试）

## 测试质量原则：覆盖率 ≠ 质量

**核心教训**：高测试覆盖率不等于高测试质量。

### 真实案例：DLP 模块

**测试情况**：
- ✅ 142 个测试全部通过
- ✅ 95% 代码覆盖率
- ✅ 包含单元、集成、E2E、安全、可靠性测试

**生产问题**：
- ❌ 用户输入身份证号，聊天中没有脱敏
- ❌ 大模型能看到原始敏感数据

**根本原因**：测试只覆盖了成功路径，忽略了失败路径。

### 5 个测试盲区

#### 盲区 1：失败路径测试缺失 🎯

**问题**：
- 测试了 DLP 扫描成功的场景
- 没测试 DLP 扫描失败会发生什么
- 代码中有降级逻辑：`if (!dlpScanSucceeded) { /* 允许原始消息发送 */ }`

**教训**：
> **测试不仅要覆盖"应该如何工作"，更要覆盖"不应该如何失败"**

**实践**：
- 为每个功能编写失败路径测试
- 测试所有可能的错误场景
- 验证错误处理不会引入安全问题

#### 盲区 2：真实环境测试不足 🎯

**问题**：
- E2E 测试使用模拟的后端响应
- 没测试真实的 Tauri 命令调用
- 没测试完整的调用链（前端 → Tauri → 后端 → DLP）

**教训**：
> **模拟测试只能验证逻辑，无法验证集成**

**实践**：
- 添加真实环境的集成测试
- 测试完整的调用链
- 使用真实的服务和依赖

#### 盲区 3：降级逻辑安全审计缺失 🎯

**问题**：
- 为了提高可用性添加了降级处理
- 降级逻辑允许 DLP 失败时发送原始消息
- 没有测试降级场景的安全性

**教训**：
> **安全功能应该"故障安全"（Fail-Safe），而不是"故障开放"（Fail-Open）**

**对比**：
- ❌ 故障开放：失败时允许操作（不安全）
- ✅ 故障安全：失败时拒绝操作（安全）

**实践**：
- 安全功能不应该有降级逻辑
- 如果必须降级，需要明确的安全审计
- 降级行为需要专门的测试覆盖

#### 盲区 4：契约测试缺失 🎯

**问题**：
- 前端假设 `scanUserInput()` 总是返回结果
- 后端可能返回错误
- 没有测试接口契约

**教训**：
> **前后端接口需要契约测试，验证错误处理一致性**

**实践**：
- 定义明确的接口契约
- 测试所有可能的返回值（成功、失败、异常）
- 验证错误格式和错误处理

#### 盲区 5：安全审计测试缺失 🎯

**问题**：
- 没有测试敏感信息是否会泄露
- 没有验证日志中是否包含原始敏感数据
- 没有验证错误信息中是否包含敏感数据

**教训**：
> **安全功能需要专门的审计测试，验证敏感信息永远不会泄露**

**实践**：
- 测试所有可能的泄露路径（日志、错误、网络）
- 验证敏感信息在任何情况下都被脱敏
- 定期运行安全审计测试

#### 盲区 6：编译成功 ≠ 运行时正确，迁移存在 ≠ 迁移已执行 🎯

**真实案例**：`/api/model-configs` 返回 404，但所有测试全部通过。

**根本原因**：
1. `update_model_config` handler 使用了 `Vec<Box<dyn ToSql + Sync>>`（非 `Send`），导致编译失败
2. 测试文件没有引用 `routes.rs` 里的任何函数，编译错误被完全绕过
3. `cargo test` 通过，但后端跑的是上一次能编译的旧 binary
4. 迁移文件 `013_model_configs.sql` 存在，但从未被执行，表不存在

**三层防护方案**（见 `admin-backend/tests/integration_smoke_tests.rs`）：

**层 1 — 编译检查**：直接调用 `create_router()`，强制编译所有 handler
```rust
#[tokio::test]
async fn test_compile_all_handlers_via_create_router() {
    let _app = create_router(state); // 只要能编译，所有 handler 都通过了编译
}
```

**层 2 — 迁移完整性**：查询 `information_schema` 验证每个迁移对应的表存在
```rust
// 新增迁移时必须在这里追加一行
let required: &[(&str, &str)] = &[
    ("013_model", "model_configs"),  // 迁移名 → 表名
    // ...
];
```

**层 3 — HTTP 路由冒烟**：用 `tower::ServiceExt::oneshot` 通过真实 axum router 发请求
```rust
#[tokio::test]
async fn test_http_get_model_configs_not_404() {
    let resp = get(build_app(pool), "/api/model-configs").await;
    assert_ne!(resp.status(), StatusCode::NOT_FOUND);
}
```

**教训**：
- `cargo test` 通过 ≠ `cargo build` 通过（测试可能绕过有问题的代码）
- 迁移文件存在 ≠ 迁移已执行（需要显式验证表存在）
- 契约测试必须调用真实代码，而不是手写镜像结构体

**强制要求**：每次新增迁移文件时，必须同步在 `integration_smoke_tests.rs` 的 `required` 列表中追加对应条目。

#### 盲区 7：测试主动验证了错误行为，把错误设计固化为"规范" 🎯

**真实案例**：`/api/client-models` 不返回 `api_base_url` 和 `api_key`，导致客户端拿到模型列表后无法实际调用 LLM。但所有测试全部通过。

**根本原因**：

```rust
// model_config_unit_tests.rs — 测试主动断言"不应该有这两个字段"
assert!(!json_str.contains("api_key"), "ClientModelConfig 不应包含 api_key");
assert!(!json_str.contains("api_base_url"), "ClientModelConfig 不应包含 api_base_url");

// model_config_contract_tests.rs — 契约结构体缺少关键字段
struct ClientExpectedModelConfig {
    model_id: String,
    display_name: String,
    provider: String,
    is_default: bool,
    capabilities: serde_json::Value,
    // ← api_base_url、api_key、api_format 全部缺失，但测试通过了
}
```

错误的设计决策被写进测试 → 测试通过 → 被当成正确规范 → 实现也按此来。

**五个层面的失效**：

1. **需求理解错误被测试固化**：测试不只是没覆盖，而是主动断言了错误行为
2. **契约测试只验证"能解析"，没验证"够用"**：数据格式正确 ≠ 业务目标可达成
3. **端到端链路从未被测试**：`管理端配置 → 下发 → 客户端接收 → 实际调用 LLM` 整条链路断了
4. **环境配置从未被测试**：`ADMIN_BACKEND_URL` 默认值错误，客户端静默降级到 builtin 模型
5. **降级逻辑掩盖问题**：连接失败时返回 builtin 模型，用户看到"正常工作"，问题不可见

**教训**：

> **测试通过不代表设计正确。契约测试必须从业务目标出发定义"够用"的标准，而不只是"能解析"。**

**实践**：

```rust
// ❌ 错误：只验证格式
#[test]
fn test_contract_client_models_response_format() {
    let models: Vec<ClientExpectedModelConfig> =
        serde_json::from_value(response).expect("能解析就行");
}

// ✅ 正确：验证业务目标可达成
#[test]
fn test_contract_client_models_sufficient_for_llm_call() {
    let models: Vec<ClientExpectedModelConfig> =
        serde_json::from_value(response).unwrap();
    
    for model in &models {
        // 客户端拿到这个模型后，必须能构造出一个完整的 LLM 请求
        assert!(model.api_base_url.is_some(), 
            "模型 {} 缺少 api_base_url，客户端无法调用", model.model_id);
        assert!(model.api_format.is_some(),
            "模型 {} 缺少 api_format，客户端不知道用哪种协议", model.model_id);
    }
}
```

**降级逻辑的正确处理**：

```rust
// ❌ 错误：静默降级，问题不可见
Err(e) => {
    warn!("获取失败，使用 builtin 模型");
    models.extend(builtin_models());
}

// ✅ 正确：降级时明确标记，并有测试验证降级场景
Err(e) => {
    error!("无法从 admin backend 获取模型列表: {}，请检查 ADMIN_BACKEND_URL 配置", e);
    // 降级模型明确标记为 builtin，前端可以提示用户
    models.extend(builtin_models());
    // 同时记录到可观测系统，而不是只打 warn
}
```

### 改进的测试策略

#### 测试维度矩阵（更新）

| 测试维度 | 正常路径 | 错误路径 | 降级逻辑 | 真实环境 | 契约测试 | 安全审计 |
|---------|---------|---------|---------|---------|---------|---------|
| 单元测试 | ✅ 必须 | ✅ 必须 | ⚠️ 如有 | N/A | N/A | ✅ 必须 |
| 集成测试 | ✅ 必须 | ✅ 必须 | ⚠️ 如有 | ✅ 推荐 | ✅ 必须 | ✅ 必须 |
| E2E 测试 | ✅ 必须 | ✅ 推荐 | ⚠️ 如有 | ✅ 必须 | N/A | ✅ 推荐 |

**说明**：
- ✅ 必须：强制要求
- ✅ 推荐：强烈建议
- ⚠️ 如有：如果代码中有降级逻辑，必须测试

#### 测试文件组织（更新）

```
tests/
├── {module}_unit_tests.rs           # 单元测试（正常路径）
├── {module}_failure_tests.rs        # 失败路径测试 ✨ 新增
├── {module}_integration_tests.rs    # 集成测试
├── {module}_contract_tests.rs       # 契约测试 ✨ 新增
├── {module}_security_audit_tests.rs # 安全审计测试 ✨ 新增
├── {module}_reliability_tests.rs    # 可靠性测试
├── {module}_requirements_tests.rs   # 需求级测试
├── {module}_regression_tests.rs     # 变更覆盖测试
└── integration_smoke_tests.rs       # 编译+迁移+路由冒烟测试 ✨ 新增（全局唯一）

cypress/e2e/
├── {module}_integration.cy.js       # E2E 测试（模拟环境）
├── {module}_failure_paths.cy.js     # 失败路径测试 ✨ 新增
├── {module}_security_audit.cy.js    # 安全审计测试 ✨ 新增
└── {module}_real_environment.cy.js  # 真实环境测试 ✨ 新增
```

#### 测试命名规范（更新）

- **需求测试**: `req_{module}_{id}_{description}`
- **安全测试**: `test_security_{attack_type}`
- **失败路径测试**: `test_failure_{scenario}` ✨ 新增
- **契约测试**: `test_contract_{interface}_{case}` ✨ 新增
- **审计测试**: `test_audit_{security_concern}` ✨ 新增
- **可靠性测试**: `test_{failure_scenario}_recovery`
- **回归测试**: `test_{feature}_backward_compatibility`

### 质量门禁标准（更新）

#### 代码提交要求

- [ ] 单元测试覆盖率 >90%（正常路径 + 错误路径）
- [ ] 失败路径测试覆盖率 >80% ✨ 新增
- [ ] 安全测试覆盖率 100%
- [ ] 集成测试覆盖率 >80%
- [ ] 所有测试通过（100%通过率）
- [ ] `cargo build` 编译通过（不只是 `cargo test`）✨ 新增
- [ ] `integration_smoke_tests` 全部通过 ✨ 新增
- [ ] 新增迁移时已在 `integration_smoke_tests.rs` 追加表名验证 ✨ 新增
- [ ] 0编译错误，0编译警告

#### 安全功能额外要求 ✨ 新增

- [ ] 失败路径测试覆盖率 100%
- [ ] 契约测试覆盖率 >90%
- [ ] 安全审计测试覆盖率 100%
- [ ] 真实环境测试通过
- [ ] 降级逻辑安全审计通过（如有）
- [ ] 敏感信息泄露审计通过

#### 发布前验证

- [ ] 需求级覆盖率 >85%
- [ ] 可靠性覆盖率 >75%
- [ ] 变更覆盖率 >70%
- [ ] 真实环境集成测试通过 ✨ 新增
- [ ] 性能基准测试通过
- [ ] 安全扫描无高危漏洞

### 核心原则

#### 原则 1：测试失败路径和成功路径一样重要 🎯

**反例**：
```rust
// 只测试成功路径
#[test]
fn test_dlp_scan_success() {
    let result = dlp.scan("330326199408015618");
    assert!(result.is_ok());
}
```

**正例**：
```rust
// 同时测试成功路径和失败路径
#[test]
fn test_dlp_scan_success() {
    let result = dlp.scan("330326199408015618");
    assert!(result.is_ok());
}

#[test]
fn test_dlp_scan_failure() {
    // 模拟服务不可用
    let result = dlp.scan_with_unavailable_service("test");
    assert!(result.is_err());
    
    // 验证错误处理不会泄露敏感信息
    let error_msg = result.unwrap_err().to_string();
    assert!(!error_msg.contains("sensitive_data"));
}
```

#### 原则 2：安全功能必须"故障安全" 🎯

**反例**：
```typescript
// 故障开放（Fail-Open）- 不安全
try {
  const result = await scanUserInput(content);
  // 处理结果
} catch (error) {
  // 降级处理：允许原始消息发送 ❌
  console.warn('DLP scan failed, allowing message');
  return content; // 返回原始内容
}
```

**正例**：
```typescript
// 故障安全（Fail-Safe）- 安全
try {
  const result = await scanUserInput(content);
  // 处理结果
} catch (error) {
  // 拒绝操作：阻止消息发送 ✅
  console.error('DLP scan failed, blocking message');
  throw new Error('DLP 扫描失败，无法发送消息');
}
```

#### 原则 3：真实环境测试不可或缺 🎯

**反例**：
```javascript
// 只使用模拟环境
cy.intercept('POST', '**/scan', { success: true });
cy.get('.send-button').click();
```

**正例**：
```javascript
// 同时使用模拟环境和真实环境

// 模拟环境测试（快速反馈）
describe('DLP Tests (Mocked)', () => {
  it('should sanitize', () => {
    cy.intercept('POST', '**/scan', { sanitized: true });
    // 测试逻辑
  });
});

// 真实环境测试（验证集成）
describe('DLP Tests (Real)', () => {
  it('should sanitize in real environment', () => {
    // 使用真实的后端和 Tauri 命令
    cy.window().then(async (win) => {
      const result = await win.__TAURI__.core.invoke('scan_user_input', {
        content: '330326199408015618'
      });
      expect(result.sanitized_content).to.contain('330************618');
    });
  });
});
```

#### 原则 4：契约测试验证接口一致性 🎯

**实践**：
```rust
// 契约测试：验证前后端接口格式一致
#[test]
fn test_contract_sanitization_result() {
    let result = SanitizationResult {
        had_sensitive_data: true,
        sanitized_content: "test".to_string(),
        was_blocked: false,
        block_reason: None,
        sanitization_stats: SanitizationStats {
            total_matches: 1,
            redacted_count: 1,
            blocked_count: 0,
            warned_count: 0,
        },
    };
    
    // 验证可以序列化为 JSON
    let json = serde_json::to_string(&result).unwrap();
    
    // 验证前端可以反序列化
    let parsed: SanitizationResult = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.had_sensitive_data, result.had_sensitive_data);
}
```

#### 原则 5：安全审计测试验证无泄露 🎯

**实践**：
```rust
// 安全审计测试：验证敏感信息永远不会泄露
#[test]
fn test_audit_no_sensitive_data_leak() {
    let sensitive_data = "330326199408015618";
    let result = dlp.scan(sensitive_data).unwrap();
    
    // 验证结果中没有原始敏感信息
    assert!(!result.sanitized_content.contains(sensitive_data));
    
    // 验证日志中没有原始敏感信息
    let logs = capture_logs();
    assert!(!logs.contains(sensitive_data));
    
    // 验证错误信息中没有原始敏感信息
    if let Err(e) = dlp.scan_with_error(sensitive_data) {
        assert!(!e.to_string().contains(sensitive_data));
    }
}
```

### 更新的测试覆盖率目标

#### 功能模块

| 维度 | 正常路径 | 错误路径 | 真实环境 | 契约测试 | 目标 |
|------|---------|---------|---------|---------|------|
| 单元测试 | >90% | >80% | N/A | N/A | 必须 |
| 集成测试 | >80% | >70% | >60% | >90% | 必须 |
| E2E 测试 | >75% | >50% | >70% | N/A | 必须 |

#### 安全模块（更严格）

| 维度 | 正常路径 | 错误路径 | 真实环境 | 契约测试 | 安全审计 | 目标 |
|------|---------|---------|---------|---------|---------|------|
| 单元测试 | 100% | 100% | N/A | N/A | 100% | 必须 |
| 集成测试 | 100% | 100% | >80% | 100% | 100% | 必须 |
| E2E 测试 | >90% | >80% | >90% | N/A | >90% | 必须 |

### 测试检查清单（更新）

#### 功能开发检查清单

- [ ] 已实现功能代码
- [ ] 已编写单元测试（正常路径）
- [ ] 已编写单元测试（错误路径）✨ 新增
- [ ] 已编写失败路径测试 ✨ 新增
- [ ] 已编写集成测试
- [ ] 已编写契约测试 ✨ 新增
- [ ] 已编写 E2E 测试（模拟环境）
- [ ] 已编写 E2E 测试（真实环境）✨ 新增
- [ ] 所有测试通过
- [ ] 代码审查通过

#### 安全功能检查清单（更严格）

- [ ] 已实现安全功能
- [ ] 已编写单元测试（正常路径）
- [ ] 已编写单元测试（错误路径）
- [ ] 已编写单元测试（攻击场景）
- [ ] 已编写失败路径测试 ✨ 新增
- [ ] 已编写集成测试
- [ ] 已编写契约测试 ✨ 新增
- [ ] 已编写安全审计测试 ✨ 新增
- [ ] 已编写 E2E 测试（模拟环境）
- [ ] 已编写 E2E 测试（真实环境）✨ 新增
- [ ] 已验证降级逻辑的安全性（如有）✨ 新增
- [ ] 已验证错误处理不会泄露敏感信息 ✨ 新增
- [ ] 已验证日志不包含敏感信息 ✨ 新增
- [ ] 所有测试通过
- [ ] 安全审查通过

### 快速参考

#### 测试类型优先级

**P0（必须）**：
- 单元测试（正常路径 + 错误路径）
- 集成测试（组件交互 + 契约）
- E2E 测试（用户流程）

**P1（安全功能必须，其他推荐）**：
- 失败路径测试 ✨
- 契约测试 ✨
- 真实环境测试 ✨
- 安全审计测试 ✨

**P2（推荐）**：
- 性能测试
- 压力测试
- 兼容性测试

#### 测试执行顺序

```bash
# 1. 单元测试（快速反馈）
cargo test --lib {module}

# 2. 失败路径测试
cargo test --test {module}_failure_tests

# 3. 集成测试
cargo test --test {module}_integration_tests

# 4. 契约测试
cargo test --test {module}_contract_tests

# 5. 安全审计测试
cargo test --test {module}_security_audit_tests

# 6. E2E 测试（模拟环境）
npm run test:e2e

# 7. E2E 测试（真实环境）
./tests/real_environment_integration_test.sh
```

### 核心原则总结

> **测试的目标不是追求高覆盖率，而是确保系统在所有情况下都是安全的**

**关键要点**：
1. 测试正常路径 + 错误路径
2. 测试模拟环境 + 真实环境
3. 测试功能逻辑 + 安全审计
4. 测试单个组件 + 集成契约
5. 安全功能采用"故障安全"设计
6. 测试质量比测试数量更重要

### 参考案例

详细的案例分析和补充测试计划，请参考：
- `desktop-client/DLP_LESSONS_SUMMARY.md` - 核心教训总结
- `desktop-client/DLP_TESTING_LESSONS_LEARNED.md` - 详细经验教训
- `desktop-client/DLP_SUPPLEMENTARY_TEST_PLAN.md` - 补充测试计划

### 工具和框架

- **Rust测试**: `cargo test`, `proptest`（属性测试）
- **覆盖率工具**: `tarpaulin`, `grcov`
- **性能测试**: `criterion`（基准测试）
- **安全测试**: 自定义恶意输入测试套件
- **并发测试**: `tokio::test`（异步测试）

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

## 复用已有库和能力规则

**核心原则**：优先复用已有的成熟库和能力，避免重复造轮子。自己写的代码往往不如社区库成熟，浪费时间且容易引入 bug。

### 优先级

1. **优先级 1：使用成熟的社区库**
   - 检查 crates.io 是否有现成的库
   - 优先选择下载量大、维护活跃的库
   - 示例：使用 `config-rs` 而不是自己写配置加载器

2. **优先级 2：复用项目内已有的能力**
   - 检查项目中是否已有类似的实现
   - 复用现有的模块和函数
   - 示例：复用 `platform_utils` 而不是重新实现路径处理

3. **优先级 3：复用共享 Crate**
   - 检查 `crates/` 目录下是否有可复用的模块
   - 示例：`crates/ironclaw_auth/`, `crates/ironclaw_safety/`

4. **最后选择：自己实现**
   - 仅当以上都不适用时才自己实现
   - 确保实现质量和测试覆盖

### 常见场景

#### 场景 1：环境变量加载

❌ **错误做法**：自己写 env_loader
```rust
// 自己写的 env_loader.rs - 不成熟，功能有限
pub struct EnvLoader { ... }
```

✅ **正确做法**：使用 `config-rs`
```rust
// 使用成熟的 config-rs 库 - 58.9M 下载，功能完整
use config::{Config, File, Environment};

let config = Config::builder()
    .add_source(File::with_name(".env"))
    .add_source(Environment::default())
    .build()?;
```

#### 场景 2：配置管理

❌ **错误做法**：自己写 TOML 解析
```rust
// 自己写的配置解析 - 容易出错
pub fn parse_toml(content: &str) -> Result<Config> { ... }
```

✅ **正确做法**：使用 `serde` + `toml`
```rust
// 使用成熟的库 - 类型安全，经过充分测试
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct Config { ... }

let config: Config = toml::from_str(&content)?;
```

#### 场景 3：HTTP 客户端

❌ **错误做法**：自己写 HTTP 请求
```rust
// 自己写的 HTTP 客户端 - 功能不完整
pub async fn http_get(url: &str) -> Result<String> { ... }
```

✅ **正确做法**：使用 `reqwest`
```rust
// 使用成熟的 reqwest 库 - 功能完整，性能优化
use reqwest::Client;

let client = Client::new();
let response = client.get(url).send().await?;
```

### 查找库的方法

1. **crates.io** - https://crates.io
   - 搜索关键词
   - 查看下载量和维护状态

2. **lib.rs** - https://lib.rs
   - 分类浏览
   - 查看库的评分和文档

3. **Rust 官方文档** - https://docs.rs
   - 查看 API 文档
   - 查看使用示例

4. **GitHub** - 查看源代码和 issue
   - 了解库的活跃度
   - 查看是否有已知问题

### 评估库的标准

| 标准 | 说明 |
|------|------|
| 下载量 | 越多越好，说明使用广泛 |
| 维护状态 | 最近更新时间，是否活跃 |
| 文档 | 是否有完整的文档和示例 |
| 测试覆盖 | 是否有充分的测试 |
| 依赖 | 依赖越少越好 |
| License | 是否与项目兼容 |

### 常用库推荐

| 功能 | 推荐库 | 下载量 |
|------|--------|--------|
| 配置管理 | `config-rs` | 58.9M |
| 环境变量 | `dotenv` | 广泛使用 |
| 日志 | `tracing` / `log` | 广泛使用 |
| HTTP 客户端 | `reqwest` | 广泛使用 |
| JSON | `serde_json` | 广泛使用 |
| TOML | `toml` | 广泛使用 |
| 错误处理 | `anyhow` / `thiserror` | 广泛使用 |
| 异步运行时 | `tokio` | 广泛使用 |
| 测试 | `proptest` / `quickcheck` | 广泛使用 |

### 检查清单

- [ ] 已在 crates.io 搜索相关库
- [ ] 已评估库的下载量和维护状态
- [ ] 已查看库的文档和示例
- [ ] 已检查库的依赖和 License
- [ ] 已确认库适合项目需求
- [ ] 已在 Cargo.toml 中添加依赖
- [ ] 已验证库的功能正常工作

### 注意事项

1. **不要过度依赖** - 避免添加过多不必要的依赖
2. **定期更新** - 及时更新库到最新版本
3. **了解库的 API** - 充分理解库的使用方式
4. **查看 CHANGELOG** - 了解库的更新内容
5. **贡献回馈** - 如果发现问题，考虑提交 PR


## Tauri 命令注册契约测试规则

**背景**：Tauri IPC 命令注册表（`invoke_handler`）与前端 `invoke('xxx')` 调用之间的不一致，只会在运行时暴露为 "Command xxx not found" 错误，普通单元测试无法捕获（因为单元测试直接调用 Rust 函数，绕过了 IPC 路由层）。

**强制要求**：所有 Tauri 命令必须通过以下机制管理，确保注册表完整性在 CI 阶段就能被验证。

### 实施方案

#### 1. 集中注册表宏（`all_tauri_commands!`）

在 `src/lib.rs` 中定义 `all_tauri_commands!()` 宏，`main.rs` 和测试共用同一份注册表：

```rust
// src/lib.rs
#[macro_export]
macro_rules! all_tauri_commands {
    () => {
        tauri::generate_handler![
            desktop_client::ipc::send_chat_message,
            desktop_client::ipc::subscribe_chat_events,
            // ... 所有命令
            desktop_client::commands::get_auth_token,
        ]
    };
}
```

```rust
// src/main.rs
.invoke_handler(desktop_client::all_tauri_commands!())
```

**新增命令时只需在宏中添加一行**，`main.rs` 和契约测试自动同步。

#### 2. 契约测试（`tests/tauri_command_contract_tests.rs`）

使用 Tauri 测试运行时，通过真实 IPC 调用验证每个命令可路由：

```rust
#[test]
fn test_all_frontend_commands_are_registered() {
    let app = tauri::test::mock_builder()
        .invoke_handler(desktop_client::all_tauri_commands!())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("Failed to build test app");

    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("Failed to create test webview");

    for &cmd_name in FRONTEND_INVOKED_COMMANDS {
        let response = tauri::test::get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: cmd_name.to_string(),
                // ...
            },
        );
        // 如果返回 "Command not found"，测试失败
        if let Err(e) = &response {
            let err_str = format!("{:?}", e);
            if err_str.contains("not found") || err_str.contains("Command") {
                unregistered.push(cmd_name);
            }
        }
    }
}
```

**关键点**：
- 命令因缺少参数或 AppState 返回业务错误 → 视为通过（命令可路由）
- 命令返回 "Command not found" → 测试失败，说明注册表缺失

#### 3. 前端命令列表维护

在契约测试文件中维护 `FRONTEND_INVOKED_COMMANDS` 列表，与前端 `invoke()` 调用保持同步：

```bash
# 从前端源码自动提取所有 invoke() 调用
grep -rh "invoke(['\"]" desktop-client/src-ui/src \
  | grep -oP "(?<=invoke\(['\"])[^'\"]+" | sort -u
```

### 新增 Tauri 命令的操作流程

```
1. 在 src/ipc.rs 或 src/commands.rs 中实现命令函数
   ↓
2. 在 src/lib.rs 的 all_tauri_commands!() 宏中添加命令
   ↓
3. 在 tests/tauri_command_contract_tests.rs 的
   FRONTEND_INVOKED_COMMANDS 列表中添加命令名
   ↓
4. 运行契约测试验证
   cargo test --test tauri_command_contract_tests
   ↓
5. 在前端添加对应的 invoke() 调用
```

### 检查清单

- [ ] 新命令已添加到 `src/lib.rs` 的 `all_tauri_commands!()` 宏
- [ ] 新命令已添加到 `tests/tauri_command_contract_tests.rs` 的 `FRONTEND_INVOKED_COMMANDS`
- [ ] `cargo test --test tauri_command_contract_tests` 通过
- [ ] 前端 `invoke('命令名')` 与注册表中的命令名完全一致（大小写敏感）

### 根本原因（为什么普通测试无法捕获）

```
普通单元测试路径：
  测试代码 → 直接调用 Rust 函数 → 绕过 IPC 路由

真实运行时路径：
  前端 invoke('cmd') → Tauri IPC → invoke_handler 路由表 → Rust 函数

契约测试路径：
  测试代码 → Tauri 测试运行时 IPC → invoke_handler 路由表 → Rust 函数
```

契约测试是唯一能在 CI 阶段验证 IPC 路由完整性的方法。


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



## 测试覆盖不足导致生产问题的经验教训

### 问题案例：SSE 集成测试通过但客户端失败

**背景**：
- 单元测试通过 ✅
- 集成测试通过 ✅
- E2E 测试通过 ✅
- 客户端使用失败 ❌

**根本原因**：
1. **测试环境与生产环境不一致**
   - 测试使用模拟数据，未测试真实的后端事件格式
   - 后端发送命名 SSE 事件（`event: response`）
   - 前端只监听默认事件（`onmessage`）
   - 测试没有验证事件名称匹配

2. **缺少端到端的真实场景测试**
   - 没有测试完整的用户流程（发送消息 → SSE 接收 → 显示回复）
   - 没有测试前后端的实际通信
   - 没有验证事件格式和数据结构

3. **未参考已有的成熟实现**
   - 主项目 `src/channels/web/static/app.js` 已有正确的 SSE 实现
   - 重新实现时未参考现有代码
   - 导致使用了错误的 API（`onmessage` vs `addEventListener`）

### 预防策略

#### 1. 添加真实环境的集成测试

**测试真实的后端事件**：
```bash
#!/bin/bash
# 测试后端实际发送的 SSE 事件

# 启动真实的后端
cargo run -- run --no-onboard &

# 连接 SSE 并验证事件格式
curl -N "http://localhost:3000/api/chat/events?token=$TOKEN" | \
  grep -E "event: (response|thinking|status)"

# 验证收到了预期的事件类型
```

#### 2. 参考已有实现作为测试基准

**检查清单**：
- [ ] 已查找主项目是否有相同功能的实现
- [ ] 已对比新实现与现有实现的差异
- [ ] 已验证使用相同的 API 和模式
- [ ] 已测试与后端的实际通信

**示例**：
```javascript
// 参考 src/channels/web/static/app.js 的实现
eventSource.addEventListener('response', (e) => { ... });
eventSource.addEventListener('thinking', (e) => { ... });
eventSource.addEventListener('status', (e) => { ... });
```

#### 3. 添加契约测试

**验证前后端接口匹配**：
```javascript
describe('SSE Contract Tests', () => {
  it('should match backend event format', async () => {
    // 1. 启动真实后端
    // 2. 连接 SSE
    // 3. 发送消息
    // 4. 验证收到的事件格式
    const events = await collectSseEvents();
    expect(events).toContainEqual({
      type: 'response',
      data: expect.objectContaining({
        content: expect.any(String),
        thread_id: expect.any(String)
      })
    });
  });
});
```

#### 4. 测试覆盖检查清单

**单元测试**：
- [ ] 测试独立功能和逻辑
- [ ] 使用模拟数据和 mock

**集成测试**：
- [ ] 测试组件间的交互
- [ ] 使用真实的依赖（数据库、API）

**E2E 测试**：
- [ ] 测试完整的用户流程
- [ ] 使用真实的后端和前端
- [ ] 验证实际的网络通信

**契约测试**：
- [ ] 验证前后端接口格式匹配
- [ ] 测试事件名称和数据结构
- [ ] 使用真实的后端事件

### 关键原则

1. **优先复用已有实现**
   - 检查主项目是否有相同功能
   - 参考现有代码的实现方式
   - 避免重复造轮子和踩坑

2. **测试真实场景**
   - 不要只依赖模拟数据
   - 测试与真实后端的通信
   - 验证实际的事件格式和数据结构

3. **完整的测试覆盖**
   - 单元测试 + 集成测试 + E2E 测试 + 契约测试
   - 每种测试有不同的目的和覆盖范围
   - 不能用单元测试代替集成测试

4. **文档化接口规范**
   - 明确定义事件格式和数据结构
   - 提供示例和说明
   - 保持文档与实现同步

### 执行流程

```
新功能开发
  ↓
检查主项目是否有相同功能
  ↓
参考现有实现（如果有）
  ↓
编写单元测试
  ↓
实现功能代码
  ↓
编写集成测试（使用真实依赖）
  ↓
编写 E2E 测试（完整流程）
  ↓
编写契约测试（接口匹配）
  ↓
所有测试通过
  ↓
提交代码
```

### 参考文档

- `SSE_INTEGRATION_ISSUE_ANALYSIS.md` - 详细的问题分析
- `src/channels/web/static/app.js` - 主项目的 SSE 实现


## 启动时序和进程冒烟测试规则

**背景**：Tauri 桌面应用中，引擎异步启动与前端命令调用之间存在竞态条件。传统的单元测试和集成测试直接构造依赖，绕过了 Tauri 的 `manage()` → `State<>` 注入链路，无法捕获以下问题：

- 引擎异步启动未完成时，前端调用 Tauri 命令导致 "state not managed" panic
- `EngineState` 未初始化时的错误处理不友好
- 重复初始化 `EngineState` 的防护缺失
- 高并发场景下的线程安全问题

### 问题案例

**Desktop Client "state not managed" 运行时错误**：

- `main.rs` 中 `start_ironclaw_engine()` 在 `tauri::async_runtime::spawn` 中异步执行
- `app_handle.manage(AppState)` 在引擎 Phase 6 才调用
- 前端窗口可能在引擎就绪前就调用命令，导致竞态条件
- 593 个单元测试全部通过，但运行时仍然 panic

**根本原因**：单元测试直接调用 Rust 函数，绕过了 Tauri 的状态注入机制，无法覆盖"状态未注册"的场景。

### 测试维度

#### 1. 启动时序测试

验证 `EngineState` 的生命周期状态转换：

```rust
#[test]
fn test_timing_engine_state_starts_unready() {
    let engine = EngineState::new();
    assert!(!engine.is_ready(), "新创建的 EngineState 应该未就绪");
}

#[test]
fn test_timing_engine_state_becomes_ready_after_initialize() {
    let engine = EngineState::new();
    engine.initialize(create_minimal_app_state()).unwrap();
    assert!(engine.is_ready());
}

#[test]
fn test_timing_double_initialize_rejected() {
    let engine = EngineState::new();
    engine.initialize(create_minimal_app_state()).unwrap();
    let result = engine.initialize(create_minimal_app_state());
    assert!(result.is_err(), "重复初始化应该被拒绝");
}
```

#### 2. 失败路径测试

验证引擎未就绪时返回友好错误而非 panic：

```rust
#[test]
fn test_failure_get_before_initialize_returns_friendly_error() {
    let engine = EngineState::new();
    let result = engine.get();
    assert!(result.is_err());
    let err_msg = result.err().expect("已断言 is_err");
    assert!(err_msg.contains("启动中") || err_msg.contains("稍后"));
}
```

#### 3. 并发安全测试

验证多线程同时访问 `EngineState` 的安全性：

```rust
#[test]
fn test_concurrent_initialize_only_one_succeeds() {
    let engine = Arc::new(EngineState::new());
    let success_count = Arc::new(AtomicUsize::new(0));
    // 10 个线程同时尝试初始化，只有 1 个应该成功
    // ...
    assert_eq!(success_count.load(Ordering::SeqCst), 1);
}
```

#### 4. 契约测试

验证 `EngineState` API 行为一致性：

- `is_ready()` 与 `get()` 的返回值一致
- 多次 `get()` 返回同一个 `AppState`
- 初始化后所有字段可访问

#### 5. 冒烟测试（模拟真实启动流程）

模拟 `main.rs` 的完整启动时序：

```rust
#[tokio::test]
async fn test_smoke_simulated_startup_sequence() {
    // Phase 1: 创建空壳状态（模拟 setup）
    let engine = Arc::new(EngineState::new());
    
    // Phase 2: 前端立即调用命令（应该返回友好错误）
    assert!(engine.get().is_err());
    
    // Phase 3: 异步启动引擎
    let engine_clone = Arc::clone(&engine);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        engine_clone.initialize(create_minimal_app_state()).unwrap();
    });
    
    // Phase 4: 前端重试直到成功
    // ...
}
```

### 测试文件组织

```
desktop-client/src/
├── engine_startup_tests.rs    # 启动时序 + 冒烟测试
├── state.rs                   # EngineState 定义
└── engine.rs                  # 引擎启动逻辑
```

### 强制要求

1. **任何涉及 Tauri 状态管理的改动**，必须运行启动时序测试：
   ```bash
   cargo test -p desktop-client --lib engine_startup_tests
   ```

2. **新增 `EngineState` 方法时**，必须同步添加对应的时序测试和失败路径测试。

3. **修改引擎启动流程时**，必须验证冒烟测试仍然通过。

4. **`AppState` 字段变更时**，必须更新 `create_minimal_app_state()` 测试辅助函数和契约测试。

### 检查清单

- [ ] 已运行 `cargo test -p desktop-client --lib engine_startup_tests` 且全部通过
- [ ] 新增的 EngineState 方法有对应的时序测试
- [ ] 新增的 EngineState 方法有对应的失败路径测试
- [ ] 并发安全测试覆盖了新的访问模式
- [ ] 冒烟测试模拟了真实的启动时序
- [ ] `create_minimal_app_state()` 与 `AppState` 字段保持同步

### 核心原则

> **单元测试通过 ≠ 运行时安全。涉及异步状态注入的场景，必须用时序测试和冒烟测试覆盖真实的启动流程。**

### 参考文件

- `desktop-client/src/engine_startup_tests.rs` — 完整的启动时序测试
- `desktop-client/src/state.rs` — `EngineState` 定义
- `desktop-client/src/engine.rs` — 引擎启动逻辑
- `desktop-client/src/main.rs` — Tauri 应用入口
