# DLP 脱敏失效问题修复指南

## 问题描述

**用户报告**：在客户端输入身份证号 `330326199408015618`，聊天中没有脱敏，大模型能看到原始数据。

## 根本原因

在 `useAiChat.ts` 中存在**降级处理逻辑**：

```typescript
} catch (dlpError) {
  // 如果 DLP 扫描失败，记录错误但允许消息继续发送（降级处理）
  if (!dlpScanSucceeded) {
    console.warn('⚠️  DLP scan failed, allowing message to proceed (degraded mode)');
    // 不抛出错误，继续发送原始消息 ⚠️ 这是问题所在！
  }
}
```

**问题**：
1. 如果 `scanUserInput()` 调用失败（例如 Tauri 命令未初始化）
2. 代码会进入降级模式，**允许原始消息继续发送**
3. 导致敏感信息未经脱敏就发送给大模型

## 已实施的修复

### 修复 1：移除降级处理逻辑

**文件**：`desktop-client/src-ui/src/app/hooks/useAiChat.ts`

**修改**：
- ✅ 移除了 `dlpScanSucceeded` 标志
- ✅ DLP 扫描失败时，**强制抛出错误**，阻止消息发送
- ✅ 添加了 `scanUserInput` 函数类型验证
- ✅ 增强了错误日志输出

**关键代码**：
```typescript
// 验证 scanUserInput 是否可用
if (typeof scanUserInput !== 'function') {
  console.error('❌ CRITICAL: scanUserInput is not a function!');
  setIsLoading(false);
  throw new Error('DLP 扫描功能未初始化，无法发送消息');
}

try {
  const scanResult = await scanUserInput(message.content);
  // ... 处理扫描结果
} catch (dlpError) {
  console.error('❌ DLP scan failed - BLOCKING MESSAGE SEND');
  setIsLoading(false);
  
  // 🚨 关键修复：DLP 扫描失败时，阻止消息发送
  throw new Error(`DLP 扫描失败，无法发送消息: ${dlpError}`);
}
```

### 修复 2：增强诊断工具

**文件**：`desktop-client/src-ui/src/app/utils/dlpDiagnostics.ts`

**改进**：
- ✅ 添加 Tauri 环境检查
- ✅ 添加 `invoke` 函数类型验证
- ✅ 添加 `testDlpScan()` 函数用于单个消息测试
- ✅ 增强错误日志输出

### 修复 3：自动加载诊断工具

**文件**：`desktop-client/src-ui/src/main.tsx`

**修改**：
```typescript
// 导入 DLP 诊断工具（在浏览器控制台中可用）
import "./app/utils/dlpDiagnostics";
```

现在可以在浏览器控制台中运行：
- `window.runDlpDiagnostics()` - 运行完整测试
- `window.testDlpScan('测试内容')` - 测试单个消息

## 验证步骤

### 步骤 1：重启应用

```bash
# 1. 停止所有服务（Ctrl+C）

# 2. 重新编译后端
cd desktop-client
cargo build

# 3. 启动后端
cargo run

# 4. 启动前端（新终端）
cd src-ui
npm run dev

# 5. 在浏览器中打开 http://localhost:5173
```

### 步骤 2：运行诊断测试

在浏览器控制台中运行：

```javascript
// 运行完整诊断
await window.runDlpDiagnostics()

// 测试特定身份证号
await window.testDlpScan('我的身份证号是 330326199408015618')
```

**预期结果**：
```javascript
{
  had_sensitive_data: true,
  sanitized_content: '我的身份证号是 330************618',
  was_blocked: false,
  sanitization_stats: {
    total_matches: 1,
    redacted_count: 1,
    blocked_count: 0,
    warned_count: 0
  }
}
```

### 步骤 3：测试聊天界面

1. 在聊天输入框输入：`我的身份证号是 330326199408015618`
2. 点击发送
3. 检查浏览器控制台日志：

**预期日志**：
```
🔍 Scanning message for sensitive data...
   Original content: 我的身份证号是 330326199408015618
✅ DLP scan completed: { had_sensitive_data: true, ... }
⚠️  Sensitive data detected, using sanitized content
   Original: 我的身份证号是 330326199408015618
   Sanitized: 我的身份证号是 330************618
   Redacted count: 1
📤 Sending message...
   Content: 我的身份证号是 330************618
```

4. 检查聊天界面：
   - ✅ 应该显示 DLP 警告 Toast："已脱敏 1 处敏感信息"
   - ✅ 消息内容应该是：`我的身份证号是 330************618`
   - ✅ 大模型收到的也应该是脱敏后的内容

### 步骤 4：如果仍然失败

如果诊断测试失败，可能的原因：

#### 原因 1：Tauri 命令未注册

**检查**：
```bash
# 查看 main.rs 中的命令注册
grep -A 5 "scan_user_input" desktop-client/src/main.rs
```

**预期**：应该看到 `scan_user_input,` 在 `invoke_handler!` 中

#### 原因 2：DLP 集成未初始化

**检查**：
```bash
# 查看 CommandState 初始化
grep -A 10 "dlp_integration" desktop-client/src/main.rs
```

**预期**：应该看到 `DlpIntegration::with_default_config()` 被调用

#### 原因 3：前端代码未更新

