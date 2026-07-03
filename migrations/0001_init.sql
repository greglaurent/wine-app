-- Wine app -- initial schema (PostgreSQL).
-- Server is the source of truth; this schema is mirrored to client SQLite
-- (sqlite-wasm-rs) for offline use.
--
-- Sync envelope on every syncable table:
--   id text PK   -- client-generated Lamport id
--   revision     -- per-row counter, bumped on each write
--   updated_at   -- last-write-wins key (Lamport id breaks ties)
--   deleted_at   -- soft-delete tombstone (propagates on sync)
--   server_seq   -- server-authoritative monotonic cursor (delta pulls)
--   created_at
--
-- Reference + catalog tables also carry `source` ('seed' | 'user') so seeded
-- canonical rows are distinguishable from user-added ones.

CREATE EXTENSION IF NOT EXISTS pg_trgm;   -- fuzzy search / autocomplete
CREATE EXTENSION IF NOT EXISTS citext;     -- case-insensitive usernames

CREATE SEQUENCE IF NOT EXISTS sync_seq;

-- Advance the server-side sync cursor on every insert/update.
CREATE OR REPLACE FUNCTION set_server_seq() RETURNS trigger AS $$
BEGIN
  NEW.server_seq := nextval('sync_seq');
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Server instance epoch: a single row set once on a FRESH database (its default
-- fires on init), so it is stable across restarts but NEW after a nuke (fresh
-- volume). Clients compare it and wipe their local store when it changes, so a
-- server nuke auto-resets every offline client. Not synced as a row.
CREATE TABLE server_meta (
  id    boolean PRIMARY KEY DEFAULT true CHECK (id),   -- single-row guard
  epoch text NOT NULL DEFAULT gen_random_uuid()::text
);
INSERT INTO server_meta (id) VALUES (true);

-- Logic-bearing single-value vocabularies (bottle status, location kind, grape
-- color) are NOT enums/CHECKs here. They are generated Rust enums in core::vocab
-- (from crates/core/src/vocab_enums.ron -- the single source), validated in the
-- app and stored as plain text. Wine color/style/sweetness are DESCRIPTIONS: a
-- wine carries several, so they are `descriptor` tags + the `wine_descriptor`
-- junction, not single-value columns.

