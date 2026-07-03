# wine-app -- project handoff

Complete context for picking this project up cold (new machine, new agent, or after
a break). Read this first, then the current work-in-progress doc:
[2026-07-03-entry-form-design.md](2026-07-03-entry-form-design.md).

Last updated: 2026-07-03.

---

## 1. What this is

A self-hosted, mobile-friendly, **offline-first** wine cellar tracker. The owner
wants EXTREMELY specific wine data:

- Bottles tracked as individual physical units, with **position** in any location
  (rack / row / column / bin / depth).
- Down to the **lot** (bottling code, disgorgement/release dates).
- Grapes down to the **clone** level, with per-vintage composition/blend.
- **Country-driven labeling**: different countries label differently (US AVAs,
  French AOC + 1855/Saint-Emilion classifications, German Pradikat, etc.), and the
  country drives what appellation types / classifications are even available.
- Drinking notes + reviews (half-star rating, 100-point score, tasting structure).

Deployed via docker-compose on the owner's server; usable on a phone, including
**offline at remote wineries** (hence local-first).

---

## 2. Architecture (LOCKED -- do not re-litigate)

- **Server:** Axum + datastar (hypermedia) + Askama (templates) + **sqlx / Postgres**.
  The sync target and source of truth.
- **Client:** a PWA. A service worker + a dedicated Web Worker run the WASM `core`
  crate + **`sqlite-wasm-rs`** (local SQLite in OPFS) as a local backend, rendering
  the SAME Askama templates the server uses. Datastar works offline because its
  transport is the interceptable Fetch API.
- **One renderer:** `crates/core` is a wasm-safe shared crate (templates, datastar
  event gen, handler/render logic, sync DTOs, Lamport id, vocab, seed). It compiles
  into BOTH the native server and the wasm client. Keep it free of tokio/sqlx/networking.
- **LOCAL-FIRST:** the client's `offline.ts` patches `window.fetch` to ALWAYS route
  datastar requests to the worker (online and offline). The UI always reads/writes
  the local OPFS store; a background sync engine reconciles with the server. This
  means **all writes happen client-side**; the server is a pure sync sink.
- **Ids:** the owner's own `causal-id` crate (Lamport hybrid), pinned to the
  `width-u128` feature so server (native) and client (wasm) pack ids identically.
  Server `source_id = 1`, each client `source_id = 2`. Ids stored as 32-char
  zero-padded hex of the packed integer, so lexicographic order == causal order
  (Rust `Ord` and SQL `ORDER BY` match). Factory resumes past the max persisted tick
  on startup so restarts never reuse ids.
- **Sync model:** client-generated Lamport id PKs; last-write-wins on `updated_at`
  (Lamport id breaks ties); soft deletes (`deleted_at`); per-row `revision`;
  server-authoritative `server_seq` cursor (set by a trigger, one global sequence)
  for delta pulls.

**Rejected alternatives (do NOT re-propose):** Leptos (datastar+Askama cover the
UI), Diesel (its wasm sqlite wrappers are abandoned), sqlx-on-the-client (does not
compile to wasm32), UUIDs (owner insisted on the Lamport id).

`causal-id` comes from a self-hosted **kellnr** registry: name `abmac-io`, index
`sparse+https://kellnr.abmac.io/api/v1/crates/`, **anonymous read (no token)**.
Configured in `.cargo/config.toml`; the Dockerfile copies `.cargo` so the container
build resolves it.

---

## 3. Repository layout

```
crates/core     shared, wasm-safe: templates, views, ids, vocab, seed, sync DTOs, Store trait, model   (compiled into BOTH sides)
  build.rs        codegen: RON -> core::vocab enums (see section 6)
  src/seed/       reference-data RON + loader/validator (section 6)
  src/vocab.rs    + vocab_enums.ron: logic-bearing enums (section 6)
crates/server   Axum + sqlx/Postgres. main.rs (bootstrap), db.rs, state.rs, assets.rs, web.rs, sync.rs, seed.rs
crates/client   wasm32: sqlite FFI + OPFS worker backend. lib.rs (init_db), sqlite.rs, store.rs, sync.rs, handle.rs
migrations/     0001_init.sql -- the single, complete schema (section 5)
web/            Vite + pnpm + TypeScript frontend (section 8)
  src/            offline.ts, worker.ts, sw.ts, types.ts (+ generated wasm/, vendored vendor/datastar.js)
  scripts/vendor.ts   downloads the datastar bundle
docs/plans/     this handoff + design docs
justfile        task runner (section 7)
Dockerfile      3-stage: toolchain -> build -> slim runtime (section 7)
docker-compose.yml + .override.yml (dev) + .prod.yml
.cargo/config.toml   kellnr registry
CLAUDE.md       project conventions (no emojis / non-ASCII in code)
```

