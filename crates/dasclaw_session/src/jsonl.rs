//! On-disk [`SessionStore`] backed by one JSON-lines file per session.
//!
//! ## Wire format
//!
//! Each session lives in `<root>/<session_id>.jsonl`. The file is a
//! sequence of newline-terminated JSON objects; the first line is the
//! [`SessionMetaRecord`] and the rest are [`MessageRecord`] entries:
//!
//! ```text
//! {"type":"session_meta","version":1,"session_id":"…","created_at_ms":…,"updated_at_ms":…,"workspace_root":"…","model":"…"}
//! {"type":"message","message":{"role":"user","content":"ping"}}
//! {"type":"message","message":{"role":"assistant","content":"pong"}}
//! ```
//!
//! The outer framing (`type` discriminant, `session_meta` and
//! `message` record kinds) deliberately mirrors `claw-code`'s session
//! jsonl layout so a future migration tool can bridge the two
//! ecosystems at the line level. The inner `message` body uses
//! [`dasclaw_core::messages::ChatMessage`]'s serde shape, which differs
//! from `claw-code`'s richer block-based message; full bidirectional
//! interop is intentionally out of scope for #914 PR-C1.
//!
//! ## Rotation
//!
//! Before each [`SessionStore::save`], if the existing live file
//! exceeds [`ROTATE_AFTER_BYTES`] it is renamed to
//! `<session_id>.rot-<timestamp_ms>.jsonl`. After the new live file is
//! written, the rotated history is trimmed so at most
//! [`MAX_ROTATED_FILES`] old files remain (oldest by mtime are
//! removed first). This mirrors `claw-code/rust/crates/runtime/src/session.rs`.
//!
//! ## Atomicity
//!
//! Saves are atomic: bytes are written to a unique temp file under
//! the same directory and then renamed onto the live path. Concurrent
//! readers either see the previous full snapshot or the new one,
//! never a partial write.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};

use dasclaw_core::messages::ChatMessage;

use crate::error::SessionError;
use crate::id::SESSION_VERSION;
use crate::snapshot::{
    SessionCompaction, SessionFork, SessionMetadata, SessionPromptEntry, SessionSnapshot,
};
use crate::store::SessionStore;

/// Live files larger than this trigger a rotate on the next save.
pub const ROTATE_AFTER_BYTES: u64 = 256 * 1024;

/// Maximum number of rotated history files kept per session.
pub const MAX_ROTATED_FILES: usize = 3;

const FILE_EXT: &str = "jsonl";
const ROTATED_INFIX: &str = ".rot-";

/// On-disk [`SessionStore`] writing one `.jsonl` file per session
/// under a configured root directory.
///
/// The root is created lazily on first save. The store holds no I/O
/// handles between calls, so it is cheap to clone (wrap in `Arc` if
/// shared across tasks).
pub struct JsonlSessionStore {
    root: PathBuf,
    temp_counter: AtomicU64,
}

impl JsonlSessionStore {
    /// Build a store rooted at `root`. The directory does not need to
    /// exist yet; it will be created on the first [`SessionStore::save`].
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            temp_counter: AtomicU64::new(0),
        }
    }

    /// Path the live `.jsonl` file for `session_id` would occupy.
    /// Exposed for tests / debug; not part of the public contract.
    #[must_use]
    pub fn session_path(&self, session_id: &str) -> PathBuf {
        self.root.join(format!("{session_id}.{FILE_EXT}"))
    }

    fn temp_path_for(&self, live: &Path) -> PathBuf {
        let stem = live
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("session");
        let counter = self.temp_counter.fetch_add(1, Ordering::Relaxed);
        live.with_file_name(format!("{stem}.tmp-{}-{counter}", current_time_millis()))
    }
}

