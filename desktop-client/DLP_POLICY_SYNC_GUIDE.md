# DLP 策略同步使用指南

## 概述

Desktop Client 现在支持从 Admin Backend 同步 DLP 策略配置，实现集中化的策略管理。

## 架构

```
Admin Backend (策略管理) ──HTTP API──> Desktop Client (策略应用)
       ✅ 策略 CRUD                        ✅ 策略同步
       ✅ 版本管理                         ✅ 自动应用
       ✅ 审计追踪                         ✅ 错误处理
```

## 配置步骤

### 1. 启动 Admin Backend

```bash
cd admin-backend
cargo run
```

Admin Backend 将在 `http://localhost:8080` 启动，提供以下 API：

- `GET /api/policies` - 获取所有策略
- `GET /api/policies/dlp` - 获取 DLP 规则
- `GET /api/policies/sensitive-ops` - 获取敏感操作规则
- `GET /api/policies/version` - 获取策略版本信息

### 2. 配置 Desktop Client

在 Desktop Client 中配置策略同步：

```rust
use desktop_client::enterprise_policy_sync::{EnterprisePolicySyncManager, RemotePolicyConfig};
use std::time::Duration;

// 创建策略同步配置
let config = RemotePolicyConfig {
    server_url: "http://localhost:8080".to_string(), // Admin Backend 地址
    sync_interval: Duration::from_secs(300),         // 5分钟同步一次
    auth_token: "your_api_token".to_string(),        // API 认证令牌
    enable_realtime: true,                           // 启用实时推送
    connection_timeout: Duration::from_secs(30),     // 连接超时
    max_retries: 3,                                  // 最大重试次数
};

// 创建策略同步管理器
let policy_manager = EnterprisePolicySyncManager::new(config);

// 启动同步服务
policy_manager.start_sync_service().await?;
```

### 3. 使用策略同步功能

#### 手动同步策略

```rust
// 手动触发策略同步
let result = policy_manager.sync_policies().await;
match result {
    Ok(()) => println!("策略同步成功"),
    Err(e) => eprintln!("策略同步失败: {}", e),
}
```

#### 检查同步状态

```rust
// 获取同步状态
let status = policy_manager.get_sync_status().await;
println!("同步状态: {:?}", status);

// 获取同步统计
let stats = policy_manager.get_sync_stats().await;
println!("总同步次数: {}", stats.total_syncs);
println!("成功次数: {}", stats.successful_syncs);
println!("DLP 策略数量: {}", stats.dlp_policies_count);
```

#### 获取本地策略

```rust
// 获取本地策略管理器
let local = policy_manager.get_local_manager().await;

// 获取 DLP 策略
let dlp_policies = local.get_dlp_policies();
for policy in dlp_policies {
    println!("DLP 规则: {} -> {}", policy.pattern, policy.replacement);
}

// 获取敏感操作策略
let sensitive_ops = local.get_sensitive_ops_policies();
for policy in sensitive_ops {
    println!("敏感操作: {} (需要审批: {})", policy.operation, policy.requires_approval);
}
```

#### 应用 DLP 策略

```rust
// 应用 DLP 策略到文本
let original_text = "我的身份证号是 330326199408015618";
let sanitized_text = local.apply_dlp_policy(original_text);
println!("脱敏后: {}", sanitized_text); // 输出: 我的身份证号是 ***************
```

## API 接口

### Admin Backend API

#### 获取所有策略

```http
GET /api/policies?include_disabled=false
Authorization: Bearer <token>

Response:
{
  "dlp_rules": [...],
  "sensitive_ops_rules": [...],
  "version": {
    "dlp_rules_version": 1,
    "sensitive_ops_version": 1,
    "last_updated": "2024-01-01T00:00:00Z",
    "total_dlp_rules": 5,
    "active_dlp_rules": 4
  }
}
```

#### 获取 DLP 规则

```http
GET /api/policies/dlp
Authorization: Bearer <token>

Response:
[
  {
    "id": "550e8400-e29b-41d4-a716-446655440001",
    "name": "身份证号脱敏",
    "pattern": "\\d{17}[\\dXx]",
    "replacement": "***************",
    "severity": "high",
    "enabled": true,
    "category": "pii",
    "description": "中国身份证号码脱敏规则"
  }
]
```

#### 获取敏感操作规则

```http
GET /api/policies/sensitive-ops
Authorization: Bearer <token>

Response:
[
  {
    "id": "550e8400-e29b-41d4-a716-446655440002",
    "name": "文件上传审批",
    "operation_type": "file_upload",
    "requires_approval": true,
    "risk_level": "high",
    "enabled": true,
    "description": "文件上传需要审批"
  }
]
```

#### 获取策略版本

```http
GET /api/policies/version
Authorization: Bearer <token>

Response:
{
  "dlp_rules_version": 1,
  "sensitive_ops_version": 1,
  "last_updated": "2024-01-01T00:00:00Z",
  "total_dlp_rules": 5,
  "total_sensitive_ops": 3,
  "active_dlp_rules": 4,
  "active_sensitive_ops": 2
}
```

## 错误处理

### 常见错误和解决方案

#### 1. 连接失败

