# SSE 测试最佳实践

## 集成测试和实际环境差异

### 1. 认证方式

#### 集成测试（Rust）
```rust
// 使用 HTTP 头传递认证令牌
let client = reqwest::Client::new();
let response = client
    .get("http://localhost:3000/api/chat/events")
    .header("Authorization", format!("Bearer {}", token))
    .send()
    .await?;
```

**优点**：
- 标准的 HTTP 认证方式
- 安全性高（令牌不在 URL 中）
- 支持所有 HTTP 方法

**缺点**：
- EventSource API 不支持自定义头

#### 实际环境（浏览器）
```typescript
// 使用 URL 参数传递认证令牌
const url = `http://localhost:3000/api/chat/events?token=${encodeURIComponent(token)}`;
const eventSource = new EventSource(url);
```

**优点**：
- EventSource API 支持
- 浏览器兼容性好

**缺点**：
- 令牌在 URL 中（可能泄露到日志、Referer 头等）
- 需要后端支持 URL 参数认证

### 2. CORS 处理

#### 集成测试
- 无 CORS 预检请求
- 直接发送 GET 请求

#### 实际环境
- 浏览器发送 OPTIONS 预检请求
- 需要后端返回正确的 CORS 头

**解决方案**：
```rust
let cors = CorsLayer::new()
    .allow_origin([...])
    .allow_methods([
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::DELETE,
        Method::OPTIONS,  // 必须包含 OPTIONS
    ])
    .allow_headers(AllowHeaders::list([
        header::CONTENT_TYPE,
        header::AUTHORIZATION,
    ]))
    .allow_credentials(true);
```

### 3. 连接方式

#### 集成测试
```rust
// 使用 reqwest 的流式响应
let mut stream = response.bytes_stream();
while let Some(chunk) = stream.next().await {
    // 处理数据块
}
```

#### 实际环境
```typescript
// 使用 EventSource API
const eventSource = new EventSource(url);
eventSource.onmessage = (event) => {
    // 处理消息
};
```

### 4. 错误处理

#### 集成测试
- 网络错误：直接返回错误
- 认证错误：HTTP 401 状态码

#### 实际环境
- 网络错误：EventSource 触发 `onerror` 事件
- 认证错误：HTTP 401 状态码（EventSource 会自动重连）
- CORS 错误：浏览器控制台显示错误，EventSource 无法连接

### 5. 最佳实践

#### 后端实现
1. **支持两种认证方式**
   ```rust
   // 优先使用 HTTP 头
   if let Some(token) = extract_bearer_token(&headers) {
       validate_token(token)?;
   }
   // 回退到 URL 参数（仅限 SSE 端点）
   else if let Some(token) = extract_query_token(&request) {
       validate_token(token)?;
   }
   else {
       return Err(StatusCode::UNAUTHORIZED);
   }
   ```

2. **正确配置 CORS**
   - 包含 OPTIONS 方法
   - 设置正确的 Allow-Origin 头
   - 设置 Allow-Credentials 为 true

3. **SSE 端点特殊处理**
   - 允许 URL 参数认证
   - 设置正确的 Content-Type: text/event-stream
   - 设置 Cache-Control: no-cache
   - 设置 X-Accel-Buffering: no

#### 前端实现
1. **使用 EventSource API**
   ```typescript
   const url = `${baseUrl}/api/chat/events?token=${encodeURIComponent(token)}`;
   const eventSource = new EventSource(url);
   ```

2. **处理连接状态**
   ```typescript
   eventSource.onopen = () => {
       console.log('Connected');
       setConnected(true);
   };
   
   eventSource.onerror = () => {
       console.error('Connection error');
       setConnected(false);
   };
   ```

3. **实现重连机制**
   ```typescript
   const attemptReconnect = () => {
       if (reconnectAttempts < maxAttempts) {
           setTimeout(() => {
               connectSSE();
           }, reconnectDelay);
       }
   };
   ```

#### 测试实现
1. **集成测试**
   - 使用 HTTP 头认证
   - 测试 SSE 事件流
   - 测试错误处理

2. **端到端测试**
   - 使用浏览器自动化工具（如 Playwright）
   - 测试 EventSource 连接
   - 测试 CORS 预检

3. **单元测试**
   - 测试事件解析
   - 测试错误处理
   - 测试重连逻辑

### 6. 调试技巧

#### 浏览器开发者工具
1. **Network 标签**
   - 查看 OPTIONS 预检请求
   - 查看 SSE 连接状态
   - 查看响应头

2. **Console 标签**
   - 查看 EventSource 错误
   - 查看 CORS 错误

3. **Application 标签**
   - 查看 Cookie（如果使用）

#### 后端日志
```rust
tracing::debug!("SSE connection from {:?}", remote_addr);
tracing::debug!("Auth token: {}", token);
tracing::debug!("Sending event: {:?}", event);
```

### 7. 常见问题

#### Q: EventSource 连接卡住
**A**: 
- 检查后端是否正确处理 OPTIONS 预检请求
- 检查 CORS 头是否正确
- 检查认证令牌是否正确

#### Q: CORS 错误
**A**:
- 确保 Allow-Origin 包含前端 URL
- 确保 Allow-Methods 包含 OPTIONS
- 确保 Allow-Headers 包含必要的头

#### Q: 认证失败
**A**:
- 检查令牌是否正确
- 检查令牌是否过期
- 检查后端是否支持 URL 参数认证

#### Q: 消息不实时显示
**A**:
- 检查 SSE 连接是否建立
- 检查后端是否发送事件
- 检查浏览器控制台是否有错误

## 参考资源

- [MDN: Server-Sent Events](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events)
- [MDN: CORS](https://developer.mozilla.org/en-US/docs/Web/HTTP/CORS)
- [Axum: CORS](https://docs.rs/tower-http/latest/tower_http/cors/)
- [EventSource API](https://html.spec.whatwg.org/multipage/server-sent-events.html)
