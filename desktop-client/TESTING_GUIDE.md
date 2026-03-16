# Desktop Client API 集成测试指南

## 快速开始

### 运行所有测试

```bash
cd desktop-client
cargo test
```

### 运行特定测试套件

```bash
# 运行集成测试
cargo test --test api_integration_tests

# 运行错误处理测试
cargo test --test api_error_tests

# 运行属性测试
cargo test --test api_property_tests
```

### 运行特定测试

```bash
# 运行单个测试
cargo test --test api_integration_tests test_create_thread

# 运行匹配模式的测试
cargo test --test api_integration_tests chat
```

### 显示测试输出

```bash
# 显示 println! 输出
cargo test -- --nocapture

# 显示测试执行时间
cargo test -- --test-threads=1
```

## 测试文件说明

### api_integration_tests.rs

**目的**: 验证所有 API 端点的基本功能

**测试数**: 30 个

**覆盖范围**:
- 聊天接口（创建对话、发送消息等）
- 记忆接口（获取、读取、写入、搜索）
- 任务接口（获取、详情、取消、重启）
- 日志接口（获取、搜索、过滤、导出、清空）
- 批准接口（批准、拒绝）

**运行**:
```bash
cargo test --test api_integration_tests
```

### api_error_tests.rs

**目的**: 验证错误处理、认证、数据解析和性能

**测试数**: 24 个

**覆盖范围**:
- HTTP 错误处理
- 认证和授权
- 数据解析（可选字段、嵌套结构、数组）
- 端到端流程
- 性能和并发

**运行**:
```bash
cargo test --test api_error_tests
```

### api_property_tests.rs

**目的**: 使用属性测试验证通用特性

**测试数**: 17 个

**覆盖范围**:
- 数据序列化 round-trip
- API 调用成功返回有效数据
- 写入操作幂等性
- 查询参数正确编码

**运行**:
```bash
cargo test --test api_property_tests
```

## 测试支持模块

### TestServer

用于启动和管理测试用的 Web Gateway 实例。

**使用示例**:
```rust
let server = TestServer::start().await.unwrap();
let client = server.create_client();

// 使用 client 进行 API 调用
let jobs = client.get_jobs().await.unwrap();
```

**自定义认证令牌**:
```rust
let server = TestServer::start_with_token("custom-token").await.unwrap();
let client = server.create_client_with_token("different-token");
```

### TestFixture

提供测试数据创建和清理功能。

**使用示例**:
```rust
let mut fixture = TestFixture::new().await.unwrap();

// 创建对话
let thread = fixture.create_thread().await.unwrap();

// 发送消息
let response = fixture.send_message(&thread.id, "Hello").await.unwrap();

// 获取 API 客户端
let client = fixture.client();
let jobs = client.get_jobs().await.unwrap();
```

### 数据生成器

为属性测试提供随机数据生成。

**可用生成器**:
- `arb_message_content()` - 消息内容
- `arb_thread_id()` - 对话 ID
- `arb_send_message_request()` - 发送消息请求
- `arb_thread_info()` - 对话信息
- `arb_message()` - 消息
- `arb_job_info()` - 任务信息
- `arb_log_entry()` - 日志条目

**使用示例**:
```rust
proptest! {
    #[test]
    fn prop_round_trip(req in arb_send_message_request()) {
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: SendMessageRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req.content, deserialized.content);
    }
}
```

## 常见任务

### 添加新的集成测试

1. 在 `api_integration_tests.rs` 中添加新的测试函数：

```rust
#[tokio::test]
async fn test_new_feature() {
    let mut fixture = TestFixture::new().await.unwrap();
    
    // 测试代码
    let result = fixture.client().some_api_call().await;
    assert!(result.is_ok());
}
```

2. 运行测试验证：

```bash
cargo test --test api_integration_tests test_new_feature
```

### 添加新的属性测试

1. 在 `api_property_tests.rs` 中添加新的属性测试：

```rust
proptest! {
    #[test]
    fn prop_new_property(data in arb_some_type()) {
        // 属性测试代码
        assert!(some_property(&data));
    }
}
```

2. 运行测试验证：

```bash
cargo test --test api_property_tests prop_new_property
```

### 调试测试失败

