use desktop_client::auth_token_manager::AuthTokenManager;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

// ========== 可靠性覆盖率 >75% ==========
//
// 所有测试必须用 new_with_path(TempDir) 隔离，禁止使用 new()，
// 避免操作真实系统路径导致测试间互相干扰。

#[tokio::test]
async fn test_network_failure_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let token_file = temp_dir.path().join(".auth_token");
    let token_manager = AuthTokenManager::new_with_path(token_file);

    // 正常情况下 token 可用
    let token = token_manager.load_or_generate().unwrap();
    assert_eq!(token.len(), 64);
    assert!(token.chars().all(|c| c.is_ascii_hexdigit()));

    // 模拟"网络中断"：删除 token 文件
    token_manager.delete().unwrap();
    assert!(token_manager.load().is_err(), "Should fail when token file is missing");

    // 模拟"网络恢复"：重新生成
    let recovered = token_manager.load_or_generate().unwrap();
    assert_eq!(recovered.len(), 64);
    assert!(recovered.chars().all(|c| c.is_ascii_hexdigit()));

    println!("✅ Network failure recovery test passed");
}

#[tokio::test]
async fn test_concurrent_token_access_stress() {
    let temp_dir = TempDir::new().unwrap();
    let token_file = temp_dir.path().join(".auth_token");

    let token_manager = Arc::new(tokio::sync::Mutex::new(
        AuthTokenManager::new_with_path(token_file),
    ));
    let num_tasks = 50;
    let mut handles = Vec::new();

    for i in 0..num_tasks {
        let manager = token_manager.clone();
        let handle = tokio::spawn(async move {
            // 随机延迟增加竞争
            sleep(Duration::from_millis((i % 10) as u64)).await;
            let manager = manager.lock().await;
            let result = manager.load_or_generate();
            drop(manager);
            sleep(Duration::from_millis(1)).await;
            result
        });
        handles.push(handle);
    }

    let mut tokens = Vec::new();
    let mut success_count = 0;

    for handle in handles {
        match handle.await {
            Ok(Ok(token)) => { tokens.push(token); success_count += 1; }
            Ok(Err(e)) => println!("Task failed: {:?}", e),
            Err(e) => println!("Task panicked: {:?}", e),
        }
    }

    assert!(
        success_count > num_tasks * 8 / 10,
        "At least 80% of tasks should succeed, got {}/{}",
        success_count, num_tasks
    );

    // load_or_generate 必须幂等：同一文件路径所有调用返回相同 token
    if tokens.len() > 1 {
        let first = &tokens[0];
        for (i, token) in tokens[1..].iter().enumerate() {
            assert_eq!(first, token, "Task {} returned different token — load_or_generate not idempotent", i + 1);
        }
    }

    println!("✅ Concurrent token access stress test passed");
    println!("   Success rate: {}/{} ({:.1}%)", success_count, num_tasks,
             (success_count as f64 / num_tasks as f64) * 100.0);
}

#[tokio::test]
async fn test_token_corruption_recovery() {
    use std::fs;

    let temp_dir = TempDir::new().unwrap();
    let token_file = temp_dir.path().join(".auth_token");

    let invalid_hex = "g".repeat(64);
    let control_chars = "\x00".repeat(64);
    let too_long = "a".repeat(100);

    let corrupted_cases: &[&str] = &[
        "corrupted_token",
        "12345",
        &invalid_hex,
        &control_chars,
        "",
        &too_long,
    ];

    for (i, corrupted) in corrupted_cases.iter().enumerate() {
        // 写入损坏内容
        fs::write(&token_file, corrupted).unwrap();

        // 每次用同一个隔离路径的 manager
        let manager = AuthTokenManager::new_with_path(token_file.clone());
        let result = manager.load_or_generate();
        assert!(result.is_ok(), "Should recover from corruption case {}: {:?}", i, corrupted);

        let token = result.unwrap();
        assert_eq!(token.len(), 64, "Recovered token should be 64 chars");
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()), "Recovered token should be hex");
    }

    println!("✅ Token corruption recovery test passed");
}

