//! W3 issue #54 — public surface contract for `ProjectDocLoader`.
//!
//! Locks the trait + struct + enum shape required by 32-execution-plan.md
//! W3 task 3 (E 类显式扩展). Downstream issues #55-#59 fill the impl; this
//! contract guards the API so they don't have to redesign it.

use std::path::{Path, PathBuf};

use dasclaw_project_docs::{DocLayer, ProjectDoc, ProjectDocLoader};

/// Placeholder loader (W1 skeleton-style) used to prove the trait is
/// object-safe and that the default behaviour is "return nothing".
struct EmptyLoader;

impl ProjectDocLoader for EmptyLoader {
    fn load(&self, _cwd: &Path) -> Vec<ProjectDoc> {
        Vec::new()
    }
}

#[test]
fn req_w3_54_loader_trait_is_object_safe() {
    // If the trait stops being object-safe (e.g. someone adds `Self: Sized`-incompatible
    // generics) downstream owners (#55-#59) cannot compose loaders behind a trait object.
    let loader: Box<dyn ProjectDocLoader> = Box::new(EmptyLoader);
    assert!(loader.load(Path::new("/tmp")).is_empty());
}

#[test]
fn req_w3_54_placeholder_load_returns_empty() {
    // The W3 acceptance criteria require a placeholder impl that returns
    // `Vec::new()` so downstream consumers can wire the trait without first
    // shipping a full multi-layer loader.
    let docs = EmptyLoader.load(Path::new("."));
    assert!(docs.is_empty(), "placeholder loader must return empty vec");
}

#[test]
fn req_w3_54_project_doc_struct_shape() {
    // Lock the struct shape; downstream issues will populate `content` /
    // `bytes` / `source_path` / `layer` from disk.
    let doc = ProjectDoc {
        content: "hello".to_string(),
        source_path: PathBuf::from("/tmp/AGENTS.md"),
        layer: DocLayer::Project,
        bytes: 5,
    };
    assert_eq!(doc.content, "hello");
    assert_eq!(doc.source_path, PathBuf::from("/tmp/AGENTS.md"));
    assert_eq!(doc.bytes, 5);
    assert!(matches!(doc.layer, DocLayer::Project));
}

#[test]
fn req_w3_54_doc_layer_three_variants() {
    // The 3-layer model (UserGlobal / Project / Cwd) is part of the issue's
    // contract — it appears in 31-target-architecture.md §4 and in #56's
    // 3-layer merge issue. Lock all three are constructible.
    let layers = [DocLayer::UserGlobal, DocLayer::Project, DocLayer::Cwd];
    assert_eq!(layers.len(), 3);
}
