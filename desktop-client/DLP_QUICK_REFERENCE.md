# DLP 功能快速参考

## 前端使用

### 基本用法

```typescript
import { useDlpScan } from './hooks/useDlpScan';

function MyComponent() {
  const { scanUserInput } = useDlpScan();
  
  const handleSubmit = async (text: string) => {
    const result = await scanUserInput(text);
    
    if (result.was_blocked) {
      alert(`消息被阻止: ${result.block_reason}`);
      return;
    }
    
    const safeText = result.had_sensitive_data 
      ? result.sanitized_content 
      : text;
    
    // 使用 safeText 继续处理
  };
}
```

### 自动集成（useAiChat）

```typescript
// DLP 已自动集成到 useAiChat
const { append } = useAiChat({ threadId, apiUrl, authToken });

// 发送消息时自动扫描
await append({ role: 'user', content: '我的身份证是 110101199003071234' });
// → 自动脱敏为: '我的身份证是 110************234'
```

## 后端使用

### Tauri 命令

```rust
// 在 Rust 代码中直接使用
let dlp = state.dlp_integration.lock().await;
let result = dlp.scan_user_input(content).await?;
```

## 支持的敏感信息类型

| 类型 | 示例 | 脱敏结果 | 动作 |
|------|------|----------|------|
| 身份证号 | 110101199003071234 | 110************234 | 脱敏 |
| 手机号 | 13800138000 | 138*****000 | 脱敏 |
| 阿里云密钥 | LTAI4G8aB9cD2eFgH3iJ | - | 阻止 |
| AWS 密钥 | AKIAIOSFODNN7EXAMPLE | - | 阻止 |
| 私钥 | -----BEGIN PRIVATE KEY----- | - | 阻止 |

## 配置管理

### 获取配置

```typescript
const { getDlpConfig } = useDlpScan();
const config = await getDlpConfig();

console.log(config.enabled);              // true/false
console.log(config.real_time_monitoring); // true/false
console.log(config.audit_logging);        // true/false
```

### 更新配置

```typescript
const { updateDlpConfig } = useDlpScan();

await updateDlpConfig({
  enabled: true,
  sanitization: {
    redaction_text: '[已脱敏]',
    preserve_format: true,
    partial_redaction: true,
  },
  real_time_monitoring: true,
  audit_logging: true,
  custom_patterns: [],
});
```

### 查看统计

```typescript
const { getDlpStatistics } = useDlpScan();
const stats = await getDlpStatistics();

console.log(`总扫描次数: ${stats.total_scans}`);
console.log(`检测到敏感数据: ${stats.sensitive_data_detected}`);
console.log(`内容阻止: ${stats.content_blocked}`);
console.log(`内容脱敏: ${stats.content_sanitized}`);
```

## 运行测试

```bash
# 运行单元测试
cargo test dlp --lib --manifest-path desktop-client/Cargo.toml

# 运行 E2E 测试
./desktop-client/scripts/run-dlp-e2e-tests.sh

# 或手动运行
npx cypress run --spec "cypress/e2e/dlp_integration.cy.js"
```

## 常见场景

### 场景 1：聊天消息脱敏

用户输入：`我的手机号是 13800138000`
→ 自动脱敏为：`我的手机号是 138*****000`
→ 显示警告：`⚠️ 敏感信息已脱敏`

### 场景 2：API 密钥阻止

用户输入：`阿里云密钥：LTAI4G8aB9cD2eFgH3iJ`
→ 消息被阻止
→ 显示错误：`❌ 消息包含敏感信息已被阻止: aliyun_access_key`

### 场景 3：混合敏感信息

用户输入：`身份证 110101199003071234，手机 13800138000`
→ 脱敏为：`身份证 110************234，手机 138*****000`
→ 统计：2 个敏感信息被脱敏

## 性能优化建议

1. **批量扫描** - 对于多条消息，使用批量扫描接口
2. **缓存结果** - 对于相同内容，缓存扫描结果
3. **异步处理** - 使用异步扫描，不阻塞 UI
4. **降级策略** - DLP 服务不可用时，允许消息通过但记录日志

## 故障排查

### 问题：DLP 扫描失败

```typescript
try {
  const result = await scanUserInput(text);
} catch (error) {
  console.error('DLP scan failed:', error);
  // 降级处理：允许消息通过
  return text;
}
```

### 问题：扫描延迟过高

检查：
1. 内容长度是否过大（> 10KB）
2. 自定义模式是否过于复杂
3. 并发扫描数量是否过多

### 问题：误报或漏报

调整配置：
1. 修改 `partial_redaction` 设置
2. 添加自定义模式
3. 调整严重级别阈值

## 更多信息

- 详细文档：`desktop-client/src/dlp/README.md`
- E2E 测试：`DLP_E2E_TESTING.md`
- 集成总结：`DLP_INTEGRATION_SUMMARY.md`
