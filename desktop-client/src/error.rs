use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug, Serialize)]
#[serde(tag = "type", content = "message")]
pub enum Error {
    #[error("Authentication error: {0}")]
    AuthError(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Token error: {0}")]
    TokenError(String),

    #[error("Config error: {0}")]
    ConfigError(String),

    #[error("DLP error: {0}")]
    DlpError(String),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::IoError(err.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::SerializationError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// 将 ironclaw 引擎抛出的原始错误字符串转换为用户友好的中文提示。
///
/// ironclaw 的错误格式为（通过 thiserror Display 链式拼接）：
/// `"LLM error: Provider {provider} request failed: {reason}"`
///
/// 其中 reason 来自底层 HTTP 客户端，格式为：
/// `"HttpError: Invalid status code {code} {text} with message: {json_body}"`
///
/// 此函数属于 desktop-client 层，不修改 ironclaw 本身。
pub fn friendly_engine_error(raw: &str) -> String {
    if let Some(msg) = try_friendly_llm_error(raw) {
        return msg;
    }
    "AI 服务出现错误，请稍后重试".to_string()
}

/// 识别 LLM 相关错误并返回友好消息，无法识别时返回 None。
fn try_friendly_llm_error(raw: &str) -> Option<String> {
    // 格式：`LLM error: Provider {provider} request failed: {reason}`
    let after_llm = raw.strip_prefix("LLM error: Provider ")?;
    let (provider, reason) = after_llm.split_once(" request failed: ")?;

    // 从 reason 中提取 HTTP 状态码。
    // 底层客户端（rig/reqwest）的格式为：
    //   `HttpError: Invalid status code {code} {text} with message: {body}`
    // 也兼容旧格式：`HTTP {code}: {body}`
    let status_code = extract_http_status(reason)?;
    let msg = match status_code {
        403 => {
            let error_type = extract_json_error_type(reason);
            match error_type.as_deref() {
                Some("AllocationQuota.FreeTierOnly") => format!(
                    "模型 {provider} 免费额度已用完，请前往控制台关闭「仅使用免费额度」选项"
                ),
                _ => format!("模型 {provider} 访问被拒绝（403），请检查 API Key 权限"),
            }
        }
        401 => format!("模型 {provider} 认证失败，请检查 API Key 是否正确"),
        429 => format!("模型 {provider} 请求频率超限，请稍后重试"),
        500 | 502 | 503 => format!("模型 {provider} 服务暂时不可用，请稍后重试"),
        code => format!("模型 {provider} 请求失败（HTTP {code}）"),
    };

    Some(msg)
}

/// 从 reason 字符串中提取 HTTP 状态码。
///
/// 支持两种格式：
/// - `HttpError: Invalid status code 403 Forbidden with message: ...`（rig/reqwest）
/// - `HTTP 403: ...`（旧格式，保留兼容）
fn extract_http_status(reason: &str) -> Option<u16> {
    // 格式 1：`HttpError: Invalid status code {code} ...`
    if let Some(after) = reason.strip_prefix("HttpError: Invalid status code ") {
        return after.split_whitespace().next()?.parse().ok();
    }
    // 格式 2：`HTTP {code}: ...`
    if let Some(after) = reason.strip_prefix("HTTP ") {
        return after.split(':').next()?.trim().parse().ok();
    }
    None
}

/// 从错误字符串中提取 JSON 响应体里的 `error.type` 字段。
fn extract_json_error_type(reason: &str) -> Option<String> {
    let json_start = reason.find('{')?;
    let value: serde_json::Value = serde_json::from_str(&reason[json_start..]).ok()?;
    value
        .get("error")?
        .get("type")?
        .as_str()
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 契约测试：用真实错误格式固定解析行为 ──────────────────────

    /// 真实错误格式（来自用户报告）：
    /// `LLM error: Provider qwen-max request failed: HttpError: Invalid status code 403 Forbidden with message: {...}`
    #[test]
    fn test_contract_real_free_tier_exhausted_format() {
        let raw = concat!(
            "LLM error: Provider qwen-max request failed: ",
            "HttpError: Invalid status code 403 Forbidden with message: ",
            r#"{"error":{"message":"The free tier of the model has been exhausted. If you wish to continue access the model on a paid basis, please disable the \"use free tier only\" mode in the management console.","type":"AllocationQuota.FreeTierOnly","param":null,"code":"AllocationQuota.FreeTierOnly"},"id":"chatcmpl-e4757e7e","request_id":"e4757e7e"}"#
        );
        let msg = friendly_engine_error(raw);
        assert!(msg.contains("免费额度已用完"), "应提示免费额度: {msg}");
        assert!(msg.contains("qwen-max"), "应包含 provider: {msg}");
        assert!(msg.contains("控制台"), "应提示操作路径: {msg}");
        // 不应暴露原始技术细节
        assert!(!msg.contains("HttpError"), "不应暴露 HttpError: {msg}");
        assert!(!msg.contains("AllocationQuota"), "不应暴露错误码: {msg}");
    }

    #[test]
    fn test_contract_real_generic_403_format() {
        let raw = "LLM error: Provider openai request failed: HttpError: Invalid status code 403 Forbidden with message: Forbidden";
        let msg = friendly_engine_error(raw);
        assert!(msg.contains("403"), "应包含状态码: {msg}");
        assert!(msg.contains("openai"), "应包含 provider: {msg}");
    }

    #[test]
    fn test_contract_real_401_format() {
        let raw = "LLM error: Provider anthropic request failed: HttpError: Invalid status code 401 Unauthorized with message: Unauthorized";
        let msg = friendly_engine_error(raw);
        assert!(msg.contains("认证失败"), "应提示认证失败: {msg}");
        assert!(msg.contains("API Key"), "应提示检查 API Key: {msg}");
    }

    #[test]
    fn test_contract_real_429_format() {
        let raw = "LLM error: Provider qwen-max request failed: HttpError: Invalid status code 429 Too Many Requests with message: Rate limited";
        let msg = friendly_engine_error(raw);
        assert!(msg.contains("频率超限"), "应提示频率限制: {msg}");
    }

    #[test]
    fn test_contract_real_503_format() {
        let raw = "LLM error: Provider openai request failed: HttpError: Invalid status code 503 Service Unavailable with message: Service Unavailable";
        let msg = friendly_engine_error(raw);
        assert!(msg.contains("暂时不可用"), "应提示服务不可用: {msg}");
    }

    // ── 兼容性测试：旧格式 HTTP {code}: 也能正确解析 ──────────────

    #[test]
    fn test_compat_old_http_format_403() {
        let raw = r#"LLM error: Provider qwen-max request failed: HTTP 403: {"error":{"type":"AllocationQuota.FreeTierOnly"}}"#;
        let msg = friendly_engine_error(raw);
        assert!(msg.contains("免费额度已用完"), "旧格式也应识别: {msg}");
    }

    // ── 失败路径测试 ───────────────────────────────────────────────

    #[test]
    fn test_failure_unrecognized_error_returns_generic() {
        let raw = "some internal panic in the engine";
        let msg = friendly_engine_error(raw);
        assert!(msg.contains("稍后重试"), "未知错误应返回通用提示: {msg}");
        assert!(!msg.contains("panic"), "不应暴露技术细节: {msg}");
    }

    #[test]
    fn test_failure_empty_string() {
        let msg = friendly_engine_error("");
        assert!(!msg.is_empty(), "空字符串不应返回空消息");
        assert!(msg.contains("稍后重试"), "空字符串应返回通用提示: {msg}");
    }

    // ── extract_http_status 单元测试 ───────────────────────────────

    #[test]
    fn test_extract_http_status_new_format() {
        assert_eq!(
            extract_http_status("HttpError: Invalid status code 403 Forbidden with message: ..."),
            Some(403)
        );
        assert_eq!(
            extract_http_status(
                "HttpError: Invalid status code 429 Too Many Requests with message: ..."
            ),
            Some(429)
        );
    }

    #[test]
    fn test_extract_http_status_old_format() {
        assert_eq!(extract_http_status("HTTP 403: body"), Some(403));
        assert_eq!(extract_http_status("HTTP 401: Unauthorized"), Some(401));
    }

    #[test]
    fn test_extract_http_status_no_match() {
        assert_eq!(extract_http_status("connection refused"), None);
        assert_eq!(extract_http_status(""), None);
    }
}
