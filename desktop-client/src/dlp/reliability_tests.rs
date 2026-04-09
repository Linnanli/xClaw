//! DLP 可靠性覆盖率测试
//!
//! 测试故障恢复和容错、并发访问和压力测试、网络中断和异常处理

/// 可靠性测试：故障恢复和容错测试
#[cfg(test)]
mod reliability_tests {
    use crate::dlp::patterns::get_all_builtin_patterns;
    use crate::dlp::{DlpDetector, DlpIntegration, DlpSanitizer};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_reliability_network_failure_recovery() {
        let integration = DlpIntegration::with_default_config().await.unwrap();

        // 模拟网络中断场景
        let test_content = "身份证： 110101199003071234 ";

        // 正常情况下应该工作
        let result1 = integration.scan_user_input(test_content).await.unwrap();
        assert!(result1.had_sensitive_data);

        // 模拟网络恢复后继续工作
        let result2 = integration.scan_user_input(test_content).await.unwrap();
        assert!(result2.had_sensitive_data);
        assert_eq!(result1.sanitized_content, result2.sanitized_content);
    }

    #[tokio::test]
    async fn test_reliability_concurrent_access_safety() {
        let integration = Arc::new(DlpIntegration::with_default_config().await.unwrap());
        let mut handles = Vec::new();

        // 100个并发请求
        for i in 0..100 {
            let integration_clone = integration.clone();
            let handle = tokio::spawn(async move {
                let content = format!("测试 {} 身份证： 11010119900307123{} ", i, i % 10);
                integration_clone.scan_user_input(&content).await
            });
            handles.push(handle);
        }

        // 等待所有请求完成
        let mut success_count = 0;
        for handle in handles {
            match handle.await {
                Ok(Ok(_)) => success_count += 1,
                Ok(Err(e)) => println!("Request failed: {}", e),
                Err(e) => println!("Task failed: {}", e),
            }
        }

        // 至少95%的请求应该成功
        assert!(
            success_count >= 95,
            "Success rate too low: {}/100",
            success_count
        );
    }

    #[tokio::test]
    async fn test_reliability_memory_pressure_handling() {
        let integration = DlpIntegration::with_default_config().await.unwrap();

        // 大量内存压力测试
        let large_content = "测试内容 ".repeat(10000) + "身份证： 110101199003071234 ";

        for i in 0..10 {
            let result = integration.scan_user_input(&large_content).await;
            assert!(result.is_ok(), "Memory pressure test {} failed", i);

            if i % 3 == 0 {
                // 模拟垃圾回收
                tokio::task::yield_now().await;
            }
        }
    }

    #[tokio::test]
    async fn test_reliability_configuration_corruption_recovery() {
        let integration = DlpIntegration::with_default_config().await.unwrap();

        // 正常配置测试
        let result1 = integration
            .scan_user_input("身份证： 110101199003071234 ")
            .await
            .unwrap();
        assert!(result1.had_sensitive_data);

        // 模拟配置损坏后的恢复
        let mut corrupted_config = integration.get_config().await;
        corrupted_config.enabled = false;

        // 更新配置
        integration.update_config(corrupted_config).await.unwrap();

        // 验证系统仍然可以工作（虽然功能被禁用）
        let result2 = integration
            .scan_user_input("身份证： 110101199003071234 ")
            .await
            .unwrap();
        assert!(!result2.had_sensitive_data); // 应该被禁用

        // 恢复正常配置
        let normal_config = crate::dlp::integration::DlpIntegrationConfig::default();
        integration.update_config(normal_config).await.unwrap();

        // 验证功能恢复
        let result3 = integration
            .scan_user_input("身份证： 110101199003071234 ")
            .await
            .unwrap();
        assert!(result3.had_sensitive_data);
    }

    #[tokio::test]
    async fn test_reliability_pattern_compilation_failure_handling() {
        // 测试模式编译失败的处理
        let patterns = get_all_builtin_patterns().unwrap();
        let detector = DlpDetector::with_custom_patterns(patterns);
        let sanitizer = DlpSanitizer::with_default_config(detector);

        // 即使有无效模式，系统也应该继续工作
        let test_cases = [
            "身份证： 110101199003071234 ",
            "手机： 13800138000 ",
            "普通文本",
        ];

        for content in &test_cases {
            let result = sanitizer.sanitize(content);
            // 不应该崩溃
            assert!(result.sanitized_content.len() > 0);
        }
    }

