//! 文件操作 Tauri Commands — Undo / Open File。
//!
//! `ic_undo_file_edit`: 撤回 code_edit 工具的文件修改。
//!   利用 code_edit 的参数（old_string / new_string / count）做反向替换。
//!
//! `ic_open_file_at_line`: 在用户的默认编辑器中打开文件（可选行号）。

use tauri::State;

use crate::state::EngineState;

/// 撤回一次 code_edit 操作。
///
/// 通过反向替换来实现：将 `new_string` 替换回 `old_string`。
/// 前端从 code_edit 工具的 args 中已持有这三个值。
///
/// 安全守则：
/// - 写入前验证 `new_string` 出现次数与 `count` 一致
/// - 文件路径由 ironclaw PathPolicy 在 code_edit 执行时已校验过；
///   这里只做存在性 & 可读性检查
#[tauri::command]
pub async fn ic_undo_file_edit(
    state: State<'_, EngineState>,
    path: String,
    old_string: String,
    new_string: String,
    count: usize,
) -> Result<String, String> {
    // 确保引擎就绪（统一的前置检查）
    let _state = state.get()?;

    let file_path = std::path::Path::new(&path);
    if !file_path.exists() {
        return Err(format!("文件不存在: {}", path));
    }

    let content = tokio::fs::read_to_string(file_path)
        .await
        .map_err(|e| format!("读取文件失败: {}", e))?;

    let actual_count = content.matches(&new_string).count();
    if actual_count < count {
        return Err(format!(
            "文件内容已变更，无法撤回（预期 {} 处 new_string，实际找到 {} 处）",
            count, actual_count
        ));
    }

    let restored = replace_first_n(&content, &new_string, &old_string, count);

    tokio::fs::write(file_path, &restored)
        .await
        .map_err(|e| format!("写入文件失败: {}", e))?;

    tracing::info!(path = %path, count = count, "File edit undone");
    Ok(format!("已撤回 {} 处替换", count))
}

/// 在默认编辑器中打开文件，可选跳转到指定行。
///
/// 优先尝试 `code` CLI（VS Code）; 若不可用则回退到系统默认 `open`。
#[tauri::command]
pub async fn ic_open_file_at_line(
    path: String,
    line: Option<u32>,
) -> Result<(), String> {
    let file_path = std::path::Path::new(&path);
    if !file_path.exists() {
        return Err(format!("文件不存在: {}", path));
    }

    // 优先尝试 VS Code CLI
    let vscode_arg = match line {
        Some(l) => format!("--goto {}:{}", path, l),
        None => path.clone(),
    };

    let vscode_result = tokio::process::Command::new("code")
        .args(vscode_arg.split_whitespace())
        .status()
        .await;

    match vscode_result {
        Ok(status) if status.success() => {
            tracing::debug!(path = %path, line = ?line, "Opened in VS Code");
            return Ok(());
        }
        _ => {
            // VS Code 不可用，回退到系统默认
            tracing::debug!("VS Code CLI not available, falling back to system open");
        }
    }

    // 回退: 使用 `open` (macOS) / `xdg-open` (Linux) / `start` (Windows)
    open::that(&path).map_err(|e| format!("无法打开文件: {}", e))?;
    Ok(())
}

/// 替换字符串中前 `n` 次出现的 `from` 为 `to`。
fn replace_first_n(text: &str, from: &str, to: &str, n: usize) -> String {
    if from.is_empty() || n == 0 {
        return text.to_string();
    }

    let mut result = String::with_capacity(text.len());
    let mut remaining = text;
    let mut replaced = 0;

    while replaced < n {
        match remaining.find(from) {
            Some(pos) => {
                result.push_str(&remaining[..pos]);
                result.push_str(to);
                remaining = &remaining[pos + from.len()..];
                replaced += 1;
            }
            None => break,
        }
    }
    result.push_str(remaining);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_first_n_single() {
        let input = "hello world hello";
        let result = replace_first_n(input, "hello", "bye", 1);
        assert_eq!(result, "bye world hello");
    }

    #[test]
    fn replace_first_n_multiple() {
        let input = "aaa bbb aaa bbb aaa";
        let result = replace_first_n(input, "aaa", "xxx", 2);
        assert_eq!(result, "xxx bbb xxx bbb aaa");
    }

    #[test]
    fn replace_first_n_zero_count() {
        let input = "hello world";
        let result = replace_first_n(input, "hello", "bye", 0);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn replace_first_n_not_found() {
        let input = "hello world";
        let result = replace_first_n(input, "xyz", "abc", 1);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn replace_first_n_empty_from() {
        let input = "hello";
        let result = replace_first_n(input, "", "x", 5);
        assert_eq!(result, "hello");
    }
}
