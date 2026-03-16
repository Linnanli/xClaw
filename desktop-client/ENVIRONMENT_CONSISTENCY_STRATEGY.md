# 环境一致性策略

## 问题背景

在 AI Agent 编程中，常见的环境不一致问题包括：

1. **测试环境 vs 实际环境**
   - 单元测试：模拟环境，不涉及网络
   - 集成测试：本地环境，可能与生产环境不同
   - 实际应用：浏览器、移动端等真实环境

2. **开发环境 vs 生产环境**
   - 依赖版本差异
   - 配置差异
   - 网络条件差异

3. **跨平台差异**
   - 浏览器 API 差异
   - 操作系统差异
   - 网络库差异

## 当前 AI Agent 编程界的解决方案

### 1. 容器化和虚拟化

**方案**：使用 Docker 确保开发、测试、生产环境一致

```dockerfile
# Dockerfile
FROM rust:latest
WORKDIR /app
COPY . .
RUN cargo build --release
CMD ["cargo", "run", "--release"]
```

**优点**：
- 完全隔离环境
- 可重现性强
- 易于扩展

**缺点**：
- 增加复杂性
- 性能开销
- 调试困难

### 2. 配置管理

**方案**：使用配置文件管理不同环境的差异

```yaml
# config.dev.yaml
api:
  base_url: http://localhost:3000
  timeout: 30s
  retry: 3

# config.prod.yaml
api:
  base_url: https://api.example.com
  timeout: 10s
  retry: 5
```

**优点**：
- 灵活性高
- 易于维护
- 支持多环境

**缺点**：
- 需要手动管理
- 容易出错
- 难以追踪变化

### 3. 特性标志（Feature Flags）

**方案**：使用特性标志控制不同环境的行为

```rust
#[cfg(feature = "dev")]
fn get_api_url() -> &'static str {
    "http://localhost:3000"
}

#[cfg(feature = "prod")]
fn get_api_url() -> &'static str {
    "https://api.example.com"
}
```

**优点**：
- 编译时确定
- 零运行时开销
- 类型安全

**缺点**：
- 需要重新编译
- 不灵活
- 难以动态切换

### 4. 依赖注入（Dependency Injection）

**方案**：通过依赖注入管理不同环境的实现

```rust
trait ApiClient {
    async fn get_threads(&self) -> Result<Vec<Thread>>;
}

struct HttpApiClient {
    base_url: String,
    auth_token: String,
}

struct MockApiClient {
    threads: Vec<Thread>,
}

impl ApiClient for HttpApiClient {
    async fn get_threads(&self) -> Result<Vec<Thread>> {
        // 真实实现
    }
}

impl ApiClient for MockApiClient {
    async fn get_threads(&self) -> Result<Vec<Thread>> {
        // 模拟实现
    }
}
```

**优点**：
- 灵活性高
- 易于测试
- 易于扩展

**缺点**：
- 增加代码复杂性
- 运行时开销
- 需要更多的抽象

### 5. 环境变量

**方案**：使用环境变量管理配置

```bash
# .env.dev
API_BASE_URL=http://localhost:3000
API_TIMEOUT=30
API_RETRY=3

# .env.prod
API_BASE_URL=https://api.example.com
API_TIMEOUT=10
API_RETRY=5
```

```rust
let api_url = std::env::var("API_BASE_URL")
    .unwrap_or_else(|_| "http://localhost:3000".to_string());
```

**优点**：
- 简单易用
- 无需重新编译
- 广泛支持

**缺点**：
- 容易遗漏
- 难以验证
- 运行时错误

### 6. 测试容器（Testcontainers）

**方案**：在测试中启动真实的服务容器

```rust
#[tokio::test]
async fn test_with_real_backend() {
    let container = testcontainers::clients::Cli::default()
        .run(testcontainers::images::generic::GenericImage::new("backend:latest"));
    
    let port = container.get_host_port_ipv4(3000);
    let api_url = format!("http://localhost:{}", port);
    
    // 测试代码
}
```

**优点**：
- 接近真实环境
- 可重现性强
- 易于调试

**缺点**：
- 测试速度慢
- 需要 Docker
- 资源消耗大

### 7. 契约测试（Contract Testing）

**方案**：定义前后端的契约，确保兼容性

```rust
// 后端契约
#[test]
fn test_api_contract() {
    let response = api_client.get_threads();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("threads"));
}

// 前端契约
#[test]
fn test_api_contract() {
    const EXPECTED_RESPONSE = r#"{"threads": [...]}"#;
    assert_eq!(actual_response, EXPECTED_RESPONSE);
}
```

**优点**：
- 确保兼容性
- 早期发现问题
- 易于维护

**缺点**：
- 需要维护契约
- 增加测试复杂性
- 可能过度设计

### 8. 集成测试框架

**方案**：使用专门的集成测试框架

```rust
// 使用 testify 或类似框架
#[integration_test]
async fn test_sse_integration() {
    let server = TestServer::new().await;
    let client = TestClient::new(server.url());
    
    let result = client.connect_sse().await;
    assert!(result.is_ok());
}
```

**优点**：
- 专门为集成测试设计
- 提供丰富的工具
- 易于使用

**缺点**：
- 学习曲线
- 可能过度设计
- 依赖特定框架

## 推荐的综合方案

### 分层测试策略

