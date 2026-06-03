//! 记忆/工作空间 IPC 命令测试。
//!
//! 覆盖维度：
//! - 单元测试（正常路径 + 错误路径）
//! - 契约测试（MemoryEntry / MemoryDocument / MemorySearchResult 格式）
//! - 安全审计测试（路径遍历防护、敏感信息不泄露）
//! - 数据级覆盖（边界值、空值、特殊字符）
//! - 失败路径测试（无效路径、空内容等）

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use ironclaw::workspace::{
        MemoryDocument as WorkspaceMemoryDocument, SearchResult as WorkspaceSearchResult,
        WorkspaceEntry,
    };
    use serde_json::json;
    use uuid::Uuid;

    use crate::ipc::memory::{
        memory_list_path, memory_search_limit, workspace_document_to_memory_document,
        workspace_entry_to_memory_entry, workspace_search_result_to_memory_search_result,
        MemoryDocument, MemoryEntry, MemorySearchResult,
    };

    fn fixed_time() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-03T10:11:12Z")
            .expect("fixed timestamp should parse")
            .with_timezone(&Utc)
    }

    fn workspace_document(path: &str, content: &str) -> WorkspaceMemoryDocument {
        let timestamp = fixed_time();
        WorkspaceMemoryDocument {
            id: Uuid::new_v4(),
            user_id: "user-1".into(),
            agent_id: Some(Uuid::new_v4()),
            path: path.into(),
            content: content.into(),
            created_at: timestamp,
            updated_at: timestamp,
            metadata: json!({"private": true}),
        }
    }

    fn workspace_search_result(path: &str, content: &str) -> WorkspaceSearchResult {
        WorkspaceSearchResult {
            document_id: Uuid::new_v4(),
            document_path: path.into(),
            chunk_id: Uuid::new_v4(),
            content: content.into(),
            score: 0.75,
            fts_rank: Some(1),
            vector_rank: Some(2),
        }
    }

    // =========================================================================
    // 单元测试 — 正常路径
    // =========================================================================

    #[test]
    fn test_memory_entry_serialization() {
        let entry = MemoryEntry {
            name: "README.md".into(),
            path: "/projects/alpha/README.md".into(),
            is_directory: false,
            content_preview: Some("# Project Alpha".into()),
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["name"], "README.md");
        assert_eq!(json["path"], "/projects/alpha/README.md");
        assert_eq!(json["is_directory"], false);
        assert_eq!(json["content_preview"], "# Project Alpha");
    }

    #[test]
    fn test_memory_entry_directory() {
        let entry = MemoryEntry {
            name: "projects".into(),
            path: "/projects".into(),
            is_directory: true,
            content_preview: None,
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["is_directory"], true);
        assert!(json["content_preview"].is_null());
    }

    #[test]
    fn test_memory_document_serialization() {
        let doc = MemoryDocument {
            path: "/notes/todo.md".into(),
            content: "- Buy milk\n- Fix bug #42".into(),
            updated_at: Some("2025-03-22T10:00:00+00:00".into()),
        };
        let json = serde_json::to_value(&doc).unwrap();
        assert_eq!(json["path"], "/notes/todo.md");
        assert!(json["content"].as_str().unwrap().contains("Fix bug #42"));
        assert_eq!(json["updated_at"], "2025-03-22T10:00:00+00:00");
    }

    #[test]
    fn test_memory_search_result_serialization() {
        let result = MemorySearchResult {
            path: "/docs/api.md".into(),
            content: "REST API documentation".into(),
            score: 0.5,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["path"], "/docs/api.md");
        // score 是 f32，使用 f32 精度比较
        assert!((json["score"].as_f64().unwrap() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_memory_entry_deserialization() {
        let json = r#"{
            "name": "config.toml",
            "path": "/config.toml",
            "is_directory": false,
            "content_preview": null
        }"#;
        let entry: MemoryEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.name, "config.toml");
        assert!(entry.content_preview.is_none());
    }

    #[test]
    fn req_memory_list_defaults_to_root_path() {
        assert_eq!(memory_list_path(None), "/");
        assert_eq!(memory_list_path(Some("/projects")), "/projects");
    }

    #[test]
    fn req_memory_search_uses_default_limit() {
        assert_eq!(memory_search_limit(None), 10);
        assert_eq!(memory_search_limit(Some(3)), 3);
        assert_eq!(memory_search_limit(Some(0)), 0);
    }

    #[test]
    fn req_memory_list_maps_workspace_entries() {
        let entry = WorkspaceEntry {
            path: "/projects/alpha/README.md".into(),
            is_directory: false,
            updated_at: Some(fixed_time()),
            content_preview: Some("# Alpha".into()),
        };

        let dto = workspace_entry_to_memory_entry(entry);

        assert_eq!(dto.name, "README.md");
        assert_eq!(dto.path, "/projects/alpha/README.md");
        assert!(!dto.is_directory);
        assert_eq!(dto.content_preview.as_deref(), Some("# Alpha"));
    }

    #[test]
    fn req_memory_read_maps_workspace_document_without_internal_ids() {
        let dto = workspace_document_to_memory_document(
            "/notes/todo.md".into(),
            workspace_document("/storage/internal.md", "ship memory coverage"),
        );
        let json = serde_json::to_string(&dto).expect("dto should serialize");

        assert_eq!(dto.path, "/notes/todo.md");
        assert_eq!(dto.content, "ship memory coverage");
        assert_eq!(dto.updated_at.as_deref(), Some("2026-06-03T10:11:12+00:00"));
        assert!(!json.contains("user_id"));
        assert!(!json.contains("agent_id"));
        assert!(!json.contains("metadata"));
    }

    #[test]
    fn req_memory_search_maps_workspace_results_without_rank_ids() {
        let dto = workspace_search_result_to_memory_search_result(workspace_search_result(
            "/docs/api.md",
            "memory search hit",
        ));
        let json = serde_json::to_string(&dto).expect("dto should serialize");

        assert_eq!(dto.path, "/docs/api.md");
        assert_eq!(dto.content, "memory search hit");
        assert!((dto.score - 0.75).abs() < f32::EPSILON);
        assert!(!json.contains("document_id"));
        assert!(!json.contains("chunk_id"));
        assert!(!json.contains("fts_rank"));
        assert!(!json.contains("vector_rank"));
    }

    // =========================================================================
    // 契约测试 — 验证与前端 TypeScript 类型的兼容性
    // =========================================================================

    /// 前端 MemoryEntry 类型定义：
    /// ```typescript
    /// interface MemoryEntry {
    ///   name: string;
    ///   path: string;
    ///   is_directory: boolean;
    ///   content_preview: string | null;
    /// }
    /// ```
    #[test]
    fn test_contract_memory_entry_matches_frontend() {
        let entry = MemoryEntry {
            name: "test.md".into(),
            path: "/test.md".into(),
            is_directory: false,
            content_preview: Some("preview".into()),
        };
        let json: serde_json::Value = serde_json::to_value(&entry).unwrap();

        assert!(json["name"].is_string());
        assert!(json["path"].is_string());
        assert!(json["is_directory"].is_boolean());
        assert!(json["content_preview"].is_string());

        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 4, "MemoryEntry should have exactly 4 fields");
    }

    /// 验证 content_preview 为 None 时序列化为 null。
    #[test]
    fn test_contract_memory_entry_null_preview() {
        let entry = MemoryEntry {
            name: "dir".into(),
            path: "/dir".into(),
            is_directory: true,
            content_preview: None,
        };
        let json: serde_json::Value = serde_json::to_value(&entry).unwrap();
        assert!(json["content_preview"].is_null());
    }

    /// 前端 MemoryDocument 类型定义：
    /// ```typescript
    /// interface MemoryDocument {
    ///   path: string;
    ///   content: string;
    ///   updated_at: string | null;
    /// }
    /// ```
    #[test]
    fn test_contract_memory_document_matches_frontend() {
        let doc = MemoryDocument {
            path: "/test.md".into(),
            content: "hello".into(),
            updated_at: Some("2025-01-01T00:00:00+00:00".into()),
        };
        let json: serde_json::Value = serde_json::to_value(&doc).unwrap();

        assert!(json["path"].is_string());
        assert!(json["content"].is_string());
        assert!(json["updated_at"].is_string());

        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 3, "MemoryDocument should have exactly 3 fields");
    }

    /// 前端 MemorySearchResult 类型定义：
    /// ```typescript
    /// interface MemorySearchResult {
    ///   path: string;
    ///   content: string;
    ///   score: number;
    /// }
    /// ```
    #[test]
    fn test_contract_memory_search_result_matches_frontend() {
        let result = MemorySearchResult {
            path: "/test.md".into(),
            content: "match".into(),
            score: 0.8,
        };
        let json: serde_json::Value = serde_json::to_value(&result).unwrap();

        assert!(json["path"].is_string());
        assert!(json["content"].is_string());
        assert!(json["score"].is_number());

        let obj = json.as_object().unwrap();
        assert_eq!(
            obj.len(),
            3,
            "MemorySearchResult should have exactly 3 fields"
        );
    }

    // =========================================================================
    // 安全审计测试
    // =========================================================================

    /// 验证 MemoryEntry 不包含 owner_id 或 agent_id。
    #[test]
    fn test_audit_memory_entry_no_owner_leak() {
        let entry = MemoryEntry {
            name: "secret.md".into(),
            path: "/secrets/api_keys.md".into(),
            is_directory: false,
            content_preview: Some("Contains API keys".into()),
        };
        let json_str = serde_json::to_string(&entry).unwrap();

        assert!(!json_str.contains("owner_id"));
        assert!(!json_str.contains("agent_id"));
        assert!(!json_str.contains("user_id"));
    }

    /// 验证 MemorySearchResult 不包含内部 ID（document_id, chunk_id）。
    #[test]
    fn test_audit_search_result_no_internal_ids() {
        let result = MemorySearchResult {
            path: "/docs/api.md".into(),
            content: "API documentation".into(),
            score: 0.9,
        };
        let json_str = serde_json::to_string(&result).unwrap();

        assert!(!json_str.contains("document_id"));
        assert!(!json_str.contains("chunk_id"));
        assert!(!json_str.contains("fts_rank"));
        assert!(!json_str.contains("vector_rank"));
    }

    /// 验证路径遍历攻击字符串在序列化中被保留（不被静默处理）。
    /// 实际的路径遍历防护在 Workspace 层，这里验证 IPC 层不会掩盖问题。
    #[test]
    fn test_audit_path_traversal_preserved_for_validation() {
        let entry = MemoryEntry {
            name: "../../etc/passwd".into(),
            path: "../../etc/passwd".into(),
            is_directory: false,
            content_preview: None,
        };
        let json = serde_json::to_value(&entry).unwrap();
        // 路径遍历字符串应被保留（由 Workspace 层验证和拒绝）
        assert_eq!(json["path"], "../../etc/passwd");
    }

    // =========================================================================
    // 数据级覆盖 — 边界值和特殊字符
    // =========================================================================

    #[test]
    fn test_data_empty_content() {
        let doc = MemoryDocument {
            path: "/empty.md".into(),
            content: "".into(),
            updated_at: None,
        };
        let json = serde_json::to_string(&doc).unwrap();
        let parsed: MemoryDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.content, "");
    }

    #[test]
    fn test_data_unicode_path() {
        let entry = MemoryEntry {
            name: "笔记.md".into(),
            path: "/中文目录/笔记.md".into(),
            is_directory: false,
            content_preview: Some("中文内容预览".into()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: MemoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "笔记.md");
        assert!(parsed.path.contains("中文目录"));
    }

    #[test]
    fn test_data_very_long_content() {
        let long_content = "x".repeat(1_000_000);
        let doc = MemoryDocument {
            path: "/large.md".into(),
            content: long_content.clone(),
            updated_at: None,
        };
        let json = serde_json::to_string(&doc).unwrap();
        let parsed: MemoryDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.content.len(), 1_000_000);
    }

    #[test]
    fn test_data_zero_score() {
        let result = MemorySearchResult {
            path: "/low.md".into(),
            content: "barely relevant".into(),
            score: 0.0,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert!((json["score"].as_f64().unwrap()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_data_max_score() {
        let result = MemorySearchResult {
            path: "/perfect.md".into(),
            content: "exact match".into(),
            score: 1.0,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert!((json["score"].as_f64().unwrap() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_data_special_characters_in_path() {
        let entry = MemoryEntry {
            name: "file (1).md".into(),
            path: "/docs/file (1).md".into(),
            is_directory: false,
            content_preview: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: MemoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.path, "/docs/file (1).md");
    }

    #[test]
    fn test_data_newlines_in_content() {
        let doc = MemoryDocument {
            path: "/multi.md".into(),
            content: "line1\nline2\r\nline3\ttab".into(),
            updated_at: None,
        };
        let json = serde_json::to_string(&doc).unwrap();
        let parsed: MemoryDocument = serde_json::from_str(&json).unwrap();
        assert!(parsed.content.contains('\n'));
        assert!(parsed.content.contains('\t'));
    }
}
