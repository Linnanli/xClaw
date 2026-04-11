use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

const DEFAULT_SCANNER_URL: &str = "http://localhost:8000";
const DEFAULT_SCANNER_TIMEOUT_MS: u64 = 30_000;

#[derive(Debug, Clone)]
pub struct ScannerConfig {
    pub enabled: bool,
    pub url: String,
    pub timeout_ms: u64,
}

impl ScannerConfig {
    pub fn from_env() -> Self {
        let enabled = std::env::var("SCANNER_ENABLED")
            .ok()
            .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);

        let url = std::env::var("SCANNER_URL").unwrap_or_else(|_| DEFAULT_SCANNER_URL.to_string());

        let timeout_ms = std::env::var("SCANNER_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_SCANNER_TIMEOUT_MS);

        Self {
            enabled,
            url,
            timeout_ms,
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

    pub async fn scan_upload(&self, name: &str, content: &str) -> Result<ScanResult, ScanError> {
        let url = format!("{}/scan-upload", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "name": name,
                "content": content,
            }))
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
        .and_then(Value::as_i64)
        .map(|v| v as i32);

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
        .or_else(|| findings.iter().map(|f| f.severity).max_by_key(severity_rank));

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