//! 引擎启动时序测试。
//!
//! ## 背景
//!
//! 此测试模块专门覆盖"进程启动时序"这一维度。
//! 传统的单元测试和集成测试直接构造依赖，绕过了 Tauri 的
//! `manage()` → `State<>` 注入链路，无法捕获以下竞态条件：
//!
//! - 引擎异步启动未完成时，前端调用 Tauri 命令
//! - `EngineState` 未初始化时的错误处理
//! - 重复初始化 `EngineState` 的防护
//!
//! ## 覆盖维度
//!
//! - 启动时序测试（引擎未就绪 → 就绪的状态转换）
//! - 失败路径测试（引擎未就绪时的友好错误）
//! - 并发安全测试（多线程同时访问 EngineState）
//! - 契约测试（EngineState API 行为一致性）
//! - 冒烟测试（模拟真实启动流程）

#[cfg(test)]
mod engine_state_timing_tests {
    use std::sync::Arc;

    use ironclaw::context::ContextManager;
    use ironclaw::safety::{SafetyConfig, SafetyLayer};
    use ironclaw::tools::ToolRegistry;
    use tokio::sync::mpsc;

    use crate::safety_bridge::SafetyBridge;
    use crate::state::{AppState, EngineState};

    /// 构造一个最小可用的 AppState（用于测试初始化流程）。
    fn create_minimal_app_state() -> AppState {
        let (tx, _rx) = mpsc::channel(1);
        let safety_config = SafetyConfig {
            max_output_length: 100_000,
            injection_check_enabled: true,
        };
        let safety = Arc::new(SafetyLayer::new(&safety_config));
        let safety_bridge = Arc::new(SafetyBridge::new(Arc::clone(&safety), None, None));
        let egress: Arc<dyn dasclaw_core::EgressGate> = Arc::new(
            dasclaw_safety::egress_gate::IronclawEgressGate::new(Arc::clone(&safety)),
        );
        let attachment_scanner = Arc::new(
            crate::safety_attachment_scanner::AttachmentScanner::new(Arc::clone(&egress)),
        );
        let tools = Arc::new(ToolRegistry::new());
        let context_manager = Arc::new(ContextManager::new(5));
        let data_reporter = Arc::new(crate::data_reporter::DataReporter::new(
            "http://localhost:3000".to_string(),
            "test-token".to_string(),
        ));

        let model_override = Arc::new(std::sync::RwLock::new(None));
        let stub_llm: Arc<dyn ironclaw::llm::LlmProvider> = Arc::new(StubLlmProvider);
        let model_switch = Arc::new(crate::model_switch::ModelSwitchProvider::new(
            Arc::clone(&stub_llm),
            Arc::clone(&model_override),
        ));

        AppState {
            msg_sender: tx,
            db: None,
            workspace: None,
            tools,
            extension_manager: None,
            skill_registry: None,
            skill_catalog: None,
            skills_config: ironclaw::config::SkillsConfig::default(),
            safety,
            safety_bridge,
            attachment_scanner,
            egress,
            context_manager,
            conversation_tracker: Arc::new(crate::conversation_tracker::ConversationTracker::new(
                "test-owner".to_string(),
            )),
            data_reporter,
            scope_id: "test-owner".to_string(),
            backend_user_id: Arc::new(std::sync::RwLock::new(None)),
            llm: Arc::clone(&model_switch) as _,
            model_override,
            model_switch,
            provider_base_url: std::sync::RwLock::new(String::new()),
            initial_provider: Arc::clone(&stub_llm),
            initial_base_url: String::new(),
            log_broadcaster: Arc::new(ironclaw::channels::web::log_layer::LogBroadcaster::new()),
            log_clear_offset: std::sync::atomic::AtomicUsize::new(0),
            routine_engine_slot: Arc::new(tokio::sync::RwLock::new(None)),
            scheduler_slot: Arc::new(tokio::sync::RwLock::new(None)),
            disabled_skills: std::sync::RwLock::new(std::collections::HashSet::new()),
            disabled_extensions: std::sync::RwLock::new(std::collections::HashSet::new()),
        }
    }

    /// 最小 LLM provider stub（仅用于启动时序测试）。
    struct StubLlmProvider;

    #[async_trait::async_trait]
    impl ironclaw::llm::LlmProvider for StubLlmProvider {
        fn model_name(&self) -> &str {
            "stub-model"
        }
        fn cost_per_token(&self) -> (rust_decimal::Decimal, rust_decimal::Decimal) {
            (rust_decimal::Decimal::ZERO, rust_decimal::Decimal::ZERO)
        }
        async fn complete(
            &self,
            _req: ironclaw::llm::CompletionRequest,
        ) -> Result<ironclaw::llm::CompletionResponse, ironclaw::error::LlmError> {
            Err(ironclaw::error::LlmError::RequestFailed {
                provider: "stub".into(),
                reason: "not implemented".into(),
            })
        }
        async fn complete_with_tools(
            &self,
            _req: ironclaw::llm::ToolCompletionRequest,
        ) -> Result<ironclaw::llm::ToolCompletionResponse, ironclaw::error::LlmError> {
            Err(ironclaw::error::LlmError::RequestFailed {
                provider: "stub".into(),
                reason: "not implemented".into(),
            })
        }
    }

