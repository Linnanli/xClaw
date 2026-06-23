use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::fs::{self, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

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
const REDACTED: &str = "<redacted>";
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
                AppServerError::capability_unavailable(
                    CAPABILITY,
                    format!("config file path is outside service root: {error}"),
                )
            })?,
            None => self.default_file(),
        };

        self.resolve_config_file(file, "config file path")
    }

    fn resolve_cwd(&self, cwd: &str) -> Result<PathBuf, AppServerError> {
        if cwd.trim().is_empty() {
            return Err(AppServerError::invalid_request(
                CAPABILITY,
                "cwd must not be empty",
            ));
        }
        let resolved = validate_path(cwd, Some(&self.root)).map_err(|error| {
            AppServerError::capability_unavailable(
                CAPABILITY,
                format!("config cwd is outside service root: {error}"),
            )
        })?;
        if !resolved.starts_with(&self.root) {
            return Err(AppServerError::capability_unavailable(
                CAPABILITY,
                "config cwd is outside service root",
            ));
        }
        Ok(resolved)
    }

    fn resolve_config_file(&self, file: PathBuf, label: &str) -> Result<PathBuf, AppServerError> {
        guard_path_under_root(&file, &self.root).map_err(|message| {
            AppServerError::capability_unavailable(
                CAPABILITY,
                format!("{label} is outside service root: {message}"),
            )
        })
    }

    fn read_target(&self, cwd: Option<&str>) -> Result<ConfigReadTarget, AppServerError> {
        match cwd {
            Some(cwd) => {
                let cwd = self.resolve_cwd(cwd)?;
                let file =
                    self.resolve_config_file(cwd.join(CONFIG_DIR).join(CONFIG_FILE), "config cwd")?;
                Ok(ConfigReadTarget {
                    file,
                    source: ConfigLayerSource::Project {
                        dot_dasclaw_folder: display_path(&cwd.join(CONFIG_DIR)),
                    },
                })
            }
            None => {
                let file = self.resolve_file(None)?;
                Ok(ConfigReadTarget {
                    source: ConfigLayerSource::User {
                        file: display_path(&file),
                    },
                    file,
                })
            }
        }
    }
}

#[derive(Debug)]
struct ConfigReadTarget {
    file: PathBuf,
    source: ConfigLayerSource,
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
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let target = self.read_target(params.cwd.as_deref())?;
        let config = read_json(&target.file)?;
        let version = version_for(&config);
        let redacted = redact_secrets(config);

        let origins = origins_for(&redacted, target.source.clone(), version.clone());
        let layers = params.include_layers.then(|| {
            vec![ConfigLayer {
                name: target.source,
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
            ensure_no_secret_keys_in_value(&edit.value)?;
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
    let temp_file = temp_file_for(file)?;
    let write_result = write_temp_json(&temp_file, &bytes)
        .and_then(|()| fs::rename(&temp_file, file).map_err(|error| error.to_string()));
    if let Err(message) = write_result {
        let _ = fs::remove_file(&temp_file);
        return Err(AppServerError::service_degraded(
            CAPABILITY,
            format!("failed to write config file: {message}"),
        ));
    }
    Ok(())
}

fn write_temp_json(temp_file: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp_file)
        .map_err(|error| error.to_string())?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    file.flush().map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())
}

fn temp_file_for(file: &Path) -> Result<PathBuf, AppServerError> {
    let parent = file.parent().ok_or_else(|| {
        AppServerError::service_degraded(CAPABILITY, "config file path has no parent")
    })?;
    let file_name = file
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            AppServerError::service_degraded(CAPABILITY, "config file path has no file name")
        })?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    Ok(parent.join(format!(".{file_name}.{}.{}.tmp", std::process::id(), nanos)))
}

fn version_for(config: &Value) -> String {
    let mut hasher = DefaultHasher::new();
    serde_json::to_string(config)
        .unwrap_or_default()
        .hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn ensure_writable_key(key_path: &str) -> Result<(), AppServerError> {
    let parts = key_path.split('.').collect::<Vec<_>>();
    let Some(first) = parts.first().copied().filter(|part| !part.is_empty()) else {
        return Err(AppServerError::invalid_request(
            CAPABILITY,
            "config keyPath must not be empty",
        ));
    };
    if parts.iter().any(|part| part.is_empty()) {
        return Err(AppServerError::invalid_request(
            CAPABILITY,
            "config keyPath must not contain empty path segments",
        ));
    }
    if is_secret_key(key_path) || parts.iter().any(|part| is_secret_key(part)) {
        return Err(AppServerError::invalid_request(
            CAPABILITY,
            format!("secret config keyPath is not writable: {key_path}"),
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

fn ensure_no_secret_keys_in_value(value: &Value) -> Result<(), AppServerError> {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if is_secret_key(key) {
                    return Err(AppServerError::invalid_request(
                        CAPABILITY,
                        format!("secret config value key is not writable: {key}"),
                    ));
                }
                ensure_no_secret_keys_in_value(value)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                ensure_no_secret_keys_in_value(value)?;
            }
        }
        _ => {}
    }
    Ok(())
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
                let value = if is_secret_key(&key) {
                    Value::String(REDACTED.to_string())
                } else {
                    redact_secrets(value)
                };
                redacted.insert(key, value);
            }
            Value::Object(redacted)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(redact_secrets).collect()),
        value => value,
    }
}

fn is_secret_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    normalized.contains("key")
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

fn guard_path_under_root(path: &Path, root: &Path) -> Result<PathBuf, String> {
    let root = root
        .canonicalize()
        .unwrap_or_else(|_| normalize_lexical(root));
    let normalized = normalize_lexical(path);
    let mut ancestor = normalized.as_path();
    let mut tail = Vec::new();

    loop {
        if ancestor.exists() {
            let canonical_ancestor = ancestor.canonicalize().map_err(|error| error.to_string())?;
            if !canonical_ancestor.starts_with(&root) {
                return Err(format!("{}", normalized.display()));
            }
            let mut guarded = canonical_ancestor;
            for part in tail.iter().rev() {
                guarded.push(part);
            }
            return Ok(guarded);
        }

        if let Some(file_name) = ancestor.file_name() {
            tail.push(file_name.to_os_string());
        }
        ancestor = ancestor
            .parent()
            .ok_or_else(|| format!("{} has no existing parent", normalized.display()))?;
    }
}
