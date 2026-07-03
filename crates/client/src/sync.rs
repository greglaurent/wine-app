//! Sync engine -- applies pull/push results to the local store. The JS side does
//! the HTTP; these functions touch only the local DB.

use std::ffi::CString;
use std::ptr;

use wasm_bindgen::prelude::*;
use wine_core::sync::{
    MetaResponse, PullResponse, PushRequest, PushResponse, RefPullResponse, RefRow, SyncBottle,
};

use crate::sqlite::{close, col_text, ffi, open, run_sql};
use crate::store::{reset_local, sync_get, sync_set};

/// Rows that still need pushing, as the server's PushRequest JSON.
#[wasm_bindgen]
pub fn get_dirty() -> Result<String, JsValue> {
    unsafe {
        let db = open()?;
        let mut stmt: *mut ffi::sqlite3_stmt = ptr::null_mut();
        let sql =
            c"SELECT id, lot_id, status, updated_at, deleted_at, revision FROM bottle WHERE dirty = 1";
        if ffi::sqlite3_prepare_v2(db, sql.as_ptr(), -1, &mut stmt, ptr::null_mut()) != ffi::SQLITE_OK
        {
            return Err("prepare dirty".into());
        }
        let mut bottles = Vec::new();
        while ffi::sqlite3_step(stmt) == ffi::SQLITE_ROW {
            let deleted_at = if ffi::sqlite3_column_type(stmt, 4) == ffi::SQLITE_NULL {
                None
            } else {
                Some(ffi::sqlite3_column_int64(stmt, 4))
            };
            bottles.push(SyncBottle {
                id: col_text(stmt, 0).unwrap_or_default(),
                lot_id: col_text(stmt, 1).unwrap_or_default(),
                status: col_text(stmt, 2).unwrap_or_default(),
                updated_at: ffi::sqlite3_column_int64(stmt, 3),
                deleted_at,
                revision: ffi::sqlite3_column_int64(stmt, 5),
            });
        }
        ffi::sqlite3_finalize(stmt);
        close(db);
        serde_json::to_string(&PushRequest { bottles }).map_err(|e| JsValue::from(e.to_string()))
    }
}

/// Apply server push acks: clear `dirty` for accepted rows, advance the cursor.
#[wasm_bindgen]
pub fn apply_acks(json: String) -> Result<(), JsValue> {
    let resp: PushResponse = serde_json::from_str(&json).map_err(|e| JsValue::from(e.to_string()))?;
    unsafe {
        let db = open()?;
        for ack in &resp.acks {
            let sql = CString::new(format!("UPDATE bottle SET dirty = 0 WHERE id = '{}'", ack.id))
                .map_err(|e| e.to_string())?;
            run_sql(db, &sql)?;
        }
        sync_set(db, "cursor", &resp.cursor.to_string())?;
        close(db);
    }
    Ok(())
}

/// Apply a server pull: upsert bottles with last-write-wins, store the cursor +
/// default lot id.
#[wasm_bindgen]
pub fn apply_pull(json: String) -> Result<(), JsValue> {
    let resp: PullResponse = serde_json::from_str(&json).map_err(|e| JsValue::from(e.to_string()))?;
    unsafe {
        let db = open()?;
        for b in &resp.bottles {
            let deleted = b
                .deleted_at
                .map(|d| d.to_string())
                .unwrap_or_else(|| "NULL".into());
            let sql = CString::new(format!(
                "INSERT INTO bottle (id, lot_id, status, updated_at, deleted_at, revision, dirty)
                 VALUES ('{}', '{}', '{}', {}, {}, {}, 0)
                 ON CONFLICT(id) DO UPDATE SET
                     lot_id = excluded.lot_id, status = excluded.status,
                     updated_at = excluded.updated_at, deleted_at = excluded.deleted_at,
                     revision = excluded.revision, dirty = 0
                 WHERE excluded.updated_at > bottle.updated_at",
                b.id, b.lot_id, b.status, b.updated_at, deleted, b.revision
            ))
            .map_err(|e| e.to_string())?;
            run_sql(db, &sql)?;
        }
        sync_set(db, "cursor", &resp.cursor.to_string())?;
        if let Some(lot) = &resp.default_lot {
            sync_set(db, "default_lot", lot)?;
        }
        close(db);
    }
    Ok(())
}