    // =========================================================================
    // 1. 启动时序测试 — EngineState 生命周期
    // =========================================================================

    #[test]
    fn test_timing_engine_state_starts_unready() {
        let engine = EngineState::new();
        assert!(!engine.is_ready(), "新创建的 EngineState 应该未就绪");
    }

    #[test]
    fn test_timing_engine_state_becomes_ready_after_initialize() {
        let engine = EngineState::new();
        assert!(!engine.is_ready());

        let state = create_minimal_app_state();
        engine.initialize(state).expect("首次初始化应该成功");

        assert!(engine.is_ready(), "初始化后 EngineState 应该就绪");
    }

    #[test]
    fn test_timing_engine_state_get_succeeds_after_initialize() {
        let engine = EngineState::new();

        let state = create_minimal_app_state();
        engine.initialize(state).unwrap();

        let app_state = engine.get();
        assert!(app_state.is_ok(), "初始化后 get() 应该成功");
        assert_eq!(app_state.unwrap().scope_id, "test-owner");
    }

    #[test]
    fn test_timing_double_initialize_rejected() {
        let engine = EngineState::new();

        let state1 = create_minimal_app_state();
        engine.initialize(state1).expect("首次初始化应该成功");

        let state2 = create_minimal_app_state();
        let result = engine.initialize(state2);
        assert!(result.is_err(), "重复初始化应该被拒绝");
    }

    // =========================================================================
    // 2. 失败路径测试 — 引擎未就绪时的行为
    // =========================================================================

    #[test]
    fn test_failure_get_before_initialize_returns_friendly_error() {
        let engine = EngineState::new();

        let result = engine.get();
        assert!(result.is_err());

        let err_msg = result.err().expect("已断言 is_err");
        assert!(
            err_msg.contains("启动中") || err_msg.contains("稍后"),
            "错误信息应该友好且有指导性，实际: {}",
            err_msg
        );
    }

    #[test]
    fn test_failure_is_ready_false_before_initialize() {
        let engine = EngineState::new();
        assert!(!engine.is_ready());
    }

    #[test]
    fn test_failure_multiple_get_calls_before_initialize_all_fail() {
        let engine = EngineState::new();

        for _ in 0..10 {
            assert!(engine.get().is_err(), "每次 get() 都应该返回错误");
        }
    }

    #[test]
    fn test_failure_get_error_does_not_panic() {
        // 确保未初始化时 get() 不会 panic，而是返回 Err
        let engine = EngineState::new();
        let _ = engine.get(); // 不应 panic
    }

    // =========================================================================
    // 3. 并发安全测试 — 多线程访问 EngineState
    // =========================================================================

    #[test]
    fn test_concurrent_get_before_initialize() {
        let engine = Arc::new(EngineState::new());
        let mut handles = vec![];

        for _ in 0..20 {
            let engine = Arc::clone(&engine);
            handles.push(std::thread::spawn(move || {
                let result = engine.get();
                assert!(result.is_err(), "未初始化时并发 get() 都应该返回错误");
            }));
        }

        for h in handles {
            h.join().expect("线程不应 panic");
        }
    }

    #[test]
    fn test_concurrent_get_after_initialize() {
        let engine = Arc::new(EngineState::new());
        engine.initialize(create_minimal_app_state()).unwrap();

        let mut handles = vec![];

        for _ in 0..20 {
            let engine = Arc::clone(&engine);
            handles.push(std::thread::spawn(move || {
                let result = engine.get();
                assert!(result.is_ok(), "初始化后并发 get() 都应该成功");
                assert_eq!(result.unwrap().scope_id, "test-owner");
            }));
        }

        for h in handles {
            h.join().expect("线程不应 panic");
        }
    }

