//! 扩展管理 IPC 命令测试。
//!
//! 覆盖维度：
//! - 单元测试（正常路径 + 错误路径）
//! - 契约测试（ExtensionInfo 格式与前端 TypeScript 类型匹配）
//! - 安全审计测试（敏感信息不泄露）
//! - 数据级覆盖（边界值、空值、特殊字符）
//! - 失败路径测试（空工具列表、未安装扩展等）

#[cfg(test)]
mod tests {
    use crate::ipc::extensions::{
        ExtensionInfo, ExtensionSetupField, ExtensionSetupResponse, ExtensionSetupSubmitResponse,
    };

    // =========================================================================
    // 单元测试 — 正常路径
    // =========================================================================

    #[test]
    fn test_extension_info_serialization() {
        let info = ExtensionInfo {
            name: "github-mcp".into(),
            display_name: Some("GitHub MCP Server".into()),
            kind: "Mcp".into(),
            installed: true,
            active: true,
            authenticated: true,
            tools: vec!["create_issue".into(), "list_repos".into()],
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["name"], "github-mcp");
        assert_eq!(json["display_name"], "GitHub MCP Server");
        assert_eq!(json["kind"], "Mcp");
        assert_eq!(json["installed"], true);
        assert_eq!(json["active"], true);
        assert_eq!(json["authenticated"], true);
        assert_eq!(json["tools"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_extension_info_deserialization() {
        let json = r#"{
            "name": "slack",
            "display_name": null,
            "kind": "Wasm",
            "installed": false,
            "active": false,
            "authenticated": false,
            "tools": []
        }"#;
        let info: ExtensionInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.name, "slack");
        assert!(info.display_name.is_none());
        assert!(!info.installed);
        assert!(info.tools.is_empty());
    }

    #[test]
    fn test_extension_info_roundtrip() {
        let original = ExtensionInfo {
            name: "test-ext".into(),
            display_name: Some("Test Extension".into()),
            kind: "Mcp".into(),
            installed: true,
            active: false,
            authenticated: true,
            tools: vec!["tool_a".into(), "tool_b".into(), "tool_c".into()],
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: ExtensionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, original.name);
        assert_eq!(parsed.display_name, original.display_name);
        assert_eq!(parsed.tools.len(), 3);
    }

    // =========================================================================
    // 契约测试 — 验证与前端 TypeScript 类型的兼容性
    // =========================================================================

    /// 前端 ExtensionInfo 类型定义：
    /// ```typescript
    /// interface ExtensionInfo {
    ///   name: string;
    ///   display_name: string | null;
    ///   kind: string;
    ///   installed: boolean;
    ///   active: boolean;
    ///   authenticated: boolean;
    ///   tools: string[];
    /// }
    /// ```
    #[test]
    fn test_contract_extension_info_matches_frontend() {
        let info = ExtensionInfo {
            name: "test".into(),
            display_name: Some("Test".into()),
            kind: "Mcp".into(),
            installed: true,
            active: true,
            authenticated: false,
            tools: vec!["tool1".into()],
        };
        let json: serde_json::Value = serde_json::to_value(&info).unwrap();

        // 验证字段名
        assert!(json.get("name").is_some(), "missing 'name'");
        assert!(json.get("display_name").is_some(), "missing 'display_name'");
        assert!(json.get("kind").is_some(), "missing 'kind'");
        assert!(json.get("installed").is_some(), "missing 'installed'");
        assert!(json.get("active").is_some(), "missing 'active'");
        assert!(json.get("authenticated").is_some(), "missing 'authenticated'");
        assert!(json.get("tools").is_some(), "missing 'tools'");

        // 验证类型
        assert!(json["name"].is_string());
        assert!(json["display_name"].is_string()); // Some → string
        assert!(json["kind"].is_string());
        assert!(json["installed"].is_boolean());
        assert!(json["active"].is_boolean());
        assert!(json["authenticated"].is_boolean());
        assert!(json["tools"].is_array());

        // 验证 tools 数组元素类型
        for tool in json["tools"].as_array().unwrap() {
            assert!(tool.is_string(), "tools array should contain strings");
        }

        // 验证字段数量
        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 7, "ExtensionInfo should have exactly 7 fields");
    }

