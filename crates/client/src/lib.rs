//! wine-client -- wasm local-first backend.
//!
//! Runs in a dedicated Web Worker (OPFS SyncAccessHandle requires one). Holds
//! the local OPFS SQLite and answers datastar requests with the same `core`
//! templates the server uses, plus a sync engine that reconciles with the
//! server via /sync/pull + /sync/push.
//!
//! Build: `just build-client` (cargo build --target wasm32 + wasm-bindgen).
//!
//! wasm-bindgen exports: `init_db`, `handle` (handle.rs),
//! `get_dirty` / `apply_acks` / `apply_pull` / `get_cursor` (sync.rs).

#[cfg(target_arch = "wasm32")]
mod handle;
#[cfg(target_arch = "wasm32")]
mod sqlite;
#[cfg(target_arch = "wasm32")]
mod store;
#[cfg(target_arch = "wasm32")]
mod sync;

/// One-time init: install the OPFS VFS and create the schema. The worker awaits
/// this once before handling any messages.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn init_db() -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::JsValue;

    console_error_panic_hook::set_once();
    store::ensure_sahpool().await.map_err(JsValue::from)?;
    unsafe {
        let db = sqlite::open()?;
        store::ensure_schema(db)?;
        sqlite::close(db);
    }
    Ok(())
}
