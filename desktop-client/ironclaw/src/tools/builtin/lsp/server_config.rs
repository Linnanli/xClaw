//! Language → LSP server mapping and configuration.
//!
//! Maps file extensions to language server commands. The default set can be
//! overridden or restricted by an Admin-pushed whitelist.

use std::collections::HashMap;
use std::path::Path;

/// Configuration for a single language server.
#[derive(Debug, Clone)]
pub struct LspServerConfig {
    /// Human-readable name (e.g. "rust-analyzer").
    pub name: String,
    /// LSP language identifier sent in `textDocument/didOpen`.
    pub language_id: String,
    /// Command to launch the server.
    pub command: String,
    /// Command-line arguments.
    pub args: Vec<String>,
}

/// Resolves file paths to the appropriate language server config.
#[derive(Debug, Clone)]
pub struct LspServerMapping {
    /// Extension (without dot) → config.
    ext_map: HashMap<String, LspServerConfig>,
}

impl LspServerMapping {
    /// Create the default mapping for well-known language servers.
    pub fn defaults() -> Self {
        let mut ext_map = HashMap::new();

        let rust_analyzer = LspServerConfig {
            name: "rust-analyzer".into(),
            language_id: "rust".into(),
            command: "rust-analyzer".into(),
            args: vec![],
        };
        ext_map.insert("rs".into(), rust_analyzer);

        let ts_server = LspServerConfig {
            name: "typescript-language-server".into(),
            language_id: "typescript".into(),
            command: "typescript-language-server".into(),
            args: vec!["--stdio".into()],
        };
        ext_map.insert("ts".into(), ts_server.clone());
        ext_map.insert(
            "tsx".into(),
            LspServerConfig {
                language_id: "typescriptreact".into(),
                ..ts_server.clone()
            },
        );

        let js_server = LspServerConfig {
            language_id: "javascript".into(),
            ..ts_server.clone()
        };
        ext_map.insert("js".into(), js_server.clone());
        ext_map.insert(
            "jsx".into(),
            LspServerConfig {
                language_id: "javascriptreact".into(),
                ..js_server
            },
        );

        let pyright = LspServerConfig {
            name: "pyright".into(),
            language_id: "python".into(),
            command: "pyright-langserver".into(),
            args: vec!["--stdio".into()],
        };
        ext_map.insert("py".into(), pyright);

        let gopls = LspServerConfig {
            name: "gopls".into(),
            language_id: "go".into(),
            command: "gopls".into(),
            args: vec!["serve".into()],
        };
        ext_map.insert("go".into(), gopls);

        Self { ext_map }
    }

    /// Look up the server config for a file path based on its extension.
    pub fn config_for_path(&self, path: &Path) -> Option<&LspServerConfig> {
        let ext = path.extension()?.to_str()?;
        self.ext_map.get(ext)
    }

    /// Look up by extension string directly.
    pub fn config_for_ext(&self, ext: &str) -> Option<&LspServerConfig> {
        self.ext_map.get(ext)
    }

    /// Return all known server names (for admin listing).
    pub fn known_servers(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.ext_map.values().map(|c| c.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// Remove servers not in the whitelist. Empty whitelist = allow all.
    pub fn apply_whitelist(&mut self, allowed: &[String]) {
        if allowed.is_empty() {
            return;
        }
        self.ext_map
            .retain(|_, config| allowed.iter().any(|a| a == &config.name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults_has_rust() {
        let m = LspServerMapping::defaults();
        let cfg = m.config_for_path(Path::new("src/main.rs"));
        assert!(cfg.is_some());
        let cfg = cfg.expect("rust config");
        assert_eq!(cfg.name, "rust-analyzer");
        assert_eq!(cfg.language_id, "rust");
    }

    #[test]
    fn test_defaults_has_typescript() {
        let m = LspServerMapping::defaults();
        let cfg = m.config_for_ext("ts").expect("ts config");
        assert_eq!(cfg.language_id, "typescript");
        let cfg = m.config_for_ext("tsx").expect("tsx config");
        assert_eq!(cfg.language_id, "typescriptreact");
    }

    #[test]
    fn test_defaults_has_python() {
        let m = LspServerMapping::defaults();
        let cfg = m.config_for_ext("py").expect("py config");
        assert_eq!(cfg.name, "pyright");
    }

    #[test]
    fn test_unknown_extension_returns_none() {
        let m = LspServerMapping::defaults();
        assert!(m.config_for_path(Path::new("photo.jpg")).is_none());
    }

    #[test]
    fn test_whitelist_filters_servers() {
        let mut m = LspServerMapping::defaults();
        m.apply_whitelist(&["rust-analyzer".into()]);
        assert!(m.config_for_ext("rs").is_some());
        assert!(m.config_for_ext("ts").is_none());
        assert!(m.config_for_ext("py").is_none());
    }

    #[test]
    fn test_empty_whitelist_allows_all() {
        let mut m = LspServerMapping::defaults();
        let before = m.known_servers().len();
        m.apply_whitelist(&[]);
        assert_eq!(m.known_servers().len(), before);
    }

    #[test]
    fn test_known_servers_deduplicated() {
        let m = LspServerMapping::defaults();
        let servers = m.known_servers();
        // ts/tsx/js/jsx all map to same server name
        assert_eq!(
            servers
                .iter()
                .filter(|s| **s == "typescript-language-server")
                .count(),
            1
        );
    }
}
