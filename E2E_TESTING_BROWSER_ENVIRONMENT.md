# 端到端测试中访问浏览器环境的方案

## 问题分析

在端到端（E2E）测试中，需要访问浏览器环境的原因：

1. **SSE 连接测试** - EventSource API 只在浏览器中可用
2. **DOM 操作测试** - 需要验证 UI 更新
3. **本地存储测试** - localStorage/sessionStorage 只在浏览器中可用
4. **Cookie 测试** - 需要验证 Cookie 管理
5. **网络请求拦截** - 需要验证 API 调用

## 可用的解决方案

### 方案 1：Tauri 测试驱动（推荐）

**优点**：
- 直接测试 Tauri 应用
- 可以访问 Tauri 命令
- 可以访问浏览器环境
- 接近真实用户场景

**缺点**：
- 需要启动完整的 Tauri 应用
- 测试速度较慢
- 需要配置测试环境

**实现方式**：

```rust
// desktop-client/tests/e2e_browser_tests.rs

use std::process::{Command, Child};
use std::thread;
use std::time::Duration;

pub struct TauriTestApp {
    process: Child,
}

impl TauriTestApp {
    pub fn start() -> Self {
        let process = Command::new("cargo")
            .args(&["tauri", "dev"])
            .spawn()
            .expect("Failed to start Tauri app");
        
        // 等待应用启动
        thread::sleep(Duration::from_secs(5));
        
        Self { process }
    }
}

impl Drop for TauriTestApp {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}

#[tokio::test]
async fn test_sse_connection_in_browser() {
    let _app = TauriTestApp::start();
    
    // 使用 WebDriver 或 Playwright 连接到浏览器
    // 测试 SSE 连接
}
```

### 方案 2：Playwright/Puppeteer（推荐用于 UI 测试）

**优点**：
- 完整的浏览器自动化
- 可以测试 UI 交互
- 支持多个浏览器
- 可以截图和录制视频

**缺点**：
- 需要额外的依赖
- 测试速度较慢
- 需要配置浏览器

**实现方式**：

```rust
// desktop-client/tests/e2e_playwright_tests.rs

use playwright::sync::*;

#[test]
fn test_sse_connection_with_playwright() {
    let playwright = Playwright::new();
    let browser = playwright.chromium().launch(Default::default()).unwrap();
    let context = browser.new_context(Default::default()).unwrap();
    let page = context.new_page(Default::default()).unwrap();
    
    // 导航到应用
    page.goto("http://localhost:5173", Default::default()).unwrap();
    
    // 等待 SSE 连接建立
    page.wait_for_selector(".sse-connected", Default::default()).unwrap();
    
    // 验证 SSE 连接状态
    let status = page.text_content(".sse-status", Default::default()).unwrap();
    assert_eq!(status, Some("Connected".to_string()));
}
```

### 方案 3：WebDriver（标准化方案）

**优点**：
- W3C 标准
- 支持多个浏览器
- 跨平台支持
- 与 CI/CD 集成良好

**缺点**：
- 需要 WebDriver 服务器
- 配置较复杂
- 文档较少

**实现方式**：

```rust
// desktop-client/tests/e2e_webdriver_tests.rs

use webdriver::client::WebDriver;

#[tokio::test]
async fn test_sse_connection_with_webdriver() {
    let mut driver = WebDriver::new("http://localhost:4444").await.unwrap();
    
    // 导航到应用
    driver.navigate("http://localhost:5173").await.unwrap();
    
    // 等待 SSE 连接
    driver.wait_for_element(".sse-connected", Duration::from_secs(10)).await.unwrap();
    
    // 验证连接状态
    let status = driver.find_element(".sse-status").await.unwrap();
    let text = status.text().await.unwrap();
    assert_eq!(text, "Connected");
}
```

### 方案 4：Cypress（最简单的方案）

**优点**：
- 最简单的 API
- 优秀的调试工具
- 自动等待
- 实时重新加载

**缺点**：
- 仅支持 Chromium 系浏览器
- 需要 Node.js
- 不支持跨浏览器测试

**实现方式**：

```javascript
// desktop-client/cypress/e2e/sse.cy.js

describe('SSE Connection Tests', () => {
  beforeEach(() => {
    cy.visit('http://localhost:5173');
  });

  it('should establish SSE connection', () => {
    cy.get('.sse-status').should('contain', 'Connected');
  });

  it('should receive SSE messages', () => {
    cy.get('.message-list').should('have.length.greaterThan', 0);
  });

  it('should handle SSE reconnection', () => {
    // 模拟网络中断
    cy.intercept('GET', '**/api/events', { forceNetworkError: true });
    
    // 等待重连
    cy.get('.sse-status').should('contain', 'Reconnecting');
    
    // 恢复网络
    cy.intercept('GET', '**/api/events', { statusCode: 200 });
    
    // 验证重连成功
    cy.get('.sse-status').should('contain', 'Connected');
  });
});
```

