//! Domain types and enums.
//!
//! Enums here mirror the Postgres native enums / CHECK constraints in
//! migrations. Reference/catalog data lives in tables (see migrations),
//! not in Rust enums, so it stays extensible without a recompile.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::ids::Id;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WineColor {
    Red,
    White,
    Rose,
    Orange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WineStyle {
    Still,
    Sparkling,
    Fortified,
    Dessert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BottleStatus {
    InCellar,
    Consumed,
    Gifted,
    Sold,
    Lost,
    Reserved,
    Pending,
}

/// The sync envelope carried by every syncable row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncMeta {
    pub revision: i64,
    pub updated_at: OffsetDateTime,
    pub deleted_at: Option<OffsetDateTime>,
    pub server_seq: i64,
    pub created_at: OffsetDateTime,
}

/// One physical unit in the cellar. (Trimmed; full columns in migrations.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bottle {
    pub id: Id,
    pub lot_id: Id,
    pub status: BottleStatus,
    pub location_id: Option<Id>,
    pub position_rack: Option<String>,
    pub position_row: Option<i16>,
    pub position_column: Option<i16>,
    pub position_bin: Option<String>,
    pub position_depth: Option<i16>,
    pub sync: SyncMeta,
}
