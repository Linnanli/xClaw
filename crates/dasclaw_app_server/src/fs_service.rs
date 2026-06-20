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
    FsWatchResponse, FsWriteFileParams, FsWriteFileResponse, FsWriteMode, ServiceHealth,
    ServiceName,
};
use dasclaw_fs_tools::path_utils::{normalize_lexical, validate_path};

use crate::AppServerError;
use crate::app_services::FsService;

#[derive(Debug)]
pub struct AppServerFsService {
    root: PathBuf,
    watches: Mutex<HashMap<String, FsWatch>>,
    events: Mutex<Vec<FsChangedNotification>>,
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
        let root = root
            .canonicalize()
            .unwrap_or_else(|_| normalize_lexical(&root));
        Self {
            root,
            watches: Mutex::new(HashMap::new()),
            events: Mutex::new(Vec::new()),
        }
    }

    fn resolve_existing(&self, raw: &str) -> Result<PathBuf, AppServerError> {
        self.resolve(raw, PathExpectation::MustExist)
    }

    fn resolve_for_create(&self, raw: &str) -> Result<PathBuf, AppServerError> {
        self.resolve(raw, PathExpectation::MayCreateLeaf)
    }

    fn resolve(&self, raw: &str, expectation: PathExpectation) -> Result<PathBuf, AppServerError> {
        reject_unsafe_relative_path(raw)?;
        let resolved = validate_path(raw, Some(&self.root))
            .map_err(|error| filesystem_unavailable(error.to_string()))?;

        let guarded = if resolved.exists() {
            resolved
                .canonicalize()
                .map_err(|error| filesystem_unavailable(error.to_string()))?
        } else if expectation == PathExpectation::MayCreateLeaf {
            let parent = resolved
                .parent()
                .ok_or_else(|| filesystem_unavailable("filesystem path has no parent"))?;
            let parent = parent
                .canonicalize()
                .map_err(|error| filesystem_unavailable(error.to_string()))?;
            parent.join(
                resolved
                    .file_name()
                    .ok_or_else(|| filesystem_unavailable("filesystem path has no file name"))?,
            )
        } else {
            return Err(filesystem_unavailable("filesystem path does not exist"));
        };

        if !guarded.starts_with(&self.root) {
            return Err(filesystem_unavailable(
                "filesystem path is outside service root",
            ));
        }
        Ok(guarded)
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathExpectation {
    MustExist,
    MayCreateLeaf,
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
        let path = self.resolve_existing(&params.path)?;
        let bytes = fs::read(path).map_err(|error| filesystem_unavailable(error.to_string()))?;
        let start = params.offset.unwrap_or(0).min(bytes.len());
        let end = params
            .length
            .map(|length| start.saturating_add(length).min(bytes.len()))
            .unwrap_or(bytes.len());

        Ok(FsReadFileResponse {
            data_base64: base64::engine::general_purpose::STANDARD.encode(&bytes[start..end]),
        })
    }

    fn write_file(&self, params: FsWriteFileParams) -> Result<FsWriteFileResponse, AppServerError> {
        let path = self.resolve_for_create(&params.path)?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(params.data_base64)
            .map_err(|error| filesystem_unavailable(error.to_string()))?;

        let mode = params.mode.unwrap_or_default();
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
    }

    fn create_directory(
        &self,
        params: FsCreateDirectoryParams,
    ) -> Result<FsCreateDirectoryResponse, AppServerError> {
        let path = self.resolve_for_create(&params.path)?;
        if params.recursive.unwrap_or(false) {
            fs::create_dir_all(path)
        } else {
            fs::create_dir(path)
        }
        .map_err(|error| filesystem_unavailable(error.to_string()))?;
        self.poll_watches();
        Ok(FsCreateDirectoryResponse {})
    }

    fn get_metadata(
        &self,
        params: FsGetMetadataParams,
    ) -> Result<FsGetMetadataResponse, AppServerError> {
        let path = self.resolve_existing(&params.path)?;
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| filesystem_unavailable(error.to_string()))?;
        Ok(FsGetMetadataResponse {
            is_file: metadata.is_file(),
            is_directory: metadata.is_dir(),
            is_symlink: metadata.file_type().is_symlink(),
            created_at_ms: system_time_ms(metadata.created().unwrap_or(UNIX_EPOCH)),
            modified_at_ms: system_time_ms(metadata.modified().unwrap_or(UNIX_EPOCH)),
        })
    }

    fn read_directory(
        &self,
        params: FsReadDirectoryParams,
    ) -> Result<FsReadDirectoryResponse, AppServerError> {
        let path = self.resolve_existing(&params.path)?;
        let mut entries = Vec::new();
        for entry in
            fs::read_dir(path).map_err(|error| filesystem_unavailable(error.to_string()))?
        {
            let entry = entry.map_err(|error| filesystem_unavailable(error.to_string()))?;
            let metadata = entry
                .metadata()
                .map_err(|error| filesystem_unavailable(error.to_string()))?;
            entries.push(FsReadDirectoryEntry {
                file_name: entry.file_name().to_string_lossy().to_string(),
                is_file: metadata.is_file(),
                is_directory: metadata.is_dir(),
            });
        }
        entries.sort_by(|left, right| left.file_name.cmp(&right.file_name));
        Ok(FsReadDirectoryResponse { entries })
    }

    fn remove(&self, params: FsRemoveParams) -> Result<FsRemoveResponse, AppServerError> {
        let path = match self.resolve_existing(&params.path) {
            Ok(path) => path,
            Err(error) if params.force.unwrap_or(false) => {
                if matches_capability_unavailable(&error) {
                    return Ok(FsRemoveResponse {});
                }
                return Err(error);
            }
            Err(error) => return Err(error),
        };

        if path.is_dir() {
            if params.recursive.unwrap_or(false) {
                fs::remove_dir_all(path)
            } else {
                fs::remove_dir(path)
            }
        } else {
            fs::remove_file(path)
        }
        .map_err(|error| filesystem_unavailable(error.to_string()))?;
        self.poll_watches();
        Ok(FsRemoveResponse {})
    }

    fn copy(&self, params: FsCopyParams) -> Result<FsCopyResponse, AppServerError> {
        let source = self.resolve_existing(&params.source_path)?;
        let destination = self.resolve_for_create(&params.destination_path)?;

        if source.is_dir() {
            if !params.recursive.unwrap_or(false) {
                return Err(filesystem_unavailable(
                    "recursive=true is required for directory copies",
                ));
            }
            copy_dir_recursive(&source, &destination)?;
        } else {
            fs::copy(source, destination)
                .map_err(|error| filesystem_unavailable(error.to_string()))?;
        }
        self.poll_watches();
        Ok(FsCopyResponse {})
    }

    fn watch(&self, params: FsWatchParams) -> Result<FsWatchResponse, AppServerError> {
        let path = self.resolve_existing(&params.path)?;
        let watch = FsWatch {
            display_path: params.path.clone(),
            last_state: Self::metadata_state(&path),
            path,
        };
        self.watches
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .insert(params.watch_id, watch);
        Ok(FsWatchResponse { path: params.path })
    }

    fn unwatch(&self, params: FsUnwatchParams) -> Result<FsUnwatchResponse, AppServerError> {
        self.watches
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .remove(&params.watch_id);
        Ok(FsUnwatchResponse {})
    }

    fn drain_changed_events(&self) -> Vec<FsChangedNotification> {
        self.poll_watches();
        self.events
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .drain(..)
            .collect()
    }
}

fn reject_unsafe_relative_path(raw: &str) -> Result<(), AppServerError> {
    if raw.as_bytes().contains(&0) {
        return Err(filesystem_unavailable(
            "filesystem path contains a null byte",
        ));
    }

    let path = Path::new(raw);
    if path.is_absolute() {
        return Err(filesystem_unavailable(
            "absolute filesystem paths are not allowed",
        ));
    }

    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::Prefix(_) | Component::RootDir
        )
    }) {
        return Err(filesystem_unavailable(
            "filesystem path traversal is not allowed",
        ));
    }

    Ok(())
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

fn matches_capability_unavailable(error: &AppServerError) -> bool {
    matches!(
        error,
        AppServerError::Protocol { data }
            if data.code == dasclaw_app_server_protocol::ErrorCode::CapabilityUnavailable
    )
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
