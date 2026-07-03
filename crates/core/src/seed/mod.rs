//! Reference ("seed") data: the closed, rarely-changing labeling VOCABULARY,
//! server-owned and synced down to clients.
//!
//! Three distinct axes (conflating them is what makes forms wrong):
//!
//! - `appellation_type`: the regulatory/geographic KIND of a place name (US: AVA,
//!   County; FR: AOC/AOP, IGP, Vin de France).
//! - `appellation_tier`: a quality tier built into the appellation system; it
//!   classifies the LAND (FR: Grand Cru, Premier Cru).
//! - classification (`classification_system` + `classification_level`): an
//!   ESTATE/producer ranking overlaid on places (FR: 1855, Saint-Emilion).
//!
//! Plus `label_rule`: per-country labeling thresholds that drive form validation
//! (US vintage 95%/85%, varietal 75%, appellation 85% AVA, "Estate Bottled").
//!
//! This is emphatically NOT instances -- individual appellations, vineyards,
//! producers, wines are unbounded user data (`source='user'`), never seeded here.
//!
//! Authored as RON files and referenced by NATURAL KEYS, not row ids: the server
//! assigns the Lamport id at seed time. `load()` parses + merges the files and
//! checks every cross-reference resolves; the server then upserts idempotently
//! keyed on those natural keys. `core` compiles into both the server and the wasm
//! client, so this is the single source of truth; clients receive the rows via sync.

use std::collections::HashSet;

use serde::Deserialize;

// ---- authored shapes (what the RON files contain) ----

#[derive(Debug, Clone, Deserialize)]
pub struct Country {
    pub iso2: String,
    pub iso3: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BottleFormat {
    pub code: String,
    pub name: String,
    pub volume_ml: i32,
}

/// A *kind* of appellation a country recognizes (AVA, County, AOC, IGP, ...).
#[derive(Debug, Clone, Deserialize)]
pub struct AppellationType {
    /// -> Country.iso2
    pub country: String,
    pub code: String,
    pub name: String,
    /// Rough specificity rank within the country (1 = broadest).
    pub ordinal: i16,
    #[serde(default = "yes")]
    pub is_legal: bool,
    /// A composite of several places (US Multi-State / Multi-County), which print
    /// per-unit percentages on the label.
    #[serde(default)]
    pub is_composite: bool,
}

/// A quality tier within a country's appellation system, classifying the land
/// (FR: Grand Cru, Premier Cru, Village, Regionale).
#[derive(Debug, Clone, Deserialize)]
pub struct AppellationTier {
    /// -> Country.iso2
    pub country: String,
    pub code: String,
    pub name: String,
    /// Tier rank within the country (1 = highest).
    pub rank: i16,
}

/// An estate/producer classification scheme (Bordeaux 1855, Saint-Emilion, ...).
#[derive(Debug, Clone, Deserialize)]
pub struct ClassificationSystem {
    /// -> Country.iso2
    pub country: String,
    pub code: String,
    pub name: String,
    /// What the scheme covers: "red", "white", "sweet", "red_white", "any".
    pub scope: String,
    /// Year first established.
    pub established: i16,
    /// Year of the current edition, if it is periodically revised.
    #[serde(default)]
    pub revised: Option<i16>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// A multi-select wine tag (red, sparkling, dry, fortified, ...). `category` is a
/// grouping label only (color/fizz/sweetness/type); it carries no rules. Global
/// (not country-scoped); a wine carries a SET of these via `wine_descriptor`.
#[derive(Debug, Clone, Deserialize)]
pub struct Descriptor {
    pub code: String,
    pub name: String,
    pub category: String,
}

/// A tier within a classification system (First Growth, Grand Cru Classe, ...).
#[derive(Debug, Clone, Deserialize)]
pub struct ClassificationLevel {
    /// -> Country.iso2 (with `system`, identifies the ClassificationSystem)
    pub country: String,
    /// -> ClassificationSystem.code (within country)
    pub system: String,
    pub code: String,
    pub name: String,
    /// Tier rank within the system (1 = highest).
    pub rank: i16,
}

/// A per-country labeling threshold/predicate that drives form validation
/// (e.g. US: a vintage on an AVA label requires >= 95% from that year).
#[derive(Debug, Clone, Deserialize)]
pub struct LabelRule {
    /// -> Country.iso2
    pub country: String,
    /// "appellation" | "varietal" | "vintage" | "estate_bottled".
    pub kind: String,
    /// The case this rule applies to, e.g. "AVA", "non-AVA", "labrusca",
    /// "default"; None when the kind has a single rule.
    #[serde(default)]
    pub condition: Option<String>,
    /// Minimum percentage the rule requires.
    pub min_percent: i16,
    /// Allowed tolerance, where the rule grants one (US composite appellations).
    #[serde(default)]
    pub tolerance_percent: Option<i16>,
    #[serde(default)]
    pub notes: Option<String>,
}

fn yes() -> bool {
    true
}

/// One RON file: any subset of the vocabulary lists (all default to empty), so
/// files can be split by domain or country and merged.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SeedFile {
    countries: Vec<Country>,
    bottle_formats: Vec<BottleFormat>,
    appellation_types: Vec<AppellationType>,
    appellation_tiers: Vec<AppellationTier>,
    classification_systems: Vec<ClassificationSystem>,
    classification_levels: Vec<ClassificationLevel>,
    label_rules: Vec<LabelRule>,
    descriptors: Vec<Descriptor>,
}

/// The merged, validated labeling vocabulary, in dependency order (countries
/// first, since the rest reference them).
#[derive(Debug, Clone)]
pub struct Seed {
    pub countries: Vec<Country>,
    pub bottle_formats: Vec<BottleFormat>,
    pub appellation_types: Vec<AppellationType>,
    pub appellation_tiers: Vec<AppellationTier>,
    pub classification_systems: Vec<ClassificationSystem>,
    pub classification_levels: Vec<ClassificationLevel>,
    pub label_rules: Vec<LabelRule>,
    pub descriptors: Vec<Descriptor>,
}

#[derive(Debug)]
pub enum SeedError {
    Parse(String),
    Duplicate(String),
    DanglingRef(String),
}

impl std::fmt::Display for SeedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SeedError::Parse(m) => write!(f, "seed parse error: {m}"),
            SeedError::Duplicate(m) => write!(f, "duplicate seed key: {m}"),
            SeedError::DanglingRef(m) => write!(f, "unresolved seed reference: {m}"),
        }
    }
}