---

## 4. Current status

### Done and verified
- **Schema** (`migrations/0001_init.sql`): the complete locked layout; applies clean
  against real Postgres. Four domains (section 5).
- **Reference vocabulary**: three-tier model (section 6), seeded server-side from
  `core::seed` (RON), and **synced down to the offline client** (verified: client
  local store shows 4 countries / 16 descriptors / 9 appellation types / 17
  classification levels, persists across reload).
- **Vocab enums via codegen**: `core/build.rs` generates `core::vocab` enums
  (`BottleStatus`, `LocationKind`, `GrapeColor`) from `crates/core/src/vocab_enums.ron`
  -- edit the RON, rebuild. `StarRating` is a hand-written half-star scale.
- **Server reconciling upsert** (`crates/server/src/seed.rs`): reconciles Postgres to
  `core::seed` on startup; idempotent and CHURN-FREE (UPDATE only on real change, so
  `server_seq` does not move on restart). Verified.
- **Sync**: reference pull (generic, pull-only, `/sync/reference`), bottle
  bidirectional (LWW, `/sync/pull` + `/sync/push`), reset-epoch (`/sync/meta`).
  All verified end-to-end in the browser.
- **Reset-epoch**: a server nuke gives a new `server_meta.epoch`; the client detects
  the change and wipes + re-hydrates its local store automatically. Verified (2 local
  bottles vanished after a nuke; a same-epoch sync does NOT reset).
- **Frontend**: Vite/pnpm/TypeScript, strict typechecked, worker readiness handshake,
  offline-tolerant sync logging (offline = warn not error), split bottle/reference/meta
  sync. Verified.
- **Docker**: consolidated custom toolchain image, ~98 MB runtime, builds clean.
- **Dev-loop fix**: `assets.rs` re-reads the Vite manifest per request in debug builds,
  so a `pnpm build` shows up without restarting the server.

### Temporary scaffolding (to retire)
- `db::ensure_seed` creates a DEMO catalog chain (country->producer->wine->vintage->lot)
  + returns a demo lot id; `web::add_bottle` / client `handle` POST `/bottles` add a
  nameless bottle to that demo lot. This is the current "app" -- a placeholder until
  the real entry form lands.
- `GET /fragments/ref-stats` (client `handle.rs`): a dev diagnostic returning local
  reference-table counts. Handy; remove or repurpose later.

### Not yet built
- **The real entry form** (IN PROGRESS -- see the form design doc). This is next.
- **User-catalog bidirectional sync**: today only `bottle` pushes to the server. The
  catalog tables (producer/wine/wine_vintage/lot/appellation/...) need bidirectional
  sync -- this is Phase 1 of the form work.
- **Auth**: `app_user`/`session` tables exist but are unused. Needed before exposing
  beyond localhost.
- **TLS/deploy (Caddy)**: service workers + OPFS need a secure context (HTTPS or
  localhost), so real phone/LAN use needs TLS.
- **Full reference data**: only a minimal, correct FR + US set is authored (section 6).
  Other countries, the complete classifications, grape varieties, sweetness terms,
  production methods, certifications are not yet seeded.
- **Structured location hierarchy**: the `location` table exists (self-nesting
  cellar->rack->bin) but has no UI; v1 entry uses free-form position fields instead.

---

## 5. Data model (`migrations/0001_init.sql`)

Every syncable table carries the sync envelope: `id text PK` (Lamport), `revision`,
`updated_at`, `deleted_at`, `server_seq` (default `nextval('sync_seq')`, bumped by the
`set_server_seq` trigger), `created_at`. Reference/catalog tables also carry
`source text ('seed'|'user')`.

- **`server_meta`**: single row, `epoch text DEFAULT gen_random_uuid()::text`. Set once
  on a fresh DB; drives the reset-epoch (section 7). NOT synced as a row.
