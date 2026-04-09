//! Qwen E2E 测试 - 诊断客户端和后端集成问题
//!
//! 这个测试验证：
//! 1. 后端是否正确配置了 Qwen
//! 2. 客户端是否能正确发送消息
//! 3. 后端是否能正确调用 Qwen API
//! 4. 客户端是否能正确接收响应

use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::sleep;

const BACKEND_URL: &str = "http://localhost:3000";
const AUTH_TOKEN: &str = "d397b61ad5584603d5691f03e73a0a3eea6fd66d1590ddc64a6b0292c7a2f270";

/// 测试结构体
struct QwenE2ETest {
    client: Client,
    backend_url: String,
    auth_token: String,
}

impl QwenE2ETest {
    /// 创建新的测试实例
    fn new() -> Self {
        Self {
            client: Client::new(),
            backend_url: BACKEND_URL.to_string(),
            auth_token: AUTH_TOKEN.to_string(),
        }
    }

    /// 检查后端是否运行
    async fn check_backend_health(&self) -> Result<(), String> {
        println!("🔍 检查后端健康状态...");

        let response = self
            .client
            .get(&format!("{}/api/health", self.backend_url))
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| format!("❌ 后端连接失败: {}", e))?;

        if response.status().is_success() {
            println!("✅ 后端运行正常");
            Ok(())
        } else {
            Err(format!("❌ 后端返回错误: {}", response.status()))
        }
    }

    /// 创建新的对话线程
    async fn create_thread(&self) -> Result<String, String> {
        println!("🔍 创建新的对话线程...");

        let response = self
            .client
            .post(&format!("{}/api/chat/thread/new", self.backend_url))
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .header("Content-Type", "application/json")
            .json(&json!({}))
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("❌ 创建线程失败: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(format!("❌ 创建线程返回错误 {}: {}", status, text));
        }

        let data: Value = response
            .json()
            .await
            .map_err(|e| format!("❌ 解析响应失败: {}", e))?;

        let thread_id = data
            .get("id")
            .or_else(|| data.get("thread_id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| "❌ 响应中没有 thread_id".to_string())?;

        println!("✅ 创建线程成功: {}", thread_id);
        Ok(thread_id.to_string())
    }

    /// 发送消息到后端
    async fn send_message(&self, thread_id: &str, content: &str) -> Result<Value, String> {
        println!("🔍 发送消息到后端...");
        println!("   线程 ID: {}", thread_id);
        println!("   消息内容: {}", content);

        let response = self
            .client
            .post(&format!("{}/api/chat/send", self.backend_url))
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .header("Content-Type", "application/json")
            .json(&json!({
                "content": content,
                "thread_id": thread_id,
            }))
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| format!("❌ 发送消息失败: {}", e))?;

        let status = response.status();
        println!("   响应状态: {}", status);

        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(format!("❌ 发送消息返回错误 {}: {}", status, text));
        }

        let data: Value = response
            .json()
            .await
            .map_err(|e| format!("❌ 解析响应失败: {}", e))?;

        println!("✅ 消息发送成功");
        println!(
            "   响应: {}",
            serde_json::to_string_pretty(&data).unwrap_or_default()
        );

        Ok(data)
    }

    /// 获取消息历史
    async fn get_messages(&self, thread_id: &str) -> Result<Vec<Value>, String> {
        println!("🔍 获取消息历史...");
        println!("   线程 ID: {}", thread_id);

        let response = self
            .client
            .get(&format!("{}/api/chat/history", self.backend_url))
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .query(&[("thread_id", thread_id)])
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("❌ 获取消息失败: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(format!("❌ 获取消息返回错误 {}: {}", status, text));
        }

        let data: Value = response
            .json()
            .await
            .map_err(|e| format!("❌ 解析响应失败: {}", e))?;

        let messages = if let Some(arr) = data.as_array() {
            arr.clone()
        } else if let Some(obj) = data.as_object() {
            if let Some(arr) = obj.get("messages").and_then(|v| v.as_array()) {
                arr.clone()
            } else if let Some(arr) = obj.get("turns").and_then(|v| v.as_array()) {
                arr.clone()
            } else {
                vec![data]
            }
        } else {
            vec![]
        };

        println!("✅ 获取消息成功: {} 条消息", messages.len());
        for (i, msg) in messages.iter().enumerate() {
            println!(
                "   消息 {}: {}",
                i + 1,
                serde_json::to_string_pretty(msg).unwrap_or_default()
            );
        }

        Ok(messages)
    }

    /// 等待消息响应
    async fn wait_for_response(
        &self,
        thread_id: &str,
        timeout_secs: u64,
    ) -> Result<String, String> {
        println!("🔍 等待 AI 响应 (超时: {} 秒)...", timeout_secs);

        let start = std::time::Instant::now();
        let mut last_count = 0;

        loop {
            sleep(Duration::from_secs(1)).await;

            let messages = self.get_messages(thread_id).await?;

            if messages.len() > last_count {
                // 检查是否有新的助手消息
                for msg in messages.iter().skip(last_count) {
                    if let Some(role) = msg.get("role").and_then(|v| v.as_str()) {
                        if role == "assistant" {
                            if let Some(content) = msg.get("content").and_then(|v| v.as_str()) {
                                println!("✅ 收到 AI 响应: {}", content);
                                return Ok(content.to_string());
                            }
                        }
                    }
                }
                last_count = messages.len();
            }

            if start.elapsed().as_secs() > timeout_secs {
                return Err(format!("❌ 等待响应超时 ({} 秒)", timeout_secs));
            }
        }
    }

    /// 运行完整的 E2E 测试
    async fn run_full_test(&self) -> Result<(), String> {
        println!("\n========================================");
        println!("🚀 开始 Qwen E2E 测试");
        println!("========================================\n");

        // 1. 检查后端健康状态
        self.check_backend_health().await?;
        println!();

        // 2. 创建新的对话线程
        let thread_id = self.create_thread().await?;
        println!();

        // 3. 发送测试消息
        let test_message = "你好，请用中文回复。这是一个测试消息。";
        self.send_message(&thread_id, test_message).await?;
        println!();

        // 4. 等待 AI 响应
        let response = self.wait_for_response(&thread_id, 30).await?;
        println!();

        // 5. 验证响应
        if response.is_empty() {
            return Err("❌ 收到空响应".to_string());
        }

        println!("========================================");
        println!("✅ Qwen E2E 测试通过!");
        println!("========================================\n");

        Ok(())
    }
}

