use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use dasclaw_app_server_protocol::{
    ConfigBatchWriteParams, ConfigEdit, ConfigLayer, ConfigLayerMetadata, ConfigLayerSource,
    ConfigReadParams, ConfigReadResponse, ConfigValueWriteParams, ConfigWriteResponse,
    MergeStrategy, ServiceHealth, ServiceName,
};
use dasclaw_fs_tools::path_utils::{normalize_lexical, validate_path};
use serde_json::{Map, Value};

use crate::AppServerError;
use crate::app_services::ConfigService;

const CAPABILITY: &str = "config";
const CONFIG_DIR: &str = ".dasclaw";
const CONFIG_FILE: &str = "app-server-config.json";
const WRITABLE_CONFIG_KEYS: &[&str] = &[
    "model",
    "review_model",
    "model_provider",
    "approval_policy",
    "sandbox_mode",
    "model_reasoning_effort",
    "model_reasoning_summary",
    "profile",
    "instructions",
    "developer_instructions",
];

#[derive(Debug)]
pub struct AppServerConfigService {
    root: PathBuf,
    lock: Mutex<()>,
}

impl AppServerConfigService {
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        let lexical_root = normalize_lexical(&root);
        let root = root.canonicalize().unwrap_or(lexical_root);
        Self {
            root,
            lock: Mutex::new(()),
        }
    }

    fn default_file(&self) -> PathBuf {
        self.root.join(CONFIG_DIR).join(CONFIG_FILE)
    }

    fn resolve_file(&self, file_path: Option<&str>) -> Result<PathBuf, AppServerError> {
        let file = match file_path {
            Some(path) if path.trim().is_empty() => {
                return Err(AppServerError::invalid_request(
                    CAPABILITY,
                    "filePath must not be empty",
                ));
            }
            Some(path) => validate_path(path, Some(&self.root)).map_err(|error| {
                AppServerError::invalid_request(
                    CAPABILITY,
                    format!("config file path is outside service root: {error}"),
                )
            })?,
            None => self.default_file(),
        };

        if !file.starts_with(&self.root) {
            return Err(AppServerError::invalid_request(
                CAPABILITY,
                "config file path is outside service root",
            ));
        }

        Ok(file)
    }
}

impl ConfigService for AppServerConfigService {
    fn health(&self) -> ServiceHealth {
        let mut health = ServiceHealth::ready(ServiceName::Config);
        health.message = Some(format!(
            "file={} guard=canonical-root-containment",
            self.default_file().display()
        ));
        health
    }

    fn read(&self, params: ConfigReadParams) -> Result<ConfigReadResponse, AppServerError> {
        let file = self.resolve_file(None)?;
        let config = read_json(&file)?;
        let version = version_for(&config);
        let redacted = redact_secrets(config);
        let name = ConfigLayerSource::User {
            file: display_path(&file),
        };

        let origins = origins_for(&redacted, name.clone(), version.clone());
        let layers = params.include_layers.then(|| {
            vec![ConfigLayer {
                name,
                version,
                config: redacted.clone(),
                disabled_reason: None,
            }]
        });

        Ok(ConfigReadResponse {
            config: redacted,
            origins,
            layers,
        })
    }

    fn write_value(
        &self,
        params: ConfigValueWriteParams,
    ) -> Result<ConfigWriteResponse, AppServerError> {
        self.write_batch(ConfigBatchWriteParams {
            edits: vec![ConfigEdit {
                key_path: params.key_path,
                value: params.value,
                merge_strategy: params.merge_strategy,
            }],
            file_path: params.file_path,
            expected_version: params.expected_version,
            reload_user_config: None,
        })
    }

    fn write_batch(
        &self,
        params: ConfigBatchWriteParams,
    ) -> Result<ConfigWriteResponse, AppServerError> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let file = self.resolve_file(params.file_path.as_deref())?;
        let mut config = read_json(&file)?;
        let current_version = version_for(&config);

        if params
            .expected_version
            .as_deref()
            .is_some_and(|expected| expected != current_version)
        {
            return Err(AppServerError::invalid_request(
                CAPABILITY,
                "expectedVersion does not match current config version",
            ));
        }

        for edit in params.edits {
            ensure_writable_key(&edit.key_path)?;
            apply_edit(&mut config, edit)?;
        }

        write_json(&file, &config)?;
        let version = version_for(&config);
        Ok(ConfigWriteResponse {
            config: redact_secrets(config),
            version,
        })
    }
}

