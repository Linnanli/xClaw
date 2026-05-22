//! Channel-related errors.
//!
//! F4.5 verbatim port from `desktop-client/ironclaw/src/error.rs` (ADR-129 §1.3).

/// Channel-related errors.
#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("Channel {name} failed to start: {reason}")]
    StartupFailed { name: String, reason: String },

    #[error("Channel {name} disconnected: {reason}")]
    Disconnected { name: String, reason: String },

    #[error("Failed to send response on channel {name}: {reason}")]
    SendFailed { name: String, reason: String },

    #[error("Channel {name} is missing a routing target: {reason}")]
    MissingRoutingTarget { name: String, reason: String },

    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    #[error("Authentication failed for channel {name}: {reason}")]
    AuthFailed { name: String, reason: String },

    #[error("Rate limited on channel {name}")]
    RateLimited { name: String },

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Channel health check failed: {name}")]
    HealthCheckFailed { name: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_error_display() {
        let err = ChannelError::StartupFailed {
            name: "telegram".to_string(),
            reason: "invalid token".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("telegram"), "Should mention channel: {msg}");
        assert!(
            msg.contains("invalid token"),
            "Should mention reason: {msg}"
        );
    }
}
