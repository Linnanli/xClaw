//! Runtime hot-reload support for LLM providers.
//!
//! Ported verbatim (modulo import paths) from `ironclaw-main/src/llm/runtime.rs`
//! per ADR-118 / ADR-129. F3.1 (PR #627 / #637) ported 13 LLM files into
//! `dasclaw_llm_provider` but skipped `runtime.rs`; downstream that gap led
//! `desktop-client/src/model_switch.rs` to re-invent a thinner
//! `ModelSwitchProvider` (`Mutex<Arc<dyn LlmProvider>>` with no metadata
//! cache and an embedded `override_source` business field). This module is
//! the protocol-layer foundation that PRβ will use to collapse
//! `ModelSwitchProvider` into a thin wrapper around [`SwappableLlmProvider`].
//!
//! ## Design notes
//!
//! - **One snapshot lock.** All cached metadata (`model_name`,
//!   `active_model_name`, cost, cache multipliers, and the inner provider
//!   itself) live in a single `RwLock<ProviderSnapshot>`. A reader always
//!   observes a consistent slice of one provider — never a mix of old and
//!   new after a swap.
//! - **No unbounded leaks.** `model_name()` returns `&'static str` because
//!   the trait requires it; we intern each distinct name through a global
//!   `Mutex<HashMap>` so leakage is bounded by the set of distinct model
//!   names a process ever sees (typically a handful).
//! - **`set_model()` is volatile.** Runtime model switches are forwarded to
//!   the current inner provider only. The next successful
//!   [`LlmReloadHandle::reload`] rebuilds the chain from the caller-supplied
//!   factory and drops any override.
//! - **Protocol-only.** This module deliberately has no `LlmConfig` /
//!   `SessionManager` / `build_provider_chain_components` dependency.
//!   `LlmReloadHandle::reload` takes a builder closure so the application
//!   layer (desktop / admin / future agents) supplies the actual chain
//!   construction. Business-level override fields (e.g. `override_source`
//!   in `ModelSwitchProvider`) stay in their respective application crates.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::provider::error::LlmError;
use crate::provider::provider::{
    CompletionRequest, CompletionResponse, LlmProvider, LlmProviderCapabilities, LlmStream,
    ModelMetadata, ToolCompletionRequest, ToolCompletionResponse,
};

/// Maximum number of distinct model names interned over a process lifetime.
/// Bounded to prevent unbounded `Box::leak` growth if `set_model` is ever
/// called with adversarial or malformed input (e.g. LLM tool-call output).
const INTERN_MAX_ENTRIES: usize = 1024;

/// Maximum length (in bytes) of a single interned model name. Anything
/// longer is treated as malformed — real model identifiers are well under
/// this (GPT-4o: 6 bytes, `anthropic.claude-opus-4-6-v1`: ~28 bytes).
const INTERN_MAX_LEN: usize = 256;

/// Fallback interned string used when a name exceeds the length limit or
/// the distinct-entry cap fills up. Chosen to be visibly wrong in logs so
/// operators notice the fallback rather than silently misattributing cost.
const INTERN_OVERFLOW_SENTINEL: &str = "<model-name-overflow>";

/// Intern a model-name string so it can be returned through the trait's
/// `fn model_name(&self) -> &str` contract without leaking on every swap.
///
/// Leakage is bounded two ways: per-entry via [`INTERN_MAX_LEN`] and
/// total via [`INTERN_MAX_ENTRIES`]. Hitting either cap logs at `warn!`
/// and returns a static sentinel, so the process can't be coerced into
/// unbounded memory growth by repeated `set_model` calls with novel
/// strings.
fn intern_model_name(name: &str) -> &'static str {
    static INTERNER: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let map = INTERNER.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    intern_into(&mut guard, name, INTERN_MAX_ENTRIES, INTERN_MAX_LEN)
}