1. 运行单个失败的测试：

```bash
cargo test --test api_integration_tests test_name -- --nocapture
```

2. 查看详细的错误信息：

```bash
RUST_BACKTRACE=1 cargo test --test api_integration_tests test_name -- --nocapture
```

3. 使用 println! 调试：

```rust
#[tokio::test]
async fn test_debug() {
    let fixture = TestFixture::new().await.unwrap();
    let result = fixture.client().get_jobs().await;
    println!("Result: {:?}", result);
    assert!(result.is_ok());
}
```

## 性能优化

### 并行执行测试

```bash
# 使用多个线程运行测试（默认）
cargo test

# 使用单个线程运行测试（用于调试）
cargo test -- --test-threads=1
```

### 只运行快速测试

```bash
# 跳过性能测试
cargo test --test api_integration_tests -- --skip concurrent
```

## 故障排查

### 测试超时

如果测试超时，可能是：
1. 服务器启动缓慢
2. 网络连接问题
3. 数据库初始化缓慢

**解决方案**:
```bash
# 增加超时时间
RUST_TEST_TIME_UNIT=60000 cargo test
```

### 端口冲突

如果出现端口冲突错误：
1. 确保使用动态端口（已默认配置）
2. 检查是否有残留的服务器进程

**解决方案**:
```bash
# 查找占用端口的进程
lsof -i :3000

# 杀死进程
kill -9 <PID>
```

### 数据库错误

如果出现数据库错误：
1. 确保内存数据库正确初始化
2. 检查迁移是否成功

**解决方案**:
```bash
# 运行单个测试查看详细错误
cargo test --test api_integration_tests test_name -- --nocapture
```

## 测试统计

### 总体统计

- **总测试数**: 71
- **通过率**: 100%
- **执行时间**: ~3-5 秒

### 按类型分类

| 类型 | 数量 |
|------|------|
| 集成测试 | 30 |
| 错误处理测试 | 24 |
| 属性测试 | 17 |

### 按功能分类

| 功能 | 测试数 |
|------|--------|
| 聊天接口 | 23 |
| 记忆接口 | 4 |
| 任务接口 | 4 |
| 日志接口 | 5 |
| 批准接口 | 2 |
| 错误处理 | 4 |
| 认证 | 3 |
| 数据解析 | 3 |
| 端到端流程 | 5 |
| 性能可靠性 | 3 |
| 生成器 | 9 |

## 最佳实践

### 编写测试

1. **使用有意义的测试名称**
   ```rust
   // ✅ 好
   #[tokio::test]
   async fn test_create_thread_returns_valid_id() { }
   
   // ❌ 不好
   #[tokio::test]
   async fn test1() { }
   ```

2. **添加测试注释**
   ```rust
   #[tokio::test]
   async fn test_create_thread() {
       // Feature: api-integration-tests, Task 2.1
       // 验证需求: 1.2, 6.1, 6.2
       // 测试 create_thread() 接口
   }
   ```

3. **使用 fixture 进行设置**
   ```rust
   let mut fixture = TestFixture::new().await.unwrap();
   let thread = fixture.create_thread().await.unwrap();
   ```

4. **验证关键属性**
   ```rust
   assert!(!thread.id.is_empty(), "thread ID should not be empty");
   assert_eq!(thread.state, "Idle", "new thread should be in Idle state");
   ```

### 运行测试

1. **在提交前运行所有测试**
   ```bash
   cargo test
   ```

2. **在 CI/CD 中运行测试**
   ```bash
   cargo test --all
   ```

3. **定期检查测试覆盖率**
   ```bash
   cargo tarpaulin --out Html
   ```

## 相关文档

- [需求文档](../../.kiro/specs/api-integration-tests/requirements.md)
- [设计文档](../../.kiro/specs/api-integration-tests/design.md)
- [任务清单](../../.kiro/specs/api-integration-tests/tasks.md)
- [完成总结](../../.kiro/specs/api-integration-tests/COMPLETION_SUMMARY.md)
- [实现报告](../../.kiro/specs/api-integration-tests/IMPLEMENTATION_REPORT.md)

## 支持

如有问题或需要帮助，请参考：
1. 测试文件中的注释
2. 支持模块的文档
3. 相关的设计文档