    /// 验证 display_name 为 None 时序列化为 null。
    #[test]
    fn test_contract_extension_info_null_display_name() {
        let info = ExtensionInfo {
            name: "test".into(),
            display_name: None,
            kind: "Mcp".into(),
            installed: false,
            active: false,
            authenticated: false,
            tools: vec![],
        };
        let json: serde_json::Value = serde_json::to_value(&info).unwrap();
        assert!(
            json["display_name"].is_null(),
            "None display_name should serialize to null"
        );
    }

    /// 验证搜索结果中 display_name 始终为 Some（来自 RegistryEntry.display_name: String）。
    #[test]
    fn test_contract_search_result_always_has_display_name() {
        // 搜索结果中 display_name 来自 RegistryEntry.display_name（非 Option），
        // 所以在 ic_search_extensions 中包装为 Some(...)
        let info = ExtensionInfo {
            name: "search-result".into(),
            display_name: Some("Search Result Display".into()),
            kind: "Mcp".into(),
            installed: false,
            active: false,
            authenticated: false,
            tools: vec![],
        };
        let json: serde_json::Value = serde_json::to_value(&info).unwrap();
        assert!(json["display_name"].is_string());
    }

    // =========================================================================
    // 安全审计测试
    // =========================================================================

    /// 验证 ExtensionInfo 不包含 URL、API Key 或认证凭据。
    #[test]
    fn test_audit_extension_info_no_credentials_leak() {
        let info = ExtensionInfo {
            name: "github-mcp".into(),
            display_name: Some("GitHub".into()),
            kind: "Mcp".into(),
            installed: true,
            active: true,
            authenticated: true,
            tools: vec!["create_issue".into()],
        };
        let json_str = serde_json::to_string(&info).unwrap();

        assert!(!json_str.contains("url"));
        assert!(!json_str.contains("api_key"));
        assert!(!json_str.contains("token"));
        assert!(!json_str.contains("secret"));
        assert!(!json_str.contains("password"));
        assert!(!json_str.contains("auth_hint"));
        assert!(!json_str.contains("source"));
    }

    /// 验证 ExtensionInfo 不包含内部配置路径。
    #[test]
    fn test_audit_extension_info_no_config_path_leak() {
        let info = ExtensionInfo {
            name: "test".into(),
            display_name: None,
            kind: "Mcp".into(),
            installed: true,
            active: true,
            authenticated: false,
            tools: vec![],
        };
        let json_str = serde_json::to_string(&info).unwrap();

        assert!(!json_str.contains("config"));
        assert!(!json_str.contains("path"));
        assert!(!json_str.contains("dir"));
        assert!(!json_str.contains("home"));
    }

    // =========================================================================
    // 数据级覆盖 — 边界值和特殊字符
    // =========================================================================

