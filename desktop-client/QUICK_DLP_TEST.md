# DLP 快速测试指南

## 🚀 快速开始

### 1. 重启应用（必须！）

```bash
# 终端 1：启动后端
cd desktop-client
cargo run

# 终端 2：启动前端
cd desktop-client/src-ui
npm run dev
```

### 2. 打开浏览器

访问：http://localhost:5173

### 3. 打开浏览器控制台

按 `F12` 或 `Cmd+Option+I`（Mac）

### 4. 运行诊断测试

在控制台中输入：

```javascript
await window.runDlpDiagnostics()
```

**预期输出**：
```
🔍 DLP 诊断测试开始...

1️⃣ 检查 Tauri 环境...
✅ Tauri 环境正常

2️⃣ 检查 invoke 函数...
✅ invoke 函数可用

3️⃣ 运行 DLP 扫描测试...

📝 测试: 身份证号
   输入: 我的身份证号是 330326199408015618
✅ 通过

📝 测试: 手机号
   输入: 我的手机号是 13800138000
✅ 通过

📊 测试结果:
   通过: 3
   失败: 0
   总计: 3
✅ 所有测试通过！DLP 功能正常工作
```

### 5. 测试聊天界面

在聊天输入框输入：

```
我的身份证号是 330326199408015618
```

**预期结果**：

1. **控制台日志**：
```
🔍 Scanning message for sensitive data...
   Original content: 我的身份证号是 330326199408015618
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
大模型应该只能看到脱敏后的内容，无法获取原始身份证号。

## ❌ 如果测试失败

### 问题 1：`window.runDlpDiagnostics is not a function`

**原因**：诊断工具未加载

**解决**：
1. 检查 `desktop-client/src-ui/src/main.tsx` 是否包含：
   ```typescript
   import "./app/utils/dlpDiagnostics";
   ```
2. 强制刷新浏览器（Ctrl+Shift+R）

### 问题 2：`invoke is not defined`

**原因**：Tauri 环境未初始化

**解决**：
1. 确认使用 Tauri 应用打开，而不是直接在浏览器中打开
2. 检查 Tauri 应用是否正确启动

### 问题 3：`scan_user_input command not found`

**原因**：Tauri 命令未注册

**解决**：
1. 检查 `desktop-client/src/main.rs` 中是否包含：
   ```rust
   scan_user_input,
   ```
2. 重新编译：`cargo build --manifest-path desktop-client/Cargo.toml`
3. 重启应用

### 问题 4：DLP 扫描返回错误

**原因**：DLP 服务未初始化

**解决**：
1. 检查后端日志是否有错误
2. 检查 `CommandState::new_with_token()` 是否正确初始化 DLP
3. 重启后端服务

## 📝 验证检查清单

- [ ] 后端服务正常运行（http://localhost:3000/health）
- [ ] 前端服务正常运行（http://localhost:5173）
- [ ] 浏览器控制台无错误
- [ ] `window.runDlpDiagnostics()` 所有测试通过
- [ ] 聊天界面输入身份证号能正确脱敏
- [ ] Toast 警告正常显示
- [ ] 大模型收到的是脱敏后的内容
- [ ] 控制台日志显示完整的扫描过程

## 🎯 成功标准

当你看到以下情况时，说明 DLP 功能正常工作：

1. ✅ 诊断测试全部通过
2. ✅ 控制台显示 `⚠️ Sensitive data detected, using sanitized content`
3. ✅ Toast 显示 `已脱敏 X 处敏感信息`
4. ✅ 消息中的敏感信息被替换为 `***`
5. ✅ 大模型无法获取原始敏感信息

## 🆘 需要帮助？

如果以上步骤都无法解决问题，请提供：

1. 浏览器控制台的完整日志
2. 后端日志（使用 `RUST_LOG=debug cargo run`）
3. `window.runDlpDiagnostics()` 的输出
4. 具体的错误信息

## 相关文档

- `DLP_ISSUE_FIX_GUIDE.md` - 详细的修复指南
- `DLP_ISSUE_ANALYSIS.md` - 问题分析
- `DLP_QUICK_REFERENCE.md` - DLP 快速参考
