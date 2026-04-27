use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;
use thiserror::Error;

const DEFAULT_SCANNER_URL: &str = "http://localhost:8000";
const DEFAULT_SCANNER_TIMEOUT_MS: u64 = 30_000;

fn parse_bool_env(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(default)
}

#[derive(Debug, Clone)]
pub struct ScannerConfig {
    pub enabled: bool,
    pub url: String,
    pub timeout_ms: u64,
    pub use_llm: bool,
    pub llm_provider: String,
    pub llm_api_key: Option<String>,
    pub llm_model: Option<String>,
    pub llm_base_url: Option<String>,
    pub llm_api_version: Option<String>,
}

impl ScannerConfig {
    pub fn from_env() -> Self {
        let enabled = parse_bool_env("SCANNER_ENABLED", false);

        let url = std::env::var("SCANNER_URL").unwrap_or_else(|_| DEFAULT_SCANNER_URL.to_string());

        let timeout_ms = std::env::var("SCANNER_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_SCANNER_TIMEOUT_MS);

        let use_llm = parse_bool_env("SCANNER_USE_LLM", true);
        let llm_provider = std::env::var("SCANNER_LLM_PROVIDER")
            .unwrap_or_else(|_| "anthropic".to_string())
            .trim()
            .to_lowercase();
        let llm_api_key = std::env::var("SCANNER_LLM_API_KEY")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        let llm_model = std::env::var("SCANNER_LLM_MODEL")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        let llm_base_url = std::env::var("SCANNER_LLM_BASE_URL")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        let llm_api_version = std::env::var("SCANNER_LLM_API_VERSION")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());

        Self {
            enabled,
            url,
            timeout_ms,
            use_llm,
            llm_provider,
            llm_api_key,
            llm_model,
            llm_base_url,
            llm_api_version,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScanUploadOptions {
    pub use_llm: bool,
    pub llm_provider: String,
    pub llm_api_key: Option<String>,
    pub llm_model: Option<String>,
    pub llm_base_url: Option<String>,
    pub llm_api_version: Option<String>,
}

impl ScanUploadOptions {
    pub fn from_config(config: &ScannerConfig) -> Self {
        Self {
            use_llm: config.use_llm,
            llm_provider: config.llm_provider.clone(),
            llm_api_key: config.llm_api_key.clone(),
            llm_model: config.llm_model.clone(),
            llm_base_url: config.llm_base_url.clone(),
            llm_api_version: config.llm_api_version.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SecurityVerdict {
    Safe,
    Suspicious,
    Dangerous,
    Blocked,
    Unknown,
}

impl SecurityVerdict {
    fn from_raw(raw: &str) -> Self {
        match raw.to_uppercase().as_str() {
            "SAFE" => Self::Safe,
            "SUSPICIOUS" => Self::Suspicious,
            "DANGEROUS" => Self::Dangerous,
            "BLOCKED" => Self::Blocked,
            _ => Self::Unknown,
        }
    }

    fn is_safe(self) -> bool {
        matches!(self, SecurityVerdict::Safe)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FindingSeverity {
    Low,
    Medium,
    High,
    Critical,
    Unknown,
}

impl FindingSeverity {
    fn from_raw(raw: &str) -> Self {
        match raw.to_uppercase().as_str() {
            "LOW" => Self::Low,
            "MEDIUM" => Self::Medium,
            "HIGH" => Self::High,
            "CRITICAL" => Self::Critical,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityFinding {
    pub rule_id: Option<String>,
    pub severity: FindingSeverity,
    pub title: Option<String>,
    pub file: Option<String>,
    pub snippet: Option<String>,
    pub recommendation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub scanner_type: String,
    pub verdict: SecurityVerdict,
    pub is_safe: bool,
    pub max_severity: Option<FindingSeverity>,
    pub findings_count: i32,
    pub findings: Vec<SecurityFinding>,
    pub scan_duration_ms: Option<i32>,
}

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("scanner request failed: {0}")]
    Request(String),
    #[error("scanner returned non-success status {status}: {body}")]
    HttpStatus { status: u16, body: String },
    #[error("scanner response parse failed: {0}")]
    Parse(String),
}

#[derive(Debug, Clone)]
pub struct SkillScanner {
    client: reqwest::Client,
    base_url: String,
}

impl SkillScanner {
    pub fn new(base_url: &str, timeout_ms: u64) -> Result<Self, ScanError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(timeout_ms))
            .build()
            .map_err(|e| ScanError::Request(e.to_string()))?;

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    pub async fn scan_upload(
        &self,
        name: &str,
        content: &str,
        options: &ScanUploadOptions,
    ) -> Result<ScanResult, ScanError> {
        let url = format!("{}/scan-upload", self.base_url);
        let zip_bytes = build_scan_upload_archive(content)?;
        let file_name = format!("{}.zip", sanitize_scan_upload_name(name));
        let file_part = reqwest::multipart::Part::bytes(zip_bytes)
            .file_name(file_name)
            .mime_str("application/zip")
            .map_err(|e| ScanError::Request(e.to_string()))?;
        let form = reqwest::multipart::Form::new()
            .part("file", file_part)
            .text("use_llm", options.use_llm.to_string())
            .text("llm_provider", options.llm_provider.clone());

        let mut request = self.client.post(&url).multipart(form);
        if let Some(llm_api_key) = options
            .llm_api_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            request = request.header("X-LLM-Key", llm_api_key);
        }
        if let Some(llm_model) = options
            .llm_model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            request = request.header("X-LLM-Model", llm_model);
        }
        if let Some(llm_base_url) = options
            .llm_base_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            request = request.header("X-LLM-Base-URL", llm_base_url);
        }
        if let Some(llm_api_version) = options
            .llm_api_version
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            request = request.header("X-LLM-API-Version", llm_api_version);
        }

        let response = request
            .send()
            .await
            .map_err(|e| ScanError::Request(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ScanError::HttpStatus {
                status: status.as_u16(),
                body,
            });
        }

        let payload: Value = response
            .json()
            .await
            .map_err(|e| ScanError::Parse(e.to_string()))?;

        parse_scan_result(payload)
    }

    pub async fn check_health(&self) -> Result<(), ScanError> {
        let url = format!("{}/health", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ScanError::Request(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ScanError::HttpStatus {
                status: status.as_u16(),
                body,
            });
        }

        Ok(())
    }
}

fn sanitize_scan_upload_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "skill".to_string();
    }

    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }

    if out.is_empty() {
        return "skill".to_string();
    }
    out
}

fn build_scan_upload_archive(content: &str) -> Result<Vec<u8>, ScanError> {
    let mut zip_bytes = std::io::Cursor::new(Vec::<u8>::new());
    {
        let mut zip_writer = zip::ZipWriter::new(&mut zip_bytes);
        let options = zip::write::SimpleFileOptions::default();
        zip_writer
            .start_file("pkg/SKILL.md", options)
            .map_err(|e| ScanError::Request(e.to_string()))?;
        zip_writer
            .write_all(content.as_bytes())
            .map_err(|e| ScanError::Request(e.to_string()))?;
        zip_writer
            .finish()
            .map_err(|e| ScanError::Request(e.to_string()))?;
    }

    Ok(zip_bytes.into_inner())
}

fn parse_scan_result(payload: Value) -> Result<ScanResult, ScanError> {
    let verdict_raw = payload
        .get("verdict")
        .and_then(Value::as_str)
        .unwrap_or("UNKNOWN");
    let verdict = SecurityVerdict::from_raw(verdict_raw);

    let scanner_type = payload
        .get("scanner_type")
        .and_then(Value::as_str)
        .unwrap_or("cisco-ai-skill-scanner")
        .to_string();

    let scan_duration_ms = payload
        .get("scan_duration_ms")
        .or_else(|| payload.get("duration_ms"))
        .or_else(|| payload.get("scan_duration_seconds"))
        .and_then(Value::as_i64)
        .map(|v| {
            if payload.get("scan_duration_seconds").is_some() {
                (v * 1000) as i32
            } else {
                v as i32
            }
        })
        .or_else(|| {
            payload
                .get("scan_duration_seconds")
                .and_then(Value::as_f64)
                .map(|v| (v * 1000.0).round() as i32)
        });

    let findings = payload
        .get("findings")
        .and_then(Value::as_array)
        .map(|items| items.iter().map(parse_finding).collect::<Vec<_>>())
        .unwrap_or_default();

    let findings_count = payload
        .get("findings_count")
        .and_then(Value::as_i64)
        .map(|v| v as i32)
        .unwrap_or(findings.len() as i32);

    let max_severity = payload
        .get("max_severity")
        .and_then(Value::as_str)
        .map(FindingSeverity::from_raw)
        .or_else(|| {
            findings
                .iter()
                .map(|f| f.severity)
                .max_by_key(severity_rank)
        });

    let is_safe = payload
        .get("is_safe")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| verdict.is_safe());