    #[test]
    fn test_data_empty_tools_list() {
        let info = ExtensionInfo {
            name: "no-tools".into(),
            display_name: None,
            kind: "Mcp".into(),
            installed: true,
            active: false,
            authenticated: false,
            tools: vec![],
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["tools"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_data_many_tools() {
        let tools: Vec<String> = (0..100).map(|i| format!("tool_{}", i)).collect();
        let info = ExtensionInfo {
            name: "mega-ext".into(),
            display_name: Some("Mega Extension".into()),
            kind: "Mcp".into(),
            installed: true,
            active: true,
            authenticated: true,
            tools,
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["tools"].as_array().unwrap().len(), 100);
    }

    #[test]
    fn test_data_unicode_display_name() {
        let info = ExtensionInfo {
            name: "chinese-ext".into(),
            display_name: Some("中文扩展 🔧".into()),
            kind: "Mcp".into(),
            installed: true,
            active: true,
            authenticated: false,
            tools: vec![],
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: ExtensionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.display_name, Some("中文扩展 🔧".into()));
    }

    #[test]
    fn test_data_all_boolean_combinations() {
        let combos = [
            (false, false, false),
            (false, false, true),
            (false, true, false),
            (false, true, true),
            (true, false, false),
            (true, false, true),
            (true, true, false),
            (true, true, true),
        ];
        for (installed, active, authenticated) in combos {
            let info = ExtensionInfo {
                name: "test".into(),
                display_name: None,
                kind: "Mcp".into(),
                installed,
                active,
                authenticated,
                tools: vec![],
            };
            let json = serde_json::to_string(&info).unwrap();
            let parsed: ExtensionInfo = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.installed, installed);
            assert_eq!(parsed.active, active);
            assert_eq!(parsed.authenticated, authenticated);
        }
    }

    #[test]
    fn test_data_kind_variants() {
        // 验证不同 kind 字符串都能正确序列化
        for kind in &["Mcp", "Wasm", "Native", "Unknown"] {
            let info = ExtensionInfo {
                name: "test".into(),
                display_name: None,
                kind: kind.to_string(),
                installed: false,
                active: false,
                authenticated: false,
                tools: vec![],
            };
            let json = serde_json::to_value(&info).unwrap();
            assert_eq!(json["kind"], *kind);
        }
    }

    #[test]
    fn test_data_special_characters_in_tool_names() {
        let info = ExtensionInfo {
            name: "test".into(),
            display_name: None,
            kind: "Mcp".into(),
            installed: true,
            active: true,
            authenticated: false,
            tools: vec![
                "create_or_update_file".into(),
                "search-code".into(),
                "get.user.info".into(),
            ],
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: ExtensionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tools[0], "create_or_update_file");
        assert_eq!(parsed.tools[1], "search-code");
        assert_eq!(parsed.tools[2], "get.user.info");
    }

    // =========================================================================
    // 扩展配置（Setup）— 单元测试
    // =========================================================================

    #[test]
    fn test_setup_field_serialization() {
        let field = ExtensionSetupField {
            name: "api_key".into(),
            prompt: "Enter your API key".into(),
            optional: false,
            provided: false,
            input_type: "Text".into(),
        };
        let json = serde_json::to_value(&field).unwrap();
        assert_eq!(json["name"], "api_key");
        assert_eq!(json["prompt"], "Enter your API key");
        assert_eq!(json["optional"], false);
        assert_eq!(json["provided"], false);
        assert_eq!(json["input_type"], "Text");
    }

    #[test]
    fn test_setup_field_roundtrip() {
        let original = ExtensionSetupField {
            name: "webhook_secret".into(),
            prompt: "Webhook secret (auto-generated if empty)".into(),
            optional: true,
            provided: true,
            input_type: "AutoGenerate".into(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: ExtensionSetupField = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, original.name);
        assert_eq!(parsed.optional, original.optional);
        assert_eq!(parsed.input_type, original.input_type);
    }

    #[test]
    fn test_setup_response_serialization() {
        let resp = ExtensionSetupResponse {
            name: "telegram".into(),
            kind: "WasmChannel".into(),
            secrets: vec![
                ExtensionSetupField {
                    name: "bot_token".into(),
                    prompt: "Telegram Bot Token".into(),
                    optional: false,
                    provided: false,
                    input_type: "Text".into(),
                },
                ExtensionSetupField {
                    name: "webhook_secret".into(),
                    prompt: "Webhook Secret".into(),
                    optional: true,
                    provided: false,
                    input_type: "AutoGenerate".into(),
                },
            ],
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["name"], "telegram");
        assert_eq!(json["kind"], "WasmChannel");
        assert_eq!(json["secrets"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_setup_submit_response_serialization() {
        let resp = ExtensionSetupSubmitResponse {
            success: true,
            message: "Extension configured successfully".into(),
            activated: true,
            auth_url: None,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["success"], true);
        assert_eq!(json["activated"], true);
        assert!(json["auth_url"].is_null());
    }

    #[test]
    fn test_setup_submit_response_with_auth_url() {
        let resp = ExtensionSetupSubmitResponse {
            success: true,
            message: "OAuth flow started".into(),
            activated: false,
            auth_url: Some("https://oauth.example.com/authorize?client_id=xxx".into()),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["success"], true);
        assert_eq!(json["activated"], false);
        assert!(json["auth_url"].is_string());
    }

    // =========================================================================
    // 扩展配置（Setup）— 契约测试
    // =========================================================================

    /// 前端 ExtensionSetupField 类型定义：
    /// ```typescript
    /// interface ExtensionSetupField {
    ///   name: string;
    ///   prompt: string;
    ///   optional: boolean;
    ///   provided: boolean;
    ///   input_type: boolean;
    /// }
    /// ```
    #[test]
    fn test_contract_setup_field_matches_frontend() {
        let field = ExtensionSetupField {
            name: "test".into(),
            prompt: "Test prompt".into(),
            optional: false,
            provided: true,
            input_type: "Text".into(),
        };
        let json = serde_json::to_value(&field).unwrap();

        assert!(json.get("name").is_some());
        assert!(json.get("prompt").is_some());
        assert!(json.get("optional").is_some());
        assert!(json.get("provided").is_some());
        assert!(json.get("input_type").is_some());

        assert!(json["name"].is_string());
        assert!(json["prompt"].is_string());
        assert!(json["optional"].is_boolean());
        assert!(json["provided"].is_boolean());
        assert!(json["input_type"].is_string());

        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 5, "ExtensionSetupField should have exactly 5 fields");
    }

    /// 前端 ExtensionSetupSubmitResponse 类型定义：
    /// ```typescript
    /// interface ExtensionSetupSubmitResponse {
    ///   success: boolean;
    ///   message: string;
    ///   activated: boolean;
    ///   auth_url: string | null;
    /// }
    /// ```
    #[test]
    fn test_contract_setup_submit_response_matches_frontend() {
        let resp = ExtensionSetupSubmitResponse {
            success: true,
            message: "OK".into(),
            activated: true,
            auth_url: None,
        };
        let json = serde_json::to_value(&resp).unwrap();

        assert!(json["success"].is_boolean());
        assert!(json["message"].is_string());
        assert!(json["activated"].is_boolean());
        assert!(json["auth_url"].is_null());

        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 4, "ExtensionSetupSubmitResponse should have exactly 4 fields");
    }

    // =========================================================================
    // 扩展配置（Setup）— 安全审计测试
    // =========================================================================

    /// 验证 setup response 不泄露实际 secret 值。
    #[test]
    fn test_audit_setup_response_no_secret_values() {
        let resp = ExtensionSetupResponse {
            name: "github-mcp".into(),
            kind: "Mcp".into(),
            secrets: vec![ExtensionSetupField {
                name: "api_key".into(),
                prompt: "GitHub API Key".into(),
                optional: false,
                provided: true, // 已配置，但不应包含实际值
                input_type: "Text".into(),
            }],
        };
        let json_str = serde_json::to_string(&resp).unwrap();

        // 不应包含实际的 secret 值
        assert!(!json_str.contains("ghp_"));
        assert!(!json_str.contains("sk-"));
        assert!(!json_str.contains("Bearer"));
    }

    /// 验证 submit response 不泄露 OAuth client_secret。
    #[test]
    fn test_audit_setup_submit_no_client_secret() {
        let resp = ExtensionSetupSubmitResponse {
            success: true,
            message: "Configured".into(),
            activated: true,
            auth_url: Some("https://oauth.example.com/authorize?client_id=public_id".into()),
        };
        let json_str = serde_json::to_string(&resp).unwrap();

        assert!(!json_str.contains("client_secret"));
        assert!(!json_str.contains("secret"));
    }
}