```rust
// 错误: Failed to fetch DLP policies: error sending request
// 解决: 检查 Admin Backend 是否启动，URL 是否正确
let config = RemotePolicyConfig {
    server_url: "http://localhost:8080".to_string(), // 确保地址正确
    connection_timeout: Duration::from_secs(30),     // 增加超时时间
    max_retries: 3,                                  // 启用重试
    ..Default::default()
};
```

#### 2. 认证失败

```rust
// 错误: HTTP 401: Unauthorized
// 解决: 检查 API 令牌是否正确
let config = RemotePolicyConfig {
    auth_token: "valid_api_token".to_string(), // 使用有效的令牌
    ..Default::default()
};
```

#### 3. 策略解析失败

```rust
// 错误: Failed to parse DLP response
// 解决: 检查 Admin Backend 返回的数据格式
// 确保 Admin Backend 和 Desktop Client 版本兼容
```

### 错误监控

```rust
// 监控同步错误
let stats = policy_manager.get_sync_stats().await;
if stats.failed_syncs > 0 {
    eprintln!("警告: 有 {} 次同步失败", stats.failed_syncs);
    
    // 获取详细的错误信息
    let status = policy_manager.get_sync_status().await;
    if let PolicySyncStatus::Failed(error_msg) = status {
        eprintln!("最后一次错误: {}", error_msg);
    }
}
```

## 安全考虑

### 1. API 认证

- 使用强密码生成 API 令牌
- 定期轮换 API 令牌
- 限制令牌的访问权限

### 2. 网络安全

- 在生产环境中使用 HTTPS
- 配置防火墙规则限制访问
- 使用 VPN 或专用网络

### 3. 敏感信息保护

- API 令牌不要硬编码在代码中
- 使用环境变量或安全的配置管理
- 错误日志中不包含敏感信息

```rust
// 安全的配置方式
let config = RemotePolicyConfig {
    server_url: std::env::var("DLP_POLICY_SERVER_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string()),
    auth_token: std::env::var("DLP_POLICY_API_TOKEN")
        .expect("DLP_POLICY_API_TOKEN environment variable is required"),
    ..Default::default()
};
```

## 监控和调试

### 启用详细日志

```rust
// 在 main.rs 中启用 tracing
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

tracing_subscriber::registry()
    .with(tracing_subscriber::EnvFilter::new("desktop_client=debug"))
    .with(tracing_subscriber::fmt::layer())
    .init();
```

### 监控指标

```rust
// 定期检查同步健康状态
async fn monitor_policy_sync(manager: &EnterprisePolicySyncManager) {
    let stats = manager.get_sync_stats().await;
    
    // 检查同步频率
    if let Some(last_sync) = stats.last_successful_sync {
        let elapsed = current_timestamp() - last_sync;
        if elapsed > 600 { // 10分钟没有成功同步
            eprintln!("警告: 策略同步已超过10分钟未成功");
        }
    }
    
    // 检查失败率
    let failure_rate = stats.failed_syncs as f64 / stats.total_syncs as f64;
    if failure_rate > 0.1 { // 失败率超过10%
        eprintln!("警告: 策略同步失败率过高: {:.1}%", failure_rate * 100.0);
    }
}
```

## 性能优化

### 1. 同步间隔优化

```rust
// 根据策略变更频率调整同步间隔
let config = RemotePolicyConfig {
    sync_interval: Duration::from_secs(300), // 策略变更频繁时使用较短间隔
    // sync_interval: Duration::from_secs(3600), // 策略稳定时使用较长间隔
    ..Default::default()
};
```

### 2. 条件同步

```rust
// 只在需要时同步
if policy_manager.needs_sync().await? {
    policy_manager.sync_policies().await?;
} else {
    println!("策略已是最新版本，跳过同步");
}
```

### 3. 缓存策略

```rust
// 本地策略会自动缓存，避免重复网络请求
let local = policy_manager.get_local_manager().await;
let cached_policies = local.get_dlp_policies(); // 从本地缓存获取
```

## 故障排除

### 检查清单

1. **Admin Backend 状态**
   - [ ] Admin Backend 服务正在运行
   - [ ] 数据库连接正常
   - [ ] API 端点可访问

2. **网络连接**
   - [ ] Desktop Client 可以访问 Admin Backend
   - [ ] 防火墙规则正确配置
   - [ ] DNS 解析正常

3. **认证配置**
   - [ ] API 令牌有效
   - [ ] 令牌权限正确
   - [ ] 令牌未过期

4. **数据格式**
   - [ ] Admin Backend 返回正确的 JSON 格式
   - [ ] 数据结构与 Desktop Client 期望一致
   - [ ] 版本兼容性

### 调试命令

```bash
# 测试 Admin Backend API
curl -H "Authorization: Bearer <token>" http://localhost:8080/api/policies/version

# 检查 Desktop Client 日志
RUST_LOG=desktop_client=debug cargo run

# 运行集成测试
cargo test --lib tests::dlp_policy_sync_integration_tests
```

## 总结

DLP 策略同步功能提供了：

- ✅ 集中化策略管理
- ✅ 自动同步和应用
- ✅ 版本控制和追踪
- ✅ 错误处理和重试
- ✅ 安全认证和审计
- ✅ 性能优化和监控

通过这个系统，管理员可以在 Admin Backend 中统一管理 DLP 策略，Desktop Client 会自动同步并应用最新的策略配置，确保数据保护的一致性和有效性。