- **Domain 1 -- geography & labeling** (mostly reference vocab):
  `country`, `appellation_type` (kinds: AVA/AOC/... + `is_composite`),
  `appellation_tier` (Grand Cru/Premier Cru/...), `appellation` (INSTANCES: the actual
  regions, self-nesting, `tier_id` optional -- user data), `classification_system`
  (+ `scope`, `established`, `revised`), `classification_level` (tiers),
  `bottle_format`, `descriptor` (multi-select wine tags, section 6),
  `label_rule` (US TTB thresholds), `production_method`, `certification`,
  `grape_variety` (`color` text = `GrapeColor` code), `grape_clone`.
- **Domain 2 -- catalog**: `producer` (`is_estate`), `wine` (producer/country/
  appellation/vineyard refs, `is_nv`; NO color/style columns -- those are descriptors),
  `wine_vintage` (year, abv, drink window; NO sweetness column), `vintage_classification`
  (M:N wine_vintage <-> classification_level), `vintage_certification` (M:N),
  `wine_descriptor` (M:N wine_vintage <-> descriptor -- the multi-tag model),
  `composition` (clone-level blend per vintage).
- **Domain 3 -- physical inventory**: `location` (self-nesting; `kind` text =
  `LocationKind` code), `lot`, `purchase`, `bottle` (ONE physical unit;
  `status` text = `BottleStatus` code; `format_id`; free-form position
  `position_rack/row/column/bin/depth/label`; `location_id` optional; purchase info),
  `consumption`, `bottle_movement` (audit trail).
- **Domain 4 -- reviews**: `tasting_note` (`star_rating smallint 1..10` = half-stars;
  `score_100`; appearance/nose/palate/finish; `is_blind`; `would_rebuy`), `wishlist`.

NOTE: editing `0001` (still done freely -- pre-release, no real data) changes its
sqlx checksum, so it requires a `just nuke` on next bring-up. The reset-epoch means a
nuke now also auto-resets any offline client.

---

## 6. Reference data model -- THREE tiers (important, was iterated hard)

The owner corrected the design several times; the resulting distinction is load-bearing:

1. **Tier 1 -- logic-bearing enums (CODE, generated from RON).** Single-value things
   the app branches on. `crates/core/src/vocab_enums.ron` is the single source;
   `crates/core/build.rs` generates `core::vocab::{BottleStatus, LocationKind,
   GrapeColor}` (variant = PascalCase(code), with `ALL`/`code`/`label`/`from_code`).
   Compiled into both sides, never synced, stored as plain text (no PG enums / CHECKs).
   Discriminator the owner insisted on: **"does code branch on it?"** -> yes -> a
   generated enum. `StarRating` is hand-written (a scale with math, not a value list).
   To change a value set: edit the RON, rebuild.
2. **Tier 2 -- seeded vocabulary (DATA, server-owned, synced down).**
   `crates/core/src/seed/` holds RON files (`files/global.ron` + per-country) authored
   with NATURAL-KEY references (the server assigns Lamport ids at seed time).
   `seed::load()` merges + validates them (dangling-ref/dup-key checks); the server
   upserts idempotently on natural keys (`crates/server/src/seed.rs`). Client is
   PULL-ONLY for `source='seed'` rows. Entities: `country`, `bottle_format`,
   `descriptor` (color/fizz/sweetness/type tags), `appellation_type`, `appellation_tier`,
   `classification_system` (+scope/established/revised), `classification_level`,
   `label_rule`.
