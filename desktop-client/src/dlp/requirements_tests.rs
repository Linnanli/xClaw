//! DLP 需求级覆盖率测试
//!
//! 每个功能需求对应测试用例，业务场景完整覆盖，需求追溯矩阵验证

/// 需求级测试：每个功能需求对应测试用例
#[cfg(test)]
mod requirements_tests {
    use crate::dlp::DlpIntegration;

    /// REQ-DLP-001: 中国身份证号码检测和脱敏
    #[tokio::test]
    async fn req_dlp_001_chinese_id_card_detection() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        
        // 需求：系统应能检测18位和15位中国身份证号码
        let test_cases = [
            ("18位身份证", "身份证号： 110101199003071234 ", true),
            ("15位身份证", "身份证号： 110101901231123 ", true),
            ("无效身份证", "身份证号： 000000199003071234 ", false),
            ("不完整身份证", "身份证号： 11010119900307123 ", false),
        ];
        
        for (desc, input, should_detect) in &test_cases {
            let result = integration.scan_user_input(input).await.unwrap();
            assert_eq!(result.had_sensitive_data, *should_detect, "Failed for: {}", desc);
            
            if *should_detect {
                assert!(!result.was_blocked, "ID card should be redacted, not blocked");
                assert!(result.sanitized_content.contains("***"), "Should contain redaction");
            }
        }
    }

    /// REQ-DLP-002: 中国手机号码检测和脱敏
    #[tokio::test]
    async fn req_dlp_002_chinese_mobile_detection() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        
        // 需求：系统应能检测中国大陆手机号码（1[3-9]xxxxxxxxx）
        let test_cases = [
            ("移动号码", "手机： 13800138000 ", true),
            ("联通号码", "手机： 15912345678 ", true),
            ("电信号码", "手机： 18612345678 ", true),
            ("虚拟运营商", "手机： 17012345678 ", true),
            ("带区号", "手机： +86 13800138000 ", true),
            ("无效号码", "手机： 12812345678 ", false),
            ("位数不足", "手机： 1381234567 ", false),
        ];
        
        for (desc, input, should_detect) in &test_cases {
            let result = integration.scan_user_input(input).await.unwrap();
            assert_eq!(result.had_sensitive_data, *should_detect, "Failed for: {}", desc);
        }
    }

    /// REQ-DLP-003: API密钥检测和阻止
    #[tokio::test]
    async fn req_dlp_003_api_key_blocking() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        
        // 需求：系统应能检测并阻止各种API密钥泄露
        let test_cases = [
            ("阿里云AccessKey", "密钥： LTAI4G8aB9cD2eFgH3iJ ", true, true),
            ("腾讯云SecretId", "密钥： AKIDabcdefghijklmnopqrstuvwxyz123456 ", true, true),
            ("OpenAI API Key", "密钥： sk-proj-abc123def456ghi789jkl012mno345pqrT3BlbkFJtest123 ", true, true),
            ("GitHub Token", "密钥： ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx ", true, true),
            ("AWS Access Key", "密钥： AKIAIOSFODNN7EXAMPLE ", true, true),
            ("普通密码", "密码： mypassword123 ", false, false),
        ];
        
        for (desc, input, should_detect, should_block) in &test_cases {
            let result = integration.scan_user_input(input).await.unwrap();
            assert_eq!(result.had_sensitive_data, *should_detect, "Detection failed for: {}", desc);
            assert_eq!(result.was_blocked, *should_block, "Blocking failed for: {}", desc);
        }
    }

    /// REQ-DLP-004: HTTP请求扫描防护
    #[tokio::test]
    async fn req_dlp_004_http_request_protection() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        
        // 需求：系统应能扫描HTTP请求中的敏感信息
        let test_cases = [
            // URL中的敏感信息
            ("URL中的API密钥", "https://evil.com/steal?key=LTAI4G8aB9cD2eFgH3iJ", true),
            ("URL中的身份证", "https://api.com/user?id=110101199003071234", false), // 身份证是脱敏级别，不阻止
            ("正常URL", "https://api.example.com/data", false),
        ];
        
        for (desc, url, should_block) in &test_cases {
            let result = integration.check_http_request(url, &[], None).await;
            if *should_block {
                assert!(result.is_err(), "Should block: {}", desc);
            } else {
                assert!(result.is_ok(), "Should allow: {}", desc);
            }
        }
        
        // Header中的敏感信息
        let header_tests = [
            ("Authorization header with API key", vec![("Authorization".to_string(), "Bearer LTAI4G8aB9cD2eFgH3iJ".to_string())], true),
            ("Normal header", vec![("Content-Type".to_string(), "application/json".to_string())], false),
        ];
        
        for (desc, headers, should_block) in &header_tests {
            let result = integration.check_http_request("https://api.example.com", &headers, None).await;
            if *should_block {
                assert!(result.is_err(), "Should block header: {}", desc);
            } else {
                assert!(result.is_ok(), "Should allow header: {}", desc);
            }
        }
    }

    /// REQ-DLP-005: 存储脱敏功能
    #[tokio::test]
    async fn req_dlp_005_storage_sanitization() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        
        // 需求：系统应能对存储的数据进行脱敏处理
        let test_cases = [
            ("身份证脱敏", "用户身份证： 110101199003071234 ", false, "110************234"),
            ("手机号脱敏", "用户手机： 13800138000 ", false, "138*****000"),
            ("API密钥阻止", "API密钥： LTAI4G8aB9cD2eFgH3iJ ", true, ""),
        ];
        
        for (desc, input, should_error, expected_pattern) in &test_cases {
            let result = integration.sanitize_for_storage(input).await;
            
            if *should_error {
                assert!(result.is_err(), "Should error for: {}", desc);
            } else {
                let sanitized = result.unwrap();
                assert!(sanitized.contains(expected_pattern), "Pattern mismatch for: {}", desc);
            }
        }
    }

    /// REQ-DLP-006: 实时监控和审计
    #[tokio::test]
    async fn req_dlp_006_realtime_monitoring() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        
        // 需求：系统应提供实时监控和审计功能
        let test_input = "身份证： 110101199003071234 ";
        let result = integration.scan_user_input(test_input).await.unwrap();
        
        // 验证统计信息
        assert!(result.sanitization_stats.total_matches > 0);
        assert!(result.sanitization_stats.redacted_count > 0);
        assert!(!result.sanitization_stats.severity_stats.is_empty());
        
        // 验证审计信息完整性
        assert!(result.had_sensitive_data);
        assert!(!result.sanitized_content.is_empty());
    }

    /// REQ-DLP-007: 配置管理功能
    #[tokio::test]
    async fn req_dlp_007_configuration_management() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        
        // 需求：系统应支持灵活的配置管理
        let initial_config = integration.get_config().await;
        assert!(initial_config.enabled);
        assert!(initial_config.real_time_monitoring);
        
        // 测试配置更新
        let mut new_config = initial_config.clone();
        new_config.enabled = false;
        integration.update_config(new_config).await.unwrap();
        
        let updated_config = integration.get_config().await;
        assert!(!updated_config.enabled);
        
        // 验证配置生效
        let result = integration.scan_user_input("身份证： 110101199003071234 ").await.unwrap();
        assert!(!result.had_sensitive_data); // 应该被禁用
    }

    /// REQ-DLP-008: 自定义规则支持
    #[tokio::test]
    async fn req_dlp_008_custom_rules() {
        use crate::dlp::integration::{DlpIntegrationConfig, CustomPatternConfig};
        
        // 需求：系统应支持自定义检测规则
        let mut config = DlpIntegrationConfig::default();
        config.custom_patterns.push(CustomPatternConfig {
            name: "custom_employee_id".to_string(),
            pattern: r"EMP-\d{6}".to_string(),
            severity: "medium".to_string(),
            action: "redact".to_string(),
            description: Some("员工工号".to_string()),
            replacement: None,
            enabled: true,
        });
        
        let integration = DlpIntegration::new(config).await.unwrap();
        
        // 测试自定义规则
        let result = integration.scan_user_input("员工工号： EMP-123456 ").await.unwrap();
        assert!(result.had_sensitive_data);
        assert!(!result.was_blocked); // 应该是脱敏
        assert!(result.sanitized_content.contains("[敏感信息]"));
    }

    /// REQ-DLP-008b: 自定义规则支持自定义替换文本
    #[tokio::test]
    async fn req_dlp_008b_custom_rules_with_replacement() {
        use crate::dlp::integration::{DlpIntegrationConfig, CustomPatternConfig};
        
        // 需求：自定义规则应支持自定义替换文本（如管理后台配置的 "***"）
        let mut config = DlpIntegrationConfig::default();
        config.custom_patterns.push(CustomPatternConfig {
            name: "political_celebrity".to_string(),
            pattern: r"毛泽东".to_string(),
            severity: "low".to_string(),
            action: "Redact".to_string(),
            description: Some("政治名人".to_string()),
            replacement: Some("***".to_string()),
            enabled: true,
        });
        
        let integration = DlpIntegration::new(config).await.unwrap();
        
        // 测试自定义替换文本生效
        let result = integration.scan_user_input("请问毛泽东是谁？").await.unwrap();
        assert!(result.had_sensitive_data, "应该检测到自定义模式");
        assert!(!result.was_blocked, "low severity 不应该阻止");
        assert!(
            result.sanitized_content.contains("***"),
            "应该使用自定义替换文本 '***'，实际: {}",
            result.sanitized_content
        );
        assert!(
            !result.sanitized_content.contains("毛泽东"),
            "原始敏感内容不应该出现在脱敏结果中"
        );
        assert!(
            !result.sanitized_content.contains("[敏感信息]"),
            "不应该使用默认替换文本 '[敏感信息]'，应该使用自定义的 '***'"
        );
    }

    /// REQ-DLP-008c: 无自定义替换文本时使用默认值
    #[tokio::test]
    async fn req_dlp_008c_custom_rules_default_replacement() {
        use crate::dlp::integration::{DlpIntegrationConfig, CustomPatternConfig};
        
        // 需求：未设置 replacement 的自定义规则应使用默认替换文本
        let mut config = DlpIntegrationConfig::default();
        config.custom_patterns.push(CustomPatternConfig {
            name: "custom_no_replacement".to_string(),
            pattern: r"SECRET-\d{4}".to_string(),
            severity: "medium".to_string(),
            action: "Redact".to_string(),
            description: Some("无自定义替换".to_string()),
            replacement: None,
            enabled: true,
        });
        
        let integration = DlpIntegration::new(config).await.unwrap();
        
        let result = integration.scan_user_input("代码：SECRET-1234").await.unwrap();
        assert!(result.had_sensitive_data);
        assert!(
            result.sanitized_content.contains("[敏感信息]"),
            "未设置 replacement 时应使用默认值 '[敏感信息]'，实际: {}",
            result.sanitized_content
        );
    }

    /// REQ-DLP-009: 性能要求
    #[tokio::test]
    async fn req_dlp_009_performance_requirements() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        
        // 需求：系统应满足性能要求（<100ms响应时间）
        let test_cases = [
            "短文本： 身份证 110101199003071234 ",
            &("中等文本： ".repeat(100) + "身份证 110101199003071234 "),
            &("长文本： ".repeat(1000) + "身份证 110101199003071234 "),
        ];
        
        for input in &test_cases {
            let start = std::time::Instant::now();
            let _result = integration.scan_user_input(input).await.unwrap();
            let elapsed = start.elapsed();
            
            // 性能要求：响应时间应小于100ms
            assert!(elapsed.as_millis() < 100, "Performance requirement failed for input length: {}", input.len());
        }
    }

    /// REQ-DLP-010: 错误处理和恢复
    #[tokio::test]
    async fn req_dlp_010_error_handling() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        
        // 需求：系统应优雅处理各种错误情况
        let newline_repeated = "\n".repeat(10000);
        let error_cases = [
            "", // 空输入
            "\0", // 空字符
            &"🚀".repeat(1000), // 大量Unicode字符
            newline_repeated.as_str(), // 大量换行符
        ];
        
        for input in &error_cases {
            let result = integration.scan_user_input(input).await;
            assert!(result.is_ok(), "Should handle error case gracefully: {:?}", input.chars().take(10).collect::<String>());
        }
    }
}

