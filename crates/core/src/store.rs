//! The `Store` trait -- the single seam between shared logic and storage.
//!
//! Server impl: sqlx / Postgres (in crates/server).
//! Client impl: sqlite-wasm-rs / local SQLite (in crates/client).
//! Handlers + templates above this trait are identical on both sides.

use async_trait::async_trait;

use crate::ids::Id;
use crate::model::Bottle;

#[derive(Debug)]
pub enum StoreError {
    NotFound,
    Backend(String),
}

#[async_trait(?Send)]
pub trait Store {
    async fn list_bottles_in_cellar(&self) -> Result<Vec<Bottle>, StoreError>;
    async fn get_bottle(&self, id: &Id) -> Result<Bottle, StoreError>;
    async fn upsert_bottle(&self, bottle: &Bottle) -> Result<(), StoreError>;
    // ... expands per feature; both impls satisfy the same contract.
}
