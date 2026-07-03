//! Page + datastar-fragment handlers.

use askama::Template;
use axum::{extract::State, http::StatusCode, response::Html};
use sqlx::PgPool;
use time::OffsetDateTime;
use wine_core::views::{BottleCountFragment, IndexTemplate};

use crate::state::AppState;

pub async fn index(State(st): State<AppState>) -> Result<Html<String>, StatusCode> {
    let bottle_count = count_bottles(&st.pool).await?;
    // `current()` re-reads the Vite manifest in debug builds, so a `pnpm build`
    // is reflected without restarting the server.
    let assets = st.assets.current();
    render(IndexTemplate {
        title: "Wine Cellar".to_string(),
        bottle_count,
        updated: now_hms(),
        offline_js: assets.offline_js,
        offline_css: assets.offline_css,
    })
}

/// datastar `@get` target: just the count fragment, morphed in by its id.
pub async fn bottle_count_fragment(
    State(st): State<AppState>,
) -> Result<Html<String>, StatusCode> {
    let bottle_count = count_bottles(&st.pool).await?;
    render(BottleCountFragment {
        bottle_count,
        updated: now_hms(),
    })
}

/// datastar `@post` target: insert a bottle into the demo lot, return the
/// updated count fragment so the page morphs the new total in.
pub async fn add_bottle(State(st): State<AppState>) -> Result<Html<String>, StatusCode> {
    let id = st.ids.next();
    sqlx::query("INSERT INTO bottle (id, lot_id) VALUES ($1, $2)")
        .bind(&id.0)
        .bind(&st.demo_lot)
        .execute(&st.pool)
        .await
        .map_err(|e| {
            tracing::error!("insert bottle failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let bottle_count = count_bottles(&st.pool).await?;
    render(BottleCountFragment {
        bottle_count,
        updated: now_hms(),
    })
}

async fn count_bottles(pool: &PgPool) -> Result<i64, StatusCode> {
    sqlx::query_scalar("SELECT count(*) FROM bottle WHERE deleted_at IS NULL")
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!("count query failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

fn now_hms() -> String {
    let t = OffsetDateTime::now_utc();
    format!("{:02}:{:02}:{:02} UTC", t.hour(), t.minute(), t.second())
}

fn render<T: Template>(t: T) -> Result<Html<String>, StatusCode> {
    t.render().map(Html).map_err(|e| {
        tracing::error!("template render failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })
}
