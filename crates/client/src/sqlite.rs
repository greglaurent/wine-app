//! Low-level libsqlite3 FFI helpers shared across the wasm client modules.

use std::ffi::{c_int, CStr};
use std::ptr;

pub use sqlite_wasm_rs as ffi;

pub const DB: &CStr = c"wine-offline.db";

pub unsafe fn open() -> Result<*mut ffi::sqlite3, String> {
    let mut db: *mut ffi::sqlite3 = ptr::null_mut();
    let rc = ffi::sqlite3_open_v2(
        DB.as_ptr(),
        &mut db,
        (ffi::SQLITE_OPEN_READWRITE | ffi::SQLITE_OPEN_CREATE) as c_int,
        ptr::null(),
    );
    if rc != ffi::SQLITE_OK {
        return Err(format!("sqlite open rc={rc}"));
    }
    Ok(db)
}

pub unsafe fn close(db: *mut ffi::sqlite3) {
    ffi::sqlite3_close(db);
}

/// Run a single statement (DDL / INSERT / UPDATE) via prepare+step.
pub unsafe fn run_sql(db: *mut ffi::sqlite3, sql: &CStr) -> Result<(), String> {
    let mut stmt: *mut ffi::sqlite3_stmt = ptr::null_mut();
    let rc = ffi::sqlite3_prepare_v2(db, sql.as_ptr(), -1, &mut stmt, ptr::null_mut());
    if rc != ffi::SQLITE_OK {
        return Err(format!("sqlite prepare rc={rc}"));
    }
    let rc = ffi::sqlite3_step(stmt);
    ffi::sqlite3_finalize(stmt);
    if rc == ffi::SQLITE_DONE || rc == ffi::SQLITE_ROW {
        Ok(())
    } else {
        Err(format!("sqlite step rc={rc}"))
    }
}

/// Run a scalar-count query and return the integer.
pub unsafe fn count(db: *mut ffi::sqlite3, sql: &CStr) -> Result<i64, String> {
    let mut stmt: *mut ffi::sqlite3_stmt = ptr::null_mut();
    if ffi::sqlite3_prepare_v2(db, sql.as_ptr(), -1, &mut stmt, ptr::null_mut()) != ffi::SQLITE_OK {
        return Err("sqlite prepare (count)".into());
    }
    let n = if ffi::sqlite3_step(stmt) == ffi::SQLITE_ROW {
        ffi::sqlite3_column_int64(stmt, 0)
    } else {
        -1
    };
    ffi::sqlite3_finalize(stmt);
    Ok(n)
}

pub unsafe fn col_text(stmt: *mut ffi::sqlite3_stmt, i: c_int) -> Option<String> {
    let t = ffi::sqlite3_column_text(stmt, i);
    if t.is_null() {
        None
    } else {
        Some(CStr::from_ptr(t.cast()).to_string_lossy().into_owned())
    }
}

/// Single text value (or None) for a query.
pub unsafe fn query_text(db: *mut ffi::sqlite3, sql: &CStr) -> Option<String> {
    let mut stmt: *mut ffi::sqlite3_stmt = ptr::null_mut();
    if ffi::sqlite3_prepare_v2(db, sql.as_ptr(), -1, &mut stmt, ptr::null_mut()) != ffi::SQLITE_OK {
        return None;
    }
    let v = if ffi::sqlite3_step(stmt) == ffi::SQLITE_ROW {
        col_text(stmt, 0)
    } else {
        None
    };
    ffi::sqlite3_finalize(stmt);
    v
}
