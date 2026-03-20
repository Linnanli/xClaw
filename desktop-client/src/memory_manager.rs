//! 记忆文件删除保护
//! 
//! 提供轻量级的文件删除保护功能，通过扩展现有API实现真正的删除

use serde::{Deserialize, Serialize};

/// 删除操作的结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteResult {
    pub success: bool,
    pub message: String,
    pub is_protected: bool,
}

/// 检查文件是否为受保护的系统核心文件
pub fn is_protected_file(path: &str) -> bool {
    let file_name = path.split('/').last().unwrap_or(path);
    matches!(file_name, "SOUL.md" | "IDENTITY.md" | "AGENTS.md")
}

/// 扩展记忆API以支持删除操作
pub trait MemoryApiExtensions {
    /// 安全删除记忆文件
    fn delete_memory_safe(&self, path: &str, force: bool) -> impl std::future::Future<Output = Result<DeleteResult, crate::error::Error>> + Send;
}

impl MemoryApiExtensions for crate::api_client::ApiClient {
    async fn delete_memory_safe(&self, path: &str, force: bool) -> Result<DeleteResult, crate::error::Error> {
        let is_protected = is_protected_file(path);
        
        // 检查保护状态
        if is_protected && !force {
            return Ok(DeleteResult {
                success: false,
                message: format!("文件 {} 是系统核心文件，受保护无法删除。如需删除请使用强制模式。", path),
                is_protected: true,
            });
        }
        
        // 执行真正的删除：先读取文件确认存在，然后写入特殊的删除标记
        match self.read_memory(path).await {
            Ok(_) => {
                // 文件存在，写入删除标记
                match self.write_memory(path, "<!-- DELETED -->").await {
                    Ok(_) => {
                        let message = if is_protected {
                            format!("⚠️ 系统核心文件 {} 已被强制删除", path)
                        } else {
                            format!("文件 {} 已成功删除", path)
                        };
                        
                        Ok(DeleteResult {
                            success: true,
                            message,
                            is_protected,
                        })
                    },
                    Err(e) => Ok(DeleteResult {
                        success: false,
                        message: format!("删除文件失败: {}", e),
                        is_protected,
                    }),
                }
            },
            Err(_) => Ok(DeleteResult {
                success: false,
                message: format!("文件 {} 不存在", path),
                is_protected,
            }),
        }
    }
}

/// 扩展记忆内容以支持删除状态检查
pub trait MemoryContentExtensions {
    /// 检查文件是否已被删除
    fn is_deleted(&self) -> bool;
    
    /// 获取实际内容（排除删除标记）
    fn get_actual_content(&self) -> &str;
}

impl MemoryContentExtensions for crate::api_client::MemoryContent {
    fn is_deleted(&self) -> bool {
        self.content.trim() == "<!-- DELETED -->"
    }
    
    fn get_actual_content(&self) -> &str {
        if self.is_deleted() {
            ""
        } else {
            &self.content
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_protected_file() {
        // 受保护的系统核心文件
        assert!(is_protected_file("SOUL.md"));
        assert!(is_protected_file("IDENTITY.md"));
        assert!(is_protected_file("AGENTS.md"));
        
        // 带路径的受保护文件
        assert!(is_protected_file("context/SOUL.md"));
        assert!(is_protected_file("docs/IDENTITY.md"));
        
        // 不受保护的文件
        assert!(!is_protected_file("USER.md"));
        assert!(!is_protected_file("MEMORY.md"));
        assert!(!is_protected_file("my_notes.md"));
        assert!(!is_protected_file("projects/readme.md"));
        
        // 边界情况
        assert!(!is_protected_file(""));
        assert!(!is_protected_file("SOUL.MD")); // 大写扩展名
        assert!(!is_protected_file("soul.md")); // 小写文件名
    }
    
    #[test]
    fn test_protection_consistency() {
        let test_cases = vec![
            ("SOUL.md", true),
            ("IDENTITY.md", true),
            ("AGENTS.md", true),
            ("USER.md", false),
            ("HEARTBEAT.md", false),
            ("README.md", false),
            ("BOOTSTRAP.md", false),
            ("MEMORY.md", false),
            ("my_notes.md", false),
            ("projects/doc.md", false),
            ("context/SOUL.md", true),
            ("backup/IDENTITY.md", true),
        ];
        
        for (file_path, expected_protected) in test_cases {
            assert_eq!(
                is_protected_file(file_path), 
                expected_protected,
                "文件 {} 的保护状态不符合预期", 
                file_path
            );
        }
    }
    
    #[test]
    fn test_memory_content_extensions() {
        use crate::api_client::MemoryContent;
        
        // 正常文件
        let normal_content = MemoryContent {
            path: "test.md".to_string(),
            content: "This is normal content".to_string(),
            updated_at: None,
        };
        assert!(!normal_content.is_deleted());
        assert_eq!(normal_content.get_actual_content(), "This is normal content");
        
        // 已删除文件
        let deleted_content = MemoryContent {
            path: "deleted.md".to_string(),
            content: "<!-- DELETED -->".to_string(),
            updated_at: None,
        };
        assert!(deleted_content.is_deleted());
        assert_eq!(deleted_content.get_actual_content(), "");
        
        // 带空格的删除标记
        let deleted_with_spaces = MemoryContent {
            path: "deleted2.md".to_string(),
            content: "  <!-- DELETED -->  ".to_string(),
            updated_at: None,
        };
        assert!(deleted_with_spaces.is_deleted());
        assert_eq!(deleted_with_spaces.get_actual_content(), "");
    }
}