fn read_json(file: &Path) -> Result<Value, AppServerError> {
    match fs::read_to_string(file) {
        Ok(content) => {
            if content.trim().is_empty() {
                return Ok(Value::Object(Map::new()));
            }
            let value = serde_json::from_str(&content).map_err(|error| {
                AppServerError::invalid_request(
                    CAPABILITY,
                    format!("config file is not valid JSON: {error}"),
                )
            })?;
            match value {
                Value::Object(_) => Ok(value),
                _ => Err(AppServerError::invalid_request(
                    CAPABILITY,
                    "config file must contain a JSON object",
                )),
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Value::Object(Map::new())),
        Err(error) => Err(AppServerError::service_degraded(
            CAPABILITY,
            format!("failed to read config file: {error}"),
        )),
    }
}

fn write_json(file: &Path, config: &Value) -> Result<(), AppServerError> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            AppServerError::service_degraded(
                CAPABILITY,
                format!("failed to create config directory: {error}"),
            )
        })?;
    }
    let bytes = serde_json::to_vec_pretty(config).map_err(|error| {
        AppServerError::service_degraded(CAPABILITY, format!("failed to encode config: {error}"))
    })?;
    fs::write(file, bytes).map_err(|error| {
        AppServerError::service_degraded(
            CAPABILITY,
            format!("failed to write config file: {error}"),
        )
    })
}

fn version_for(config: &Value) -> String {
    let mut hasher = DefaultHasher::new();
    serde_json::to_string(config)
        .unwrap_or_default()
        .hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn ensure_writable_key(key_path: &str) -> Result<(), AppServerError> {
    let mut parts = key_path.split('.');
    let Some(first) = parts.next().filter(|part| !part.is_empty()) else {
        return Err(AppServerError::invalid_request(
            CAPABILITY,
            "config keyPath must not be empty",
        ));
    };
    if parts.any(str::is_empty) {
        return Err(AppServerError::invalid_request(
            CAPABILITY,
            "config keyPath must not contain empty path segments",
        ));
    }
    if WRITABLE_CONFIG_KEYS.contains(&first) {
        Ok(())
    } else {
        Err(AppServerError::invalid_request(
            CAPABILITY,
            format!("config keyPath is not writable: {key_path}"),
        ))
    }
}

fn apply_edit(config: &mut Value, edit: ConfigEdit) -> Result<(), AppServerError> {
    let parts = edit.key_path.split('.').collect::<Vec<_>>();
    let Some((last, parents)) = parts.split_last() else {
        return Err(AppServerError::invalid_request(
            CAPABILITY,
            "config keyPath must not be empty",
        ));
    };

    if !config.is_object() {
        *config = Value::Object(Map::new());
    }

    let mut current = config;
    for part in parents {
        let object = current.as_object_mut().ok_or_else(|| {
            AppServerError::invalid_request(CAPABILITY, "config keyPath parent is not an object")
        })?;
        current = object
            .entry((*part).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !current.is_object() {
            *current = Value::Object(Map::new());
        }
    }

    let object = current.as_object_mut().ok_or_else(|| {
        AppServerError::invalid_request(CAPABILITY, "config keyPath parent is not an object")
    })?;
    match edit.merge_strategy {
        MergeStrategy::Replace => {
            object.insert((*last).to_string(), edit.value);
        }
        MergeStrategy::Upsert => {
            let target = object.entry((*last).to_string()).or_insert(Value::Null);
            merge_value(target, edit.value);
        }
    }
    Ok(())
}

fn merge_value(target: &mut Value, patch: Value) {
    match (target, patch) {
        (Value::Object(target), Value::Object(patch)) => {
            for (key, value) in patch {
                merge_value(target.entry(key).or_insert(Value::Null), value);
            }
        }
        (target, value) => *target = value,
    }
}

fn redact_secrets(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut redacted = Map::new();
            for (key, value) in object {
                if !is_secret_key(&key) {
                    redacted.insert(key, redact_secrets(value));
                }
            }
            Value::Object(redacted)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(redact_secrets).collect()),
        value => value,
    }
}

fn is_secret_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    normalized.contains("api_key")
        || normalized.contains("apikey")
        || normalized.contains("secret")
        || normalized.contains("token")
        || normalized.contains("password")
        || normalized.contains("credential")
}

fn origins_for(
    config: &Value,
    name: ConfigLayerSource,
    version: String,
) -> BTreeMap<String, ConfigLayerMetadata> {
    config
        .as_object()
        .into_iter()
        .flat_map(|object| object.keys())
        .map(|key| {
            (
                key.clone(),
                ConfigLayerMetadata {
                    name: name.clone(),
                    version: version.clone(),
                },
            )
        })
        .collect()
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
