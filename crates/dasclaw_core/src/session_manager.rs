//! Session manager for multi-user, multi-thread conversation handling.
//!
//! Maps external channel thread IDs to internal UUIDs and manages undo state
//! for each thread.
//!
//! Ported from ironclaw's `src/agent/session_manager.rs` (Phase 3 Step D-5,
//! plan variant C'). The behaviour is identical; the only change is that the
//! concrete `HookRegistry` dependency is replaced with the
//! [`SessionHooks`](crate::session_hooks::SessionHooks) trait so this module
//! lives in the `dasclaw_core` runtime crate.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::session::Session;
use crate::session_hooks::SessionHooks;
use crate::undo::UndoManager;

/// Warn when session count exceeds this threshold.
const SESSION_COUNT_WARNING_THRESHOLD: usize = 1000;

/// Key for mapping external thread IDs to internal ones.
#[derive(Clone, Hash, Eq, PartialEq)]
struct ThreadKey {
    user_id: String,
    channel: String,
    external_thread_id: Option<String>,
}

/// Manages sessions, threads, and undo state for all users.
pub struct SessionManager {
    sessions: RwLock<HashMap<String, Arc<Mutex<Session>>>>,
    thread_map: RwLock<HashMap<ThreadKey, Uuid>>,
    undo_managers: RwLock<HashMap<Uuid, Arc<Mutex<UndoManager>>>>,
    hooks: Option<Arc<dyn SessionHooks>>,
}

