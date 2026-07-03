//! Shared application state.

use std::sync::Arc;

use sqlx::PgPool;
use wine_core::ids::IdGen;

use crate::assets::Assets;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub ids: Arc<IdGen>,
    /// Temporary demo lot that bottles are inserted into until the real
    /// bottle-entry form (and catalog) land.
    pub demo_lot: String,
    /// Hashed asset URLs from Vite's manifest.
    pub assets: Assets,
}
