//! `dasclaw_image_tools` — vision / image LLM tools.
//!
//! Verbatim port of `desktop-client/ironclaw/src/tools/builtin/image_*` per
//! ADR-156 §6.3 F4.6.2.
//!
//! # Modules
//!
//! - [`image_analyze`] — analyze an image via a vision-capable LLM.
//! - [`image_edit`] — edit an image via image edit API.
//! - [`image_gen`] — generate an image via image generation API.

pub mod image_analyze;
pub mod image_edit;
pub mod image_gen;

pub use image_analyze::ImageAnalyzeTool;
pub use image_edit::ImageEditTool;
pub use image_gen::ImageGenerateTool;

/// Detect image media type from file extension via `mime_guess`.
/// Falls back to `image/jpeg` for unrecognized or non-image extensions.
pub(crate) fn media_type_from_path(path: &str) -> String {
    mime_guess::from_path(path)
        .first_raw()
        .filter(|m| m.starts_with("image/"))
        .unwrap_or("image/jpeg")
        .to_string()
}
