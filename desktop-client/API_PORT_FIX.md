# Desktop Client API 端口修复

## 问题描述

### 问题 1: `get_dlp_config` 接口频繁调用

**现象**: 每次在输入框输入一个字符,就会调用一次 `ipc://localhost/get_dlp_config` 接口。

**原因**: `DlpStatusIndicator` 组件在 `useEffect` 中调用 `getDlpConfig`,但依赖项包含了 `getDlpConfig` 函数本身。由于 React 的特性,每次组件重新渲染时 `getDlpConfig` 都是一个新的函数引用,导致 `useEffect` 重新执行。

**位置**: `desktop-client/src-ui/src/app/components/DlpStatusIndicator.tsx:27`

```typescript
// ❌ 错误代码
useEffect(() => {
  const loadConfig = async () => {
    const config = await getDlpConfig();
    setEnabled(config.enabled);
  };
  loadConfig();
}, [getDlpConfig]); // 问题:getDlpConfig 每次渲染都是新函数
```

### 问题 2: `/api/chat/send` 返回 404

**现象**: 前端调用 `http://localhost:3000/api/chat/send` 返回 404 错误。

**原因**: Desktop Client 的嵌入式 IronClaw 服务器运行在端口 `38080`,而不是 `3000`。前端代码中硬编码了错误的端口号。

**位置**: 多个文件中使用了 `http://localhost:3000`

## 解决方案

### 1. 修复 DLP 配置频繁调用

修改 `DlpStatusIndicator.tsx`,移除 `getDlpConfig` 依赖,只在组件挂载时加载一次:

```typescript
// ✅ 正确代码
useEffect(() => {
  const loadConfig = async () => {
    try {
      const config = await getDlpConfig();
      setEnabled(config.enabled);
    } catch (error) {
      console.error('Failed to load DLP config:', error);
    } finally {
      setLoading(false);
    }
  };

  loadConfig();
  // eslint-disable-next-line react-hooks/exhaustive-deps
}, []); // 只在组件挂载时加载一次
```

### 2. 统一 API 端口配置

创建统一的 API 配置文件 `desktop-client/src-ui/src/app/config/api.ts`:

```typescript
/**
 * Desktop Client 嵌入式服务器端口
 * 与 desktop-client/src/embedded_server.rs 中的 EMBEDDED_SERVER_PORT 保持一致
 */
export const EMBEDDED_SERVER_PORT = 38080;

/**
 * API 基础 URL
 */
export const API_BASE_URL = `http://localhost:${EMBEDDED_SERVER_PORT}`;
```

### 3. 更新所有使用 API URL 的文件

更新以下文件使用统一的 `API_BASE_URL`:

- `desktop-client/src-ui/src/app/hooks/useAiChat.ts`
- `desktop-client/src-ui/src/app/components/tabs/ChatTabWithAiSdk.tsx`
- `desktop-client/src-ui/src/app/components/tabs/LogsTab.tsx`
- `desktop-client/src-ui/src/app/components/tabs/SettingsTab.tsx`

## 修改的文件

### 新增文件

- `desktop-client/src-ui/src/app/config/api.ts` - API 配置文件

### 修改的文件

1. `desktop-client/src-ui/src/app/components/DlpStatusIndicator.tsx`
   - 修复 `useEffect` 依赖项,避免频繁调用

2. `desktop-client/src-ui/src/app/hooks/useAiChat.ts`
   - 导入 `API_BASE_URL`
   - 修改默认 API URL 从 `http://localhost:3000` 到 `API_BASE_URL`

3. `desktop-client/src-ui/src/app/components/tabs/ChatTabWithAiSdk.tsx`
   - 导入 `API_BASE_URL`
   - 使用 `API_BASE_URL` 替代硬编码的 URL

4. `desktop-client/src-ui/src/app/components/tabs/LogsTab.tsx`
   - 导入 `API_BASE_URL`
   - 使用 `API_BASE_URL` 替代硬编码的 URL

5. `desktop-client/src-ui/src/app/components/tabs/SettingsTab.tsx`
   - 导入 `API_BASE_URL`
   - 使用 `API_BASE_URL` 作为默认值和占位符

6. `desktop-client/src-ui/src/app/hooks/useDlpScan.ts`
   - 导入 `API_BASE_URL` (为未来可能的使用做准备)

## 验证

### 1. 验证 DLP 配置调用

启动应用后,打开浏览器开发者工具,在输入框输入字符,确认不再频繁调用 `get_dlp_config`。

### 2. 验证聊天功能

1. 确保 IronClaw 服务器运行在端口 38080:
   ```bash
   cargo run --manifest-path ironclaw/Cargo.toml -- run --no-onboard
   ```

2. 启动 Desktop Client:
   ```bash
   cd desktop-client
   cargo tauri dev
   ```

3. 在聊天界面发送消息,确认:
   - 消息成功发送到 `http://localhost:38080/api/chat/send`
   - 收到 SSE 响应
   - 消息正常显示

## 最佳实践

### 1. 统一配置管理

所有 API 端点配置应该集中在 `config/api.ts` 文件中,避免在代码中硬编码 URL。

### 2. React Hook 依赖项

使用 `useEffect` 时要注意依赖项:
- 如果只需要在组件挂载时执行一次,使用空数组 `[]`
- 如果依赖函数,考虑使用 `useCallback` 包装函数
- 使用 ESLint 规则 `react-hooks/exhaustive-deps` 检查依赖项

### 3. 端口配置同步

确保前后端端口配置保持一致:
- Rust: `desktop-client/src/embedded_server.rs` 中的 `EMBEDDED_SERVER_PORT`
- TypeScript: `desktop-client/src-ui/src/app/config/api.ts` 中的 `EMBEDDED_SERVER_PORT`

## 相关文档

- `desktop-client/src/embedded_server.rs` - 嵌入式服务器配置
- `desktop-client/DLP_INTEGRATION_SUMMARY.md` - DLP 集成总结
- `desktop-client/DESKTOP_CLIENT_FEATURE_CHECKLIST.md` - 功能检查清单
