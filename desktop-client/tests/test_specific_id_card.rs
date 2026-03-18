#[cfg(test)]
mod test_specific_id_card {
    use desktop_client::dlp::{DlpIntegration, DlpIntegrationConfig};

    #[tokio::test]
    async fn test_id_card_330326199408015618() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        let content = "我的身份证号是 330326199408015618";
        
        let result = integration.scan_user_input(content).await.unwrap();
        
        println!("扫描结果:");
        println!("  had_sensitive_data: {}", result.had_sensitive_data);
        println!("  was_blocked: {}", result.was_blocked);
        println!("  sanitized_content: {}", result.sanitized_content);
        println!("  total_matches: {}", result.sanitization_stats.total_matches);
        println!("  redacted_count: {}", result.sanitization_stats.redacted_count);
        
        assert!(result.had_sensitive_data, "应该检测到敏感数据");
        assert!(!result.was_blocked, "不应该被阻止");
        assert!(result.sanitized_content.contains("330************618"), 
                "应该脱敏为 330************618，实际: {}", result.sanitized_content);
    }

    #[tokio::test]
    async fn test_id_card_110101199003071234() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        let content = "我的身份证号是 110101199003071234";
        
        let result = integration.scan_user_input(content).await.unwrap();
        
        println!("扫描结果:");
        println!("  had_sensitive_data: {}", result.had_sensitive_data);
        println!("  sanitized_content: {}", result.sanitized_content);
        
        assert!(result.had_sensitive_data);
        assert!(result.sanitized_content.contains("110************234"));
    }
}
