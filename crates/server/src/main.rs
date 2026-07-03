//! wine-server -- Axum + sqlx/Postgres. Online sync target + source of truth.

mod assets;
mod db;
mod seed;
mod state;
mod sync;
mod web;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use tower_http::services::ServeDir;
use wine_core::ids::IdGen;

use assets::Assets;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let db_url = std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL must be set"))?;
    let bind = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8090".into());

    let pool = db::connect(&db_url).await?;
    db::run_migrations(&pool).await?;

    // Server id source = 1; resume past the highest persisted tick so restarts
    // don't reuse ids.
    let resume_tick = db::max_persisted_tick(&pool).await?;
    let ids = Arc::new(IdGen::resume(1, resume_tick));
    tracing::info!("id factory resuming from tick {resume_tick}");

    // Reconcile reference data (countries, formats, descriptors, the labeling
    // vocabulary) from core::seed. Demo catalog/lot is temporary scaffolding for
    // the bottle-add demo until the real entry form lands.
    seed::sync_reference_data(&pool, &ids).await?;
    let demo_lot = db::ensure_seed(&pool, &ids).await?;
    let assets = Assets::load()?;

    let state = AppState {
        pool,
        ids,
        demo_lot,
        assets,
    };

    let app = Router::new()
        .route("/", get(web::index))
        .route("/bottles", post(web::add_bottle))
        .route("/fragments/bottle-count", get(web::bottle_count_fragment))
        .route("/sync/meta", get(sync::meta))
        .route("/sync/pull", get(sync::pull))
        .route("/sync/reference", get(sync::reference_pull))
        .route("/sync/push", post(sync::push))
        .route("/health", get(|| async { "ok" }))
        // Vite build output (hashed assets, /sw.js at root scope, icon, manifest).
        .fallback_service(ServeDir::new("web/dist"))
        .with_state(state);

    let addr: SocketAddr = bind.parse()?;
    tracing::info!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
