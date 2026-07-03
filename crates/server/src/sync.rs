//! Sync API: `/sync/pull` (server->client deltas) and `/sync/push` (client->server
//! upserts with last-write-wins). The `server_seq` trigger advances the cursor.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use sqlx::PgPool;
use wine_core::sync::{
    MetaResponse, PullResponse, PushAck, PushRequest, PushResponse, RefPullResponse, RefRow,
    SyncBottle,
};

use crate::state::AppState;

#[derive(Deserialize)]
pub struct SinceQuery {
    since: Option<i64>,
}

fn ise(e: sqlx::Error) -> StatusCode {
    tracing::error!("sync error: {e}");
    StatusCode::INTERNAL_SERVER_ERROR
}

/// The server instance epoch (changes on a fresh DB). Clients reset when it moves.
pub async fn meta(State(st): State<AppState>) -> Result<Json<MetaResponse>, StatusCode> {
    let epoch: String = sqlx::query_scalar("SELECT epoch FROM server_meta")
        .fetch_one(&st.pool)
        .await
        .map_err(ise)?;
    Ok(Json(MetaResponse { epoch }))
}

/// Reference tables, projected to a generic `(table, server_seq, deleted_at_ms,
/// data_json)` stream. `deleted_at` rides the envelope; `data` is the business
/// columns the client mirrors. Ordered by the global `server_seq` cursor.
const REFERENCE_PULL_SQL: &str = "\
SELECT t, server_seq, deleted_ms, data FROM (
  SELECT 'country' t, server_seq, (extract(epoch FROM deleted_at)*1000)::bigint deleted_ms,
    jsonb_build_object('id',id,'iso2',iso2,'iso3',iso3,'name',name,'source',source)::text data
  FROM country WHERE server_seq > $1
  UNION ALL
  SELECT 'bottle_format', server_seq, (extract(epoch FROM deleted_at)*1000)::bigint,
    jsonb_build_object('id',id,'code',code,'name',name,'volume_ml',volume_ml,'source',source)::text
  FROM bottle_format WHERE server_seq > $1
  UNION ALL
  SELECT 'descriptor', server_seq, (extract(epoch FROM deleted_at)*1000)::bigint,
    jsonb_build_object('id',id,'code',code,'name',name,'category',category,'source',source)::text
  FROM descriptor WHERE server_seq > $1
  UNION ALL
  SELECT 'appellation_type', server_seq, (extract(epoch FROM deleted_at)*1000)::bigint,
    jsonb_build_object('id',id,'country_id',country_id,'code',code,'name',name,'ordinal',ordinal,
      'is_legal',is_legal,'is_composite',is_composite,'source',source)::text
  FROM appellation_type WHERE server_seq > $1
  UNION ALL
  SELECT 'appellation_tier', server_seq, (extract(epoch FROM deleted_at)*1000)::bigint,
    jsonb_build_object('id',id,'country_id',country_id,'code',code,'name',name,'rank',rank,'source',source)::text
  FROM appellation_tier WHERE server_seq > $1
  UNION ALL
  SELECT 'classification_system', server_seq, (extract(epoch FROM deleted_at)*1000)::bigint,
    jsonb_build_object('id',id,'country_id',country_id,'code',code,'name',name,'scope',scope,
      'established',established,'revised',revised,'notes',notes,'source',source)::text
  FROM classification_system WHERE server_seq > $1
  UNION ALL
  SELECT 'classification_level', server_seq, (extract(epoch FROM deleted_at)*1000)::bigint,
    jsonb_build_object('id',id,'system_id',system_id,'code',code,'name',name,'rank',rank,'source',source)::text
  FROM classification_level WHERE server_seq > $1
  UNION ALL
  SELECT 'label_rule', server_seq, (extract(epoch FROM deleted_at)*1000)::bigint,
    jsonb_build_object('id',id,'country_id',country_id,'kind',kind,'condition',condition,
      'min_percent',min_percent,'tolerance_percent',tolerance_percent,'notes',notes,'source',source)::text
  FROM label_rule WHERE server_seq > $1
) u ORDER BY server_seq";