3. **Tier 3 -- instances (unbounded USER data).** Individual appellations (Napa Valley,
   Margaux), vineyards, producers, wines, vintages, lots, bottles. `source='user'`,
   Lamport ids, bidirectionally synced. NEVER seeded (the owner: "populating every
   vineyard into a file is bonkers").

Two big model corrections captured here:
- **Definition vs description.** Wine color/style/sweetness are DESCRIPTIONS -- a wine
  carries several at once (a demi-sec sparkling rose = {rose, sparkling, off-dry}) --
  so they are `descriptor` tags (Tier 2) with a `wine_descriptor` many-to-many, NOT
  single-value columns. `category` on a descriptor is a grouping label only (no
  cardinality rules -- "who's going to tag a wine red AND white"). Sweetness is
  SUBJECTIVE, so it is a tag, not a normalized scale (the old `sweetness` table was
  dropped).
- **Seed = closed labeling VOCABULARY, not instances.** Types/tiers/systems/scales,
  never the geographic entities.

**Authored data (FR + US, research-sourced, verified):** 4 countries (US/FR/IT/DE),
bottle formats (split/half/standard/magnum), 16 descriptors, US appellation types
(American/Multi-State/State/Multi-County/County/AVA) + US TTB `label_rule` thresholds
(vintage 95%/85%, varietal 75%/51%, appellation 75%/85% AVA, estate-bottled), FR
appellation types (AOC/IGP/VDF) + tiers (Grand Cru/Premier Cru/Village/Regionale) +
7 classification systems with full tiers (1855 Medoc 5 tiers, 1855 Sauternes 3,
Graves, Saint-Emilion 2022, Cru Bourgeois 2025, Cru Artisan 2023, Provence Cru Classe).
France has NO `label_rule` rows -- French labeling is governed per-appellation by each
AOC's cahier des charges, not country-level thresholds. Champagne's Echelle des Crus
is deliberately omitted, flagged in `france.ron` to revisit. `name` fields carry proper
French accents; codes/keys stay ASCII. Current classification editions are baked in.

---

## 7. Sync + reset details

- **Global cursor.** One `sync_seq` sequence stamps `server_seq` on every syncable row,
  so a single monotonic cursor covers all tables. Clients store per-domain cursors in
  `sync_state`: `cursor` (bottles), `ref_cursor` (reference), `epoch`.
- **Reference pull** (`GET /sync/reference?since=N`, `crates/server/src/sync.rs`
  `reference_pull`): one `UNION ALL` over the 8 reference tables projecting each row's
  business columns to JSON (`jsonb_build_object(...)::text`), ordered by `server_seq`.
  Wire type: `core::sync::RefRow { table, deleted_at, data }` + `RefPullResponse`.
  Client `apply_reference` (client `sync.rs`) does a generic blind upsert (table
  whitelisted, string literals escaped) into local mirror tables. Pull-only.
- **Bottle sync** (bidirectional, LWW): `get_dirty` -> `POST /sync/push` -> `apply_acks`;
  `cursor` -> `GET /sync/pull?since=N` -> `apply_pull`. Typed `SyncBottle`.
- **Reset-epoch**: `GET /sync/meta` -> `MetaResponse { epoch }`. Client `check_epoch`
  runs FIRST in `offline.ts` `sync()`; if the stored epoch differs from the server's
  (and one was stored), it `reset_local()` (drop all data tables incl bottle, re-run
  `ensure_schema`, keep `sync_state`) + zeroes `cursor`/`ref_cursor` + stores the new
  epoch; the following pulls re-hydrate. First run just records the epoch.
- **NOT yet generalized to push:** producer/wine/vintage/lot/appellation etc. Only
  `bottle` pushes. The entry form needs the push side generalized (Phase 1, see the
  form design doc) -- the mirror image of the generic reference pull.

The offline-first sync engine lives in `web/src/offline.ts` `sync()`, split into
`syncMeta()` (epoch), `syncBottles()` (bidirectional), `syncReference()` (pull). Each
is independent (one failing does not block the others) and offline-tolerant.

---

## 8. Build + dev workflow

- **Frontend** (`web/`): pnpm project (NOT npm). Vite (vite-plugin-pwa injectManifest +
  vite-plugin-wasm, `target: esnext`) bundles + content-hashes everything and generates
  `sw.js`. `datastar` is VENDORED (its npm package is deprecated/beta) via
  `scripts/vendor.ts`; pico is pnpm-managed. `pnpm build` runs `tsc --noEmit` (strict)
  then `vite build`. Two tsconfigs (browser `src` vs node `scripts`+`vite.config`).
- **wasm client**: `just build-client` = `cargo build -p wine-client --target
  wasm32-unknown-unknown` + `wasm-bindgen --target bundler --out-dir web/src/wasm`.
- **`just build`** = build-client -> `pnpm build` -> `cargo build -p wine-server`.
- **justfile recipes**: `build`, `build-client`, `check`, `check-client` (the wasm
  client is excluded from default-members, so plain `check`/`clippy`/`test` miss it),
  `typecheck` (frontend), `clippy` (native AND wasm client), `image` (docker build app),
  `up`/`up-d`/`db`/`prod`/`down`/`nuke`/`logs`/`ps`/`psql`, `migrate`/`sqlx-prepare`.
- **Docker**: one `toolchain` stage `FROM rust:1-bookworm` adds clang-19 + node24 +
  pnpm + wasm32 target + `wasm-bindgen-cli 0.2.126`; one `build` stage does
  wasm + server + vite; slim `debian:bookworm-slim` runtime. `just up` = dev
  (`docker-compose.override.yml` bind-mounts `web/dist`).
- **Local dev loop**: `just db` (Postgres on the host) + run the server on the host.
  There is a Claude Preview launch config (`.claude/launch.json`) that runs
  `./target/debug/wine-server` on `127.0.0.1:8099`.

### Hard-won gotchas (READ before debugging)
- **clang-19 required** for the Docker wasm build: sqlite-wasm-rs compiles SQLite to
  wasm32 via clang and the bookworm default clang-14 is too old for its C23 attributes.
  `ENV CC_wasm32_unknown_unknown=clang-19`.
- **pnpm blocks dep build scripts** by default -> `web/pnpm-workspace.yaml allowBuilds`
  + esbuild as a direct devDep.
- **Worker message-drop race (subtle!):** the wasm-heavy worker module evaluates slowly
  (top-level-await on the 2.6 MB wasm import), and messages posted to it BEFORE it
  finishes evaluating are SILENTLY DROPPED. Fixed with a readiness handshake: the worker
  posts `{ready:true}` after `init_db`, and `offline.ts` holds every `call()` behind a
  `workerReady` promise. This bit us for ~2h; do not remove the handshake.
- **Stale Vite manifest in dev**: fixed (`assets.rs current()` re-reads in debug). If you
  ever see the offline.ts patch "silently absent," it is a stale bundle.
- **OPFS cannot be cleared while the worker holds sahpool handles** -- navigate away first
  (or just rely on reset-epoch, which drops tables instead of clearing OPFS).
- **launch.json preview runs the prebuilt binary** -- rebuild before restarting it.
- **Vite minifies + strips comments** -- a comment-only change does not change the content
  hash.
- **`just nuke` wipes ONLY the server DB** (the container volume). The browser's OPFS is a
  SEPARATE database; reset-epoch is what makes a nuke propagate to clients.

---

## 9. Conventions / working style (from the owner + CLAUDE.md)

- **NO GIT OPERATIONS** -- the owner manages all git. Never run git commands. (This
  handoff doc is written but NOT committed by the agent.)
- **Explicit approval before actions** -- do not edit/delete/run without a clear go.
- **No emojis and no non-ASCII typography** (em-dash, arrow, ellipsis, middle-dot) in
  code, comments, or config -- only in end-user display content. Accented letters in
  real wine terms (Medoc, methode) are fine. Sweep: `sed -i 's/\xe2\x80\x94/--/g;
  s/\xe2\x86\x92/->/g; s/\xe2\x80\xa6/.../g; s/\xc2\xb7/-/g'`.
- **Decide WITH the owner.** They are detail-obsessed and will correct over-engineering
  hard ("you are WAY overcomplicating everything"). Research, do not guess. Do not build
  ahead of the agreed step.

---

## 10. Roadmap (suggested order)

1. **Entry form** -- the first real feature. See
   [2026-07-03-entry-form-design.md](2026-07-03-entry-form-design.md). Currently
   PAUSED on the visual direction (the functional design is settled). Phasing:
   (1) generalize user-table push sync, (2) local catalog schema + resolve-or-create
   write handler, (3) the country-driven form template + datastar cascade.
2. **Retire the demo** `ensure_seed` / `demo_lot` once real entry works.
3. **Auth** (`app_user`/`session`) before exposing beyond localhost.
4. **TLS / Caddy** for real phone/LAN use (secure context).
5. **Full reference data** (more countries, complete classifications, grapes, sweetness
   terms, production methods, certifications) -- a data-authoring pass, research-sourced.
6. **Multi-device concerns**: dedup of user catalog rows by natural key (LWW does not
   dedupe by name), structured location hierarchy UI.
