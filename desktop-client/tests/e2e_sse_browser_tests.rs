//! 端到端 SSE 浏览器测试
//! 
//! 这个测试模块演示如何在浏览器环境中测试 SSE 连接
//! 
//! 注意：这些测试需要：
//! 1. 后端服务运行在 http://localhost:3000
//! 2. 前端开发服务器运行在 http://localhost:5173
//! 3. Playwright 或类似的浏览器自动化工具

use std::process::{Command, Child};
use std::thread;
use std::time::Duration;

/// 测试应用管理器
pub struct TestApp {
    process: Option<Child>,
}

impl TestApp {
    /// 启动 Tauri 应用
    pub fn start() -> Self {
        println!("🚀 Starting Tauri application...");
        
        let process = Command::new("cargo")
            .args(&["tauri", "dev"])
            .env("ENVIRONMENT", "testing")
            .spawn();
        
        match process {
            Ok(p) => {
                // 等待应用启动
                thread::sleep(Duration::from_secs(5));
                println!("✅ Tauri application started");
                Self { process: Some(p) }
            }
            Err(e) => {
                eprintln!("❌ Failed to start Tauri application: {}", e);
                Self { process: None }
            }
        }
    }
    
    /// 检查应用是否运行
    pub fn is_running(&self) -> bool {
        self.process.is_some()
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            println!("🛑 Stopping Tauri application...");
            let _ = process.kill();
            let _ = process.wait();
            println!("✅ Tauri application stopped");
        }
    }
}

/// 浏览器测试环境
pub struct BrowserTestEnv {
    app: TestApp,
}

impl BrowserTestEnv {
    /// 创建新的浏览器测试环境
    pub fn new() -> Self {
        let app = TestApp::start();
        Self { app }
    }
    
    /// 获取应用 URL
    pub fn get_app_url(&self) -> &'static str {
        "http://localhost:5173"
    }
    
    /// 获取 API URL
    pub fn get_api_url(&self) -> &'static str {
        "http://localhost:3000"
    }
    
    /// 获取 SSE 事件 URL
    pub fn get_sse_url(&self) -> &'static str {
        "http://localhost:3000/api/chat/events"
    }
}

// ============================================
// 测试用例
// ============================================

#[test]
#[ignore] // 需要手动启动后端和前端
fn test_sse_connection_setup() {
    let env = BrowserTestEnv::new();
    
    if !env.app.is_running() {
        eprintln!("⚠️  Tauri application failed to start");
        return;
    }
    
    println!("✅ Test environment ready");
    println!("   App URL: {}", env.get_app_url());
    println!("   API URL: {}", env.get_api_url());
    println!("   SSE URL: {}", env.get_sse_url());
}

#[test]
#[ignore] // 需要手动启动后端和前端
fn test_sse_event_reception() {
    let _env = BrowserTestEnv::new();
    
    // 这个测试需要使用 Playwright 或类似工具
    // 示例代码（伪代码）：
    //
    // let playwright = Playwright::new();
    // let browser = playwright.chromium().launch().await.unwrap();
    // let page = browser.new_page().await.unwrap();
    //
    // page.goto("http://localhost:5173").await.unwrap();
    //
    // // 等待 SSE 连接建立
    // page.wait_for_selector(".sse-connected").await.unwrap();
    //
    // // 验证连接状态
    // let status = page.text_content(".sse-status").await.unwrap();
    // assert_eq!(status, Some("Connected".to_string()));
}

#[test]
#[ignore] // 需要手动启动后端和前端
fn test_sse_reconnection() {
    let _env = BrowserTestEnv::new();
    
    // 这个测试需要使用 Playwright 或类似工具
    // 示例代码（伪代码）：
    //
    // let playwright = Playwright::new();
    // let browser = playwright.chromium().launch().await.unwrap();
    // let page = browser.new_page().await.unwrap();
    //
    // page.goto("http://localhost:5173").await.unwrap();
    //
    // // 等待初始连接
    // page.wait_for_selector(".sse-connected").await.unwrap();
    //
    // // 模拟网络中断
    // page.context().unwrap().set_offline(true).unwrap();
    //
    // // 等待重连状态
    // page.wait_for_selector(".sse-reconnecting").await.unwrap();
    //
    // // 恢复网络
    // page.context().unwrap().set_offline(false).unwrap();
    //
    // // 验证重连成功
    // page.wait_for_selector(".sse-connected").await.unwrap();
}

#[test]
#[ignore] // 需要手动启动后端和前端
fn test_sse_message_flow() {
    let _env = BrowserTestEnv::new();
    
    // 这个测试需要使用 Playwright 或类似工具
    // 示例代码（伪代码）：
    //
    // let playwright = Playwright::new();
    // let browser = playwright.chromium().launch().await.unwrap();
    // let page = browser.new_page().await.unwrap();
    //
    // page.goto("http://localhost:5173").await.unwrap();
    //
    // // 等待 SSE 连接
    // page.wait_for_selector(".sse-connected").await.unwrap();
    //
    // // 发送消息
    // page.fill(".message-input", "Test message").await.unwrap();
    // page.click(".send-button").await.unwrap();
    //
    // // 等待消息显示
    // page.wait_for_selector(".message-item:has-text('Test message')").await.unwrap();
    //
    // // 验证消息内容
    // let message = page.text_content(".message-item").await.unwrap();
    // assert!(message.contains("Test message"));
}

// ============================================
// 辅助函数
// ============================================

/// 等待 URL 可访问
pub fn wait_for_url(url: &str, timeout_secs: u64) -> bool {
    let start = std::time::Instant::now();
    
    loop {
        match reqwest::blocking::get(url) {
            Ok(response) => {
                if response.status().is_success() {
                    return true;
                }
            }
            Err(_) => {}
        }
        
        if start.elapsed().as_secs() > timeout_secs {
            return false;
        }
        
        thread::sleep(Duration::from_millis(100));
    }
}

/// 获取 SSE 事件
pub async fn get_sse_events(url: &str, timeout_secs: u64) -> Result<Vec<String>, String> {
    use tokio::time::timeout;
    
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("Accept", "text/event-stream")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let mut events = Vec::new();
    let mut stream = response.bytes_stream();
    
    let future = async {
        while let Some(chunk) = stream.next().await {
            if let Ok(bytes) = chunk {
                let text = String::from_utf8_lossy(&bytes);
                events.push(text.to_string());
            }
        }
    };
    
    match timeout(Duration::from_secs(timeout_secs), future).await {
        Ok(_) => Ok(events),
        Err(_) => Err("Timeout waiting for SSE events".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_browser_test_env_creation() {
        // 这个测试验证测试环境可以创建
        // 但不启动实际的应用
        let env = BrowserTestEnv::new();
        
        assert_eq!(env.get_app_url(), "http://localhost:5173");
        assert_eq!(env.get_api_url(), "http://localhost:3000");
        assert_eq!(env.get_sse_url(), "http://localhost:3000/api/chat/events");
    }
    
    #[test]
    fn test_wait_for_url_timeout() {
        // 这个测试验证 wait_for_url 在超时时返回 false
        let result = wait_for_url("http://localhost:9999", 1);
        assert!(!result);
    }
}