/// Server -> client, PULL-ONLY: reference rows changed since the client's cursor,
/// table-tagged and applied blindly (server-authoritative). Distinct from the
/// bidirectional bottle sync.
pub async fn reference_pull(
    State(st): State<AppState>,
    Query(q): Query<SinceQuery>,
) -> Result<Json<RefPullResponse>, StatusCode> {
    let since = q.since.unwrap_or(0);
    let rows: Vec<(String, i64, Option<i64>, String)> = sqlx::query_as(REFERENCE_PULL_SQL)
        .bind(since)
        .fetch_all(&st.pool)
        .await
        .map_err(ise)?;

    let mut cursor = since;
    let mut out = Vec::with_capacity(rows.len());
    for (table, server_seq, deleted_at, data) in rows {
        cursor = cursor.max(server_seq);
        let data = serde_json::from_str(&data).map_err(|e| {
            tracing::error!("reference data parse: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        out.push(RefRow {
            table,
            deleted_at,
            data,
        });
    }
    Ok(Json(RefPullResponse { cursor, rows: out }))
}

async fn bottle_cursor(pool: &PgPool) -> Result<i64, StatusCode> {
    sqlx::query_scalar("SELECT coalesce(max(server_seq), 0) FROM bottle")
        .fetch_one(pool)
        .await
        .map_err(ise)
}

/// Server -> client: bottles changed since the client's cursor, plus the default
/// lot id so offline-created bottles have a valid home.
pub async fn pull(
    State(st): State<AppState>,
    Query(q): Query<SinceQuery>,
) -> Result<Json<PullResponse>, StatusCode> {
    let since = q.since.unwrap_or(0);
    let rows: Vec<(String, String, String, i64, Option<i64>, i64)> = sqlx::query_as(
        "SELECT id, lot_id, status,
                (extract(epoch FROM updated_at) * 1000)::bigint,
                (extract(epoch FROM deleted_at) * 1000)::bigint,
                revision
         FROM bottle WHERE server_seq > $1 ORDER BY server_seq",
    )
    .bind(since)
    .fetch_all(&st.pool)
    .await
    .map_err(ise)?;

    let bottles = rows
        .into_iter()
        .map(|(id, lot_id, status, updated_at, deleted_at, revision)| SyncBottle {
            id,
            lot_id,
            status,
            updated_at,
            deleted_at,
            revision,
        })
        .collect();

    let cursor = bottle_cursor(&st.pool).await?;
    let default_lot = sqlx::query_scalar("SELECT id FROM lot ORDER BY id LIMIT 1")
        .fetch_optional(&st.pool)
        .await
        .map_err(ise)?;

    Ok(Json(PullResponse {
        cursor,
        default_lot,
        bottles,
    }))
}

/// Client -> server: upsert each bottle with last-write-wins (newer `updated_at`
/// wins; Lamport id breaks ties).
pub async fn push(
    State(st): State<AppState>,
    Json(req): Json<PushRequest>,
) -> Result<Json<PushResponse>, StatusCode> {
    let mut acks = Vec::with_capacity(req.bottles.len());
    for b in &req.bottles {
        sqlx::query(
            "INSERT INTO bottle (id, lot_id, status, updated_at, deleted_at, revision)
             VALUES ($1, $2, $3, to_timestamp($4 / 1000.0), to_timestamp($5 / 1000.0), $6)
             ON CONFLICT (id) DO UPDATE SET
                 lot_id = excluded.lot_id, status = excluded.status,
                 updated_at = excluded.updated_at, deleted_at = excluded.deleted_at,
                 revision = excluded.revision
             WHERE excluded.updated_at > bottle.updated_at
                OR (excluded.updated_at = bottle.updated_at AND excluded.id > bottle.id)",
        )
        .bind(&b.id)
        .bind(&b.lot_id)
        .bind(&b.status)
        .bind(b.updated_at)
        .bind(b.deleted_at)
        .bind(b.revision)
        .execute(&st.pool)
        .await
        .map_err(ise)?;

        let server_seq = sqlx::query_scalar("SELECT server_seq FROM bottle WHERE id = $1")
            .bind(&b.id)
            .fetch_one(&st.pool)
            .await
            .map_err(ise)?;
        acks.push(PushAck {
            id: b.id.clone(),
            server_seq,
        });
    }

    let cursor = bottle_cursor(&st.pool).await?;
    Ok(Json(PushResponse { cursor, acks }))
}
