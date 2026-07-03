//! Sync wire types -- the JSON contract for `/sync/pull` and `/sync/push`,
//! shared by the native server and the wasm client so both agree on the shape.
//! Timestamps cross the wire as epoch milliseconds.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncBottle {
    pub id: String,
    pub lot_id: String,
    pub status: String,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub revision: i64,
}

/// Client -> server push body (also what the client's `get_dirty` returns).
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PushRequest {
    pub bottles: Vec<SyncBottle>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PushAck {
    pub id: String,
    pub server_seq: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PushResponse {
    pub cursor: i64,
    pub acks: Vec<PushAck>,
}

/// Server -> client pull response.
#[derive(Debug, Serialize, Deserialize)]
pub struct PullResponse {
    pub cursor: i64,
    pub default_lot: Option<String>,
    pub bottles: Vec<SyncBottle>,
}

/// One changed reference row in the generic, PULL-ONLY reference stream. The
/// reference vocabulary (countries, formats, descriptors, the labeling tables) is
/// server-authoritative, so unlike bottles it is table-tagged and applied blindly
/// (server always wins) -- no per-table DTO, no LWW. `data` holds the business
/// columns (including `id`) as a JSON object; the client upserts them by name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefRow {
    pub table: String,
    pub deleted_at: Option<i64>,
    pub data: serde_json::Value,
}

/// Server -> client reference pull. `cursor` is the high-water `server_seq`.
#[derive(Debug, Serialize, Deserialize)]
pub struct RefPullResponse {
    pub cursor: i64,
    pub rows: Vec<RefRow>,
}

/// Server instance metadata. `epoch` changes when the server DB is wiped (fresh
/// volume), so a client that sees a different epoch than it stored wipes its
/// local store and re-hydrates from scratch.
#[derive(Debug, Serialize, Deserialize)]
pub struct MetaResponse {
    pub epoch: String,
}