**解决**：
```bash
# 强制清除缓存并重启
cd desktop-client/src-ui
rm -rf node_modules/.vite
npm run dev
```

然后在浏览器中强制刷新（Ctrl+Shift+R）

## 紧急修复方案

如果上述修复仍然无效，可以在 UI 层直接调用 DLP 扫描：

**文件**：`desktop-client/src-ui/src/app/components/tabs/ChatTabWithAiSdk.tsx`

在 `handleSendMessage` 函数中添加：

```typescript
const handleSendMessage = async (e: React.FormEvent) => {
  e.preventDefault();
  if (!chat.input.trim() || !selectedConversation) return;

  try {
    setLoading(true);
    
    // 🚨 紧急修复：在 UI 层直接调用 DLP 扫描
    console.log('🔍 [UI Layer] Scanning message for sensitive data...');
    const { invoke } = await import('@tauri-apps/api/core');
    
    try {
      const scanResult = await invoke('scan_user_input', { 
        content: chat.input 
      }) as any;
      
      console.log('✅ [UI Layer] DLP scan completed:', scanResult);
      
      if (scanResult.was_blocked) {
        throw new Error(`消息被阻止: ${scanResult.block_reason}`);
      }
      
      // 使用脱敏后的内容
      const contentToSend = scanResult.had_sensitive_data 
        ? scanResult.sanitized_content 
        : chat.input;
      
      if (scanResult.had_sensitive_data) {
        console.warn('⚠️  [UI Layer] Using sanitized content');
        console.log('   Original:', chat.input);
        console.log('   Sanitized:', contentToSend);
        
        // 显示警告
        chat.setDlpWarning({
          redacted: scanResult.sanitization_stats.redacted_count,
          blocked: scanResult.sanitization_stats.blocked_count,
        });
        setTimeout(() => chat.setDlpWarning(null), 3000);
      }
      
      // 发送脱敏后的消息
      await chat.append({
        role: 'user',
        content: contentToSend,
      });
      
    } catch (dlpError) {
      console.error('❌ [UI Layer] DLP scan failed:', dlpError);
      throw new Error(`DLP 扫描失败: ${dlpError}`);
    }
    
    chat.setInput('');
  } catch (err) {
    console.error('❌ Failed to send message:', err);
  } finally {
    setLoading(false);
  }
};
```

## 测试验证

### 测试用例 1：身份证号

**输入**：`我的身份证号是 330326199408015618`

**预期**：
- 控制台显示：`✅ DLP scan completed: { had_sensitive_data: true, ... }`
- Toast 显示：`已脱敏 1 处敏感信息`
- 消息显示：`我的身份证号是 330************618`
- 大模型收到：`我的身份证号是 330************618`

### 测试用例 2：手机号

**输入**：`联系我：13800138000`

**预期**：
- 控制台显示：`✅ DLP scan completed: { had_sensitive_data: true, ... }`
- Toast 显示：`已脱敏 1 处敏感信息`
- 消息显示：`联系我：138****8000`

### 测试用例 3：API密钥（应被阻止）

**输入**：`阿里云密钥：LTAI4G8aB9cD2eFgH3iJ`

**预期**：
- 控制台显示：`❌ Message blocked by DLP`
- 错误提示：`消息包含敏感信息已被阻止`
- 消息不会发送

### 测试用例 4：普通文本

**输入**：`你好，今天天气怎么样？`

**预期**：
- 控制台显示：`✅ No sensitive data detected`
- 消息正常发送
- 无 Toast 提示

## 检查清单

- [ ] 已修复 `useAiChat.ts` 的降级处理逻辑
- [ ] 已导入 DLP 诊断工具到 `main.tsx`
- [ ] 已重新编译后端（`cargo build`）
- [ ] 已重启后端服务
- [ ] 已重启前端服务
- [ ] 已在浏览器中强制刷新（Ctrl+Shift+R）
- [ ] 已运行 `window.runDlpDiagnostics()` 测试
- [ ] 所有诊断测试通过
- [ ] 已测试聊天界面的实际使用
- [ ] 身份证号正确脱敏
- [ ] Toast 警告正常显示
- [ ] 大模型收到的是脱敏后的内容

## 后续优化

1. **添加 DLP 状态监控**
   - 在 UI 中显示 DLP 服务状态
   - 如果 DLP 服务不可用，禁用发送按钮

2. **改进错误提示**
   - 当 DLP 扫描失败时，显示更友好的错误信息
   - 提供重试选项

3. **添加配置选项**
   - 允许用户临时禁用 DLP（仅限开发环境）
   - 添加 DLP 严格模式开关

4. **性能优化**
   - 对短消息（<10字符）跳过 DLP 扫描
   - 缓存扫描结果避免重复扫描

## 相关文件

- `desktop-client/src-ui/src/app/hooks/useAiChat.ts` - 修复降级处理逻辑
- `desktop-client/src-ui/src/app/utils/dlpDiagnostics.ts` - 诊断工具
- `desktop-client/src-ui/src/main.tsx` - 导入诊断工具
- `desktop-client/src/commands.rs` - Tauri 命令实现
- `desktop-client/src/main.rs` - 命令注册
- `desktop-client/src/dlp/integration.rs` - DLP 核心逻辑