/// Lockless core of [`intern_model_name`], split out so the cap logic can
/// be unit-tested against a local `HashMap` without contaminating the
/// process-wide `OnceLock` interner that other tests read from.
fn intern_into(
    map: &mut HashMap<String, &'static str>,
    name: &str,
    max_entries: usize,
    max_len: usize,
) -> &'static str {
    if name.len() > max_len {
        tracing::warn!(
            len = name.len(),
            max = max_len,
            "model name exceeds interner length limit; using overflow sentinel",
        );
        return INTERN_OVERFLOW_SENTINEL;
    }
    if let Some(existing) = map.get(name) {
        return existing;
    }
    if map.len() >= max_entries {
        tracing::warn!(
            entries = map.len(),
            max = max_entries,
            "model name interner is full; using overflow sentinel",
        );
        return INTERN_OVERFLOW_SENTINEL;
    }
    let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
    map.insert(name.to_string(), leaked);
    leaked
}

struct ProviderSnapshot {
    inner: Arc<dyn LlmProvider>,
    model_name: &'static str,
    active_model_name: Arc<str>,
    cost_per_token: (Decimal, Decimal),
    cache_write_multiplier: Decimal,
    cache_read_discount: Decimal,
}

impl std::fmt::Debug for ProviderSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderSnapshot")
            .field("model_name", &self.model_name)
            .field("active_model_name", &&*self.active_model_name)
            .finish_non_exhaustive()
    }
}

impl ProviderSnapshot {
    fn capture(provider: Arc<dyn LlmProvider>) -> Self {
        let model_name = intern_model_name(provider.model_name());
        let active_model_name = Arc::from(provider.active_model_name());
        let cost_per_token = provider.cost_per_token();
        let cache_write_multiplier = provider.cache_write_multiplier();
        let cache_read_discount = provider.cache_read_discount();
        Self {
            inner: provider,
            model_name,
            active_model_name,
            cost_per_token,
            cache_write_multiplier,
            cache_read_discount,
        }
    }
}

fn read<T>(lock: &RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn write<T>(lock: &RwLock<T>) -> std::sync::RwLockWriteGuard<'_, T> {
    lock.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A provider wrapper whose inner provider can be swapped at runtime.
///
/// See the module-level docs for the invariants this type guarantees.
pub struct SwappableLlmProvider {
    state: RwLock<ProviderSnapshot>,
}

impl std::fmt::Debug for SwappableLlmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let snap = read(&self.state);
        f.debug_struct("SwappableLlmProvider")
            .field("model_name", &snap.model_name)
            .field("active_model_name", &&*snap.active_model_name)
            .finish_non_exhaustive()
    }
}

impl SwappableLlmProvider {
    /// Wrap an initial provider. The returned handle is `Arc`-friendly and
    /// can be cloned freely; all clones see the same swap state.
    pub fn new(inner: Arc<dyn LlmProvider>) -> Self {
        Self {
            state: RwLock::new(ProviderSnapshot::capture(inner)),
        }
    }

    /// Replace the inner provider chain with a freshly rebuilt provider.
    /// Metadata is refreshed atomically in the same critical section.
    pub fn swap(&self, inner: Arc<dyn LlmProvider>) {
        let fresh = ProviderSnapshot::capture(inner);
        *write(&self.state) = fresh;
    }

    fn current(&self) -> Arc<dyn LlmProvider> {
        read(&self.state).inner.clone()
    }
}

#[async_trait]
impl LlmProvider for SwappableLlmProvider {
    fn model_name(&self) -> &str {
        read(&self.state).model_name
    }

    fn cost_per_token(&self) -> (Decimal, Decimal) {
        read(&self.state).cost_per_token
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        self.current().complete(request).await
    }

    async fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, LlmError> {
        self.current().complete_with_tools(request).await
    }

