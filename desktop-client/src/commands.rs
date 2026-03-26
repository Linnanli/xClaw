//! Tauri 命令 — 仅包含需要在 IPC 层直接暴露的命令。
//!
//! 大部分命令已迁移到 `ipc/` 模块，此文件仅保留 `get_auth_token`。

use crate::auth_token_manager::AuthTokenManager;
use crate::{Error, Result};

/// 获取认证令牌。
///
/// 从本地文件加载或自动生成 64 位十六进制 token，
/// 并验证格式合法性后返回给前端。
#[tauri::command]
pub async fn get_auth_token() -> Result<String> {
    let token_manager = AuthTokenManager::new();
    let token = token_manager
        .load_or_generate()
        .map_err(|e| Error::ConfigError(e.to_string()))?;

    if token.len() != 64 {
        tracing::error!("Token length is {} (expected 64)", token.len());
        return Err(Error::ConfigError(format!(
            "Invalid token length: {}",
            token.len()
        )));
    }

    if !token.chars().all(|c| c.is_ascii_hexdigit()) {
        tracing::error!("Token contains non-hex characters");
        return Err(Error::ConfigError(
            "Token contains non-hex characters".to_string(),
        ));
    }

    Ok(token)
}
