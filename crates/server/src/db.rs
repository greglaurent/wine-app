//! Database: pool, migrations, id-resume, and (temporary) demo seeding.

use sqlx::{postgres::PgPoolOptions, PgPool};
use wine_core::ids::{IdGen, SOURCE_BITS};

pub async fn connect(db_url: &str) -> anyhow::Result<PgPool> {
    Ok(PgPoolOptions::new()
        .max_connections(10)
        .connect(db_url)
        .await?)
}

/// Apply schema migrations (embedded at compile time by `sqlx::migrate!`).
/// A fresh volume self-initializes on first boot.
pub async fn run_migrations(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::migrate!("../../migrations").run(pool).await?;
    Ok(())
}

/// Highest Lamport tick already persisted, across the id-bearing tables the
/// server writes to, so the factory can resume past it on restart. (Single
/// source today; add a source filter once clients mint ids too.)
pub async fn max_persisted_tick(pool: &PgPool) -> anyhow::Result<u128> {
    let max_hex: Option<String> = sqlx::query_scalar(
        "SELECT max(id) FROM (
             SELECT id FROM country
             UNION ALL SELECT id FROM bottle_format
             UNION ALL SELECT id FROM descriptor
             UNION ALL SELECT id FROM appellation_type
             UNION ALL SELECT id FROM appellation_tier
             UNION ALL SELECT id FROM classification_system
             UNION ALL SELECT id FROM classification_level
             UNION ALL SELECT id FROM label_rule
             UNION ALL SELECT id FROM producer
             UNION ALL SELECT id FROM wine
             UNION ALL SELECT id FROM wine_vintage
             UNION ALL SELECT id FROM lot
             UNION ALL SELECT id FROM bottle
         ) ids",
    )
    .fetch_one(pool)
    .await?;
    Ok(match max_hex {
        Some(hex) => u128::from_str_radix(&hex, 16).unwrap_or(0) >> SOURCE_BITS,
        None => 0,
    })
}

/// Ensure a minimal catalog chain exists (country -> producer -> wine -> vintage ->
/// lot) so bottles have somewhere to live. Returns the demo lot id. Idempotent:
/// reuse the first existing lot. (Temporary -- real catalog entry replaces this.)
pub async fn ensure_seed(pool: &PgPool, ids: &IdGen) -> anyhow::Result<String> {
    if let Some(lot) = sqlx::query_scalar::<_, String>("SELECT id FROM lot LIMIT 1")
        .fetch_optional(pool)
        .await?
    {
        return Ok(lot);
    }

    // Reuse a seeded country (sync_reference_data runs first), don't mint one.
    let country: String = sqlx::query_scalar("SELECT id FROM country WHERE iso2 = 'US'")
        .fetch_one(pool)
        .await?;

    let producer = ids.next().0;
    sqlx::query("INSERT INTO producer (id, country_id, name) VALUES ($1, $2, $3)")
        .bind(&producer)
        .bind(&country)
        .bind("Demo Winery")
        .execute(pool)
        .await?;

    let wine = ids.next().0;
    sqlx::query("INSERT INTO wine (id, producer_id, country_id, name) VALUES ($1, $2, $3, $4)")
        .bind(&wine)
        .bind(&producer)
        .bind(&country)
        .bind("Demo Red")
        .execute(pool)
        .await?;

    let vintage = ids.next().0;
    sqlx::query("INSERT INTO wine_vintage (id, wine_id, year) VALUES ($1, $2, $3)")
        .bind(&vintage)
        .bind(&wine)
        .bind(2021_i16)
        .execute(pool)
        .await?;

    let lot = ids.next().0;
    sqlx::query("INSERT INTO lot (id, wine_vintage_id) VALUES ($1, $2)")
        .bind(&lot)
        .bind(&vintage)
        .execute(pool)
        .await?;

    tracing::info!("seeded demo catalog (lot {lot})");
    Ok(lot)
}
