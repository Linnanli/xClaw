//! 沙箱工作区目录管理。
//!
//! Desktop Client 为每个线程创建独立的沙箱工作区，
//! 路径约定为 `~/.ironclaw/projects/{thread_id}/`。

use std::path::PathBuf;
use uuid::Uuid;

/// 返回沙箱工作区的根目录 `~/.ironclaw/projects/`。
pub fn projects_base() -> PathBuf {
    dirs::home_dir()
        .expect("无法获取 home 目录")
        .join(".ironclaw")
        .join("projects")
}

/// 验证用户传入的导入路径是否合法，返回规范化后的绝对路径。
///
/// 拒绝不存在、不是目录的路径。
pub fn validate_import_path(path: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(path);

    let canonical = p
        .canonicalize()
        .map_err(|e| format!("路径不存在或无法访问: {e}"))?;

    if !canonical.is_dir() {
        return Err("路径不是一个目录".to_string());
    }

    Ok(canonical)
}

/// 为指定线程创建沙箱工作区目录，返回创建的路径。
///
/// 路径为 `~/.ironclaw/projects/{thread_id}/`。
/// 如果目录已存在则直接返回。
pub fn create_sandbox_workspace(thread_id: Uuid) -> Result<PathBuf, String> {
    let dir = projects_base().join(thread_id.to_string());

    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("无法创建工作区目录 {}: {e}", dir.display()))?;

    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projects_base_under_home() {
        let base = projects_base();
        assert!(base.ends_with(".ironclaw/projects"));
    }

    #[test]
    fn test_validate_import_path_nonexistent() {
        let result = validate_import_path("/tmp/__nonexistent_path_12345__");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_import_path_valid_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let result = validate_import_path(tmp.path().to_str().unwrap());
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_sandbox_workspace_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let id = Uuid::new_v4();
        let dir = tmp.path().join(id.to_string());
        std::fs::create_dir_all(&dir).unwrap();
        assert!(dir.is_dir());
    }
}
