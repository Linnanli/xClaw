use std::io;
use std::path::Path;

const MAX_DEPTH: usize = 20;
const HARD_MAX_RESULTS: usize = 200;
const IGNORED_DIRS: &[&str] = &["node_modules", "target", "dist", "build", ".git"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuzzyPathMatchType {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FuzzyPathSearchResult {
    pub root: String,
    pub path: String,
    pub match_type: FuzzyPathMatchType,
    pub file_name: String,
    pub score: f64,
    pub indices: Option<Vec<usize>>,
}

pub fn fuzzy_file_search(
    root: &Path,
    query: &str,
    max_results: usize,
) -> io::Result<Vec<FuzzyPathSearchResult>> {
    let query = query.trim();
    if query.is_empty() || max_results == 0 {
        return Ok(Vec::new());
    }

    let root = root.canonicalize()?;
    let limit = max_results.min(HARD_MAX_RESULTS);
    let mut results = Vec::new();
    collect_matches(&root, &root, query, 0, &mut results)?;
    results.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.path.cmp(&right.path))
    });
    results.truncate(limit);
    Ok(results)
}

fn collect_matches(
    root: &Path,
    dir: &Path,
    query: &str,
    depth: usize,
    results: &mut Vec<FuzzyPathSearchResult>,
) -> io::Result<()> {
    if depth > MAX_DEPTH {
        return Ok(());
    }

    let mut entries = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if should_skip_entry(&name) {
            continue;
        }

        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            continue;
        }

        if file_type.is_file() {
            push_match(root, &path, &name, FuzzyPathMatchType::File, query, results);
        } else if file_type.is_dir() {
            push_match(
                root,
                &path,
                &name,
                FuzzyPathMatchType::Directory,
                query,
                results,
            );
            collect_matches(root, &path, query, depth + 1, results)?;
        }
    }
    Ok(())
}

fn push_match(
    root: &Path,
    path: &Path,
    file_name: &str,
    match_type: FuzzyPathMatchType,
    query: &str,
    results: &mut Vec<FuzzyPathSearchResult>,
) {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let rel_path = rel
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    let Some((score, indices)) = score_path(file_name, &rel_path, query) else {
        return;
    };

    results.push(FuzzyPathSearchResult {
        root: root.to_string_lossy().to_string(),
        path: rel_path,
        match_type,
        file_name: file_name.to_string(),
        score,
        indices: Some(indices),
    });
}

fn score_path(file_name: &str, rel_path: &str, query: &str) -> Option<(f64, Vec<usize>)> {
    match ordered_subsequence_indices(file_name, query) {
        Some(indices) => Some((score_match(file_name, query, &indices), indices)),
        None => ordered_subsequence_indices(rel_path, query)
            .map(|indices| (score_match(rel_path, query, &indices) * 0.9, indices)),
    }
}

fn ordered_subsequence_indices(haystack: &str, query: &str) -> Option<Vec<usize>> {
    let haystack_lower = haystack.to_lowercase();
    let query_lower = query.to_lowercase();
    let mut indices = Vec::new();
    let mut haystack_chars = haystack_lower.char_indices();

    for query_char in query_lower.chars() {
        let (index, _) = haystack_chars.find(|(_, haystack_char)| *haystack_char == query_char)?;
        indices.push(index);
    }
    Some(indices)
}

fn score_match(haystack: &str, query: &str, indices: &[usize]) -> f64 {
    let first = indices.first().copied().unwrap_or(0);
    let last = indices.last().copied().unwrap_or(first);
    let span = (last - first + 1).max(1) as f64;
    let query_len = query.chars().count().max(1) as f64;
    let haystack_len = haystack.chars().count().max(1) as f64;

    let density = query_len / span;
    let length_bonus = query_len / haystack_len;
    let prefix_bonus = if first == 0 { 1.0 } else { 0.0 };
    density * 100.0 + length_bonus * 20.0 + prefix_bonus * 25.0
}

fn should_skip_entry(name: &str) -> bool {
    name.starts_with('.') || IGNORED_DIRS.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_path_search_matches_ordered_subsequence() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("src/app")).expect("mkdir");
        std::fs::write(temp.path().join("src/app/config_service.rs"), "").expect("write");
        std::fs::write(temp.path().join("src/app/command_service.rs"), "").expect("write");

        let results = fuzzy_file_search(temp.path(), "cfg", 10).expect("search");
        assert_eq!(results[0].file_name, "config_service.rs");
        assert_eq!(results[0].match_type, FuzzyPathMatchType::File);
        assert!(results[0].indices.as_ref().expect("indices").len() >= 2);
    }

    #[test]
    fn fuzzy_path_search_sorts_before_truncating_results() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("late")).expect("mkdir");

        for index in 0..225 {
            std::fs::write(
                temp.path()
                    .join(format!("a{index:03}_c_padding_f_padding_g.rs")),
                "",
            )
            .expect("write low score file");
        }
        std::fs::write(temp.path().join("late/cfg.rs"), "").expect("write high score file");

        let results = fuzzy_file_search(temp.path(), "cfg", 50).expect("search");
        assert_eq!(results[0].file_name, "cfg.rs");
    }

    #[cfg(unix)]
    #[test]
    fn fuzzy_path_search_does_not_follow_symlink_entries() {
        let root = tempfile::tempdir().expect("root tempdir");
        let outside = tempfile::tempdir().expect("outside tempdir");
        std::fs::write(outside.path().join("cfg.rs"), "").expect("write outside file");
        std::os::unix::fs::symlink(outside.path(), root.path().join("outside-link"))
            .expect("symlink outside");

        let results = fuzzy_file_search(root.path(), "cfg", 50).expect("search");
        assert!(
            results.is_empty(),
            "symlink entry should not be traversed: {results:?}"
        );
    }
}
