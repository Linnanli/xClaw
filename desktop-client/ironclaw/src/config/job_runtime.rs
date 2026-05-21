//! Job runtime mode configuration.
//!
//! Per [ADR-119](../../../../docs/plans/architecture-refactor/adr-119-job-runtime-decision-for-desktop-client.md),
//! desktop clients pick exactly one of:
//!
//! - [`JobRuntimeMode::Disabled`] — enterprise default; job tools are not
//!   exposed and no Docker probe is performed at startup.
//! - [`JobRuntimeMode::LocalContainer`] — the existing path that delegates
//!   `create_job` to a local Docker / OrbStack / Colima / Rancher / Podman
//!   socket via `bollard`.
//! - [`JobRuntimeMode::Cloud`] — reserved enum slot for a future cloud Job
//!   service; not implemented in the current code.
//!
//! # Backward compatibility
//!
//! The legacy `[sandbox] enabled = true` boolean is still honored for
//! installs whose config predates ADR-119: when no explicit
//! `[job_runtime]` block is present we derive a mode from the legacy bool
//! (`true` → `LocalContainer`, `false` → `Disabled`) and emit a single
//! deprecation warning. F2/F3/F4 (the orchestrator, registry, and audit
//! follow-ups in ADR-119 §5) will eventually retire the legacy bool.

use std::sync::Once;

use serde::{Deserialize, Serialize};

use crate::error::ConfigError;
use crate::settings::Settings;

/// Job runtime mode for the desktop client.
///
/// Tagged enum on the wire so a future `Cloud` payload can grow without
/// breaking existing TOML files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum JobRuntimeMode {
    /// Job tooling is unavailable. Enterprise default.
    ///
    /// `create_job` and the related discovery tools (`list_jobs`,
    /// `job_status`, `cancel_job`, `job_events`, `job_prompt`) MUST NOT
    /// be exposed to the LLM in this mode (enforced by F3, ADR-119 §5).
    #[default]
    Disabled,
    /// Run jobs in a local container via Docker or a Docker-API-compatible
    /// runtime (Docker Desktop, OrbStack, Colima, Rancher Desktop, Podman
    /// rootless). Requires the daemon to be reachable at boot; no silent
    /// fallback to an in-process scheduler (enforced by F2, ADR-119 §5).
    LocalContainer,
    /// Route jobs to a managed cloud endpoint. Reserved enum slot — the
    /// current code does not implement dispatch and rejecting Cloud at
    /// resolution is intentional until a follow-up ADR fills in auth,
    /// offline behaviour, and tenant isolation.
    Cloud {
        /// Endpoint URL of the managed Job service.
        endpoint: String,
    },
}

impl JobRuntimeMode {
    /// Lower-case identifier suitable for log fields and the eventual audit
    /// `runtime_mode` payload (ADR-119 D5).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::LocalContainer => "local_container",
            Self::Cloud { .. } => "cloud",
        }
    }
}

/// Resolved job runtime configuration.
///
/// Wraps [`JobRuntimeMode`] in a struct so future per-mode tunables can
/// land without churning every call site that reads `config.job_runtime`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JobRuntimeConfig {
    /// Selected runtime mode.
    pub mode: JobRuntimeMode,
}

/// Wire-format settings block for `[job_runtime]` in TOML / DB.
///
/// Optional in [`Settings`]; absence triggers the legacy fallback in
/// [`JobRuntimeConfig::resolve`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobRuntimeSettings {
    /// Selected runtime mode. See [`JobRuntimeMode`] for the wire shape.
    #[serde(flatten)]
    pub mode: JobRuntimeMode,
}

static WARNED_LEGACY_SANDBOX_DERIVE: Once = Once::new();

