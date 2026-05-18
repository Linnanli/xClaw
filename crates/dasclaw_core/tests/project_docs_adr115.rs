//! ADR-115 red-line test: ensure the project-doc injection path never
//! reaches into the [`dasclaw_hooks`] system.
//!
//! The check is intentionally a build-time / source-grep test rather than a
//! runtime assertion, because ADR-115 forbids *any* coupling — including a
//! latent dependency we'd later forget to delete. If you find yourself
//! adding a `dasclaw_hooks` import to satisfy a feature request, stop and
//! re-read ADR-115 § "为什么不进 hook 系统".

use std::fs;
use std::path::PathBuf;

fn project_docs_source() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/project_docs.rs")
}

#[test]
fn req_dasclaw_core_59b_adr115_no_dasclaw_hooks_reference() {
    let body = fs::read_to_string(project_docs_source())
        .expect("project_docs.rs must exist for #59b injection path");

    // Strip line comments and block comments before scanning so an ADR-115
    // explanatory comment that mentions the symbol doesn't fail the test.
    let stripped = strip_comments(&body);

    assert!(
        !stripped.contains("dasclaw_hooks"),
        "ADR-115 violation: project_docs.rs must not reference `dasclaw_hooks` \
         (found a non-comment occurrence). The injection path is constructor \
         DI only."
    );
}

fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Block comment
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            continue;
        }
        // Line comment (covers both `//` and `///`)
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}