impl SessionManager {
    /// Create a new session manager.
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            thread_map: RwLock::new(HashMap::new()),
            undo_managers: RwLock::new(HashMap::new()),
            hooks: None,
        }
    }

    /// Attach a hook implementation for session lifecycle events.
    pub fn with_hooks(mut self, hooks: Arc<dyn SessionHooks>) -> Self {
        self.hooks = Some(hooks);
        self
    }

    /// Get or create a session for a user.
    pub async fn get_or_create_session(&self, user_id: &str) -> Arc<Mutex<Session>> {
        // Fast path: check if session exists
        {
            let sessions = self.sessions.read().await;
            if let Some(session) = sessions.get(user_id) {
                return Arc::clone(session);
            }
        }

        // Slow path: create new session
        let mut sessions = self.sessions.write().await;
        // Double-check after acquiring write lock
        if let Some(session) = sessions.get(user_id) {
            return Arc::clone(session);
        }

        let new_session = Session::new(user_id);
        let session_id = new_session.id.to_string();
        let session = Arc::new(Mutex::new(new_session));
        sessions.insert(user_id.to_string(), Arc::clone(&session));

        if sessions.len() >= SESSION_COUNT_WARNING_THRESHOLD && sessions.len() % 100 == 0 {
            tracing::warn!(
                "High session count: {} active sessions. \
                 Pruning runs every 10 minutes; consider reducing session_idle_timeout.",
                sessions.len()
            );
        }

        // Fire OnSessionStart hook (fire-and-forget)
        if let Some(ref hooks) = self.hooks {
            let hooks = Arc::clone(hooks);
            let uid = user_id.to_string();
            let sid = session_id;
            tokio::spawn(async move {
                hooks.on_session_start(&uid, &sid).await;
            });
        }

        session
    }

    /// Resolve an external thread ID to an internal thread.
    ///
    /// Returns the session and thread ID. Creates both if they don't exist.
    /// Delegates to [`resolve_thread_with_parsed_uuid`](Self::resolve_thread_with_parsed_uuid)
    /// with `parsed_uuid: None`.
    pub async fn resolve_thread(
        &self,
        user_id: &str,
        channel: &str,
        external_thread_id: Option<&str>,
    ) -> (Arc<Mutex<Session>>, Uuid) {
        self.resolve_thread_with_parsed_uuid(user_id, channel, external_thread_id, None)
            .await
    }

    /// Like [`resolve_thread`](Self::resolve_thread), but accepts a pre-parsed
    /// UUID to skip redundant parsing when the caller has already validated
    /// the external thread ID as a UUID (e.g. the approval routing path).
    ///
    /// Uses a single read-lock acquisition for both the key lookup and the UUID
    /// adoption check to reduce contention under concurrent approval load.
    pub async fn resolve_thread_with_parsed_uuid(
        &self,
        user_id: &str,
        channel: &str,
        external_thread_id: Option<&str>,
        parsed_uuid: Option<Uuid>,
    ) -> (Arc<Mutex<Session>>, Uuid) {
        let session = self.get_or_create_session(user_id).await;

        let key = ThreadKey {
            user_id: user_id.to_string(),
            channel: channel.to_string(),
            external_thread_id: external_thread_id.map(String::from),
        };

        // Use pre-parsed UUID if available, otherwise parse from string.
        let ext_uuid = parsed_uuid
            .or_else(|| external_thread_id.and_then(|ext_tid| Uuid::parse_str(ext_tid).ok()));

        // Validate that parsed_uuid (if provided) is consistent with external_thread_id.
        #[cfg(debug_assertions)]
        if let (Some(parsed), Some(ext_tid)) = (&parsed_uuid, external_thread_id) {
            debug_assert_eq!(
                Uuid::parse_str(ext_tid).ok().as_ref(),
                Some(parsed),
                "parsed_uuid must be the parsed form of external_thread_id"
            );
        }

        // Single read lock for both the key lookup and UUID adoption check
        let adoptable_uuid = {
            let thread_map = self.thread_map.read().await;

            // Fast path: exact key match
            if let Some(&thread_id) = thread_map.get(&key) {
                let sess = session.lock().await;
                if sess.threads.contains_key(&thread_id) {
                    return (Arc::clone(&session), thread_id);
                }
            }

            // UUID adoption check (still under the same read lock).
            // If external_thread_id is a valid UUID not mapped elsewhere,
            // it may be a thread created by chat_new_thread_handler or
            // hydrated from DB that we can adopt.
            // Only attempt adoption when external_thread_id is Some, preserving
            // the invariant that None external_thread_id never triggers adoption.
            if external_thread_id.is_some() {
                ext_uuid.filter(|&uuid| !thread_map.values().any(|&v| v == uuid))
            } else {
                None
            }
        }; // Single read lock dropped here

        // If we found an adoptable UUID, verify it exists in session and acquire write lock
        if let Some(ext_uuid) = adoptable_uuid {
            let sess = session.lock().await;
            if sess.threads.contains_key(&ext_uuid) {
                drop(sess);

                let mut thread_map = self.thread_map.write().await;
                // Re-check after acquiring write lock to prevent race condition
                // where another task mapped this UUID between our read and write.
                if !thread_map.values().any(|&v| v == ext_uuid) {
                    thread_map.insert(key, ext_uuid);
                    drop(thread_map);
                    // Ensure undo manager exists
                    let mut undo_managers = self.undo_managers.write().await;
                    undo_managers
                        .entry(ext_uuid)
                        .or_insert_with(|| Arc::new(Mutex::new(UndoManager::new())));
                    return (session, ext_uuid);
                }
                // If mapped elsewhere while unlocked, fall through to create new thread
            }
        }

        // Create new thread (always create a new one for a new key)
        let thread_id = {
            let mut sess = session.lock().await;
            let thread = sess.create_thread();
            thread.id
        };

        // Store mapping
        {
            let mut thread_map = self.thread_map.write().await;
            thread_map.insert(key, thread_id);
        }

        // Create undo manager for thread
        {
            let mut undo_managers = self.undo_managers.write().await;
            undo_managers.insert(thread_id, Arc::new(Mutex::new(UndoManager::new())));
        }

        (session, thread_id)
    }

    /// Register a hydrated thread so subsequent `resolve_thread` calls find it.
    ///
    /// Inserts into the thread_map and creates an undo manager for the thread.
    pub async fn register_thread(
        &self,
        user_id: &str,
        channel: &str,
        thread_id: Uuid,
        session: Arc<Mutex<Session>>,
    ) {
        let key = ThreadKey {
            user_id: user_id.to_string(),
            channel: channel.to_string(),
            external_thread_id: Some(thread_id.to_string()),
        };

        {
            let mut thread_map = self.thread_map.write().await;
            thread_map.insert(key, thread_id);
        }

        {
            let mut undo_managers = self.undo_managers.write().await;
            undo_managers
                .entry(thread_id)
                .or_insert_with(|| Arc::new(Mutex::new(UndoManager::new())));
        }

        // Ensure the session is tracked
        {
            let mut sessions = self.sessions.write().await;
            sessions.entry(user_id.to_string()).or_insert(session);
        }
    }

    /// Get undo manager for a thread.
    pub async fn get_undo_manager(&self, thread_id: Uuid) -> Arc<Mutex<UndoManager>> {
        // Fast path
        {
            let managers = self.undo_managers.read().await;
            if let Some(mgr) = managers.get(&thread_id) {
                return Arc::clone(mgr);
            }
        }

        // Create if missing
        let mut managers = self.undo_managers.write().await;
        // Double-check
        if let Some(mgr) = managers.get(&thread_id) {
            return Arc::clone(mgr);
        }

        let mgr = Arc::new(Mutex::new(UndoManager::new()));
        managers.insert(thread_id, Arc::clone(&mgr));
        mgr
    }

    /// Remove sessions that have been idle for longer than the given duration.
    ///
    /// Returns the number of sessions pruned.
    pub async fn prune_stale_sessions(&self, max_idle: std::time::Duration) -> usize {
        let cutoff = chrono::Utc::now() - chrono::TimeDelta::seconds(max_idle.as_secs() as i64);

        // Find stale sessions (user_id + session_id)
        let stale_sessions: Vec<(String, String)> = {
            let sessions = self.sessions.read().await;
            sessions
                .iter()
                .filter_map(|(user_id, session)| {
                    // Try to lock; skip if contended (someone is actively using it)
                    let sess = session.try_lock().ok()?;
                    if sess.last_active_at < cutoff {
                        Some((user_id.clone(), sess.id.to_string()))
                    } else {
                        None
                    }
                })
                .collect()
        };

        let stale_users: Vec<String> = stale_sessions
            .iter()
            .map(|(user_id, _)| user_id.clone())
            .collect();

        if stale_users.is_empty() {
            return 0;
        }

        // Collect thread IDs from stale sessions for cleanup
        let mut stale_thread_ids: Vec<Uuid> = Vec::new();
        {
            let sessions = self.sessions.read().await;
            for user_id in &stale_users {
                if let Some(session) = sessions.get(user_id)
                    && let Ok(sess) = session.try_lock()
                {
                    stale_thread_ids.extend(sess.threads.keys());
                }
            }
        }

        // Fire OnSessionEnd hooks for stale sessions (fire-and-forget)
        if let Some(ref hooks) = self.hooks {
            for (user_id, session_id) in &stale_sessions {
                let hooks = Arc::clone(hooks);
                let uid = user_id.clone();
                let sid = session_id.clone();
                tokio::spawn(async move {
                    hooks.on_session_end(&uid, &sid).await;
                });
            }
        }

        // Remove sessions
        let count = {
            let mut sessions = self.sessions.write().await;
            let before = sessions.len();
            for user_id in &stale_users {
                sessions.remove(user_id);
            }
            before - sessions.len()
        };

        // Clean up thread mappings that point to stale sessions
        {
            let mut thread_map = self.thread_map.write().await;
            thread_map.retain(|key, _| !stale_users.contains(&key.user_id));
        }

        // Clean up undo managers for stale threads
        {
            let mut undo_managers = self.undo_managers.write().await;
            for thread_id in &stale_thread_ids {
                undo_managers.remove(thread_id);
            }
        }

        if count > 0 {
            tracing::info!(
                "Pruned {} stale session(s) (idle > {}s)",
                count,
                max_idle.as_secs()
            );
        }

        count
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{Session, Thread};
    use crate::session_hooks::SessionHooks;
    use async_trait::async_trait;

    /// Capturing hook implementation for validating fire-and-forget wiring.
    struct CapturingHooks {
        events: Mutex<Vec<(String, String, String)>>, // (kind, user, session)
    }

    impl CapturingHooks {
        fn new() -> Self {
            Self {
                events: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl SessionHooks for CapturingHooks {
        async fn on_session_start(&self, user_id: &str, session_id: &str) {
            self.events.lock().await.push((
                "start".to_string(),
                user_id.to_string(),
                session_id.to_string(),
            ));
        }
        async fn on_session_end(&self, user_id: &str, session_id: &str) {
            self.events.lock().await.push((
                "end".to_string(),
                user_id.to_string(),
                session_id.to_string(),
            ));
        }
    }

    #[tokio::test]
    async fn test_get_or_create_session() {
        let manager = SessionManager::new();

        let session1 = manager.get_or_create_session("user-1").await;
        let session2 = manager.get_or_create_session("user-1").await;

        // Same user should get same session
        assert!(Arc::ptr_eq(&session1, &session2));

        let session3 = manager.get_or_create_session("user-2").await;
        assert!(!Arc::ptr_eq(&session1, &session3));
    }

    #[tokio::test]
    async fn test_resolve_thread() {
        let manager = SessionManager::new();

        let (session1, thread1) = manager.resolve_thread("user-1", "cli", None).await;
        let (session2, thread2) = manager.resolve_thread("user-1", "cli", None).await;

        // Same channel+user should get same thread
        assert!(Arc::ptr_eq(&session1, &session2));
        assert_eq!(thread1, thread2);

        // Different channel should get different thread
        let (_, thread3) = manager.resolve_thread("user-1", "http", None).await;
        assert_ne!(thread1, thread3);
    }

    #[tokio::test]
    async fn test_undo_manager() {
        let manager = SessionManager::new();
        let (_, thread_id) = manager.resolve_thread("user-1", "cli", None).await;

        let undo1 = manager.get_undo_manager(thread_id).await;
        let undo2 = manager.get_undo_manager(thread_id).await;

        assert!(Arc::ptr_eq(&undo1, &undo2));
    }

    #[tokio::test]
    async fn test_prune_stale_sessions() {
        let manager = SessionManager::new();

        // Create two sessions and resolve threads (which updates last_active_at)
        let (_, _thread_id) = manager.resolve_thread("user-active", "cli", None).await;
        let (s2, _thread_id) = manager.resolve_thread("user-stale", "cli", None).await;

        // Backdate the stale session's last_active_at AFTER thread creation
        {
            let mut sess = s2.lock().await;
            sess.last_active_at = chrono::Utc::now() - chrono::TimeDelta::seconds(86400 * 10); // 10 days ago
        }

        // Prune with 7-day timeout
        let pruned = manager
            .prune_stale_sessions(std::time::Duration::from_secs(86400 * 7))
            .await;
        assert_eq!(pruned, 1);

        // Active session should still exist
        let sessions = manager.sessions.read().await;
        assert!(sessions.contains_key("user-active"));
        assert!(!sessions.contains_key("user-stale"));
    }

    #[tokio::test]
    async fn test_prune_no_stale_sessions() {
        let manager = SessionManager::new();
        let _s1 = manager.get_or_create_session("user-1").await;

        // Nothing should be pruned when timeout is long
        let pruned = manager
            .prune_stale_sessions(std::time::Duration::from_secs(86400 * 365))
            .await;
        assert_eq!(pruned, 0);
    }

    #[tokio::test]
    async fn test_register_thread() {
        let manager = SessionManager::new();
        let thread_id = Uuid::new_v4();

        // Create a session with a hydrated thread
        let session = Arc::new(Mutex::new(Session::new("user-hydrate")));
        {
            let mut sess = session.lock().await;
            let thread = Thread::with_id(thread_id, sess.id);
            sess.threads.insert(thread_id, thread);
            sess.active_thread = Some(thread_id);
        }

        // Register the thread
        manager
            .register_thread("user-hydrate", "gateway", thread_id, Arc::clone(&session))
            .await;

        // resolve_thread should find it (using the UUID as external_thread_id)
        let (resolved_session, resolved_tid) = manager
            .resolve_thread("user-hydrate", "gateway", Some(&thread_id.to_string()))
            .await;
        assert_eq!(resolved_tid, thread_id);

        // Should be the same session object
        let sess = resolved_session.lock().await;
        assert!(sess.threads.contains_key(&thread_id));
    }

    #[tokio::test]
    async fn test_resolve_thread_with_explicit_external_id() {
        let manager = SessionManager::new();

        let (_, t1) = manager
            .resolve_thread("user-1", "gateway", Some("ext-abc"))
            .await;
        let (_, t2) = manager
            .resolve_thread("user-1", "gateway", Some("ext-abc"))
            .await;
        assert_eq!(t1, t2);

        let (_, t3) = manager
            .resolve_thread("user-1", "gateway", Some("ext-xyz"))
            .await;
        assert_ne!(t1, t3);
    }

    #[tokio::test]
    async fn test_resolve_thread_none_vs_some_external_id() {
        let manager = SessionManager::new();

        let (_, t_none) = manager.resolve_thread("user-1", "cli", None).await;
        let (_, t_some) = manager.resolve_thread("user-1", "cli", Some("ext-1")).await;
        assert_ne!(t_none, t_some);
    }

    #[tokio::test]
    async fn test_resolve_thread_different_users_isolated() {
        let manager = SessionManager::new();

        let (_, t1) = manager
            .resolve_thread("user-a", "gateway", Some("same-ext"))
            .await;
        let (_, t2) = manager
            .resolve_thread("user-b", "gateway", Some("same-ext"))
            .await;

        assert_ne!(t1, t2);
    }

    #[tokio::test]
    async fn test_resolve_thread_different_channels_isolated() {
        let manager = SessionManager::new();

        let (_, t1) = manager
            .resolve_thread("user-1", "gateway", Some("thread-x"))
            .await;
        let (_, t2) = manager
            .resolve_thread("user-1", "telegram", Some("thread-x"))
            .await;

        assert_ne!(t1, t2);
    }

    #[tokio::test]
    async fn test_resolve_thread_stale_mapping_creates_new_thread() {
        let manager = SessionManager::new();

        let (session, original_tid) = manager
            .resolve_thread("user-1", "gateway", Some("ext-1"))
            .await;

        {
            let mut sess = session.lock().await;
            sess.threads.remove(&original_tid);
        }

        let (_, new_tid) = manager
            .resolve_thread("user-1", "gateway", Some("ext-1"))
            .await;
        assert_ne!(original_tid, new_tid);

        let sess = session.lock().await;
        assert!(sess.threads.contains_key(&new_tid));
    }

    #[tokio::test]
    async fn test_register_thread_preserves_uuid_on_resolve() {
        let manager = SessionManager::new();
        let known_uuid = Uuid::new_v4();

        let session = Arc::new(Mutex::new(Session::new("user-web")));
        let session_id = {
            let sess = session.lock().await;
            sess.id
        };

        {
            let mut sess = session.lock().await;
            let thread = Thread::with_id(known_uuid, session_id);
            sess.threads.insert(known_uuid, thread);
        }

        manager
            .register_thread("user-web", "gateway", known_uuid, Arc::clone(&session))
            .await;

        let (_, resolved) = manager
            .resolve_thread("user-web", "gateway", Some(&known_uuid.to_string()))
            .await;
        assert_eq!(resolved, known_uuid);
    }

    #[tokio::test]
    async fn test_register_thread_idempotent() {
        let manager = SessionManager::new();
        let tid = Uuid::new_v4();

        let session = Arc::new(Mutex::new(Session::new("user-idem")));
        {
            let mut sess = session.lock().await;
            let thread = Thread::with_id(tid, sess.id);
            sess.threads.insert(tid, thread);
        }

        manager
            .register_thread("user-idem", "gateway", tid, Arc::clone(&session))
            .await;
        manager
            .register_thread("user-idem", "gateway", tid, Arc::clone(&session))
            .await;

        let (_, resolved) = manager
            .resolve_thread("user-idem", "gateway", Some(&tid.to_string()))
            .await;
        assert_eq!(resolved, tid);
    }

    #[tokio::test]
    async fn test_register_thread_creates_undo_manager() {
        let manager = SessionManager::new();
        let tid = Uuid::new_v4();

        let session = Arc::new(Mutex::new(Session::new("user-undo")));
        {
            let mut sess = session.lock().await;
            let thread = Thread::with_id(tid, sess.id);
            sess.threads.insert(tid, thread);
        }

        manager
            .register_thread("user-undo", "gateway", tid, Arc::clone(&session))
            .await;

        let undo = manager.get_undo_manager(tid).await;
        let undo2 = manager.get_undo_manager(tid).await;
        assert!(Arc::ptr_eq(&undo, &undo2));
    }

    #[tokio::test]
    async fn test_register_thread_stores_session() {
        let manager = SessionManager::new();
        let tid = Uuid::new_v4();

        let session = Arc::new(Mutex::new(Session::new("user-new")));
        {
            let mut sess = session.lock().await;
            let thread = Thread::with_id(tid, sess.id);
            sess.threads.insert(tid, thread);
        }

        {
            let sessions = manager.sessions.read().await;
            assert!(!sessions.contains_key("user-new"));
        }

        manager
            .register_thread("user-new", "gateway", tid, Arc::clone(&session))
            .await;

        {
            let sessions = manager.sessions.read().await;
            assert!(sessions.contains_key("user-new"));
        }
    }

    #[tokio::test]
    async fn test_multiple_threads_per_user() {
        let manager = SessionManager::new();

        let (_, t1) = manager
            .resolve_thread("user-1", "gateway", Some("thread-a"))
            .await;
        let (_, t2) = manager
            .resolve_thread("user-1", "gateway", Some("thread-b"))
            .await;
        let (session, t3) = manager
            .resolve_thread("user-1", "gateway", Some("thread-c"))
            .await;

        assert_ne!(t1, t2);
        assert_ne!(t2, t3);
        assert_ne!(t1, t3);

        let sess = session.lock().await;
        assert!(sess.threads.contains_key(&t1));
        assert!(sess.threads.contains_key(&t2));
        assert!(sess.threads.contains_key(&t3));
    }

    #[tokio::test]
    async fn test_prune_cleans_thread_map_and_undo_managers() {
        let manager = SessionManager::new();

        let (stale_session, stale_tid) = manager.resolve_thread("user-stale", "cli", None).await;

        {
            let mut sess = stale_session.lock().await;
            sess.last_active_at = chrono::Utc::now() - chrono::TimeDelta::seconds(86400 * 30);
        }

        {
            let tm = manager.thread_map.read().await;
            assert!(!tm.is_empty());
        }
        {
            let um = manager.undo_managers.read().await;
            assert!(um.contains_key(&stale_tid));
        }

        let pruned = manager
            .prune_stale_sessions(std::time::Duration::from_secs(86400 * 7))
            .await;
        assert_eq!(pruned, 1);

        {
            let tm = manager.thread_map.read().await;
            assert!(tm.is_empty());
        }
        {
            let um = manager.undo_managers.read().await;
            assert!(!um.contains_key(&stale_tid));
        }
    }

    #[tokio::test]
    async fn test_resolve_thread_active_thread_set() {
        let manager = SessionManager::new();

        let (session, thread_id) = manager
            .resolve_thread("user-1", "gateway", Some("ext-1"))
            .await;

        let sess = session.lock().await;
        assert_eq!(sess.active_thread, Some(thread_id));
    }

    // === Concurrent session stress tests ===

    #[tokio::test]
    async fn concurrent_get_or_create_same_user_returns_same_session() {
        let manager = Arc::new(SessionManager::new());

        let handles: Vec<_> = (0..30)
            .map(|_| {
                let mgr = Arc::clone(&manager);
                tokio::spawn(async move { mgr.get_or_create_session("shared-user").await })
            })
            .collect();

        let mut sessions = Vec::new();
        for handle in handles {
            sessions.push(handle.await.expect("task should not panic"));
        }

        for s in &sessions {
            assert!(Arc::ptr_eq(&sessions[0], s));
        }
    }

    #[tokio::test]
    async fn concurrent_resolve_thread_distinct_users_no_cross_talk() {
        let manager = Arc::new(SessionManager::new());

        let handles: Vec<_> = (0..20)
            .map(|i| {
                let mgr = Arc::clone(&manager);
                tokio::spawn(async move {
                    let user = format!("user-{i}");
                    let (session, tid) = mgr.resolve_thread(&user, "gateway", None).await;
                    (user, session, tid)
                })
            })
            .collect();

        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await.expect("task should not panic"));
        }

        let tids: std::collections::HashSet<_> = results.iter().map(|(_, _, t)| *t).collect();
        assert_eq!(tids.len(), 20);

        for (_, session, tid) in &results {
            let sess = session.lock().await;
            assert!(sess.threads.contains_key(tid));
            assert_eq!(sess.threads.len(), 1);
        }
    }

    #[tokio::test]
    async fn concurrent_get_undo_manager_same_thread_returns_same_arc() {
        let manager = Arc::new(SessionManager::new());
        let (_, tid) = manager.resolve_thread("undo-user", "gateway", None).await;

        let handles: Vec<_> = (0..20)
            .map(|_| {
                let mgr = Arc::clone(&manager);
                tokio::spawn(async move { mgr.get_undo_manager(tid).await })
            })
            .collect();

        let mut managers = Vec::new();
        for handle in handles {
            managers.push(handle.await.expect("task should not panic"));
        }

        for m in &managers {
            assert!(Arc::ptr_eq(&managers[0], m));
        }
    }

    // === SessionHooks wiring tests (new — validate trait abstraction) ===

    #[tokio::test]
    async fn test_session_start_hook_fires() {
        let hooks = Arc::new(CapturingHooks::new());
        let manager = SessionManager::new().with_hooks(hooks.clone() as Arc<dyn SessionHooks>);

        let _session = manager.get_or_create_session("alice").await;

        // Let the fire-and-forget tokio::spawn run.
        tokio::task::yield_now().await;
        // Give the spawned task time to record the event.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let events = hooks.events.lock().await;
        assert_eq!(events.len(), 1, "expected exactly one start event");
        assert_eq!(events[0].0, "start");
        assert_eq!(events[0].1, "alice");
    }

    #[tokio::test]
    async fn test_session_end_hook_fires_on_prune() {
        let hooks = Arc::new(CapturingHooks::new());
        let manager = SessionManager::new().with_hooks(hooks.clone() as Arc<dyn SessionHooks>);

        let (sess, _) = manager.resolve_thread("bob", "cli", None).await;

        {
            let mut s = sess.lock().await;
            s.last_active_at = chrono::Utc::now() - chrono::TimeDelta::seconds(86400 * 30);
        }

        let pruned = manager
            .prune_stale_sessions(std::time::Duration::from_secs(86400 * 7))
            .await;
        assert_eq!(pruned, 1);

        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let events = hooks.events.lock().await;
        // One start (from get_or_create_session) + one end (from prune)
        assert_eq!(events.len(), 2);
        assert!(events.iter().any(|(k, _, _)| k == "start"));
        assert!(events.iter().any(|(k, _, _)| k == "end"));
    }

    #[tokio::test]
    async fn test_no_hooks_no_panic() {
        // SessionManager without hooks must work normally — this guards against
        // a regression where the code might unwrap() on self.hooks.
        let manager = SessionManager::new();
        let _ = manager.get_or_create_session("no-hooks").await;
        let (_, _) = manager.resolve_thread("no-hooks", "cli", None).await;
        let pruned = manager
            .prune_stale_sessions(std::time::Duration::from_secs(1))
            .await;
        assert_eq!(pruned, 0);
    }
}