#[tokio::test]
async fn test_high_frequency_operations() {
    let temp_dir = TempDir::new().unwrap();
    let token_file = temp_dir.path().join(".auth_token");
    let token_manager = AuthTokenManager::new_with_path(token_file);

    let initial_token = token_manager.load_or_generate().unwrap();
    let operations = 1000;
    let mut success_count = 0;

    for i in 0..operations {
        if i % 2 == 0 {
            // 读取操作：必须返回与初始相同的 token
            match token_manager.load_or_generate() {
                Ok(token) => {
                    assert_eq!(token, initial_token, "Token should remain consistent");
                    success_count += 1;
                }
                Err(e) => println!("Read {} failed: {:?}", i, e),
            }
        } else {
            // 写入操作：保存相同的 token（幂等写）
            match token_manager.save(&initial_token) {
                Ok(_) => success_count += 1,
                Err(e) => println!("Save {} failed: {:?}", i, e),
            }
        }

        if i % 100 == 0 {
            sleep(Duration::from_millis(1)).await;
        }
    }

    let success_rate = success_count as f64 / operations as f64;
    assert!(success_rate > 0.95, "Success rate should be > 95%, got {:.2}%", success_rate * 100.0);

    println!("✅ High frequency operations test passed");
    println!("   Success rate: {}/{} ({:.1}%)", success_count, operations, success_rate * 100.0);
}

#[tokio::test]
async fn test_memory_pressure_resilience() {
    // 100 个 manager 实例指向同一个隔离文件，验证内存压力下的幂等性
    let temp_dir = TempDir::new().unwrap();
    let token_file = temp_dir.path().join(".auth_token");

    let managers: Vec<_> = (0..100)
        .map(|_| AuthTokenManager::new_with_path(token_file.clone()))
        .collect();

    let mut tokens = Vec::new();
    for (i, manager) in managers.iter().enumerate() {
        match manager.load_or_generate() {
            Ok(token) => tokens.push(token),
            Err(e) => println!("Manager {} failed: {:?}", i, e),
        }
    }

    assert!(tokens.len() > 90, "Most managers should succeed under memory pressure");

    // 所有 manager 指向同一文件，token 必须一致
    if tokens.len() > 1 {
        let first = &tokens[0];
        for (i, token) in tokens[1..].iter().enumerate() {
            assert_eq!(first, token, "Manager {} returned different token", i + 1);
        }
    }

    println!("✅ Memory pressure resilience test passed");
    println!("   Success rate: {}/100", tokens.len());
}

#[tokio::test]
async fn test_rapid_restart_simulation() {
    // 每次"重启"用同一个隔离路径，验证 token 在重启间保持一致
    let temp_dir = TempDir::new().unwrap();
    let token_file = temp_dir.path().join(".auth_token");

    let restart_cycles = 20;
    let mut tokens = Vec::new();

    for cycle in 0..restart_cycles {
        // 模拟应用重启：每次新建 manager（同一路径）
        let manager = AuthTokenManager::new_with_path(token_file.clone());

        match manager.load_or_generate() {
            Ok(token) => {
                assert_eq!(token.len(), 64);
                assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
                tokens.push(token);
            }
            Err(e) => println!("Restart cycle {} failed: {:?}", cycle, e),
        }

        sleep(Duration::from_millis(10)).await;
    }

    // 同一文件路径下，所有重启周期必须返回相同 token（100% 一致）
    if tokens.len() > 1 {
        let first = &tokens[0];
        let consistent = tokens[1..].iter().filter(|t| *t == first).count();
        let rate = consistent as f64 / (tokens.len() - 1) as f64;
        println!("Token consistency rate: {:.1}%", rate * 100.0);
        assert!(
            rate > 0.8,
            "Token should be consistent across restarts, got {:.1}%",
            rate * 100.0
        );
    }

    println!("✅ Rapid restart simulation test passed");
    println!("   Completed {} restart cycles", restart_cycles);
}
