//! Reconcile the server's reference ("seed") data to `core::seed`, on startup.
//!
//! The RON in `core` is the source; here we upsert it into Postgres keyed on each
//! table's natural key, generating a Lamport id for new rows and resolving
//! natural-key references (iso2, (country, code)) to ids as we go. Idempotent and
//! CHURN-FREE: a row is UPDATEd only when a value actually changed, so the
//! `server_seq` trigger fires only on real changes and clients pull true deltas.

use std::collections::HashMap;

use anyhow::Context;
use sqlx::PgPool;
use wine_core::ids::IdGen;
use wine_core::seed;

/// Existing classification_system row (id, name, scope, established, revised, notes).
type SystemRow = (String, String, String, Option<i16>, Option<i16>, Option<String>);

pub async fn sync_reference_data(pool: &PgPool, ids: &IdGen) -> anyhow::Result<()> {
    let data = seed::load()?;

    // Globals first; the country map resolves country-scoped references.
    let mut country_id: HashMap<&str, String> = HashMap::new();
    for c in &data.countries {
        country_id.insert(c.iso2.as_str(), upsert_country(pool, ids, c).await?);
    }
    for b in &data.bottle_formats {
        upsert_bottle_format(pool, ids, b).await?;
    }
    for d in &data.descriptors {
        upsert_descriptor(pool, ids, d).await?;
    }

    for t in &data.appellation_types {
        let cid = country_id
            .get(t.country.as_str())
            .with_context(|| format!("appellation_type {} -> country {}", t.code, t.country))?;
        upsert_appellation_type(pool, ids, t, cid).await?;
    }
    for t in &data.appellation_tiers {
        let cid = country_id
            .get(t.country.as_str())
            .with_context(|| format!("appellation_tier {} -> country {}", t.code, t.country))?;
        upsert_appellation_tier(pool, ids, t, cid).await?;
    }

    let mut system_id: HashMap<(&str, &str), String> = HashMap::new();
    for s in &data.classification_systems {
        let cid = country_id
            .get(s.country.as_str())
            .with_context(|| format!("classification_system {} -> country {}", s.code, s.country))?;
        let id = upsert_classification_system(pool, ids, s, cid).await?;
        system_id.insert((s.country.as_str(), s.code.as_str()), id);
    }
    for l in &data.classification_levels {
        let sid = system_id
            .get(&(l.country.as_str(), l.system.as_str()))
            .with_context(|| format!("classification_level {} -> system {}", l.code, l.system))?;
        upsert_classification_level(pool, ids, l, sid).await?;
    }
    for r in &data.label_rules {
        let cid = country_id
            .get(r.country.as_str())
            .with_context(|| format!("label_rule {}/{} -> country {}", r.kind, r.country, r.country))?;
        upsert_label_rule(pool, ids, r, cid).await?;
    }

    tracing::info!(
        "reference data synced: {} countries, {} formats, {} descriptors, {} appellation types, \
         {} tiers, {} classification systems, {} levels, {} label rules",
        data.countries.len(),
        data.bottle_formats.len(),
        data.descriptors.len(),
        data.appellation_types.len(),
        data.appellation_tiers.len(),
        data.classification_systems.len(),
        data.classification_levels.len(),
        data.label_rules.len(),
    );
    Ok(())
}

async fn upsert_country(pool: &PgPool, ids: &IdGen, c: &seed::Country) -> anyhow::Result<String> {
    let row: Option<(String, String, String)> =
        sqlx::query_as("SELECT id, iso3, name FROM country WHERE iso2 = $1")
            .bind(&c.iso2)
            .fetch_optional(pool)
            .await?;
    if let Some((id, iso3, name)) = row {
        if iso3 != c.iso3 || name != c.name {
            sqlx::query(
                "UPDATE country SET iso3=$1, name=$2, source='seed', \
                 revision=revision+1, updated_at=now() WHERE id=$3",
            )
            .bind(&c.iso3)
            .bind(&c.name)
            .bind(&id)
            .execute(pool)
            .await?;
        }
        return Ok(id);
    }
    let id = ids.next().0;
    sqlx::query("INSERT INTO country (id, iso2, iso3, name, source) VALUES ($1,$2,$3,$4,'seed')")
        .bind(&id)
        .bind(&c.iso2)
        .bind(&c.iso3)
        .bind(&c.name)
        .execute(pool)
        .await?;
    Ok(id)
}