#[async_trait]
impl SessionStore for JsonlSessionStore {
    async fn save(&self, snapshot: &SessionSnapshot) -> Result<(), SessionError> {
        fs::create_dir_all(&self.root).await?;

        let live = self.session_path(&snapshot.session_id);
        rotate_if_needed(&live).await?;

        let rendered = render_jsonl(snapshot)?;
        let temp = self.temp_path_for(&live);
        fs::write(&temp, rendered).await?;
        // tokio::fs::rename is atomic on POSIX; on Windows it falls
        // back to MoveFileEx which is also atomic when both paths
        // share a filesystem (the case here, since temp sits next to
        // the live file).
        fs::rename(&temp, &live).await?;

        cleanup_rotated(&live).await?;
        Ok(())
    }

    async fn load(&self, session_id: &str) -> Result<Option<SessionSnapshot>, SessionError> {
        let live = self.session_path(session_id);
        let bytes = match fs::read(&live).await {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        let snap = parse_jsonl(&bytes)?;
        Ok(Some(snap))
    }

    async fn list(&self) -> Result<Vec<SessionMetadata>, SessionError> {
        let mut out = Vec::new();
        let mut entries = match fs::read_dir(&self.root).await {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(err) => return Err(err.into()),
        };
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if !is_live_session_file(&path) {
                continue;
            }
            if let Some(meta) = read_metadata_only(&path).await? {
                out.push(meta);
            }
        }
        Ok(out)
    }

    async fn delete(&self, session_id: &str) -> Result<(), SessionError> {
        let live = self.session_path(session_id);
        match fs::remove_file(&live).await {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
        // Also remove any rotated history for this session id.
        let stem = session_id.to_string();
        let prefix = format!("{stem}{ROTATED_INFIX}");
        let mut entries = match fs::read_dir(&self.root).await {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err.into()),
        };
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if is_rotated_for(&path, &prefix) {
                fs::remove_file(&path).await?;
            }
        }
        Ok(())
    }
}

// -------------------------------------------------------------------
// Wire records
// -------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct SessionMetaRecord<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    version: u32,
    session_id: &'a str,
    created_at_ms: u64,
    updated_at_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace_root: Option<&'a Path>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fork: Option<&'a SessionFork>,
}

#[derive(Debug, Serialize)]
struct MessageRecord<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    message: &'a ChatMessage,
}

#[derive(Debug, Serialize)]
struct CompactionRecord<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    count: u32,
    removed_message_count: usize,
    summary: &'a str,
}

#[derive(Debug, Serialize)]
struct PromptHistoryRecord<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    timestamp_ms: u64,
    text: &'a str,
}

fn render_jsonl(snapshot: &SessionSnapshot) -> Result<Vec<u8>, SessionError> {
    let meta = SessionMetaRecord {
        kind: "session_meta",
        version: snapshot.version,
        session_id: &snapshot.session_id,
        created_at_ms: snapshot.created_at_ms,
        updated_at_ms: snapshot.updated_at_ms,
        workspace_root: snapshot.workspace_root.as_deref(),
        model: snapshot.model.as_deref(),
        fork: snapshot.fork.as_ref(),
    };

    let mut out = Vec::with_capacity(256 + snapshot.messages.len() * 128);
    serde_json::to_writer(&mut out, &meta)?;
    out.push(b'\n');
    if let Some(compaction) = &snapshot.compaction {
        let record = CompactionRecord {
            kind: "compaction",
            count: compaction.count,
            removed_message_count: compaction.removed_message_count,
            summary: &compaction.summary,
        };
        serde_json::to_writer(&mut out, &record)?;
        out.push(b'\n');
    }
    for entry in &snapshot.prompt_history {
        let record = PromptHistoryRecord {
            kind: "prompt_history",
            timestamp_ms: entry.timestamp_ms,
            text: &entry.text,
        };
        serde_json::to_writer(&mut out, &record)?;
        out.push(b'\n');
    }
    for message in &snapshot.messages {
        let record = MessageRecord {
            kind: "message",
            message,
        };
        serde_json::to_writer(&mut out, &record)?;
        out.push(b'\n');
    }
    Ok(out)
}