impl JobRuntimeConfig {
    /// Resolve job runtime configuration from settings.
    ///
    /// Priority (highest first):
    /// 1. `JOB_RUNTIME_MODE` env var (`disabled`, `local_container`,
    ///    `cloud:<endpoint>`).
    /// 2. Explicit `[job_runtime]` block in TOML / DB settings.
    /// 3. Derived from legacy `[sandbox] enabled` for backward compatibility
    ///    (`true` → `LocalContainer`, `false` → `Disabled`); emits a single
    ///    deprecation warning when the derivation actually fires.
    pub(crate) fn resolve(settings: &Settings) -> Result<Self, ConfigError> {
        if let Some(mode) = parse_env_override()? {
            return Ok(Self { mode });
        }

        if let Some(explicit) = settings.job_runtime.as_ref() {
            if let JobRuntimeMode::Cloud { endpoint } = &explicit.mode
                && endpoint.trim().is_empty()
            {
                return Err(ConfigError::InvalidValue {
                    key: "job_runtime.endpoint".to_string(),
                    message: "Cloud mode requires a non-empty endpoint".to_string(),
                });
            }
            return Ok(Self {
                mode: explicit.mode.clone(),
            });
        }

        let mode = if settings.sandbox.enabled {
            WARNED_LEGACY_SANDBOX_DERIVE.call_once(|| {
                tracing::warn!(
                    "[sandbox] enabled = true is deprecated; set [job_runtime] mode = \
                     \"local_container\" instead. See ADR-119 §5 F1 (xClaw#167)."
                );
            });
            JobRuntimeMode::LocalContainer
        } else {
            JobRuntimeMode::Disabled
        };

        Ok(Self { mode })
    }
}