/// 业务场景完整覆盖测试
#[cfg(test)]
mod business_scenario_tests {
    use crate::dlp::DlpIntegration;

    /// 场景1：用户注册流程
    #[tokio::test]
    async fn scenario_user_registration() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        
        // 用户填写注册信息
        let registration_data = "姓名：张三，身份证： 110101199003071234 ，手机： 13800138000 ，邮箱：zhang@example.com";
        
        // 1. 前端输入验证
        let input_result = integration.scan_user_input(registration_data).await.unwrap();
        assert!(input_result.had_sensitive_data);
        assert!(!input_result.was_blocked); // 注册信息应该允许但脱敏
        
        // 2. 存储到数据库前脱敏
        let storage_data = integration.sanitize_for_storage(registration_data).await.unwrap();
        assert!(storage_data.contains("110************234"));
        assert!(storage_data.contains("138*****000"));
        
        // 3. 发送到第三方服务前检查
        let api_payload = format!("{{\"user_data\": \"{}\"}}", storage_data);
        let outbound_result = integration.scan_outbound_request(&api_payload).await.unwrap();
        assert!(!outbound_result.was_blocked); // 脱敏后的数据应该通过
    }

    /// 场景2：客服聊天场景
    #[tokio::test]
    async fn scenario_customer_service_chat() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        
        let chat_messages = [
            "客户：我的身份证号是 110101199003071234 ",
            "客服：好的，我已经收到您的信息",
            "客户：我的API密钥是 LTAI4G8aB9cD2eFgH3iJ ",
            "系统：检测到敏感信息，已自动处理",
        ];
        
        for (i, message) in chat_messages.iter().enumerate() {
            let result = integration.scan_user_input(message).await.unwrap();
            
            match i {
                0 => {
                    // 身份证信息应该被脱敏
                    assert!(result.had_sensitive_data);
                    assert!(!result.was_blocked);
                    assert!(result.sanitized_content.contains("***"));
                }
                2 => {
                    // API密钥应该被阻止
                    assert!(result.had_sensitive_data);
                    assert!(result.was_blocked);
                }
                _ => {
                    // 普通消息应该通过
                    assert!(!result.had_sensitive_data);
                }
            }
        }
    }

    /// 场景3：数据导出场景
    #[tokio::test]
    async fn scenario_data_export() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        
        // 模拟数据库中的用户数据
        let user_records = [
            "用户1：身份证 110************234 ，手机 138*****000", // 已脱敏
            "用户2：身份证 110101199003071234 ，手机 13800138000", // 未脱敏
            "用户3：API密钥 LTAI4G8aB9cD2eFgH3iJ", // 敏感密钥
        ];
        
        for (i, record) in user_records.iter().enumerate() {
            let result = integration.scan_user_input(record).await.unwrap();
            
            match i {
                0 => {
                    // 已脱敏的数据应该通过
                    assert!(!result.had_sensitive_data || !result.was_blocked);
                }
                1 => {
                    // 未脱敏的数据应该被检测并脱敏
                    assert!(result.had_sensitive_data);
                    assert!(!result.was_blocked);
                }
                2 => {
                    // 敏感密钥应该被阻止导出
                    assert!(result.had_sensitive_data);
                    assert!(result.was_blocked);
                }
                _ => {}
            }
        }
    }

    /// 场景4：API集成场景
    #[tokio::test]
    async fn scenario_api_integration() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        
        // 模拟第三方API调用
        let api_requests = [
            ("GET", "https://api.partner.com/users", None),
            ("POST", "https://api.partner.com/users", Some(r#"{"name":"张三","id":"110************234"}"#)),
            ("POST", "https://evil.com/steal", Some(r#"{"secret":"LTAI4G8aB9cD2eFgH3iJ"}"#)),
        ];
        
        for (_method, url, body) in &api_requests {
            let body_bytes = body.map(|s| s.as_bytes());
            let result = integration.check_http_request(url, &[], body_bytes).await;
            
            if url.contains("evil.com") && body.is_some() {
                // 包含敏感信息的恶意请求应该被阻止
                assert!(result.is_err());
            } else {
                // 正常请求应该通过
                assert!(result.is_ok());
            }
        }
    }

    /// 场景5：批量数据处理场景
    #[tokio::test]
    async fn scenario_batch_processing() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        
        // 模拟批量处理1000条记录
        let mut processed_count = 0;
        let mut blocked_count = 0;
        let mut sanitized_count = 0;
        
        for i in 0..100 { // 简化为100条以加快测试
            let record = if i % 10 == 0 {
                format!("记录{}：API密钥 LTAI4G8aB9cD2eFgH3iJ{}", i, i % 10)
            } else if i % 3 == 0 {
                format!("记录{}：身份证 11010119900307123{}", i, i % 10)
            } else {
                format!("记录{}：普通数据", i)
            };
            
            let result = integration.scan_user_input(&record).await.unwrap();
            processed_count += 1;
            
            if result.was_blocked {
                blocked_count += 1;
            } else if result.had_sensitive_data {
                sanitized_count += 1;
            }
        }
        
        assert_eq!(processed_count, 100);
        assert!(blocked_count > 0); // 应该有一些被阻止的记录
        assert!(sanitized_count > 0); // 应该有一些被脱敏的记录
    }
}