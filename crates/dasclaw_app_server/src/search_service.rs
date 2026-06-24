use std::path::PathBuf;
use std::sync::Mutex;

use dasclaw_app_server_protocol::{
    FuzzyFileSearchMatchType, FuzzyFileSearchParams, FuzzyFileSearchResponse,
    FuzzyFileSearchResult, FuzzyFileSearchSessionCompletedNotification,
    FuzzyFileSearchSessionUpdatedNotification, ServiceHealth, ServiceName,
};
use dasclaw_fs_tools::path_search::{FuzzyPathMatchType, fuzzy_file_search};
use dasclaw_fs_tools::path_utils::{normalize_lexical, validate_path};

use crate::AppServerError;
use crate::app_services::SearchService;

const CAPABILITY: &str = "search";
const MAX_RESULTS: usize = 50;

pub struct AppServerSearchService {
    root: PathBuf,
    events: Mutex<Vec<SearchNotification>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SearchNotification {
    Updated(FuzzyFileSearchSessionUpdatedNotification),
    Completed(FuzzyFileSearchSessionCompletedNotification),
}

impl AppServerSearchService {
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        let lexical_root = normalize_lexical(&root);
        let root = root.canonicalize().unwrap_or(lexical_root);
        Self {
            root,
            events: Mutex::new(Vec::new()),
        }
    }

    fn resolve_root(&self, raw: &str) -> Result<PathBuf, AppServerError> {
        if raw.trim().is_empty() {
            return Err(AppServerError::invalid_request(
                CAPABILITY,
                "search root must not be empty",
            ));
        }

        let resolved = validate_path(raw, Some(&self.root)).map_err(|error| {
            AppServerError::capability_unavailable(
                CAPABILITY,
                format!("search root is outside service root: {error}"),
            )
        })?;
        let canonical = resolved.canonicalize().map_err(|error| {
            AppServerError::capability_unavailable(
                CAPABILITY,
                format!("search root is unavailable: {error}"),
            )
        })?;
        if !canonical.starts_with(&self.root) {
            return Err(AppServerError::capability_unavailable(
                CAPABILITY,
                "search root is outside service root",
            ));
        }
        Ok(canonical)
    }

    fn push_event(&self, event: SearchNotification) {
        let mut events = self
            .events
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        events.push(event);
    }
}

impl SearchService for AppServerSearchService {
    fn health(&self) -> ServiceHealth {
        let mut health = ServiceHealth::ready(ServiceName::Search);
        health.message = Some(format!(
            "root={} guard=canonical-root-containment",
            self.root.display()
        ));
        health
    }

    fn fuzzy_file_search(
        &self,
        params: FuzzyFileSearchParams,
    ) -> Result<FuzzyFileSearchResponse, AppServerError> {
        let query = params.query.trim();
        if query.is_empty() {
            return Ok(FuzzyFileSearchResponse { files: Vec::new() });
        }

        let mut files = Vec::new();
        for root in &params.roots {
            let resolved = self.resolve_root(root)?;
            let mut root_results =
                fuzzy_file_search(&resolved, query, MAX_RESULTS).map_err(|error| {
                    AppServerError::capability_unavailable(
                        CAPABILITY,
                        format!("fuzzy file search failed: {error}"),
                    )
                })?;
            files.extend(root_results.drain(..).map(map_search_result));
        }
        files.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.path.cmp(&right.path))
        });
        files.truncate(MAX_RESULTS);

        if let Some(session_id) = params.cancellation_token {
            self.push_event(SearchNotification::Updated(
                FuzzyFileSearchSessionUpdatedNotification {
                    session_id: session_id.clone(),
                    query: query.to_string(),
                    files: files.clone(),
                },
            ));
            self.push_event(SearchNotification::Completed(
                FuzzyFileSearchSessionCompletedNotification { session_id },
            ));
        }

        Ok(FuzzyFileSearchResponse { files })
    }

    fn drain_search_events(&self) -> Vec<SearchNotification> {
        self.events
            .lock()
            .map(|mut events| std::mem::take(&mut *events))
            .unwrap_or_default()
    }
}

fn map_search_result(
    result: dasclaw_fs_tools::path_search::FuzzyPathSearchResult,
) -> FuzzyFileSearchResult {
    FuzzyFileSearchResult {
        root: result.root,
        path: result.path,
        match_type: match result.match_type {
            FuzzyPathMatchType::File => FuzzyFileSearchMatchType::File,
            FuzzyPathMatchType::Directory => FuzzyFileSearchMatchType::Directory,
        },
        file_name: result.file_name,
        score: result.score,
        indices: result.indices,
    }
}
