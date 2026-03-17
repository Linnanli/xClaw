//! 记忆删除保护测试
//! 
//! 测试轻量级的记忆文件删除保护功能和API扩展

use desktop_client::memory_manager::{is_protected_file, MemoryContentExtensions};
use desktop_client::api_client::MemoryContent;

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
    assert!(!is_protected_file("HEARTBEAT.md"));
    assert!(!is_protected_file("README.md"));
    assert!(!is_protected_file("BOOTSTRAP.md"));
    assert!(!is_protected_file("my_notes.md"));
    assert!(!is_protected_file("projects/readme.md"));
}

#[test]
fn test_protection_with_paths() {
    let test_cases = vec![
        ("SOUL.md", true),
        ("IDENTITY.md", true),
        ("AGENTS.md", true),
        ("context/SOUL.md", true),
        ("backup/IDENTITY.md", true),
        ("docs/AGENTS.md", true),
        ("USER.md", false),
        ("HEARTBEAT.md", false),
        ("README.md", false),
        ("BOOTSTRAP.md", false),
        ("MEMORY.md", false),
        ("my_notes.md", false),
        ("projects/doc.md", false),
        ("daily/2024-01-01.md", false),
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
    
    // 包含删除标记但不完全匹配的内容
    let partial_match = MemoryContent {
        path: "partial.md".to_string(),
        content: "Some content <!-- DELETED --> more content".to_string(),
        updated_at: None,
    };
    assert!(!partial_match.is_deleted());
    assert_eq!(partial_match.get_actual_content(), "Some content <!-- DELETED --> more content");
}

#[test]
fn test_edge_cases() {
    // 边界情况
    assert!(!is_protected_file(""));
    assert!(!is_protected_file(" "));
    assert!(!is_protected_file("."));
    assert!(!is_protected_file(".."));
    assert!(!is_protected_file("/"));
    assert!(!is_protected_file("\\"));
    
    // 大小写敏感
    assert!(!is_protected_file("SOUL.MD")); // 大写扩展名
    assert!(!is_protected_file("soul.md")); // 小写文件名
    assert!(!is_protected_file("Soul.md")); // 混合大小写
    
    // 带额外扩展名
    assert!(!is_protected_file("SOUL.md.backup"));
    assert!(!is_protected_file("IDENTITY.md.old"));
    
    // 隐藏文件
    assert!(!is_protected_file(".SOUL.md"));
    assert!(!is_protected_file(".IDENTITY.md"));
}

#[test]
fn test_security_malicious_paths() {
    let malicious_paths = vec![
        "../../../etc/passwd",
        "..\\..\\windows\\system32\\config\\sam",
        "/etc/shadow",
        "C:\\Windows\\System32\\config\\SAM",
        "file:///etc/passwd",
        "\\\\server\\share\\file.txt",
        "../../SOUL.md", // 路径遍历攻击
        "../IDENTITY.md",
    ];
    
    for malicious_path in malicious_paths {
        // 恶意路径中的受保护文件名不应该被识别为受保护
        // 除非它们是直接的文件名匹配
        let is_protected = is_protected_file(malicious_path);
        
        // 只有当路径的最后部分确实是受保护文件名时才应该返回true
        let expected = malicious_path.ends_with("SOUL.md") || 
                      malicious_path.ends_with("IDENTITY.md") || 
                      malicious_path.ends_with("AGENTS.md");
        
        assert_eq!(is_protected, expected, 
                  "恶意路径 {} 的保护检查结果不符合预期", malicious_path);
    }
}

#[test]
fn test_api_extension_trait() {
    // 测试扩展trait的存在性（编译时检查）
    use desktop_client::memory_manager::MemoryApiExtensions;
    use desktop_client::api_client::ApiClient;
    
    // 这个测试主要是确保trait正确实现
    // 创建一个ApiClient实例来验证trait方法可用
    let client = ApiClient::new("http://localhost:3000".to_string());
    
    // 验证trait已正确实现（编译时检查）
    // 如果trait没有正确实现，这里会编译失败
    let _has_method = client.delete_memory_safe("test.md", false);
    
    // 测试通过意味着trait扩展成功
    assert!(true, "MemoryApiExtensions trait is properly implemented");
}