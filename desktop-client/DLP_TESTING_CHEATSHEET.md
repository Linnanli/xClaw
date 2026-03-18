# DLP 测试经验速查表

## 🎯 核心教训

**问题**：142 个测试，95% 覆盖率，但生产环境仍然出现敏感信息泄露。

**原因**：测试覆盖率高 ≠ 测试质量高

## 5 个测试盲区

| 盲区 | 问题 | 教训 | 解决方案 |
|------|------|------|---------|
| 1️⃣ 失败路径 | 只测试成功，不测试失败 | 测试"不应该如何失败" | 添加失败路径测试 |
| 2️⃣ 真实环境 | 只用模拟，不用真实服务 | 模拟无法验证集成 | 添加真实环境测试 |
| 3️⃣ 降级逻辑 | 降级允许发送原始消息 | 安全功能应"故障安全" | 移除降级或审计 |
| 4️⃣ 契约测试 | 前后端接口不一致 | 需要契约测试 | 添加契约测试 |
| 5️⃣ 安全审计 | 未验证敏感信息泄露 | 需要审计测试 | 添加安全审计测试 |

## 核心原则

### 原则 1：故障安全 > 故障开放

```typescript
// ❌ 故障开放（Fail-Open）- 不安全
try {
  await scanUserInput(content);
} catch {
  return content; // 允许原始消息发送
}

// ✅ 故障安全（Fail-Safe）- 安全
try {
  await scanUserInput(content);
} catch {
  throw new Error('DLP 扫描失败，无法发送消息');
}
```

### 原则 2：测试失败路径

```rust
// ❌ 只测试成功
#[test]
fn test_scan() {
    assert!(dlp.scan("test").is_ok());
}

// ✅ 同时测试成功和失败
#[test]
fn test_scan_success() {
    assert!(dlp.scan("test").is_ok());
}

#[test]
fn test_scan_failure() {
    assert!(dlp.scan_with_error().is_err());
    // 验证错误处理不泄露敏感信息
}
```

### 原则 3：真实环境测试

```javascript
// ❌ 只用模拟
cy.intercept('POST', '**/scan', { success: true });

// ✅ 同时用模拟和真实
// 模拟环境（快速反馈）
cy.intercept('POST', '**/scan', { success: true });

// 真实环境（验证集成）
cy.window().then(async (win) => {
  await win.__TAURI__.core.invoke('scan_user_input', { content });
});
```

### 原则 4：契约测试

```rust
// 验证前后端接口格式一致
#[test]
fn test_contract() {
    let result = SanitizationResult { ... };
    let json = serde_json::to_string(&result).unwrap();
    let parsed: SanitizationResult = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.had_sensitive_data, result.had_sensitive_data);
}
```

### 原则 5：安全审计测试

```rust
// 验证敏感信息永远不会泄露
#[test]
fn test_no_leak() {
    let sensitive = "330326199408015618";
    let result = dlp.scan(sensitive).unwrap();
    
    // 验证结果、日志、错误中都没有原始数据
    assert!(!result.sanitized_content.contains(sensitive));
    assert!(!logs.contains(sensitive));
    assert!(!errors.contains(sensitive));
}
```

## 测试检查清单

### 功能开发

- [ ] 单元测试（正常路径）
- [ ] 单元测试（错误路径）✨
- [ ] 失败路径测试 ✨
- [ ] 集成测试
- [ ] 契约测试 ✨
- [ ] E2E 测试（模拟）
- [ ] E2E 测试（真实）✨

### 安全功能（更严格）

- [ ] 单元测试（正常路径）
- [ ] 单元测试（错误路径）
- [ ] 单元测试（攻击场景）
- [ ] 失败路径测试
- [ ] 集成测试
- [ ] 契约测试
- [ ] 安全审计测试 ✨
- [ ] E2E 测试（模拟）
- [ ] E2E 测试（真实）
- [ ] 降级逻辑审计（如有）✨
- [ ] 敏感信息泄露审计 ✨

## 快速命令

```bash
# 验证修复
./desktop-client/verify-dlp-fix.sh

# 运行诊断（浏览器控制台）
await window.runDlpDiagnostics()

# 测试单个消息（浏览器控制台）
await window.testDlpScan('我的身份证号是 330326199408015618')

# 运行所有测试
cargo test --lib dlp

# 生成覆盖率报告
cargo tarpaulin --out Html --output-dir coverage/
```

## 核心要点

1. **测试覆盖率 ≠ 测试质量**
   - 关注测试场景的完整性，而不是代码行数

2. **安全功能必须"故障安全"**
   - 失败时拒绝操作，而不是降级

3. **测试失败路径和成功路径一样重要**
   - 错误处理可能引入安全问题

4. **模拟测试无法替代真实测试**
   - 需要在真实环境中验证集成

5. **契约测试验证接口一致性**
   - 前后端接口需要契约测试

6. **安全审计测试验证无泄露**
   - 验证敏感信息在任何情况下都被脱敏

## 相关文档

- `AGENTS.md` - 完整的测试规则（已更新）
- `DLP_LESSONS_SUMMARY.md` - 详细教训总结
- `DLP_USER_TEST_GUIDE.md` - 用户测试指南
- `verify-dlp-fix.sh` - 自动验证脚本
