//! wine-core -- shared domain, rendering, and sync logic.
//!
//! This crate is compiled into both the native server and the wasm client.
//! Keep it free of tokio / sqlx / networking so it stays wasm-safe.

pub mod ids;
pub mod model;
pub mod seed;
pub mod store;
pub mod sync;
pub mod views;
pub mod vocab;

pub use ids::Id;
