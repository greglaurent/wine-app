//! OPFS VFS install, the local schema, sync-cursor state, and id resume.

use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};

use sqlite_wasm_rs::WasmOsCallback;
use sqlite_wasm_vfs::sahpool::{install as install_opfs_sahpool, OpfsSAHPoolCfg};
use wine_core::ids::SOURCE_BITS;

use crate::sqlite::{ffi, query_text, run_sql};

/// This browser's id source (distinct from the server's `1`).
pub const CLIENT_SOURCE: u128 = 2;

static SAHPOOL_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Install the OPFS sahpool VFS once per worker, as the default VFS.
pub async fn ensure_sahpool() -> Result<(), String> {
    if !SAHPOOL_INSTALLED.swap(true, Ordering::SeqCst) {
        install_opfs_sahpool::<WasmOsCallback>(&OpfsSAHPoolCfg::default(), true)
            .await
            .map_err(|e| format!("install sahpool: {e:?}"))?;
    }
    Ok(())
}

/// Local schema (SQLite dialect). `bottle` is the bidirectional user table (has
/// `dirty`); the rest mirror the server's reference vocabulary (pull-only,
/// columns match the `/sync/reference` JSON + a `deleted_at` tombstone).
pub unsafe fn ensure_schema(db: *mut ffi::sqlite3) -> Result<(), String> {
    run_sql(
        db,
        c"CREATE TABLE IF NOT EXISTS bottle (
            id TEXT PRIMARY KEY, lot_id TEXT, status TEXT DEFAULT 'in_cellar',
            updated_at INTEGER, deleted_at INTEGER, revision INTEGER DEFAULT 1,
            dirty INTEGER DEFAULT 1)",
    )?;
    run_sql(
        db,
        c"CREATE TABLE IF NOT EXISTS sync_state (k TEXT PRIMARY KEY, v TEXT)",
    )?;
    // Reference mirror tables (no FKs -- it is a cache; server is authoritative).
    run_sql(db, c"CREATE TABLE IF NOT EXISTS country (
        id TEXT PRIMARY KEY, iso2 TEXT, iso3 TEXT, name TEXT, source TEXT, deleted_at INTEGER)")?;
    run_sql(db, c"CREATE TABLE IF NOT EXISTS bottle_format (
        id TEXT PRIMARY KEY, code TEXT, name TEXT, volume_ml INTEGER, source TEXT, deleted_at INTEGER)")?;
    run_sql(db, c"CREATE TABLE IF NOT EXISTS descriptor (
        id TEXT PRIMARY KEY, code TEXT, name TEXT, category TEXT, source TEXT, deleted_at INTEGER)")?;
    run_sql(db, c"CREATE TABLE IF NOT EXISTS appellation_type (
        id TEXT PRIMARY KEY, country_id TEXT, code TEXT, name TEXT, ordinal INTEGER,
        is_legal INTEGER, is_composite INTEGER, source TEXT, deleted_at INTEGER)")?;
    run_sql(db, c"CREATE TABLE IF NOT EXISTS appellation_tier (
        id TEXT PRIMARY KEY, country_id TEXT, code TEXT, name TEXT, rank INTEGER, source TEXT, deleted_at INTEGER)")?;
    run_sql(db, c"CREATE TABLE IF NOT EXISTS classification_system (
        id TEXT PRIMARY KEY, country_id TEXT, code TEXT, name TEXT, scope TEXT,
        established INTEGER, revised INTEGER, notes TEXT, source TEXT, deleted_at INTEGER)")?;
    run_sql(db, c"CREATE TABLE IF NOT EXISTS classification_level (
        id TEXT PRIMARY KEY, system_id TEXT, code TEXT, name TEXT, rank INTEGER, source TEXT, deleted_at INTEGER)")?;
    run_sql(db, c"CREATE TABLE IF NOT EXISTS label_rule (
        id TEXT PRIMARY KEY, country_id TEXT, kind TEXT, condition TEXT, min_percent INTEGER,
        tolerance_percent INTEGER, notes TEXT, source TEXT, deleted_at INTEGER)")
}

/// Every synced data table (NOT `sync_state`). Used to wipe + rebuild the local
/// store when the server epoch changes (a server nuke).
const DATA_TABLES: &[&str] = &[
    "bottle",
    "country",
    "bottle_format",
    "descriptor",
    "appellation_type",
    "appellation_tier",
    "classification_system",
    "classification_level",
    "label_rule",
];

/// Drop every synced data table and recreate them empty (keeps `sync_state` so we
/// can store the new epoch + reset cursors). The client re-hydrates on next sync.
pub unsafe fn reset_local(db: *mut ffi::sqlite3) -> Result<(), String> {
    for t in DATA_TABLES {
        let sql = CString::new(format!("DROP TABLE IF EXISTS {t}")).map_err(|e| e.to_string())?;
        run_sql(db, &sql)?;
    }
    ensure_schema(db)
}

pub unsafe fn sync_get(db: *mut ffi::sqlite3, key: &str) -> Option<String> {
    let sql = CString::new(format!("SELECT v FROM sync_state WHERE k='{key}'")).ok()?;
    query_text(db, &sql)
}

pub unsafe fn sync_set(db: *mut ffi::sqlite3, key: &str, val: &str) -> Result<(), String> {
    let sql = CString::new(format!(
        "INSERT OR REPLACE INTO sync_state (k, v) VALUES ('{key}', '{val}')"
    ))
    .map_err(|e| e.to_string())?;
    run_sql(db, &sql)
}

/// Highest Lamport tick in the local store (so we resume past it).
pub unsafe fn local_max_tick(db: *mut ffi::sqlite3) -> u128 {
    match query_text(db, c"SELECT max(id) FROM bottle") {
        Some(hex) => u128::from_str_radix(&hex, 16).unwrap_or(0) >> SOURCE_BITS,
        None => 0,
    }
}