/// Compare the server's epoch to the stored one. On a MISMATCH after a prior
/// epoch was recorded (i.e. the server DB was nuked), wipe the local store and
/// reset both cursors to 0 so the next pulls re-hydrate from scratch. Returns
/// true iff a reset happened. On first run (no stored epoch) just records it.
#[wasm_bindgen]
pub fn check_epoch(json: String) -> Result<bool, JsValue> {
    let meta: MetaResponse =
        serde_json::from_str(&json).map_err(|e| JsValue::from(e.to_string()))?;
    unsafe {
        let db = open()?;
        let stored = sync_get(db, "epoch");
        let reset = match &stored {
            Some(prev) if prev != &meta.epoch => {
                reset_local(db)?;
                sync_set(db, "cursor", "0")?;
                sync_set(db, "ref_cursor", "0")?;
                true
            }
            _ => false,
        };
        if stored.as_deref() != Some(meta.epoch.as_str()) {
            sync_set(db, "epoch", &meta.epoch)?;
        }
        close(db);
        Ok(reset)
    }
}

/// The client's current sync cursor (max server_seq it has applied).
#[wasm_bindgen]
pub fn get_cursor() -> Result<i64, JsValue> {
    unsafe {
        let db = open()?;
        let c = sync_get(db, "cursor")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        close(db);
        Ok(c)
    }
}

/// The reference vocabulary is server-authoritative and PULL-ONLY: its own cursor,
/// separate from the bidirectional bottle cursor.
#[wasm_bindgen]
pub fn get_ref_cursor() -> Result<i64, JsValue> {
    unsafe {
        let db = open()?;
        let c = sync_get(db, "ref_cursor")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        close(db);
        Ok(c)
    }
}

/// Apply a reference pull: blind upsert each table-tagged row (server wins), then
/// advance the reference cursor.
#[wasm_bindgen]
pub fn apply_reference(json: String) -> Result<(), JsValue> {
    let resp: RefPullResponse =
        serde_json::from_str(&json).map_err(|e| JsValue::from(e.to_string()))?;
    unsafe {
        let db = open()?;
        for row in &resp.rows {
            apply_ref_row(db, row)?;
        }
        sync_set(db, "ref_cursor", &resp.cursor.to_string())?;
        close(db);
    }
    Ok(())
}

/// Whitelist of mirror tables we will write to (the server is trusted, but never
/// build SQL with an unvetted table name).
fn is_reference_table(t: &str) -> bool {
    matches!(
        t,
        "country"
            | "bottle_format"
            | "descriptor"
            | "appellation_type"
            | "appellation_tier"
            | "classification_system"
            | "classification_level"
            | "label_rule"
    )
}

/// A JSON scalar as a SQLite literal (strings single-quoted and escaped).
fn sql_literal(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::Bool(b) => if *b { "1" } else { "0" }.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        other => format!("'{}'", other.to_string().replace('\'', "''")),
    }
}

/// Generic upsert of one reference row: columns are the JSON keys, plus the
/// `deleted_at` tombstone from the envelope. Server wins (no LWW), so plain
/// `ON CONFLICT(id) DO UPDATE`.
unsafe fn apply_ref_row(db: *mut ffi::sqlite3, row: &RefRow) -> Result<(), String> {
    if !is_reference_table(&row.table) {
        return Err(format!("unknown reference table: {}", row.table));
    }
    let obj = row
        .data
        .as_object()
        .ok_or("reference row data is not an object")?;

    let mut cols: Vec<&str> = Vec::with_capacity(obj.len() + 1);
    let mut vals: Vec<String> = Vec::with_capacity(obj.len() + 1);
    for (k, v) in obj {
        cols.push(k.as_str());
        vals.push(sql_literal(v));
    }
    cols.push("deleted_at");
    vals.push(
        row.deleted_at
            .map(|d| d.to_string())
            .unwrap_or_else(|| "NULL".to_string()),
    );

    let assignments = cols
        .iter()
        .map(|c| format!("{c}=excluded.{c}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = CString::new(format!(
        "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT(id) DO UPDATE SET {}",
        row.table,
        cols.join(", "),
        vals.join(", "),
        assignments
    ))
    .map_err(|e| e.to_string())?;
    run_sql(db, &sql)
}
