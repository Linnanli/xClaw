# Vercel AI SDK 集成计划

**日期**: 2026-03-16  
**状态**: 🚀 开始集成

---

## 为什么选择 Vercel AI SDK?

✅ **成熟的库**
- 广泛使用，由 Vercel 官方维护
- 完整的文档和示例
- 生产级别的代码质量

✅ **完整的功能**
- useChat Hook 处理所有流式响应逻辑
- 自动消息历史管理
- 自动重试和错误处理
- 支持工具调用

✅ **完全可定制**
- 只提供逻辑，不强制 UI
- 支持任何 CSS 框架
- 可以保留现有的样式

---

## 集成步骤

### 第 1 步: 安装依赖 ✅
```bash
npm install ai
```

**文件**: `desktop-client/src-ui/package.json`  
**状态**: ✅ 已添加 `"ai": "^4.0.0"`

### 第 2 步: 创建 Hook 包装器 ✅
**文件**: `desktop-client/src-ui/src/app/hooks/useAiChat.ts`  
**状态**: ✅ 已创建

**功能**:
- 包装 Vercel AI SDK 的 useChat Hook
- 与后端 SSE 端点集成
- 处理认证令牌
- 提供简单的 API

### 第 3 步: 更新 ChatTabWithSSE 组件 (待做)
**文件**: `desktop-client/src-ui/src/app/components/tabs/ChatTabWithSSE.tsx`

**改动**:
1. 导入 `useAiChat` Hook
2. 替换手动的 SSE 连接代码
3. 使用 Hook 提供的消息和输入状态
4. 保留现有的 UI 和样式

### 第 4 步: 测试集成 (待做)
- 启动后端服务
- 启动前端开发服务器
- 测试消息发送和接收
- 验证流式响应

---

## 当前状态

| 步骤 | 状态 | 说明 |
|------|------|------|
| 1. 安装依赖 | ✅ | 已在 package.json 中添加 |
| 2. 创建 Hook | ✅ | 已创建 useAiChat.ts |
| 3. 更新组件 | ⏳ | 待做 |
| 4. 测试 | ⏳ | 待做 |

---

## 下一步

1. **运行 npm install**
   ```bash
   cd desktop-client/src-ui
   npm install
   ```

2. **更新 ChatTabWithSSE 组件**
   - 使用 `useAiChat` Hook
   - 移除手动的 SSE 连接代码
   - 保留现有的 UI 组件

3. **测试集成**
   - 启动后端和前端
   - 测试消息发送
   - 验证流式响应

---

## 优势

✅ **解决当前问题**
- 不再需要手动处理 EventSource
- 自动处理连接、重试、错误

✅ **更好的用户体验**
- 自动流式响应
- 自动消息历史
- 自动加载状态

✅ **更易维护**
- 使用成熟的库
- 更少的自己写的代码
- 更好的错误处理

---

## 参考资源

- [Vercel AI SDK 文档](https://sdk.vercel.ai/)
- [useChat Hook 文档](https://sdk.vercel.ai/docs/ai-sdk-ui/chatbot)
- [AI Elements 组件库](https://vercel.com/academy/ai-sdk/ai-elements)
