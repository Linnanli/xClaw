# 事件监听器泄漏问题修复

## 问题现象

### 问题 1: 频繁调用取消订阅
客户端不停地调用：
- `ipc://localhost/unsubscribe_chat_events`
- `ipc://localhost/plugin%3Aevent%7Cunlisten`

导致性能问题和资源泄漏。

### 问题 2: 重复订阅错误
```json
{
  "message": "Already subscribed to chat events",
  "type": "InvalidOperation"
}
```

## 根本原因

### 1. useEffect 依赖项导致频繁重新订阅

**问题代码**：
```typescript
useEffect(() => {
  // 订阅事件...
  return () => {
    // 清理...
    invoke('unsubscribe_chat_events');
  };
}, [handleChatEvent, onError]); // ❌ handleChatEvent 每次渲染都会变化
```

**问题分析**：
- `handleChatEvent` 是用 `useCallback` 创建的，但依赖了 `onError` 和 `onStatusChange`
- 每次这些回调变化时，`handleChatEvent` 就会重新创建
- 导致 `useEffect` 重新运行，重新订阅事件
- 旧的订阅被清理，新的订阅被创建
- 形成订阅-取消订阅的循环

### 2. 清理函数执行时机问题

**问题**：
- React 的清理函数可能在组件重新渲染时执行
- 如果 effect 频繁重新运行，清理函数也会频繁执行
- 导致 `unsubscribe_chat_events` 被频繁调用

### 3. 后端不支持幂等订阅

**问题**：
- 后端检查是否已订阅，如果已订阅就返回错误
- 前端在热重载或组件重新挂载时会尝试重新订阅
- 导致"Already subscribed"错误

## 解决方案

### 1. 使用 useRef 存储事件处理器

**修复后**：
```typescript
// 使用 ref 存储回调，避免作为依赖项
const handleChatEventRef = useRef((event: ChatEvent) => {
  // 事件处理逻辑...
});

// 单独的 effect 更新 ref
useEffect(() => {
  handleChatEventRef.current = (event: ChatEvent) => {
    // 使用最新的 onError 和 onStatusChange
  };
}, [onError, onStatusChange]);
```

**优点**：
- `handleChatEventRef` 引用永远不变
- 不会触发订阅 effect 重新运行
- 但仍能使用最新的回调函数

### 2. 空依赖数组，只订阅一次

**修复后**：
```typescript
useEffect(() => {
  // 订阅事件...
  const unlisten = await listen('chat-event', (event) => {
    handleChatEventRef.current(event.payload); // 使用 ref
  });

  return () => {
    // 清理...
  };
}, []); // ✅ 空依赖数组，只在挂载/卸载时执行
```

**优点**：
- 只在组件挂载时订阅一次
- 只在组件卸载时取消订阅一次
- 避免频繁的订阅-取消订阅循环

### 3. 防止重复清理

**修复后**：
```typescript
useEffect(() => {
  let cleanupExecuted = false;

  // 订阅逻辑...

  return () => {
    if (cleanupExecuted) {
      console.log('⚠️  Cleanup already executed, skipping...');
      return;
    }
    cleanupExecuted = true;

    // 清理逻辑...
  };
}, []);
```

**优点**：
- 防止清理函数被意外多次调用
- 提供调试日志

### 4. 后端支持幂等订阅

**修复后 (Rust)**：
```rust
pub async fn subscribe_chat_events(...) -> Result<()> {
    if manager.lock().await.is_subscribed().await {
        tracing::info!("ℹ️  Already subscribed, returning success");
        return Ok(()); // ✅ 幂等操作：已订阅时直接返回成功
    }
    // 订阅逻辑...
}
```

**修复后 (TypeScript)**：
```typescript
try {
  await invoke('subscribe_chat_events');
  console.log('✅ Chat events subscribed');
} catch (err) {
  // 忽略"已订阅"错误
  const errorMsg = err instanceof Error ? err.message : String(err);
  if (errorMsg.includes('Already subscribed')) {
    console.log('ℹ️  Already subscribed to chat events');
  } else {
    throw err;
  }
}
```

**优点**：
- 支持多次调用订阅命令
- 避免"Already subscribed"错误
- 更健壮的错误处理

## 修复前后对比

### 修复前

```
组件渲染 → 订阅事件
  ↓
onError 变化 → handleChatEvent 重新创建
  ↓
useEffect 重新运行 → 取消订阅 + 重新订阅
  ↓
onStatusChange 变化 → handleChatEvent 重新创建
  ↓
useEffect 重新运行 → 取消订阅 + 重新订阅
  ↓
循环往复... 💥
```

### 修复后

```
组件挂载 → 订阅事件（一次）
  ↓
onError 变化 → 更新 handleChatEventRef.current
  ↓
onStatusChange 变化 → 更新 handleChatEventRef.current
  ↓
组件卸载 → 取消订阅（一次）
```

## 验证方法

### 1. 检查控制台日志

修复后应该只看到：
```
🔗 Setting up chat event listener...
✅ Chat events subscribed
```

而不是频繁的：
```
🔗 Setting up chat event listener...
🧹 Cleaning up chat event listener...
🔗 Setting up chat event listener...
🧹 Cleaning up chat event listener...
```

### 2. 检查网络面板

修复后不应该看到频繁的：
- `unsubscribe_chat_events` 调用
- `plugin:event|unlisten` 调用

### 3. 性能监控

修复后应该看到：
- CPU 使用率降低
- 内存使用稳定
- 无内存泄漏

## 最佳实践

### 1. 事件监听器生命周期管理

```typescript
useEffect(() => {
  // 订阅
  const unlisten = await listen('event-name', handler);

  // 清理
  return () => {
    unlisten();
  };
}, []); // 空依赖数组
```

### 2. 使用 useRef 避免依赖变化

```typescript
const handlerRef = useRef(handler);

useEffect(() => {
  handlerRef.current = handler;
}, [handler]);

useEffect(() => {
  const unlisten = await listen('event-name', (e) => {
    handlerRef.current(e);
  });
  return () => unlisten();
}, []); // 不依赖 handler
```

### 3. 防止重复清理

```typescript
useEffect(() => {
  let cleaned = false;

  return () => {
    if (cleaned) return;
    cleaned = true;
    // 清理逻辑...
  };
}, []);
```

## 相关文档

- [React useEffect 清理函数](https://react.dev/reference/react/useEffect#cleanup-function)
- [Tauri Event System](https://v2.tauri.app/reference/javascript/api/namespacecore/#listen)
- `desktop-client/src-ui/src/app/hooks/useAiChatTauri.ts` - 修复后的代码

## 经验教训

1. **useEffect 依赖项要谨慎** - 每个依赖项的变化都会触发 effect 重新运行
2. **事件监听器要正确清理** - 避免内存泄漏和性能问题
3. **使用 useRef 存储回调** - 当回调需要访问最新状态但不应触发 effect 时
4. **空依赖数组用于一次性订阅** - 订阅/取消订阅应该只在挂载/卸载时执行
5. **添加调试日志** - 帮助发现和诊断问题