fn parse_jsonl(bytes: &[u8]) -> Result<SessionSnapshot, SessionError> {
    let text = std::str::from_utf8(bytes).map_err(|err| {
        SessionError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, err))
    })?;

    let mut version = None;
    let mut session_id = None;
    let mut created_at_ms = None;
    let mut updated_at_ms = None;
    let mut workspace_root = None;
    let mut model = None;
    let mut compaction: Option<SessionCompaction> = None;
    let mut fork: Option<SessionFork> = None;
    let mut prompt_history: Vec<SessionPromptEntry> = Vec::new();
    let mut messages: Vec<ChatMessage> = Vec::new();

    for (idx, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line)?;
        let object = value.as_object().ok_or_else(|| {
            SessionError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("jsonl record at line {} must be an object", idx + 1),
            ))
        })?;
        let kind = object.get("type").and_then(Value::as_str).ok_or_else(|| {
            SessionError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("jsonl record at line {} missing \"type\"", idx + 1),
            ))
        })?;

        match kind {
            "session_meta" => {
                let v = object
                    .get("version")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| {
                        SessionError::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "session_meta missing \"version\"",
                        ))
                    })?;
                let v = u32::try_from(v).map_err(|_| {
                    SessionError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "session_meta version out of u32 range",
                    ))
                })?;
                if v > SESSION_VERSION {
                    return Err(SessionError::UnsupportedVersion {
                        found: v,
                        supported: SESSION_VERSION,
                    });
                }
                version = Some(v);
                session_id = Some(
                    object
                        .get("session_id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            SessionError::Io(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "session_meta missing \"session_id\"",
                            ))
                        })?
                        .to_string(),
                );
                created_at_ms = object.get("created_at_ms").and_then(Value::as_u64);
                updated_at_ms = object.get("updated_at_ms").and_then(Value::as_u64);
                workspace_root = object
                    .get("workspace_root")
                    .and_then(Value::as_str)
                    .map(PathBuf::from);
                model = object
                    .get("model")
                    .and_then(Value::as_str)
                    .map(String::from);
                fork = object
                    .get("fork")
                    .map(|v| serde_json::from_value::<SessionFork>(v.clone()))
                    .transpose()?;
            }
            "message" => {
                let message_value = object.get("message").ok_or_else(|| {
                    SessionError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "jsonl message record at line {} missing \"message\"",
                            idx + 1
                        ),
                    ))
                })?;
                let msg: ChatMessage = serde_json::from_value(message_value.clone())?;
                messages.push(msg);
            }
            "compaction" => {
                let count_raw = object.get("count").and_then(Value::as_u64).ok_or_else(|| {
                    SessionError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("compaction record at line {} missing \"count\"", idx + 1),
                    ))
                })?;
                let count = u32::try_from(count_raw).map_err(|_| {
                    SessionError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "compaction.count out of u32 range",
                    ))
                })?;
                let removed_raw = object
                    .get("removed_message_count")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| {
                        SessionError::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!(
                                "compaction record at line {} missing \"removed_message_count\"",
                                idx + 1
                            ),
                        ))
                    })?;
                let removed_message_count = usize::try_from(removed_raw).map_err(|_| {
                    SessionError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "compaction.removed_message_count out of usize range",
                    ))
                })?;
                let summary = object
                    .get("summary")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        SessionError::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("compaction record at line {} missing \"summary\"", idx + 1),
                        ))
                    })?
                    .to_string();
                compaction = Some(SessionCompaction {
                    count,
                    removed_message_count,
                    summary,
                });
            }
            "prompt_history" => {
                let ts = object
                    .get("timestamp_ms")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| {
                        SessionError::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!(
                                "prompt_history record at line {} missing \"timestamp_ms\"",
                                idx + 1
                            ),
                        ))
                    })?;
                let text = object
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        SessionError::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("prompt_history record at line {} missing \"text\"", idx + 1),
                        ))
                    })?
                    .to_string();
                prompt_history.push(SessionPromptEntry {
                    timestamp_ms: ts,
                    text,
                });
            }
            other => {
                return Err(SessionError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unsupported jsonl record type at line {}: {other}", idx + 1),
                )));
            }
        }
    }

    let session_id = session_id.ok_or_else(|| {
        SessionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "jsonl missing session_meta record",
        ))
    })?;
    let version = version.unwrap_or(SESSION_VERSION);
    let created_at_ms = created_at_ms.unwrap_or(0);
    let updated_at_ms = updated_at_ms.unwrap_or(created_at_ms);

    Ok(SessionSnapshot {
        session_id,
        version,
        created_at_ms,
        updated_at_ms,
        workspace_root,
        model,
        compaction,
        fork,
        prompt_history,
        messages,
    })
}