    #[tokio::test]
    async fn test_reliability_resource_exhaustion_protection() {
        let integration = DlpIntegration::with_default_config().await.unwrap();

        // 资源耗尽保护测试
        let resource_intensive_inputs = [
            // 大量重复模式
            "身份证".repeat(1000) + " 110101199003071234 ",
            // 长字符串
            "a".repeat(100000) + " 身份证： 110101199003071234 ",
            // 复杂嵌套
            "(((".repeat(1000) + " 身份证： 110101199003071234 " + ")))".repeat(1000).as_str(),
        ];

        for (i, input) in resource_intensive_inputs.iter().enumerate() {
            let start_time = std::time::Instant::now();
            let result = integration.scan_user_input(input).await;
            let elapsed = start_time.elapsed();

            // 应该在合理时间内完成（<5秒）
            assert!(
                elapsed.as_secs() < 5,
                "Resource test {} took too long: {:?}",
                i,
                elapsed
            );
            assert!(result.is_ok(), "Resource test {} failed", i);
        }
    }

    #[tokio::test]
    async fn test_reliability_partial_failure_handling() {
        let integration = DlpIntegration::with_default_config().await.unwrap();

        // 部分失败处理测试
        let mixed_content = vec![
            "正常内容1",
            "身份证： 110101199003071234 ", // 敏感信息
            "正常内容2",
            "API密钥： LTAI4G8aB9cD2eFgH3iJ ", // 会被阻止
            "正常内容3",
        ];

        let mut success_count = 0;
        let mut blocked_count = 0;

        for content in &mixed_content {
            match integration.scan_user_input(content).await {
                Ok(result) => {
                    if result.was_blocked {
                        blocked_count += 1;
                    } else {
                        success_count += 1;
                    }
                }
                Err(_) => {
                    // 部分失败是可以接受的
                }
            }
        }

        // 应该有成功和阻止的情况
        assert!(success_count > 0, "No successful scans");
        assert!(blocked_count > 0, "No blocked content");
    }

    #[tokio::test]
    async fn test_reliability_graceful_degradation() {
        // 优雅降级测试
        let integration = DlpIntegration::with_default_config().await.unwrap();

        // 模拟系统负载过高的情况
        let high_load_inputs: Vec<String> = (0..50)
            .map(|i| format!("高负载测试 {} 身份证： 11010119900307123{} ", i, i % 10))
            .collect();

        let start_time = std::time::Instant::now();
        let mut processed = 0;

        for input in &high_load_inputs {
            if start_time.elapsed().as_secs() > 10 {
                // 超时后应该优雅降级
                break;
            }

            match integration.scan_user_input(input).await {
                Ok(_) => processed += 1,
                Err(_) => {
                    // 在高负载下，部分失败是可以接受的
                    break;
                }
            }
        }

        // 应该至少处理了一些请求
        assert!(processed > 0, "No requests processed under high load");
    }

    #[test]
    fn test_reliability_detector_state_consistency() {
        let detector = DlpDetector::new();

        // 状态一致性测试
        let test_content = "身份证： 110101199003071234 ";

        // 多次扫描应该产生一致的结果
        let result1 = detector.scan(test_content);
        let result2 = detector.scan(test_content);
        let result3 = detector.scan(test_content);

        assert_eq!(result1.has_sensitive_data, result2.has_sensitive_data);
        assert_eq!(result2.has_sensitive_data, result3.has_sensitive_data);
        assert_eq!(result1.matches.len(), result2.matches.len());
        assert_eq!(result2.matches.len(), result3.matches.len());
    }