    #[test]
    fn test_concurrent_initialize_only_one_succeeds() {
        // 模拟多个线程同时尝试初始化（防止重复引擎启动）
        let engine = Arc::new(EngineState::new());
        let success_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut handles = vec![];

        for _ in 0..10 {
            let engine = Arc::clone(&engine);
            let count = Arc::clone(&success_count);
            handles.push(std::thread::spawn(move || {
                let state = create_minimal_app_state();
                if engine.initialize(state).is_ok() {
                    count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
            }));
        }

        for h in handles {
            h.join().expect("线程不应 panic");
        }

        assert_eq!(
            success_count.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "只有一个线程应该成功初始化"
        );
        assert!(engine.is_ready());
    }

    #[test]
    fn test_concurrent_mixed_get_and_initialize() {
        // 模拟真实场景：一些线程在读取状态，一个线程在初始化
        let engine = Arc::new(EngineState::new());
        let mut handles = vec![];

        // 先启动 10 个读取线程（模拟前端命令调用）
        for _ in 0..10 {
            let engine = Arc::clone(&engine);
            handles.push(std::thread::spawn(move || {
                // 循环尝试获取状态，模拟前端重试
                for _ in 0..50 {
                    let _ = engine.get();
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }));
        }

        // 延迟一点后初始化（模拟引擎启动耗时）
        {
            let engine = Arc::clone(&engine);
            handles.push(std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(10));
                engine.initialize(create_minimal_app_state()).unwrap();
            }));
        }

        for h in handles {
            h.join().expect("线程不应 panic");
        }

