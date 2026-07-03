//! Askama templates (the shared renderer). Lives in core so the server and
//! the wasm client render identical HTML.

use askama::Template;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub title: String,
    pub bottle_count: i64,
    pub updated: String,
    /// Hashed asset URLs resolved from Vite's manifest.
    pub offline_js: String,
    pub offline_css: String,
}

/// Standalone fragment for the datastar `@get('/fragments/bottle-count')`
/// round-trip. Same markup the index includes, so the morph-by-id is seamless.
#[derive(Template)]
#[template(path = "fragments/bottle_count.html")]
pub struct BottleCountFragment {
    pub bottle_count: i64,
    pub updated: String,
}

/// Render the bottle-count fragment to HTML. Shared by the native server and
/// the wasm client so both produce byte-identical markup. (String error keeps
/// callers free of an `askama` dependency.)
pub fn render_bottle_count(bottle_count: i64, updated: &str) -> Result<String, String> {
    BottleCountFragment {
        bottle_count,
        updated: updated.to_string(),
    }
    .render()
    .map_err(|e| e.to_string())
}
