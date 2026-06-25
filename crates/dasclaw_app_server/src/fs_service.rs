use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use dasclaw_app_server_protocol::{
    FsChangedKind, FsChangedNotification, FsCopyParams, FsCopyResponse, FsCreateDirectoryParams,
    FsCreateDirectoryResponse, FsGetMetadataParams, FsGetMetadataResponse, FsReadDirectoryEntry,
    FsReadDirectoryParams, FsReadDirectoryResponse, FsReadFileParams, FsReadFileResponse,
    FsRemoveParams, FsRemoveResponse, FsUnwatchParams, FsUnwatchResponse, FsWatchParams,
    FsWatchResponse, FsWriteFileParams, FsWriteFileResponse, FsWriteMode, LogEntryEvent, LogLevel,
    ServiceHealth, ServiceName,
};
use dasclaw_fs_tools::path_utils::{normalize_lexical, validate_path};

use crate::AppServerError;
use crate::app_services::FsService;

#[derive(Debug)]
pub struct AppServerFsService {
    root: PathBuf,
    lexical_root: PathBuf,
    watches: Mutex<HashMap<String, FsWatch>>,
    events: Mutex<Vec<FsChangedNotification>>,
    audit_entries: Mutex<Vec<LogEntryEvent>>,
}

#[derive(Debug, Clone)]
struct FsWatch {
    path: PathBuf,
    display_path: String,
    last_state: WatchState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum WatchState {
    Missing,
    Present {
        modified_at_ms: u64,
        len: u64,
        is_directory: bool,
    },
}

impl AppServerFsService {
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        let lexical_root = normalize_lexical(&root);
        let root = root.canonicalize().unwrap_or_else(|_| lexical_root.clone());
        Self {
            root,
            lexical_root,
            watches: Mutex::new(HashMap::new()),
            events: Mutex::new(Vec::new()),
            audit_entries: Mutex::new(Vec::new()),
        }
    }

    fn resolve_existing(&self, raw: &str) -> Result<ResolvedPath, ResolveError> {
        self.resolve(raw, PathExpectation::MustExist)
    }

    fn resolve_for_create(&self, raw: &str) -> Result<PathBuf, ResolveError> {
        self.resolve(raw, PathExpectation::MayCreateLeaf)
            .map(|resolved| resolved.canonical)
    }

    fn resolve_for_recursive_create(&self, raw: &str) -> Result<PathBuf, ResolveError> {
        self.reject_unsafe_path(raw)?;
        let resolved = validate_path(raw, Some(&self.root))
            .map_err(|error| ResolveError::security(error.to_string()))?;
        let mut ancestor = resolved.as_path();
        let mut tail_parts = Vec::new();

        while !ancestor.exists() {
            let file_name = ancestor
                .file_name()
                .ok_or_else(|| ResolveError::security("filesystem path has no file name"))?;
            tail_parts.push(file_name.to_os_string());
            ancestor = ancestor
                .parent()
                .ok_or_else(|| ResolveError::security("filesystem path has no existing parent"))?;
        }

        let canonical_ancestor = ancestor
            .canonicalize()
            .map_err(|error| ResolveError::io(error.to_string()))?;
        if !canonical_ancestor.starts_with(&self.root) {
            return Err(ResolveError::security(
                "filesystem path is outside service root",
            ));
        }

        let mut guarded = canonical_ancestor;
        for part in tail_parts.iter().rev() {
            guarded.push(part);
        }
        Ok(guarded)
    }

    fn resolve(
        &self,
        raw: &str,
        expectation: PathExpectation,
    ) -> Result<ResolvedPath, ResolveError> {
        self.reject_unsafe_path(raw)?;
        let original = self.original_path(raw);
        let resolved = validate_path(raw, Some(&self.root))
            .map_err(|error| ResolveError::security(error.to_string()))?;

        let guarded = if resolved.exists() {
            resolved
                .canonicalize()
                .map_err(|error| ResolveError::io(error.to_string()))?
        } else if expectation == PathExpectation::MayCreateLeaf {
            let parent = resolved
                .parent()
                .ok_or_else(|| ResolveError::security("filesystem path has no parent"))?;
            let parent = parent
                .canonicalize()
                .map_err(|error| ResolveError::io(error.to_string()))?;
            parent.join(
                resolved
                    .file_name()
                    .ok_or_else(|| ResolveError::security("filesystem path has no file name"))?,
            )
        } else {
            return Err(ResolveError::NotFound(
                "filesystem path does not exist".to_string(),
            ));
        };

        if !guarded.starts_with(&self.root) {
            return Err(ResolveError::security(
                "filesystem path is outside service root",
            ));
        }
        Ok(ResolvedPath {
            original,
            canonical: guarded,
        })
    }