### 方案 5：Tauri 集成测试 + 浏览器模拟

**优点**：
- 可以测试 Tauri 命令
- 可以测试浏览器环境
- 可以测试集成
- 速度较快

**缺点**：
- 需要自定义实现
- 需要浏览器模拟库

**实现方式**：

```rust
// desktop-client/tests/e2e_integration_tests.rs

use tauri::test::mock_builder;
use tauri::Manager;

#[tokio::test]
async fn test_sse_with_tauri_integration() {
    let app = mock_builder()
        .build()
        .expect("Failed to build app");
    
    // 测试 Tauri 命令
    let init_info = app.invoke_handler(|invoke| {
        match invoke.message.command.as_str() {
            "get_app_init_info" => {
                invoke.resolve(serde_json::json!({
                    "auth_token": "test_token",
                    "api_base_url": "http://localhost:3000",
                    "database_type": "sqlite",
                    "os": "macOS",
                    "log_level": "debug",
                    "environment": "testing"
                }))
            }
            _ => invoke.reject("Unknown command")
        }
    }).await;
    
    // 验证初始化信息
    assert_eq!(init_info.api_base_url, "http://localhost:3000");
}
```

## 推荐方案

### 对于 SSE 测试

**推荐：Playwright + Tauri 应用**

```rust
// desktop-client/tests/e2e_sse_tests.rs

use playwright::sync::*;
use std::process::{Command, Child};
use std::thread;
use std::time::Duration;

struct TestApp {
    process: Child,
}

impl TestApp {
    fn start() -> Self {
        let process = Command::new("cargo")
            .args(&["tauri", "dev"])
            .env("ENVIRONMENT", "testing")
            .spawn()
            .expect("Failed to start app");
        
        thread::sleep(Duration::from_secs(5));
        Self { process }
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}

#[test]
fn test_sse_connection() {
    let _app = TestApp::start();
    
    let playwright = Playwright::new();
    let browser = playwright.chromium()
        .launch(Default::default())
        .unwrap();
    let page = browser.new_page(Default::default()).unwrap();
    
    // 导航到应用
    page.goto("http://localhost:5173", Default::default()).unwrap();
    
    // 等待 SSE 连接
    page.wait_for_selector(".sse-connected", Default::default()).unwrap();
    
    // 验证连接状态
    let status = page.text_content(".sse-status", Default::default()).unwrap();
    assert_eq!(status, Some("Connected".to_string()));
}

#[test]
fn test_sse_message_reception() {
    let _app = TestApp::start();
    
    let playwright = Playwright::new();
    let browser = playwright.chromium()
        .launch(Default::default())
        .unwrap();
    let page = browser.new_page(Default::default()).unwrap();
    
    page.goto("http://localhost:5173", Default::default()).unwrap();
    
    // 等待消息接收
    page.wait_for_selector(".message-item", Default::default()).unwrap();
    
    // 验证消息内容
    let message_count = page.query_selector_all(".message-item", Default::default())
        .unwrap()
        .len();
    assert!(message_count > 0);
}

#[test]
fn test_sse_reconnection() {
    let _app = TestApp::start();
    
    let playwright = Playwright::new();
    let browser = playwright.chromium()
        .launch(Default::default())
        .unwrap();
    let page = browser.new_page(Default::default()).unwrap();
    
    page.goto("http://localhost:5173", Default::default()).unwrap();
    
    // 等待初始连接
    page.wait_for_selector(".sse-connected", Default::default()).unwrap();
    
    // 模拟网络中断
    page.context().unwrap().set_offline(true).unwrap();
    
    // 等待重连状态
    page.wait_for_selector(".sse-reconnecting", Default::default()).unwrap();
    
    // 恢复网络
    page.context().unwrap().set_offline(false).unwrap();
    
    // 验证重连成功
    page.wait_for_selector(".sse-connected", Default::default()).unwrap();
}
```

### 对于 UI 测试

**推荐：Cypress**

```javascript
// desktop-client/cypress/e2e/chat.cy.js

describe('Chat UI Tests', () => {
  beforeEach(() => {
    cy.visit('http://localhost:5173');
    cy.get('.chat-tab').click();
  });

  it('should display chat interface', () => {
    cy.get('.chat-container').should('be.visible');
    cy.get('.message-input').should('be.visible');
    cy.get('.send-button').should('be.visible');
  });

  it('should send and receive messages', () => {
    cy.get('.message-input').type('Hello, World!');
    cy.get('.send-button').click();
    
    cy.get('.message-item').should('contain', 'Hello, World!');
  });

  it('should display SSE connection status', () => {
    cy.get('.sse-status').should('contain', 'Connected');
  });
});
```

