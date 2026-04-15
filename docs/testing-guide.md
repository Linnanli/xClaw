# 测试指南

本文档包含项目测试策略的详细说明、代码示例和历史教训的完整分析。  
核心规则见 `AGENTS.md`，本文档是其补充。

---

## 测试代码示例

### 失败路径测试

```rust
// ❌ 只测试成功路径
#[test]
fn test_dlp_scan_success() {
    let result = dlp.scan("330326199408015618");
    assert!(result.is_ok());
}

// ✅ 同时测试失败路径
#[test]
fn test_dlp_scan_failure() {
    let result = dlp.scan_with_unavailable_service("test");
    assert!(result.is_err());
    // 验证错误处理不会泄露敏感信息
    let error_msg = result.unwrap_err().to_string();
    assert!(!error_msg.contains("sensitive_data"));
}
```

### 故障安全 vs 故障开放

```typescript
// ❌ 故障开放（Fail-Open）— 不安全
try {
  const result = await scanUserInput(content);
} catch (error) {
  console.warn('DLP scan failed, allowing message');
  return content; // 返回原始内容
}

// ✅ 故障安全（Fail-Safe）— 安全
try {
  const result = await scanUserInput(content);
} catch (error) {
  console.error('DLP scan failed, blocking message');
  throw new Error('DLP 扫描失败，无法发送消息');
}
```

### 契约测试：验证业务目标可达成

```rust
// ❌ 只验证格式
#[test]
fn test_contract_client_models_response_format() {
    let models: Vec<ClientExpectedModelConfig> =
        serde_json::from_value(response).expect("能解析就行");
}

// ✅ 验证业务目标可达成
#[test]
fn test_contract_client_models_sufficient_for_llm_call() {
    let models: Vec<ClientExpectedModelConfig> =
        serde_json::from_value(response).unwrap();
    for model in &models {
        assert!(model.api_base_url.is_some(),
            "模型 {} 缺少 api_base_url，客户端无法调用", model.model_id);
    }
}
```

### 安全审计测试

```rust
#[test]
fn test_audit_no_sensitive_data_leak() {
    let sensitive_data = "330326199408015618";
    let result = dlp.scan(sensitive_data).unwrap();
    assert!(!result.sanitized_content.contains(sensitive_data));

    let logs = capture_logs();
    assert!(!logs.contains(sensitive_data));

    if let Err(e) = dlp.scan_with_error(sensitive_data) {
        assert!(!e.to_string().contains(sensitive_data));
    }
}
```

---

## 历史教训详细分析

### 盲区 1：DLP 失败路径缺失

142 个测试全部通过、95% 覆盖率，但用户输入身份证号时聊天中没有脱敏。  
根因：测试只覆盖成功路径，降级逻辑 `if (!dlpScanSucceeded)` 允许原始消息发送。  
详见：`desktop-client/DLP_LESSONS_SUMMARY.md`

### 盲区 2：model-configs 404

`/api/model-configs` 返回 404，但 `cargo test` 全部通过。  
根因：handler 编译失败但测试文件未引用 `routes.rs`，`cargo test` 绕过了编译错误；迁移文件存在但未执行。  
防护：`admin-backend/tests/integration_smoke_tests.rs` 三层防护（编译检查 + 迁移完整性 + HTTP 冒烟）。

### 盲区 3：client-models 缺字段

测试主动断言 `assert!(!json_str.contains("api_key"))`，把错误设计固化为"规范"。  
客户端拿到模型列表后无法实际调用 LLM。  
教训：契约测试必须从业务目标出发定义"够用"的标准。

### 盲区 4：SSE 事件名不匹配

后端发送命名 SSE 事件（`event: response`），前端只监听 `onmessage`。  
测试用模拟数据通过，但真实环境失败。  
教训：参考主项目已有实现（`src/channels/web/static/app.js`），测试真实后端事件格式。

### 盲区 5：Tauri state not managed

593 个单元测试全部通过，但运行时 panic。  
根因：单元测试直接调用 Rust 函数，绕过 Tauri 状态注入机制。  
防护：`desktop-client/src/engine_startup_tests.rs` 启动时序测试。

---

## E2E 测试模式

### Cypress 基础模式

```javascript
describe('Feature Tests', () => {
  beforeEach(() => {
    cy.visit('http://localhost:5173');
  });

  it('should send message', () => {
    cy.get('.message-input').type('Hello');
    cy.get('.send-button').click();
    cy.get('.message-item').should('contain', 'Hello');
  });
});
```

### 网络错误模拟

```javascript
cy.intercept('GET', '**/api/events', { forceNetworkError: true });
cy.get('.sse-status').should('contain', 'Reconnecting');
```

### 真实环境 SSE 测试

```javascript
describe('SSE Tests (Real)', () => {
  it('should receive real backend events', () => {
    cy.window().then(async (win) => {
      const result = await win.__TAURI__.core.invoke('scan_user_input', {
        content: '330326199408015618'
      });
      expect(result.sanitized_content).to.contain('330************618');
    });
  });
});
```

### 测试环境启动

```bash
cargo run -- run --cli-only --no-onboard &
cd desktop-client/src-ui && npm run dev &
npm run test:e2e
```

---

## 测试执行顺序

```bash
# 1. 单元测试
cargo test --lib {module}

# 2. 失败路径测试
cargo test --test {module}_failure_tests

# 3. 集成测试
cargo test --test {module}_integration_tests

# 4. 契约测试
cargo test --test {module}_contract_tests

# 5. 安全审计测试
cargo test --test {module}_security_audit_tests

# 6. E2E 测试
npm run test:e2e
```

---

## 参考文档

- `desktop-client/DLP_LESSONS_SUMMARY.md` — DLP 核心教训总结
- `desktop-client/DLP_TESTING_LESSONS_LEARNED.md` — 详细经验教训
- `desktop-client/DLP_SUPPLEMENTARY_TEST_PLAN.md` — 补充测试计划
- `desktop-client/src/engine_startup_tests.rs` — 启动时序测试
- `admin-backend/tests/integration_smoke_tests.rs` — 冒烟测试
- `desktop-client/tests/tauri_command_contract_tests.rs` — Tauri 命令契约测试
- `admin-backend/ui/cypress/` — Cypress E2E 测试
- `docs/plans/project-level-pict-test-design.md` — 项目级 PICT 测试矩阵与子模型设计
- `docs/plans/skill-management-test-generation-comparison.md` — 技能管理模块的常规生成 vs PICT 驱动生成对照
- `scripts/pict_generate.py` — 统一的 PICT/pypict/Docker 生成入口，供 agent 和开发者直接生成 pairwise 组合