    async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        self.current().list_models().await
    }

    async fn model_metadata(&self) -> Result<ModelMetadata, LlmError> {
        self.current().model_metadata().await
    }

    fn effective_model_name(&self, requested_model: Option<&str>) -> String {
        self.current().effective_model_name(requested_model)
    }

    fn active_model_name(&self) -> String {
        read(&self.state).active_model_name.to_string()
    }

    fn set_model(&self, model: &str) -> Result<(), LlmError> {
        // Hold the write lock across both the delegate call and the snapshot
        // refresh so a concurrent `swap()` cannot overwrite the just-updated
        // inner provider with a snapshot captured from an older one. Inner
        // `set_model` impls are synchronous (no `.await`), so holding a
        // std::sync lock across the call is safe.
        let mut guard = write(&self.state);
        guard.inner.set_model(model)?;
        let refreshed = ProviderSnapshot::capture(Arc::clone(&guard.inner));
        *guard = refreshed;
        Ok(())
    }

    fn cache_write_multiplier(&self) -> Decimal {
        read(&self.state).cache_write_multiplier
    }

    fn cache_read_discount(&self) -> Decimal {
        read(&self.state).cache_read_discount
    }

    async fn stream_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<LlmStream, LlmError> {
        self.current().stream_with_tools(request).await
    }

    fn capabilities(&self) -> LlmProviderCapabilities {
        self.current().capabilities()
    }
}

/// Output of an [`LlmReloadHandle`] rebuild step.
///
/// `primary` always carries the freshly-built chain. `cheap` is `Some`
/// when the configuration enables a cheap-model lane, `None` otherwise;
/// see [`LlmReloadHandle::reload`] for the asymmetry rules around it.
pub struct ProviderChainComponents {
    /// Primary (default) provider chain.
    pub primary: Arc<dyn LlmProvider>,
    /// Optional cheap-model lane.
    pub cheap: Option<Arc<dyn LlmProvider>>,
}

/// Stable hot-reload handle for the primary/cheap provider chain.
///
/// Holds the two [`SwappableLlmProvider`] wrappers created at startup and
/// serializes concurrent reloads through an internal mutex so rapid setting
/// changes don't trigger overlapping chain rebuilds (which would redo
/// potentially-expensive work like OAuth refresh and HTTP probes).
///
/// The chain builder is supplied per-call by the application layer rather
/// than baked in; this keeps `dasclaw_llm_provider` decoupled from any
/// particular `LlmConfig` / `SessionManager` shape.
#[derive(Debug)]
pub struct LlmReloadHandle {
    primary: Arc<SwappableLlmProvider>,
    cheap: Option<Arc<SwappableLlmProvider>>,
    /// Serializes concurrent `reload()` calls so rapid setting toggles
    /// don't fire overlapping chain rebuilds (each rebuild can touch OAuth
    /// refresh and HTTP probes; letting them pile up wastes upstream quota
    /// and leaves the wrapper briefly pointing at a half-built chain).
    reload_lock: tokio::sync::Mutex<()>,
}