/// Parse the `JOB_RUNTIME_MODE` env var into a [`JobRuntimeMode`].
///
/// Accepts:
/// - `disabled`
/// - `local_container`
/// - `cloud:<endpoint>` (URL after the colon, trimmed)
fn parse_env_override() -> Result<Option<JobRuntimeMode>, ConfigError> {
    let raw = match std::env::var("JOB_RUNTIME_MODE") {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "disabled" => Ok(Some(JobRuntimeMode::Disabled)),
        "local_container" => Ok(Some(JobRuntimeMode::LocalContainer)),
        s if s.starts_with("cloud:") => {
            let endpoint = trimmed["cloud:".len()..].trim().to_string();
            if endpoint.is_empty() {
                return Err(ConfigError::InvalidValue {
                    key: "JOB_RUNTIME_MODE".to_string(),
                    message: "cloud mode requires an endpoint after the colon".to_string(),
                });
            }
            Ok(Some(JobRuntimeMode::Cloud { endpoint }))
        }
        other => Err(ConfigError::InvalidValue {
            key: "JOB_RUNTIME_MODE".to_string(),
            message: format!(
                "expected one of 'disabled', 'local_container', 'cloud:<endpoint>'; got '{other}'"
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::helpers::lock_env;
    use crate::settings::Settings;

    fn settings_with_sandbox(enabled: bool) -> Settings {
        let mut s = Settings::default();
        s.sandbox.enabled = enabled;
        s.job_runtime = None;
        s
    }

    fn clear_env() {
        // SAFETY: callers hold lock_env() guard.
        unsafe {
            std::env::remove_var("JOB_RUNTIME_MODE");
        }
    }

    #[test]
    fn default_mode_is_disabled() {
        assert_eq!(JobRuntimeMode::default(), JobRuntimeMode::Disabled);
        assert_eq!(JobRuntimeConfig::default().mode, JobRuntimeMode::Disabled);
    }

    #[test]
    fn as_str_matches_audit_payload_contract() {
        assert_eq!(JobRuntimeMode::Disabled.as_str(), "disabled");
        assert_eq!(JobRuntimeMode::LocalContainer.as_str(), "local_container");
        assert_eq!(
            JobRuntimeMode::Cloud {
                endpoint: "https://example.com".into()
            }
            .as_str(),
            "cloud"
        );
    }

    #[test]
    fn serde_round_trip_disabled() {
        let m = JobRuntimeMode::Disabled;
        let s = toml::to_string(&JobRuntimeSettings { mode: m.clone() }).unwrap();
        assert!(s.contains("mode = \"disabled\""), "got: {s}");
        let back: JobRuntimeSettings = toml::from_str(&s).unwrap();
        assert_eq!(back.mode, m);
    }

    #[test]
    fn serde_round_trip_local_container() {
        let m = JobRuntimeMode::LocalContainer;
        let s = toml::to_string(&JobRuntimeSettings { mode: m.clone() }).unwrap();
        assert!(s.contains("mode = \"local_container\""), "got: {s}");
        let back: JobRuntimeSettings = toml::from_str(&s).unwrap();
        assert_eq!(back.mode, m);
    }

    #[test]
    fn serde_round_trip_cloud() {
        let m = JobRuntimeMode::Cloud {
            endpoint: "https://jobs.example.com/v1".to_string(),
        };
        let s = toml::to_string(&JobRuntimeSettings { mode: m.clone() }).unwrap();
        assert!(s.contains("mode = \"cloud\""), "got: {s}");
        assert!(s.contains("endpoint = \"https://jobs.example.com/v1\""));
        let back: JobRuntimeSettings = toml::from_str(&s).unwrap();
        assert_eq!(back.mode, m);
    }

    #[test]
    fn legacy_sandbox_true_derives_local_container() {
        let _guard = lock_env();
        clear_env();
        let s = settings_with_sandbox(true);
        let cfg = JobRuntimeConfig::resolve(&s).unwrap();
        assert_eq!(cfg.mode, JobRuntimeMode::LocalContainer);
    }

    #[test]
    fn legacy_sandbox_false_derives_disabled() {
        let _guard = lock_env();
        clear_env();
        let s = settings_with_sandbox(false);
        let cfg = JobRuntimeConfig::resolve(&s).unwrap();
        assert_eq!(cfg.mode, JobRuntimeMode::Disabled);
    }

    #[test]
    fn explicit_block_overrides_legacy() {
        let _guard = lock_env();
        clear_env();
        let mut s = Settings::default();
        s.sandbox.enabled = true;
        s.job_runtime = Some(JobRuntimeSettings {
            mode: JobRuntimeMode::Disabled,
        });
        let cfg = JobRuntimeConfig::resolve(&s).unwrap();
        assert_eq!(cfg.mode, JobRuntimeMode::Disabled);
    }

    #[test]
    fn explicit_cloud_requires_endpoint() {
        let _guard = lock_env();
        clear_env();
        let s = Settings {
            job_runtime: Some(JobRuntimeSettings {
                mode: JobRuntimeMode::Cloud {
                    endpoint: "   ".to_string(),
                },
            }),
            ..Settings::default()
        };
        let err = JobRuntimeConfig::resolve(&s).unwrap_err();
        assert!(
            matches!(err, ConfigError::InvalidValue { ref key, .. } if key == "job_runtime.endpoint")
        );
    }

    #[test]
    fn env_var_override_disabled() {
        let _guard = lock_env();
        // SAFETY: under ENV_MUTEX.
        unsafe {
            std::env::set_var("JOB_RUNTIME_MODE", "disabled");
        }
        let s = settings_with_sandbox(true);
        let cfg = JobRuntimeConfig::resolve(&s).unwrap();
        assert_eq!(cfg.mode, JobRuntimeMode::Disabled);
        clear_env();
    }

    #[test]
    fn env_var_override_cloud_with_endpoint() {
        let _guard = lock_env();
        // SAFETY: under ENV_MUTEX.
        unsafe {
            std::env::set_var("JOB_RUNTIME_MODE", "cloud:https://jobs.example.com/v1");
        }
        let s = settings_with_sandbox(false);
        let cfg = JobRuntimeConfig::resolve(&s).unwrap();
        assert_eq!(
            cfg.mode,
            JobRuntimeMode::Cloud {
                endpoint: "https://jobs.example.com/v1".to_string(),
            }
        );
        clear_env();
    }

    #[test]
    fn env_var_unknown_value_is_error() {
        let _guard = lock_env();
        // SAFETY: under ENV_MUTEX.
        unsafe {
            std::env::set_var("JOB_RUNTIME_MODE", "bogus");
        }
        let s = settings_with_sandbox(false);
        let err = JobRuntimeConfig::resolve(&s).unwrap_err();
        assert!(
            matches!(err, ConfigError::InvalidValue { ref key, .. } if key == "JOB_RUNTIME_MODE")
        );
        clear_env();
    }

    #[test]
    fn env_var_cloud_without_endpoint_is_error() {
        let _guard = lock_env();
        // SAFETY: under ENV_MUTEX.
        unsafe {
            std::env::set_var("JOB_RUNTIME_MODE", "cloud:");
        }
        let s = settings_with_sandbox(false);
        let err = JobRuntimeConfig::resolve(&s).unwrap_err();
        assert!(
            matches!(err, ConfigError::InvalidValue { ref key, .. } if key == "JOB_RUNTIME_MODE")
        );
        clear_env();
    }
}
