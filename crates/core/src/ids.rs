//! Identifier type.
//!
//! IDs are client-generated Lamport hybrid causal ids from the user's
//! `causal-id` crate, so records created offline get their permanent id
//! on-device, and the embedded Lamport tick breaks last-write-wins ties.
//!
//! `causal-id` is pinned to **width-u128** (see Cargo.toml) so the packed id
//! layout is identical on the native server and the wasm client -- ids from
//! either side are directly comparable, no sync-time remapping.
//!
//! The DB column is `text`. We store the id's packed integer as fixed-width
//! zero-padded hex (32 chars = u128), so lexicographic order -- Rust `Ord` on
//! the string AND SQL `ORDER BY id` -- is identical to causal order. (The human
//! `Display` form `tick:source` would sort `"10:1"` before `"2:1"`, wrong for
//! the LWW tiebreak.)

use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use causal_id::{CausalId, IdFactory, PTR_WIDTH_BITS};

/// Bits reserved for the source identifier (device/server). The remaining
/// high bits hold the monotonic Lamport tick.
pub const SOURCE_BITS: u32 = 16;
/// Bits for the Lamport tick (the rest of the pointer width -- 112 at u128).
pub const TICK_BITS: u32 = PTR_WIDTH_BITS - SOURCE_BITS;

type Factory = IdFactory<TICK_BITS, SOURCE_BITS>;
type RawId = CausalId<TICK_BITS, SOURCE_BITS>;

/// Hex width sized to u128 -- the encoding is sort-preserving and a stable
/// column width.
const HEX_WIDTH: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Id(pub String);

impl Id {
    /// Encode a `CausalId` as zero-padded hex of its packed integer, so
    /// lexicographic order (Rust `Ord` / SQL `ORDER BY id`) == causal order.
    pub fn from_causal(id: RawId) -> Self {
        Id(format!("{:0w$x}", id.to_raw(), w = HEX_WIDTH))
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Thread-safe id generator. `causal-id`'s `AtomicIdFactory` isn't available at
/// width-u128 (no `AtomicU128`), so we put the non-atomic factory behind a
/// mutex. Id generation is not a hot path, so the lock is free in practice.
pub struct IdGen(Mutex<Factory>);

impl IdGen {
    /// `source_id` identifies this device/server (distinct per node).
    pub fn new(source_id: u128) -> Self {
        IdGen(Mutex::new(Factory::new(source_id)))
    }

    /// Resume after a previously-used tick (e.g. the max id persisted in the
    /// DB), so a restart continues past it instead of reusing ids from tick 1.
    pub fn resume(source_id: u128, tick: u128) -> Self {
        IdGen(Mutex::new(Factory::resume(source_id, tick)))
    }

    /// Mint the next causally-ordered id.
    pub fn next(&self) -> Id {
        let mut f = self.0.lock().expect("id factory mutex poisoned");
        Id::from_causal(f.next_id().expect("causal-id Lamport tick overflow"))
    }
}
