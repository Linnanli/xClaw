# DLP 用户测试指南

## 🎯 问题已修复

**问题**：输入身份证号 `330326199408015618`，聊天中没有脱敏。

**修复**：移除了降级处理逻辑，DLP 扫描失败时会阻止消息发送。

## ✅ 验证修复（3 步）

### 步骤 1：重启应用

```bash
# 终端 1：启动后端
cd desktop-client
cargo run

# 终端 2：启动前端
cd desktop-client/src-ui
npm run dev
```

### 步骤 2：运行诊断（浏览器控制台）

1. 打开 http://localhost:5173
2. 按 `F12` 打开控制台
3. 输入并运行：

```javascript
await window.runDlpDiagnostics()
```

**预期**：所有测试通过 ✅

### 步骤 3：测试聊天

在聊天输入框输入：

```
我的身份证号是 330326199408015618
```

**预期结果**：

1. **控制台日志**：
```
🔍 Scanning message for sensitive data...
✅ DLP scan completed: { had_sensitive_data: true, ... }
⚠️  Sensitive data detected, using sanitized content
   Original: 我的身份证号是 330326199408015618
   Sanitized: 我的身份证号是 330************618
```

2. **Toast 提示**：
```
⚠️ 已脱敏 1 处敏感信息
```

3. **消息显示**：
```
我的身份证号是 330************618
```

4. **大模型回复**：
大模型只能看到 `330************618`，无法获取原始身份证号。

## 🧪 更多测试用例

### 测试 1：手机号

**输入**：`联系我：13800138000`

**预期**：`联系我：138****8000`

### 测试 2：多个敏感信息

**输入**：`我的身份证是 330326199408015618，手机号是 13800138000`

**预期**：`我的身份证是 330************618，手机号是 138****8000`

### 测试 3：API密钥（应被阻止）

**输入**：`阿里云密钥：LTAI4G8aB9cD2eFgH3iJ`

**预期**：显示错误 `消息包含敏感信息已被阻止`

### 测试 4：普通文本

**输入**：`你好，今天天气怎么样？`

**预期**：正常发送，无 Toast 提示

## ❌ 如果仍然失败

### 方案 1：清除缓存

```bash
cd desktop-client/src-ui
rm -rf node_modules/.vite
npm run dev
```

然后在浏览器中强制刷新（Ctrl+Shift+R）

### 方案 2：检查控制台日志

如果看到以下错误：

**错误 1**：`scanUserInput is not a function`
- **原因**：useDlpScan Hook 未正确初始化
- **解决**：检查 `useDlpScan.ts` 是否正确导出

**错误 2**：`scan_user_input command not found`
- **原因**：Tauri 命令未注册
- **解决**：重新编译并重启应用

**错误 3**：`DLP 扫描失败，无法发送消息`
- **原因**：DLP 服务未初始化
- **解决**：检查后端日志，确认 DLP 集成是否正常

### 方案 3：运行完整验证

```bash
./desktop-client/verify-dlp-fix.sh
```

应该看到所有检查通过 ✅

## 📞 需要帮助？

如果问题仍然存在，请提供：

1. 浏览器控制台的完整日志
2. `window.runDlpDiagnostics()` 的输出
3. 后端日志（使用 `RUST_LOG=debug cargo run`）
4. 具体的错误信息

## 相关文档

- `DLP_ISSUE_FIX_GUIDE.md` - 详细的修复指南
- `DLP_ISSUE_ANALYSIS.md` - 问题分析
- `QUICK_DLP_TEST.md` - 快速测试指南