impl std::error::Error for SeedError {}

/// The embedded RON sources. Globals first, then one file per country.
const FILES: &[&str] = &[
    include_str!("files/global.ron"),
    include_str!("files/us.ron"),
    include_str!("files/france.ron"),
];

/// Parse, merge, and validate the embedded seed vocabulary. Errors if the data
/// is malformed, has duplicate natural keys, or dangling references.
pub fn load() -> Result<Seed, SeedError> {
    let mut m = SeedFile::default();
    for raw in FILES {
        let f: SeedFile = ron::from_str(raw).map_err(|e| SeedError::Parse(e.to_string()))?;
        m.countries.extend(f.countries);
        m.bottle_formats.extend(f.bottle_formats);
        m.appellation_types.extend(f.appellation_types);
        m.appellation_tiers.extend(f.appellation_tiers);
        m.classification_systems.extend(f.classification_systems);
        m.classification_levels.extend(f.classification_levels);
        m.label_rules.extend(f.label_rules);
        m.descriptors.extend(f.descriptors);
    }
    resolve(m)
}

fn resolve(f: SeedFile) -> Result<Seed, SeedError> {
    // countries: unique by iso2.
    let mut countries: HashSet<&str> = HashSet::new();
    for c in &f.countries {
        if !countries.insert(c.iso2.as_str()) {
            return Err(SeedError::Duplicate(format!("country {}", c.iso2)));
        }
    }

    // bottle_formats: unique by code (global, no FK).
    let mut formats: HashSet<&str> = HashSet::new();
    for b in &f.bottle_formats {
        if !formats.insert(b.code.as_str()) {
            return Err(SeedError::Duplicate(format!("bottle_format {}", b.code)));
        }
    }

    // appellation_types: country exists; unique by (country, code).
    let mut atypes: HashSet<(&str, &str)> = HashSet::new();
    for t in &f.appellation_types {
        require_country(&countries, &t.country, "appellation_type", &t.code)?;
        if !atypes.insert((t.country.as_str(), t.code.as_str())) {
            return Err(SeedError::Duplicate(format!(
                "appellation_type {}/{}",
                t.country, t.code
            )));
        }
    }

    // appellation_tiers: country exists; unique by (country, code).
    let mut tiers: HashSet<(&str, &str)> = HashSet::new();
    for t in &f.appellation_tiers {
        require_country(&countries, &t.country, "appellation_tier", &t.code)?;
        if !tiers.insert((t.country.as_str(), t.code.as_str())) {
            return Err(SeedError::Duplicate(format!(
                "appellation_tier {}/{}",
                t.country, t.code
            )));
        }
    }

    // classification_systems: country exists; unique by (country, code).
    let mut systems: HashSet<(&str, &str)> = HashSet::new();
    for s in &f.classification_systems {
        require_country(&countries, &s.country, "classification_system", &s.code)?;
        if !systems.insert((s.country.as_str(), s.code.as_str())) {
            return Err(SeedError::Duplicate(format!(
                "classification_system {}/{}",
                s.country, s.code
            )));
        }
    }

    // classification_levels: system exists; unique by (country, system, code).
    let mut levels: HashSet<(&str, &str, &str)> = HashSet::new();
    for l in &f.classification_levels {
        if !systems.contains(&(l.country.as_str(), l.system.as_str())) {
            return Err(SeedError::DanglingRef(format!(
                "classification_level {}/{}/{} -> system {}/{}",
                l.country, l.system, l.code, l.country, l.system
            )));
        }
        if !levels.insert((l.country.as_str(), l.system.as_str(), l.code.as_str())) {
            return Err(SeedError::Duplicate(format!(
                "classification_level {}/{}/{}",
                l.country, l.system, l.code
            )));
        }
    }

    // label_rules: country exists; unique by (country, kind, condition).
    let mut rules: HashSet<(&str, &str, Option<&str>)> = HashSet::new();
    for r in &f.label_rules {
        require_country(&countries, &r.country, "label_rule", &r.kind)?;
        let key = (r.country.as_str(), r.kind.as_str(), r.condition.as_deref());
        if !rules.insert(key) {
            return Err(SeedError::Duplicate(format!(
                "label_rule {}/{}/{}",
                r.country,
                r.kind,
                r.condition.as_deref().unwrap_or("-")
            )));
        }
    }

    // descriptors: unique by code (global, no FK).
    let mut descriptors: HashSet<&str> = HashSet::new();
    for d in &f.descriptors {
        if !descriptors.insert(d.code.as_str()) {
            return Err(SeedError::Duplicate(format!("descriptor {}", d.code)));
        }
    }

    Ok(Seed {
        countries: f.countries,
        bottle_formats: f.bottle_formats,
        appellation_types: f.appellation_types,
        appellation_tiers: f.appellation_tiers,
        classification_systems: f.classification_systems,
        classification_levels: f.classification_levels,
        label_rules: f.label_rules,
        descriptors: f.descriptors,
    })
}