/// Read just the first line of a session file (the `session_meta`
/// record) and project it into a [`SessionMetadata`]. Used by
/// [`SessionStore::list`] so the index does not pay for full message
/// deserialization.
async fn read_metadata_only(path: &Path) -> Result<Option<SessionMetadata>, SessionError> {
    let file = match fs::File::open(path).await {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    let mut reader = BufReader::new(file);
    let mut first_line = String::new();
    let bytes = reader.read_line(&mut first_line).await?;
    if bytes == 0 {
        return Ok(None);
    }
    let line = first_line.trim();
    if line.is_empty() {
        return Ok(None);
    }
    let value: HashMap<String, Value> = serde_json::from_str(line)?;
    if value.get("type").and_then(Value::as_str) != Some("session_meta") {
        return Ok(None);
    }
    let raw_version = value
        .get("version")
        .and_then(Value::as_u64)
        .unwrap_or(u64::from(SESSION_VERSION));
    let version = u32::try_from(raw_version).unwrap_or(SESSION_VERSION);
    Ok(Some(SessionMetadata {
        session_id: value
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        version,
        created_at_ms: value
            .get("created_at_ms")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        updated_at_ms: value
            .get("updated_at_ms")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        model: value.get("model").and_then(Value::as_str).map(String::from),
    }))
}

// -------------------------------------------------------------------
// Rotation helpers
// -------------------------------------------------------------------

async fn rotate_if_needed(live: &Path) -> Result<(), SessionError> {
    let metadata = match fs::metadata(live).await {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    if metadata.len() < ROTATE_AFTER_BYTES {
        return Ok(());
    }
    let rotated = rotated_path_for(live);
    fs::rename(live, rotated).await?;
    Ok(())
}

fn rotated_path_for(live: &Path) -> PathBuf {
    let stem = live
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("session");
    live.with_file_name(format!(
        "{stem}{ROTATED_INFIX}{}.{FILE_EXT}",
        current_time_millis()
    ))
}

async fn cleanup_rotated(live: &Path) -> Result<(), SessionError> {
    let Some(parent) = live.parent() else {
        return Ok(());
    };
    let stem = live
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("session");
    let prefix = format!("{stem}{ROTATED_INFIX}");

    let mut rotated: Vec<(PathBuf, SystemTime)> = Vec::new();
    let mut entries = match fs::read_dir(parent).await {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if !is_rotated_for(&path, &prefix) {
            continue;
        }
        let modified = entry
            .metadata()
            .await
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(UNIX_EPOCH);
        rotated.push((path, modified));
    }

    if rotated.len() <= MAX_ROTATED_FILES {
        return Ok(());
    }
    rotated.sort_by_key(|(_, mtime)| *mtime);
    let to_remove = rotated.len() - MAX_ROTATED_FILES;
    for (path, _) in rotated.into_iter().take(to_remove) {
        fs::remove_file(path).await?;
    }
    Ok(())
}

fn is_live_session_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(OsStr::to_str) else {
        return false;
    };
    if name.contains(ROTATED_INFIX) {
        return false;
    }
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|ext| ext.eq_ignore_ascii_case(FILE_EXT))
}

fn is_rotated_for(path: &Path, prefix: &str) -> bool {
    let Some(name) = path.file_name().and_then(OsStr::to_str) else {
        return false;
    };
    name.starts_with(prefix)
        && path
            .extension()
            .and_then(OsStr::to_str)
            .is_some_and(|ext| ext.eq_ignore_ascii_case(FILE_EXT))
}

fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}