// ============================================
// 测试用例
// ============================================

#[tokio::test]
#[ignore] // 需要手动启动后端
async fn test_qwen_backend_health() {
    let test = QwenE2ETest::new();

    match test.check_backend_health().await {
        Ok(_) => println!("✅ 后端健康检查通过"),
        Err(e) => {
            eprintln!("{}", e);
            panic!("后端健康检查失败");
        }
    }
}

#[tokio::test]
#[ignore] // 需要手动启动后端
async fn test_qwen_create_thread() {
    let test = QwenE2ETest::new();

    // 先检查后端
    if let Err(e) = test.check_backend_health().await {
        eprintln!("{}", e);
        panic!("后端不可用");
    }

    // 创建线程
    match test.create_thread().await {
        Ok(thread_id) => println!("✅ 创建线程成功: {}", thread_id),
        Err(e) => {
            eprintln!("{}", e);
            panic!("创建线程失败");
        }
    }
}

#[tokio::test]
#[ignore] // 需要手动启动后端
async fn test_qwen_send_message() {
    let test = QwenE2ETest::new();

    // 先检查后端
    if let Err(e) = test.check_backend_health().await {
        eprintln!("{}", e);
        panic!("后端不可用");
    }

    // 创建线程
    let thread_id = match test.create_thread().await {
        Ok(id) => id,
        Err(e) => {
            eprintln!("{}", e);
            panic!("创建线程失败");
        }
    };

    // 发送消息
    match test.send_message(&thread_id, "测试消息").await {
        Ok(response) => println!("✅ 发送消息成功: {}", response),
        Err(e) => {
            eprintln!("{}", e);
            panic!("发送消息失败");
        }
    }
}

#[tokio::test]
#[ignore] // 需要手动启动后端
async fn test_qwen_full_flow() {
    let test = QwenE2ETest::new();

    match test.run_full_test().await {
        Ok(_) => println!("✅ 完整流程测试通过"),
        Err(e) => {
            eprintln!("{}", e);
            panic!("完整流程测试失败");
        }
    }
}

// ============================================
// 诊断工具
// ============================================

/// 诊断 Qwen 配置
#[tokio::test]
#[ignore]
async fn diagnose_qwen_setup() {
    println!("\n========================================");
    println!("🔍 诊断 Qwen 配置");
    println!("========================================\n");

    let test = QwenE2ETest::new();

    // 1. 检查后端
    println!("1️⃣  检查后端连接...");
    match test.check_backend_health().await {
        Ok(_) => println!("   ✅ 后端可访问\n"),
        Err(e) => {
            println!("   {}\n", e);
            println!("   💡 解决方案:");
            println!("      - 确保后端服务已启动");
            println!("      - 运行: cargo run -- run --cli-only --no-onboard\n");
            return;
        }
    }

    // 2. 检查线程创建
    println!("2️⃣  检查线程创建...");
    let thread_id = match test.create_thread().await {
        Ok(id) => {
            println!("   ✅ 线程创建成功\n");
            id
        }
        Err(e) => {
            println!("   {}\n", e);
            println!("   💡 解决方案:");
            println!("      - 检查认证令牌是否正确");
            println!("      - 检查后端日志\n");
            return;
        }
    };

    // 3. 检查消息发送
    println!("3️⃣  检查消息发送...");
    match test.send_message(&thread_id, "测试").await {
        Ok(_) => println!("   ✅ 消息发送成功\n"),
        Err(e) => {
            println!("   {}\n", e);
            println!("   💡 解决方案:");
            println!("      - 检查后端日志");
            println!("      - 检查 Qwen API 配置\n");
            return;
        }
    }

    // 4. 检查响应
    println!("4️⃣  检查 AI 响应 (等待 30 秒)...");
    match test.wait_for_response(&thread_id, 30).await {
        Ok(response) => {
            println!("   ✅ 收到 AI 响应");
            println!("   响应内容: {}\n", response);
        }
        Err(e) => {
            println!("   {}\n", e);
            println!("   💡 可能的原因:");
            println!("      1. Qwen API 密钥未配置或无效");
            println!("      2. Qwen API 调用失败");
            println!("      3. 网络连接问题\n");
            println!("   💡 解决方案:");
            println!("      - 检查环境变量:");
            println!("        export LLM_BACKEND=\"openai_compatible\"");
            println!(
                "        export LLM_BASE_URL=\"https://dashscope.aliyuncs.com/compatible-mode/v1\""
            );
            println!("        export LLM_API_KEY=\"sk-...\"");
            println!("        export LLM_MODEL=\"qwen-max\"");
            println!("      - 重启后端服务");
            println!("      - 查看后端日志\n");
            return;
        }
    }

    println!("========================================");
    println!("✅ 诊断完成 - 所有检查通过!");
    println!("========================================\n");
}
