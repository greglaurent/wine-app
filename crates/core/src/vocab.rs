//! Tier-1 vocabulary: small, single-valued, logic-bearing types.
//!
//! `BottleStatus`, `LocationKind`, and `GrapeColor` are GENERATED at build time
//! from `src/vocab_enums.ron` (the single source of truth -- edit the RON and
//! rebuild). They are DEFINITIONS (one value -- a bottle IS in_cellar), compiled
//! into both the server and the wasm client, so they never sync; they are stored
//! as plain text and validated by the enum (`from_code`), and `ALL`/`label` feed
//! the form dropdowns. App logic matches on them.
//!
//! `StarRating` is hand-written below because it is a scale with math, not a value
//! list. Multi-valued wine descriptors (color/style/sweetness) are NOT here -- those
//! are descriptions: seeded `descriptor` tags + a many-to-many to the wine. See
//! [`crate::seed::Descriptor`].

// The generated enums (see build.rs).
include!(concat!(env!("OUT_DIR"), "/vocab_enums.rs"));

/// A 0.5-step rating, stored as half-stars: `1` = 0.5 stars .. `10` = 5.0 stars.
/// Integer math, no floats in storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StarRating(u8);

impl StarRating {
    /// Lowest selectable value (0.5 stars).
    pub const MIN_HALF_STARS: u8 = 1;
    /// Highest selectable value (5.0 stars).
    pub const MAX_HALF_STARS: u8 = 10;

    /// Build from a half-star count; `None` if outside 1..=10.
    pub fn from_half_stars(half_stars: u8) -> Option<Self> {
        if (Self::MIN_HALF_STARS..=Self::MAX_HALF_STARS).contains(&half_stars) {
            Some(StarRating(half_stars))
        } else {
            None
        }
    }

    /// The stored value (1..=10).
    pub fn half_stars(self) -> u8 {
        self.0
    }

    /// The display value in stars (0.5 .. 5.0).
    pub fn stars(self) -> f32 {
        self.0 as f32 / 2.0
    }

    /// The ten selectable steps, ascending -- feeds the rating widget.
    pub fn steps() -> impl Iterator<Item = StarRating> {
        (Self::MIN_HALF_STARS..=Self::MAX_HALF_STARS).map(StarRating)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_enums_round_trip() {
        for &s in BottleStatus::ALL {
            assert_eq!(BottleStatus::from_code(s.code()), Some(s));
            assert!(!s.label().is_empty());
        }
        for &k in LocationKind::ALL {
            assert_eq!(LocationKind::from_code(k.code()), Some(k));
        }
        for &c in GrapeColor::ALL {
            assert_eq!(GrapeColor::from_code(c.code()), Some(c));
        }
        // Variant names are derived from codes.
        assert_eq!(BottleStatus::InCellar.code(), "in_cellar");
        assert_eq!(LocationKind::Offsite.label(), "Off-site");
        assert_eq!(BottleStatus::from_code("nonsense"), None);
    }

    #[test]
    fn star_rating_scale() {
        assert!(StarRating::from_half_stars(0).is_none());
        assert!(StarRating::from_half_stars(11).is_none());
        assert_eq!(StarRating::from_half_stars(1).unwrap().stars(), 0.5);
        assert_eq!(StarRating::from_half_stars(7).unwrap().stars(), 3.5);
        assert_eq!(StarRating::from_half_stars(10).unwrap().stars(), 5.0);
        assert_eq!(StarRating::steps().count(), 10);
    }
}