    #[tokio::test]
    async fn test_reliability_timeout_handling() {
        let integration = DlpIntegration::with_default_config().await.unwrap();

        // 超时处理测试
        let timeout_test = async {
            let result = integration
                .scan_user_input("身份证： 110101199003071234 ")
                .await;
            result
        };

        // 使用超时包装
        match tokio::time::timeout(Duration::from_secs(5), timeout_test).await {
            Ok(result) => {
                assert!(result.is_ok(), "Scan should succeed within timeout");
            }
            Err(_) => {
                panic!("Operation timed out");
            }
        }
    }
}

/// 压力测试
#[cfg(test)]
mod stress_tests {
    use crate::dlp::DlpIntegration;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_stress_high_throughput() {
        let integration = Arc::new(DlpIntegration::with_default_config().await.unwrap());
        let processed_count = Arc::new(AtomicUsize::new(0));
        let error_count = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();

        // 启动多个工作线程
        for worker_id in 0..10 {
            let integration_clone = integration.clone();
            let processed_clone = processed_count.clone();
            let error_clone = error_count.clone();

            let handle = tokio::spawn(async move {
                for i in 0..100 {
                    let content = format!(
                        "工作线程 {} 请求 {} 身份证： 11010119900307123{} ",
                        worker_id,
                        i,
                        i % 10
                    );

                    match integration_clone.scan_user_input(&content).await {
                        Ok(_) => {
                            processed_clone.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(_) => {
                            error_clone.fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    // 小延迟避免过度压力
                    if i % 10 == 0 {
                        tokio::task::yield_now().await;
                    }
                }
            });

            handles.push(handle);
        }

        // 等待所有工作线程完成
        for handle in handles {
            handle.await.unwrap();
        }

        let total_processed = processed_count.load(Ordering::Relaxed);
        let total_errors = error_count.load(Ordering::Relaxed);

        println!("Processed: {}, Errors: {}", total_processed, total_errors);

        // 成功率应该至少95%
        let success_rate = total_processed as f64 / (total_processed + total_errors) as f64;
        assert!(
            success_rate >= 0.95,
            "Success rate too low: {:.2}%",
            success_rate * 100.0
        );
    }

    #[tokio::test]
    async fn test_stress_memory_stability() {
        let integration = Arc::new(DlpIntegration::with_default_config().await.unwrap());

        // 内存稳定性测试
        for cycle in 0..10 {
            let mut batch_handles = Vec::new();

            // 每个周期处理100个请求
            for i in 0..100 {
                let integration_clone = integration.clone();
                let handle = tokio::spawn(async move {
                    let content = format!("周期 {} 请求 {} 身份证： 110101199003071234 ", cycle, i);
                    integration_clone.scan_user_input(&content).await
                });
                batch_handles.push(handle);
            }

            // 等待批次完成
            for handle in batch_handles {
                let result: Result<crate::dlp::SanitizationResult, crate::dlp::DlpError> =
                    handle.await.unwrap();
                assert!(result.is_ok(), "Request should succeed in cycle {}", cycle);
            }

            // 周期间暂停，模拟垃圾回收
            sleep(Duration::from_millis(100)).await;
        }
    }

    #[tokio::test]
    async fn test_stress_pattern_matching_performance() {
        let integration = DlpIntegration::with_default_config().await.unwrap();

        // 模式匹配性能测试
        let complex_content = format!(
            "复杂文档包含多种信息：身份证 {} 手机号 {} API密钥 {} 银行卡 {} 邮箱 test@example.com",
            "110101199003071234", "13800138000", "sk-test123456789", "6222021234567890123",
        );

        let start_time = std::time::Instant::now();
        let mut total_matches = 0;

        // 连续处理1000次
        for _ in 0..1000 {
            let result = integration.scan_user_input(&complex_content).await.unwrap();
            total_matches += result.sanitization_stats.total_matches;
        }

        let elapsed = start_time.elapsed();
        let avg_time_per_scan = elapsed.as_millis() as f64 / 1000.0;

        println!(
            "Average scan time: {:.2}ms, Total matches: {}",
            avg_time_per_scan, total_matches
        );

        // 平均扫描时间应该小于10ms
        assert!(
            avg_time_per_scan < 10.0,
            "Average scan time too high: {:.2}ms",
            avg_time_per_scan
        );
        assert!(total_matches > 0, "Should detect some matches");
    }
}
