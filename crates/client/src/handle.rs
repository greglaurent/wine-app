//! The local backend: serve datastar requests against the OPFS store, rendering
//! the same `core` templates the server uses.

use std::ffi::CString;

use wasm_bindgen::prelude::*;
use wine_core::ids::IdGen;

use crate::sqlite::{close, count, open, run_sql};
use crate::store::{local_max_tick, sync_get, CLIENT_SOURCE};

fn now_ms() -> i64 {
    js_sys::Date::now() as i64
}

#[wasm_bindgen]
pub fn handle(method: String, path: String, _body: String) -> Result<String, JsValue> {
    unsafe {
        let db = open()?;
        let result: Result<String, String> = match (method.as_str(), path.as_str()) {
            ("POST", "/bottles") => {
                let lot = sync_get(db, "default_lot").unwrap_or_default();
                let id = IdGen::resume(CLIENT_SOURCE, local_max_tick(db)).next();
                let sql = CString::new(format!(
                    "INSERT INTO bottle (id, lot_id, status, updated_at, revision, dirty)
                     VALUES ('{}', '{}', 'in_cellar', {}, 1, 1)",
                    id.0,
                    lot,
                    now_ms()
                ))
                .map_err(|e| e.to_string())?;
                run_sql(db, &sql)?;
                let n = count(db, c"SELECT count(*) FROM bottle WHERE deleted_at IS NULL")?;
                wine_core::views::render_bottle_count(n, "local-first")
            }
            ("GET", "/fragments/bottle-count") => {
                let n = count(db, c"SELECT count(*) FROM bottle WHERE deleted_at IS NULL")?;
                wine_core::views::render_bottle_count(n, "local-first")
            }
            // Reference-vocabulary row counts in the local store (dev/diagnostic).
            ("GET", "/fragments/ref-stats") => {
                let countries = count(db, c"SELECT count(*) FROM country")?;
                let descriptors = count(db, c"SELECT count(*) FROM descriptor")?;
                let types = count(db, c"SELECT count(*) FROM appellation_type")?;
                let levels = count(db, c"SELECT count(*) FROM classification_level")?;
                Ok(format!(
                    "<p id=\"ref-stats\">countries={countries} descriptors={descriptors} \
                     appellation_types={types} classification_levels={levels}</p>"
                ))
            }
            _ => Ok(format!("<p id=\"bottle-count\">no local handler for {method} {path}</p>")),
        };
        close(db);
        result.map_err(JsValue::from)
    }
}