```
┌─────────────────────────────────────────┐
│         端到端测试（E2E）               │
│  使用真实浏览器和后端，测试完整流程    │
│  工具：Playwright, Cypress              │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│       集成测试（Integration）           │
│  使用 Testcontainers 启动真实服务      │
│  测试组件间的交互                       │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│       单元测试（Unit）                  │
│  使用 Mock/Stub，测试单个组件          │
│  快速、隔离、可重现                     │
└─────────────────────────────────────────┘
```

### 实现步骤

#### 1. 定义环境配置

```rust
// src/config.rs
#[derive(Debug, Clone)]
pub struct Config {
    pub api_base_url: String,
    pub api_timeout: Duration,
    pub api_retry: u32,
    pub environment: Environment,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Environment {
    Development,
    Testing,
    Production,
}

impl Config {
    pub fn from_env() -> Self {
        let env = std::env::var("ENVIRONMENT")
            .unwrap_or_else(|_| "development".to_string());
        
        match env.as_str() {
            "testing" => Self::testing(),
            "production" => Self::production(),
            _ => Self::development(),
        }
    }
    
    pub fn development() -> Self {
        Self {
            api_base_url: "http://localhost:3000".to_string(),
            api_timeout: Duration::from_secs(30),
            api_retry: 3,
            environment: Environment::Development,
        }
    }
    
    pub fn testing() -> Self {
        Self {
            api_base_url: "http://localhost:3001".to_string(),
            api_timeout: Duration::from_secs(5),
            api_retry: 1,
            environment: Environment::Testing,
        }
    }
    
    pub fn production() -> Self {
        Self {
            api_base_url: "https://api.example.com".to_string(),
            api_timeout: Duration::from_secs(10),
            api_retry: 5,
            environment: Environment::Production,
        }
    }
}
```

#### 2. 使用依赖注入

```rust
// src/api_client.rs
pub trait ApiClient: Send + Sync {
    async fn get_threads(&self) -> Result<Vec<Thread>>;
    async fn send_message(&self, thread_id: &str, content: &str) -> Result<Message>;
}

pub struct HttpApiClient {
    config: Config,
    client: reqwest::Client,
}

pub struct MockApiClient {
    threads: Vec<Thread>,
}

impl ApiClient for HttpApiClient {
    async fn get_threads(&self) -> Result<Vec<Thread>> {
        // 真实实现
    }
}

impl ApiClient for MockApiClient {
    async fn get_threads(&self) -> Result<Vec<Thread>> {
        Ok(self.threads.clone())
    }
}
```

#### 3. 创建测试工厂

```rust
// tests/support/test_factory.rs
pub struct TestFactory;

impl TestFactory {
    pub fn create_api_client() -> Arc<dyn ApiClient> {
        if std::env::var("USE_MOCK_API").is_ok() {
            Arc::new(MockApiClient::new())
        } else {
            Arc::new(HttpApiClient::new(Config::testing()))
        }
    }
    
    pub async fn create_test_server() -> TestServer {
        TestServer::new().await
    }
}
```

#### 4. 编写分层测试

```rust
// tests/unit_tests.rs
#[test]
fn test_message_parsing() {
    let json = r#"{"id": "123", "content": "Hello"}"#;
    let message: Message = serde_json::from_str(json).unwrap();
    assert_eq!(message.id, "123");
}

// tests/integration_tests.rs
#[tokio::test]
async fn test_api_integration() {
    let client = TestFactory::create_api_client();
    let threads = client.get_threads().await.unwrap();
    assert!(!threads.is_empty());
}

// tests/e2e_tests.rs
#[tokio::test]
async fn test_sse_e2e() {
    let server = TestFactory::create_test_server().await;
    let browser = launch_browser().await;
    
    browser.goto(&format!("http://localhost:5173")).await;
    browser.wait_for_element(".sse-connected").await;
}
```

## 最佳实践

### 1. 环境隔离
- 使用不同的端口
- 使用不同的数据库
- 使用不同的配置文件

### 2. 配置管理
- 使用环境变量
- 使用配置文件
- 使用特性标志

### 3. 测试策略
- 单元测试：快速、隔离
- 集成测试：真实环境、可重现
- 端到端测试：完整流程、用户视角

### 4. 监控和日志
- 记录环境信息
- 记录配置信息
- 记录错误堆栈

### 5. 文档化
- 记录环境差异
- 记录配置选项
- 记录故障排除步骤

## 工具和框架

### Rust 生态
- **testcontainers-rs**：启动容器进行测试
- **mockall**：生成 Mock 对象
- **proptest**：属性测试
- **criterion**：性能测试

### 前端生态
- **Playwright**：浏览器自动化
- **Cypress**：端到端测试
- **Vitest**：单元测试
- **MSW**：Mock Service Worker

### 通用工具
- **Docker Compose**：多容器编排
- **Kubernetes**：容器编排
- **Terraform**：基础设施即代码
- **GitHub Actions**：CI/CD

## 总结

解决环境不一致问题的关键是：

1. **分层测试**：单元 → 集成 → 端到端
2. **配置管理**：环境变量 + 配置文件
3. **依赖注入**：灵活切换实现
4. **容器化**：确保环境一致
5. **监控日志**：快速定位问题
6. **文档化**：记录差异和解决方案

通过这些方案的组合使用，可以有效地解决环境不一致问题，提高代码质量和可维护性。
