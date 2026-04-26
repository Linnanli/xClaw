//! Proxy URL parsing utilities — port from
//! `codex-cli-main/codex-rs/sandboxing/src/seatbelt.rs` (W2.2b).
//!
//! Extracts loopback ports from `HTTP_PROXY` / `HTTPS_PROXY` style env vars so
//! the sandbox can punch holes for the local proxy server. Pure algorithm, no
//! `codex_network_proxy` dep — the env-var key list is owned here.

use std::collections::{BTreeSet, HashMap};

use url::Url;

/// Standard proxy URL env var keys (mirrors codex `PROXY_URL_ENV_KEYS`).
pub const PROXY_URL_ENV_KEYS: &[&str] = &[
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
];

/// Return `true` for `localhost`, `127.0.0.1`, or `::1`.
pub fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1"
}

/// Default port for a proxy scheme when not specified in the URL.
pub fn proxy_scheme_default_port(scheme: &str) -> u16 {
    match scheme {
        "https" => 443,
        "socks5" | "socks5h" | "socks4" | "socks4a" => 1080,
        _ => 80,
    }
}

/// Walk all known proxy env vars; for each value pointing at a loopback host,
/// collect its (explicit or default) port.
pub fn detect_loopback_ports(env: &HashMap<String, String>) -> Vec<u16> {
    let mut ports = BTreeSet::new();
    for key in PROXY_URL_ENV_KEYS {
        let Some(raw) = env.get(*key).map(String::as_str) else {
            continue;
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }

        let candidate = if trimmed.contains("://") {
            trimmed.to_string()
        } else {
            format!("http://{trimmed}")
        };
        let Ok(parsed) = Url::parse(&candidate) else {
            continue;
        };
        let Some(host) = parsed.host_str() else {
            continue;
        };
        if !is_loopback_host(host) {
            continue;
        }

        let scheme = parsed.scheme().to_ascii_lowercase();
        let port = parsed
            .port()
            .unwrap_or_else(|| proxy_scheme_default_port(scheme.as_str()));
        ports.insert(port);
    }
    ports.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn loopback_host_recognises_canonical_forms() {
        assert!(is_loopback_host("localhost"));
        assert!(is_loopback_host("LOCALHOST"));
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("::1"));
        assert!(!is_loopback_host("192.168.1.1"));
        assert!(!is_loopback_host("example.com"));
    }

    #[test]
    fn scheme_default_ports_are_correct() {
        assert_eq!(proxy_scheme_default_port("http"), 80);
        assert_eq!(proxy_scheme_default_port("https"), 443);
        assert_eq!(proxy_scheme_default_port("socks5"), 1080);
        assert_eq!(proxy_scheme_default_port("unknown"), 80);
    }

    #[test]
    fn detects_explicit_loopback_port() {
        let e = env(&[("HTTP_PROXY", "http://127.0.0.1:8888")]);
        assert_eq!(detect_loopback_ports(&e), vec![8888]);
    }

    #[test]
    fn detects_default_https_port_for_loopback() {
        let e = env(&[("HTTPS_PROXY", "https://localhost")]);
        assert_eq!(detect_loopback_ports(&e), vec![443]);
    }

    #[test]
    fn ignores_non_loopback_proxies() {
        let e = env(&[("HTTP_PROXY", "http://corp.proxy.example.com:3128")]);
        assert!(detect_loopback_ports(&e).is_empty());
    }

    #[test]
    fn handles_url_without_scheme() {
        let e = env(&[("HTTP_PROXY", "127.0.0.1:9999")]);
        assert_eq!(detect_loopback_ports(&e), vec![9999]);
    }

    #[test]
    fn deduplicates_and_sorts_ports() {
        let e = env(&[
            ("HTTP_PROXY", "http://localhost:8080"),
            ("HTTPS_PROXY", "http://127.0.0.1:8080"),
            ("ALL_PROXY", "socks5://localhost:1080"),
        ]);
        assert_eq!(detect_loopback_ports(&e), vec![1080, 8080]);
    }

    #[test]
    fn ignores_empty_and_malformed() {
        let e = env(&[
            ("HTTP_PROXY", ""),
            ("HTTPS_PROXY", "  "),
            ("ALL_PROXY", "::not-a-url::"),
        ]);
        assert!(detect_loopback_ports(&e).is_empty());
    }
}