    fn original_path(&self, raw: &str) -> PathBuf {
        let path = PathBuf::from(raw);
        if path.is_absolute() {
            path
        } else {
            self.root.join(path)
        }
    }

    fn reject_unsafe_path(&self, raw: &str) -> Result<(), ResolveError> {
        if raw.as_bytes().contains(&0) {
            return Err(ResolveError::security(
                "filesystem path contains a null byte",
            ));
        }

        let path = Path::new(raw);
        if path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(ResolveError::security(
                "filesystem path traversal is not allowed",
            ));
        }

        if path.is_absolute() {
            let lexical = normalize_lexical(path);
            if !lexical.starts_with(&self.root) && !lexical.starts_with(&self.lexical_root) {
                return Err(ResolveError::security(
                    "absolute filesystem path is outside service root",
                ));
            }
        }

        Ok(())
    }

    fn metadata_state(path: &Path) -> WatchState {
        match fs::metadata(path) {
            Ok(metadata) => WatchState::Present {
                modified_at_ms: system_time_ms(metadata.modified().unwrap_or(UNIX_EPOCH)),
                len: metadata.len(),
                is_directory: metadata.is_dir(),
            },
            Err(_) => WatchState::Missing,
        }
    }

    fn poll_watches(&self) {
        let mut emitted = Vec::new();
        let mut watches = self
            .watches
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());

        for (watch_id, watch) in watches.iter_mut() {
            let next = Self::metadata_state(&watch.path);
            if next == watch.last_state {
                continue;
            }

            let kind = match (&watch.last_state, &next) {
                (WatchState::Missing, WatchState::Present { .. }) => FsChangedKind::Created,
                (WatchState::Present { .. }, WatchState::Missing) => FsChangedKind::Removed,
                (WatchState::Present { .. }, WatchState::Present { .. }) => FsChangedKind::Modified,
                (WatchState::Missing, WatchState::Missing) => continue,
            };
            watch.last_state = next;
            emitted.push(FsChangedNotification {
                watch_id: watch_id.clone(),
                path: watch.display_path.clone(),
                kind,
            });
        }

        if !emitted.is_empty() {
            self.events
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .extend(emitted);
        }
    }

    fn push_audit(
        &self,
        operation: &'static str,
        outcome: &'static str,
        mut fields: serde_json::Map<String, serde_json::Value>,
    ) {
        fields.insert("operation".to_string(), operation.into());
        fields.insert("outcome".to_string(), outcome.into());

        self.audit_entries
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .push(LogEntryEvent {
                level: LogLevel::Info,
                target: "audit.filesystem".to_string(),
                message: "filesystem audit".to_string(),
                time: crate::unix_timestamp_string(),
                fields,
            });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathExpectation {
    MustExist,
    MayCreateLeaf,
}

#[derive(Debug, Clone)]
struct ResolvedPath {
    original: PathBuf,
    canonical: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ResolveError {
    Security(String),
    NotFound(String),
    Io(String),
}

impl ResolveError {
    fn security(message: impl Into<String>) -> Self {
        Self::Security(message.into())
    }

    fn io(message: impl Into<String>) -> Self {
        Self::Io(message.into())
    }

    fn into_app_error(self) -> AppServerError {
        match self {
            Self::Security(message) | Self::NotFound(message) | Self::Io(message) => {
                filesystem_unavailable(message)
            }
        }
    }
}

impl FsService for AppServerFsService {
    fn health(&self) -> ServiceHealth {
        let mut health = ServiceHealth::ready(ServiceName::Filesystem);
        health.message = Some(format!(
            "root={} guard=canonical-root-containment",
            self.root.display()
        ));
        health
    }

    fn read_file(&self, params: FsReadFileParams) -> Result<FsReadFileResponse, AppServerError> {
        let mut fields = serde_json::Map::new();
        fields.insert("hasPath".to_string(), true.into());
        fields.insert("hasOffset".to_string(), params.offset.is_some().into());
        fields.insert("hasLength".to_string(), params.length.is_some().into());
        let result = (|| {
            let path = self
                .resolve_existing(&params.path)
                .map_err(ResolveError::into_app_error)?;
            let bytes = fs::read(path.canonical)
                .map_err(|error| filesystem_unavailable(error.to_string()))?;
            let start = params.offset.unwrap_or(0).min(bytes.len());
            let end = params
                .length
                .map(|length| start.saturating_add(length).min(bytes.len()))
                .unwrap_or(bytes.len());

            Ok(FsReadFileResponse {
                data_base64: base64::engine::general_purpose::STANDARD.encode(&bytes[start..end]),
            })
        })();
        self.push_audit(
            "read",
            if result.is_ok() {
                "completed"
            } else {
                "rejected"
            },
            fields,
        );
        result
    }

    fn write_file(&self, params: FsWriteFileParams) -> Result<FsWriteFileResponse, AppServerError> {
        let mode = params.mode.unwrap_or_default();
        let mut fields = serde_json::Map::new();
        fields.insert("hasPath".to_string(), true.into());
        fields.insert("mode".to_string(), format!("{mode:?}").into());
        let result = (|| {
            let path = self
                .resolve_for_create(&params.path)
                .map_err(ResolveError::into_app_error)?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(params.data_base64)
                .map_err(|error| filesystem_unavailable(error.to_string()))?;
            fields.insert("bytes".to_string(), bytes.len().into());

            let mut options = OpenOptions::new();
            options.write(true);
            match mode {
                FsWriteMode::Create => {
                    options.create_new(true);
                }
                FsWriteMode::Overwrite => {
                    options.create(true).truncate(true);
                }
                FsWriteMode::Append => {
                    options.create(true).append(true);
                }
            }

            let mut file = options
                .open(path)
                .map_err(|error| filesystem_unavailable(error.to_string()))?;
            file.write_all(&bytes)
                .map_err(|error| filesystem_unavailable(error.to_string()))?;
            self.poll_watches();
            Ok(FsWriteFileResponse {})
        })();
        self.push_audit(
            "write",
            if result.is_ok() {
                "completed"
            } else {
                "rejected"
            },
            fields,
        );
        result
    }

    fn create_directory(
        &self,
        params: FsCreateDirectoryParams,
    ) -> Result<FsCreateDirectoryResponse, AppServerError> {
        let recursive = params.recursive.unwrap_or(false);
        let mut fields = serde_json::Map::new();
        fields.insert("hasPath".to_string(), true.into());
        fields.insert("recursive".to_string(), recursive.into());
        let result = (|| {
            if recursive {
                let path = self
                    .resolve_for_recursive_create(&params.path)
                    .map_err(ResolveError::into_app_error)?;
                fs::create_dir_all(path)
            } else {
                let path = self
                    .resolve_for_create(&params.path)
                    .map_err(ResolveError::into_app_error)?;
                fs::create_dir(path)
            }
            .map_err(|error| filesystem_unavailable(error.to_string()))?;
            self.poll_watches();
            Ok(FsCreateDirectoryResponse {})
        })();
        self.push_audit(
            "createDirectory",
            if result.is_ok() {
                "completed"
            } else {
                "rejected"
            },
            fields,
        );
        result
    }

    fn get_metadata(
        &self,
        params: FsGetMetadataParams,
    ) -> Result<FsGetMetadataResponse, AppServerError> {
        let mut fields = serde_json::Map::new();
        fields.insert("hasPath".to_string(), true.into());
        let result = (|| {
            let path = self
                .resolve_existing(&params.path)
                .map_err(ResolveError::into_app_error)?;
            let metadata = fs::symlink_metadata(path.original)
                .map_err(|error| filesystem_unavailable(error.to_string()))?;
            Ok(FsGetMetadataResponse {
                is_file: metadata.is_file(),
                is_directory: metadata.is_dir(),
                is_symlink: metadata.file_type().is_symlink(),
                created_at_ms: system_time_ms(metadata.created().unwrap_or(UNIX_EPOCH)),
                modified_at_ms: system_time_ms(metadata.modified().unwrap_or(UNIX_EPOCH)),
            })
        })();
        self.push_audit(
            "metadata",
            if result.is_ok() {
                "completed"
            } else {
                "rejected"
            },
            fields,
        );
        result
    }

    fn read_directory(
        &self,
        params: FsReadDirectoryParams,
    ) -> Result<FsReadDirectoryResponse, AppServerError> {
        let mut fields = serde_json::Map::new();
        fields.insert("hasPath".to_string(), true.into());
        let result = (|| {
            let path = self
                .resolve_existing(&params.path)
                .map_err(ResolveError::into_app_error)?;
            let mut entries = Vec::new();
            for entry in fs::read_dir(path.canonical)
                .map_err(|error| filesystem_unavailable(error.to_string()))?
            {
                let entry = entry.map_err(|error| filesystem_unavailable(error.to_string()))?;
                let metadata = fs::symlink_metadata(entry.path())
                    .map_err(|error| filesystem_unavailable(error.to_string()))?;
                entries.push(FsReadDirectoryEntry {
                    file_name: entry.file_name().to_string_lossy().to_string(),
                    is_file: metadata.is_file(),
                    is_directory: metadata.is_dir(),
                });
            }
            entries.sort_by(|left, right| left.file_name.cmp(&right.file_name));
            Ok(FsReadDirectoryResponse { entries })
        })();
        self.push_audit(
            "readDirectory",
            if result.is_ok() {
                "completed"
            } else {
                "rejected"
            },
            fields,
        );
        result
    }

    fn remove(&self, params: FsRemoveParams) -> Result<FsRemoveResponse, AppServerError> {
        let recursive = params.recursive.unwrap_or(false);
        let force = params.force.unwrap_or(false);
        let mut fields = serde_json::Map::new();
        fields.insert("hasPath".to_string(), true.into());
        fields.insert("recursive".to_string(), recursive.into());
        fields.insert("force".to_string(), force.into());
        let result = (|| {
            let path = match self.resolve_existing(&params.path) {
                Ok(path) => path,
                Err(ResolveError::NotFound(_)) if force => {
                    return Ok(FsRemoveResponse {});
                }
                Err(error) => return Err(error.into_app_error()),
            };

            if path.canonical.is_dir() {
                if recursive {
                    fs::remove_dir_all(path.canonical)
                } else {
                    fs::remove_dir(path.canonical)
                }
            } else {
                fs::remove_file(path.canonical)
            }
            .map_err(|error| filesystem_unavailable(error.to_string()))?;
            self.poll_watches();
            Ok(FsRemoveResponse {})
        })();
        self.push_audit(
            "remove",
            if result.is_ok() {
                "completed"
            } else {
                "rejected"
            },
            fields,
        );
        result
    }

    fn copy(&self, params: FsCopyParams) -> Result<FsCopyResponse, AppServerError> {
        let recursive = params.recursive.unwrap_or(false);
        let mut fields = serde_json::Map::new();
        fields.insert("hasSourcePath".to_string(), true.into());
        fields.insert("hasDestinationPath".to_string(), true.into());
        fields.insert("recursive".to_string(), recursive.into());
        let result = (|| {
            let source = self
                .resolve_existing(&params.source_path)
                .map_err(ResolveError::into_app_error)?;
            let destination = self
                .resolve_for_create(&params.destination_path)
                .map_err(ResolveError::into_app_error)?;

            if source.canonical.is_dir() {
                if !recursive {
                    return Err(filesystem_unavailable(
                        "recursive=true is required for directory copies",
                    ));
                }
                copy_dir_recursive(&source.canonical, &destination)?;
            } else {
                fs::copy(source.canonical, destination)
                    .map_err(|error| filesystem_unavailable(error.to_string()))?;
            }
            self.poll_watches();
            Ok(FsCopyResponse {})
        })();
        self.push_audit(
            "copy",
            if result.is_ok() {
                "completed"
            } else {
                "rejected"
            },
            fields,
        );
        result
    }

    fn watch(&self, params: FsWatchParams) -> Result<FsWatchResponse, AppServerError> {
        let mut fields = serde_json::Map::new();
        fields.insert("hasPath".to_string(), true.into());
        fields.insert("hasWatchId".to_string(), true.into());
        let result = (|| {
            let path = self
                .resolve_existing(&params.path)
                .map_err(ResolveError::into_app_error)?;
            let watch = FsWatch {
                display_path: params.path.clone(),
                last_state: Self::metadata_state(&path.canonical),
                path: path.canonical,
            };
            self.watches
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .insert(params.watch_id, watch);
            Ok(FsWatchResponse { path: params.path })
        })();
        self.push_audit(
            "watch",
            if result.is_ok() {
                "completed"
            } else {
                "rejected"
            },
            fields,
        );
        result
    }

    fn unwatch(&self, params: FsUnwatchParams) -> Result<FsUnwatchResponse, AppServerError> {
        let mut fields = serde_json::Map::new();
        fields.insert("hasWatchId".to_string(), true.into());
        let result = {
            self.watches
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .remove(&params.watch_id);
            Ok(FsUnwatchResponse {})
        };
        self.push_audit("unwatch", "completed", fields);
        result
    }

    fn drain_changed_events(&self) -> Vec<FsChangedNotification> {
        self.poll_watches();
        self.events
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .drain(..)
            .collect()
    }

    fn drain_audit_entries(&self) -> Vec<LogEntryEvent> {
        self.audit_entries
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .drain(..)
            .collect()
    }
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<(), AppServerError> {
    fs::create_dir(destination).map_err(|error| filesystem_unavailable(error.to_string()))?;
    for entry in fs::read_dir(source).map_err(|error| filesystem_unavailable(error.to_string()))? {
        let entry = entry.map_err(|error| filesystem_unavailable(error.to_string()))?;
        let from = entry.path();
        let to = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|error| filesystem_unavailable(error.to_string()))?;
        if file_type.is_symlink() {
            return Err(filesystem_unavailable(
                "symbolic links are not allowed in recursive filesystem copies",
            ));
        }

        if file_type.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            fs::copy(from, to).map_err(|error| filesystem_unavailable(error.to_string()))?;
        }
    }
    Ok(())
}

fn system_time_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn filesystem_unavailable(message: impl Into<String>) -> AppServerError {
    AppServerError::capability_unavailable("filesystem", message)
}

#[cfg(test)]
mod tests {
    use base64::Engine;
    use dasclaw_app_server_protocol::{
        FsChangedKind, FsCopyParams, FsCreateDirectoryParams, FsGetMetadataParams,
        FsReadDirectoryParams, FsReadFileParams, FsRemoveParams, FsUnwatchParams, FsWatchParams,
        FsWriteFileParams, FsWriteMode,
    };

    use super::*;

    fn b64(input: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(input)
    }

    fn read_params(path: &str) -> FsReadFileParams {
        FsReadFileParams {
            path: path.to_string(),
            offset: None,
            length: None,
        }
    }

    fn write_params(path: &str, input: &[u8], mode: Option<FsWriteMode>) -> FsWriteFileParams {
        FsWriteFileParams {
            path: path.to_string(),
            data_base64: b64(input),
            mode,
        }
    }

    #[test]
    fn fs_service_reads_writes_lists_metadata_and_removes_inside_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerFsService::new(temp.path().to_path_buf());

        service
            .create_directory(FsCreateDirectoryParams {
                path: "dir".to_string(),
                recursive: None,
            })
            .expect("create directory");
        service
            .write_file(write_params("dir/file.txt", b"hello", None))
            .expect("write file");

        let read = service
            .read_file(FsReadFileParams {
                path: "dir/file.txt".to_string(),
                offset: Some(1),
                length: Some(3),
            })
            .expect("read file slice");
        assert_eq!(read.data_base64, b64(b"ell"));

        let metadata = service
            .get_metadata(FsGetMetadataParams {
                path: "dir/file.txt".to_string(),
            })
            .expect("metadata");
        assert!(metadata.is_file);
        assert!(!metadata.is_directory);
        assert!(!metadata.is_symlink);

        let listing = service
            .read_directory(FsReadDirectoryParams {
                path: "dir".to_string(),
            })
            .expect("list directory");
        assert_eq!(listing.entries.len(), 1);
        assert_eq!(listing.entries[0].file_name, "file.txt");

        service
            .remove(FsRemoveParams {
                path: "dir/file.txt".to_string(),
                recursive: None,
                force: None,
            })
            .expect("remove file");
        assert!(!temp.path().join("dir/file.txt").exists());
    }

    #[test]
    fn fs_service_base64_roundtrip_and_write_modes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerFsService::new(temp.path().to_path_buf());

        service
            .write_file(write_params(
                "data.bin",
                b"\x00hello",
                Some(FsWriteMode::Create),
            ))
            .expect("create file");
        assert!(
            service
                .write_file(write_params(
                    "data.bin",
                    b"again",
                    Some(FsWriteMode::Create)
                ))
                .is_err()
        );
        service
            .write_file(write_params(
                "data.bin",
                b" world",
                Some(FsWriteMode::Append),
            ))
            .expect("append file");

        let read = service
            .read_file(read_params("data.bin"))
            .expect("read roundtrip");
        assert_eq!(read.data_base64, b64(b"\x00hello world"));

        service
            .write_file(write_params(
                "data.bin",
                b"reset",
                Some(FsWriteMode::Overwrite),
            ))
            .expect("overwrite file");
        let read = service
            .read_file(read_params("data.bin"))
            .expect("read overwritten");
        assert_eq!(read.data_base64, b64(b"reset"));
    }

    #[test]
    fn fs_service_audit_redacts_paths_and_file_contents() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerFsService::new(temp.path().to_path_buf());
        let path_secret = "path-secret-token.txt";
        let content_secret = b"content-secret-token";

        service
            .write_file(write_params(path_secret, content_secret, None))
            .expect("write file");
        service
            .read_file(read_params(path_secret))
            .expect("read file");
        let audit_text = serde_json::to_string(&service.drain_audit_entries())
            .expect("audit entries should serialize");

        assert!(audit_text.contains("audit.filesystem"));
        assert!(audit_text.contains("\"operation\":\"write\""));
        assert!(audit_text.contains("\"operation\":\"read\""));
        assert!(audit_text.contains("\"bytes\""));
        assert!(!audit_text.contains(path_secret));
        assert!(!audit_text.contains("content-secret-token"));
        assert!(!audit_text.contains(&b64(content_secret)));
    }

    #[test]
    fn fs_service_rejects_escape_outside_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        let service = AppServerFsService::new(temp.path().to_path_buf());

        assert!(service.read_file(read_params("../secret.txt")).is_err());
        assert!(
            service
                .read_file(read_params(outside.path().to_string_lossy().as_ref()))
                .is_err()
        );
    }

    #[test]
    fn fs_service_allows_absolute_paths_inside_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerFsService::new(temp.path().to_path_buf());
        let file = temp.path().join("absolute.txt");
        let file = file.to_string_lossy();

        service
            .write_file(write_params(&file, b"absolute", None))
            .expect("write absolute path inside root");
        let read = service
            .read_file(read_params(&file))
            .expect("read absolute path inside root");

        assert_eq!(read.data_base64, b64(b"absolute"));
    }

    #[cfg(unix)]
    #[test]
    fn fs_service_rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        let outside_file = outside.path().join("secret.txt");
        fs::write(&outside_file, b"secret").expect("write outside file");
        symlink(&outside_file, temp.path().join("link.txt")).expect("symlink");

        let service = AppServerFsService::new(temp.path().to_path_buf());
        assert!(service.read_file(read_params("link.txt")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn fs_service_rejects_outside_absolute_symlink_to_inside_root() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        let inside_file = temp.path().join("inside.txt");
        fs::write(&inside_file, b"inside").expect("write inside file");
        let outside_link = outside.path().join("link_to_inside.txt");
        symlink(&inside_file, &outside_link).expect("symlink");
        let outside_link = outside_link.to_string_lossy();
        let service = AppServerFsService::new(temp.path().to_path_buf());

        assert!(service.read_file(read_params(&outside_link)).is_err());
        assert!(
            service
                .get_metadata(FsGetMetadataParams {
                    path: outside_link.to_string(),
                })
                .is_err()
        );
        assert!(
            service
                .remove(FsRemoveParams {
                    path: outside_link.to_string(),
                    recursive: None,
                    force: Some(true),
                })
                .is_err()
        );
        assert!(
            service
                .watch(FsWatchParams {
                    path: outside_link.to_string(),
                    watch_id: "outside_link".to_string(),
                })
                .is_err()
        );
        assert!(
            service
                .copy(FsCopyParams {
                    source_path: outside_link.to_string(),
                    destination_path: "copy.txt".to_string(),
                    recursive: None,
                })
                .is_err()
        );
        assert!(
            service
                .write_file(write_params(&outside_link, b"overwrite", None))
                .is_err()
        );
        assert_eq!(
            fs::read(&inside_file).expect("inside file should remain unchanged"),
            b"inside"
        );
    }

    #[test]
    fn fs_service_recursive_create_directory_creates_missing_parent_tree() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerFsService::new(temp.path().to_path_buf());

        service
            .create_directory(FsCreateDirectoryParams {
                path: "a/b/c".to_string(),
                recursive: Some(true),
            })
            .expect("recursive mkdir");

        assert!(temp.path().join("a/b/c").is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn fs_service_recursive_create_directory_rejects_symlink_parent_escape() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        symlink(outside.path(), temp.path().join("link")).expect("symlink");
        let service = AppServerFsService::new(temp.path().to_path_buf());

        assert!(
            service
                .create_directory(FsCreateDirectoryParams {
                    path: "link/a/b".to_string(),
                    recursive: Some(true),
                })
                .is_err()
        );
        assert!(!outside.path().join("a").exists());
    }

    #[test]
    fn fs_service_remove_force_only_ignores_missing_inside_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerFsService::new(temp.path().to_path_buf());

        service
            .remove(FsRemoveParams {
                path: "missing.txt".to_string(),
                recursive: None,
                force: Some(true),
            })
            .expect("force remove missing root path");
        assert!(
            service
                .remove(FsRemoveParams {
                    path: "../secret.txt".to_string(),
                    recursive: None,
                    force: Some(true),
                })
                .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn fs_service_list_directory_does_not_follow_symlink_entries() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        let outside_file = outside.path().join("secret.txt");
        fs::write(&outside_file, b"secret").expect("write outside file");
        symlink(&outside_file, temp.path().join("link.txt")).expect("symlink");
        let service = AppServerFsService::new(temp.path().to_path_buf());

        let listing = service
            .read_directory(FsReadDirectoryParams {
                path: ".".to_string(),
            })
            .expect("list root");
        let link = listing
            .entries
            .iter()
            .find(|entry| entry.file_name == "link.txt")
            .expect("link entry");

        assert!(!link.is_file);
        assert!(!link.is_directory);
    }

    #[cfg(unix)]
    #[test]
    fn fs_service_stat_preserves_symlink_identity_for_inside_target() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("target.txt"), b"target").expect("write target");
        symlink(temp.path().join("target.txt"), temp.path().join("link.txt")).expect("symlink");
        let service = AppServerFsService::new(temp.path().to_path_buf());

        let metadata = service
            .get_metadata(FsGetMetadataParams {
                path: "link.txt".to_string(),
            })
            .expect("metadata");

        assert!(metadata.is_symlink);
        assert!(!metadata.is_file);
        assert!(!metadata.is_directory);
    }

    #[test]
    fn fs_service_watch_emits_changed_event() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerFsService::new(temp.path().to_path_buf());
        service
            .write_file(write_params("watched.txt", b"before", None))
            .expect("seed watched file");
        service
            .watch(FsWatchParams {
                path: "watched.txt".to_string(),
                watch_id: "watch_1".to_string(),
            })
            .expect("watch file");

        std::thread::sleep(std::time::Duration::from_millis(5));
        service
            .write_file(write_params("watched.txt", b"after", None))
            .expect("modify watched file");

        let events = service.drain_changed_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].watch_id, "watch_1");
        assert_eq!(events[0].path, "watched.txt");
        assert_eq!(events[0].kind, FsChangedKind::Modified);

        service
            .unwatch(FsUnwatchParams {
                watch_id: "watch_1".to_string(),
            })
            .expect("unwatch");
    }

    #[test]
    fn fs_service_does_not_create_unknown_parent_tree() {
        let temp = tempfile::tempdir().expect("tempdir");
        let service = AppServerFsService::new(temp.path().to_path_buf());

        assert!(
            service
                .write_file(write_params("missing/file.txt", b"hello", None))
                .is_err()
        );
        assert!(!temp.path().join("missing").exists());
    }

    #[cfg(unix)]
    #[test]
    fn fs_service_rejects_symlink_inside_recursive_copy() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        let outside_file = outside.path().join("secret.txt");
        fs::write(&outside_file, b"secret").expect("write outside file");
        fs::create_dir(temp.path().join("source")).expect("create source");
        symlink(&outside_file, temp.path().join("source/link.txt")).expect("symlink");

        let service = AppServerFsService::new(temp.path().to_path_buf());
        assert!(
            service
                .copy(FsCopyParams {
                    source_path: "source".to_string(),
                    destination_path: "copy".to_string(),
                    recursive: Some(true),
                })
                .is_err()
        );
    }
}