fn require_country(
    countries: &HashSet<&str>,
    country: &str,
    what: &str,
    code: &str,
) -> Result<(), SeedError> {
    if countries.contains(country) {
        Ok(())
    } else {
        Err(SeedError::DanglingRef(format!(
            "{what} {country}/{code} -> country {country}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn us() -> Country {
        Country {
            iso2: "US".into(),
            iso3: "USA".into(),
            name: "United States".into(),
        }
    }

    fn ava() -> AppellationType {
        AppellationType {
            country: "US".into(),
            code: "AVA".into(),
            name: "American Viticultural Area".into(),
            ordinal: 6,
            is_legal: true,
            is_composite: false,
        }
    }

    #[test]
    fn seed_vocabulary_is_coherent() {
        let seed = load().expect("seed must parse and resolve every reference");
        assert!(!seed.countries.is_empty());
        assert!(!seed.bottle_formats.is_empty());
        assert!(!seed.appellation_types.is_empty());
        assert!(!seed.appellation_tiers.is_empty());
        assert!(!seed.classification_systems.is_empty());
        assert!(!seed.classification_levels.is_empty());
        assert!(!seed.label_rules.is_empty());
        assert!(!seed.descriptors.is_empty());
        // Every classification level points at a system that exists.
        let systems: HashSet<(&str, &str)> = seed
            .classification_systems
            .iter()
            .map(|s| (s.country.as_str(), s.code.as_str()))
            .collect();
        for l in &seed.classification_levels {
            assert!(
                systems.contains(&(l.country.as_str(), l.system.as_str())),
                "level {}/{}/{} has no system",
                l.country,
                l.system,
                l.code
            );
        }
    }

    #[test]
    fn rejects_dangling_country() {
        let f = SeedFile {
            appellation_types: vec![ava()],
            ..Default::default()
        };
        assert!(matches!(resolve(f), Err(SeedError::DanglingRef(_))));
    }

    #[test]
    fn rejects_dangling_system() {
        let f = SeedFile {
            countries: vec![us()],
            classification_levels: vec![ClassificationLevel {
                country: "US".into(),
                system: "nope".into(),
                code: "1".into(),
                name: "One".into(),
                rank: 1,
            }],
            ..Default::default()
        };
        assert!(matches!(resolve(f), Err(SeedError::DanglingRef(_))));
    }

    #[test]
    fn rejects_duplicate_type() {
        let f = SeedFile {
            countries: vec![us()],
            appellation_types: vec![ava(), ava()],
            ..Default::default()
        };
        assert!(matches!(resolve(f), Err(SeedError::Duplicate(_))));
    }

    #[test]
    fn rejects_duplicate_label_rule() {
        let rule = || LabelRule {
            country: "US".into(),
            kind: "vintage".into(),
            condition: Some("AVA".into()),
            min_percent: 95,
            tolerance_percent: None,
            notes: None,
        };
        let f = SeedFile {
            countries: vec![us()],
            label_rules: vec![rule(), rule()],
            ..Default::default()
        };
        assert!(matches!(resolve(f), Err(SeedError::Duplicate(_))));
    }
}
