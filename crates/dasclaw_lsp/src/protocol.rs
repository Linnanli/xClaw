//! LSP-specific types and JSON-RPC message framing.
//!
//! Uses `lsp-types` for protocol structs, and adds a thin framing layer
//! (Content-Length header) for stdio communication.

use serde::{Deserialize, Serialize};

/// Actions the LLM can request via `LspQueryTool`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LspAction {
    /// Go to definition of the symbol at (line, col).
    GotoDefinition,
    /// Find all references to the symbol at (line, col).
    FindReferences,
    /// Retrieve diagnostics (errors/warnings) for a file.
    Diagnostics,
    /// Get hover information (type info, docs) at (line, col).
    Hover,
    /// List document symbols (functions, classes, etc.).
    DocumentSymbols,
    /// Rename the symbol at (line, col) to a new name.
    Rename,
    /// Get completions at (line, col).
    Completions,
}

impl LspAction {
    /// Whether this action needs a (line, col) position.
    pub fn requires_position(self) -> bool {
        !matches!(self, Self::Diagnostics | Self::DocumentSymbols)
    }
}

/// A JSON-RPC request for the LSP server.
#[derive(Debug, Clone, Serialize)]
pub struct LspRequest {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: String,
    pub params: serde_json::Value,
}

impl LspRequest {
    pub fn new(id: u64, method: impl Into<String>, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: method.into(),
            params,
        }
    }

    /// Encode as LSP wire format: `Content-Length: N\r\n\r\n{json}`.
    pub fn encode(&self) -> Result<Vec<u8>, serde_json::Error> {
        let body = serde_json::to_string(self)?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        let mut buf = Vec::with_capacity(header.len() + body.len());
        buf.extend_from_slice(header.as_bytes());
        buf.extend_from_slice(body.as_bytes());
        Ok(buf)
    }
}

/// A JSON-RPC notification (no id, no response expected).
#[derive(Debug, Clone, Serialize)]
pub struct LspNotification {
    pub jsonrpc: &'static str,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl LspNotification {
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            method: method.into(),
            params,
        }
    }

    /// Encode as LSP wire format.
    pub fn encode(&self) -> Result<Vec<u8>, serde_json::Error> {
        let body = serde_json::to_string(self)?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        let mut buf = Vec::with_capacity(header.len() + body.len());
        buf.extend_from_slice(header.as_bytes());
        buf.extend_from_slice(body.as_bytes());
        Ok(buf)
    }
}

/// A JSON-RPC response from the LSP server.
#[derive(Debug, Clone, Deserialize)]
pub struct LspResponse {
    pub id: Option<u64>,
    pub result: Option<serde_json::Value>,
    pub error: Option<LspError>,
}

/// JSON-RPC error payload.
#[derive(Debug, Clone, Deserialize)]
pub struct LspError {
    pub code: i64,
    pub message: String,
}

impl std::fmt::Display for LspError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LSP error {}: {}", self.code, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsp_action_requires_position() {
        assert!(LspAction::GotoDefinition.requires_position());
        assert!(LspAction::FindReferences.requires_position());
        assert!(LspAction::Hover.requires_position());
        assert!(LspAction::Rename.requires_position());
        assert!(LspAction::Completions.requires_position());
        assert!(!LspAction::Diagnostics.requires_position());
        assert!(!LspAction::DocumentSymbols.requires_position());
    }

    #[test]
    fn test_request_encode_has_content_length() {
        let req = LspRequest::new(1, "textDocument/hover", serde_json::json!({}));
        let encoded = req.encode().expect("encode failed");
        let s = String::from_utf8(encoded).expect("utf8");
        assert!(s.starts_with("Content-Length:"));
        assert!(s.contains("\r\n\r\n"));
    }

    #[test]
    fn test_notification_encode() {
        let notif = LspNotification::new("initialized", None);
        let encoded = notif.encode().expect("encode failed");
        let s = String::from_utf8(encoded).expect("utf8");
        assert!(s.contains("initialized"));
        assert!(!s.contains("\"id\""));
    }

    #[test]
    fn test_response_deserialize_success() {
        let json = r#"{"id":1,"result":{"contents":"hello"}}"#;
        let resp: LspResponse = serde_json::from_str(json).expect("deser");
        assert_eq!(resp.id, Some(1));
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_response_deserialize_error() {
        let json = r#"{"id":1,"error":{"code":-32600,"message":"Invalid Request"}}"#;
        let resp: LspResponse = serde_json::from_str(json).expect("deser");
        assert!(resp.error.is_some());
        let err = resp.error.as_ref().expect("err");
        assert_eq!(err.code, -32600);
    }

    #[test]
    fn test_lsp_action_serde_roundtrip() {
        let action = LspAction::GotoDefinition;
        let s = serde_json::to_string(&action).expect("ser");
        assert_eq!(s, "\"goto_definition\"");
        let back: LspAction = serde_json::from_str(&s).expect("deser");
        assert_eq!(back, action);
    }
}