async fn upsert_bottle_format(pool: &PgPool, ids: &IdGen, b: &seed::BottleFormat) -> anyhow::Result<()> {
    let row: Option<(String, String, i32)> =
        sqlx::query_as("SELECT id, name, volume_ml FROM bottle_format WHERE code = $1")
            .bind(&b.code)
            .fetch_optional(pool)
            .await?;
    match row {
        Some((id, name, vol)) if name != b.name || vol != b.volume_ml => {
            sqlx::query(
                "UPDATE bottle_format SET name=$1, volume_ml=$2, source='seed', \
                 revision=revision+1, updated_at=now() WHERE id=$3",
            )
            .bind(&b.name)
            .bind(b.volume_ml)
            .bind(&id)
            .execute(pool)
            .await?;
        }
        Some(_) => {}
        None => {
            sqlx::query(
                "INSERT INTO bottle_format (id, code, name, volume_ml, source) \
                 VALUES ($1,$2,$3,$4,'seed')",
            )
            .bind(ids.next().0)
            .bind(&b.code)
            .bind(&b.name)
            .bind(b.volume_ml)
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}

async fn upsert_descriptor(pool: &PgPool, ids: &IdGen, d: &seed::Descriptor) -> anyhow::Result<()> {
    let row: Option<(String, String, String)> =
        sqlx::query_as("SELECT id, name, category FROM descriptor WHERE code = $1")
            .bind(&d.code)
            .fetch_optional(pool)
            .await?;
    match row {
        Some((id, name, category)) if name != d.name || category != d.category => {
            sqlx::query(
                "UPDATE descriptor SET name=$1, category=$2, source='seed', \
                 revision=revision+1, updated_at=now() WHERE id=$3",
            )
            .bind(&d.name)
            .bind(&d.category)
            .bind(&id)
            .execute(pool)
            .await?;
        }
        Some(_) => {}
        None => {
            sqlx::query(
                "INSERT INTO descriptor (id, code, name, category, source) VALUES ($1,$2,$3,$4,'seed')",
            )
            .bind(ids.next().0)
            .bind(&d.code)
            .bind(&d.name)
            .bind(&d.category)
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}

async fn upsert_appellation_type(
    pool: &PgPool,
    ids: &IdGen,
    t: &seed::AppellationType,
    country_id: &str,
) -> anyhow::Result<()> {
    let row: Option<(String, String, i16, bool, bool)> = sqlx::query_as(
        "SELECT id, name, ordinal, is_legal, is_composite FROM appellation_type \
         WHERE country_id=$1 AND code=$2",
    )
    .bind(country_id)
    .bind(&t.code)
    .fetch_optional(pool)
    .await?;
    match row {
        Some((id, name, ordinal, is_legal, is_composite))
            if name != t.name
                || ordinal != t.ordinal
                || is_legal != t.is_legal
                || is_composite != t.is_composite =>
        {
            sqlx::query(
                "UPDATE appellation_type SET name=$1, ordinal=$2, is_legal=$3, is_composite=$4, \
                 source='seed', revision=revision+1, updated_at=now() WHERE id=$5",
            )
            .bind(&t.name)
            .bind(t.ordinal)
            .bind(t.is_legal)
            .bind(t.is_composite)
            .bind(&id)
            .execute(pool)
            .await?;
        }
        Some(_) => {}
        None => {
            sqlx::query(
                "INSERT INTO appellation_type (id, country_id, code, name, ordinal, is_legal, is_composite, source) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,'seed')",
            )
            .bind(ids.next().0)
            .bind(country_id)
            .bind(&t.code)
            .bind(&t.name)
            .bind(t.ordinal)
            .bind(t.is_legal)
            .bind(t.is_composite)
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}

async fn upsert_appellation_tier(
    pool: &PgPool,
    ids: &IdGen,
    t: &seed::AppellationTier,
    country_id: &str,
) -> anyhow::Result<()> {
    let row: Option<(String, String, i16)> = sqlx::query_as(
        "SELECT id, name, rank FROM appellation_tier WHERE country_id=$1 AND code=$2",
    )
    .bind(country_id)
    .bind(&t.code)
    .fetch_optional(pool)
    .await?;
    match row {
        Some((id, name, rank)) if name != t.name || rank != t.rank => {
            sqlx::query(
                "UPDATE appellation_tier SET name=$1, rank=$2, source='seed', \
                 revision=revision+1, updated_at=now() WHERE id=$3",
            )
            .bind(&t.name)
            .bind(t.rank)
            .bind(&id)
            .execute(pool)
            .await?;
        }
        Some(_) => {}
        None => {
            sqlx::query(
                "INSERT INTO appellation_tier (id, country_id, code, name, rank, source) \
                 VALUES ($1,$2,$3,$4,$5,'seed')",
            )
            .bind(ids.next().0)
            .bind(country_id)
            .bind(&t.code)
            .bind(&t.name)
            .bind(t.rank)
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}

async fn upsert_classification_system(
    pool: &PgPool,
    ids: &IdGen,
    s: &seed::ClassificationSystem,
    country_id: &str,
) -> anyhow::Result<String> {
    let row: Option<SystemRow> = sqlx::query_as(
        "SELECT id, name, scope, established, revised, notes FROM classification_system \
         WHERE country_id=$1 AND code=$2",
    )
        .bind(country_id)
        .bind(&s.code)
        .fetch_optional(pool)
        .await?;
    if let Some((id, name, scope, established, revised, notes)) = row {
        if name != s.name
            || scope != s.scope
            || established != Some(s.established)
            || revised != s.revised
            || notes != s.notes
        {
            sqlx::query(
                "UPDATE classification_system SET name=$1, scope=$2, established=$3, revised=$4, \
                 notes=$5, source='seed', revision=revision+1, updated_at=now() WHERE id=$6",
            )
            .bind(&s.name)
            .bind(&s.scope)
            .bind(s.established)
            .bind(s.revised)
            .bind(s.notes.as_deref())
            .bind(&id)
            .execute(pool)
            .await?;
        }
        return Ok(id);
    }
    let id = ids.next().0;
    sqlx::query(
        "INSERT INTO classification_system (id, country_id, code, name, scope, established, revised, notes, source) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'seed')",
    )
    .bind(&id)
    .bind(country_id)
    .bind(&s.code)
    .bind(&s.name)
    .bind(&s.scope)
    .bind(s.established)
    .bind(s.revised)
    .bind(s.notes.as_deref())
    .execute(pool)
    .await?;
    Ok(id)
}

async fn upsert_classification_level(
    pool: &PgPool,
    ids: &IdGen,
    l: &seed::ClassificationLevel,
    system_id: &str,
) -> anyhow::Result<()> {
    let row: Option<(String, String, i16)> = sqlx::query_as(
        "SELECT id, name, rank FROM classification_level WHERE system_id=$1 AND code=$2",
    )
    .bind(system_id)
    .bind(&l.code)
    .fetch_optional(pool)
    .await?;
    match row {
        Some((id, name, rank)) if name != l.name || rank != l.rank => {
            sqlx::query(
                "UPDATE classification_level SET name=$1, rank=$2, source='seed', \
                 revision=revision+1, updated_at=now() WHERE id=$3",
            )
            .bind(&l.name)
            .bind(l.rank)
            .bind(&id)
            .execute(pool)
            .await?;
        }
        Some(_) => {}
        None => {
            sqlx::query(
                "INSERT INTO classification_level (id, system_id, code, name, rank, source) \
                 VALUES ($1,$2,$3,$4,$5,'seed')",
            )
            .bind(ids.next().0)
            .bind(system_id)
            .bind(&l.code)
            .bind(&l.name)
            .bind(l.rank)
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}

async fn upsert_label_rule(
    pool: &PgPool,
    ids: &IdGen,
    r: &seed::LabelRule,
    country_id: &str,
) -> anyhow::Result<()> {
    let condition = r.condition.as_deref().unwrap_or("default");
    let row: Option<(String, i16, Option<i16>, Option<String>)> = sqlx::query_as(
        "SELECT id, min_percent, tolerance_percent, notes FROM label_rule \
         WHERE country_id=$1 AND kind=$2 AND condition=$3",
    )
    .bind(country_id)
    .bind(&r.kind)
    .bind(condition)
    .fetch_optional(pool)
    .await?;
    if let Some((id, min_percent, tolerance, notes)) = row {
        if min_percent != r.min_percent || tolerance != r.tolerance_percent || notes != r.notes {
            sqlx::query(
                "UPDATE label_rule SET min_percent=$1, tolerance_percent=$2, notes=$3, \
                 source='seed', revision=revision+1, updated_at=now() WHERE id=$4",
            )
            .bind(r.min_percent)
            .bind(r.tolerance_percent)
            .bind(r.notes.as_deref())
            .bind(&id)
            .execute(pool)
            .await?;
        }
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO label_rule (id, country_id, kind, condition, min_percent, tolerance_percent, notes, source) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,'seed')",
    )
    .bind(ids.next().0)
    .bind(country_id)
    .bind(&r.kind)
    .bind(condition)
    .bind(r.min_percent)
    .bind(r.tolerance_percent)
    .bind(r.notes.as_deref())
    .execute(pool)
    .await?;
    Ok(())
}
