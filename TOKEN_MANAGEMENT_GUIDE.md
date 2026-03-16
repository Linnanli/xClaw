# 令牌管理指南

## 问题

每次启动后端时都会生成新的认证令牌，导致前端需要手动更新令牌。

## 解决方案

实现了自动令牌管理系统，支持多种获取令牌的方式。

---

## 令牌获取优先级

前端会按以下优先级获取令牌：

1. **URL 参数** - 最高优先级
   ```
   http://localhost:5173?token=36c1a0275ae2708954a402871a27255bcecf9aa24f7542a43ff0c5e1f651b19b
   ```

2. **本地存储** - 中等优先级
   - 自动保存从 URL 获取的令牌
   - 下次访问时自动使用

3. **默认值** - 最低优先级
   - 如果以上都不可用，使用硬编码的默认值

---

## 使用方法

### 方法 1: 从启动脚本自动获取（推荐）

启动脚本会显示网关 URL，包含令牌：

```bash
bash scripts/start-all.sh
```

输出：
```
✅ 网关 URL: http://127.0.0.1:3000/?token=36c1a0275ae2708954a402871a27255bcecf9aa24f7542a43ff0c5e1f651b19b
```

然后访问前端时添加令牌参数：
```
http://localhost:5173?token=36c1a0275ae2708954a402871a27255bcecf9aa24f7542a43ff0c5e1f651b19b
```

### 方法 2: 使用本地存储

第一次访问时使用 URL 参数，令牌会自动保存到本地存储。之后访问时会自动使用保存的令牌。

### 方法 3: 手动更新默认值

编辑文件：`desktop-client/src-ui/src/app/utils/tokenManager.ts`

```typescript
// 返回默认值（应该在启动脚本中更新）
return '36c1a0275ae2708954a402871a27255bcecf9aa24f7542a43ff0c5e1f651b19b';
```

---

## 令牌管理器 API

### TokenManager 类

位置：`desktop-client/src-ui/src/app/utils/tokenManager.ts`

#### 方法

| 方法 | 说明 |
|------|------|
| `getToken()` | 获取认证令牌（自动选择最佳来源） |
| `saveToken(token)` | 保存令牌到本地存储 |
| `clearToken()` | 清除本地存储的令牌 |
| `isTokenValid(token)` | 检查令牌是否有效 |
| `getTokenSummary(token)` | 获取令牌摘要（用于日志） |

#### 使用示例

```typescript
import { TokenManager } from '../../utils/tokenManager';

// 获取令牌
const token = TokenManager.getToken();

// 保存令牌
TokenManager.saveToken('new-token-value');

// 清除令牌
TokenManager.clearToken();

// 检查令牌有效性
if (TokenManager.isTokenValid(token)) {
  console.log('令牌有效');
}

// 获取令牌摘要
console.log(TokenManager.getTokenSummary(token));
// 输出: 36c1a027...651b19b
```

---

## 前端集成

### ChatTabWithAiSdk 组件

```typescript
import { TokenManager } from '../../utils/tokenManager';

export function ChatTabWithAiSdk() {
  // 获取认证令牌
  const authToken = TokenManager.getToken();

  // 使用令牌
  const chat = useAiChat({
    threadId: selectedConversation || '',
    apiUrl: 'http://localhost:3000',
    authToken: authToken,
  });
  
  // ...
}
```

---

## 完整的启动流程

### 步骤 1: 启动后端和前端

```bash
export LLM_API_KEY="sk-d162b1775e514757b083a2dfe729d6e6"
bash scripts/start-all.sh
```

### 步骤 2: 获取令牌

启动脚本会显示：
```
✅ 网关 URL: http://127.0.0.1:3000/?token=36c1a0275ae2708954a402871a27255bcecf9aa24f7542a43ff0c5e1f651b19b
```

### 步骤 3: 访问前端

使用令牌参数访问前端：
```
http://localhost:5173?token=36c1a0275ae2708954a402871a27255bcecf9aa24f7542a43ff0c5e1f651b19b
```

### 步骤 4: 令牌自动保存

令牌会自动保存到本地存储，下次访问时无需添加参数。

---

## 故障排除

### Q: 仍然显示 401 Unauthorized

**A**: 检查以下几点：

1. 令牌是否正确
   ```typescript
   console.log(TokenManager.getToken());
   ```

2. 后端是否运行
   ```bash
   curl http://localhost:3000/api/health
   ```

3. 清除本地存储并重新访问
   ```javascript
   // 在浏览器控制台运行
   localStorage.removeItem('gateway_auth_token');
   location.reload();
   ```

### Q: 如何更新令牌

**A**: 有两种方式：

1. **清除本地存储并使用新的 URL 参数**
   ```javascript
   localStorage.removeItem('gateway_auth_token');
   ```
   然后访问：`http://localhost:5173?token=new-token`

2. **直接调用 TokenManager**
   ```typescript
   TokenManager.saveToken('new-token');
   location.reload();
   ```

---

## 最佳实践

1. **总是使用 URL 参数启动** - 确保获取最新的令牌
2. **定期清除本地存储** - 如果令牌过期，清除本地存储
3. **检查浏览器控制台** - 查看令牌获取的日志
4. **使用令牌摘要** - 便于调试

---

## 相关文件

- `desktop-client/src-ui/src/app/utils/tokenManager.ts` - 令牌管理器
- `desktop-client/src-ui/src/app/components/tabs/ChatTabWithAiSdk.tsx` - 聊天组件
- `scripts/start-all.sh` - 启动脚本

