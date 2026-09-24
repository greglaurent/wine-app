//! Database-free, opt-in design preview. No production write/sync routes.

use askama::Template;
use axum::{http::StatusCode, response::Html, routing::get, Router};
use tower_http::services::ServeDir;
use wine_core::preview::PreviewTemplate;

async fn page() -> Result<Html<String>, StatusCode> {
    let render = || -> anyhow::Result<String> {
        let assets = crate::assets::Assets::load_entry("src/preview.ts")?;
        Ok(PreviewTemplate::new(assets.offline_js, assets.offline_css).render()?)
    };
    render().map(Html).map_err(|error| {
        tracing::error!(%error, "preview render failed");
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

pub async fn serve() -> anyhow::Result<()> {
    crate::assets::Assets::load_entry("src/preview.ts")?;
    let bind = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8091".into());
    let app = Router::new()
        .route("/", get(page))
        .route("/preview", get(page))
        .fallback_service(ServeDir::new("web/dist"));
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(%bind, "Barback static design preview; sample data only");
    axum::serve(listener, app).await?;
    Ok(())
}