impl LlmReloadHandle {
    /// Create a handle around the wrappers allocated at startup.
    pub fn new(
        primary: Arc<SwappableLlmProvider>,
        cheap: Option<Arc<SwappableLlmProvider>>,
    ) -> Self {
        Self {
            primary,
            cheap,
            reload_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Get the primary provider as a trait object suitable for callers
    /// that hold `Arc<dyn LlmProvider>`.
    pub fn primary_provider(&self) -> Arc<dyn LlmProvider> {
        self.primary.clone() as Arc<dyn LlmProvider>
    }

    /// Get the cheap-lane provider, if one was allocated at startup.
    pub fn cheap_provider(&self) -> Option<Arc<dyn LlmProvider>> {
        self.cheap
            .as_ref()
            .map(|provider| provider.clone() as Arc<dyn LlmProvider>)
    }

    /// Rebuild the provider chain via the caller-supplied `build` closure
    /// and atomically replace the inner providers of the primary (and
    /// cheap, if present) wrappers.
    ///
    /// Reloads are serialized so two concurrent callers cannot race.
    pub async fn reload<F, Fut>(&self, build: F) -> Result<(), LlmError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<ProviderChainComponents, LlmError>>,
    {
        let _guard = self.reload_lock.lock().await;

        let components = build().await?;

        self.primary.swap(components.primary);

        if let Some(ref cheap_handle) = self.cheap {
            let new_cheap = components
                .cheap
                .unwrap_or_else(|| self.primary.clone() as Arc<dyn LlmProvider>);
            cheap_handle.swap(new_cheap);
        } else if components.cheap.is_some() {
            // Asymmetry: no cheap wrapper was allocated at startup, so a
            // newly-configured cheap model cannot be activated via hot-reload.
            // Surfacing this through tracing so ops don't think the swap
            // silently took effect.
            tracing::warn!(
                "llm hot-reload: cheap provider is now configured but was not at startup; \
                 it will only take effect after a full restart",
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::RwLock as StdRwLock;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Simple stub that supports `set_model()` so we can exercise the
    /// snapshot-refresh path and the "override is lost on swap" behaviour.
    #[derive(Debug)]
    struct TestProvider {
        configured: &'static str,
        active: StdRwLock<String>,
        cost: (Decimal, Decimal),
        cache_write: Decimal,
        cache_read: Decimal,
        complete_calls: AtomicUsize,
    }

    impl TestProvider {
        fn new(configured: &'static str, active: &str, cost: (Decimal, Decimal)) -> Self {
            Self {
                configured,
                active: StdRwLock::new(active.to_string()),
                cost,
                cache_write: Decimal::ONE,
                cache_read: Decimal::ONE,
                complete_calls: AtomicUsize::new(0),
            }
        }

        fn complete_calls(&self) -> usize {
            self.complete_calls.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl LlmProvider for TestProvider {
        fn model_name(&self) -> &str {
            self.configured
        }

        fn cost_per_token(&self) -> (Decimal, Decimal) {
            self.cost
        }

        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            self.complete_calls.fetch_add(1, Ordering::SeqCst);
            Err(LlmError::RequestFailed {
                provider: self.configured.to_string(),
                reason: "TestProvider does not implement complete".to_string(),
            })
        }

        async fn complete_with_tools(
            &self,
            _request: ToolCompletionRequest,
        ) -> Result<ToolCompletionResponse, LlmError> {
            Err(LlmError::RequestFailed {
                provider: self.configured.to_string(),
                reason: "TestProvider does not implement complete_with_tools".to_string(),
            })
        }

        fn active_model_name(&self) -> String {
            self.active
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
        }

        fn set_model(&self, model: &str) -> Result<(), LlmError> {
            *self
                .active
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = model.to_string();
            Ok(())
        }

        fn cache_write_multiplier(&self) -> Decimal {
            self.cache_write
        }

        fn cache_read_discount(&self) -> Decimal {
            self.cache_read
        }
    }

    fn empty_completion_request() -> CompletionRequest {
        CompletionRequest::new(Vec::new())
    }

    #[test]
    fn swap_replaces_all_metadata_atomically() {
        let a = Arc::new(TestProvider::new(
            "cfg-a",
            "active-a",
            (Decimal::new(1, 0), Decimal::new(2, 0)),
        ));
        let wrapper = SwappableLlmProvider::new(a);

        assert_eq!(wrapper.model_name(), "cfg-a");
        assert_eq!(wrapper.active_model_name(), "active-a");
        assert_eq!(
            wrapper.cost_per_token(),
            (Decimal::new(1, 0), Decimal::new(2, 0))
        );

        let b = Arc::new(TestProvider::new(
            "cfg-b",
            "active-b",
            (Decimal::new(3, 0), Decimal::new(4, 0)),
        ));
        wrapper.swap(b);

        assert_eq!(wrapper.model_name(), "cfg-b");
        assert_eq!(wrapper.active_model_name(), "active-b");
        assert_eq!(
            wrapper.cost_per_token(),
            (Decimal::new(3, 0), Decimal::new(4, 0))
        );
    }

    /// After `swap`, `complete()` must reach the new inner provider and not
    /// the old one. Counters on the test providers prove which inner ran.
    #[tokio::test]
    async fn swap_routes_complete_to_new_inner() {
        let a = Arc::new(TestProvider::new("a", "a", (Decimal::ZERO, Decimal::ZERO)));
        let b = Arc::new(TestProvider::new("b", "b", (Decimal::ZERO, Decimal::ZERO)));
        let wrapper = SwappableLlmProvider::new(Arc::clone(&a) as Arc<dyn LlmProvider>);

        let _ = wrapper.complete(empty_completion_request()).await;
        wrapper.swap(Arc::clone(&b) as Arc<dyn LlmProvider>);
        let _ = wrapper.complete(empty_completion_request()).await;
        let _ = wrapper.complete(empty_completion_request()).await;

        assert_eq!(a.complete_calls(), 1, "old provider should see one call");
        assert_eq!(b.complete_calls(), 2, "new provider should see two calls");
    }

    /// An in-flight `complete()` future captured the old `Arc<dyn LlmProvider>`
    /// via `current()`; a `swap()` during its execution must not redirect it.
    #[tokio::test]
    async fn in_flight_call_uses_old_provider() {
        use std::sync::Barrier;
        use std::thread;

        let a = Arc::new(TestProvider::new("a", "a", (Decimal::ZERO, Decimal::ZERO)));
        let b = Arc::new(TestProvider::new("b", "b", (Decimal::ZERO, Decimal::ZERO)));
        let wrapper = Arc::new(SwappableLlmProvider::new(
            Arc::clone(&a) as Arc<dyn LlmProvider>
        ));

        // Capture the current inner before swap, exactly the way
        // `complete()` would have at the start of its execution.
        let captured = wrapper.current();

        let barrier = Arc::new(Barrier::new(2));
        let bw = barrier.clone();
        let ww = Arc::clone(&wrapper);
        let bclone = Arc::clone(&b);
        let swapper = thread::spawn(move || {
            bw.wait();
            ww.swap(bclone as Arc<dyn LlmProvider>);
        });

        barrier.wait();
        // The captured Arc still points at `a`; calling `complete` on it
        // must touch `a` even though `swap()` is racing with us.
        let _ = captured.complete(empty_completion_request()).await;
        swapper.join().expect("swapper thread");

        assert_eq!(
            a.complete_calls(),
            1,
            "captured Arc must still hit the old provider after swap"
        );
        assert_eq!(b.complete_calls(), 0);
    }

    #[test]
    fn set_model_forwards_and_refreshes_snapshot() {
        let inner = Arc::new(TestProvider::new(
            "cfg",
            "cfg",
            (Decimal::ZERO, Decimal::ZERO),
        ));
        let wrapper = SwappableLlmProvider::new(inner);

        wrapper
            .set_model("cfg-override")
            .expect("test provider supports set_model");

        assert_eq!(wrapper.active_model_name(), "cfg-override");
    }

    #[test]
    fn set_model_override_is_dropped_on_swap() {
        let initial = Arc::new(TestProvider::new(
            "cfg-a",
            "cfg-a",
            (Decimal::ZERO, Decimal::ZERO),
        ));
        let wrapper = SwappableLlmProvider::new(initial);

        wrapper
            .set_model("cfg-a-override")
            .expect("set_model supported");
        assert_eq!(wrapper.active_model_name(), "cfg-a-override");

        let replacement = Arc::new(TestProvider::new(
            "cfg-b",
            "cfg-b",
            (Decimal::ZERO, Decimal::ZERO),
        ));
        wrapper.swap(replacement);

        assert_eq!(wrapper.active_model_name(), "cfg-b");
    }

    #[test]
    fn model_name_interner_reuses_leaked_strings() {
        let a = intern_model_name("gpt-5");
        let b = intern_model_name("gpt-5");
        assert_eq!(a.as_ptr(), b.as_ptr());
    }

    /// A model name that overshoots the per-entry length limit must fall
    /// back to the overflow sentinel rather than leak a huge string.
    /// Exercises `intern_into` against a local map so the process-wide
    /// interner stays untouched (other tests depend on it returning real
    /// interned strings for normal names).
    #[test]
    fn intern_into_rejects_oversized_input() {
        let mut map: HashMap<String, &'static str> = HashMap::new();
        let huge = "x".repeat(INTERN_MAX_LEN + 1);
        let interned = intern_into(&mut map, &huge, INTERN_MAX_ENTRIES, INTERN_MAX_LEN);
        assert_eq!(interned, INTERN_OVERFLOW_SENTINEL);
        assert!(map.is_empty());
    }

    /// Once the distinct-entry cap is reached, further novel names must
    /// route to the overflow sentinel.
    #[test]
    fn intern_into_caps_distinct_entries() {
        let mut map: HashMap<String, &'static str> = HashMap::new();
        let cap = 4;
        for i in 0..cap {
            let interned = intern_into(&mut map, &format!("name-{i}"), cap, INTERN_MAX_LEN);
            assert_ne!(interned, INTERN_OVERFLOW_SENTINEL);
        }
        let over = intern_into(&mut map, "one-too-many", cap, INTERN_MAX_LEN);
        assert_eq!(over, INTERN_OVERFLOW_SENTINEL);
        let reused = intern_into(&mut map, "name-0", cap, INTERN_MAX_LEN);
        assert_ne!(reused, INTERN_OVERFLOW_SENTINEL);
    }

    /// Concurrent `swap` and `set_model` against the same wrapper must
    /// never crash or deadlock, and the final snapshot must be readable.
    /// Before the fix, `set_model` released the state read lock between
    /// mutating the inner and writing the snapshot — a parallel `swap`
    /// could overwrite the snapshot with one from an older provider.
    #[test]
    fn set_model_and_swap_are_mutually_atomic() {
        use std::sync::Arc as StdArc;
        use std::thread;

        let initial = StdArc::new(TestProvider::new(
            "p-a",
            "p-a",
            (Decimal::ZERO, Decimal::ZERO),
        ));
        let wrapper = StdArc::new(SwappableLlmProvider::new(initial));

        const ITERS: usize = 200;

        let w1 = StdArc::clone(&wrapper);
        let swapper = thread::spawn(move || {
            for i in 0..ITERS {
                let replacement = StdArc::new(TestProvider::new(
                    if i % 2 == 0 { "p-even" } else { "p-odd" },
                    if i % 2 == 0 { "p-even" } else { "p-odd" },
                    (Decimal::ZERO, Decimal::ZERO),
                ));
                w1.swap(replacement);
            }
        });

        let w2 = StdArc::clone(&wrapper);
        let setter = thread::spawn(move || {
            for i in 0..ITERS {
                let _ = w2.set_model(&format!("override-{i}"));
            }
        });

        swapper.join().expect("swapper thread");
        setter.join().expect("setter thread");

        let configured = wrapper.model_name();
        assert!(
            matches!(configured, "p-a" | "p-even" | "p-odd"),
            "configured model_name must come from a real swap: {configured}",
        );
        let _ = wrapper.active_model_name();
        let _ = wrapper.cost_per_token();
    }

    /// Two concurrent `reload()` invocations must run sequentially —
    /// the second builder closure must not start until the first finishes.
    /// Verified by recording start/end timestamps and asserting no overlap.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn reload_handle_serializes_concurrent_reloads() {
        use std::sync::Mutex as StdMutex;
        use std::time::{Duration, Instant};

        let primary = Arc::new(SwappableLlmProvider::new(Arc::new(TestProvider::new(
            "p0",
            "p0",
            (Decimal::ZERO, Decimal::ZERO),
        ))
            as Arc<dyn LlmProvider>));
        let handle = Arc::new(LlmReloadHandle::new(Arc::clone(&primary), None));

        let intervals: Arc<StdMutex<Vec<(Instant, Instant)>>> = Arc::new(StdMutex::new(Vec::new()));

        let mut joins = Vec::new();
        for i in 0..4 {
            let h = Arc::clone(&handle);
            let intervals = Arc::clone(&intervals);
            joins.push(tokio::spawn(async move {
                h.reload(|| async move {
                    let start = Instant::now();
                    tokio::time::sleep(Duration::from_millis(40)).await;
                    let end = Instant::now();
                    intervals
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .push((start, end));
                    Ok(ProviderChainComponents {
                        primary: Arc::new(TestProvider::new(
                            "p-new",
                            &format!("p-new-{i}"),
                            (Decimal::ZERO, Decimal::ZERO),
                        )) as Arc<dyn LlmProvider>,
                        cheap: None,
                    })
                })
                .await
                .expect("reload ok");
            }));
        }

        for j in joins {
            j.await.expect("task");
        }

        let mut intervals = intervals
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        intervals.sort_by_key(|(s, _)| *s);
        for pair in intervals.windows(2) {
            let (_, end_a) = pair[0];
            let (start_b, _) = pair[1];
            assert!(
                start_b >= end_a,
                "reload intervals overlapped: {:?} then {:?}",
                pair[0],
                pair[1]
            );
        }
    }

    /// A reload with a cheap chain must propagate to the cheap wrapper.
    /// A reload without a cheap chain on a handle that had one falls back
    /// to mirroring the primary (the documented behaviour).
    #[tokio::test]
    async fn reload_propagates_cheap_chain_or_falls_back_to_primary() {
        let primary = Arc::new(SwappableLlmProvider::new(Arc::new(TestProvider::new(
            "p0",
            "p0",
            (Decimal::ZERO, Decimal::ZERO),
        ))
            as Arc<dyn LlmProvider>));
        let cheap = Arc::new(SwappableLlmProvider::new(Arc::new(TestProvider::new(
            "c0",
            "c0",
            (Decimal::ZERO, Decimal::ZERO),
        ))
            as Arc<dyn LlmProvider>));
        let handle = LlmReloadHandle::new(Arc::clone(&primary), Some(Arc::clone(&cheap)));

        // With cheap supplied → cheap wrapper gets the new cheap inner.
        handle
            .reload(|| async {
                Ok(ProviderChainComponents {
                    primary: Arc::new(TestProvider::new(
                        "p1",
                        "p1",
                        (Decimal::ZERO, Decimal::ZERO),
                    )) as Arc<dyn LlmProvider>,
                    cheap: Some(Arc::new(TestProvider::new(
                        "c1",
                        "c1",
                        (Decimal::ZERO, Decimal::ZERO),
                    )) as Arc<dyn LlmProvider>),
                })
            })
            .await
            .expect("reload ok");
        assert_eq!(primary.active_model_name(), "p1");
        assert_eq!(cheap.active_model_name(), "c1");

        // Without cheap supplied → cheap wrapper mirrors primary.
        handle
            .reload(|| async {
                Ok(ProviderChainComponents {
                    primary: Arc::new(TestProvider::new(
                        "p2",
                        "p2",
                        (Decimal::ZERO, Decimal::ZERO),
                    )) as Arc<dyn LlmProvider>,
                    cheap: None,
                })
            })
            .await
            .expect("reload ok");
        assert_eq!(primary.active_model_name(), "p2");
        assert_eq!(cheap.active_model_name(), "p2");
    }

    /// A builder that returns an error must not mutate either wrapper.
    #[tokio::test]
    async fn reload_error_leaves_wrappers_untouched() {
        let primary = Arc::new(SwappableLlmProvider::new(Arc::new(TestProvider::new(
            "p0",
            "p0",
            (Decimal::ZERO, Decimal::ZERO),
        ))
            as Arc<dyn LlmProvider>));
        let handle = LlmReloadHandle::new(Arc::clone(&primary), None);

        let err = handle
            .reload(|| async {
                Err::<ProviderChainComponents, _>(LlmError::RequestFailed {
                    provider: "test".to_string(),
                    reason: "boom".to_string(),
                })
            })
            .await;
        assert!(err.is_err());
        assert_eq!(primary.active_model_name(), "p0");
    }
}