    Ok(ScanResult {
        scanner_type,
        verdict,
        is_safe,
        max_severity,
        findings_count,
        findings,
        scan_duration_ms,
    })
}

fn parse_finding(item: &Value) -> SecurityFinding {
    let severity = item
        .get("severity")
        .and_then(Value::as_str)
        .map(FindingSeverity::from_raw)
        .unwrap_or(FindingSeverity::Unknown);

    SecurityFinding {
        rule_id: item
            .get("rule_id")
            .or_else(|| item.get("id"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        severity,
        title: item
            .get("title")
            .or_else(|| item.get("message"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        file: item
            .get("file")
            .or_else(|| item.get("path"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        snippet: item
            .get("snippet")
            .or_else(|| item.get("code"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        recommendation: item
            .get("recommendation")
            .or_else(|| item.get("fix"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
    }
}

fn severity_rank(severity: &FindingSeverity) -> i32 {
    match severity {
        FindingSeverity::Unknown => 0,
        FindingSeverity::Low => 1,
        FindingSeverity::Medium => 2,
        FindingSeverity::High => 3,
        FindingSeverity::Critical => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_scan_result, FindingSeverity, SecurityVerdict};

    #[test]
    fn parse_minimal_scan_result() {
        let payload = serde_json::json!({
            "verdict": "SAFE",
            "findings": [],
            "duration_ms": 12
        });

        let result = parse_scan_result(payload).expect("parse should succeed");
        assert_eq!(result.verdict, SecurityVerdict::Safe);
        assert!(result.is_safe);
        assert_eq!(result.findings_count, 0);
        assert_eq!(result.scan_duration_ms, Some(12));
    }

    #[test]
    fn parse_finding_and_max_severity() {
        let payload = serde_json::json!({
            "verdict": "SUSPICIOUS",
            "findings": [
                {"rule_id": "R-1", "severity": "LOW", "title": "x"},
                {"rule_id": "R-2", "severity": "HIGH", "title": "y"}
            ]
        });

        let result = parse_scan_result(payload).expect("parse should succeed");
        assert_eq!(result.verdict, SecurityVerdict::Suspicious);
        assert_eq!(result.findings_count, 2);
        assert_eq!(result.max_severity, Some(FindingSeverity::High));
    }
}