-- ===================== auth (server-only, not synced) =====================
CREATE TABLE app_user (
  id            text PRIMARY KEY,
  username      citext NOT NULL UNIQUE,
  password_hash text NOT NULL,
  role          text NOT NULL DEFAULT 'admin' CHECK (role IN ('admin','member','readonly')),
  created_at    timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE session (
  id         text PRIMARY KEY,                 -- opaque token
  user_id    text NOT NULL REFERENCES app_user(id) ON DELETE CASCADE,
  expires_at timestamptz NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);

-- ===================== domain 1: geography & labeling =====================
CREATE TABLE country (
  id     text PRIMARY KEY,
  iso2   char(2) NOT NULL UNIQUE,
  iso3   char(3) NOT NULL UNIQUE,
  name   text NOT NULL,
  source text NOT NULL DEFAULT 'user' CHECK (source IN ('seed','user')),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE appellation_type (             -- AVA / AOC / DOCG / Einzellage ...
  id         text PRIMARY KEY,
  country_id text NOT NULL REFERENCES country(id),
  code       text NOT NULL,
  name       text NOT NULL,
  ordinal      smallint NOT NULL,           -- specificity rank within country
  is_legal     boolean NOT NULL DEFAULT true,
  is_composite boolean NOT NULL DEFAULT false, -- US Multi-State / Multi-County
  source       text NOT NULL DEFAULT 'user' CHECK (source IN ('seed','user')),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (country_id, code)
);

CREATE TABLE appellation_tier (             -- quality tier of the land: Grand Cru, Premier Cru ...
  id         text PRIMARY KEY,
  country_id text NOT NULL REFERENCES country(id),
  code       text NOT NULL,
  name       text NOT NULL,
  rank       smallint NOT NULL,             -- 1 = highest
  source     text NOT NULL DEFAULT 'user' CHECK (source IN ('seed','user')),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (country_id, code)
);

CREATE TABLE appellation (                  -- self-nesting: Napa->Oakville
  id                  text PRIMARY KEY,
  country_id          text NOT NULL REFERENCES country(id),
  parent_id           text REFERENCES appellation(id),
  appellation_type_id text NOT NULL REFERENCES appellation_type(id),
  tier_id             text REFERENCES appellation_tier(id),
  name                text NOT NULL,
  source              text NOT NULL DEFAULT 'user' CHECK (source IN ('seed','user')),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE vineyard (                     -- climat / Einzellage / single-vineyard
  id             text PRIMARY KEY,
  country_id     text NOT NULL REFERENCES country(id),
  appellation_id text REFERENCES appellation(id),
  producer_id    text,                       -- FK added after producer exists
  name           text NOT NULL,
  source         text NOT NULL DEFAULT 'user' CHECK (source IN ('seed','user')),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE classification_system (        -- 1855 / Burgundy Cru / Prädikat / VDP
  id         text PRIMARY KEY,
  country_id text NOT NULL REFERENCES country(id),
  code       text NOT NULL,
  name       text NOT NULL,
  notes      text,
  scope      text NOT NULL DEFAULT 'any'    -- red / white / sweet / red_white / any
             CHECK (scope IN ('red','white','sweet','red_white','any')),
  established smallint,                      -- year first established
  revised    smallint,                      -- year of the current edition, if revised
  source     text NOT NULL DEFAULT 'user' CHECK (source IN ('seed','user')),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (country_id, code)
);

CREATE TABLE classification_level (         -- Grand Cru / 1st Growth / Spätlese ...
  id        text PRIMARY KEY,
  system_id text NOT NULL REFERENCES classification_system(id),
  code      text NOT NULL,
  name      text NOT NULL,
  rank      smallint NOT NULL,
  source    text NOT NULL DEFAULT 'user' CHECK (source IN ('seed','user')),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (system_id, code)
);

CREATE TABLE production_method (            -- méthode champenoise / solera / amphora
  id     text PRIMARY KEY,
  code   text NOT NULL UNIQUE,
  name   text NOT NULL,
  source text NOT NULL DEFAULT 'user' CHECK (source IN ('seed','user')),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE certification (                -- organic / biodynamic / vegan / demeter
  id     text PRIMARY KEY,
  code   text NOT NULL UNIQUE,
  name   text NOT NULL,
  source text NOT NULL DEFAULT 'user' CHECK (source IN ('seed','user')),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE bottle_format (                -- half / standard / magnum / jeroboam
  id        text PRIMARY KEY,
  code      text NOT NULL UNIQUE,
  name      text NOT NULL,
  volume_ml integer NOT NULL,
  source    text NOT NULL DEFAULT 'user' CHECK (source IN ('seed','user')),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE descriptor (                   -- multi-select wine tags: red / sparkling / dry / fortified ...
  id       text PRIMARY KEY,
  code     text NOT NULL UNIQUE,
  name     text NOT NULL,
  category text NOT NULL,                    -- grouping label only (color/fizz/sweetness/type); no rules
  source   text NOT NULL DEFAULT 'user' CHECK (source IN ('seed','user')),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE label_rule (                   -- per-country labeling thresholds (US/TTB); drives form validation
  id                text PRIMARY KEY,
  country_id        text NOT NULL REFERENCES country(id),
  kind              text NOT NULL,           -- appellation / varietal / vintage / estate_bottled
  condition         text NOT NULL,           -- AVA / non-AVA / labrusca / default / ...
  min_percent       smallint NOT NULL,
  tolerance_percent smallint,
  notes             text,
  source            text NOT NULL DEFAULT 'user' CHECK (source IN ('seed','user')),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (country_id, kind, condition)
);

CREATE TABLE grape_variety (
  id     text PRIMARY KEY,
  name   text NOT NULL UNIQUE,
  color  text,                              -- core::vocab::GrapeColor code
  source text NOT NULL DEFAULT 'user' CHECK (source IN ('seed','user')),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE grape_clone (                  -- '777' / 'Pommard' / 'Dijon 115'
  id               text PRIMARY KEY,
  grape_variety_id text NOT NULL REFERENCES grape_variety(id),
  code             text NOT NULL,
  name             text,
  rootstock        text,
  source           text NOT NULL DEFAULT 'user' CHECK (source IN ('seed','user')),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (grape_variety_id, code)
);

-- ===================== domain 2: wine catalog =====================
CREATE TABLE producer (
  id         text PRIMARY KEY,
  country_id text NOT NULL REFERENCES country(id),
  name       text NOT NULL,
  is_estate  boolean NOT NULL DEFAULT true,   -- false = négociant
  website    text,
  notes      text,
  source     text NOT NULL DEFAULT 'user' CHECK (source IN ('seed','user')),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE vineyard
  ADD CONSTRAINT vineyard_producer_fk
  FOREIGN KEY (producer_id) REFERENCES producer(id);

CREATE TABLE wine (                         -- vintage-independent label / cuvée
  id                   text PRIMARY KEY,
  producer_id          text NOT NULL REFERENCES producer(id),
  country_id           text NOT NULL REFERENCES country(id),
  appellation_id       text REFERENCES appellation(id),
  vineyard_id          text REFERENCES vineyard(id),
  name                 text NOT NULL,
  is_nv                boolean NOT NULL DEFAULT false,
  production_method_id text REFERENCES production_method(id),
  source               text NOT NULL DEFAULT 'user' CHECK (source IN ('seed','user')),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE wine_vintage (
  id                  text PRIMARY KEY,
  wine_id             text NOT NULL REFERENCES wine(id) ON DELETE CASCADE,
  year                smallint,               -- NULL = true NV
  abv                 numeric(4,2),
  drink_from          smallint,
  drink_until         smallint,
  release_price_cents bigint,
  notes               text,
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (wine_id, year)
);

CREATE TABLE vintage_classification (       -- M:N, multiple systems at once
  id                      text PRIMARY KEY,
  wine_vintage_id         text NOT NULL REFERENCES wine_vintage(id) ON DELETE CASCADE,
  classification_level_id text NOT NULL REFERENCES classification_level(id),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (wine_vintage_id, classification_level_id)
);

CREATE TABLE vintage_certification (        -- M:N
  id               text PRIMARY KEY,
  wine_vintage_id  text NOT NULL REFERENCES wine_vintage(id) ON DELETE CASCADE,
  certification_id text NOT NULL REFERENCES certification(id),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (wine_vintage_id, certification_id)
);

CREATE TABLE wine_descriptor (              -- M:N: a vintage carries a SET of descriptor tags
  id              text PRIMARY KEY,
  wine_vintage_id text NOT NULL REFERENCES wine_vintage(id) ON DELETE CASCADE,
  descriptor_id   text NOT NULL REFERENCES descriptor(id),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (wine_vintage_id, descriptor_id)
);

CREATE TABLE composition (                  -- the clone-level blend, per vintage
  id               text PRIMARY KEY,
  wine_vintage_id  text NOT NULL REFERENCES wine_vintage(id) ON DELETE CASCADE,
  grape_variety_id text NOT NULL REFERENCES grape_variety(id),
  grape_clone_id   text REFERENCES grape_clone(id),
  vineyard_id      text REFERENCES vineyard(id),
  percentage       numeric(5,2),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now()
);

-- ===================== domain 3: physical inventory =====================
CREATE TABLE location (                     -- self-nesting: cellar->rack->shelf->bin
  id        text PRIMARY KEY,
  name      text NOT NULL,
  parent_id text REFERENCES location(id),
  kind      text NOT NULL,                  -- core::vocab::LocationKind code
  grid_rows smallint,
  grid_cols smallint,
  notes     text,
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE lot (
  id                text PRIMARY KEY,
  wine_vintage_id   text NOT NULL REFERENCES wine_vintage(id),
  lot_code          text,                     -- the L-code
  bottling_date     date,
  disgorgement_date date,
  release_date      date,
  notes             text,
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE purchase (                     -- optional acquisition log
  id           text PRIMARY KEY,
  user_id      text NOT NULL REFERENCES app_user(id),
  vendor       text,
  purchased_at date,
  total_cents  bigint,
  currency     char(3) NOT NULL DEFAULT 'USD',
  notes        text,
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE bottle (                       -- ONE physical unit = one row
  id                  text PRIMARY KEY,
  lot_id              text NOT NULL REFERENCES lot(id),
  format_id           text REFERENCES bottle_format(id),
  status              text NOT NULL DEFAULT 'in_cellar',  -- core::vocab::BottleStatus code
  location_id         text REFERENCES location(id),
  position_rack       text,
  position_row        smallint,
  position_column     smallint,
  position_bin        text,
  position_depth      smallint,
  position_label      text,
  purchase_id         text REFERENCES purchase(id),
  purchase_date       date,
  purchase_price_cents bigint,
  currency            char(3) NOT NULL DEFAULT 'USD',
  vendor              text,
  notes               text,
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE consumption (                  -- drinking a bottle (decrements stock)
  id          text PRIMARY KEY,
  bottle_id   text NOT NULL REFERENCES bottle(id),
  user_id     text NOT NULL REFERENCES app_user(id),
  consumed_at timestamptz NOT NULL DEFAULT now(),
  occasion    text,
  companions  text,
  notes       text,
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE bottle_movement (              -- audit trail of moves / status changes
  id               text PRIMARY KEY,
  bottle_id        text NOT NULL REFERENCES bottle(id),
  user_id          text REFERENCES app_user(id),
  moved_at         timestamptz NOT NULL DEFAULT now(),
  from_location_id text REFERENCES location(id),
  to_location_id   text REFERENCES location(id),
  old_status       text,
  new_status       text,
  reason           text,
  note             text,
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now()
);

-- ===================== domain 4: reviews =====================
CREATE TABLE tasting_note (                 -- drinking notes + star review, per user
  id              text PRIMARY KEY,
  user_id         text NOT NULL REFERENCES app_user(id),
  wine_vintage_id text NOT NULL REFERENCES wine_vintage(id),
  bottle_id       text REFERENCES bottle(id),
  consumption_id  text REFERENCES consumption(id),
  star_rating     smallint CHECK (star_rating BETWEEN 1 AND 10),  -- half-stars: 1 = 0.5 .. 10 = 5.0
  score_100       smallint CHECK (score_100 BETWEEN 50 AND 100),
  note            text,
  appearance      text,
  nose            text,
  palate          text,
  finish          text,
  tasted_at       timestamptz,
  is_blind        boolean NOT NULL DEFAULT false,
  would_rebuy     boolean,
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE wishlist (
  id                text PRIMARY KEY,
  user_id           text NOT NULL REFERENCES app_user(id),
  wine_id           text REFERENCES wine(id),
  wine_vintage_id   text REFERENCES wine_vintage(id),
  note              text,
  target_price_cents bigint,
  currency          char(3) NOT NULL DEFAULT 'USD',
  added_at          timestamptz NOT NULL DEFAULT now(),
  revision bigint NOT NULL DEFAULT 1,
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  server_seq bigint NOT NULL DEFAULT nextval('sync_seq'),
  created_at timestamptz NOT NULL DEFAULT now()
);

-- ===================== triggers + sync-cursor indexes =====================
-- Attach the server_seq trigger and a server_seq index to every syncable table.
DO $$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY[
    'country','appellation_type','appellation_tier','appellation','vineyard',
    'classification_system','classification_level','descriptor','label_rule',
    'production_method','certification','bottle_format',
    'grape_variety','grape_clone','producer','wine','wine_vintage',
    'vintage_classification','vintage_certification','wine_descriptor','composition',
    'location','lot','purchase','bottle','consumption','bottle_movement',
    'tasting_note','wishlist'
  ] LOOP
    EXECUTE format(
      'CREATE TRIGGER %I_server_seq BEFORE INSERT OR UPDATE ON %I
         FOR EACH ROW EXECUTE FUNCTION set_server_seq()', t, t);
    EXECUTE format(
      'CREATE INDEX %I_server_seq_idx ON %I (server_seq)', t, t);
  END LOOP;
END $$;

-- ===================== query indexes =====================
-- fuzzy autocomplete (pg_trgm)
CREATE INDEX producer_name_trgm      ON producer      USING gin (name gin_trgm_ops);
CREATE INDEX wine_name_trgm          ON wine          USING gin (name gin_trgm_ops);
CREATE INDEX appellation_name_trgm   ON appellation   USING gin (name gin_trgm_ops);
CREATE INDEX vineyard_name_trgm      ON vineyard      USING gin (name gin_trgm_ops);
CREATE INDEX grape_variety_name_trgm ON grape_variety USING gin (name gin_trgm_ops);

-- hot foreign-key / filter paths
CREATE INDEX bottle_lot_idx          ON bottle (lot_id);
CREATE INDEX bottle_location_idx     ON bottle (location_id);
CREATE INDEX bottle_status_idx       ON bottle (status) WHERE deleted_at IS NULL;
CREATE INDEX lot_vintage_idx         ON lot (wine_vintage_id);
CREATE INDEX wine_producer_idx       ON wine (producer_id);
CREATE INDEX wine_vintage_wine_idx   ON wine_vintage (wine_id);
CREATE INDEX composition_vintage_idx ON composition (wine_vintage_id);
CREATE INDEX consumption_bottle_idx  ON consumption (bottle_id);
CREATE INDEX tasting_note_vintage_idx ON tasting_note (wine_vintage_id);
CREATE INDEX tasting_note_user_idx   ON tasting_note (user_id);