        assert!(engine.is_ready());
        assert!(engine.get().is_ok());
    }

    // =========================================================================
    // 4. 契约测试 — EngineState API 行为一致性
    // =========================================================================

    #[test]
    fn test_contract_get_returns_same_state_on_multiple_calls() {
        let engine = EngineState::new();
        engine.initialize(create_minimal_app_state()).unwrap();

        let state1 = engine.get().unwrap();
        let state2 = engine.get().unwrap();

        // 同一个 EngineState 的多次 get() 应该返回同一个 AppState
        assert_eq!(
            state1.scope_id, state2.scope_id,
            "多次 get() 应该返回相同的状态"
        );
    }

    #[test]
    fn test_contract_is_ready_consistent_with_get() {
        let engine = EngineState::new();

        // 未初始化：is_ready=false, get=Err
        assert!(!engine.is_ready());
        assert!(engine.get().is_err());

        // 初始化后：is_ready=true, get=Ok
        engine.initialize(create_minimal_app_state()).unwrap();
        assert!(engine.is_ready());
        assert!(engine.get().is_ok());
    }

    #[test]
    fn test_contract_app_state_fields_accessible_after_init() {
        let engine = EngineState::new();
        engine.initialize(create_minimal_app_state()).unwrap();

        let state = engine.get().unwrap();

        // 验证所有字段都可访问（不 panic）
        assert_eq!(state.scope_id, "test-owner");
        assert!(state.db.is_none()); // 最小状态没有 DB
        assert!(state.workspace.is_none());
        assert!(state.extension_manager.is_none());
        assert!(state.skill_registry.is_none());
        assert!(state.skill_catalog.is_none());
        // tools, safety, safety_bridge, context_manager 是 Arc，始终存在
        let _ = state.tools.count();
        let _ = state.safety_bridge.cumulative_stats();
    }

    // =========================================================================
    // 5. 冒烟测试 — 模拟真实启动流程
    // =========================================================================

    /// 模拟 main.rs 的启动流程：
    /// 1. 创建 EngineState（同步）
    /// 2. 前端立即调用命令（应该返回友好错误）
    /// 3. 引擎启动完成，初始化 EngineState
    /// 4. 前端再次调用命令（应该成功）
    #[tokio::test]
    async fn test_smoke_simulated_startup_sequence() {
        // Phase 1: 创建空壳状态（模拟 main.rs setup）
        let engine = Arc::new(EngineState::new());

        // Phase 2: 前端立即调用命令（引擎未就绪）
        assert!(engine.get().is_err(), "引擎未就绪时命令应该失败");

        // Phase 3: 异步启动引擎（模拟 spawn(start_ironclaw_engine)）
        let engine_clone = Arc::clone(&engine);
        let init_handle = tokio::spawn(async move {
            // 模拟引擎启动耗时
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let state = create_minimal_app_state();
            engine_clone.initialize(state).unwrap();
        });

        // Phase 4: 前端在引擎启动期间重试
        let engine_clone = Arc::clone(&engine);
        let retry_handle = tokio::spawn(async move {
            let mut attempts = 0;
            loop {
                match engine_clone.get() {
                    Ok(state) => {
                        assert_eq!(state.scope_id, "test-owner");
                        return attempts;
                    }
                    Err(_) => {
                        attempts += 1;
                        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                    }
                }
            }
        });

        init_handle.await.unwrap();
        let attempts = retry_handle.await.unwrap();

        assert!(engine.is_ready());
        assert!(
            attempts > 0,
            "应该至少重试过一次（证明确实经历了未就绪阶段）"
        );
    }

    /// 模拟引擎启动失败的场景：
    /// EngineState 永远不会被初始化，所有命令持续返回错误。
    #[tokio::test]
    async fn test_smoke_engine_startup_failure() {
        let engine = Arc::new(EngineState::new());

        // 模拟引擎启动失败（不调用 initialize）
        // 前端命令应该持续返回友好错误，不应 panic
        for _ in 0..100 {
            let result = engine.get();
            assert!(result.is_err());
            let err = result.err().expect("已断言 is_err");
            assert!(!err.is_empty(), "错误信息不应为空");
        }

        assert!(!engine.is_ready());
    }

    // =========================================================================
    // 6. 启动失败状态测试 — set_failed() / is_failed()
    // =========================================================================

    #[test]
    fn test_failure_set_failed_changes_error_message() {
        let engine = EngineState::new();

        // 启动中：返回"正在启动中"
        let err1 = engine.get().err().expect("应该返回错误");
        assert!(err1.contains("启动中"), "启动中错误: {}", err1);

        // 标记失败
        engine.set_failed("LLM_API_KEY not configured".to_string());

        // 失败后：返回具体失败原因
        let err2 = engine.get().err().expect("应该返回错误");
        assert!(
            err2.contains("启动失败") && err2.contains("LLM_API_KEY"),
            "失败错误应包含原因: {}",
            err2
        );
    }

    #[test]
    fn test_failure_is_failed_reflects_state() {
        let engine = EngineState::new();
        assert!(!engine.is_failed(), "新建时不应是失败状态");
        assert!(!engine.is_ready(), "新建时不应是就绪状态");

        engine.set_failed("config error".to_string());
        assert!(engine.is_failed(), "set_failed 后应是失败状态");
        assert!(!engine.is_ready(), "失败后不应是就绪状态");
    }

    #[test]
    fn test_failure_set_failed_does_not_block_initialize() {
        // 边界情况：如果 set_failed 后又调用 initialize（不应发生，但要安全）
        let engine = EngineState::new();
        engine.set_failed("temporary error".to_string());

        // initialize 仍然可以成功（OnceLock 独立于 failure）
        let result = engine.initialize(create_minimal_app_state());
        assert!(result.is_ok(), "initialize 不应被 set_failed 阻止");

        // 初始化成功后，get() 应该返回 Ok（Ready 优先于 Failed）
        assert!(engine.is_ready());
        assert!(engine.get().is_ok(), "Ready 状态应优先于 Failed");
    }

    #[test]
    fn test_failure_concurrent_set_failed_and_get() {
        let engine = Arc::new(EngineState::new());
        let mut handles = vec![];

        // 一个线程设置失败
        {
            let engine = Arc::clone(&engine);
            handles.push(std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(5));
                engine.set_failed("async config error".to_string());
            }));
        }

        // 多个线程并发读取
        for _ in 0..20 {
            let engine = Arc::clone(&engine);
            handles.push(std::thread::spawn(move || {
                for _ in 0..50 {
                    let _ = engine.get(); // 不应 panic
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }));
        }

        for h in handles {
            h.join().expect("线程不应 panic");
        }
    }

    /// 模拟真实场景：引擎启动失败后前端持续重试。
    #[tokio::test]
    async fn test_smoke_engine_failure_with_set_failed() {
        let engine = Arc::new(EngineState::new());

        // Phase 1: 引擎正在启动
        let err = engine.get().err().expect("应该返回错误");
        assert!(err.contains("启动中"));

        // Phase 2: 引擎启动失败
        let engine_clone = Arc::clone(&engine);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            engine_clone.set_failed(
                "Failed to load IronClaw configuration: LLM_API_KEY is required".to_string(),
            );
        })
        .await
        .unwrap();

        // Phase 3: 前端再次调用，应该得到具体的失败原因
        let err = engine.get().err().expect("应该返回错误");
        assert!(err.contains("启动失败"), "应包含'启动失败': {}", err);
        assert!(err.contains("LLM_API_KEY"), "应包含具体原因: {}", err);
    }

    /// 模拟高并发场景：引擎启动的同时大量命令涌入。
    #[tokio::test]
    async fn test_smoke_high_concurrency_during_startup() {
        let engine = Arc::new(EngineState::new());
        let mut handles = vec![];

        // 启动 50 个并发命令（模拟前端多个组件同时初始化）
        for i in 0..50 {
            let engine = Arc::clone(&engine);
            handles.push(tokio::spawn(async move {
                // 随机延迟，模拟不同组件的加载时间
                tokio::time::sleep(std::time::Duration::from_millis(i % 10)).await;
                let _ = engine.get(); // 不应 panic
            }));
        }

        // 引擎在 20ms 后启动完成
        {
            let engine = Arc::clone(&engine);
            handles.push(tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                engine.initialize(create_minimal_app_state()).unwrap();
            }));
        }

        for h in handles {
            h.await.expect("任务不应 panic");
        }

        assert!(engine.is_ready());
    }
}