### 对于集成测试

**推荐：Tauri 集成测试**

```rust
// desktop-client/tests/e2e_integration_tests.rs

#[tokio::test]
async fn test_full_workflow() {
    // 1. 初始化应用
    let init_info = invoke_command("get_app_init_info").await;
    assert!(!init_info.auth_token.is_empty());
    
    // 2. 获取配置
    let config = invoke_command("get_app_config").await;
    assert_eq!(config.api_base_url, "http://localhost:3000");
    
    // 3. 检查环境一致性
    let check_result = invoke_command("check_environment_consistency").await;
    assert!(check_result.all_passed);
    
    // 4. 创建线程
    let thread = invoke_command("create_thread").await;
    assert!(!thread.id.is_empty());
    
    // 5. 发送消息
    let message = invoke_command("send_message", {
        "thread_id": thread.id,
        "content": "Test message"
    }).await;
    assert!(!message.id.is_empty());
}
```

## 环境配置

### 测试环境变量

```bash
# .env.testing
ENVIRONMENT=testing
API_BASE_URL=http://localhost:3000
API_TIMEOUT_SECS=10
DATABASE_TYPE=sqlite
DATABASE_PATH=/tmp/test.db
LOG_LEVEL=debug
NETWORK_VERIFY_SSL=false
```

### 测试启动脚本

```bash
#!/bin/bash
# scripts/run-e2e-tests.sh

# 启动后端服务
cargo run -- run --cli-only --no-onboard &
BACKEND_PID=$!

# 等待后端启动
sleep 3

# 启动前端开发服务器
cd desktop-client/src-ui
npm run dev &
FRONTEND_PID=$!

# 等待前端启动
sleep 3

# 运行 E2E 测试
cd ../..
cargo test --test e2e_sse_tests -- --nocapture

# 清理
kill $BACKEND_PID
kill $FRONTEND_PID
```

## 最佳实践

### 1. 隔离测试环境

```rust
#[tokio::test]
async fn test_with_isolated_environment() {
    // 使用临时数据库
    let db_path = "/tmp/test_db_".to_string() + &uuid::Uuid::new_v4().to_string();
    
    std::env::set_var("DATABASE_PATH", &db_path);
    std::env::set_var("ENVIRONMENT", "testing");
    
    // 运行测试
    // ...
    
    // 清理
    let _ = std::fs::remove_file(&db_path);
}
```

### 2. 使用测试夹具

```rust
struct TestFixture {
    app: TestApp,
    browser: Browser,
    page: Page,
}

impl TestFixture {
    async fn new() -> Self {
        let app = TestApp::start();
        let playwright = Playwright::new();
        let browser = playwright.chromium()
            .launch(Default::default())
            .await
            .unwrap();
        let page = browser.new_page(Default::default()).await.unwrap();
        
        Self { app, browser, page }
    }
}

#[tokio::test]
async fn test_with_fixture() {
    let fixture = TestFixture::new().await;
    
    // 使用 fixture 进行测试
    fixture.page.goto("http://localhost:5173", Default::default()).await.unwrap();
}
```

### 3. 并行测试

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_parallel_sse_connections() {
    let handles: Vec<_> = (0..4)
        .map(|i| {
            tokio::spawn(async move {
                // 每个线程运行独立的测试
                test_sse_connection(i).await
            })
        })
        .collect();
    
    for handle in handles {
        handle.await.unwrap();
    }
}
```

## 总结

| 方案 | 优点 | 缺点 | 推荐用途 |
|------|------|------|---------|
| Playwright | 完整自动化 | 速度慢 | SSE/UI 测试 |
| Cypress | 简单易用 | 仅 Chromium | UI 测试 |
| WebDriver | 标准化 | 配置复杂 | 跨浏览器测试 |
| Tauri 集成 | 快速 | 功能有限 | 集成测试 |
| 浏览器模拟 | 快速 | 不真实 | 单元测试 |

**推荐组合**：
- SSE 测试：Playwright + Tauri 应用
- UI 测试：Cypress
- 集成测试：Tauri 集成测试
- 单元测试：浏览器模拟

## 相关文件

- `desktop-client/tests/e2e_sse_tests.rs` - SSE E2E 测试示例
- `desktop-client/cypress/e2e/` - Cypress 测试示例
- `scripts/run-e2e-tests.sh` - E2E 测试启动脚本
- `.env.testing` - 测试环境配置
